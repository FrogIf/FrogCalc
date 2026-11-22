use num_traits::{One, Signed, ToPrimitive, Zero};

use crate::error::{CalcError, CalcResult};
use crate::numeric::Rat;
use crate::polynomial::{
    factor_poly, poly_degree, poly_div_mod, poly_from_expr, poly_gcd, poly_is_zero, poly_to_expr,
    Poly,
};
use crate::symbolic::{add, collect_symbols, div, integer, mul, num, one, pow, zero, Expr};

const MAX_EXPAND_TERMS: usize = 8000;

pub fn expand(expr: &Expr) -> CalcResult<Expr> {
    expand_depth(expr, 0)
}

fn expand_depth(expr: &Expr, depth: usize) -> CalcResult<Expr> {
    if depth > 80 {
        return Err(CalcError::new("展开层数过深"));
    }
    match expr {
        Expr::Add(terms) => {
            let expanded: CalcResult<Vec<Expr>> =
                terms.iter().map(|t| expand_depth(t, depth + 1)).collect();
            Ok(add(expanded?))
        }
        Expr::Mul(factors) => {
            let mut terms: Vec<Vec<Expr>> = vec![Vec::new()];
            for factor in factors {
                let expanded = expand_depth(factor, depth + 1)?;
                let factor_terms: Vec<Expr> = match expanded {
                    Expr::Add(items) => items,
                    other => vec![other],
                };
                let mut next = Vec::new();
                for prefix in &terms {
                    for term in &factor_terms {
                        let mut combo = prefix.clone();
                        combo.push(term.clone());
                        next.push(combo);
                        if next.len() > MAX_EXPAND_TERMS {
                            return Err(CalcError::new("展开结果过大"));
                        }
                    }
                }
                terms = next;
            }
            Ok(add(terms.into_iter().map(|factors| mul(factors)).collect()))
        }
        Expr::Pow(base, exp) => {
            let base = expand_depth(base, depth + 1)?;
            if let Expr::Num(v) = &**exp {
                if v.is_integer() && v.is_positive() {
                    if let Some(n) = v.numer().to_u32() {
                        if n <= 20 {
                            if let Expr::Add(_) = base {
                                let mut result = one();
                                for _ in 0..n {
                                    result = expand_mul(&result, &base, depth + 1)?;
                                }
                                return Ok(result);
                            }
                        }
                    }
                }
            }
            Ok(pow(base, expand_depth(exp, depth + 1)?))
        }
        Expr::Call(name, args) => {
            let args: CalcResult<Vec<Expr>> =
                args.iter().map(|a| expand_depth(a, depth + 1)).collect();
            Ok(crate::symbolic::call_expr(name, args?))
        }
        Expr::Eq(lhs, rhs) => Ok(crate::symbolic::eq_expr(
            expand_depth(lhs, depth + 1)?,
            expand_depth(rhs, depth + 1)?,
        )),
        Expr::List(items) => {
            let items: CalcResult<Vec<Expr>> =
                items.iter().map(|i| expand_depth(i, depth + 1)).collect();
            Ok(Expr::List(items?))
        }
        _ => Ok(expr.clone()),
    }
}

fn expand_mul(a: &Expr, b: &Expr, depth: usize) -> CalcResult<Expr> {
    let left = expand_depth(a, depth + 1)?;
    let right = expand_depth(b, depth + 1)?;
    let left_terms: Vec<Expr> = match left {
        Expr::Add(items) => items,
        other => vec![other],
    };
    let right_terms: Vec<Expr> = match right {
        Expr::Add(items) => items,
        other => vec![other],
    };
    if left_terms.len() * right_terms.len() > MAX_EXPAND_TERMS {
        return Err(CalcError::new("展开结果过大"));
    }
    let mut out = Vec::new();
    for l in &left_terms {
        for r in &right_terms {
            out.push(mul(vec![l.clone(), r.clone()]));
        }
    }
    Ok(add(out))
}

#[derive(Clone, Debug)]
pub struct FractionParts {
    pub n: Expr,
    pub d: Expr,
}

pub fn to_fraction(expr: &Expr) -> FractionParts {
    match expr {
        Expr::Add(terms) => {
            let mut acc = FractionParts {
                n: zero(),
                d: one(),
            };
            for term in terms {
                let f = to_fraction(term);
                acc = FractionParts {
                    n: add(vec![
                        mul(vec![acc.n.clone(), f.d.clone()]),
                        mul(vec![f.n.clone(), acc.d.clone()]),
                    ]),
                    d: mul(vec![acc.d.clone(), f.d]),
                };
            }
            acc
        }
        Expr::Mul(factors) => {
            let mut n = one();
            let mut d = one();
            for factor in factors {
                let f = to_fraction(factor);
                n = mul(vec![n, f.n]);
                d = mul(vec![d, f.d]);
            }
            FractionParts { n, d }
        }
        Expr::Pow(base, exp) => {
            if let Expr::Num(v) = &**exp {
                if v.is_integer() {
                    if let Some(e) = v.numer().to_i64() {
                        if e.unsigned_abs() <= 100 {
                            let base = to_fraction(base);
                            if e >= 0 {
                                return FractionParts {
                                    n: pow(base.n, integer(e)),
                                    d: pow(base.d, integer(e)),
                                };
                            }
                            let e = -e;
                            return FractionParts {
                                n: pow(base.d, integer(e)),
                                d: pow(base.n, integer(e)),
                            };
                        }
                    }
                }
            }
            FractionParts {
                n: expr.clone(),
                d: one(),
            }
        }
        _ => FractionParts {
            n: expr.clone(),
            d: one(),
        },
    }
}

