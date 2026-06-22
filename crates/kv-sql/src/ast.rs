// AST 节点定义：Statement, Expr, Operator, OrderBy, Join, SelectItem
use kv_common::types::{ColumnDef, Row, Value};

#[derive(Debug, Clone)]
pub enum Statement {
    Select {
        columns: Vec<SelectItem>,
        from: String,
        where_clause: Option<Expr>,
        order_by: Vec<OrderBy>,
        join: Option<Join>,
    },
    Insert {
        table: String,
        columns: Option<Vec<String>>,
        values: Vec<Vec<Expr>>,
    },
    Update {
        table: String,
        set: Vec<(String, Expr)>,
        where_clause: Option<Expr>,
    },
    Delete {
        table: String,
        where_clause: Option<Expr>,
    },
    CreateTable {
        name: String,
        columns: Vec<ColumnDef>,
        primary_key: String,
    },
    CreateIndex {
        name: String,
        table: String,
        column: String,
    },
    DropTable {
        name: String,
    },
    Begin,
    Commit,
    Rollback,
}

#[derive(Debug, Clone)]
pub enum SelectItem {
    Star,
    Column(String),
    Alias(String, String),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Column(String),
    LiteralInt(i64),
    LiteralFloat(f64),
    LiteralString(String),
    LiteralBool(bool),
    LiteralNull,
    BinaryOp(Box<Expr>, Operator, Box<Expr>),
}

impl Expr {
    /// 用给定行的值计算表达式的值
    pub fn evaluate(&self, row: &Row, column_names: &[String]) -> Option<Value> {
        match self {
            Expr::Column(name) => {
                let idx = column_names
                    .iter()
                    .position(|c| c.eq_ignore_ascii_case(name))?;
                row.values.get(idx).cloned()
            }
            Expr::LiteralInt(i) => Some(Value::Int(*i)),
            Expr::LiteralFloat(f) => Some(Value::Float(*f)),
            Expr::LiteralString(s) => Some(Value::String(s.clone())),
            Expr::LiteralBool(b) => Some(Value::Bool(*b)),
            Expr::LiteralNull => Some(Value::Null),
            Expr::BinaryOp(left, op, right) => {
                let lv = left.evaluate(row, column_names)?;
                let rv = right.evaluate(row, column_names)?;
                match op {
                    Operator::And => match (lv, rv) {
                        (Value::Bool(a), Value::Bool(b)) => Some(Value::Bool(a && b)),
                        _ => None,
                    },
                    Operator::Or => match (lv, rv) {
                        (Value::Bool(a), Value::Bool(b)) => Some(Value::Bool(a || b)),
                        _ => None,
                    },
                    _ => {
                        let cmp = compare_values(&lv, &rv)?;
                        match op {
                            Operator::Eq => Some(Value::Bool(cmp == std::cmp::Ordering::Equal)),
                            Operator::Neq => Some(Value::Bool(cmp != std::cmp::Ordering::Equal)),
                            Operator::Gt => Some(Value::Bool(cmp == std::cmp::Ordering::Greater)),
                            Operator::Lt => Some(Value::Bool(cmp == std::cmp::Ordering::Less)),
                            Operator::Gte => Some(Value::Bool(cmp != std::cmp::Ordering::Less)),
                            Operator::Lte => Some(Value::Bool(cmp != std::cmp::Ordering::Greater)),
                            _ => None,
                        }
                    }
                }
            }
        }
    }
}

fn compare_values(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Null, Value::Null) => Some(std::cmp::Ordering::Equal),
        (Value::Null, _) | (_, Value::Null) => None,
        (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
        (Value::Int(a), Value::Float(b)) => (*a as f64).partial_cmp(b),
        (Value::Float(a), Value::Int(b)) => a.partial_cmp(&(*b as f64)),
        (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
        (Value::Bool(a), Value::Bool(b)) => Some(a.cmp(b)),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub enum Operator {
    Eq,
    Neq,
    Gt,
    Lt,
    Gte,
    Lte,
    And,
    Or,
}

#[derive(Debug, Clone)]
pub struct OrderBy {
    pub column: String,
    pub ascending: bool,
}

#[derive(Debug, Clone)]
pub struct Join {
    pub table: String,
    pub on: (String, String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_select() {
        let stmt = Statement::Select {
            columns: vec![SelectItem::Star],
            from: "t".to_string(),
            where_clause: None,
            order_by: vec![],
            join: None,
        };
        match &stmt {
            Statement::Select { from, .. } => assert_eq!(from, "t"),
            _ => panic!("Expected Select"),
        }
    }

    #[test]
    fn test_ast_insert() {
        let stmt = Statement::Insert {
            table: "t".to_string(),
            columns: None,
            values: vec![vec![Expr::LiteralInt(1)]],
        };
        match &stmt {
            Statement::Insert { table, .. } => assert_eq!(table, "t"),
            _ => panic!("Expected Insert"),
        }
    }

    #[test]
    fn test_expr_evaluate() {
        let row = Row::new(vec![Value::Int(10), Value::String("hi".to_string())]);
        let cols = vec!["id".to_string(), "name".to_string()];

        let expr = Expr::BinaryOp(
            Box::new(Expr::Column("id".to_string())),
            Operator::Gt,
            Box::new(Expr::LiteralInt(5)),
        );
        assert_eq!(expr.evaluate(&row, &cols), Some(Value::Bool(true)));
    }
}
