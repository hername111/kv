//! 将表目录和索引映射到独立 B+Tree 的存储引擎。

use crate::btree::BPlusTree;
use crate::buffer::{BufferPool, BufferedPager};
use async_trait::async_trait;
use kv_common::error::KvResult;
use kv_common::traits::{Pager, StorageEngine};
use kv_common::types::{ColumnId, IndexId, TableId, TableMeta};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

struct IndexEntry {
    tree: Arc<BPlusTree>,
}

/// 数据库的持久化存储实现。
///
/// 表数据、目录和二级索引使用独立 B+Tree，共享同一缓冲页面管理器。
pub struct KvStorage {
    pager: Arc<dyn Pager>,
    trees: Mutex<HashMap<TableId, Arc<BPlusTree>>>,
    indexes: Mutex<HashMap<IndexId, IndexEntry>>,
    meta_tree: Mutex<Option<Arc<BPlusTree>>>,
    next_table_id: AtomicU64,
    next_index_id: AtomicU64,
}

impl KvStorage {
    /// 使用给定页面后端和缓存容量创建存储实例。
    pub fn new(pager: Arc<dyn Pager>, buffer_capacity: usize) -> Self {
        let buffer_pool = Arc::new(BufferPool::new(buffer_capacity));
        let pager = Arc::new(BufferedPager::new(pager, buffer_pool));
        KvStorage {
            pager,
            trees: Mutex::new(HashMap::new()),
            indexes: Mutex::new(HashMap::new()),
            meta_tree: Mutex::new(None),
            next_table_id: AtomicU64::new(1),
            next_index_id: AtomicU64::new(1),
        }
    }

    async fn get_or_init_meta_tree(&self) -> KvResult<Arc<BPlusTree>> {
        {
            let guard = self.meta_tree.lock().unwrap();
            if let Some(ref tree) = *guard {
                return Ok(tree.clone());
            }
        }
        let meta_root = self.pager.get_meta_root().await?;
        let tree = if meta_root != 0 {
            BPlusTree::open(self.pager.clone(), meta_root)
        } else {
            let t = BPlusTree::new(self.pager.clone()).await?;
            self.pager
                .set_meta_root(t.root_page_id.load(Ordering::Relaxed))
                .await?;
            t
        };
        let tree = Arc::new(tree);
        *self.meta_tree.lock().unwrap() = Some(tree.clone());
        Ok(tree)
    }

    fn get_tree(&self, table_id: TableId) -> KvResult<Arc<BPlusTree>> {
        self.trees
            .lock()
            .unwrap()
            .get(&table_id)
            .cloned()
            .ok_or_else(|| {
                kv_common::error::KvError::TableNotFound(format!("table_id={}", table_id))
            })
    }

    /// 扫描已有记录并批量构建二级索引。
    pub async fn build_index(
        &self,
        index_id: IndexId,
        _table_id: TableId,
        col_idx: usize,
        all_rows: &[(Vec<u8>, Vec<u8>)],
    ) -> KvResult<()> {
        let tree = BPlusTree::new(self.pager.clone()).await?;
        for (pk, row_data) in all_rows {
            if let Ok(row) = crate::codec::deserialize_row(row_data)
                && let Some(col_val) = row.values.get(col_idx)
            {
                let index_key = crate::codec::serialize_value(col_val);
                tree.insert(&index_key, pk).await?;
            }
        }
        self.indexes.lock().unwrap().insert(
            index_id,
            IndexEntry {
                tree: Arc::new(tree),
            },
        );
        Ok(())
    }

    pub async fn save_table_meta(&self, name: &str, meta: &TableMeta) -> KvResult<()> {
        let tree = self.get_or_init_meta_tree().await?;
        let key = format!("table:{}", name).into_bytes();
        let value = serde_json::to_vec(meta)
            .map_err(|e| kv_common::error::KvError::Internal(e.to_string()))?;
        tree.insert(&key, &value).await?;
        Ok(())
    }

    pub async fn load_all_table_meta(&self) -> KvResult<Vec<TableMeta>> {
        let tree = self.get_or_init_meta_tree().await?;
        let entries = tree.scan(b"table:", b"table;").await?;
        let mut metas = Vec::new();
        for (_, val) in entries {
            let meta: TableMeta = serde_json::from_slice(&val)
                .map_err(|e| kv_common::error::KvError::Internal(e.to_string()))?;
            metas.push(meta);
        }
        let max_tid = metas.iter().map(|m| m.table_id).max().unwrap_or(0);
        let max_iid = metas
            .iter()
            .flat_map(|m| m.indexes.iter().map(|i| i.index_id))
            .max()
            .unwrap_or(0);
        self.next_table_id.store(max_tid + 1, Ordering::Relaxed);
        self.next_index_id.store(max_iid + 1, Ordering::Relaxed);
        Ok(metas)
    }

    pub async fn delete_table_meta(&self, name: &str) -> KvResult<()> {
        let tree = self.get_or_init_meta_tree().await?;
        let key = format!("table:{}", name).into_bytes();
        tree.delete(&key).await?;
        Ok(())
    }
}