fn cancel_fraction(parts: FractionParts) -> FractionParts {
    let mut n = parts.n;
    let mut d = parts.d;
    if matches!(&d, Expr::Num(v) if v.is_one()) {
        return FractionParts { n, d };
    }
    if matches!(&n, Expr::Num(v) if v.is_zero()) {
        return FractionParts {
            n: zero(),
            d: one(),
        };
    }
    let mut symbols = Vec::new();
    collect_symbols(&n, &mut symbols);
    collect_symbols(&d, &mut symbols);
    for name in symbols {
        if matches!(name.as_str(), "pi" | "e" | "i") {
            continue;
        }
        let Some(pn) = poly_from_expr(&n, &name) else {
            continue;
        };
        let Some(pd) = poly_from_expr(&d, &name) else {
            continue;
        };
        if poly_is_zero(&pn) || poly_is_zero(&pd) {
            continue;
        }
        let Ok(gcd) = poly_gcd(&pn, &pd) else {
            continue;
        };
        if poly_degree(&gcd).unwrap_or(0) == 0 {
            continue;
        }
        let Ok((qn, rn)) = poly_div_mod(&pn, &gcd) else {
            continue;
        };
        let Ok((qd, rd)) = poly_div_mod(&pd, &gcd) else {
            continue;
        };
        if !poly_is_zero(&rn) || !poly_is_zero(&rd) {
            continue;
        }
        n = poly_to_expr(&qn);
        d = poly_to_expr(&qd);
        if matches!(&d, Expr::Num(v) if v.is_one()) {
            break;
        }
    }
    FractionParts { n, d }
}

fn simplify_fraction(expr: &Expr) -> Expr {
    let parts = cancel_fraction(to_fraction(expr));
    if matches!(&parts.d, Expr::Num(v) if v.is_one()) {
        parts.n
    } else {
        div(parts.n, parts.d)
    }
}

pub fn simplify(expr: &Expr) -> Expr {
    match expr {
        Expr::Add(terms) => simplify_fraction(&add(terms.iter().map(simplify).collect())),
        Expr::Mul(factors) => simplify_fraction(&mul(factors.iter().map(simplify).collect())),
        Expr::Pow(base, exp) => simplify_fraction(&pow(simplify(base), simplify(exp))),
        Expr::Call(name, args) => simplify_fraction(&crate::symbolic::call_expr(
            name,
            args.iter().map(simplify).collect(),
        )),
        Expr::Eq(lhs, rhs) => crate::symbolic::eq_expr(simplify(lhs), simplify(rhs)),
        Expr::List(items) => Expr::List(items.iter().map(simplify).collect()),
        other => other.clone(),
    }
}

pub fn factor(expr: &Expr) -> Expr {
    let mut symbols = Vec::new();
    collect_symbols(expr, &mut symbols);
    let mut best: Option<(String, Poly)> = None;
    for name in symbols {
        if matches!(name.as_str(), "pi" | "e" | "i") {
            continue;
        }
        if let Some(poly) = poly_from_expr(expr, &name) {
            let degree = poly_degree(&poly).unwrap_or(0);
            if degree > 0
                && best
                    .as_ref()
                    .map(|(_, p)| degree > poly_degree(p).unwrap_or(0))
                    .unwrap_or(true)
            {
                best = Some((name, poly));
            }
        }
    }
    let Some((_, poly)) = best else {
        return expr.clone();
    };
    let Ok(result) = factor_poly(&poly) else {
        return expr.clone();
    };
    if result.factors.is_empty() {
        return num(result.content);
    }
    let mut parts = Vec::new();
    if !result.content.is_one() {
        parts.push(num(result.content.clone()));
    }
    for (factor, mult) in &result.factors {
        let p = poly_to_expr(factor);
        parts.push(if *mult == 1 {
            p
        } else {
            pow(p, integer(*mult as i64))
        });
    }
    if parts.len() == 1 && result.content.is_one() {
        if result.factors.len() == 1 && result.factors[0].1 == 1 {
            return expr.clone();
        }
    }
    mul(parts)
}

pub fn fraction_to_expr(parts: FractionParts) -> Expr {
    if matches!(&parts.d, Expr::Num(v) if v.is_one()) {
        parts.n
    } else {
        div(parts.n, parts.d)
    }
}

pub fn rational_from_i64(n: i64, d: i64) -> Rat {
    crate::numeric::rat(n, d)
}
