use kv_network::KvServer;
use kv_sql::SqlExecutor;
use kv_storage::{BufferPool, BufferedPager, DiskPager, KvStorage};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting KV Database Server...");
    let data_dir = std::env::var("KV_DATA_DIR").unwrap_or_else(|_| "kv_data".to_string());
    let db_path = format!("{}/kv.db", data_dir);
    std::fs::create_dir_all(&data_dir)?;
    let disk = Arc::new(DiskPager::open(&db_path)?);
    let pool = Arc::new(BufferPool::new(256));
    let pager = Arc::new(BufferedPager::new(disk, pool));
    let storage = Arc::new(KvStorage::new(pager, 256));
    let executor = Arc::new(SqlExecutor::new(storage));
    let addr = std::env::var("KV_ADDR").unwrap_or_else(|_| "127.0.0.1:3307".to_string());
    println!("Binding to {}", addr);
    println!("Data directory: {}", data_dir);
    let server = KvServer::new(addr).with_handler(executor);
    server.start().await?;
    Ok(())
}
