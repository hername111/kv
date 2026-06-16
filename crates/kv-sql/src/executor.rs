use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use kv_common::error::{KvError, KvResult};
use kv_common::traits::{CommandHandler, StorageEngine};
use kv_common::types::*;
use kv_txn::lock::{LockManager, LockMode};
use kv_txn::manager::TxnManager;
use crate::ast::{Expr, SelectItem};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::planner::{PlanNode, Planner};

/// Per-transaction write buffer: stores pending operations before commit
#[derive(Default)]
struct TxnBuffer {
    inserts: HashMap<u64, Vec<(Vec<u8>, Vec<u8>)>>,  // table_id -> (key, value)
    deletes: HashMap<u64, Vec<Vec<u8>>>,               // table_id -> keys
}

pub struct SqlExecutor {
    storage: Arc<dyn StorageEngine>,
    tables: Mutex<HashMap<String, TableMeta>>,
    txn_manager: Mutex<TxnManager>,
    lock_manager: LockManager,
    txn_buffers: Mutex<HashMap<u64, TxnBuffer>>,
}

impl SqlExecutor {
    pub fn new(storage: Arc<dyn StorageEngine>) -> Self {
        SqlExecutor {
            storage,
            tables: Mutex::new(HashMap::new()),
            txn_manager: Mutex::new(TxnManager::new()),
            lock_manager: LockManager::new(5000),
            txn_buffers: Mutex::new(HashMap::new()),
        }
    }

    fn acquire_lock(&self, txn_id: u64, table: &str, mode: LockMode) -> KvResult<()> {
        let result = match mode {
            LockMode::Shared => self.lock_manager.try_lock_shared(txn_id, table),
            LockMode::Exclusive => self.lock_manager.try_lock_exclusive(txn_id, table),
        };
        result.map_err(|e| KvError::Internal(format!("lock error: {}", e)))
    }

    fn buffer_insert(&self, txn_id: u64, table_id: u64, key: Vec<u8>, value: Vec<u8>) {
        self.txn_buffers.lock().unwrap()
            .entry(txn_id).or_default()
            .inserts.entry(table_id).or_default()
            .push((key, value));
    }

    fn buffer_delete(&self, txn_id: u64, table_id: u64, key: Vec<u8>) {
        self.txn_buffers.lock().unwrap()
            .entry(txn_id).or_default()
            .deletes.entry(table_id).or_default()
            .push(key);
    }

