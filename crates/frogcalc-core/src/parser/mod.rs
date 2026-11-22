pub mod ast;
mod lexer;
mod parser;

pub use ast::{FunctionBody, Node, Program, Statement};
pub use parser::parse;
