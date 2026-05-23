// TCP Server (tokio)：连接管理、请求分发
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::{Arc, Mutex};

pub struct KvServer {
    pub addr: String,
}

impl KvServer {
    pub fn new(addr: String) -> Self {
        Self { addr }
    }

    pub async fn start(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind(&self.addr).await?;
        println!("KV Server listening on {}", self.addr);

        loop {
            let (mut socket, addr) = listener.accept().await?;
            println!("New connection from {}", addr);
            tokio::spawn(async move {
                if let Err(e) = handle_client(&mut socket).await {
                    eprintln!("Error with client {}: {:?}", addr, e);
                }
            });
        }
    }
}

async fn handle_client(socket: &mut TcpStream) -> std::io::Result<()> {
    let mut buf = [0u8; 1024];
    loop {
        let n = socket.read(&mut buf).await?;
        if n == 0 { break; }
        // 这里可以集成 MySQL 协议解析，由 hhy 的 protocol.rs 处理
        // 先简单回显
        socket.write_all(&buf[0..n]).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_server_start() {
        let server = KvServer::new("127.0.0.1:0".to_string()); // 0 随机端口
        let _ = timeout(Duration::from_millis(100), server.start()).await;
    }
}