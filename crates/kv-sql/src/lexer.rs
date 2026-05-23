// 词法分析：SQL 文本 → Token 流
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Select,
    Insert,
    Update,
    Delete,
    From,
    Where,
    Join,
    OrderBy,
    Ident(String),
    Star,
    Comma,
    Semicolon,
    LiteralInt(i64),
    LiteralString(String),
    Equal,
    Unknown(String),
}

pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        let words = self.input.split_whitespace();
        for w in words {
            // separate trailing punctuation like ',' or ';'
            let mut core = w.to_string();
            let mut trailing = Vec::new();
            while core.ends_with(',') || core.ends_with(';') {
                if core.ends_with(',') { trailing.push(','); core.pop(); }
                else if core.ends_with(';') { trailing.push(';'); core.pop(); }
            }

            let token = match core.to_uppercase().as_str() {
                "SELECT" => Token::Select,
                "INSERT" => Token::Insert,
                "UPDATE" => Token::Update,
                "DELETE" => Token::Delete,
                "FROM" => Token::From,
                "WHERE" => Token::Where,
                "JOIN" => Token::Join,
                "ORDER" => Token::OrderBy,
                "*" => Token::Star,
                "" => continue,
                _ => Token::Ident(core.clone()),
            };
            tokens.push(token);
            // emit trailing punctuation tokens in order they appeared
            for p in trailing.into_iter().rev() {
                match p {
                    ',' => tokens.push(Token::Comma),
                    ';' => tokens.push(Token::Semicolon),
                    _ => {}
                }
            }
        }
        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_select() {
        let mut lexer = Lexer::new("SELECT * FROM t;");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::Select);
        assert_eq!(tokens[1], Token::Star);
    }
}