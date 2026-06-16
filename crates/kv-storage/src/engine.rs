use async_trait::async_trait;
use kv_common::error::KvResult;
use kv_common::traits::{Pager, StorageEngine};
use kv_common::types::{ColumnId, IndexId, TableId};
use std::collections::HashMap;
use std::sync::Arc;
use crate::btree::BPlusTree;
use crate::buffer::BufferPool;

pub struct KvStorage {
    pager: Arc<dyn Pager>,
    trees: HashMap<TableId, BPlusTree>,
    buffer_pool: BufferPool,
    next_table_id: TableId,
    next_index_id: IndexId,
}

impl KvStorage {
    pub fn new(pager: Arc<dyn Pager>, buffer_capacity: usize) -> Self {
        KvStorage {
            pager,
            trees: HashMap::new(),
            buffer_pool: BufferPool::new(buffer_capacity),
            next_table_id: 1,
            next_index_id: 1,
        }
    }

    pub async fn create_table(&mut self, _table_name: &str) -> KvResult<TableId> {
        let table_id = self.next_table_id;
        self.next_table_id += 1;
        let tree = BPlusTree::new(self.pager.clone()).await?;
        self.trees.insert(table_id, tree);
        Ok(table_id)
    }

    fn get_tree(&self, table_id: TableId) -> KvResult<&BPlusTree> {
        self.trees.get(&table_id)
            .ok_or_else(|| kv_common::error::KvError::TableNotFound(format!("table_id={}", table_id)))
    }
}

#[async_trait]
impl StorageEngine for KvStorage {
    async fn put(&self, table_id: TableId, key: &[u8], value: &[u8], _txn_id: u64) -> KvResult<u64> {
        self.get_tree(table_id)?.insert(key, value).await?;
        Ok(1)
    }

    async fn get(&self, table_id: TableId, key: &[u8], _txn_id: u64) -> KvResult<Option<Vec<u8>>> {
        self.get_tree(table_id)?.search(key).await
    }

    async fn scan(&self, table_id: TableId, start: &[u8], end: &[u8], _txn_id: u64) -> KvResult<Vec<(Vec<u8>, Vec<u8>)>> {
        self.get_tree(table_id)?.scan(start, end).await
    }

    async fn delete(&self, table_id: TableId, key: &[u8], _txn_id: u64) -> KvResult<()> {
        self.get_tree(table_id)?.delete(key).await?;
        Ok(())
    }

    async fn create_index(&self, _table_id: TableId, _col_id: ColumnId) -> KvResult<IndexId> {
        let id = 1u64;
        Ok(id)
    }

    async fn index_lookup(&self, _index_id: IndexId, key: &[u8], _txn_id: u64) -> KvResult<Vec<Vec<u8>>> {
        let mut results = Vec::new();
        for tree in self.trees.values() {
            if let Some(val) = tree.search(key).await? { results.push(val); }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct MemPager { pages: Mutex<HashMap<u64, Vec<u8>>>, counter: Mutex<u64> }
    #[async_trait]
    impl Pager for MemPager {
        async fn read_page(&self, pid: u64) -> KvResult<Vec<u8>> {
            Ok(self.pages.lock().unwrap().get(&pid).cloned().unwrap_or_else(|| vec![0u8; 4096]))
        }
        async fn write_page(&self, pid: u64, data: &[u8]) -> KvResult<()> {
            let mut d = vec![0u8; 4096]; let l = data.len().min(4096); d[..l].copy_from_slice(&data[..l]);
            self.pages.lock().unwrap().insert(pid, d); Ok(())
        }
        async fn allocate_page(&self) -> KvResult<u64> { let mut c = self.counter.lock().unwrap(); let id = *c; *c += 1; Ok(id) }
        async fn free_page(&self, _pid: u64) -> KvResult<()> { Ok(()) }
        async fn flush(&self) -> KvResult<()> { Ok(()) }
    }

    #[tokio::test]
    async fn test_engine_put_get() {
        let pager = Arc::new(MemPager { pages: Mutex::new(HashMap::new()), counter: Mutex::new(1) });
        let mut s = KvStorage::new(pager, 64);
        let tid = s.create_table("t").await.unwrap();
        s.put(tid, b"k", b"v", 0).await.unwrap();
        assert_eq!(s.get(tid, b"k", 0).await.unwrap(), Some(b"v".to_vec()));
    }

    #[tokio::test]
    async fn test_engine_scan() {
        let pager = Arc::new(MemPager { pages: Mutex::new(HashMap::new()), counter: Mutex::new(1) });
        let mut s = KvStorage::new(pager, 64);
        let tid = s.create_table("t").await.unwrap();
        for i in 0..5u8 { s.put(tid, &[i], &[i*2], 0).await.unwrap(); }
        assert_eq!(s.scan(tid, &[1u8], &[4u8], 0).await.unwrap().len(), 3);
    }
}
