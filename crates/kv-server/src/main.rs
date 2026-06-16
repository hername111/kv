use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use kv_common::traits::Pager;
use kv_common::error::KvResult;
use kv_network::KvServer;
use kv_sql::SqlExecutor;
use kv_storage::KvStorage;

struct MemPager {
    pages: Mutex<HashMap<u64, Vec<u8>>>,
    counter: Mutex<u64>,
}

impl MemPager {
    fn new() -> Self {
        MemPager { pages: Mutex::new(HashMap::new()), counter: Mutex::new(1) }
    }
}

#[async_trait::async_trait]
impl Pager for MemPager {
    async fn read_page(&self, page_id: u64) -> KvResult<Vec<u8>> {
        let pages = self.pages.lock().unwrap();
        Ok(pages.get(&page_id).cloned().unwrap_or_else(|| vec![0u8; 4096]))
    }
    async fn write_page(&self, page_id: u64, data: &[u8]) -> KvResult<()> {
        let mut d = vec![0u8; 4096];
        let len = data.len().min(4096);
        d[..len].copy_from_slice(&data[..len]);
        self.pages.lock().unwrap().insert(page_id, d);
        Ok(())
    }
    async fn allocate_page(&self) -> KvResult<u64> {
        let mut c = self.counter.lock().unwrap();
        let id = *c; *c += 1;
        Ok(id)
    }
    async fn free_page(&self, _pid: u64) -> KvResult<()> { Ok(()) }
    async fn flush(&self) -> KvResult<()> { Ok(()) }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting KV Database Server...");
    let pager = Arc::new(MemPager::new());
    let storage = Arc::new(KvStorage::new(pager, 64));
    let executor = Arc::new(SqlExecutor::new(storage));
    let addr = std::env::var("KV_ADDR").unwrap_or_else(|_| "127.0.0.1:3307".to_string());
    println!("Binding to {}", addr);
    let server = KvServer::new(addr).with_handler(executor);
    server.start().await?;
    Ok(())
}
