//! 将 AST 转换为可递归执行的逻辑计划树。

use crate::ast::*;
use kv_common::error::KvResult;
use kv_common::types::ColumnDef;

/// 可执行的逻辑计划节点。
///
/// 计划树保持递归结构：扫描、过滤、排序、连接和投影可以按节点组合执行。
#[derive(Debug, Clone)]
pub enum PlanNode {
    SeqScan {
        table: String,
        columns: Vec<SelectItem>,
        filter: Option<Expr>,
    },
    IndexScan {
        table: String,
        index: String,
        key: String,
    },
    Insert {
        table: String,
        columns: Vec<String>,
        rows: Vec<Vec<Expr>>,
    },
    Update {
        table: String,
        sets: Vec<(String, Expr)>,
        filter: Option<Expr>,
    },
    Delete {
        table: String,
        filter: Option<Expr>,
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
    BeginTransaction,
    CommitTransaction,
    RollbackTransaction,
    Projection {
        source: Box<PlanNode>,
        columns: Vec<SelectItem>,
    },
    Filter {
        source: Box<PlanNode>,
        predicate: Expr,
    },
    Sort {
        source: Box<PlanNode>,
        order_by: Vec<OrderBy>,
    },
    Join {
        left: Box<PlanNode>,
        right: Box<PlanNode>,
        join: Join,
    },
}

/// 无状态的逻辑计划生成器。
pub struct Planner;

impl Planner {
    /// 将 AST 转换为逻辑计划。
    pub fn plan(stmt: Statement) -> KvResult<PlanNode> {
        match stmt {
            Statement::Select {
                columns,
                from,
                where_clause,
                order_by,
                join,
            } => {
                let mut plan = PlanNode::SeqScan {
                    table: from,
                    columns: columns.clone(),
                    filter: where_clause.clone(),
                };
                if let Some(pred) = where_clause {
                    plan = PlanNode::Filter {
                        source: Box::new(plan),
                        predicate: pred,
                    };
                }
                if let Some(j) = join {
                    let right = PlanNode::SeqScan {
                        table: j.table.clone(),
                        columns: vec![SelectItem::Star],
                        filter: None,
                    };
                    plan = PlanNode::Join {
                        left: Box::new(plan),
                        right: Box::new(right),
                        join: j,
                    };
                }
                if !order_by.is_empty() {
                    plan = PlanNode::Sort {
                        source: Box::new(plan),
                        order_by,
                    };
                }
                plan = PlanNode::Projection {
                    source: Box::new(plan),
                    columns,
                };
                Ok(plan)
            }
            Statement::Insert {
                table,
                columns,
                values,
            } => Ok(PlanNode::Insert {
                table,
                columns: columns.unwrap_or_default(),
                rows: values,
            }),
            Statement::Update {
                table,
                set,
                where_clause,
            } => Ok(PlanNode::Update {
                table,
                sets: set,
                filter: where_clause,
            }),
            Statement::Delete {
                table,
                where_clause,
            } => Ok(PlanNode::Delete {
                table,
                filter: where_clause,
            }),
            Statement::CreateTable {
                name,
                columns,
                primary_key,
            } => Ok(PlanNode::CreateTable {
                name,
                columns,
                primary_key,
            }),
            Statement::CreateIndex {
                name,
                table,
                column,
            } => Ok(PlanNode::CreateIndex {
                name,
                table,
                column,
            }),
            Statement::DropTable { name } => Ok(PlanNode::DropTable { name }),
            Statement::Begin => Ok(PlanNode::BeginTransaction),
            Statement::Commit => Ok(PlanNode::CommitTransaction),
            Statement::Rollback => Ok(PlanNode::RollbackTransaction),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    #[test]
    fn test_plan_select() {
        let sql = "SELECT * FROM t WHERE id > 5";
        let mut lexer = Lexer::new(sql);
        let tokens = lexer.tokenize().unwrap();
        let stmt = Parser::new(tokens).parse_statement().unwrap();
        let plan = Planner::plan(stmt).unwrap();
        match plan {
            PlanNode::Projection { .. } => {}
            _ => panic!("Expected Projection"),
        }
    }

    #[test]
    fn test_plan_insert() {
        let sql = "INSERT INTO t VALUES (1, 'x')";
        let mut lexer = Lexer::new(sql);
        let tokens = lexer.tokenize().unwrap();
        let stmt = Parser::new(tokens).parse_statement().unwrap();
        let plan = Planner::plan(stmt).unwrap();
        match plan {
            PlanNode::Insert { table, .. } => assert_eq!(table, "t"),
            _ => panic!("Expected Insert"),
        }
    }
}
