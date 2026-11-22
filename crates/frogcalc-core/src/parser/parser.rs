use crate::error::{CalcError, CalcResult};
use crate::numeric::rat_from_str;

use super::ast::{FunctionBody, Node, Program, Statement};
use super::lexer::{tokenize, Token, TokenKind};

const BP_OR: i32 = 3;
const BP_AND: i32 = 5;
const BP_CMP: i32 = 7;
const BP_ADD: i32 = 10;
const BP_MUL: i32 = 20;
const BP_UNARY: i32 = 30;
const BP_POW: i32 = 40;
const BP_POSTFIX: i32 = 50;

pub fn parse(source: &str) -> CalcResult<Program> {
    let tokens = tokenize(source)?;
    Parser {
        tokens,
        index: 0,
        paren_depth: 0,
    }
    .parse_program()
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
    paren_depth: usize,
}

impl Parser {
    fn parse_program(&mut self) -> CalcResult<Program> {
        let mut statements = Vec::new();
        self.skip_newlines();
        while !self.at_end() {
            let statement = self.parse_statement()?;
            let mut suppress = false;
            if self.at_op(";") {
                while self.at_op(";") {
                    self.index += 1;
                }
                suppress = true;
            }
            statements.push(statement.with_suppress(suppress));
            self.skip_newlines();
        }
        Ok(Program { statements })
    }

    fn parse_statement(&mut self) -> CalcResult<Statement> {
        self.skip_newlines();
        let first = self.current().clone();
        if let TokenKind::Ident(name) = &first.kind {
            match name.as_str() {
                "if" => return self.parse_if(),
                "while" => return self.parse_while(),
                "for" => return self.parse_for(),
                "break" => {
                    let pos = first.pos;
                    self.index += 1;
                    return Ok(Statement::Break { pos });
                }
                "continue" => {
                    let pos = first.pos;
                    self.index += 1;
                    return Ok(Statement::Continue { pos });
                }
                _ => {}
            }
            if let Some(Token {
                kind: TokenKind::Op(op),
                ..
            }) = self.peek(1)
            {
                if op == "=" {
                    self.index += 1;
                    self.index += 1;
                    let value = self.parse_expression(0)?;
                    return Ok(Statement::Assign(name.clone(), value, false));
                }
            }
            if matches!(
                self.peek(1),
                Some(Token {
                    kind: TokenKind::Op(op),
                    ..
                }) if op == "("
            ) {
                let saved_index = self.index;
                let saved_depth = self.paren_depth;
                self.index += 1;
                self.index += 1;
                self.paren_depth += 1;
                let mut params = Vec::new();
                let mut valid = true;
                self.skip_soft_newlines();
                if !self.at_op(")") {
                    loop {
                        self.skip_soft_newlines();
                        match &self.current().kind {
                            TokenKind::Ident(param) => {
                                params.push(param.clone());
                                self.index += 1;
                            }
                            _ => {
                                valid = false;
                                break;
                            }
                        }
                        self.skip_soft_newlines();
                        if self.at_op(",") {
                            self.index += 1;
                            continue;
                        }
                        break;
                    }
                }
                self.skip_soft_newlines();
                if valid && self.at_op(")") {
                    self.index += 1;
                    self.paren_depth = saved_depth;
                    self.skip_soft_newlines();
                    if self.at_op("=") {
                        self.index += 1;
                        self.skip_newlines();
                        let body = if self.at_op("{") && self.brace_starts_block() {
                            FunctionBody::Block(self.parse_block()?)
                        } else {
                            FunctionBody::Expr(self.parse_expression(0)?)
                        };
                        let mut unique = params.clone();
                        unique.sort();
                        unique.dedup();
                        if unique.len() != params.len() {
                            return Err(CalcError::at("函数参数名不能重复", first.pos));
                        }
                        return Ok(Statement::FuncDef(name.clone(), params, body, false));
                    }
                }
                self.paren_depth = saved_depth;
                self.index = saved_index;
            }
        }
        let expr = self.parse_expression(0)?;
        Ok(Statement::Expr(expr, false))
    }

