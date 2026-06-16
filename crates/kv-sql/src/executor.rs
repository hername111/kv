use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use kv_common::error::{KvError, KvResult};
use kv_common::traits::{CommandHandler, StorageEngine};
use kv_common::types::*;
use crate::ast::{Expr, SelectItem};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::planner::{PlanNode, Planner};

pub struct SqlExecutor {
    storage: Arc<dyn StorageEngine>,
    tables: Mutex<HashMap<String, TableMeta>>,
}

impl SqlExecutor {
    pub fn new(storage: Arc<dyn StorageEngine>) -> Self {
        SqlExecutor { storage, tables: Mutex::new(HashMap::new()) }
    }

    pub async fn execute_sql(&self, sql: &str, session: &Session) -> KvResult<ResultSet> {
        let mut lexer = Lexer::new(sql);
        let tokens = lexer.tokenize()?;
        let stmt = Parser::new(tokens).parse_statement()?;
        let plan = Planner::plan(stmt)?;
        self.execute_plan(plan, session).await
    }

    async fn execute_plan(&self, plan: PlanNode, session: &Session) -> KvResult<ResultSet> {
        match plan {
            PlanNode::Projection { source, columns } => {
                let inner = Box::pin(self.execute_plan(*source, session)).await?;
                let col_defs = self.project_columns(&inner.columns, &columns);
                Ok(ResultSet { columns: col_defs, rows: inner.rows, affected_rows: 0, last_insert_id: None })
            }
            PlanNode::SeqScan { table, columns: _, filter } => {
                let meta = {
                    let tables = self.tables.lock().unwrap();
                    tables.get(&table).cloned()
                        .ok_or_else(|| KvError::TableNotFound(table.clone()))?
                };
                let col_names: Vec<String> = meta.columns.iter().map(|c| c.name.clone()).collect();
                let txn_id = session.txn_id.unwrap_or(0);
                let all = self.storage.scan(meta.table_id, &[], &[255u8; 32], txn_id).await?;
                let mut rows = Vec::new();
                for (_, val) in &all {
                    if let Ok(row) = kv_storage::codec::deserialize_row(val) {
                        let include = match &filter {
                            Some(pred) => pred.evaluate(&row, &col_names) == Some(Value::Bool(true)),
                            None => true,
                        };
                        if include { rows.push(row); }
                    }
                }
                Ok(ResultSet::with_rows(meta.columns.clone(), rows))
            }
            PlanNode::Filter { source, predicate } => {
                let inner = Box::pin(self.execute_plan(*source, session)).await?;
                let col_names: Vec<String> = inner.columns.iter().map(|c| c.name.clone()).collect();
                let rows: Vec<Row> = inner.rows.into_iter()
                    .filter(|r| predicate.evaluate(r, &col_names) == Some(Value::Bool(true)))
                    .collect();
                Ok(ResultSet::with_rows(inner.columns, rows))
            }
            PlanNode::Insert { table, columns: _, rows: value_rows } => {
                let meta = {
                    let tables = self.tables.lock().unwrap();
                    tables.get(&table).cloned()
                        .ok_or_else(|| KvError::TableNotFound(table.clone()))?
                };
                let txn_id = session.txn_id.unwrap_or(0);
                let mut count = 0u64;
                let mut last_id = None;
                for exprs in &value_rows {
                    let values: Vec<Value> = exprs.iter().map(|e| match e {
                        Expr::LiteralInt(i) => Value::Int(*i),
                        Expr::LiteralFloat(f) => Value::Float(*f),
                        Expr::LiteralString(s) => Value::String(s.clone()),
                        Expr::LiteralBool(b) => Value::Bool(*b),
                        Expr::LiteralNull => Value::Null,
                        _ => Value::Null,
                    }).collect();
                    let row = Row::new(values);
                    let key = self.encode_key(&row, &meta)?;
                    let val = kv_storage::codec::serialize_row(&row);
                    last_id = Some(self.storage.put(meta.table_id, &key, &val, txn_id).await?);
                    count += 1;
                }
                Ok(ResultSet::ok(count, last_id))
            }
            PlanNode::Update { table, sets, filter } => {
                let meta = {
                    let tables = self.tables.lock().unwrap();
                    tables.get(&table).cloned()
                        .ok_or_else(|| KvError::TableNotFound(table.clone()))?
                };
                let txn_id = session.txn_id.unwrap_or(0);
                let all = self.storage.scan(meta.table_id, &[], &[255u8; 32], txn_id).await?;
                let col_names: Vec<String> = meta.columns.iter().map(|c| c.name.clone()).collect();
                let mut count = 0u64;
                for (key, val) in &all {
                    let mut row = kv_storage::codec::deserialize_row(val)
                        .map_err(|e| KvError::Internal(e.to_string()))?;
                    let matches = match &filter {
                        Some(pred) => pred.evaluate(&row, &col_names) == Some(Value::Bool(true)),
                        None => true,
                    };
                    if matches {
                        for (col, expr) in &sets {
                            if let Some(idx) = col_names.iter().position(|c| c.eq_ignore_ascii_case(col)) {
                                let new_val = expr.evaluate(&row, &col_names).unwrap_or(Value::Null);
                                if idx < row.values.len() { row.values[idx] = new_val; }
                            }
                        }
                        let new_val = kv_storage::codec::serialize_row(&row);
                        self.storage.put(meta.table_id, key, &new_val, txn_id).await?;
                        count += 1;
                    }
                }
                Ok(ResultSet::ok(count, None))
            }
            PlanNode::Delete { table, filter } => {
                let meta = {
                    let tables = self.tables.lock().unwrap();
                    tables.get(&table).cloned()
                        .ok_or_else(|| KvError::TableNotFound(table.clone()))?
                };
                let txn_id = session.txn_id.unwrap_or(0);
                let all = self.storage.scan(meta.table_id, &[], &[255u8; 32], txn_id).await?;
                let col_names: Vec<String> = meta.columns.iter().map(|c| c.name.clone()).collect();
                let mut count = 0u64;
                for (key, val) in &all {
                    let row = kv_storage::codec::deserialize_row(val)
                        .map_err(|e| KvError::Internal(e.to_string()))?;
                    let matches = match &filter {
                        Some(pred) => pred.evaluate(&row, &col_names) == Some(Value::Bool(true)),
                        None => true,
                    };
                    if matches {
                        self.storage.delete(meta.table_id, key, txn_id).await?;
                        count += 1;
                    }
                }
                Ok(ResultSet::ok(count, None))
            }
            PlanNode::CreateTable { name, columns, primary_key } => {
                let pk_idx = columns.iter().position(|c| c.name == primary_key).unwrap_or(0);
                let meta = TableMeta {
                    table_id: 1u64,
                    table_name: name.clone(),
                    columns: columns.clone(),
                    primary_key_index: pk_idx,
                    indexes: Vec::new(),
                };
                self.tables.lock().unwrap().insert(name, meta);
                Ok(ResultSet::ok(0, None))
            }
            PlanNode::CreateIndex { name: _, table, column: _ } => {
                let meta = {
                    let tables = self.tables.lock().unwrap();
                    tables.get(&table).cloned()
                        .ok_or_else(|| KvError::TableNotFound(table.clone()))?
                };
                self.storage.create_index(meta.table_id, 1).await?;
                Ok(ResultSet::ok(0, None))
            }
            PlanNode::DropTable { name } => {
                self.tables.lock().unwrap().remove(&name);
                Ok(ResultSet::ok(0, None))
            }
            _ => Err(KvError::NotImplemented("plan node".to_string())),
        }
    }

