//! 变量假设与由假设驱动的化简。
//!
//! 假设只记录符号的符号性质（正/负/零/非负/非正），用 `assume(x > 0)` 这类语句建立。
//! 化简阶段用它把 `abs(x)`、`sqrt(x^2)` 一类表达式改写成更简单的形式。

use std::collections::BTreeMap;

use num_integer::Integer;
use num_traits::{Signed, ToPrimitive, Zero};

use crate::numeric::Rat;
use crate::symbolic::{add, call_expr, eq_expr, integer, mul, neg, num, one, pow, zero, Expr};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign {
    Positive,
    Negative,
    Zero,
    NonNegative,
    NonPositive,
    Unknown,
}

impl Sign {
    pub fn is_positive(self) -> bool {
        self == Sign::Positive
    }

    pub fn is_non_negative(self) -> bool {
        matches!(self, Sign::Positive | Sign::NonNegative | Sign::Zero)
    }

    fn negate(self) -> Sign {
        match self {
            Sign::Positive => Sign::Negative,
            Sign::Negative => Sign::Positive,
            Sign::Zero => Sign::Zero,
            Sign::NonNegative => Sign::NonPositive,
            Sign::NonPositive => Sign::NonNegative,
            Sign::Unknown => Sign::Unknown,
        }
    }

    fn product(self, other: Sign) -> Sign {
        use Sign::*;
        match (self, other) {
            (Zero, _) | (_, Zero) => Zero,
            (Unknown, _) | (_, Unknown) => Unknown,
            (Positive, Positive) => Positive,
            (Positive, Negative) => Negative,
            (Negative, Positive) => Negative,
            (Negative, Negative) => Positive,
            (Positive, NonNegative) | (NonNegative, Positive) => NonNegative,
            (NonNegative, NonNegative) => NonNegative,
            (Positive, NonPositive) | (NonPositive, Positive) => NonPositive,
            (Negative, NonNegative) | (NonNegative, Negative) => NonPositive,
            (Negative, NonPositive) | (NonPositive, Negative) => NonNegative,
            (NonNegative, NonPositive) | (NonPositive, NonNegative) => NonPositive,
            (NonPositive, NonPositive) => NonNegative,
        }
    }

