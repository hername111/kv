// B+Tree (ORDER=4)：基于 Pager trait 的持久化 B+树索引
use kv_common::error::KvResult;
use kv_common::traits::Pager;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use async_trait::async_trait;

const ORDER: usize = 4;
const MAX_KEYS: usize = ORDER - 1;

const FLAG_INTERNAL: u8 = 0;
const FLAG_LEAF: u8 = 1;

fn encode_internal_node(_page_id: u64, keys: &[Vec<u8>], children: &[u64]) -> Vec<u8> {
    let mut buf = vec![FLAG_INTERNAL];
    buf.extend(&(keys.len() as u16).to_le_bytes());
    for k in keys {
        buf.extend(&(k.len() as u16).to_le_bytes());
        buf.extend(k);
    }
    for &c in children {
        buf.extend(&c.to_le_bytes());
    }
    buf.resize(4096, 0);
    buf
}

fn encode_leaf_node(keys: &[Vec<u8>], values: &[Vec<u8>], next: u64) -> Vec<u8> {
    let mut buf = vec![FLAG_LEAF];
    buf.extend(&(keys.len() as u16).to_le_bytes());
    for k in keys {
        buf.extend(&(k.len() as u16).to_le_bytes());
        buf.extend(k);
    }
    for v in values {
        buf.extend(&(v.len() as u32).to_le_bytes());
        buf.extend(v);
    }
    buf.extend(&next.to_le_bytes());
    buf.resize(4096, 0);
    buf
}

fn decode_leaf_entry(data: &[u8]) -> (Vec<Vec<u8>>, Vec<Vec<u8>>, u64) {
    let num_keys = u16::from_le_bytes(data[1..3].try_into().unwrap()) as usize;
    let mut pos = 3;
    let mut keys = Vec::with_capacity(num_keys);
    for _ in 0..num_keys {
        let len = u16::from_le_bytes(data[pos..pos+2].try_into().unwrap()) as usize;
        pos += 2;
        keys.push(data[pos..pos+len].to_vec());
        pos += len;
    }
    let mut values = Vec::with_capacity(num_keys);
    for _ in 0..num_keys {
        let len = u32::from_le_bytes(data[pos..pos+4].try_into().unwrap()) as usize;
        pos += 4;
        values.push(data[pos..pos+len].to_vec());
        pos += len;
    }
    let next = u64::from_le_bytes(data[pos..pos+8].try_into().unwrap());
    (keys, values, next)
}

fn decode_internal_entry(data: &[u8]) -> (Vec<Vec<u8>>, Vec<u64>) {
    let num_keys = u16::from_le_bytes(data[1..3].try_into().unwrap()) as usize;
    let mut pos = 3;
    let mut keys = Vec::with_capacity(num_keys);
    for _ in 0..num_keys {
        let len = u16::from_le_bytes(data[pos..pos+2].try_into().unwrap()) as usize;
        pos += 2;
        keys.push(data[pos..pos+len].to_vec());
        pos += len;
    }
    let mut children = Vec::with_capacity(num_keys + 1);
    for _ in 0..=num_keys {
        children.push(u64::from_le_bytes(data[pos..pos+8].try_into().unwrap()));
        pos += 8;
    }
    (keys, children)
}

fn find_key_index(keys: &[Vec<u8>], target: &[u8]) -> usize {
    keys.iter().position(|k| target < k.as_slice()).unwrap_or(keys.len())
}

pub struct BPlusTree {
    pub pager: Arc<dyn Pager>,
    pub root_page_id: AtomicU64,
}

impl BPlusTree {
    pub async fn new(pager: Arc<dyn Pager>) -> KvResult<Self> {
        let root_page_id = pager.allocate_page().await?;
        let leaf = encode_leaf_node(&[], &[], 0);
        pager.write_page(root_page_id, &leaf).await?;
        Ok(BPlusTree { pager, root_page_id: AtomicU64::new(root_page_id) })
    }

