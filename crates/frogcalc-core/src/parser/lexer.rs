use crate::error::{CalcError, CalcResult};

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Number(String),
    Ident(String),
    String(String),
    Op(String),
    Newline,
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub pos: usize,
}

pub fn tokenize(source: &str) -> CalcResult<Vec<Token>> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    let mut line = 1usize;
    let mut col = 1usize;

    while i < chars.len() {
        let ch = chars[i];
        match ch {
            ' ' | '\t' | '\r' => {
                i += 1;
                col += 1;
            }
            '\n' => {
                tokens.push(Token {
                    kind: TokenKind::Newline,
                    pos: i,
                });
                i += 1;
                line += 1;
                col = 1;
            }
            '#' => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                    col += 1;
                }
            }
            '"' => {
                let start = i;
                i += 1;
                col += 1;
                let mut value = String::new();
                let mut closed = false;
                while i < chars.len() {
                    let c = chars[i];
                    i += 1;
                    col += 1;
                    if c == '"' {
                        closed = true;
                        break;
                    }
                    if c == '\\' {
                        if i >= chars.len() {
                            break;
                        }
                        let esc = chars[i];
                        i += 1;
                        col += 1;
                        match esc {
                            'n' => value.push('\n'),
                            't' => value.push('\t'),
                            '"' => value.push('"'),
                            '\\' => value.push('\\'),
                            other => value.push(other),
                        }
                    } else {
                        value.push(c);
                    }
                }
                if !closed {
                    return Err(CalcError::at(
                        format!("字符串未闭合（第 {line} 行第 {col} 列）"),
                        start,
                    ));
                }
                tokens.push(Token {
                    kind: TokenKind::String(value),
                    pos: start,
                });
            }
            c if c.is_ascii_digit()
                || (c == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) =>
            {
                let start = i;
                let mut raw = String::new();
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '_') {
                    raw.push(chars[i]);
                    i += 1;
                    col += 1;
                }
                if i < chars.len() && chars[i] == '.' {
                    raw.push('.');
                    i += 1;
                    col += 1;
                    while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '_') {
                        raw.push(chars[i]);
                        i += 1;
                        col += 1;
                    }
                }
                if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                    let mut j = i + 1;
                    if j < chars.len() && (chars[j] == '+' || chars[j] == '-') {
                        j += 1;
                    }
                    if j < chars.len() && chars[j].is_ascii_digit() {
                        raw.push(chars[i]);
                        i += 1;
                        col += 1;
                        if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
                            raw.push(chars[i]);
                            i += 1;
                            col += 1;
                        }
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            raw.push(chars[i]);
                            i += 1;
                            col += 1;
                        }
                    }
                }
                tokens.push(Token {
                    kind: TokenKind::Number(raw.replace('_', "")),
                    pos: start,
                });
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                let mut name = String::new();
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    name.push(chars[i]);
                    i += 1;
                    col += 1;
                }
                tokens.push(Token {
                    kind: TokenKind::Ident(name),
                    pos: start,
                });
            }
            '=' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                tokens.push(Token {
                    kind: TokenKind::Op("==".into()),
                    pos: i,
                });
                i += 2;
                col += 2;
            }
            '<' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                tokens.push(Token {
                    kind: TokenKind::Op("<=".into()),
                    pos: i,
                });
                i += 2;
                col += 2;
            }
            '>' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                tokens.push(Token {
                    kind: TokenKind::Op(">=".into()),
                    pos: i,
                });
                i += 2;
                col += 2;
            }
            '!' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                tokens.push(Token {
                    kind: TokenKind::Op("!=".into()),
                    pos: i,
                });
                i += 2;
                col += 2;
            }
            '*' if i + 1 < chars.len() && chars[i + 1] == '*' => {
                tokens.push(Token {
                    kind: TokenKind::Op("**".into()),
                    pos: i,
                });
                i += 2;
                col += 2;
            }
            '+' | '-' | '*' | '/' | '^' | '=' | '(' | ')' | ',' | ';' | '!' | '{' | '}' | '<'
            | '>' => {
                tokens.push(Token {
                    kind: TokenKind::Op(ch.to_string()),
                    pos: i,
                });
                i += 1;
                col += 1;
            }
            other => {
                return Err(CalcError::at(
                    format!("无法识别的字符 “{other}”（第 {line} 行第 {col} 列）"),
                    i,
                ));
            }
        }
    }
    tokens.push(Token {
        kind: TokenKind::Eof,
        pos: chars.len(),
    });
    Ok(tokens)
}
