use num_integer::Integer;
use num_traits::Zero;

use crate::numeric::rat;
use crate::polynomial::{poly_degree, poly_from_expr, solve_poly};
use crate::symbolic::{
    add, call_expr, div, has_float, integer, mul, neg, num, occurs, one, pow, sub, sym, Expr,
};

pub struct SolveResult {
    pub solutions: Vec<Expr>,
    pub note: Option<String>,
    pub unsupported: bool,
}

pub fn solve_equation(expr: &Expr, var: &str) -> SolveResult {
    let normalized = match expr {
        Expr::Eq(lhs, rhs) => sub((**lhs).clone(), (**rhs).clone()),
        other => other.clone(),
    };
    let x = sym(var);
    if !occurs(&normalized, var) {
        if matches!(&normalized, Expr::Num(v) if v.is_zero()) {
            return SolveResult {
                solutions: vec![x],
                note: Some("等式恒成立，任意 x 都是解".into()),
                unsupported: false,
            };
        }
        return SolveResult {
            solutions: Vec::new(),
            note: Some("等式与 x 无关，无解".into()),
            unsupported: false,
        };
    }
    if let Some(poly) = poly_from_expr(&normalized, var) {
        if poly_degree(&poly).unwrap_or(0) > 0 {
            match solve_poly(&poly) {
                Ok(mut solutions) => {
                    if !solutions.is_empty() {
                        solutions.sort_by(|a, b| {
                            let a_num = matches!(a, Expr::Num(_));
                            let b_num = matches!(b, Expr::Num(_));
                            if a_num && b_num {
                                match (a, b) {
                                    (Expr::Num(x), Expr::Num(y)) => x.cmp(y),
                                    _ => std::cmp::Ordering::Equal,
                                }
                            } else if a_num {
                                std::cmp::Ordering::Less
                            } else if b_num {
                                std::cmp::Ordering::Greater
                            } else {
                                std::cmp::Ordering::Equal
                            }
                        });
                        let numeric = solutions.iter().any(has_float);
                        return SolveResult {
                            solutions,
                            note: if numeric {
                                Some("高次多项式没有全部精确根，部分结果为数值近似根".into())
                            } else {
                                None
                            },
                            unsupported: false,
                        };
                    }
                    return SolveResult {
                        solutions: Vec::new(),
                        note: Some("多项式没有找到根".into()),
                        unsupported: false,
                    };
                }
                Err(_) => {}
            }
        }
    }
    if let Some(candidates) = invert(&normalized, var, &Expr::Num(rat(0, 1)), 0) {
        if !candidates.is_empty() {
            let mut seen = std::collections::BTreeSet::new();
            let mut solutions = Vec::new();
            for candidate in candidates {
                if seen.insert(crate::symbolic::key(&candidate)) {
                    solutions.push(candidate);
                }
            }
            return SolveResult {
                solutions,
                note: Some("使用逆运算求得主值解；周期方程（如三角方程）可能还有更多分支".into()),
                unsupported: false,
            };
        }
    }
    SolveResult {
        solutions: Vec::new(),
        note: None,
        unsupported: true,
    }
}

fn invert(expr: &Expr, var: &str, target: &Expr, depth: usize) -> Option<Vec<Expr>> {
    if depth > 40 {
        return None;
    }
    if let Expr::Sym(name) = expr {
        if name == var {
            return Some(vec![target.clone()]);
        }
    }
    if !occurs(expr, var) {
        return None;
    }
    match expr {
        Expr::Add(terms) => {
            let with_var: Vec<&Expr> = terms.iter().filter(|t| occurs(t, var)).collect();
            if with_var.len() != 1 {
                return None;
            }
            let others: Vec<Expr> = terms.iter().filter(|t| !occurs(t, var)).cloned().collect();
            invert(
                with_var[0],
                var,
                &sub(target.clone(), add(others)),
                depth + 1,
            )
        }
        Expr::Mul(factors) => {
            let with_var: Vec<&Expr> = factors.iter().filter(|f| occurs(f, var)).collect();
            if with_var.len() != 1 {
                return None;
            }
            let others: Vec<Expr> = factors
                .iter()
                .filter(|f| !occurs(f, var))
                .cloned()
                .collect();
            invert(
                with_var[0],
                var,
                &div(target.clone(), mul(others)),
                depth + 1,
            )
        }
        Expr::Pow(base, exp) => {
            if occurs(exp, var) {
                return None;
            }
            let Expr::Num(n) = &**exp else {
                return None;
            };
            let root = pow(target.clone(), pow((**exp).clone(), integer(-1)));
            if n.is_integer() && n.numer().is_even() {
                let a = invert(base, var, &root, depth + 1);
                let b = invert(base, var, &neg(root), depth + 1);
                merge(a, b)
            } else {
                invert(base, var, &root, depth + 1)
            }
        }
        Expr::Call(name, args) => {
            if args.len() != 1 {
                return None;
            }
            let targets = inverse_function(name, target)?;
            let mut results = Vec::new();
            for t in targets {
                if let Some(r) = invert(&args[0], var, &t, depth + 1) {
                    results.extend(r);
                }
            }
            if results.is_empty() {
                None
            } else {
                Some(results)
            }
        }
        _ => None,
    }
}

fn merge(a: Option<Vec<Expr>>, b: Option<Vec<Expr>>) -> Option<Vec<Expr>> {
    match (a, b) {
        (None, None) => None,
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (Some(mut a), Some(b)) => {
            a.extend(b);
            Some(a)
        }
    }
}

fn inverse_function(name: &str, t: &Expr) -> Option<Vec<Expr>> {
    match name.to_lowercase().as_str() {
        "sin" => Some(vec![
            call_expr("asin", vec![t.clone()]),
            sub(sym("pi"), call_expr("asin", vec![t.clone()])),
        ]),
        "cos" => Some(vec![
            call_expr("acos", vec![t.clone()]),
            neg(call_expr("acos", vec![t.clone()])),
        ]),
        "tan" => Some(vec![call_expr("atan", vec![t.clone()])]),
        "asin" => Some(vec![call_expr("sin", vec![t.clone()])]),
        "acos" => Some(vec![call_expr("cos", vec![t.clone()])]),
        "atan" => Some(vec![call_expr("tan", vec![t.clone()])]),
        "exp" => Some(vec![call_expr("ln", vec![t.clone()])]),
        "ln" => Some(vec![call_expr("exp", vec![t.clone()])]),
        "sqrt" => Some(vec![pow(t.clone(), integer(2))]),
        "abs" => Some(vec![t.clone(), neg(t.clone())]),
        "sinh" => Some(vec![call_expr(
            "ln",
            vec![add(vec![
                t.clone(),
                pow(add(vec![pow(t.clone(), integer(2)), one()]), num(rat(1, 2))),
            ])],
        )]),
        "cosh" => {
            let principal = call_expr(
                "ln",
                vec![add(vec![
                    t.clone(),
                    pow(sub(pow(t.clone(), integer(2)), one()), num(rat(1, 2))),
                ])],
            );
            Some(vec![principal.clone(), neg(principal)])
        }
        "tanh" => Some(vec![mul(vec![
            num(rat(1, 2)),
            call_expr(
                "ln",
                vec![div(add(vec![one(), t.clone()]), sub(one(), t.clone()))],
            ),
        ])]),
        _ => None,
    }
}
