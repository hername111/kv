use crate::ast::*;
use crate::lexer::Token;
use kv_common::error::{KvError, KvResult};
use kv_common::types::{ColumnDef, DataType};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }
    fn advance(&mut self) -> Option<&Token> {
        let t = self.tokens.get(self.pos);
        self.pos += 1;
        t
    }
    /// 解析可能带表前缀的列名，如 "table.col" → 返回 "col"
    fn parse_on_column(&mut self) -> KvResult<String> {
        let first = self.expect_ident()?;
        if self.peek() == Some(&Token::Dot) {
            self.advance();
            Ok(self.expect_ident()?)
        } else {
            Ok(first)
        }
    }

    fn expect_ident(&mut self) -> KvResult<String> {
        let pos = self.pos;
        match self.advance() {
            Some(Token::Ident(s)) => Ok(s.clone()),
            Some(t) => Err(KvError::ParseError {
                pos,
                message: format!("expected identifier, got: {:?}", t),
            }),
            None => Err(KvError::ParseError {
                pos,
                message: "unexpected end".to_string(),
            }),
        }
    }
    fn match_kw(&mut self, word: &str) -> bool {
        match self.peek() {
            Some(t) if token_matches_keyword(t, word) => {
                self.advance();
                true
            }
            _ => false,
        }
    }
    fn expect(&mut self, t: Token) -> KvResult<()> {
        let pos = self.pos;
        match self.advance() {
            Some(tok) if std::mem::discriminant(tok) == std::mem::discriminant(&t) => Ok(()),
            Some(tok) => Err(KvError::ParseError {
                pos,
                message: format!("expected {:?}, got {:?}", t, tok),
            }),
            None => Err(KvError::ParseError {
                pos,
                message: "unexpected end".to_string(),
            }),
        }
    }

    pub fn parse_statement(&mut self) -> KvResult<Statement> {
        match self.peek() {
            Some(Token::Select) => self.parse_select(),
            Some(Token::Insert) => self.parse_insert(),
            Some(Token::Update) => self.parse_update(),
            Some(Token::Delete) => self.parse_delete(),
            Some(Token::Create) => self.parse_create(),
            Some(Token::Drop) => self.parse_drop(),
            Some(Token::Begin) => {
                self.advance();
                Ok(Statement::Begin)
            }
            Some(Token::Commit) => {
                self.advance();
                Ok(Statement::Commit)
            }
            Some(Token::Rollback) => {
                self.advance();
                Ok(Statement::Rollback)
            }
            Some(t) => Err(KvError::ParseError {
                pos: self.pos,
                message: format!("unexpected token: {:?}", t),
            }),
            None => Err(KvError::ParseError {
                pos: self.pos,
                message: "empty statement".to_string(),
            }),
        }
    }

    fn parse_select(&mut self) -> KvResult<Statement> {
        self.advance();
        let columns = self.parse_select_items()?;
        self.match_kw("FROM");
        let from = self.expect_ident()?;
        let mut where_clause = None;
        if self.match_kw("WHERE") {
            where_clause = Some(self.parse_expr()?);
        }
        let mut order_by = Vec::new();
        if self.match_kw("ORDER") {
            self.match_kw("BY");
            order_by = self.parse_order_by()?;
        }
        let mut join = None;
        if self.match_kw("JOIN") {
            let table = self.expect_ident()?;
            self.match_kw("ON");
            let left = self.parse_on_column()?;
            self.expect(Token::Equal)?;
            let right = self.parse_on_column()?;
            join = Some(Join {
                table,
                on: (left, right),
            });
        }
        if self.peek() == Some(&Token::Semicolon) {
            self.advance();
        }
        Ok(Statement::Select {
            columns,
            from,
            where_clause,
            order_by,
            join,
        })
    }

    fn parse_select_items(&mut self) -> KvResult<Vec<SelectItem>> {
        let mut items = Vec::new();
        loop {
            match self.peek() {
                Some(Token::Star) => {
                    items.push(SelectItem::Star);
                    self.advance();
                }
                Some(Token::Ident(_)) => {
                    let name = self.expect_ident()?;
                    if self.match_kw("AS") {
                        let alias = self.expect_ident()?;
                        items.push(SelectItem::Alias(name, alias));
                    } else {
                        items.push(SelectItem::Column(name));
                    }
                }
                _ => break,
            }
            if self.peek() == Some(&Token::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        if items.is_empty() {
            return Err(KvError::ParseError {
                pos: self.pos,
                message: "expected columns after SELECT".to_string(),
            });
        }
        Ok(items)
    }

    fn parse_insert(&mut self) -> KvResult<Statement> {
        self.advance();
        self.match_kw("INTO");
        let table = self.expect_ident()?;
        let columns = if self.peek() == Some(&Token::LeftParen) {
            self.advance();
            let mut cols = Vec::new();
            loop {
                cols.push(self.expect_ident()?);
                if self.peek() == Some(&Token::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(Token::RightParen)?;
            Some(cols)
        } else {
            None
        };
        self.match_kw("VALUES");
        let values = self.parse_value_lists()?;
        if self.peek() == Some(&Token::Semicolon) {
            self.advance();
        }
        Ok(Statement::Insert {
            table,
            columns,
            values,
        })
    }

    fn parse_value_lists(&mut self) -> KvResult<Vec<Vec<Expr>>> {
        let mut lists = Vec::new();
        loop {
            self.expect(Token::LeftParen)?;
            let mut vals = Vec::new();
            loop {
                vals.push(self.parse_expr()?);
                if self.peek() == Some(&Token::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(Token::RightParen)?;
            lists.push(vals);
            if self.peek() == Some(&Token::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        Ok(lists)
    }

    fn parse_update(&mut self) -> KvResult<Statement> {
        self.advance();
        let table = self.expect_ident()?;
        self.match_kw("SET");
        let mut set = Vec::new();
        loop {
            let col = self.expect_ident()?;
            self.expect(Token::Equal)?;
            let val = self.parse_expr()?;
            set.push((col, val));
            if self.peek() == Some(&Token::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        let mut where_clause = None;
        if self.match_kw("WHERE") {
            where_clause = Some(self.parse_expr()?);
        }
        if self.peek() == Some(&Token::Semicolon) {
            self.advance();
        }
        Ok(Statement::Update {
            table,
            set,
            where_clause,
        })
    }

    fn parse_delete(&mut self) -> KvResult<Statement> {
        self.advance();
        self.match_kw("FROM");
        let table = self.expect_ident()?;
        let mut where_clause = None;
        if self.match_kw("WHERE") {
            where_clause = Some(self.parse_expr()?);
        }
        if self.peek() == Some(&Token::Semicolon) {
            self.advance();
        }
        Ok(Statement::Delete {
            table,
            where_clause,
        })
    }

    fn parse_create(&mut self) -> KvResult<Statement> {
        self.advance();
        if self.match_kw("TABLE") {
            let name = self.expect_ident()?;
            self.expect(Token::LeftParen)?;
            let mut columns = Vec::new();
            let mut primary_key = String::new();
            let mut col_id = 1u64;
            loop {
                let col_name = self.expect_ident()?;
                let type_name = self.expect_ident()?;
                let data_type = parse_data_type(&type_name, self)?;
                let mut nullable = true;
                let mut is_pk = false;
                if self.match_kw("PRIMARY") {
                    self.match_kw("KEY");
                    nullable = false;
                    is_pk = true;
                    primary_key = col_name.clone();
                } else if self.match_kw("NOT") {
                    self.match_kw("NULL");
                    nullable = false;
                }
                columns.push(ColumnDef {
                    id: col_id,
                    name: col_name,
                    data_type,
                    nullable,
                    is_primary_key: is_pk,
                });
                col_id += 1;
                if self.peek() == Some(&Token::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(Token::RightParen)?;
            if self.peek() == Some(&Token::Semicolon) {
                self.advance();
            }
            Ok(Statement::CreateTable {
                name,
                columns,
                primary_key,
            })
        } else if self.match_kw("INDEX") {
            let name = self.expect_ident()?;
            self.match_kw("ON");
            let table = self.expect_ident()?;
            self.expect(Token::LeftParen)?;
            let column = self.expect_ident()?;
            self.expect(Token::RightParen)?;
            if self.peek() == Some(&Token::Semicolon) {
                self.advance();
            }
            Ok(Statement::CreateIndex {
                name,
                table,
                column,
            })
        } else {
            Err(KvError::ParseError {
                pos: self.pos,
                message: "CREATE requires TABLE or INDEX".to_string(),
            })
        }
    }

    fn parse_drop(&mut self) -> KvResult<Statement> {
        self.advance();
        self.match_kw("TABLE");
        let name = self.expect_ident()?;
        if self.peek() == Some(&Token::Semicolon) {
            self.advance();
        }
        Ok(Statement::DropTable { name })
    }

    fn parse_expr(&mut self) -> KvResult<Expr> {
        self.parse_logical()
    }

    /// logical: comparison (AND/OR comparison)*
    fn parse_logical(&mut self) -> KvResult<Expr> {
        let mut left = self.parse_comparison()?;
        while let Some(tok) = self.peek() {
            let op = match tok {
                Token::And => {
                    self.advance();
                    Operator::And
                }
                Token::Or => {
                    self.advance();
                    Operator::Or
                }
                _ => break,
            };
            let right = self.parse_comparison()?;
            left = Expr::BinaryOp(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    /// comparison: primary ((= | <> | != | > | < | >= | <=) primary)?
    fn parse_comparison(&mut self) -> KvResult<Expr> {
        let left = self.parse_primary()?;
        match self.peek() {
            Some(Token::Equal)
            | Some(Token::NotEqual)
            | Some(Token::Gt)
            | Some(Token::Lt)
            | Some(Token::Gte)
            | Some(Token::Lte) => {
                let op = match self.advance() {
                    Some(Token::Equal) => Operator::Eq,
                    Some(Token::NotEqual) => Operator::Neq,
                    Some(Token::Gt) => Operator::Gt,
                    Some(Token::Lt) => Operator::Lt,
                    Some(Token::Gte) => Operator::Gte,
                    Some(Token::Lte) => Operator::Lte,
                    _ => unreachable!(),
                };
                let right = self.parse_primary()?;
                Ok(Expr::BinaryOp(Box::new(left), op, Box::new(right)))
            }
            _ => Ok(left),
        }
    }

    fn parse_primary(&mut self) -> KvResult<Expr> {
        let pos = self.pos;
        match self.advance() {
            Some(Token::Ident(s)) => Ok(Expr::Column(s.clone())),
            Some(Token::LiteralInt(i)) => Ok(Expr::LiteralInt(*i)),
            Some(Token::LiteralFloat(f)) => Ok(Expr::LiteralFloat(*f)),
            Some(Token::LiteralString(s)) => Ok(Expr::LiteralString(s.clone())),
            Some(Token::Null) => Ok(Expr::LiteralNull),
            Some(Token::LeftParen) => {
                let expr = self.parse_expr()?;
                self.expect(Token::RightParen)?;
                Ok(expr)
            }
            Some(t) => Err(KvError::ParseError {
                pos,
                message: format!("unexpected token in expr: {:?}", t),
            }),
            None => Err(KvError::ParseError {
                pos,
                message: "incomplete expression".to_string(),
            }),
        }
    }

    fn parse_order_by(&mut self) -> KvResult<Vec<OrderBy>> {
        let mut items = Vec::new();
        loop {
            let column = self.expect_ident()?;
            let ascending = if self.match_kw("DESC") {
                false
            } else {
                self.match_kw("ASC");
                true
            };
            items.push(OrderBy { column, ascending });
            if self.peek() == Some(&Token::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        Ok(items)
    }
}

fn token_matches_keyword(tok: &Token, word: &str) -> bool {
    let w = word.to_uppercase();
    match tok {
        Token::Ident(s) => s.eq_ignore_ascii_case(word),
        Token::Select => w == "SELECT",
        Token::Insert => w == "INSERT",
        Token::Into => w == "INTO",
        Token::Values => w == "VALUES",
        Token::Update => w == "UPDATE",
        Token::Set => w == "SET",
        Token::Delete => w == "DELETE",
        Token::From => w == "FROM",
        Token::Where => w == "WHERE",
        Token::Join => w == "JOIN",
        Token::On => w == "ON",
        Token::Order => w == "ORDER",
        Token::By => w == "BY",
        Token::Asc => w == "ASC",
        Token::Desc => w == "DESC",
        Token::Create => w == "CREATE",
        Token::Table => w == "TABLE",
        Token::Index => w == "INDEX",
        Token::Drop => w == "DROP",
        Token::Primary => w == "PRIMARY",
        Token::Key => w == "KEY",
        Token::And => w == "AND",
        Token::Or => w == "OR",
        Token::Not => w == "NOT",
        Token::Null => w == "NULL",
        Token::Begin => w == "BEGIN",
        Token::Commit => w == "COMMIT",
        Token::Rollback => w == "ROLLBACK",
        _ => false,
    }
}

fn parse_data_type(type_name: &str, parser: &mut Parser) -> KvResult<DataType> {
    match type_name.to_uppercase().as_str() {
        "INT" | "INTEGER" => Ok(DataType::Int),
        "BIGINT" => Ok(DataType::BigInt),
        "FLOAT" => Ok(DataType::Float),
        "DOUBLE" => Ok(DataType::Double),
        "VARCHAR" => {
            if parser.peek() == Some(&Token::LeftParen) {
                parser.advance();
                let len = match parser.advance() {
                    Some(Token::LiteralInt(n)) => *n as u16,
                    _ => {
                        return Err(KvError::ParseError {
                            pos: parser.pos,
                            message: "VARCHAR needs length".to_string(),
                        });
                    }
                };
                parser.expect(Token::RightParen)?;
                Ok(DataType::VarChar(len))
            } else {
                Ok(DataType::VarChar(255))
            }
        }
        "TEXT" => Ok(DataType::Text),
        "BOOL" | "BOOLEAN" => Ok(DataType::Bool),
        "DATE" => Ok(DataType::Date),
        "TIMESTAMP" => Ok(DataType::Timestamp),
        _ => Err(KvError::ParseError {
            pos: parser.pos,
            message: format!("unknown type: {}", type_name),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    fn parse(sql: &str) -> KvResult<Statement> {
        let mut lexer = Lexer::new(sql);
        let tokens = lexer.tokenize()?;
        Parser::new(tokens).parse_statement()
    }
    #[test]
    fn test_select() {
        let s = parse("SELECT * FROM t;").unwrap();
        match s {
            Statement::Select { from, .. } => assert_eq!(from, "t"),
            _ => panic!(),
        }
    }
    #[test]
    fn test_select_where() {
        let s = parse("SELECT id FROM t WHERE id>5").unwrap();
        match s {
            Statement::Select { where_clause, .. } => assert!(where_clause.is_some()),
            _ => panic!(),
        }
    }
    #[test]
    fn test_insert() {
        let s = parse("INSERT INTO t VALUES (1,'hi')").unwrap();
        match s {
            Statement::Insert { table, .. } => assert_eq!(table, "t"),
            _ => panic!(),
        }
    }
    #[test]
    fn test_update() {
        let s = parse("UPDATE t SET n='x' WHERE id=1").unwrap();
        match s {
            Statement::Update { table, .. } => assert_eq!(table, "t"),
            _ => panic!(),
        }
    }
    #[test]
    fn test_delete() {
        let s = parse("DELETE FROM t WHERE id=1").unwrap();
        match s {
            Statement::Delete { table, .. } => assert_eq!(table, "t"),
            _ => panic!(),
        }
    }
    #[test]
    fn test_create_table() {
        let s = parse("CREATE TABLE t (id INT PRIMARY KEY, name VARCHAR(100))").unwrap();
        match s {
            Statement::CreateTable { name, columns, .. } => {
                assert_eq!(name, "t");
                assert_eq!(columns.len(), 2);
            }
            _ => panic!(),
        }
    }
    #[test]
    fn test_drop() {
        let s = parse("DROP TABLE t;").unwrap();
        match s {
            Statement::DropTable { name } => assert_eq!(name, "t"),
            _ => panic!(),
        }
    }
}