    fn parse_if(&mut self) -> CalcResult<Statement> {
        let pos = self.current().pos;
        self.index += 1;
        let cond = self.parse_expression(0)?;
        let then_block = self.parse_block()?;
        self.skip_newlines();
        let mut else_block = Vec::new();
        if matches!(&self.current().kind, TokenKind::Ident(name) if name == "else") {
            self.index += 1;
            self.skip_newlines();
            if matches!(&self.current().kind, TokenKind::Ident(name) if name == "if") {
                else_block = vec![self.parse_if()?];
            } else {
                else_block = self.parse_block()?;
            }
        }
        Ok(Statement::If {
            cond,
            then_block,
            else_block,
            suppress: false,
            pos,
        })
    }

    fn parse_while(&mut self) -> CalcResult<Statement> {
        let pos = self.current().pos;
        self.index += 1;
        let cond = self.parse_expression(0)?;
        let body = self.parse_block()?;
        Ok(Statement::While {
            cond,
            body,
            suppress: false,
            pos,
        })
    }

    fn parse_for(&mut self) -> CalcResult<Statement> {
        let pos = self.current().pos;
        self.index += 1;
        let var = match &self.current().kind {
            TokenKind::Ident(name) if !is_keyword(name) => name.clone(),
            _ => {
                return Err(CalcError::at(
                    "for 需要一个循环变量名，例如 for i in range(1, 10)",
                    self.current().pos,
                ))
            }
        };
        self.index += 1;
        match &self.current().kind {
            TokenKind::Ident(name) if name == "in" => self.index += 1,
            _ => {
                return Err(CalcError::at(
                    "for 变量后需要 in，例如 for i in range(1, 10)",
                    self.current().pos,
                ))
            }
        }
        let iter = self.parse_expression(0)?;
        let body = self.parse_block()?;
        Ok(Statement::For {
            var,
            iter,
            body,
            suppress: false,
            pos,
        })
    }

    fn brace_starts_block(&self) -> bool {
        let mut brace_depth = 0usize;
        let mut paren_depth = 0usize;
        let mut block_marker = false;
        for token in &self.tokens[self.index..] {
            match &token.kind {
                TokenKind::Op(op) if op == "{" => brace_depth += 1,
                TokenKind::Op(op) if op == "}" => {
                    brace_depth = brace_depth.saturating_sub(1);
                    if brace_depth == 0 {
                        break;
                    }
                }
                TokenKind::Op(op) if op == "(" => paren_depth += 1,
                TokenKind::Op(op) if op == ")" => paren_depth = paren_depth.saturating_sub(1),
                TokenKind::Op(op) if brace_depth == 1 && paren_depth == 0 => {
                    if op == "," {
                        return false;
                    }
                    if op == ";" || op == "=" {
                        block_marker = true;
                    }
                }
                TokenKind::Newline if brace_depth == 1 && paren_depth == 0 => {
                    block_marker = true;
                }
                TokenKind::Ident(name)
                    if brace_depth == 1
                        && paren_depth == 0
                        && matches!(name.as_str(), "if" | "while" | "for") =>
                {
                    block_marker = true;
                }
                _ => {}
            }
        }
        block_marker
    }

    fn parse_block(&mut self) -> CalcResult<Vec<Statement>> {
        self.skip_newlines();
        if !self.at_op("{") {
            return Err(CalcError::at(
                format!("这里需要 “{{”，但遇到 “{}”", token_text(self.current())),
                self.current().pos,
            ));
        }
        self.index += 1;
        let mut statements = Vec::new();
        self.skip_newlines();
        while !self.at_op("}") {
            if self.at_end() {
                return Err(CalcError::at("代码块缺少 }", self.current().pos));
            }
            statements.push(self.parse_statement()?);
            while self.at_op(";") {
                self.index += 1;
            }
            self.skip_newlines();
        }
        self.index += 1;
        Ok(statements)
    }

