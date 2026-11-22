use crate::numeric::Rat;

#[derive(Clone, Debug)]
pub enum Node {
    Number(Rat, usize),
    String(String, usize),
    Ident(String, usize),
    Unary(char, Box<Node>, usize),
    Binary(char, Box<Node>, Box<Node>, usize),
    Call(String, Vec<Node>, usize),
    Eq(Box<Node>, Box<Node>, usize),
    List(Vec<Node>, usize),
}

impl Node {
    pub fn pos(&self) -> usize {
        match self {
            Node::Number(_, p)
            | Node::String(_, p)
            | Node::Ident(_, p)
            | Node::Unary(_, _, p)
            | Node::Binary(_, _, _, p)
            | Node::Call(_, _, p)
            | Node::Eq(_, _, p)
            | Node::List(_, p) => *p,
        }
    }
}

#[derive(Clone, Debug)]
pub enum FunctionBody {
    Expr(Node),
    Block(Vec<Statement>),
}

#[derive(Clone, Debug)]
pub enum Statement {
    Expr(Node, bool),
    Assign(String, Node, bool),
    FuncDef(String, Vec<String>, FunctionBody, bool),
    If {
        cond: Node,
        then_block: Vec<Statement>,
        else_block: Vec<Statement>,
        suppress: bool,
        pos: usize,
    },
    While {
        cond: Node,
        body: Vec<Statement>,
        suppress: bool,
        pos: usize,
    },
    For {
        var: String,
        iter: Node,
        body: Vec<Statement>,
        suppress: bool,
        pos: usize,
    },
}

impl Statement {
    pub fn with_suppress(self, suppress: bool) -> Self {
        match self {
            Statement::Expr(node, _) => Statement::Expr(node, suppress),
            Statement::Assign(name, node, _) => Statement::Assign(name, node, suppress),
            Statement::FuncDef(name, params, body, _) => {
                Statement::FuncDef(name, params, body, suppress)
            }
            Statement::If {
                cond,
                then_block,
                else_block,
                pos,
                ..
            } => Statement::If {
                cond,
                then_block,
                else_block,
                suppress,
                pos,
            },
            Statement::While {
                cond, body, pos, ..
            } => Statement::While {
                cond,
                body,
                suppress,
                pos,
            },
            Statement::For {
                var,
                iter,
                body,
                pos,
                ..
            } => Statement::For {
                var,
                iter,
                body,
                suppress,
                pos,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct Program {
    pub statements: Vec<Statement>,
}