#[async_trait]
impl StorageEngine for KvStorage {
    async fn create_table(&self, _table_name: &str) -> KvResult<TableId> {
        let table_id = self.next_table_id.fetch_add(1, Ordering::Relaxed);
        let tree = BPlusTree::new(self.pager.clone()).await?;
        self.trees.lock().unwrap().insert(table_id, Arc::new(tree));
        Ok(table_id)
    }

    async fn put(
        &self,
        table_id: TableId,
        key: &[u8],
        value: &[u8],
        _txn_id: u64,
    ) -> KvResult<u64> {
        self.get_tree(table_id)?.insert(key, value).await?;
        Ok(1)
    }

    async fn get(&self, table_id: TableId, key: &[u8], _txn_id: u64) -> KvResult<Option<Vec<u8>>> {
        self.get_tree(table_id)?.search(key).await
    }

    async fn scan(
        &self,
        table_id: TableId,
        start: &[u8],
        end: &[u8],
        _txn_id: u64,
    ) -> KvResult<Vec<(Vec<u8>, Vec<u8>)>> {
        self.get_tree(table_id)?.scan(start, end).await
    }

    async fn delete(&self, table_id: TableId, key: &[u8], _txn_id: u64) -> KvResult<()> {
        self.get_tree(table_id)?.delete(key).await?;
        Ok(())
    }

    async fn create_index(&self, _table_id: TableId, _col_id: ColumnId) -> KvResult<IndexId> {
        let id = self.next_index_id.fetch_add(1, Ordering::Relaxed);
        Ok(id)
    }

    async fn index_lookup(
        &self,
        index_id: IndexId,
        key: &[u8],
        _txn_id: u64,
    ) -> KvResult<Vec<Vec<u8>>> {
        let tree = {
            let indexes = self.indexes.lock().unwrap();
            indexes.get(&index_id).map(|entry| entry.tree.clone())
        };
        if let Some(tree) = tree
            && let Some(val) = tree.search(key).await?
        {
            return Ok(vec![val]);
        }
        Ok(Vec::new())
    }

    async fn save_table_meta(&self, name: &str, meta: &TableMeta) -> KvResult<()> {
        self.save_table_meta(name, meta).await
    }

    async fn load_all_table_meta(&self) -> KvResult<Vec<TableMeta>> {
        self.load_all_table_meta().await
    }

    async fn delete_table_meta(&self, name: &str) -> KvResult<()> {
        self.delete_table_meta(name).await
    }

    async fn get_table_root(&self, table_id: TableId) -> KvResult<u64> {
        let tree = self.get_tree(table_id)?;
        Ok(tree.root_page_id.load(Ordering::Relaxed))
    }

    async fn restore_table(&self, table_id: TableId, root_page_id: u64) -> KvResult<()> {
        let tree = BPlusTree::open(self.pager.clone(), root_page_id);
        self.trees.lock().unwrap().insert(table_id, Arc::new(tree));
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MemPager {
        pages: Mutex<HashMap<u64, Vec<u8>>>,
        counter: Mutex<u64>,
    }
    #[async_trait]
    impl Pager for MemPager {
        async fn read_page(&self, pid: u64) -> KvResult<Vec<u8>> {
            Ok(self
                .pages
                .lock()
                .unwrap()
                .get(&pid)
                .cloned()
                .unwrap_or_else(|| vec![0u8; 4096]))
        }
        async fn write_page(&self, pid: u64, data: &[u8]) -> KvResult<()> {
            let mut d = vec![0u8; 4096];
            let l = data.len().min(4096);
            d[..l].copy_from_slice(&data[..l]);
            self.pages.lock().unwrap().insert(pid, d);
            Ok(())
        }
        async fn allocate_page(&self) -> KvResult<u64> {
            let mut c = self.counter.lock().unwrap();
            let id = *c;
            *c += 1;
            Ok(id)
        }
        async fn free_page(&self, _pid: u64) -> KvResult<()> {
            Ok(())
        }
        async fn flush(&self) -> KvResult<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_engine_put_get() {
        let pager = Arc::new(MemPager {
            pages: Mutex::new(HashMap::new()),
            counter: Mutex::new(1),
        });
        let s = KvStorage::new(pager, 64);
        let tid = s.create_table("t").await.unwrap();
        s.put(tid, b"k", b"v", 0).await.unwrap();
        assert_eq!(s.get(tid, b"k", 0).await.unwrap(), Some(b"v".to_vec()));
    }

    #[tokio::test]
    async fn test_engine_scan() {
        let pager = Arc::new(MemPager {
            pages: Mutex::new(HashMap::new()),
            counter: Mutex::new(1),
        });
        let s = KvStorage::new(pager, 64);
        let tid = s.create_table("t").await.unwrap();
        for i in 0..5u8 {
            s.put(tid, &[i], &[i * 2], 0).await.unwrap();
        }
        assert_eq!(s.scan(tid, &[1u8], &[4u8], 0).await.unwrap().len(), 3);
    }
}