    fn from_rat(value: &Rat) -> Sign {
        if value.is_zero() {
            Sign::Zero
        } else if value.is_negative() {
            Sign::Negative
        } else {
            Sign::Positive
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Assumptions {
    facts: BTreeMap<String, Fact>,
}

#[derive(Clone, Copy, Debug)]
pub struct Fact {
    pub sign: Sign,
    pub nonzero: bool,
}

impl Default for Fact {
    fn default() -> Self {
        Self {
            sign: Sign::Unknown,
            nonzero: false,
        }
    }
}

impl Assumptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }

    pub fn clear(&mut self) {
        self.facts.clear();
    }

    /// 记录 `name` 的符号性质；互相矛盾的假设会被新的假设覆盖。
    pub fn set(&mut self, name: &str, sign: Sign) {
        let entry = self.facts.entry(name.to_string()).or_default();
        entry.sign = sign;
        if sign != Sign::Unknown {
            entry.nonzero = sign != Sign::Zero;
        }
        if entry.sign == Sign::Unknown && !entry.nonzero {
            self.facts.remove(name);
        }
    }

    pub fn set_nonzero(&mut self, name: &str, nonzero: bool) {
        let entry = self.facts.entry(name.to_string()).or_default();
        entry.nonzero = nonzero;
        if entry.sign == Sign::Unknown && !entry.nonzero {
            self.facts.remove(name);
        }
    }

    pub fn forget(&mut self, name: &str) -> bool {
        self.facts.remove(name).is_some()
    }

    pub fn get(&self, name: &str) -> Option<Fact> {
        self.facts.get(name).copied()
    }

    pub fn entries(&self) -> impl Iterator<Item = (&String, &Fact)> {
        self.facts.iter()
    }

    pub fn describe(&self) -> Vec<(String, String)> {
        self.facts
            .iter()
            .map(|(name, fact)| (name.clone(), fact_text(*fact)))
            .collect()
    }

    pub fn to_source(&self) -> Vec<String> {
        self.facts
            .iter()
            .filter_map(|(name, fact)| {
                let op = match fact.sign {
                    Sign::Positive => ">",
                    Sign::Negative => "<",
                    Sign::NonNegative => ">=",
                    Sign::NonPositive => "<=",
                    Sign::Zero => "==",
                    Sign::Unknown => {
                        return if fact.nonzero {
                            Some(format!("assume({name} != 0)"))
                        } else {
                            None
                        }
                    }
                };
                Some(format!("assume({name} {op} 0)"))
            })
            .collect()
    }

    pub fn sign_of(&self, expr: &Expr) -> Sign {
        match expr {
            Expr::Num(value) => Sign::from_rat(value),
            Expr::Float(value) => {
                if value.is_zero() {
                    Sign::Zero
                } else if value.is_negative() {
                    Sign::Negative
                } else {
                    Sign::Positive
                }
            }
            Expr::Sym(name) => match name.as_str() {
                "pi" | "e" => Sign::Positive,
                "i" => Sign::Unknown,
                other => self
                    .facts
                    .get(other)
                    .map(|fact| fact.sign)
                    .unwrap_or(Sign::Unknown),
            },
            Expr::Add(terms) => self.sum_sign(terms),
            Expr::Mul(factors) => factors.iter().fold(Sign::Positive, |acc, factor| {
                acc.product(self.sign_of(factor))
            }),
            Expr::Pow(base, exp) => match &**exp {
                Expr::Num(value) if value.is_integer() => {
                    let base_sign = self.sign_of(base);
                    let exponent = value.numer();
                    if exponent.is_zero() {
                        return Sign::Positive;
                    }
                    if base_sign == Sign::Zero {
                        return if exponent.is_positive() {
                            Sign::Zero
                        } else {
                            Sign::Unknown
                        };
                    }
                    if exponent.is_even() {
                        Sign::Positive
                    } else {
                        base_sign
                    }
                }
                Expr::Num(_) => {
                    if self.sign_of(base).is_positive() {
                        Sign::Positive
                    } else {
                        Sign::Unknown
                    }
                }
                _ => Sign::Unknown,
            },
            Expr::Call(name, _) => match name.to_lowercase().as_str() {
                "abs" => Sign::NonNegative,
                "exp" | "cosh" => Sign::Positive,
                "sqrt" => Sign::NonNegative,
                _ => Sign::Unknown,
            },
            _ => Sign::Unknown,
        }
    }

    fn sum_sign(&self, terms: &[Expr]) -> Sign {
        let mut positive = false;
        let mut non_negative = false;
        let mut negative = false;
        let mut non_positive = false;
        let mut zero_term = false;
        for term in terms {
            match self.sign_of(term) {
                Sign::Positive => positive = true,
                Sign::NonNegative => non_negative = true,
                Sign::Negative => negative = true,
                Sign::NonPositive => non_positive = true,
                Sign::Zero => zero_term = true,
                Sign::Unknown => return Sign::Unknown,
            }
        }
        if positive && (negative || non_positive) {
            return Sign::Unknown;
        }
        if negative && (positive || non_negative) {
            return Sign::Unknown;
        }
        if positive {
            Sign::Positive
        } else if non_negative {
            Sign::NonNegative
        } else if negative {
            Sign::Negative
        } else if non_positive {
            Sign::NonPositive
        } else if zero_term {
            Sign::Zero
        } else {
            Sign::Unknown
        }
    }

    // -----------------------------------------------------------------------
    // 由假设驱动的改写
    // -----------------------------------------------------------------------

    pub fn rewrite(&self, expr: &Expr) -> Expr {
        match expr {
            Expr::Add(terms) => add(terms.iter().map(|t| self.rewrite(t)).collect()),
            Expr::Mul(factors) => mul(factors.iter().map(|f| self.rewrite(f)).collect()),
            Expr::Pow(base, exp) => {
                let base = self.rewrite(base);
                let exp = self.rewrite(exp);
                if let Some(value) = self.rewrite_root_of_even_power(&base, &exp) {
                    return value;
                }
                pow(base, exp)
            }
            Expr::Call(name, args) => {
                let args: Vec<Expr> = args.iter().map(|a| self.rewrite(a)).collect();
                match (name.to_lowercase().as_str(), args.len()) {
                    ("abs", 1) => self.rewrite_abs(args[0].clone()),
                    ("sign", 1) => match self.sign_of(&args[0]) {
                        Sign::Positive => one(),
                        Sign::Negative => integer(-1),
                        Sign::Zero => zero(),
                        _ => call_expr("sign", args),
                    },
                    _ => call_expr(name, args),
                }
            }
            Expr::Eq(lhs, rhs) => eq_expr(self.rewrite(lhs), self.rewrite(rhs)),
            Expr::List(items) => Expr::List(items.iter().map(|i| self.rewrite(i)).collect()),
            other => other.clone(),
        }
    }

    fn rewrite_abs(&self, value: Expr) -> Expr {
        match self.sign_of(&value) {
            Sign::Positive | Sign::NonNegative => value,
            Sign::Negative | Sign::NonPositive => neg(value),
            Sign::Zero => zero(),
            Sign::Unknown => call_expr("abs", vec![value]),
        }
    }

    /// `(u^(2m))^(p/q) = |u|^(2m p/q)`；当指数为整数时返回结果。
    fn rewrite_root_of_even_power(&self, base: &Expr, exp: &Expr) -> Option<Expr> {
        let Expr::Num(e) = exp else { return None };
        let Expr::Pow(inner, inner_exp) = base else {
            return None;
        };
        let Expr::Num(k) = &**inner_exp else {
            return None;
        };
        if !k.is_integer() {
            return None;
        }
        let k = k.numer().to_i64()?;
        if k % 2 != 0 {
            return None;
        }
        let exponent = Rat::from_integer(k.into()) * e.clone();
        if !exponent.is_integer() {
            return None;
        }
        let magnitude = self.rewrite_abs((**inner).clone());
        Some(pow(magnitude, num(exponent)))
    }
}

fn fact_text(fact: Fact) -> String {
    if fact.sign == Sign::Unknown {
        if fact.nonzero {
            "≠ 0".to_string()
        } else {
            "未知".to_string()
        }
    } else {
        sign_text(fact.sign).to_string()
    }
}

fn sign_text(sign: Sign) -> &'static str {
    match sign {
        Sign::Positive => "> 0",
        Sign::Negative => "< 0",
        Sign::Zero => "= 0",
        Sign::NonNegative => "≥ 0",
        Sign::NonPositive => "≤ 0",
        Sign::Unknown => "未知",
    }
}

/// 供 `assume` 语句解析使用：把 `x > 0` 这类比较转成符号性质。
pub fn sign_from_comparison(operator: &str, is_reversed: bool) -> Option<Sign> {
    let forward = match operator {
        "gt" => Sign::Positive,
        "ge" => Sign::NonNegative,
        "lt" => Sign::Negative,
        "le" => Sign::NonPositive,
        "eq" => Sign::Zero,
        _ => return None,
    };
    if is_reversed {
        Some(forward.negate())
    } else {
        Some(forward)
    }
}
