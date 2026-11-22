use num_traits::Zero;

use crate::error::{CalcError, CalcResult};
use crate::symbolic::transform::simplify;
use crate::symbolic::{
    add, call_expr, div, eq_expr, integer, mul, num, one, pow, sub, sym, zero, Expr,
};

pub fn diff(expr: &Expr, var: &str, order: usize) -> CalcResult<Expr> {
    let mut result = expr.clone();
    for _ in 0..order {
        result = simplify(&diff_raw(&result, var));
    }
    Ok(result)
}

fn diff_raw(expr: &Expr, var: &str) -> Expr {
    match expr {
        Expr::Num(_) | Expr::Float(_) | Expr::Str(_) => zero(),
        Expr::Sym(name) => {
            if name == var {
                one()
            } else {
                zero()
            }
        }
        Expr::Add(terms) => add(terms.iter().map(|t| diff_raw(t, var)).collect()),
        Expr::Mul(factors) => {
            let mut terms = Vec::new();
            for (i, factor) in factors.iter().enumerate() {
                let d = diff_raw(factor, var);
                if matches!(&d, Expr::Num(v) if v.is_zero()) {
                    continue;
                }
                let mut product = vec![d];
                for (j, other) in factors.iter().enumerate() {
                    if i != j {
                        product.push(other.clone());
                    }
                }
                terms.push(mul(product));
            }
            add(terms)
        }
        Expr::Pow(base, exp) => {
            let db = diff_raw(base, var);
            let de = diff_raw(exp, var);
            if matches!(&db, Expr::Num(v) if v.is_zero())
                && matches!(&de, Expr::Num(v) if v.is_zero())
            {
                return zero();
            }
            if matches!(&de, Expr::Num(v) if v.is_zero()) {
                return mul(vec![
                    (**exp).clone(),
                    pow((**base).clone(), sub((**exp).clone(), one())),
                    db,
                ]);
            }
            mul(vec![
                expr.clone(),
                add(vec![
                    mul(vec![de, call_expr("ln", vec![(**base).clone()])]),
                    div(mul(vec![(**exp).clone(), db]), (**base).clone()),
                ]),
            ])
        }
        Expr::Call(name, args) => {
            if args.len() != 1 {
                return call_expr("diff", vec![expr.clone(), sym(var)]);
            }
            diff_call(name, &args[0], var)
        }
        Expr::Eq(lhs, rhs) => eq_expr(diff_raw(lhs, var), diff_raw(rhs, var)),
        Expr::List(items) => Expr::List(items.iter().map(|i| diff_raw(i, var)).collect()),
    }
}

fn diff_call(name: &str, arg: &Expr, var: &str) -> Expr {
    let lower = name.to_lowercase();
    let du = diff_raw(arg, var);
    if matches!(&du, Expr::Num(v) if v.is_zero()) {
        return zero();
    }
    match lower.as_str() {
        "sin" => mul(vec![call_expr("cos", vec![arg.clone()]), du]),
        "cos" => mul(vec![integer(-1), call_expr("sin", vec![arg.clone()]), du]),
        "tan" => div(du, pow(call_expr("cos", vec![arg.clone()]), integer(2))),
        "asin" => div(
            du,
            pow(
                sub(one(), pow(arg.clone(), integer(2))),
                num(crate::numeric::rat(1, 2)),
            ),
        ),
        "acos" => mul(vec![
            integer(-1),
            div(
                du,
                pow(
                    sub(one(), pow(arg.clone(), integer(2))),
                    num(crate::numeric::rat(1, 2)),
                ),
            ),
        ]),
        "atan" => div(du, add(vec![one(), pow(arg.clone(), integer(2))])),
        "sinh" => mul(vec![call_expr("cosh", vec![arg.clone()]), du]),
        "cosh" => mul(vec![call_expr("sinh", vec![arg.clone()]), du]),
        "tanh" => mul(vec![
            sub(one(), pow(call_expr("tanh", vec![arg.clone()]), integer(2))),
            du,
        ]),
        "exp" => mul(vec![call_expr("exp", vec![arg.clone()]), du]),
        "ln" => div(du, arg.clone()),
        "log10" => div(
            du,
            mul(vec![arg.clone(), call_expr("ln", vec![integer(10)])]),
        ),
        "log2" => div(
            du,
            mul(vec![arg.clone(), call_expr("ln", vec![integer(2)])]),
        ),
        "abs" => mul(vec![
            du,
            div(arg.clone(), call_expr("abs", vec![arg.clone()])),
        ]),
        "sign" | "floor" | "ceil" | "round" => zero(),
        _ => call_expr("diff", vec![Expr::Call(lower, vec![arg.clone()]), sym(var)]),
    }
}

pub fn check_order(order: i64) -> CalcResult<usize> {
    if order < 0 {
        return Err(CalcError::new("求导阶数必须是非负整数"));
    }
    Ok(order as usize)
}
