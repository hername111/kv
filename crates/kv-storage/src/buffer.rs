// Buffer Pool：线程安全的 LRU 页缓存
use kv_common::traits::Pager;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct BufferPool {
    capacity: usize,
    map: Mutex<HashMap<u64, Vec<u8>>>,
    lru: Mutex<Vec<u64>>,
}

impl BufferPool {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            map: Mutex::new(HashMap::new()),
            lru: Mutex::new(Vec::new()),
        }
    }

    pub fn get(&self, page_id: u64) -> Option<Vec<u8>> {
        let mut map = self.map.lock().unwrap();
        let result = map.get(&page_id).cloned();
        if result.is_some() {
            let mut lru = self.lru.lock().unwrap();
            lru.retain(|&id| id != page_id);
            lru.push(page_id);
        }
        result
    }

    pub fn put(&self, page_id: u64, data: Vec<u8>) {
        let mut map = self.map.lock().unwrap();
        let mut lru = self.lru.lock().unwrap();
        if map.len() >= self.capacity && !map.contains_key(&page_id) {
            if let Some(old) = lru.first().copied() {
                if old != page_id {
                    map.remove(&old);
                    lru.retain(|&id| id != old);
                }
            }
        }
        lru.retain(|&id| id != page_id);
        lru.push(page_id);
        map.insert(page_id, data);
    }
}

/// Pager 包装器：在 BufferPool 中缓存页，减少磁盘 I/O
pub struct BufferedPager {
    inner: Arc<dyn Pager>,
    pool: Arc<BufferPool>,
}

impl BufferedPager {
    pub fn new(inner: Arc<dyn Pager>, pool: Arc<BufferPool>) -> Self {
        Self { inner, pool }
    }
}

#[async_trait::async_trait]
impl Pager for BufferedPager {
    async fn read_page(&self, page_id: u64) -> kv_common::error::KvResult<Vec<u8>> {
        if let Some(data) = self.pool.get(page_id) {
            return Ok(data);
        }
        let data = self.inner.read_page(page_id).await?;
        self.pool.put(page_id, data.clone());
        Ok(data)
    }

    async fn write_page(&self, page_id: u64, data: &[u8]) -> kv_common::error::KvResult<()> {
        self.inner.write_page(page_id, data).await?;
        self.pool.put(page_id, data.to_vec());
        Ok(())
    }

    async fn allocate_page(&self) -> kv_common::error::KvResult<u64> {
        self.inner.allocate_page().await
    }

    async fn free_page(&self, page_id: u64) -> kv_common::error::KvResult<()> {
        self.inner.free_page(page_id).await
    }

    async fn flush(&self) -> kv_common::error::KvResult<()> {
        self.inner.flush().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct MemPager {
        pages: Mutex<HashMap<u64, Vec<u8>>>,
        counter: Mutex<u64>,
    }

    #[async_trait::async_trait]
    impl Pager for MemPager {
        async fn read_page(&self, page_id: u64) -> kv_common::error::KvResult<Vec<u8>> {
            Ok(self.pages.lock().unwrap().get(&page_id).cloned().unwrap_or_else(|| vec![0u8; 4096]))
        }
        async fn write_page(&self, page_id: u64, data: &[u8]) -> kv_common::error::KvResult<()> {
            let mut d = vec![0u8; 4096];
            let l = data.len().min(4096);
            d[..l].copy_from_slice(&data[..l]);
            self.pages.lock().unwrap().insert(page_id, d);
            Ok(())
        }
        async fn allocate_page(&self) -> kv_common::error::KvResult<u64> {
            let mut c = self.counter.lock().unwrap();
            let id = *c;
            *c += 1;
            Ok(id)
        }
        async fn free_page(&self, _pid: u64) -> kv_common::error::KvResult<()> { Ok(()) }
        async fn flush(&self) -> kv_common::error::KvResult<()> { Ok(()) }
    }

    #[tokio::test]
    async fn test_buffered_pager_cache_hit() {
        let mem = Arc::new(MemPager { pages: Mutex::new(HashMap::new()), counter: Mutex::new(1) });
        let pool = Arc::new(BufferPool::new(4));
        let bp = BufferedPager::new(mem, pool);

        bp.write_page(1, &[42u8; 4096]).await.unwrap();
        let data = bp.read_page(1).await.unwrap();
        assert_eq!(data[0], 42);
        let data2 = bp.read_page(1).await.unwrap();
        assert_eq!(data2[0], 42);
    }

    #[test]
    fn buffer_put_get() {
        let bp = BufferPool::new(2);
        bp.put(1, vec![1, 2, 3]);
        bp.put(2, vec![4, 5, 6]);
        assert_eq!(bp.get(1).unwrap(), vec![1, 2, 3]);
        bp.put(3, vec![7, 8, 9]);
        assert!(bp.get(2).is_none());
    }
}
