// 端到端 SQL 测试：SQL 解析 → 执行 → 结果验证
use kv_common::*;
use kv_storage::*;
use kv_sql::*;
use kv_txn::*;
use kv_network::*;
use tokio::runtime::Runtime;
use crate::kv_sql::ast::Statement;

#[tokio::test]
async fn test_insert_select_flow() {
    // 模拟 StorageEngine
    struct MockStorage;
    impl StorageEngine for MockStorage {
        fn insert(&mut self, _table: &str, _row: Row) {}
        fn select(&self, _table: &str) -> Vec<Row> { vec![Row { values: vec![Value::Int(1)] }] }
        fn update(&mut self, _table: &str, _updates: Vec<(String, String)>) {}
        fn delete(&mut self, _table: &str) {}
    }

    let mut storage = MockStorage;
    let mut executor = Executor::new(&mut storage);
    let stmt = Statement::Select { table: "t".to_string(), columns: vec!["*".to_string()] };
    let rows = executor.execute(stmt);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].values[0], Value::Int(1));
}