    pub async fn insert(&self, key: &[u8], value: &[u8]) -> KvResult<()> {
        let result = self.insert_recursive(self.root_page_id.load(Ordering::Relaxed), key, value).await?;
        if let (Some(promo_key), Some(new_child)) = result {
            let new_root = encode_internal_node(0, &[promo_key], &[self.root_page_id.load(Ordering::Relaxed), new_child]);
            let new_root_id = self.pager.allocate_page().await?;
            self.pager.write_page(new_root_id, &new_root).await?;
            self.root_page_id.store(new_root_id, Ordering::Relaxed);
        }
        Ok(())
    }

    async fn insert_recursive(
        &self,
        page_id: u64,
        key: &[u8],
        value: &[u8],
    ) -> KvResult<(Option<Vec<u8>>, Option<u64>)> {
        let data = self.pager.read_page(page_id).await?;
        let flag = data[0];

        if flag == FLAG_LEAF {
            let (mut keys, mut values, next) = decode_leaf_entry(&data);
            let idx = keys.binary_search(&key.to_vec()).unwrap_or_else(|e| e);
            if idx < keys.len() && keys[idx] == key {
                values[idx] = value.to_vec();
                let new_data = encode_leaf_node(&keys, &values, next);
                self.pager.write_page(page_id, &new_data).await?;
                return Ok((None, None));
            }
            keys.insert(idx, key.to_vec());
            values.insert(idx, value.to_vec());

            if keys.len() > MAX_KEYS {
                let mid = keys.len() / 2;
                let right_keys = keys.split_off(mid);
                let right_vals = values.split_off(mid);
                let promo = right_keys[0].clone();

                let right_id = self.pager.allocate_page().await?;
                let right_data = encode_leaf_node(&right_keys, &right_vals, next);
                self.pager.write_page(right_id, &right_data).await?;

                let left_data = encode_leaf_node(&keys, &values, right_id);
                self.pager.write_page(page_id, &left_data).await?;
                Ok((Some(promo), Some(right_id)))
            } else {
                let new_data = encode_leaf_node(&keys, &values, next);
                self.pager.write_page(page_id, &new_data).await?;
                Ok((None, None))
            }
        } else {
            let (keys, children) = decode_internal_entry(&data);
            let child_idx = find_key_index(&keys, key);
            let child_page = children[child_idx];

            let (maybe_promo, maybe_new_child) = Box::pin(
                self.insert_recursive(child_page, key, value)
            ).await?;

            match (maybe_promo, maybe_new_child) {
                (Some(promo_key), Some(new_child_id)) => {
                    let (mut int_keys, mut int_children) = decode_internal_entry(&data);
                    let insert_idx = int_keys.binary_search(&promo_key).unwrap_or_else(|e| e);
                    int_keys.insert(insert_idx, promo_key);
                    int_children.insert(insert_idx + 1, new_child_id);

                    if int_keys.len() > MAX_KEYS {
                        let mid = int_keys.len() / 2;
                        let promo = int_keys[mid].clone();
                        let right_keys = int_keys.split_off(mid + 1);
                        int_keys.pop();
                        let right_children = int_children.split_off(mid + 1);

                        let right_id = self.pager.allocate_page().await?;
                        let right_data = encode_internal_node(0, &right_keys, &right_children);
                        self.pager.write_page(right_id, &right_data).await?;

                        let left_data = encode_internal_node(0, &int_keys, &int_children);
                        self.pager.write_page(page_id, &left_data).await?;
                        Ok((Some(promo), Some(right_id)))
                    } else {
                        let new_data = encode_internal_node(0, &int_keys, &int_children);
                        self.pager.write_page(page_id, &new_data).await?;
                        Ok((None, None))
                    }
                }
                _ => Ok((None, None)),
            }
        }
    }

    pub async fn search(&self, key: &[u8]) -> KvResult<Option<Vec<u8>>> {
        let mut page_id = self.root_page_id.load(Ordering::Relaxed);
        loop {
            let data = self.pager.read_page(page_id).await?;
            if data[0] == FLAG_LEAF {
                let (keys, values, _) = decode_leaf_entry(&data);
                return Ok(keys.iter().position(|k| k == key).map(|i| values[i].clone()));
            }
            let (keys, children) = decode_internal_entry(&data);
            page_id = children[find_key_index(&keys, key)];
        }
    }

