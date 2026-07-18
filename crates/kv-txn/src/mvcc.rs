//! 基于事务 ID 的内存版本链。

use std::collections::HashMap;
use std::sync::Mutex;

/// 一个键在某次事务中的值或删除标记。
#[derive(Debug, Clone)]
pub struct Version {
    /// 创建该版本的事务编号。
    pub txn_id: u64,
    /// 删除版本不保存值。
    pub value: Option<Vec<u8>>,
    pub is_deleted: bool,
    /// 上一个版本的事务编号，用于回退到快照可见版本。
    pub prev_version_txn: Option<u64>,
}

/// 按创建顺序保存同一键的多个版本。
#[derive(Debug, Clone)]
pub struct VersionChain {
    versions: Vec<Version>,
}

impl VersionChain {
    /// 创建空版本链。
    pub fn new() -> Self {
        VersionChain {
            versions: Vec::new(),
        }
    }

    /// 在链尾追加一个新版本。
    pub fn add_version(&mut self, txn_id: u64, value: Option<Vec<u8>>, is_deleted: bool) {
        let prev = self.versions.last().map(|v| v.txn_id);
        self.versions.push(Version {
            txn_id,
            value,
            is_deleted,
            prev_version_txn: prev,
        });
    }

    /// 返回不晚于快照且不属于其他活动事务的最新版本。
    pub fn visible_version(&self, snapshot_txn_id: u64, active_txns: &[u64]) -> Option<&Version> {
        for v in self.versions.iter().rev() {
            if v.txn_id <= snapshot_txn_id {
                if v.txn_id == snapshot_txn_id || !active_txns.contains(&v.txn_id) {
                    return Some(v);
                }
                if let Some(prev) = v.prev_version_txn
                    && prev <= snapshot_txn_id
                {
                    for pv in self.versions.iter().rev() {
                        if pv.txn_id == prev {
                            return Some(pv);
                        }
                    }
                }
            }
        }
        self.versions.first()
    }
}

impl Default for VersionChain {
    fn default() -> Self {
        Self::new()
    }
}

/// 将键映射到版本链的线程安全内存存储。
pub struct MvccStore {
    chains: Mutex<HashMap<Vec<u8>, VersionChain>>,
}

impl MvccStore {
    /// 创建空 MVCC 存储。
    pub fn new() -> Self {
        MvccStore {
            chains: Mutex::new(HashMap::new()),
        }
    }

    /// 写入一个键的新版本。
    pub fn write(&self, key: Vec<u8>, value: Vec<u8>, txn_id: u64) {
        let mut chains = self.chains.lock().unwrap();
        chains
            .entry(key)
            .or_default()
            .add_version(txn_id, Some(value), false);
    }

    /// 写入删除标记。
    pub fn delete(&self, key: Vec<u8>, txn_id: u64) {
        let mut chains = self.chains.lock().unwrap();
        chains
            .entry(key)
            .or_default()
            .add_version(txn_id, None, true);
    }

    /// 按快照事务号读取可见版本。
    pub fn read(&self, key: &[u8], snapshot_txn_id: u64, active_txns: &[u64]) -> Option<Vec<u8>> {
        let chains = self.chains.lock().unwrap();
        chains.get(key).and_then(|chain| {
            chain
                .visible_version(snapshot_txn_id, active_txns)
                .and_then(|v| v.value.clone().filter(|_| !v.is_deleted))
        })
    }
}

impl Default for MvccStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_chain() {
        let mut chain = VersionChain::new();
        chain.add_version(1, Some(b"v1".to_vec()), false);
        chain.add_version(3, Some(b"v3".to_vec()), false);
        chain.add_version(5, None, true);

        let v = chain.visible_version(2, &[]);
        assert_eq!(v.and_then(|x| x.value.clone()), Some(b"v1".to_vec()));

        let v = chain.visible_version(4, &[]);
        assert_eq!(v.and_then(|x| x.value.clone()), Some(b"v3".to_vec()));

        let v = chain.visible_version(6, &[]);
        assert!(v.unwrap().is_deleted);
    }

    #[test]
    fn test_mvcc_store() {
        let store = MvccStore::new();
        store.write(b"k".to_vec(), b"val1".to_vec(), 1);
        store.write(b"k".to_vec(), b"val2".to_vec(), 2);

        assert_eq!(store.read(b"k", 1, &[]), Some(b"val1".to_vec()));
        assert_eq!(store.read(b"k", 2, &[]), Some(b"val2".to_vec()));
    }
}
