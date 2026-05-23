use kv_common::types::{Row, Value};

#[derive(Debug, Clone)]
pub enum Statement {
    Select { table: String, columns: Vec<String> },
    Insert { table: String, row: Row },
    Update { table: String, updates: Vec<(String, String)> },
    Delete { table: String },
}

#[derive(Debug, Clone)]
pub enum Expr {
    Column(String),
    LiteralInt(i64),
    LiteralString(String),
}

#[derive(Debug, Clone)]
pub enum Operator {
    Eq,
    Neq,
    Gt,
    Lt,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kv_common::types::{Row, Value};

    #[test]
    fn test_ast_select() {
        let stmt = Statement::Select { table: "t".to_string(), columns: vec!["*".to_string()] };
        match stmt {
            Statement::Select { table, .. } => assert_eq!(table, "t"),
            _ => panic!("Expected Select"),
        }
    }

    #[test]
    fn test_ast_insert() {
        let row = Row { values: vec![Value::Int(1)] };
        let stmt = Statement::Insert { table: "t".to_string(), row };
        match stmt {
            Statement::Insert { table, .. } => assert_eq!(table, "t"),
            _ => panic!("Expected Insert"),
        }
    }
}