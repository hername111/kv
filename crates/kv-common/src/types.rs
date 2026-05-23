// 基础类型定义：Row, Value, Column, DataType, Schema
#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    Int,
    Float,
    String,
    Bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub name: String,
    pub data_type: DataType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub values: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Schema {
    pub columns: Vec<Column>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_schema_test() {
        let col = Column { name: "id".to_string(), data_type: DataType::Int };
        let schema = Schema { columns: vec![col.clone()] };
        let row = Row { values: vec![Value::Int(1)] };
        assert_eq!(schema.columns.len(), 1);
        assert_eq!(row.values.len(), 1);
    }
}