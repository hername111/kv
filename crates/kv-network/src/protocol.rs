// MySQL Wire Protocol
use bytes::BytesMut;
use std::io;

pub const PROTOCOL_VERSION: u8 = 10;
pub const SERVER_VERSION: &str = "5.7.32-kv";

pub fn read_packet(buf: &mut BytesMut) -> io::Result<Option<Vec<u8>>> {
    if buf.len() < 4 {
        return Ok(None);
    }
    let payload_len = buf[0] as usize | ((buf[1] as usize) << 8) | ((buf[2] as usize) << 16);
    let total = 4 + payload_len;
    if buf.len() < total {
        return Ok(None);
    }
    let _ = buf.split_to(4);
    let payload = buf.split_to(payload_len).to_vec();
    Ok(Some(payload))
}

pub fn write_packet(payload: &[u8], seq_id: u8) -> Vec<u8> {
    let len = payload.len();
    let mut packet = Vec::with_capacity(4 + len);
    packet.push((len & 0xff) as u8);
    packet.push(((len >> 8) & 0xff) as u8);
    packet.push(((len >> 16) & 0xff) as u8);
    packet.push(seq_id);
    packet.extend_from_slice(payload);
    packet
}

pub fn make_handshake() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.push(PROTOCOL_VERSION);
    payload.extend_from_slice(SERVER_VERSION.as_bytes());
    payload.push(0);
    payload.extend_from_slice(&[0u8; 4]);
    payload.extend_from_slice(b"01234567");
    payload.push(0);
    let caps: u32 = 0x0000_0fff;
    payload.extend_from_slice(&(caps as u16).to_le_bytes());
    payload.push(8);
    payload.extend_from_slice(&[0u8; 2]);
    payload.extend_from_slice(&((caps >> 16) as u16).to_le_bytes());
    payload.push(21);
    payload.extend_from_slice(&[0u8; 10]);
    payload.extend_from_slice(b"012345678901");
    payload.push(0);
    payload.extend_from_slice(b"mysql_native_password");
    payload.push(0);
    write_packet(&payload, 0)
}

pub fn make_ok_packet(_affected_rows: u64, _last_insert_id: u64, seq_id: u8) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.push(0x00);
    payload.push(0x00);
    payload.push(0x00);
    payload.extend_from_slice(&[0x02, 0x00]);
    payload.extend_from_slice(&[0x00, 0x00]);
    write_packet(&payload, seq_id)
}

pub fn make_err_packet(code: u16, message: &str, seq_id: u8) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.push(0xff);
    payload.extend_from_slice(&code.to_le_bytes());
    payload.push(b'#');
    payload.extend_from_slice(b"HY000");
    payload.extend_from_slice(message.as_bytes());
    write_packet(&payload, seq_id)
}

pub fn make_column_def(name: &str, seq_id: u8) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.push(4);
    payload.extend_from_slice(b"def");
    payload.push(0);
    payload.push(0);
    payload.push(0);
    payload.push(name.len() as u8);
    payload.extend_from_slice(name.as_bytes());
    payload.push(0);
    payload.push(0x0c);
    payload.extend_from_slice(&[0x08, 0x00]);
    payload.extend_from_slice(&[0xff, 0xff, 0xff, 0xff]);
    payload.push(0xfd);
    payload.extend_from_slice(&[0x00, 0x00]);
    payload.push(0x00);
    payload.extend_from_slice(&[0x00, 0x00]);
    write_packet(&payload, seq_id)
}

pub fn make_eof_packet(seq_id: u8) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.push(0xfe);
    payload.extend_from_slice(&[0x00, 0x00]);
    payload.extend_from_slice(&[0x02, 0x00]);
    write_packet(&payload, seq_id)
}

pub fn make_text_row(values: &[String], seq_id: u8) -> Vec<u8> {
    let mut payload = Vec::new();
    for v in values {
        payload.push(v.len() as u8);
        payload.extend_from_slice(v.as_bytes());
    }
    write_packet(&payload, seq_id)
}

pub fn build_result_set(columns: &[String], rows: &[Vec<String>]) -> Vec<u8> {
    let mut seq = 1u8;
    let mut packets = Vec::new();
    let mut count_payload = Vec::new();
    count_payload.push(columns.len() as u8);
    packets.extend_from_slice(&write_packet(&count_payload, seq));
    seq = seq.wrapping_add(1);
    for col in columns {
        packets.extend_from_slice(&make_column_def(col, seq));
        seq = seq.wrapping_add(1);
    }
    packets.extend_from_slice(&make_eof_packet(seq));
    seq = seq.wrapping_add(1);
    for row in rows {
        packets.extend_from_slice(&make_text_row(row, seq));
        seq = seq.wrapping_add(1);
    }
    packets.extend_from_slice(&make_eof_packet(seq));
    packets
}

pub fn result_set_to_packets(rs: &kv_common::types::ResultSet) -> Vec<u8> {
    let columns: Vec<String> = rs.columns.iter().map(|c| c.name.clone()).collect();
    let str_rows: Vec<Vec<String>> = rs
        .rows
        .iter()
        .map(|row| row.values.iter().map(|v| format!("{}", v)).collect())
        .collect();
    build_result_set(&columns, &str_rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handshake() {
        assert!(make_handshake().len() > 4);
    }
    #[test]
    fn test_ok() {
        assert_eq!(make_ok_packet(1, 0, 1)[4], 0x00);
    }
    #[test]
    fn test_err() {
        assert_eq!(make_err_packet(1049, "x", 1)[4], 0xff);
    }
    #[test]
    fn test_read_write() {
        let p = write_packet(b"hi", 5);
        let mut b = BytesMut::from(&p[..]);
        assert_eq!(read_packet(&mut b).unwrap().unwrap(), b"hi");
    }
    #[test]
    fn test_result_set() {
        let p = build_result_set(&["id".to_string()], &[vec!["1".to_string()]]);
        assert!(p.len() > 4);
    }
}
