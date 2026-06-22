mod demo_http;

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
    executor.load_catalog().await?;
    let addr = std::env::var("KV_ADDR").unwrap_or_else(|_| "127.0.0.1:3307".to_string());
    println!("Binding to {}", addr);
    println!("Data directory: {}", data_dir);
    let demo_addr = std::env::var("KV_DEMO_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let demo_executor = executor.clone();
    tokio::spawn(async move {
        if let Err(err) = demo_http::start_demo_http(demo_addr, demo_executor).await {
            eprintln!("Demo HTTP server failed: {}", err);
        }
    });
    let server = KvServer::new(addr).with_handler(executor);
    server.start().await?;
    Ok(())
}