    fn parse_expression(&mut self, min_bp: i32) -> CalcResult<Node> {
        self.skip_soft_newlines();
        let mut left = self.parse_prefix()?;
        loop {
            self.skip_soft_newlines();
            let token = self.current().clone();
            if let TokenKind::Op(op) = &token.kind {
                if op == "!" && BP_POSTFIX > min_bp {
                    self.index += 1;
                    left = Node::Call("factorial".into(), vec![left], token.pos);
                    continue;
                }
            }
            let starts_primary = matches!(&token.kind, TokenKind::Number(_))
                || matches!(&token.kind, TokenKind::Ident(name) if !is_keyword(name))
                || matches!(&token.kind, TokenKind::Op(op) if op == "(");
            if starts_primary {
                if BP_MUL > min_bp {
                    let right = self.parse_expression(BP_MUL + 1)?;
                    left = Node::Binary('*', Box::new(left), Box::new(right), token.pos);
                    continue;
                }
                break;
            }
            if let TokenKind::Ident(name) = &token.kind {
                let (lbp, rbp, op) = match name.as_str() {
                    "or" => (BP_OR, BP_OR + 1, "or"),
                    "and" => (BP_AND, BP_AND + 1, "and"),
                    _ => (0, 0, ""),
                };
                if lbp > min_bp {
                    self.index += 1;
                    let right = self.parse_expression(rbp)?;
                    left = Node::Call(op.into(), vec![left, right], token.pos);
                    continue;
                }
            }
            let (lbp, rbp, op) = match &token.kind {
                TokenKind::Op(op) if op == "==" => (BP_CMP, BP_CMP + 1, None),
                TokenKind::Op(op) if matches!(op.as_str(), "<" | "<=" | ">" | ">=" | "!=") => {
                    (BP_CMP, BP_CMP + 1, None)
                }
                TokenKind::Op(op) if op == "+" || op == "-" => {
                    (BP_ADD, BP_ADD + 1, Some(op.chars().next().unwrap()))
                }
                TokenKind::Op(op) if op == "*" || op == "/" => {
                    (BP_MUL, BP_MUL + 1, Some(op.chars().next().unwrap()))
                }
                TokenKind::Op(op) if op == "^" || op == "**" => (BP_POW, BP_POW - 1, Some('^')),
                _ => (0, 0, None),
            };
            if lbp <= min_bp {
                break;
            }
            if matches!(&token.kind, TokenKind::Op(op) if op == "==") {
                self.index += 1;
                let right = self.parse_expression(rbp)?;
                left = Node::Eq(Box::new(left), Box::new(right), token.pos);
                continue;
            }
            if let TokenKind::Op(text) = &token.kind {
                let name = match text.as_str() {
                    "<" => Some("lt"),
                    "<=" => Some("le"),
                    ">" => Some("gt"),
                    ">=" => Some("ge"),
                    "!=" => Some("neq"),
                    _ => None,
                };
                if let Some(name) = name {
                    self.index += 1;
                    let right = self.parse_expression(rbp)?;
                    left = Node::Call(name.into(), vec![left, right], token.pos);
                    continue;
                }
            }
            let Some(op) = op else { break };
            self.index += 1;
            let right = self.parse_expression(rbp)?;
            left = Node::Binary(op, Box::new(left), Box::new(right), token.pos);
        }
        Ok(left)
    }

