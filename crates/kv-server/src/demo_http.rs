use kv_common::types::{ColumnDef, ResultSet, Row, Session, TableMeta, Value};
use kv_sql::{SqlExecutor, TableSnapshot};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const INDEX_HTML: &str = "KV demo API is running. Start demo-client with npm run dev.";

pub async fn start_demo_http(addr: String, executor: Arc<SqlExecutor>) -> std::io::Result<()> {
    let listener = TcpListener::bind(&addr).await?;
    let session = Arc::new(Session::new());
    println!("Demo HTTP API listening on http://{}", addr);
    loop {
        let (stream, peer) = listener.accept().await?;
        let executor = executor.clone();
        let session = session.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_http(stream, executor, session).await {
                eprintln!("Demo HTTP error from {}: {}", peer, err);
            }
        });
    }
}

async fn handle_http(
    mut stream: TcpStream,
    executor: Arc<SqlExecutor>,
    session: Arc<Session>,
) -> std::io::Result<()> {
    let mut buffer = vec![0u8; 16384];
    let read = stream.read(&mut buffer).await?;
    if read == 0 {
        return Ok(());
    }
    let request = String::from_utf8_lossy(&buffer[..read]);
    let Some((head, initial_body)) = request.split_once("\r\n\r\n") else {
        return write_response(&mut stream, 400, "text/plain", "Bad request").await;
    };
    let mut lines = head.lines();
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();
    let content_length = lines
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    let mut body = initial_body.as_bytes().to_vec();
    while body.len() < content_length {
        let mut chunk = vec![0u8; content_length - body.len()];
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    let body = String::from_utf8_lossy(&body);

    match (method, path) {
        ("OPTIONS", _) => write_response(&mut stream, 204, "text/plain", "").await,
        ("GET", "/") => write_response(&mut stream, 200, "text/plain", INDEX_HTML).await,
        ("GET", "/api/state") => {
            let body = match make_state_json(&executor).await {
                Ok(json) => json,
                Err(err) => format!("{{\"ok\":false,\"error\":{}}}", json_string(&err)),
            };
            write_response(&mut stream, 200, "application/json", &body).await
        }
        ("POST", "/api/query") => {
            let sql = extract_json_string(&body, "sql").unwrap_or_default();
            let started_at = Instant::now();
            let result = executor.execute_sql(&sql, session.as_ref()).await;
            let duration_micros = started_at.elapsed().as_micros();
            let state = make_state_json(&executor)
                .await
                .unwrap_or_else(|err| format!("{{\"ok\":false,\"error\":{}}}", json_string(&err)));
            let body = match result {
                Ok(rs) => format!(
                    "{{\"ok\":true,\"sql\":{},\"durationMicros\":{},\"result\":{},\"state\":{}}}",
                    json_string(&sql),
                    duration_micros,
                    result_set_json(&rs),
                    state
                ),
                Err(err) => format!(
                    "{{\"ok\":false,\"sql\":{},\"durationMicros\":{},\"error\":{},\"state\":{}}}",
                    json_string(&sql),
                    duration_micros,
                    json_string(&err.to_string()),
                    state
                ),
            };
            write_response(&mut stream, 200, "application/json", &body).await
        }
        ("POST", "/api/reset") => {
            let body = match reset_demo(&executor, session.as_ref()).await {
                Ok(state) => state,
                Err(err) => format!("{{\"ok\":false,\"error\":{}}}", json_string(&err)),
            };
            write_response(&mut stream, 200, "application/json", &body).await
        }
        _ => write_response(&mut stream, 404, "application/json", "{\"ok\":false}").await,
    }
}

async fn reset_demo(executor: &SqlExecutor, session: &Session) -> Result<String, String> {
    if session.txn_id().is_some() {
        executor
            .execute_sql("ROLLBACK", session)
            .await
            .map_err(|err| err.to_string())?;
    }
    let tables = executor
        .snapshot_tables()
        .await
        .map_err(|err| err.to_string())?;
    for table in tables {
        executor
            .execute_sql(&format!("DROP TABLE {}", table.meta.table_name), session)
            .await
            .map_err(|err| err.to_string())?;
    }
    make_state_json(executor).await
}

async fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "OK",
    };
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type\r\nAccess-Control-Allow-Methods: GET,POST,OPTIONS\r\nConnection: close\r\n\r\n{}",
        status,
        reason,
        content_type,
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).await
}

