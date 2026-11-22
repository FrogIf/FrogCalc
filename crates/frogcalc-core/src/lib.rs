//! FrogCalc 计算内核。
//!
//! 纯 Rust 实现：精确有理数、高精度十进制、符号表达式、微积分与方程求解。

pub mod calculus;
pub mod error;
pub mod eval;
pub mod numeric;
pub mod parser;
pub mod polynomial;
pub mod solve;
pub mod symbolic;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
