// 词法分析器：SQL 文本 → Token 流
use kv_common::error::{KvError, KvResult};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Select,
    Insert,
    Into,
    Values,
    Update,
    Set,
    Delete,
    From,
    Where,
    Join,
    On,
    Order,
    By,
    Asc,
    Desc,
    Create,
    Table,
    Index,
    Drop,
    Primary,
    Key,
    And,
    Or,
    Not,
    Null,
    Begin,
    Commit,
    Rollback,
    Ident(String),
    Star,
    Comma,
    Semicolon,
    LeftParen,
    RightParen,
    Dot,
    Equal,
    NotEqual,
    Gt,
    Lt,
    Gte,
    Lte,
    LiteralInt(i64),
    LiteralFloat(f64),
    LiteralString(String),
    Unknown(String),
}

pub struct Lexer<'a> {
    input: &'a str,
    chars: Vec<char>,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, chars: input.chars().collect(), pos: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        self.pos += 1;
        c
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() { self.advance(); } else { break; }
        }
    }

    fn read_word(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        s
    }

    fn read_number(&mut self) -> Token {
        let mut s = String::new();
        let mut is_float = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.advance();
            } else if c == '.' {
                is_float = true;
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        if is_float {
            Token::LiteralFloat(s.parse().unwrap_or(0.0))
        } else {
            Token::LiteralInt(s.parse().unwrap_or(0))
        }
    }

    fn read_string(&mut self) -> Result<Token, KvError> {
        self.advance(); // skip opening quote
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == '\'' {
                self.advance(); // skip closing quote
                return Ok(Token::LiteralString(s));
            }
            s.push(c);
            self.advance();
        }
        Err(KvError::ParseError { pos: self.pos, message: "字符串未闭合".to_string() })
    }

    fn keyword_or_ident(word: &str) -> Token {
        match word.to_uppercase().as_str() {
            "SELECT" => Token::Select,
            "INSERT" => Token::Insert,
            "INTO" => Token::Into,
            "VALUES" => Token::Values,
            "UPDATE" => Token::Update,
            "SET" => Token::Set,
            "DELETE" => Token::Delete,
            "FROM" => Token::From,
            "WHERE" => Token::Where,
            "JOIN" => Token::Join,
            "ON" => Token::On,
            "ORDER" => Token::Order,
            "BY" => Token::By,
            "ASC" => Token::Asc,
            "DESC" => Token::Desc,
            "CREATE" => Token::Create,
            "TABLE" => Token::Table,
            "INDEX" => Token::Index,
            "DROP" => Token::Drop,
            "PRIMARY" => Token::Primary,
            "KEY" => Token::Key,
            "AND" => Token::And,
            "OR" => Token::Or,
            "NOT" => Token::Not,
            "NULL" => Token::Null,
            "BEGIN" => Token::Begin,
            "COMMIT" => Token::Commit,
            "ROLLBACK" => Token::Rollback,
            _ => Token::Ident(word.to_string()),
        }
    }

    pub fn tokenize(&mut self) -> KvResult<Vec<Token>> {
        let mut tokens = Vec::new();
        while self.pos < self.chars.len() {
            self.skip_whitespace();
            if self.pos >= self.chars.len() { break; }

            let c = self.peek().unwrap();
            match c {
                '\'' => tokens.push(self.read_string()?),
                ',' => { tokens.push(Token::Comma); self.advance(); }
                ';' => { tokens.push(Token::Semicolon); self.advance(); }
                '(' => { tokens.push(Token::LeftParen); self.advance(); }
                ')' => { tokens.push(Token::RightParen); self.advance(); }
                '.' => { tokens.push(Token::Dot); self.advance(); }
                '*' => { tokens.push(Token::Star); self.advance(); }
                '=' => { tokens.push(Token::Equal); self.advance(); }
                '<' => {
                    self.advance();
                    match self.peek() {
                        Some('>') => { tokens.push(Token::NotEqual); self.advance(); }
                        Some('=') => { tokens.push(Token::Lte); self.advance(); }
                        _ => tokens.push(Token::Lt),
                    }
                }
                '>' => {
                    self.advance();
                    if self.peek() == Some('=') { tokens.push(Token::Gte); self.advance(); }
                    else { tokens.push(Token::Gt); }
                }
                '!' => {
                    self.advance();
                    if self.peek() == Some('=') { tokens.push(Token::NotEqual); self.advance(); }
                    else { return Err(KvError::ParseError { pos: self.pos, message: "! 之后期望 =".to_string() }); }
                }
                _ if c.is_ascii_digit() => tokens.push(self.read_number()),
                _ if c.is_alphanumeric() || c == '_' => {
                    let word = self.read_word();
                    tokens.push(Self::keyword_or_ident(&word));
                }
                _ => { self.advance(); } // 跳过未知字符
            }
        }
        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_star() {
        let mut lexer = Lexer::new("SELECT * FROM t;");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::Select);
        assert_eq!(tokens[1], Token::Star);
        assert_eq!(tokens[2], Token::From);
        assert_eq!(tokens[3], Token::Ident("t".to_string()));
    }

    #[test]
    fn test_insert() {
        let mut lexer = Lexer::new("INSERT INTO t VALUES (1, 'hi')");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::Insert);
        assert_eq!(tokens[4], Token::LeftParen);
        assert_eq!(tokens[5], Token::LiteralInt(1));
        assert_eq!(tokens[7], Token::LiteralString("hi".to_string()));
    }

    #[test]
    fn test_where_clause() {
        let mut lexer = Lexer::new("SELECT * FROM t WHERE id > 5 AND name <> 'x'");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[4], Token::Where);
        assert_eq!(tokens[5], Token::Ident("id".to_string()));
        assert_eq!(tokens[6], Token::Gt);
        assert_eq!(tokens[8], Token::And);
    }

    #[test]
    fn test_create_table() {
        let mut lexer = Lexer::new("CREATE TABLE t (id INT PRIMARY KEY, name VARCHAR(100))");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::Create);
        assert_eq!(tokens[1], Token::Table);
        assert_eq!(tokens[2], Token::Ident("t".to_string()));
        assert_eq!(tokens[3], Token::LeftParen);
        assert_eq!(tokens[4], Token::Ident("id".to_string()));
        assert_eq!(tokens[5], Token::Ident("INT".to_string()));
        assert_eq!(tokens[6], Token::Primary);
        assert_eq!(tokens[7], Token::Key);
    }
}
