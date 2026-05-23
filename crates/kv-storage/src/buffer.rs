// Buffer Pool：固定大小页缓存 (LRU-2 淘汰)
use std::collections::{HashMap, VecDeque};

pub struct BufferPool {
    capacity: usize,
    map: HashMap<u64, Vec<u8>>, // page_id -> page data
    lru_queue: VecDeque<u64>,   // LRU-2 queue placeholder
}

impl BufferPool {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            map: HashMap::new(),
            lru_queue: VecDeque::new(),
        }
    }

    pub fn get(&mut self, page_id: u64) -> Option<&Vec<u8>> {
        self.map.get(&page_id)
    }

    pub fn put(&mut self, page_id: u64, data: Vec<u8>) {
        if self.map.len() >= self.capacity {
            if let Some(old) = self.lru_queue.pop_front() {
                self.map.remove(&old);
            }
        }
        self.lru_queue.push_back(page_id);
        self.map.insert(page_id, data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_put_get() {
        let mut bp = BufferPool::new(2);
        bp.put(1, vec![1,2,3]);
        bp.put(2, vec![4,5,6]);
        assert_eq!(bp.get(1).unwrap(), &vec![1,2,3]);
    }
}