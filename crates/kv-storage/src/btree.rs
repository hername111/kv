//! 基于 [`Pager`] 的固定阶持久化 B+Tree。
//!
//! 节点直接编码到固定大小页面：内部节点保存分隔键和子页号，叶子节点保存键值对并通过
//! `next` 指针串联。课程项目采用较小阶数，便于测试中稳定触发分裂和根节点变化。
use crate::page::PAGE_SIZE;
use kv_common::error::{KvError, KvResult};
use kv_common::traits::Pager;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const ORDER: usize = 4;
const MAX_KEYS: usize = ORDER - 1;

const FLAG_INTERNAL: u8 = 0;
const FLAG_LEAF: u8 = 1;

type NodeKeys = Vec<Vec<u8>>;
type NodeValues = Vec<Vec<u8>>;
type DecodedLeaf = (NodeKeys, NodeValues, u64);

fn finish_page(mut buffer: Vec<u8>) -> KvResult<Vec<u8>> {
    if buffer.len() > PAGE_SIZE {
        return Err(KvError::InvalidQuery(format!(
            "B+Tree node requires {} bytes, page size is {} bytes",
            buffer.len(),
            PAGE_SIZE
        )));
    }
    buffer.resize(PAGE_SIZE, 0);
    Ok(buffer)
}

/// 从页面中读取定长或变长字段，同时推进游标。
///
/// 所有解码函数都走这一层，避免损坏页面导致越界切片。
fn take_bytes<'a>(data: &'a [u8], position: &mut usize, len: usize) -> KvResult<&'a [u8]> {
    let end = position
        .checked_add(len)
        .ok_or_else(|| KvError::Internal("B+Tree page offset overflow".to_string()))?;
    let bytes = data
        .get(*position..end)
        .ok_or_else(|| KvError::Internal("truncated or corrupt B+Tree page".to_string()))?;
    *position = end;
    Ok(bytes)
}

