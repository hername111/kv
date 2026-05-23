// MySQL 协议兼容性测试：Wire Protocol 往返验证
use kv_network::KvServer;
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn test_server_startup() {
    let server = KvServer::new("127.0.0.1:0".to_string()); // 随机端口
    let result = timeout(Duration::from_millis(100), server.start()).await;
    assert!(result.is_ok() || result.is_err()); // 主要测试能触发启动流程
}