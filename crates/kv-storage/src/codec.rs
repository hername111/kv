//! 行值的紧凑二进制编解码。
use kv_common::types::{Row, Value};
use std::io::{Error, ErrorKind};

// 每个值以一字节标签开头：Null=0，Int=1，Float=2，String=3，Bool=4。
// 定长数值使用小端序；字符串标签后依次保存 u32 长度和 UTF-8 字节。

pub fn serialize_row(row: &Row) -> Vec<u8> {
    let mut buf = Vec::new();
    for val in &row.values {
        serialize_value_into(val, &mut buf);
    }
    buf
}

pub fn serialize_value(val: &Value) -> Vec<u8> {
    let mut buf = Vec::new();
    serialize_value_into(val, &mut buf);
    buf
}

fn serialize_value_into(val: &Value, buf: &mut Vec<u8>) {
    match val {
        Value::Null => buf.push(0x00),
        Value::Int(i) => {
            buf.push(0x01);
            buf.extend(&i.to_le_bytes());
        }
        Value::Float(f) => {
            buf.push(0x02);
            buf.extend(&f.to_le_bytes());
        }
        Value::String(s) => {
            buf.push(0x03);
            let bytes = s.as_bytes();
            buf.extend(&(bytes.len() as u32).to_le_bytes());
            buf.extend(bytes);
        }
        Value::Bool(b) => {
            buf.push(0x04);
            buf.push(*b as u8);
        }
    }
}

pub fn deserialize_row(mut data: &[u8]) -> Result<Row, Error> {
    let mut values = Vec::new();
    while !data.is_empty() {
        let tag = data[0];
        data = &data[1..];
        match tag {
            0x00 => values.push(Value::Null),
            0x01 => {
                if data.len() < 8 {
                    return Err(Error::new(
                        ErrorKind::UnexpectedEof,
                        "insufficient bytes for Int",
                    ));
                }
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&data[..8]);
                values.push(Value::Int(i64::from_le_bytes(buf)));
                data = &data[8..];
            }
            0x02 => {
                if data.len() < 8 {
                    return Err(Error::new(
                        ErrorKind::UnexpectedEof,
                        "insufficient bytes for Float",
                    ));
                }
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&data[..8]);
                values.push(Value::Float(f64::from_le_bytes(buf)));
                data = &data[8..];
            }
            0x03 => {
                if data.len() < 4 {
                    return Err(Error::new(
                        ErrorKind::UnexpectedEof,
                        "insufficient bytes for String len",
                    ));
                }
                let mut lenb = [0u8; 4];
                lenb.copy_from_slice(&data[..4]);
                let len = u32::from_le_bytes(lenb) as usize;
                data = &data[4..];
                if data.len() < len {
                    return Err(Error::new(
                        ErrorKind::UnexpectedEof,
                        "insufficient bytes for String data",
                    ));
                }
                let s = String::from_utf8(data[..len].to_vec())
                    .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
                values.push(Value::String(s));
                data = &data[len..];
            }
            0x04 => {
                if data.is_empty() {
                    return Err(Error::new(
                        ErrorKind::UnexpectedEof,
                        "insufficient bytes for Bool",
                    ));
                }
                let b = data[0] != 0;
                values.push(Value::Bool(b));
                data = &data[1..];
            }
            other => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("unknown tag: {}", other),
                ));
            }
        }
    }
    Ok(Row { values })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kv_common::types::Value;

    #[test]
    fn codec_test() {
        let row = Row {
            values: vec![
                Value::Int(10),
                Value::Float(2.5),
                Value::String("hello".to_string()),
                Value::Bool(true),
                Value::Null,
            ],
        };

        let buf = serialize_row(&row);
        let decoded = deserialize_row(&buf).unwrap();
        assert_eq!(decoded.values.len(), 5);

        match &decoded.values[0] {
            Value::Int(i) => assert_eq!(*i, 10),
            _ => panic!("expected Int"),
        }
        match &decoded.values[1] {
            Value::Float(f) => assert!((*f - 2.5).abs() < 1e-9),
            _ => panic!("expected Float"),
        }
        match &decoded.values[2] {
            Value::String(s) => assert_eq!(s, "hello"),
            _ => panic!("expected String"),
        }
        match &decoded.values[3] {
            Value::Bool(b) => assert!(*b),
            _ => panic!("expected Bool"),
        }
        match &decoded.values[4] {
            Value::Null => (),
            _ => panic!("expected Null"),
        }
    }
}
