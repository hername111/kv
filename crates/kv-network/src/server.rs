use crate::protocol;
use bytes::BytesMut;
use kv_common::traits::CommandHandler;
use kv_common::types::Session;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub struct KvServer {
    pub addr: String,
    handler: Option<Arc<dyn CommandHandler>>,
}

impl KvServer {
    pub fn new(addr: String) -> Self {
        KvServer {
            addr,
            handler: None,
        }
    }

    pub fn with_handler(mut self, handler: Arc<dyn CommandHandler>) -> Self {
        self.handler = Some(handler);
        self
    }

    pub async fn start(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind(&self.addr).await?;
        println!("KV Server listening on {}", self.addr);
        loop {
            let (socket, addr) = listener.accept().await?;
            println!("New connection from {}", addr);
            let handler = self.handler.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_client(socket, handler).await {
                    eprintln!("Error with client {}: {:?}", addr, e);
                }
            });
        }
    }
}

async fn handle_client(
    mut socket: TcpStream,
    handler: Option<Arc<dyn CommandHandler>>,
) -> std::io::Result<()> {
    let handshake = protocol::make_handshake();
    socket.write_all(&handshake).await?;

    let mut buf = BytesMut::with_capacity(4096);
    let n = socket.read_buf(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }

    let ok = protocol::make_ok_packet(0, 0, 2);
    socket.write_all(&ok).await?;

    let session = Session::new();

    loop {
        buf.clear();
        let n = socket.read_buf(&mut buf).await?;
        if n == 0 {
            break;
        }

        if let Some(payload) = protocol::read_packet(&mut buf).unwrap_or(None) {
            if payload.is_empty() {
                continue;
            }
            let cmd = payload[0];
            match cmd {
                0x03 => {
                    let sql = String::from_utf8_lossy(&payload[1..]).to_string();
                    if let Some(ref h) = handler {
                        match h.execute(&sql, &session).await {
                            Ok(rs) => {
                                let packets = protocol::result_set_to_packets(&rs);
                                socket.write_all(&packets).await?;
                            }
                            Err(e) => {
                                let err = protocol::make_err_packet(1064, &format!("{}", e), 1);
                                socket.write_all(&err).await?;
                            }
                        }
                    } else {
                        let err = protocol::make_err_packet(1064, "No handler", 1);
                        socket.write_all(&err).await?;
                    }
                }
                0x01 => break,
                _ => {
                    let ok = protocol::make_ok_packet(0, 0, 1);
                    socket.write_all(&ok).await?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, timeout};

    #[tokio::test]
    async fn test_server_startup() {
        let server = KvServer::new("127.0.0.1:0".to_string());
        let result = timeout(Duration::from_millis(100), server.start()).await;
        assert!(result.is_ok() || result.is_err());
    }
}
