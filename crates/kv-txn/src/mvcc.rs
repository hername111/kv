use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct Version {
    pub txn_id: u64,
    pub value: Option<Vec<u8>>,
    pub is_deleted: bool,
    pub prev_version_txn: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct VersionChain {
    pub versions: Vec<Version>,
}

impl VersionChain {
    pub fn new() -> Self {
        VersionChain {
            versions: Vec::new(),
        }
    }

    pub fn add_version(&mut self, txn_id: u64, value: Option<Vec<u8>>, is_deleted: bool) {
        let prev = self.versions.last().map(|v| v.txn_id);
        self.versions.push(Version {
            txn_id,
            value,
            is_deleted,
            prev_version_txn: prev,
        });
    }

    pub fn visible_version(&self, snapshot_txn_id: u64, active_txns: &[u64]) -> Option<&Version> {
        for v in self.versions.iter().rev() {
            if v.txn_id <= snapshot_txn_id {
                if v.txn_id == snapshot_txn_id || !active_txns.contains(&v.txn_id) {
                    return Some(v);
                }
                if let Some(prev) = v.prev_version_txn {
                    if prev <= snapshot_txn_id {
                        for pv in self.versions.iter().rev() {
                            if pv.txn_id == prev {
                                return Some(pv);
                            }
                        }
                    }
                }
            }
        }
        self.versions.first()
    }
}

pub struct MvccStore {
    pub chains: Mutex<HashMap<Vec<u8>, VersionChain>>,
}

impl MvccStore {
    pub fn new() -> Self {
        MvccStore {
            chains: Mutex::new(HashMap::new()),
        }
    }

    pub fn write(&self, key: Vec<u8>, value: Vec<u8>, txn_id: u64) {
        let mut chains = self.chains.lock().unwrap();
        chains
            .entry(key)
            .or_insert_with(VersionChain::new)
            .add_version(txn_id, Some(value), false);
    }

    pub fn delete(&self, key: Vec<u8>, txn_id: u64) {
        let mut chains = self.chains.lock().unwrap();
        chains
            .entry(key)
            .or_insert_with(VersionChain::new)
            .add_version(txn_id, None, true);
    }

    pub fn read(&self, key: &[u8], snapshot_txn_id: u64, active_txns: &[u64]) -> Option<Vec<u8>> {
        let chains = self.chains.lock().unwrap();
        chains.get(key).and_then(|chain| {
            chain
                .visible_version(snapshot_txn_id, active_txns)
                .and_then(|v| v.value.clone().filter(|_| !v.is_deleted))
        })
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
