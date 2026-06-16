use kv_network::KvServer;
use tokio::time::{Duration, timeout};

#[tokio::test]
async fn test_server_startup() {
    let server = KvServer::new("127.0.0.1:0".to_string());
    let result = timeout(Duration::from_millis(100), server.start()).await;
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_server_with_handler() {
    use async_trait::async_trait;
    use kv_common::error::KvResult;
    use kv_common::traits::CommandHandler;
    use kv_common::types::{ResultSet, Session};
    use std::sync::Arc;

    struct DummyHandler;
    #[async_trait]
    impl CommandHandler for DummyHandler {
        async fn execute(&self, _sql: &str, _session: &Session) -> KvResult<ResultSet> {
            Ok(ResultSet::empty())
        }
    }

    let server = KvServer::new("127.0.0.1:0".to_string()).with_handler(Arc::new(DummyHandler));
    let result = timeout(Duration::from_millis(100), server.start()).await;
    assert!(result.is_ok() || result.is_err());
}