    fn parse_prefix(&mut self) -> CalcResult<Node> {
        self.skip_soft_newlines();
        let token = self.current().clone();
        match token.kind {
            TokenKind::Number(raw) => {
                self.index += 1;
                let value =
                    rat_from_str(&raw).ok_or_else(|| CalcError::at("数字格式错误", token.pos))?;
                Ok(Node::Number(value, token.pos))
            }
            TokenKind::String(value) => {
                self.index += 1;
                Ok(Node::String(value, token.pos))
            }
            TokenKind::Ident(name) if name == "not" => {
                self.index += 1;
                let arg = self.parse_expression(BP_AND)?;
                Ok(Node::Call("not".into(), vec![arg], token.pos))
            }
            TokenKind::Ident(name) => {
                if is_keyword(&name) {
                    return Err(CalcError::at(
                        format!("关键字 “{name}” 不能作为表达式使用"),
                        token.pos,
                    ));
                }
                self.index += 1;
                if self.at_op("(") {
                    self.index += 1;
                    self.paren_depth += 1;
                    let mut args = Vec::new();
                    self.skip_soft_newlines();
                    if !self.at_op(")") {
                        loop {
                            args.push(self.parse_expression(0)?);
                            self.skip_soft_newlines();
                            if self.at_op(",") {
                                self.index += 1;
                                continue;
                            }
                            break;
                        }
                    }
                    self.skip_soft_newlines();
                    self.expect_op(")")?;
                    self.paren_depth -= 1;
                    return Ok(Node::Call(name, args, token.pos));
                }
                Ok(Node::Ident(name, token.pos))
            }
            TokenKind::Op(op) if op == "+" || op == "-" => {
                self.index += 1;
                let arg = self.parse_expression(BP_UNARY)?;
                Ok(Node::Unary(
                    op.chars().next().unwrap(),
                    Box::new(arg),
                    token.pos,
                ))
            }
            TokenKind::Op(op) if op == "(" => {
                self.index += 1;
                self.paren_depth += 1;
                let expr = self.parse_expression(0)?;
                self.skip_soft_newlines();
                self.expect_op(")")?;
                self.paren_depth -= 1;
                Ok(expr)
            }
            TokenKind::Op(op) if op == "{" => {
                self.index += 1;
                self.paren_depth += 1;
                let mut items = Vec::new();
                self.skip_soft_newlines();
                if !self.at_op("}") {
                    loop {
                        items.push(self.parse_expression(0)?);
                        self.skip_soft_newlines();
                        if self.at_op(",") {
                            self.index += 1;
                            continue;
                        }
                        break;
                    }
                }
                self.skip_soft_newlines();
                self.expect_op("}")?;
                self.paren_depth -= 1;
                Ok(Node::List(items, token.pos))
            }
            _ => Err(CalcError::at(
                format!(
                    "这里需要一个数字、变量或左括号，但遇到 “{}”",
                    token_text(&token)
                ),
                token.pos,
            )),
        }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.index.min(self.tokens.len() - 1)]
    }

    fn peek(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.index + offset)
    }

    fn at_end(&self) -> bool {
        matches!(self.current().kind, TokenKind::Eof)
    }

    fn at_op(&self, value: &str) -> bool {
        matches!(&self.current().kind, TokenKind::Op(op) if op == value)
    }

    fn expect_op(&mut self, value: &str) -> CalcResult<()> {
        if self.at_op(value) {
            self.index += 1;
            Ok(())
        } else {
            Err(CalcError::at(
                format!(
                    "这里需要 “{value}”，但遇到 “{}”",
                    token_text(self.current())
                ),
                self.current().pos,
            ))
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.current().kind, TokenKind::Newline) {
            self.index += 1;
        }
    }

    fn skip_soft_newlines(&mut self) {
        if self.paren_depth > 0 {
            while matches!(self.current().kind, TokenKind::Newline) {
                self.index += 1;
            }
        }
    }
}

fn is_keyword(name: &str) -> bool {
    matches!(
        name,
        "if" | "else" | "while" | "for" | "in" | "and" | "or" | "not" | "break" | "continue"
    )
}

fn token_text(token: &Token) -> String {
    match &token.kind {
        TokenKind::Number(v) | TokenKind::Ident(v) | TokenKind::String(v) | TokenKind::Op(v) => {
            v.clone()
        }
        TokenKind::Newline => "换行".into(),
        TokenKind::Eof => "结尾".into(),
    }
}
