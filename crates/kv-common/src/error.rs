//! 数据库各层共享的错误类型。
use crate::types::DataType;

/// 项目内部统一使用的结果类型。
pub type KvResult<T> = Result<T, KvError>;

/// 数据库各层共享的错误枚举。
#[derive(Debug, thiserror::Error)]
pub enum KvError {
    #[error("IO error: {0}")]
    Io(std::io::Error),

    #[error("Parse error at position {pos}: {message}")]
    ParseError { pos: usize, message: String },

    #[error("Table '{0}' not found")]
    TableNotFound(String),

    #[error("Column '{0}' not found")]
    ColumnNotFound(String),

    #[error("Index '{0}' not found")]
    IndexNotFound(String),

    #[error("Database '{0}' not found")]
    DatabaseNotFound(String),

    #[error("Type mismatch: expected {expected:?}, got {actual:?}")]
    TypeMismatch {
        expected: DataType,
        actual: DataType,
    },

    #[error("Duplicate key: {0:?}")]
    DuplicateKey(Vec<u8>),

    #[error("Transaction conflict: txn {txn_id}")]
    TxnConflict { txn_id: u64 },

    #[error("Transaction {0} not found")]
    TxnNotFound(u64),

    #[error("Transaction {0} invalid state: {1:?}")]
    TxnInvalidState(u64, String),

    #[error("Lock timeout")]
    LockTimeout,

    #[error("Internal: {0}")]
    Internal(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("{0}")]
    InvalidQuery(String),
}

impl From<std::io::Error> for KvError {
    fn from(e: std::io::Error) -> Self {
        KvError::Io(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let e = KvError::TableNotFound("users".to_string());
        assert!(format!("{}", e).contains("users"));

        let e2 = KvError::ParseError {
            pos: 5,
            message: "unexpected token".to_string(),
        };
        assert!(format!("{}", e2).contains("5"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let kv_err: KvError = io_err.into();
        match kv_err {
            KvError::Io(_) => {}
            _ => panic!("Expected Io variant"),
        }
    }
}
