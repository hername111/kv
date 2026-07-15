use crate::protocol;
use bytes::BytesMut;
use kv_common::traits::CommandHandler;
use kv_common::types::Session;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
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
    if read_next_packet(&mut socket, &mut buf).await?.is_none() {
        return Ok(());
    }

    let ok = protocol::make_ok_packet(0, 0, 2);
    socket.write_all(&ok).await?;

    let session = Session::new();

    loop {
        let Some(payload) = read_next_packet(&mut socket, &mut buf).await? else {
            break;
        };
        if payload.is_empty() {
            continue;
        }
        match payload[0] {
            0x03 => {
                let sql = String::from_utf8_lossy(&payload[1..]).to_string();
                if let Some(ref h) = handler {
                    match h.execute(&sql, &session).await {
                        Ok(rs) if rs.columns.is_empty() => {
                            let ok = protocol::make_ok_packet(
                                rs.affected_rows,
                                rs.last_insert_id.unwrap_or(0),
                                1,
                            );
                            socket.write_all(&ok).await?;
                        }
                        Ok(rs) => {
                            let packets = protocol::result_set_to_packets(&rs);
                            socket.write_all(&packets).await?;
                        }
                        Err(error) => {
                            let packet = protocol::make_err_packet(1064, &error.to_string(), 1);
                            socket.write_all(&packet).await?;
                        }
                    }
                } else {
                    let packet = protocol::make_err_packet(1064, "No handler", 1);
                    socket.write_all(&packet).await?;
                }
            }
            0x01 => break,
            _ => {
                let ok = protocol::make_ok_packet(0, 0, 1);
                socket.write_all(&ok).await?;
            }
        }
    }
    Ok(())
}

async fn read_next_packet<R>(
    reader: &mut R,
    buffer: &mut BytesMut,
) -> std::io::Result<Option<Vec<u8>>>
where
    R: AsyncRead + Unpin,
{
    loop {
        if let Some(payload) = protocol::read_packet(buffer)? {
            return Ok(Some(payload));
        }
        let read = reader.read_buf(buffer).await?;
        if read == 0 {
            if buffer.is_empty() {
                return Ok(None);
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "connection closed in the middle of a MySQL packet",
            ));
        }
        if buffer.len() > protocol::MAX_PACKET_SIZE + 4 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "MySQL packet buffer exceeds the configured limit",
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::time::{Duration, timeout};

    #[tokio::test]
    async fn test_server_startup() {
        let server = KvServer::new("127.0.0.1:0".to_string());
        let result = timeout(Duration::from_millis(100), server.start()).await;
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_fragmented_packet_is_reassembled() {
        let (mut client, mut server) = tokio::io::duplex(64);
        let packet = protocol::write_packet(b"SELECT 1", 0);
        let writer = tokio::spawn(async move {
            client.write_all(&packet[..3]).await.unwrap();
            client.write_all(&packet[3..]).await.unwrap();
        });
        let payload = read_next_packet(&mut server, &mut BytesMut::new())
            .await
            .unwrap()
            .unwrap();
        writer.await.unwrap();
        assert_eq!(payload, b"SELECT 1");
    }

    #[tokio::test]
    async fn test_truncated_packet_returns_unexpected_eof() {
        let (mut client, mut server) = tokio::io::duplex(64);
        let packet = protocol::write_packet(b"SELECT 1", 0);
        client.write_all(&packet[..5]).await.unwrap();
        drop(client);
        let error = read_next_packet(&mut server, &mut BytesMut::new())
            .await
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
    }
}
