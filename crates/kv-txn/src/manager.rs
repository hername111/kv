//! 事务状态机和读写集合。
use std::collections::{HashMap, HashSet};

/// 事务只能从 `Active` 转移到一个终态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxnState {
    Active,
    Committed,
    RolledBack,
}

#[derive(Debug, Clone)]
struct Transaction {
    state: TxnState,
    read_set: HashMap<String, HashSet<String>>,
    write_set: HashMap<String, HashSet<String>>,
}

/// 分配事务 ID 并维护事务状态与读写集合。
pub struct TxnManager {
    counter: u64,
    transactions: HashMap<u64, Transaction>,
}

/// 事务状态机错误。
#[derive(Debug)]
pub enum TxnError {
    NotFound(u64),
    InvalidState(u64, TxnState),
}

impl std::fmt::Display for TxnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxnError::NotFound(id) => write!(f, "Transaction {} not found", id),
            TxnError::InvalidState(id, state) => {
                write!(f, "Transaction {} invalid state: {:?}", id, state)
            }
        }
    }
}

impl std::error::Error for TxnError {}

impl Default for TxnManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TxnManager {
    /// 创建空事务管理器。
    pub fn new() -> Self {
        Self {
            counter: 0,
            transactions: HashMap::new(),
        }
    }

    /// 开始事务并返回单调递增的事务 ID。
    pub fn begin(&mut self) -> u64 {
        self.counter += 1;
        let txn = Transaction {
            state: TxnState::Active,
            read_set: HashMap::new(),
            write_set: HashMap::new(),
        };
        self.transactions.insert(self.counter, txn);
        self.counter
    }

    /// 将活动事务标记为已提交。
    pub fn commit(&mut self, txn_id: u64) -> Result<(), TxnError> {
        match self.transactions.get_mut(&txn_id) {
            Some(txn) => match txn.state {
                TxnState::Active => {
                    txn.state = TxnState::Committed;
                    Ok(())
                }
                state => Err(TxnError::InvalidState(txn_id, state)),
            },
            None => Err(TxnError::NotFound(txn_id)),
        }
    }

    /// 将活动事务标记为已回滚。
    pub fn rollback(&mut self, txn_id: u64) -> Result<(), TxnError> {
        match self.transactions.get_mut(&txn_id) {
            Some(txn) => match txn.state {
                TxnState::Active => {
                    txn.state = TxnState::RolledBack;
                    Ok(())
                }
                state => Err(TxnError::InvalidState(txn_id, state)),
            },
            None => Err(TxnError::NotFound(txn_id)),
        }
    }

    /// 将行加入事务读集合。
    pub fn add_read(&mut self, txn_id: u64, table: &str, row_id: &str) -> Result<(), TxnError> {
        match self.transactions.get_mut(&txn_id) {
            Some(txn) if txn.state == TxnState::Active => {
                txn.read_set
                    .entry(table.to_string())
                    .or_default()
                    .insert(row_id.to_string());
                Ok(())
            }
            Some(txn) => Err(TxnError::InvalidState(txn_id, txn.state)),
            None => Err(TxnError::NotFound(txn_id)),
        }
    }

    /// 将行加入事务写集合。
    pub fn add_write(&mut self, txn_id: u64, table: &str, row_id: &str) -> Result<(), TxnError> {
        match self.transactions.get_mut(&txn_id) {
            Some(txn) if txn.state == TxnState::Active => {
                txn.write_set
                    .entry(table.to_string())
                    .or_default()
                    .insert(row_id.to_string());
                Ok(())
            }
            Some(txn) => Err(TxnError::InvalidState(txn_id, txn.state)),
            None => Err(TxnError::NotFound(txn_id)),
        }
    }

    /// 返回事务当前状态。
    pub fn status(&self, txn_id: u64) -> Option<TxnState> {
        self.transactions.get(&txn_id).map(|t| t.state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_txn_lifecycle_commit() {
        let mut mgr = TxnManager::new();
        let id = mgr.begin();
        assert_eq!(mgr.status(id).unwrap(), TxnState::Active);
        mgr.add_read(id, "t", "1").unwrap();
        mgr.add_write(id, "t", "1").unwrap();
        mgr.commit(id).unwrap();
        assert_eq!(mgr.status(id).unwrap(), TxnState::Committed);
    }

    #[test]
    fn test_txn_rollback() {
        let mut mgr = TxnManager::new();
        let id = mgr.begin();
        mgr.rollback(id).unwrap();
        assert_eq!(mgr.status(id).unwrap(), TxnState::RolledBack);
    }

    #[test]
    fn test_invalid_operations() {
        let mut mgr = TxnManager::new();
        let id = mgr.begin();
        mgr.commit(id).unwrap();
        // 再次提交应失败
        assert!(matches!(mgr.commit(id), Err(TxnError::InvalidState(_, _))));
        // 回滚已提交的事务应失败
        assert!(matches!(
            mgr.rollback(id),
            Err(TxnError::InvalidState(_, _))
        ));
        // 非存在事务
        assert!(matches!(mgr.commit(999), Err(TxnError::NotFound(999))));
    }
}
