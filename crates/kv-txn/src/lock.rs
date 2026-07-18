//! 带超时清理的表级共享锁和排他锁。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// 表锁模式。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LockMode {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone)]
struct LockEntry {
    txn_id: u64,
    mode: LockMode,
    acquired_at: Instant,
}

/// 按表名保存锁持有者；过期锁在下一次加锁时惰性清理。
pub struct LockManager {
    locks: Mutex<HashMap<String, Vec<LockEntry>>>,
    timeout: Duration,
}

impl LockManager {
    /// 创建表级锁管理器。`timeout_ms` 用于清理异常会话遗留的锁。
    pub fn new(timeout_ms: u64) -> Self {
        LockManager {
            locks: Mutex::new(HashMap::new()),
            timeout: Duration::from_millis(timeout_ms),
        }
    }

    /// 尝试获取共享锁。多个事务可同时持有同一表的共享锁。
    pub fn try_lock_shared(&self, txn_id: u64, table: &str) -> Result<(), String> {
        let mut locks = self.locks.lock().unwrap();
        self.remove_expired(&mut locks);
        let entries = locks.entry(table.to_string()).or_default();

        if entries
            .iter()
            .any(|e| e.txn_id != txn_id && e.mode == LockMode::Exclusive)
        {
            return Err(format!("table {} locked exclusively", table));
        }

        if !entries.iter().any(|e| e.txn_id == txn_id) {
            entries.push(LockEntry {
                txn_id,
                mode: LockMode::Shared,
                acquired_at: Instant::now(),
            });
        }
        Ok(())
    }

    /// 尝试获取排他锁。同一事务可将已有锁升级为排他锁。
    pub fn try_lock_exclusive(&self, txn_id: u64, table: &str) -> Result<(), String> {
        let mut locks = self.locks.lock().unwrap();
        self.remove_expired(&mut locks);
        let entries = locks.entry(table.to_string()).or_default();

        if entries.iter().any(|e| e.txn_id != txn_id) {
            return Err(format!("table {} has active locks", table));
        }

        entries.retain(|e| e.txn_id == txn_id);
        entries.push(LockEntry {
            txn_id,
            mode: LockMode::Exclusive,
            acquired_at: Instant::now(),
        });
        Ok(())
    }

    /// 释放指定事务持有的全部锁。
    pub fn unlock_all(&self, txn_id: u64) {
        let mut locks = self.locks.lock().unwrap();
        for entries in locks.values_mut() {
            entries.retain(|e| e.txn_id != txn_id);
        }
        locks.retain(|_, v| !v.is_empty());
    }

    /// 查询事务是否持有某张表上的锁。
    pub fn has_lock(&self, txn_id: u64, table: &str) -> bool {
        let locks = self.locks.lock().unwrap();
        locks
            .get(table)
            .is_some_and(|e| e.iter().any(|l| l.txn_id == txn_id))
    }

    fn remove_expired(&self, locks: &mut HashMap<String, Vec<LockEntry>>) {
        let now = Instant::now();
        // 锁超时采用惰性清理，避免为课程项目引入后台清理线程。
        locks.values_mut().for_each(|entries| {
            entries.retain(|entry| now.duration_since(entry.acquired_at) <= self.timeout)
        });
        locks.retain(|_, entries| !entries.is_empty());
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