    pub async fn scan(&self, start: &[u8], end: &[u8]) -> KvResult<Vec<(Vec<u8>, Vec<u8>)>> {
        let mut page_id = self.root_page_id.load(Ordering::Relaxed);
        loop {
            let data = self.pager.read_page(page_id).await?;
            if data[0] == FLAG_LEAF { break; }
            let (keys, children) = decode_internal_entry(&data);
            page_id = children[find_key_index(&keys, start)];
        }

        let mut results = Vec::new();
        let mut current_id = page_id;
        loop {
            let data = self.pager.read_page(current_id).await?;
            let (keys, values, next) = decode_leaf_entry(&data);
            for (k, v) in keys.iter().zip(values.iter()) {
                if k.as_slice() < start { continue; }
                if k.as_slice() >= end { return Ok(results); }
                results.push((k.clone(), v.clone()));
            }
            if next == 0 { break; }
            current_id = next;
        }
        Ok(results)
    }

    pub async fn delete(&self, key: &[u8]) -> KvResult<bool> {
        let found = self.delete_recursive(self.root_page_id.load(Ordering::Relaxed), key).await?;
        let data = self.pager.read_page(self.root_page_id.load(Ordering::Relaxed)).await?;
        if data[0] == FLAG_INTERNAL {
            let (keys, children) = decode_internal_entry(&data);
            if keys.is_empty() && !children.is_empty() {
                self.pager.free_page(self.root_page_id.load(Ordering::Relaxed)).await?;
                self.root_page_id.store(children[0], Ordering::Relaxed);
            }
        }
        Ok(found)
    }

    async fn delete_recursive(&self, page_id: u64, key: &[u8]) -> KvResult<bool> {
        let data = self.pager.read_page(page_id).await?;
        if data[0] == FLAG_LEAF {
            let (mut keys, mut values, next) = decode_leaf_entry(&data);
            if let Some(idx) = keys.iter().position(|k| k == key) {
                keys.remove(idx);
                values.remove(idx);
                let new_data = encode_leaf_node(&keys, &values, next);
                self.pager.write_page(page_id, &new_data).await?;
                return Ok(true);
            }
            Ok(false)
        } else {
            let (keys, children) = decode_internal_entry(&data);
            let child_idx = find_key_index(&keys, key);
            Box::pin(self.delete_recursive(children[child_idx], key)).await
        }
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

    impl MemPager {
        fn new() -> Self {
            MemPager { pages: Mutex::new(HashMap::new()), counter: Mutex::new(1) }
        }
    }

    #[async_trait]
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
            let id = *c;
            *c += 1;
            Ok(id)
        }

        async fn free_page(&self, page_id: u64) -> KvResult<()> {
            self.pages.lock().unwrap().remove(&page_id);
            Ok(())
        }

        async fn flush(&self) -> KvResult<()> { Ok(()) }
    }

    async fn make_tree() -> BPlusTree {
        let pager = Arc::new(MemPager::new());
        BPlusTree::new(pager).await.unwrap()
    }

    #[tokio::test]
    async fn test_insert_and_search() {
        let tree = make_tree().await;
        tree.insert(b"hello", b"world").await.unwrap();
        let val = tree.search(b"hello").await.unwrap();
        assert_eq!(val, Some(b"world".to_vec()));
        assert_eq!(tree.search(b"not_found").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_scan() {
        let tree = make_tree().await;
        for i in 0..20u8 {
            tree.insert(&[i], &[i * 2]).await.unwrap();
        }
        let results = tree.scan(&[5u8], &[15u8]).await.unwrap();
        assert!(results.len() >= 1);
        for (k, v) in &results {
            assert_eq!(v[0], k[0] * 2);
        }
    }

    #[tokio::test]
    async fn test_delete() {
        let tree = make_tree().await;
        tree.insert(b"key", b"val").await.unwrap();
        assert!(tree.delete(b"key").await.unwrap());
        assert!(tree.search(b"key").await.unwrap().is_none());
        assert!(!tree.delete(b"key").await.unwrap());
    }

    #[tokio::test]
    async fn test_split() {
        let tree = make_tree().await;
        for i in 0..10u8 {
            tree.insert(&[i], &[i; 10]).await.unwrap();
        }
        for i in 0..10u8 {
            assert_eq!(tree.search(&[i]).await.unwrap().unwrap(), &[i; 10]);
        }
    }
}
