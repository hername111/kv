use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LockMode {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone)]
pub struct LockEntry {
    pub txn_id: u64,
    pub mode: LockMode,
    pub table: String,
    pub acquired_at: Instant,
}

pub struct LockManager {
    locks: Mutex<HashMap<String, Vec<LockEntry>>>,
    timeout: Duration,
}

impl LockManager {
    pub fn new(timeout_ms: u64) -> Self {
        LockManager {
            locks: Mutex::new(HashMap::new()),
            timeout: Duration::from_millis(timeout_ms),
        }
    }

    pub fn try_lock_shared(&self, txn_id: u64, table: &str) -> Result<(), String> {
        let mut locks = self.locks.lock().unwrap();
        let entries = locks.entry(table.to_string()).or_default();

        if entries.iter().any(|e| e.txn_id != txn_id && e.mode == LockMode::Exclusive) {
            return Err(format!("table {} locked exclusively", table));
        }

        if !entries.iter().any(|e| e.txn_id == txn_id) {
            entries.push(LockEntry {
                txn_id,
                mode: LockMode::Shared,
                table: table.to_string(),
                acquired_at: Instant::now(),
            });
        }
        Ok(())
    }

    pub fn try_lock_exclusive(&self, txn_id: u64, table: &str) -> Result<(), String> {
        let mut locks = self.locks.lock().unwrap();
        let entries = locks.entry(table.to_string()).or_default();

        if entries.iter().any(|e| e.txn_id != txn_id) {
            return Err(format!("table {} has active locks", table));
        }

        entries.retain(|e| e.txn_id == txn_id);
        entries.push(LockEntry {
            txn_id,
            mode: LockMode::Exclusive,
            table: table.to_string(),
            acquired_at: Instant::now(),
        });
        Ok(())
    }

    pub fn unlock_all(&self, txn_id: u64) {
        let mut locks = self.locks.lock().unwrap();
        for entries in locks.values_mut() {
            entries.retain(|e| e.txn_id != txn_id);
        }
        locks.retain(|_, v| !v.is_empty());
    }

    pub fn has_lock(&self, txn_id: u64, table: &str) -> bool {
        let locks = self.locks.lock().unwrap();
        locks.get(table).map_or(false, |e| e.iter().any(|l| l.txn_id == txn_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_lock() {
        let lm = LockManager::new(1000);
        assert!(lm.try_lock_shared(1, "t").is_ok());
        assert!(lm.try_lock_shared(2, "t").is_ok());
    }

    #[test]
    fn test_exclusive_lock_blocks() {
        let lm = LockManager::new(1000);
        lm.try_lock_exclusive(1, "t").unwrap();
        assert!(lm.try_lock_shared(2, "t").is_err());
        assert!(lm.try_lock_exclusive(2, "t").is_err());
    }

    #[test]
    fn test_unlock() {
        let lm = LockManager::new(1000);
        lm.try_lock_exclusive(1, "t").unwrap();
        lm.unlock_all(1);
        assert!(lm.try_lock_shared(2, "t").is_ok());
    }
}
