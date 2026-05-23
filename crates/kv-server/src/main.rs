// 组装所有模块，启动数据库服务
use kv_common::*;
use kv_storage::*;
use kv_sql::*;
use kv_txn::*;
use kv_network::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 BufferPool
    let mut buffer_pool = BufferPool::new(64);

    // 初始化事务管理器
    let mut txn_mgr = TxnManager::new();

    // 初始化 Executor (这里暂时用 MockStorage，实际可接 B+Tree StorageEngine)
    struct StorageWrapper;
    impl StorageEngine for StorageWrapper {
        fn insert(&mut self, _table: &str, _row: crate::types::Row) {}
        fn select(&self, _table: &str) -> Vec<crate::types::Row> { vec![] }
        fn update(&mut self, _table: &str, _updates: Vec<(String, String)>) {}
        fn delete(&mut self, _table: &str) {}
    }
    let mut executor = Executor::new(&mut StorageWrapper);

    // 启动 TCP Server
    let server = KvServer::new("127.0.0.1:3306".to_string());
    server.start().await?;

    Ok(())
}