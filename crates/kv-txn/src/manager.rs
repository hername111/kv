// 事务管理器：begin/commit/rollback 生命周期
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub enum TxnState {
    Active,
    Committed,
    RolledBack,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: u64,
    pub state: TxnState,
    // 简单的读写集示例：表名 -> set of row ids (as strings for generality)
    pub read_set: HashMap<String, HashSet<String>>,
    pub write_set: HashMap<String, HashSet<String>>,
}

pub struct TxnManager {
    counter: u64,
    pub transactions: HashMap<u64, Transaction>,
}

#[derive(Debug)]
pub enum TxnError {
    NotFound(u64),
    InvalidState(u64, TxnState),
}

impl std::fmt::Display for TxnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxnError::NotFound(id) => write!(f, "Transaction {} not found", id),
            TxnError::InvalidState(id, state) => write!(f, "Transaction {} invalid state: {:?}", id, state),
        }
    }
}

impl std::error::Error for TxnError {}

impl TxnManager {
    pub fn new() -> Self {
        Self { counter: 0, transactions: HashMap::new() }
    }

    /// 开始一个新事务，返回事务 id
    pub fn begin(&mut self) -> u64 {
        self.counter += 1;
        let txn = Transaction {
            id: self.counter,
            state: TxnState::Active,
            read_set: HashMap::new(),
            write_set: HashMap::new(),
        };
        self.transactions.insert(self.counter, txn);
        self.counter
    }

    /// 提交事务：只有 Active 状态可以提交
    pub fn commit(&mut self, txn_id: u64) -> Result<(), TxnError> {
        match self.transactions.get_mut(&txn_id) {
            Some(txn) => match txn.state {
                TxnState::Active => {
                    txn.state = TxnState::Committed;
                    // 提交时可在此触发日志、刷盘等操作
                    Ok(())
                }
                ref s => Err(TxnError::InvalidState(txn_id, s.clone())),
            },
            None => Err(TxnError::NotFound(txn_id)),
        }
    }

    /// 回滚事务：只有 Active 状态可以回滚
    pub fn rollback(&mut self, txn_id: u64) -> Result<(), TxnError> {
        match self.transactions.get_mut(&txn_id) {
            Some(txn) => match txn.state {
                TxnState::Active => {
                    txn.state = TxnState::RolledBack;
                    // 回滚时可在此撤销写集或触发补偿逻辑
                    Ok(())
                }
                ref s => Err(TxnError::InvalidState(txn_id, s.clone())),
            },
            None => Err(TxnError::NotFound(txn_id)),
        }
    }

    /// 标记读集合
    pub fn add_read(&mut self, txn_id: u64, table: &str, row_id: &str) -> Result<(), TxnError> {
        match self.transactions.get_mut(&txn_id) {
            Some(txn) if txn.state == TxnState::Active => {
                txn.read_set.entry(table.to_string()).or_default().insert(row_id.to_string());
                Ok(())
            }
            Some(txn) => Err(TxnError::InvalidState(txn_id, txn.state.clone())),
            None => Err(TxnError::NotFound(txn_id)),
        }
    }

    /// 标记写集合
    pub fn add_write(&mut self, txn_id: u64, table: &str, row_id: &str) -> Result<(), TxnError> {
        match self.transactions.get_mut(&txn_id) {
            Some(txn) if txn.state == TxnState::Active => {
                txn.write_set.entry(table.to_string()).or_default().insert(row_id.to_string());
                Ok(())
            }
            Some(txn) => Err(TxnError::InvalidState(txn_id, txn.state.clone())),
            None => Err(TxnError::NotFound(txn_id)),
        }
    }

    /// 查询事务状态（只读）
    pub fn status(&self, txn_id: u64) -> Option<TxnState> {
        self.transactions.get(&txn_id).map(|t| t.state.clone())
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
        assert!(matches!(mgr.rollback(id), Err(TxnError::InvalidState(_, _))));
        // 非存在事务
        assert!(matches!(mgr.commit(999), Err(TxnError::NotFound(999))));
    }
}