    fn get_txn_buffer(&self, txn_id: u64) -> TxnBuffer {
        self.txn_buffers.lock().unwrap().remove(&txn_id).unwrap_or_default()
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
                let txn_id = session.txn_id().unwrap_or(0);
                if txn_id > 0 {
                    self.acquire_lock(txn_id, &table, LockMode::Shared)?;
                }
                let mut all = self.storage.scan(meta.table_id, &[], &[255u8; 32], txn_id).await?;

                // If inside a transaction, merge buffered writes into scan results
                if txn_id > 0 {
                    let buffers = self.txn_buffers.lock().unwrap();
                    if let Some(buf) = buffers.get(&txn_id) {
                        // Remove deleted keys from scan results
                        if let Some(deletes) = buf.deletes.get(&meta.table_id) {
                            all.retain(|(k, _)| !deletes.contains(k));
                        }
                    }
                    drop(buffers);
                }

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

                // Add buffered inserts if inside a transaction
                if txn_id > 0 {
                    let buffers = self.txn_buffers.lock().unwrap();
                    if let Some(buf) = buffers.get(&txn_id) {
                        if let Some(inserts) = buf.inserts.get(&meta.table_id) {
                            for (_, val) in inserts {
                                if let Ok(row) = kv_storage::codec::deserialize_row(val) {
                                    let include = match &filter {
                                        Some(pred) => pred.evaluate(&row, &col_names) == Some(Value::Bool(true)),
                                        None => true,
                                    };
                                    if include { rows.push(row); }
                                }
                            }
                        }
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
                let txn_id = session.txn_id().unwrap_or(0);
                if txn_id > 0 {
                    self.acquire_lock(txn_id, &table, LockMode::Exclusive)?;
                }
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
                    if txn_id > 0 {
                        self.buffer_insert(txn_id, meta.table_id, key, val);
                    } else {
                        last_id = Some(self.storage.put(meta.table_id, &key, &val, txn_id).await?);
                    }
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
                let txn_id = session.txn_id().unwrap_or(0);
                if txn_id > 0 {
                    self.acquire_lock(txn_id, &table, LockMode::Exclusive)?;
                }
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
                        if txn_id > 0 {
                            self.buffer_insert(txn_id, meta.table_id, key.clone(), new_val);
                        } else {
                            self.storage.put(meta.table_id, key, &new_val, txn_id).await?;
                        }
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
                let txn_id = session.txn_id().unwrap_or(0);
                if txn_id > 0 {
                    self.acquire_lock(txn_id, &table, LockMode::Exclusive)?;
                }
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
                        if txn_id > 0 {
                            self.buffer_delete(txn_id, meta.table_id, key.clone());
                        } else {
                            self.storage.delete(meta.table_id, key, txn_id).await?;
                        }
                        count += 1;
                    }
                }
                Ok(ResultSet::ok(count, None))
            }
            PlanNode::CreateTable { name, columns, primary_key } => {
                let pk_idx = columns.iter().position(|c| c.name == primary_key).unwrap_or(0);
                let table_id = self.storage.create_table(&name).await?;
                let meta = TableMeta {
                    table_id,
                    table_name: name.clone(),
                    columns: columns.clone(),
                    primary_key_index: pk_idx,
                    indexes: Vec::new(),
                };
                self.tables.lock().unwrap().insert(name, meta);
                Ok(ResultSet::ok(0, None))
            }
            PlanNode::CreateIndex { name: _, table, column } => {
                let meta = {
                    let tables = self.tables.lock().unwrap();
                    tables.get(&table).cloned()
                        .ok_or_else(|| KvError::TableNotFound(table.clone()))?
                };
                let col_idx = meta.columns.iter()
                    .position(|c| c.name.eq_ignore_ascii_case(&column))
                    .ok_or_else(|| KvError::Internal(format!("column {} not found", column)))?;
                let index_id = self.storage.create_index(meta.table_id, meta.columns[col_idx].id).await?;
                // Scan base table and build the index
                let txn_id = session.txn_id().unwrap_or(0);
                let all = self.storage.scan(meta.table_id, &[], &[255u8; 32], txn_id).await?;
                // Downcast to access build_index
                if let Some(ks) = self.storage.as_any().downcast_ref::<kv_storage::KvStorage>() {
                    ks.build_index(index_id, meta.table_id, col_idx, &all).await?;
                }
                // Record index in table metadata
                {
                    let mut tables = self.tables.lock().unwrap();
                    if let Some(tm) = tables.get_mut(&table) {
                        tm.indexes.push(kv_common::types::IndexMeta {
                            index_id,
                            name: table.clone() + "_idx",
                            table_id: meta.table_id,
                            column_id: meta.columns[col_idx].id,
                            is_unique: false,
                        });
                    }
                }
                Ok(ResultSet::ok(0, None))
            }
            PlanNode::IndexScan { table, index: _, key } => {
                let meta = {
                    let tables = self.tables.lock().unwrap();
                    tables.get(&table).cloned()
                        .ok_or_else(|| KvError::TableNotFound(table.clone()))?
                };
                let txn_id = session.txn_id().unwrap_or(0);
                // Find the index ID from metadata
                let index_id = meta.indexes.first()
                    .map(|idx| idx.index_id)
                    .ok_or_else(|| KvError::Internal("no index found".to_string()))?;
                let pk_list = self.storage.index_lookup(index_id, key.as_bytes(), txn_id).await?;
                let mut rows = Vec::new();
                for pk in &pk_list {
                    if let Some(val) = self.storage.get(meta.table_id, pk, txn_id).await? {
                        if let Ok(row) = kv_storage::codec::deserialize_row(&val) {
                            rows.push(row);
                        }
                    }
                }
                Ok(ResultSet::with_rows(meta.columns.clone(), rows))
            }
            PlanNode::DropTable { name } => {
                self.tables.lock().unwrap().remove(&name);
                Ok(ResultSet::ok(0, None))
            }
            PlanNode::BeginTransaction => {
                let txn_id = self.txn_manager.lock().unwrap().begin();
                session.set_txn_id(Some(txn_id));
                Ok(ResultSet::ok(0, None))
            }
            PlanNode::CommitTransaction => {
                let txn_id = session.txn_id().ok_or_else(|| KvError::Internal("no active transaction".to_string()))?;
                {
                    let mut mgr = self.txn_manager.lock().unwrap();
                    mgr.commit(txn_id).map_err(|e| KvError::Internal(e.to_string()))?;
                }
                // Flush buffered writes to storage
                let buffer = self.get_txn_buffer(txn_id);
                for (table_id, entries) in &buffer.inserts {
                    for (key, val) in entries {
                        self.storage.put(*table_id, key, val, txn_id).await?;
                    }
                }
                for (table_id, keys) in &buffer.deletes {
                    for key in keys {
                        self.storage.delete(*table_id, key, txn_id).await?;
                    }
                }
                self.lock_manager.unlock_all(txn_id);
                session.set_txn_id(None);
                Ok(ResultSet::ok(0, None))
            }
            PlanNode::RollbackTransaction => {
                let txn_id = session.txn_id().ok_or_else(|| KvError::Internal("no active transaction".to_string()))?;
                {
                    let mut mgr = self.txn_manager.lock().unwrap();
                    mgr.rollback(txn_id).map_err(|e| KvError::Internal(e.to_string()))?;
                }
                self.get_txn_buffer(txn_id); // discard
                self.lock_manager.unlock_all(txn_id);
                session.set_txn_id(None);
                Ok(ResultSet::ok(0, None))
            }
            PlanNode::Sort { source, order_by } => {
                let mut inner = Box::pin(self.execute_plan(*source, session)).await?;
                let col_names: Vec<String> = inner.columns.iter().map(|c| c.name.clone()).collect();
                inner.rows.sort_by(|a, b| {
                    for ob in &order_by {
                        let idx = col_names.iter().position(|c| c.eq_ignore_ascii_case(&ob.column));
                        if let Some(pos) = idx {
                            let va = a.values.get(pos).unwrap_or(&Value::Null);
                            let vb = b.values.get(pos).unwrap_or(&Value::Null);
                            let ordering = Self::compare_values(va, vb);
                            if ordering != std::cmp::Ordering::Equal {
                                return if ob.ascending { ordering } else { ordering.reverse() };
                            }
                        }
                    }
                    std::cmp::Ordering::Equal
                });
                Ok(inner)
            }
            PlanNode::Join { left, right, join } => {
                let left_rs = Box::pin(self.execute_plan(*left, session)).await?;
                let right_rs = Box::pin(self.execute_plan(*right, session)).await?;
                let left_cols: Vec<String> = left_rs.columns.iter().map(|c| c.name.clone()).collect();
                let right_cols: Vec<String> = right_rs.columns.iter().map(|c| c.name.clone()).collect();
                let left_idx = left_cols.iter().position(|c| c.eq_ignore_ascii_case(&join.on.0));
                let right_idx = right_cols.iter().position(|c| c.eq_ignore_ascii_case(&join.on.1));
                let mut rows = Vec::new();
                let mut all_cols = left_rs.columns.clone();
                all_cols.extend(right_rs.columns.clone());
                for lr in &left_rs.rows {
                    for rr in &right_rs.rows {
                        let match_on = match (left_idx, right_idx) {
                            (Some(li), Some(ri)) => {
                                let lv = lr.values.get(li).unwrap_or(&Value::Null);
                                let rv = rr.values.get(ri).unwrap_or(&Value::Null);
                                Self::compare_values(lv, rv) == std::cmp::Ordering::Equal
                            }
                            _ => true,
                        };
                        if match_on {
                            let mut combined = lr.values.clone();
                            combined.extend(rr.values.clone());
                            rows.push(Row::new(combined));
                        }
                    }
                }
                Ok(ResultSet { columns: all_cols, rows, affected_rows: 0, last_insert_id: None })
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

    fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
        match (a, b) {
            (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
            (Value::Null, _) => std::cmp::Ordering::Less,
            (_, Value::Null) => std::cmp::Ordering::Greater,
            (Value::Int(a), Value::Int(b)) => a.cmp(b),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
            (Value::Int(a), Value::Float(b)) => (*a as f64).partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
            (Value::Float(a), Value::Int(b)) => a.partial_cmp(&(*b as f64)).unwrap_or(std::cmp::Ordering::Equal),
            (Value::String(a), Value::String(b)) => a.cmp(b),
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            _ => std::cmp::Ordering::Equal,
        }
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

    struct MockStorage { data: Mutex<HashMap<u64, Vec<(Vec<u8>, Vec<u8>)>>>, next_id: Mutex<u64> }

    #[async_trait]
    impl StorageEngine for MockStorage {
        async fn create_table(&self, _name: &str) -> KvResult<TableId> {
            let mut id = self.next_id.lock().unwrap();
            let tid = *id; *id += 1;
            Ok(tid)
        }
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
        fn as_any(&self) -> &dyn std::any::Any { self }
    }

    #[tokio::test]
    async fn test_executor_insert_select() {
        let storage = Arc::new(MockStorage { data: Mutex::new(HashMap::new()), next_id: Mutex::new(1) });
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
