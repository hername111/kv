//! SQL 值、行、模式、目录元数据和会话状态。
use serde::{Deserialize, Serialize};

pub type TableId = u64;
pub type ColumnId = u64;
pub type IndexId = u64;

/// SQL 层支持的列类型。
///
/// 当前执行器只实现课程项目所需的核心类型，未覆盖完整 MySQL 类型系统。
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

/// 建表语句中的列定义。
///
/// 该类型保留默认值信息，便于后续扩展 DDL；执行阶段使用 [`ColumnDef`] 保存已分配
/// 列编号后的稳定元数据。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub default_value: Option<Value>,
}

/// 已登记到目录中的列元数据。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnDef {
    /// 表内稳定列编号，用于索引元数据引用列。
    pub id: ColumnId,
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub is_primary_key: bool,
}

/// SQL 执行过程中传递的标量值。
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

/// 一行结果或一条存储记录的逻辑表示。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub values: Vec<Value>,
}

impl Row {
    /// 按 schema 中的列顺序创建行。
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }
}

/// 表结构定义。
///
/// `primary_key_index` 是 `columns` 中的下标，而不是列编号；这样执行器可以在行值数组中
/// 直接定位主键。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    pub columns: Vec<ColumnDef>,
    pub primary_key_index: usize,
}

impl Schema {
    /// 创建 schema。调用方负责保证主键下标在 `columns` 范围内。
    pub fn new(columns: Vec<ColumnDef>, primary_key_index: usize) -> Self {
        Self {
            columns,
            primary_key_index,
        }
    }

    /// 按列名查找列，大小写不敏感以匹配常见 SQL 使用习惯。
    pub fn column_by_name(&self, name: &str) -> Option<(usize, &ColumnDef)> {
        self.columns
            .iter()
            .enumerate()
            .find(|(_, c)| c.name.eq_ignore_ascii_case(name))
    }

    /// 返回主键列定义。
    pub fn primary_key_col(&self) -> &ColumnDef {
        &self.columns[self.primary_key_index]
    }
}

/// 表目录元数据，负责把 SQL 表名映射到存储层 B+Tree 根页。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableMeta {
    pub table_id: TableId,
    pub table_name: String,
    pub columns: Vec<ColumnDef>,
    pub primary_key_index: usize,
    pub indexes: Vec<IndexMeta>,
    /// 表数据 B+Tree 的根页。旧格式文件没有该字段时默认为 0。
    #[serde(default)]
    pub root_page_id: u64,
}

impl TableMeta {
    /// 提取执行器常用的 schema 视图。
    pub fn schema(&self) -> Schema {
        Schema {
            columns: self.columns.clone(),
            primary_key_index: self.primary_key_index,
        }
    }
}

/// 二级索引目录元数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMeta {
    pub index_id: IndexId,
    pub name: String,
    pub table_id: TableId,
    pub column_id: ColumnId,
    pub is_unique: bool,
}

/// SQL 执行结果。
///
/// 查询语句填充 `columns` 和 `rows`；写入类语句主要使用 `affected_rows` 和
/// `last_insert_id`。
#[derive(Debug, Clone)]
pub struct ResultSet {
    pub columns: Vec<ColumnDef>,
    pub rows: Vec<Row>,
    pub affected_rows: u64,
    pub last_insert_id: Option<u64>,
}

impl ResultSet {
    /// 不返回行也不影响数据的空结果。
    pub fn empty() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            affected_rows: 0,
            last_insert_id: None,
        }
    }

    /// 构造查询结果。
    pub fn with_rows(columns: Vec<ColumnDef>, rows: Vec<Row>) -> Self {
        Self {
            columns,
            rows,
            affected_rows: 0,
            last_insert_id: None,
        }
    }

    /// 构造写入类语句的 OK 结果。
    pub fn ok(affected_rows: u64, last_insert_id: Option<u64>) -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            affected_rows,
            last_insert_id,
        }
    }
}

use std::sync::atomic::{AtomicU64, Ordering};

/// 客户端会话状态。
///
/// `txn_id` 使用 0 表示“当前不在事务中”，避免在异步处理路径中为简单状态引入额外锁。
#[derive(Debug)]
pub struct Session {
    pub database: Option<String>,
    txn_id: AtomicU64,
}

impl Session {
    /// 创建默认会话。
    pub fn new() -> Self {
        Self {
            database: None,
            txn_id: AtomicU64::new(0),
        }
    }

    /// 返回当前事务编号；没有显式事务时返回 `None`。
    pub fn txn_id(&self) -> Option<u64> {
        let id = self.txn_id.load(Ordering::Relaxed);
        if id == 0 { None } else { Some(id) }
    }

    /// 更新当前事务编号。`None` 表示退出事务。
    pub fn set_txn_id(&self, id: Option<u64>) {
        self.txn_id.store(id.unwrap_or(0), Ordering::Relaxed);
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

/// 事务隔离级别。
///
/// 当前事务管理器以读已提交为默认行为，并保留可重复读枚举以便扩展。
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
