// 统一错误类型 KvError
#[derive(Debug)]
pub enum KvError {
    NotFound(String),
    InvalidQuery(String),
    TypeMismatch(String),
    IOError(std::io::Error),
}

impl From<std::io::Error> for KvError {
    fn from(e: std::io::Error) -> Self {
        KvError::IOError(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_creation() {
        let e = KvError::NotFound("table".to_string());
        match e {
            KvError::NotFound(ref msg) => assert_eq!(msg, "table"),
            _ => panic!("Unexpected variant"),
        }
    }
}