    fn encode_key(&self, row: &Row, meta: &TableMeta) -> KvResult<Vec<u8>> {
        let pk_idx = meta.primary_key_index;
        if pk_idx >= row.values.len() {
            return Err(KvError::Internal("primary key index out of range".to_string()));
        }
        Ok(format!("{}", row.values[pk_idx]).into_bytes())
    }

    fn project_columns(&self, all_cols: &[ColumnDef], items: &[SelectItem]) -> Vec<ColumnDef> {
        use crate::ast::SelectItem;
        if items.iter().any(|i| matches!(i, SelectItem::Star)) {
            return all_cols.to_vec();
        }
        items.iter().filter_map(|item| {
            let name = match item {
                SelectItem::Column(n) | SelectItem::Alias(n, _) => n.clone(),
                SelectItem::Star => return None,
            };
            all_cols.iter().find(|c| c.name.eq_ignore_ascii_case(&name)).cloned()
        }).collect()
    }
}

#[async_trait]
impl CommandHandler for SqlExecutor {
    async fn execute(&self, sql: &str, session: &Session) -> KvResult<ResultSet> {
        self.execute_sql(sql, session).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockStorage { data: Mutex<HashMap<u64, Vec<(Vec<u8>, Vec<u8>)>>> }

    #[async_trait]
    impl StorageEngine for MockStorage {
        async fn put(&self, tid: TableId, key: &[u8], value: &[u8], _txn: u64) -> KvResult<u64> {
            self.data.lock().unwrap().entry(tid).or_default().push((key.to_vec(), value.to_vec()));
            Ok(1)
        }
        async fn get(&self, tid: TableId, key: &[u8], _txn: u64) -> KvResult<Option<Vec<u8>>> {
            Ok(self.data.lock().unwrap().get(&tid).and_then(|v| v.iter().find(|(k,_)| k==key).map(|(_,v)| v.clone())))
        }
        async fn scan(&self, tid: TableId, _start: &[u8], _end: &[u8], _txn: u64) -> KvResult<Vec<(Vec<u8>, Vec<u8>)>> {
            Ok(self.data.lock().unwrap().get(&tid).cloned().unwrap_or_default())
        }
        async fn delete(&self, tid: TableId, key: &[u8], _txn: u64) -> KvResult<()> {
            self.data.lock().unwrap().get_mut(&tid).map(|v| v.retain(|(k,_)| k!=key));
            Ok(())
        }
        async fn create_index(&self, _tid: TableId, _col: ColumnId) -> KvResult<IndexId> { Ok(1) }
        async fn index_lookup(&self, _iid: IndexId, _key: &[u8], _txn: u64) -> KvResult<Vec<Vec<u8>>> { Ok(vec![]) }
    }

    #[tokio::test]
    async fn test_executor_insert_select() {
        let storage = Arc::new(MockStorage { data: Mutex::new(HashMap::new()) });
        let exec = SqlExecutor::new(storage);
        exec.tables.lock().unwrap().insert("t".to_string(), TableMeta {
            table_id: 1, table_name: "t".to_string(),
            columns: vec![ColumnDef { id: 1, name: "id".to_string(), data_type: DataType::Int, nullable: false, is_primary_key: true }],
            primary_key_index: 0, indexes: vec![],
        });
        let session = Session::new();
        let rs = exec.execute_sql("INSERT INTO t VALUES (1)", &session).await.unwrap();
        assert_eq!(rs.affected_rows, 1);
        let rs = exec.execute_sql("SELECT * FROM t", &session).await.unwrap();
        assert_eq!(rs.rows.len(), 1);
    }
}
