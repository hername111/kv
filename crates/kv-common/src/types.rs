// 基础类型定义：Row, Value, Column, DataType, Schema 等
use serde::{Deserialize, Serialize};

// ===== 类型别名 =====
pub type TableId = u64;
pub type ColumnId = u64;
pub type IndexId = u64;

// ===== 数据类型 =====
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DataType {
    Int,
    BigInt,
    Float,
    Double,
    VarChar(u16),
    Text,
    Bool,
    Date,
    Timestamp,
}

// ===== 列定义 =====
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub default_value: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnDef {
    pub id: ColumnId,
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub is_primary_key: bool,
}

// ===== 值定义 =====
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Null,
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "NULL"),
            Value::Int(i) => write!(f, "{}", i),
            Value::Float(v) => write!(f, "{}", v),
            Value::String(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
        }
    }
}

// ===== 行和模式 =====
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub values: Vec<Value>,
}

impl Row {
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    pub columns: Vec<ColumnDef>,
    pub primary_key_index: usize,
}

impl Schema {
    pub fn new(columns: Vec<ColumnDef>, primary_key_index: usize) -> Self {
        Self {
            columns,
            primary_key_index,
        }
    }

    pub fn column_by_name(&self, name: &str) -> Option<(usize, &ColumnDef)> {
        self.columns
            .iter()
            .enumerate()
            .find(|(_, c)| c.name.eq_ignore_ascii_case(name))
    }

    pub fn primary_key_col(&self) -> &ColumnDef {
        &self.columns[self.primary_key_index]
    }
}

// ===== 表元数据 =====
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableMeta {
    pub table_id: TableId,
    pub table_name: String,
    pub columns: Vec<ColumnDef>,
    pub primary_key_index: usize,
    pub indexes: Vec<IndexMeta>,
    #[serde(default)]
    pub root_page_id: u64,
}

impl TableMeta {
    pub fn schema(&self) -> Schema {
        Schema {
            columns: self.columns.clone(),
            primary_key_index: self.primary_key_index,
        }
    }
}

// ===== 索引元数据 =====
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMeta {
    pub index_id: IndexId,
    pub name: String,
    pub table_id: TableId,
    pub column_id: ColumnId,
    pub is_unique: bool,
}

// ===== 结果集 =====
#[derive(Debug, Clone)]
pub struct ResultSet {
    pub columns: Vec<ColumnDef>,
    pub rows: Vec<Row>,
    pub affected_rows: u64,
    pub last_insert_id: Option<u64>,
}

impl ResultSet {
    pub fn empty() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            affected_rows: 0,
            last_insert_id: None,
        }
    }

    pub fn with_rows(columns: Vec<ColumnDef>, rows: Vec<Row>) -> Self {
        Self {
            columns,
            rows,
            affected_rows: 0,
            last_insert_id: None,
        }
    }

    pub fn ok(affected_rows: u64, last_insert_id: Option<u64>) -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            affected_rows,
            last_insert_id,
        }
    }
}

// ===== 会话信息 =====
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
pub struct Session {
    pub database: Option<String>,
    txn_id: AtomicU64,
}

impl Session {
    pub fn new() -> Self {
        Self {
            database: None,
            txn_id: AtomicU64::new(0),
        }
    }

    pub fn txn_id(&self) -> Option<u64> {
        let id = self.txn_id.load(Ordering::Relaxed);
        if id == 0 { None } else { Some(id) }
    }

    pub fn set_txn_id(&self, id: Option<u64>) {
        self.txn_id.store(id.unwrap_or(0), Ordering::Relaxed);
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

// ===== 隔离级别 =====
#[derive(Debug, Clone, PartialEq, Default)]
pub enum IsolationLevel {
    #[default]
    ReadCommitted,
    RepeatableRead,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_row_schema() {
        let col = ColumnDef {
            id: 1,
            name: "id".to_string(),
            data_type: DataType::Int,
            nullable: false,
            is_primary_key: true,
        };
        let schema = Schema::new(vec![col.clone()], 0);
        let row = Row::new(vec![Value::Int(1)]);
        assert_eq!(schema.columns.len(), 1);
        assert_eq!(row.values.len(), 1);
    }

    #[test]
    fn test_result_set() {
        let rs = ResultSet::ok(1, Some(42));
        assert_eq!(rs.affected_rows, 1);
        assert_eq!(rs.last_insert_id, Some(42));
    }

    #[test]
    fn test_schema_column_by_name() {
        let cols = vec![
            ColumnDef {
                id: 1,
                name: "id".to_string(),
                data_type: DataType::Int,
                nullable: false,
                is_primary_key: true,
            },
            ColumnDef {
                id: 2,
                name: "name".to_string(),
                data_type: DataType::VarChar(100),
                nullable: true,
                is_primary_key: false,
            },
        ];
        let schema = Schema::new(cols, 0);
        let (idx, col) = schema.column_by_name("name").unwrap();
        assert_eq!(idx, 1);
        assert_eq!(col.name, "name");
    }
}