fn read_u16(data: &[u8], position: &mut usize) -> KvResult<u16> {
    let bytes: [u8; 2] = take_bytes(data, position, 2)?
        .try_into()
        .map_err(|_| KvError::Internal("invalid u16 field in B+Tree page".to_string()))?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u32(data: &[u8], position: &mut usize) -> KvResult<u32> {
    let bytes: [u8; 4] = take_bytes(data, position, 4)?
        .try_into()
        .map_err(|_| KvError::Internal("invalid u32 field in B+Tree page".to_string()))?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(data: &[u8], position: &mut usize) -> KvResult<u64> {
    let bytes: [u8; 8] = take_bytes(data, position, 8)?
        .try_into()
        .map_err(|_| KvError::Internal("invalid u64 field in B+Tree page".to_string()))?;
    Ok(u64::from_le_bytes(bytes))
}

fn page_flag(data: &[u8]) -> KvResult<u8> {
    match data.first().copied() {
        Some(FLAG_INTERNAL) => Ok(FLAG_INTERNAL),
        Some(FLAG_LEAF) => Ok(FLAG_LEAF),
        Some(flag) => Err(KvError::Internal(format!(
            "unknown B+Tree page type: {flag}"
        ))),
        None => Err(KvError::Internal(
            "empty B+Tree page returned by pager".to_string(),
        )),
    }
}

fn encode_internal_node(keys: &[Vec<u8>], children: &[u64]) -> KvResult<Vec<u8>> {
    if children.len() != keys.len() + 1 {
        return Err(KvError::Internal(
            "B+Tree internal node has an invalid child count".to_string(),
        ));
    }
    let mut buf = vec![FLAG_INTERNAL];
    buf.extend(&(keys.len() as u16).to_le_bytes());
    for k in keys {
        buf.extend(&(k.len() as u16).to_le_bytes());
        buf.extend(k);
    }
    for &c in children {
        buf.extend(&c.to_le_bytes());
    }
    finish_page(buf)
}

/// 编码叶子节点。
///
/// 布局为：类型标记、键数量、所有键、所有值、下一叶子页号。
fn encode_leaf_node(keys: &[Vec<u8>], values: &[Vec<u8>], next: u64) -> KvResult<Vec<u8>> {
    if keys.len() != values.len() {
        return Err(KvError::Internal(
            "B+Tree leaf has different key and value counts".to_string(),
        ));
    }
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
    finish_page(buf)
}

fn decode_leaf_entry(data: &[u8]) -> KvResult<DecodedLeaf> {
    if data.first() != Some(&FLAG_LEAF) {
        return Err(KvError::Internal("expected B+Tree leaf page".to_string()));
    }
    let mut pos = 1;
    let num_keys = read_u16(data, &mut pos)? as usize;
    let mut keys = Vec::with_capacity(num_keys);
    for _ in 0..num_keys {
        let len = read_u16(data, &mut pos)? as usize;
        keys.push(take_bytes(data, &mut pos, len)?.to_vec());
    }
    let mut values = Vec::with_capacity(num_keys);
    for _ in 0..num_keys {
        let len = read_u32(data, &mut pos)? as usize;
        values.push(take_bytes(data, &mut pos, len)?.to_vec());
    }
    let next = read_u64(data, &mut pos)?;
    Ok((keys, values, next))
}

fn decode_internal_entry(data: &[u8]) -> KvResult<(Vec<Vec<u8>>, Vec<u64>)> {
    if data.first() != Some(&FLAG_INTERNAL) {
        return Err(KvError::Internal(
            "expected B+Tree internal page".to_string(),
        ));
    }
    let mut pos = 1;
    let num_keys = read_u16(data, &mut pos)? as usize;
    let mut keys = Vec::with_capacity(num_keys);
    for _ in 0..num_keys {
        let len = read_u16(data, &mut pos)? as usize;
        keys.push(take_bytes(data, &mut pos, len)?.to_vec());
    }
    let mut children = Vec::with_capacity(num_keys + 1);
    for _ in 0..=num_keys {
        children.push(read_u64(data, &mut pos)?);
    }
    Ok((keys, children))
}

fn find_key_index(keys: &[Vec<u8>], target: &[u8]) -> usize {
    keys.iter()
        .position(|k| target < k.as_slice())
        .unwrap_or(keys.len())
}

/// 以有序字节串为键的 B+Tree。
///
/// 叶节点通过 `next` 页号串联，因此范围扫描不需要返回父节点。
pub struct BPlusTree {
    /// 底层页面读写接口。
    pub pager: Arc<dyn Pager>,
    /// 当前根页号。根分裂或根收缩时会更新该值。
    pub root_page_id: AtomicU64,
}

impl BPlusTree {
    /// 分配一个空叶节点作为新树的根。
    pub async fn new(pager: Arc<dyn Pager>) -> KvResult<Self> {
        let root_page_id = pager.allocate_page().await?;
        let leaf = encode_leaf_node(&[], &[], 0)?;
        pager.write_page(root_page_id, &leaf).await?;
        Ok(BPlusTree {
            pager,
            root_page_id: AtomicU64::new(root_page_id),
        })
    }

    /// 使用目录中已持久化的根页号打开树。
    pub fn open(pager: Arc<dyn Pager>, root_page_id: u64) -> Self {
        BPlusTree {
            pager,
            root_page_id: AtomicU64::new(root_page_id),
        }
    }

    /// 插入或替换键值；节点超过单页容量时返回错误而不截断数据。
    pub async fn insert(&self, key: &[u8], value: &[u8]) -> KvResult<()> {
        let result = self
            .insert_recursive(self.root_page_id.load(Ordering::Relaxed), key, value)
            .await?;
        if let (Some(promo_key), Some(new_child)) = result {
            let new_root = encode_internal_node(
                &[promo_key],
                &[self.root_page_id.load(Ordering::Relaxed), new_child],
            )?;
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
        let flag = page_flag(&data)?;

        if flag == FLAG_LEAF {
            let (mut keys, mut values, next) = decode_leaf_entry(&data)?;
            let idx = keys.binary_search(&key.to_vec()).unwrap_or_else(|e| e);
            if idx < keys.len() && keys[idx] == key {
                values[idx] = value.to_vec();
                let new_data = encode_leaf_node(&keys, &values, next)?;
                self.pager.write_page(page_id, &new_data).await?;
                return Ok((None, None));
            }
            keys.insert(idx, key.to_vec());
            values.insert(idx, value.to_vec());

            if keys.len() > MAX_KEYS {
                // 叶子分裂时把右页第一个键提升给父节点；该键仍保留在右页中，
                // 这是 B+Tree 与 B-Tree 在叶子层的主要区别。
                let mid = keys.len() / 2;
                let right_keys = keys.split_off(mid);
                let right_vals = values.split_off(mid);
                let promo = right_keys[0].clone();

                let right_id = self.pager.allocate_page().await?;
                let right_data = encode_leaf_node(&right_keys, &right_vals, next)?;
                self.pager.write_page(right_id, &right_data).await?;

                let left_data = encode_leaf_node(&keys, &values, right_id)?;
                self.pager.write_page(page_id, &left_data).await?;
                Ok((Some(promo), Some(right_id)))
            } else {
                let new_data = encode_leaf_node(&keys, &values, next)?;
                self.pager.write_page(page_id, &new_data).await?;
                Ok((None, None))
            }
        } else {
            let (keys, children) = decode_internal_entry(&data)?;
            let child_idx = find_key_index(&keys, key);
            let child_page = children[child_idx];

            let (maybe_promo, maybe_new_child) =
                Box::pin(self.insert_recursive(child_page, key, value)).await?;

            match (maybe_promo, maybe_new_child) {
                (Some(promo_key), Some(new_child_id)) => {
                    let (mut int_keys, mut int_children) = decode_internal_entry(&data)?;
                    let insert_idx = int_keys.binary_search(&promo_key).unwrap_or_else(|e| e);
                    int_keys.insert(insert_idx, promo_key);
                    int_children.insert(insert_idx + 1, new_child_id);

                    if int_keys.len() > MAX_KEYS {
                        // 内部节点分裂时中间键只进入父节点，不留在左右子节点。
                        let mid = int_keys.len() / 2;
                        let promo = int_keys[mid].clone();
                        let right_keys = int_keys.split_off(mid + 1);
                        int_keys.pop();
                        let right_children = int_children.split_off(mid + 1);

                        let right_id = self.pager.allocate_page().await?;
                        let right_data = encode_internal_node(&right_keys, &right_children)?;
                        self.pager.write_page(right_id, &right_data).await?;

                        let left_data = encode_internal_node(&int_keys, &int_children)?;
                        self.pager.write_page(page_id, &left_data).await?;
                        Ok((Some(promo), Some(right_id)))
                    } else {
                        let new_data = encode_internal_node(&int_keys, &int_children)?;
                        self.pager.write_page(page_id, &new_data).await?;
                        Ok((None, None))
                    }
                }
                _ => Ok((None, None)),
            }
        }
    }

    /// 查找单个键。
    pub async fn search(&self, key: &[u8]) -> KvResult<Option<Vec<u8>>> {
        let mut page_id = self.root_page_id.load(Ordering::Relaxed);
        loop {
            let data = self.pager.read_page(page_id).await?;
            if page_flag(&data)? == FLAG_LEAF {
                let (keys, values, _) = decode_leaf_entry(&data)?;
                return Ok(keys
                    .iter()
                    .position(|k| k == key)
                    .map(|i| values[i].clone()));
            }
            let (keys, children) = decode_internal_entry(&data)?;
            page_id = children[find_key_index(&keys, key)];
        }
    }

    /// 按键顺序扫描半开区间 `[start, end)`。
    pub async fn scan(&self, start: &[u8], end: &[u8]) -> KvResult<Vec<(Vec<u8>, Vec<u8>)>> {
        let mut page_id = self.root_page_id.load(Ordering::Relaxed);
        loop {
            let data = self.pager.read_page(page_id).await?;
            if page_flag(&data)? == FLAG_LEAF {
                break;
            }
            let (keys, children) = decode_internal_entry(&data)?;
            page_id = children[find_key_index(&keys, start)];
        }

        let mut results = Vec::new();
        let mut current_id = page_id;
        loop {
            let data = self.pager.read_page(current_id).await?;
            let (keys, values, next) = decode_leaf_entry(&data)?;
            for (k, v) in keys.iter().zip(values.iter()) {
                if k.as_slice() < start {
                    continue;
                }
                if k.as_slice() >= end {
                    return Ok(results);
                }
                results.push((k.clone(), v.clone()));
            }
            if next == 0 {
                break;
            }
            current_id = next;
        }
        Ok(results)
    }

    /// 删除键。
    ///
    /// 当前实现删除叶子记录并处理空根收缩；为保持实现规模可控，未做兄弟节点借位和合并。
    pub async fn delete(&self, key: &[u8]) -> KvResult<bool> {
        let found = self
            .delete_recursive(self.root_page_id.load(Ordering::Relaxed), key)
            .await?;
        let data = self
            .pager
            .read_page(self.root_page_id.load(Ordering::Relaxed))
            .await?;
        if page_flag(&data)? == FLAG_INTERNAL {
            let (keys, children) = decode_internal_entry(&data)?;
            if keys.is_empty() && !children.is_empty() {
                self.pager
                    .free_page(self.root_page_id.load(Ordering::Relaxed))
                    .await?;
                self.root_page_id.store(children[0], Ordering::Relaxed);
            }
        }
        Ok(found)
    }

    async fn delete_recursive(&self, page_id: u64, key: &[u8]) -> KvResult<bool> {
        let data = self.pager.read_page(page_id).await?;
        if page_flag(&data)? == FLAG_LEAF {
            let (mut keys, mut values, next) = decode_leaf_entry(&data)?;
            if let Some(idx) = keys.iter().position(|k| k == key) {
                keys.remove(idx);
                values.remove(idx);
                let new_data = encode_leaf_node(&keys, &values, next)?;
                self.pager.write_page(page_id, &new_data).await?;
                return Ok(true);
            }
            Ok(false)
        } else {
            let (keys, children) = decode_internal_entry(&data)?;
            let child_idx = find_key_index(&keys, key);
            Box::pin(self.delete_recursive(children[child_idx], key)).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct MemPager {
        pages: Mutex<HashMap<u64, Vec<u8>>>,
        counter: Mutex<u64>,
    }

    impl MemPager {
        fn new() -> Self {
            MemPager {
                pages: Mutex::new(HashMap::new()),
                counter: Mutex::new(1),
            }
        }
    }

    #[async_trait]
    impl Pager for MemPager {
        async fn read_page(&self, page_id: u64) -> KvResult<Vec<u8>> {
            let pages = self.pages.lock().unwrap();
            Ok(pages
                .get(&page_id)
                .cloned()
                .unwrap_or_else(|| vec![0u8; 4096]))
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

        async fn flush(&self) -> KvResult<()> {
            Ok(())
        }
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
        assert!(!results.is_empty());
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

    #[tokio::test]
    async fn test_oversized_entry_is_rejected_without_corrupting_tree() {
        let tree = make_tree().await;
        let oversized = vec![0u8; PAGE_SIZE];
        assert!(tree.insert(b"key", &oversized).await.is_err());
        assert_eq!(tree.search(b"key").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_corrupt_page_returns_error() {
        let tree = make_tree().await;
        let root = tree.root_page_id.load(Ordering::Relaxed);
        tree.pager
            .write_page(root, &[FLAG_LEAF, 1, 0, 0xff, 0xff])
            .await
            .unwrap();
        assert!(tree.search(b"key").await.is_err());
    }
}
