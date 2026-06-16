use kv_common::types::{ColumnDef, DataType, Session, TableMeta, Value};
use kv_common::traits::StorageEngine;
use kv_common::error::KvResult;
use kv_sql::SqlExecutor;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

struct MockStorage { data: Mutex<HashMap<u64, Vec<(Vec<u8>, Vec<u8>)>>> }

#[async_trait::async_trait]
impl StorageEngine for MockStorage {
    async fn put(&self, tid: u64, key: &[u8], value: &[u8], _txn: u64) -> KvResult<u64> {
        self.data.lock().unwrap().entry(tid).or_default().push((key.to_vec(), value.to_vec()));
        Ok(1)
    }
    async fn get(&self, tid: u64, key: &[u8], _txn: u64) -> KvResult<Option<Vec<u8>>> {
        Ok(self.data.lock().unwrap().get(&tid).and_then(|v| v.iter().find(|(k,_)| k==key).map(|(_,v)| v.clone())))
    }
    async fn scan(&self, tid: u64, _s: &[u8], _e: &[u8], _txn: u64) -> KvResult<Vec<(Vec<u8>, Vec<u8>)>> {
        Ok(self.data.lock().unwrap().get(&tid).cloned().unwrap_or_default())
    }
    async fn delete(&self, tid: u64, key: &[u8], _txn: u64) -> KvResult<()> {
        self.data.lock().unwrap().get_mut(&tid).map(|v| v.retain(|(k,_)| k!=key));
        Ok(())
    }
    async fn create_index(&self, _t: u64, _c: u64) -> KvResult<u64> { Ok(1) }
    async fn index_lookup(&self, _i: u64, _k: &[u8], _txn: u64) -> KvResult<Vec<Vec<u8>>> { Ok(vec![]) }
}

#[tokio::test]
async fn test_insert_select_flow() {
    let storage = Arc::new(MockStorage { data: Mutex::new(HashMap::new()) });
    let mut executor = SqlExecutor::new(storage);
    executor.tables.insert("t".to_string(), TableMeta {
        table_id: 1, table_name: "t".to_string(),
        columns: vec![ColumnDef { id: 1, name: "id".to_string(), data_type: DataType::Int, nullable: false, is_primary_key: true }],
        primary_key_index: 0, indexes: vec![],
    });
    let session = Session::new();
    let rs = executor.execute_sql("INSERT INTO t VALUES (1)", &session).await.unwrap();
    assert_eq!(rs.affected_rows, 1);
    let rs = executor.execute_sql("INSERT INTO t VALUES (2)", &session).await.unwrap();
    assert_eq!(rs.affected_rows, 1);
    let rs = executor.execute_sql("SELECT * FROM t", &session).await.unwrap();
    assert_eq!(rs.rows.len(), 2);
    assert_eq!(rs.rows[0].values[0], Value::Int(1));
}

#[tokio::test]
async fn test_delete_flow() {
    let storage = Arc::new(MockStorage { data: Mutex::new(HashMap::new()) });
    let mut executor = SqlExecutor::new(storage);
    executor.tables.insert("t".to_string(), TableMeta {
        table_id: 1, table_name: "t".to_string(),
        columns: vec![ColumnDef { id: 1, name: "id".to_string(), data_type: DataType::Int, nullable: false, is_primary_key: true }],
        primary_key_index: 0, indexes: vec![],
    });
    let session = Session::new();
    executor.execute_sql("INSERT INTO t VALUES (1)", &session).await.unwrap();
    executor.execute_sql("INSERT INTO t VALUES (2)", &session).await.unwrap();
    let rs = executor.execute_sql("DELETE FROM t WHERE id = 1", &session).await.unwrap();
    assert_eq!(rs.affected_rows, 1);
    let rs = executor.execute_sql("SELECT * FROM t", &session).await.unwrap();
    assert_eq!(rs.rows.len(), 1);
}

#[tokio::test]
async fn test_update_flow() {
    let storage = Arc::new(MockStorage { data: Mutex::new(HashMap::new()) });
    let mut executor = SqlExecutor::new(storage);
    executor.tables.insert("t".to_string(), TableMeta {
        table_id: 1, table_name: "t".to_string(),
        columns: vec![
            ColumnDef { id: 1, name: "id".to_string(), data_type: DataType::Int, nullable: false, is_primary_key: true },
            ColumnDef { id: 2, name: "val".to_string(), data_type: DataType::Int, nullable: true, is_primary_key: false },
        ],
        primary_key_index: 0, indexes: vec![],
    });
    let session = Session::new();
    executor.execute_sql("INSERT INTO t VALUES (1, 10)", &session).await.unwrap();
    let rs = executor.execute_sql("UPDATE t SET val = 99 WHERE id = 1", &session).await.unwrap();
    assert_eq!(rs.affected_rows, 1);
}