async fn make_state_json(executor: &SqlExecutor) -> Result<String, String> {
    let tables = executor
        .snapshot_tables()
        .await
        .map_err(|err| err.to_string())?;
    Ok(format!(
        "{{\"ok\":true,\"tables\":[{}]}}",
        tables
            .iter()
            .map(table_snapshot_json)
            .collect::<Vec<_>>()
            .join(",")
    ))
}

fn table_snapshot_json(snapshot: &TableSnapshot) -> String {
    format!(
        "{{\"meta\":{},\"rows\":[{}]}}",
        table_meta_json(&snapshot.meta),
        snapshot
            .rows
            .iter()
            .map(row_json)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn table_meta_json(meta: &TableMeta) -> String {
    format!(
        "{{\"tableId\":{},\"tableName\":{},\"primaryKeyIndex\":{},\"columns\":[{}],\"indexes\":{}}}",
        meta.table_id,
        json_string(&meta.table_name),
        meta.primary_key_index,
        meta.columns
            .iter()
            .map(column_json)
            .collect::<Vec<_>>()
            .join(","),
        meta.indexes.len()
    )
}

fn result_set_json(rs: &ResultSet) -> String {
    format!(
        "{{\"columns\":[{}],\"rows\":[{}],\"affectedRows\":{},\"lastInsertId\":{}}}",
        rs.columns
            .iter()
            .map(column_json)
            .collect::<Vec<_>>()
            .join(","),
        rs.rows.iter().map(row_json).collect::<Vec<_>>().join(","),
        rs.affected_rows,
        rs.last_insert_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "null".to_string())
    )
}

fn column_json(column: &ColumnDef) -> String {
    format!(
        "{{\"id\":{},\"name\":{},\"dataType\":{},\"nullable\":{},\"primaryKey\":{}}}",
        column.id,
        json_string(&column.name),
        json_string(&format!("{:?}", column.data_type)),
        column.nullable,
        column.is_primary_key
    )
}

fn row_json(row: &Row) -> String {
    format!(
        "[{}]",
        row.values
            .iter()
            .map(value_json)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn value_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Int(value) => value.to_string(),
        Value::Float(value) => value.to_string(),
        Value::String(value) => json_string(value),
        Value::Bool(value) => value.to_string(),
    }
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn extract_json_string(body: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\"", key);
    let key_pos = body.find(&needle)?;
    let colon_pos = body[key_pos + needle.len()..].find(':')? + key_pos + needle.len();
    let start_quote = body[colon_pos + 1..].find('"')? + colon_pos + 1;
    parse_json_string_at(body, start_quote)
}

fn parse_json_string_at(input: &str, quote_pos: usize) -> Option<String> {
    let mut escaped = false;
    let mut out = String::new();
    for ch in input[quote_pos + 1..].chars() {
        if escaped {
            match ch {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                other => out.push(other),
            }
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(out);
        } else {
            out.push(ch);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use kv_storage::{DiskPager, KvStorage};

    #[tokio::test]
    async fn reset_rolls_back_active_transaction_and_clears_tables() {
        let dir = tempfile::tempdir().unwrap();
        let disk = Arc::new(DiskPager::open(dir.path().join("reset.db")).unwrap());
        let storage = Arc::new(KvStorage::new(disk, 16));
        let executor = SqlExecutor::new(storage);
        executor.load_catalog().await.unwrap();
        let session = Session::new();

        executor
            .execute_sql(
                "CREATE TABLE demo (id INT PRIMARY KEY, value INT)",
                &session,
            )
            .await
            .unwrap();
        executor.execute_sql("BEGIN", &session).await.unwrap();
        executor
            .execute_sql("INSERT INTO demo VALUES (1, 10)", &session)
            .await
            .unwrap();

        let state = reset_demo(&executor, &session).await.unwrap();
        assert!(session.txn_id().is_none());
        assert!(executor.snapshot_tables().await.unwrap().is_empty());
        assert_eq!(state, "{\"ok\":true,\"tables\":[]}");
    }
}
