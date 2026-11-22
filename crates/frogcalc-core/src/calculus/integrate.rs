use num_traits::{One, Signed, ToPrimitive, Zero};

use crate::numeric::{rat, rat_to_f64, BigFloat, Rat};
use crate::polynomial::{
    poly_degree, poly_div_mod, poly_from_expr, poly_is_zero, poly_major_coeff, poly_monic,
    poly_mul, poly_pow, poly_scale, poly_to_expr, squarefree_parts, Poly,
};
use crate::symbolic::transform::{simplify, to_fraction};
use crate::symbolic::{
    add, call_expr, div, equals, integer, mul, num, occurs, one, pow, sub, sym, zero, Expr,
};

use super::diff::diff;

const MAX_DEPTH: usize = 24;

fn x_of(var: &str) -> Expr {
    sym(var)
}

fn unevaluated(expr: &Expr, var: &str) -> Expr {
    call_expr("integrate", vec![expr.clone(), sym(var)])
}

fn is_unevaluated(expr: &Expr, var: &str) -> bool {
    matches!(
        expr,
        Expr::Call(name, args)
            if name == "integrate"
                && args.len() >= 2
                && matches!(&args[1], Expr::Sym(n) if n == var)
    )
}

fn contains_unevaluated(expr: &Expr, var: &str) -> bool {
    match expr {
        Expr::Call(_, args) if is_unevaluated(expr, var) => {
            let _ = args;
            true
        }
        Expr::Add(terms) => terms.iter().any(|t| contains_unevaluated(t, var)),
        Expr::Mul(factors) => factors.iter().any(|t| contains_unevaluated(t, var)),
        Expr::Pow(base, exp) => contains_unevaluated(base, var) || contains_unevaluated(exp, var),
        Expr::Call(_, args) => args.iter().any(|t| contains_unevaluated(t, var)),
        Expr::Eq(l, r) => contains_unevaluated(l, var) || contains_unevaluated(r, var),
        Expr::List(items) => items.iter().any(|t| contains_unevaluated(t, var)),
        _ => false,
    }
}

pub fn integrate(expr: &Expr, var: &str) -> Expr {
    simplify(&integrate_raw(expr, var, 0))
}

fn safe_integrate(expr: &Expr, var: &str, depth: usize) -> Option<Expr> {
    let result = integrate_raw(expr, var, depth + 1);
    if is_unevaluated(&result, var) || contains_unevaluated(&result, var) {
        None
    } else {
        Some(result)
    }
}

fn integrate_raw(expr: &Expr, var: &str, depth: usize) -> Expr {
    if depth > MAX_DEPTH {
        return unevaluated(expr, var);
    }
    if !occurs(expr, var) {
        return mul(vec![expr.clone(), x_of(var)]);
    }
    match expr {
        Expr::Sym(name) if name == var => div(pow(x_of(var), integer(2)), integer(2)),
        Expr::Sym(_) => mul(vec![expr.clone(), x_of(var)]),
        Expr::Add(terms) => add(terms
            .iter()
            .map(|t| integrate_raw(t, var, depth + 1))
            .collect()),
        Expr::Mul(factors) => integrate_mul(factors, var, depth),
        Expr::Pow(base, exp) => integrate_pow(base, exp, var, depth),
        Expr::Call(name, args) => integrate_call(name, args, var, depth),
        _ => unevaluated(expr, var),
    }
}

fn integrate_mul(factors: &[Expr], var: &str, depth: usize) -> Expr {
    let constant_factors: Vec<Expr> = factors
        .iter()
        .filter(|f| !occurs(f, var))
        .cloned()
        .collect();
    let variable_factors: Vec<Expr> = factors.iter().filter(|f| occurs(f, var)).cloned().collect();
    if !constant_factors.is_empty() {
        let constant = mul(constant_factors);
        let rest = if variable_factors.len() == 1 {
            variable_factors[0].clone()
        } else {
            mul(variable_factors)
        };
        return mul(vec![constant, integrate_raw(&rest, var, depth + 1)]);
    }
    if variable_factors.len() == 1 {
        return integrate_raw(&variable_factors[0], var, depth + 1);
    }
    let expr = mul(factors.to_vec());
    if let Some(result) = try_substitution(&expr, var, depth) {
        return result;
    }
    if let Some(result) = try_rational(&expr, var, depth) {
        return result;
    }
    if let Some(result) = try_by_parts(&expr, var, depth) {
        return result;
    }
    if let Some(result) = try_log_derivative(&expr, var, depth) {
        return result;
    }
    unevaluated(&expr, var)
}

fn integrate_pow(base: &Expr, exp: &Expr, var: &str, depth: usize) -> Expr {
    let x = x_of(var);
    if let Expr::Num(n) = exp {
        if let Expr::Sym(name) = base {
            if name == var {
                if n == &rat(-1, 1) {
                    return call_expr("ln", vec![call_expr("abs", vec![x])]);
                }
                let next = n + Rat::one();
                return div(pow(x, num(next.clone())), num(next));
            }
        }
        if let Some(linear) = poly_from_expr(base, var) {
            if poly_degree(&linear) == Some(1) {
                let a = linear.coeffs[1].clone();
                if n == &rat(-1, 1) {
                    return div(
                        call_expr("ln", vec![call_expr("abs", vec![base.clone()])]),
                        num(a),
                    );
                }
                let next = n + Rat::one();
                return div(pow(base.clone(), num(next.clone())), num(a * next));
            }
        }
        if n.is_integer() && n.is_positive() {
            if let Some(e) = n.numer().to_u32() {
                if e <= 12 {
                    if let Some(_) = poly_from_expr(base, var) {
                        let expanded =
                            crate::symbolic::transform::expand(&pow(base.clone(), exp.clone()));
                        if let Ok(expanded) = expanded {
                            return integrate_raw(&expanded, var, depth + 1);
                        }
                    }
                }
            }
        }
        if n == &rat(-1, 1) {
            if let Some(p) = poly_from_expr(base, var) {
                if poly_degree(&p) == Some(2) && p.coeffs[1].is_zero() {
                    let c = &p.coeffs[0];
                    let a = &p.coeffs[2];
                    if a.is_one() && c.is_one() {
                        return call_expr("atan", vec![x]);
                    }
                    if a.is_positive() && c.is_positive() {
                        let k = c / a;
                        let sqrt_k = pow(num(k.clone()), num(rat(1, 2)));
                        return mul(vec![
                            div(one(), mul(vec![sqrt_k.clone(), num(a.clone())])),
                            call_expr("atan", vec![div(x, sqrt_k)]),
                        ]);
                    }
                }
            }
        }
        if n == &rat(-1, 2) || n == &rat(1, 2) {
            if let Some(p) = poly_from_expr(base, var) {
                if poly_degree(&p) == Some(2) && p.coeffs[1].is_zero() {
                    let c = &p.coeffs[0];
                    let a = &p.coeffs[2];
                    if a == &rat(-1, 1) && c.is_positive() {
                        let sqrt_c = pow(num(c.clone()), num(rat(1, 2)));
                        if n == &rat(-1, 2) {
                            return call_expr("asin", vec![div(x.clone(), sqrt_c)]);
                        }
                        return add(vec![
                            div(
                                mul(vec![x.clone(), pow(base.clone(), num(rat(1, 2)))]),
                                integer(2),
                            ),
                            div(
                                mul(vec![
                                    num(c.clone()),
                                    call_expr("asin", vec![div(x, sqrt_c)]),
                                ]),
                                integer(2),
                            ),
                        ]);
                    }
                    if a.is_one() && c.is_positive() {
                        let sqrt_base = pow(base.clone(), num(rat(1, 2)));
                        let log = call_expr(
                            "ln",
                            vec![call_expr(
                                "abs",
                                vec![add(vec![x.clone(), sqrt_base.clone()])],
                            )],
                        );
                        if n == &rat(-1, 2) {
                            return log;
                        }
                        return add(vec![
                            div(mul(vec![x, sqrt_base]), integer(2)),
                            div(mul(vec![num(c.clone()), log]), integer(2)),
                        ]);
                    }
                }
            }
        }
    }
    let expr = pow(base.clone(), exp.clone());
    if let Some(result) = try_rational(&expr, var, depth) {
        return result;
    }
    if let Some(linear) = poly_from_expr(base, var) {
        if poly_degree(&linear) == Some(1) {
            if let Some(outer) = try_outer_antiderivative(&expr, base) {
                return div(outer, num(linear.coeffs[1].clone()));
            }
        }
    }
    if let Some(result) = try_log_derivative(&expr, var, depth) {
        return result;
    }
    unevaluated(&expr, var)
}

fn integrate_call(name: &str, args: &[Expr], var: &str, depth: usize) -> Expr {
    let expr = call_expr(name, args.to_vec());
    let Some(arg) = args.first() else {
        return unevaluated(&expr, var);
    };
    let linear = poly_from_expr(arg, var);
    let a = linear
        .as_ref()
        .filter(|p| poly_degree(p) == Some(1))
        .map(|p| p.coeffs[1].clone());
    if let Some(outer) = try_outer_antiderivative(&expr, arg) {
        if let Some(a) = a {
            return div(outer, num(a));
        }
        return unevaluated(&expr, var);
    }
    if name.eq_ignore_ascii_case("abs") && a.is_some() {
        let a = a.unwrap();
        return div(
            mul(vec![arg.clone(), call_expr("abs", vec![arg.clone()])]),
            num(a * rat(2, 1)),
        );
    }
    if let Some(result) = try_log_derivative(&expr, var, depth) {
        return result;
    }
    let _ = depth;
    unevaluated(&expr, var)
}

fn try_outer_antiderivative(f: &Expr, u: &Expr) -> Option<Expr> {
    match f {
        Expr::Call(name, _) => match name.to_lowercase().as_str() {
            "exp" => Some(call_expr("exp", vec![u.clone()])),
            "sin" => Some(mul(vec![integer(-1), call_expr("cos", vec![u.clone()])])),
            "cos" => Some(call_expr("sin", vec![u.clone()])),
            "tan" => Some(mul(vec![
                integer(-1),
                call_expr(
                    "ln",
                    vec![call_expr("abs", vec![call_expr("cos", vec![u.clone()])])],
                ),
            ])),
            "sinh" => Some(call_expr("cosh", vec![u.clone()])),
            "cosh" => Some(call_expr("sinh", vec![u.clone()])),
            "tanh" => Some(call_expr("ln", vec![call_expr("cosh", vec![u.clone()])])),
            "ln" => Some(sub(
                mul(vec![u.clone(), call_expr("ln", vec![u.clone()])]),
                u.clone(),
            )),
            "asin" => Some(add(vec![
                mul(vec![u.clone(), call_expr("asin", vec![u.clone()])]),
                pow(sub(one(), pow(u.clone(), integer(2))), num(rat(1, 2))),
            ])),
            "acos" => Some(sub(
                mul(vec![u.clone(), call_expr("acos", vec![u.clone()])]),
                pow(sub(one(), pow(u.clone(), integer(2))), num(rat(1, 2))),
            )),
            "atan" => Some(sub(
                mul(vec![u.clone(), call_expr("atan", vec![u.clone()])]),
                mul(vec![
                    num(rat(1, 2)),
                    call_expr("ln", vec![add(vec![one(), pow(u.clone(), integer(2))])]),
                ]),
            )),
            _ => None,
        },
        Expr::Pow(base, exp) => {
            if equals(base, u) {
                if let Expr::Num(n) = &**exp {
                    if n == &rat(-1, 1) {
                        return Some(call_expr("ln", vec![call_expr("abs", vec![u.clone()])]));
                    }
                    let next = n + Rat::one();
                    return Some(div(pow(u.clone(), num(next.clone())), num(next)));
                }
            }
            None
        }
        _ => None,
    }
}

fn try_substitution(expr: &Expr, var: &str, depth: usize) -> Option<Expr> {
    let Expr::Mul(factors) = expr else {
        return None;
    };
    for (i, factor) in factors.iter().enumerate() {
        let u = match factor {
            Expr::Call(_, args) if args.len() == 1 => Some(args[0].clone()),
            Expr::Pow(base, _) => Some((**base).clone()),
            _ => None,
        };
        let Some(u) = u else { continue };
        if !occurs(&u, var) {
            continue;
        }
        let Some(outer) = try_outer_antiderivative(factor, &u) else {
            continue;
        };
        let rest = mul(factors
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, f)| f.clone())
            .collect());
        let du = diff(&u, var, 1).ok()?;
        let quotient = simplify(&div(rest, du));
        if !occurs(&quotient, var) {
            return Some(mul(vec![quotient, outer]));
        }
    }
    let _ = depth;
    None
}

fn try_by_parts(expr: &Expr, var: &str, depth: usize) -> Option<Expr> {
    if depth > MAX_DEPTH - 4 {
        return None;
    }
    let Expr::Mul(factors) = expr else {
        return None;
    };
    for (i, factor) in factors.iter().enumerate() {
        if let Expr::Call(name, _) = factor {
            if name.eq_ignore_ascii_case("ln") {
                let rest = mul(factors
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, f)| f.clone())
                    .collect());
                let v_int = safe_integrate(&rest, var, depth)?;
                let du = diff(factor, var, 1).ok()?;
                let inner = safe_integrate(&mul(vec![v_int.clone(), du]), var, depth)?;
                return Some(sub(mul(vec![factor.clone(), v_int]), inner));
            }
        }
    }
    for (i, factor) in factors.iter().enumerate() {
        let Some(p) = poly_from_expr(factor, var) else {
            continue;
        };
        if poly_degree(&p).unwrap_or(0) == 0 {
            continue;
        }
        let rest = mul(factors
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, f)| f.clone())
            .collect());
        let Some(v_int) = safe_integrate(&rest, var, depth) else {
            continue;
        };
        let du = diff(factor, var, 1).ok()?;
        let Some(inner) = safe_integrate(&mul(vec![v_int.clone(), du]), var, depth) else {
            continue;
        };
        return Some(sub(mul(vec![factor.clone(), v_int]), inner));
    }
    None
}

fn try_log_derivative(expr: &Expr, var: &str, depth: usize) -> Option<Expr> {
    if depth > MAX_DEPTH - 4 {
        return None;
    }
    let mut candidates: Vec<Expr> = Vec::new();
    fn collect(expr: &Expr, var: &str, out: &mut Vec<Expr>) {
        match expr {
            Expr::Mul(factors) => {
                for factor in factors {
                    collect(factor, var, out);
                    out.push(factor.clone());
                }
            }
            Expr::Pow(base, _) => {
                collect(base, var, out);
                out.push((**base).clone());
            }
            Expr::Call(_, args) => {
                for arg in args {
                    collect(arg, var, out);
                    out.push(arg.clone());
                }
            }
            Expr::Add(terms) => {
                for term in terms {
                    collect(term, var, out);
                }
            }
            _ => {}
        }
    }
    collect(expr, var, &mut candidates);
    for candidate in candidates {
        if !occurs(&candidate, var) {
            continue;
        }
        let Ok(derivative) = diff(&candidate, var, 1) else {
            continue;
        };
        if matches!(&derivative, Expr::Num(value) if value.is_zero()) {
            continue;
        }
        let quotient = simplify(&div(mul(vec![expr.clone(), candidate.clone()]), derivative));
        if !occurs(&quotient, var) {
            return Some(mul(vec![
                quotient,
                call_expr("ln", vec![call_expr("abs", vec![candidate])]),
            ]));
        }
    }
    None
}

fn try_rational(expr: &Expr, var: &str, _depth: usize) -> Option<Expr> {
    let parts = to_fraction(expr);
    let pn = poly_from_expr(&parts.n, var)?;
    let pd = poly_from_expr(&parts.d, var)?;
    if poly_is_zero(&pd) || poly_degree(&pd).unwrap_or(0) == 0 {
        return None;
    }
    let mut numerator = pn;
    let denominator = pd;
    let mut polynomial_part: Option<Poly> = None;
    if poly_degree(&numerator).unwrap_or(0) >= poly_degree(&denominator).unwrap_or(0) {
        let (q, r) = poly_div_mod(&numerator, &denominator).ok()?;
        polynomial_part = Some(q);
        numerator = r;
        if poly_is_zero(&numerator) {
            return Some(integrate_poly(&polynomial_part.unwrap(), var));
        }
    }
    let proper = integrate_proper_fraction(&numerator, &denominator, var)?;
    if let Some(poly) = polynomial_part {
        Some(add(vec![integrate_poly(&poly, var), proper]))
    } else {
        Some(proper)
    }
}

fn integrate_poly(poly: &Poly, var: &str) -> Expr {
    let x = x_of(var);
    let mut terms = Vec::new();
    for (i, coeff) in poly.coeffs.iter().enumerate() {
        if coeff.is_zero() {
            continue;
        }
        let power = i as i64 + 1;
        terms.push(div(
            mul(vec![num(coeff.clone()), pow(x.clone(), integer(power))]),
            integer(power),
        ));
    }
    add(terms)
}

fn integrate_proper_fraction(numerator: &Poly, denominator: &Poly, var: &str) -> Option<Expr> {
    let leading = poly_major_coeff(denominator)?;
    let monic = poly_monic(denominator);
    let scaled = poly_scale(numerator, &leading.recip());
    let total = poly_degree(&monic)?;
    let parts = squarefree_parts(&monic);
    if parts.is_empty() {
        return None;
    }
    #[derive(Clone)]
    struct Column {
        part: usize,
        power: usize,
        shift: usize,
        poly: Poly,
    }
    let mut columns: Vec<Column> = Vec::new();
    for (part_index, (part, multiplicity)) in parts.iter().enumerate() {
        let degree = poly_degree(part)?;
        for power in 1..=*multiplicity as usize {
            let powered = poly_pow(part, power as u32).ok()?;
            let (cofactor, remainder) = poly_div_mod(&monic, &powered).ok()?;
            if !poly_is_zero(&remainder) {
                return None;
            }
            for shift in 0..degree {
                let mut coeffs = vec![Rat::zero(); shift];
                coeffs.push(Rat::one());
                let monomial = Poly {
                    var: monic.var.clone(),
                    coeffs,
                };
                let poly = poly_mul(&monomial, &cofactor).ok()?;
                columns.push(Column {
                    part: part_index,
                    power,
                    shift,
                    poly,
                });
            }
        }
    }
    if columns.len() != total {
        return None;
    }
    let mut matrix = Vec::new();
    let mut rhs = Vec::new();
    for k in 0..total {
        matrix.push(
            columns
                .iter()
                .map(|c| c.poly.coeffs.get(k).cloned().unwrap_or_else(Rat::zero))
                .collect::<Vec<_>>(),
        );
        rhs.push(scaled.coeffs.get(k).cloned().unwrap_or_else(Rat::zero));
    }
    let solution = solve_linear_system(matrix, rhs)?;
    let mut terms = Vec::new();
    for (part_index, (part, multiplicity)) in parts.iter().enumerate() {
        let degree = poly_degree(part)?;
        for power in 1..=*multiplicity as usize {
            let mut numerator_coeffs = vec![Rat::zero(); degree];
            for (index, column) in columns.iter().enumerate() {
                if column.part == part_index && column.power == power {
                    numerator_coeffs[column.shift] = solution[index].clone();
                }
            }
            if numerator_coeffs.iter().all(|c| c.is_zero()) {
                continue;
            }
            terms.push(integrate_repeated_factor(
                part,
                &numerator_coeffs,
                power,
                var,
            )?);
        }
    }
    Some(add(terms))
}

fn solve_linear_system(mut a: Vec<Vec<Rat>>, b: Vec<Rat>) -> Option<Vec<Rat>> {
    let n = b.len();
    if n == 0 || a.len() != n || a.iter().any(|row| row.len() != n) {
        return None;
    }
    for (i, row) in a.iter_mut().enumerate() {
        row.push(b[i].clone());
    }
    for col in 0..n {
        let mut pivot = None;
        for row in col..n {
            if !a[row][col].is_zero() {
                pivot = Some(row);
                break;
            }
        }
        let pivot = pivot?;
        if pivot != col {
            a.swap(col, pivot);
        }
        let p = a[col][col].clone();
        for j in col..=n {
            a[col][j] = &a[col][j] / &p;
        }
        for row in 0..n {
            if row == col {
                continue;
            }
            let factor = a[row][col].clone();
            if factor.is_zero() {
                continue;
            }
            for j in col..=n {
                let value = &a[row][j] - &factor * &a[col][j];
                a[row][j] = value;
            }
        }
    }
    Some(a.into_iter().map(|row| row[n].clone()).collect())
}

/// 积分 `(分子) / (q^power)`，其中 `q` 是 square-free 的 monic 因子。
fn integrate_repeated_factor(q: &Poly, numerator: &[Rat], power: usize, var: &str) -> Option<Expr> {
    let degree = poly_degree(q)?;
    let q_expr = poly_to_expr(q);
    match degree {
        1 => {
            let ratio = q.coeffs.get(1)?;
            let coefficient = numerator.first().cloned().unwrap_or_else(Rat::zero) / ratio;
            if coefficient.is_zero() {
                return Some(zero());
            }
            if power == 1 {
                return Some(mul(vec![
                    num(coefficient),
                    call_expr("ln", vec![call_expr("abs", vec![q_expr])]),
                ]));
            }
            let exponent = 1 - power as i64;
            Some(div(
                mul(vec![num(coefficient), pow(q_expr, integer(exponent))]),
                integer(exponent),
            ))
        }
        2 => {
            let a = &q.coeffs[2];
            let b = &q.coeffs[1];
            let c = &q.coeffs[0];
            let linear = numerator.get(1).cloned().unwrap_or_else(Rat::zero);
            let constant = numerator.first().cloned().unwrap_or_else(Rat::zero);
            let alpha = linear.clone() / (a * rat(2, 1));
            let beta = constant - &linear * b / (a * rat(2, 1));
            let mut terms = Vec::new();
            if !alpha.is_zero() {
                if power == 1 {
                    terms.push(mul(vec![
                        num(alpha),
                        call_expr("ln", vec![call_expr("abs", vec![q_expr.clone()])]),
                    ]));
                } else {
                    let exponent = 1 - power as i64;
                    terms.push(div(
                        mul(vec![num(alpha), pow(q_expr.clone(), integer(exponent))]),
                        integer(exponent),
                    ));
                }
            }
            if !beta.is_zero() {
                let integral = integrate_quadratic_power(a, b, c, power, var)?;
                terms.push(mul(vec![num(beta), integral]));
            }
            Some(add(terms))
        }
        _ => None,
    }
}

/// ∫ dx / (a x^2 + b x + c)^power，`a` 非零、判别式不为零。
fn integrate_quadratic_power(a: &Rat, b: &Rat, c: &Rat, power: usize, var: &str) -> Option<Expr> {
    let x = x_of(var);
    let q_expr = poly_to_expr(&Poly {
        var: var.to_string(),
        coeffs: vec![c.clone(), b.clone(), a.clone()],
    });
    let delta = a * c * rat(4, 1) - b * b;
    if delta.is_zero() || power == 0 || !a.is_one() {
        return None;
    }
    if power == 1 {
        // 先化成首一形式，再套用标准公式。
        let bm = b / a;
        let cm = c / a;
        let delta_m = cm * rat(4, 1) - &bm * &bm;
        if delta_m.is_zero() {
            return None;
        }
        if delta_m.is_positive() {
            let sqrt_delta = pow(num(delta_m), num(rat(1, 2)));
            return Some(mul(vec![
                div(integer(2), sqrt_delta.clone()),
                call_expr(
                    "atan",
                    vec![div(
                        add(vec![mul(vec![integer(2), x]), num(bm)]),
                        sqrt_delta,
                    )],
                ),
            ]));
        }
        let disc = -delta_m;
        let sqrt_disc = pow(num(disc), num(rat(1, 2)));
        let linear = add(vec![mul(vec![integer(2), x]), num(bm)]);
        let ratio = div(
            sub(linear.clone(), sqrt_disc.clone()),
            add(vec![linear, sqrt_disc.clone()]),
        );
        return Some(div(
            call_expr("ln", vec![call_expr("abs", vec![ratio])]),
            sqrt_disc,
        ));
    }
    let j = power as i64;
    let previous = integrate_quadratic_power(a, b, c, power - 1, var)?;
    let first = div(
        add(vec![mul(vec![integer(2), x]), num(b.clone())]),
        mul(vec![
            integer(j - 1),
            num(delta.clone()),
            pow(q_expr, integer(j - 1)),
        ]),
    );
    let factor = div(num(rat(2 * (2 * j - 3), j - 1)), num(delta));
    Some(add(vec![first, mul(vec![factor, previous])]))
}

// ---------------------------------------------------------------------------
// 定积分
// ---------------------------------------------------------------------------

pub fn integrate_definite(expr: &Expr, var: &str, a: &Expr, b: &Expr) -> Expr {
    let antiderivative = integrate(expr, var);
    if !is_unevaluated(&antiderivative, var) {
        let upper = crate::symbolic::substitute(&antiderivative, var, b);
        let lower = crate::symbolic::substitute(&antiderivative, var, a);
        return simplify(&sub(upper, lower));
    }
    if let Some(numeric) = numeric_definite(expr, var, a, b) {
        return numeric;
    }
    unevaluated(expr, var)
}

fn numeric_definite(expr: &Expr, var: &str, a: &Expr, b: &Expr) -> Option<Expr> {
    let (Expr::Num(lo), Expr::Num(hi)) = (a, b) else {
        return None;
    };
    let lo = rat_to_f64(lo);
    let hi = rat_to_f64(hi);
    if !lo.is_finite() || !hi.is_finite() {
        return None;
    }
    let f = |x: f64| eval_numeric_f64(expr, var, x);
    let fa = f(lo);
    let fm = f((lo + hi) / 2.0);
    let fb = f(hi);
    if !fa.is_finite() || !fm.is_finite() || !fb.is_finite() {
        return None;
    }
    let value = simpson(f, lo, hi, fa, fm, fb, 0);
    if !value.is_finite() {
        return None;
    }
    BigFloat::from_f64(value, 15).map(Expr::Float)
}

fn simpson(
    f: impl Fn(f64) -> f64 + Copy,
    a: f64,
    b: f64,
    fa: f64,
    fm: f64,
    fb: f64,
    depth: usize,
) -> f64 {
    if depth > 40 {
        return (b - a) / 6.0 * (fa + 4.0 * fm + fb);
    }
    let mid = (a + b) / 2.0;
    let lm = (a + mid) / 2.0;
    let rm = (mid + b) / 2.0;
    let flm = f(lm);
    let frm = f(rm);
    if !flm.is_finite() || !frm.is_finite() {
        return f64::NAN;
    }
    let whole = (b - a) / 6.0 * (fa + 4.0 * fm + fb);
    let half = (b - a) / 12.0 * (fa + 4.0 * flm + 2.0 * fm + 4.0 * frm + fb);
    if (half - whole).abs() < 1e-12 * half.abs().max(1.0) {
        half
    } else {
        simpson(f, a, mid, fa, flm, fm, depth + 1) + simpson(f, mid, b, fm, frm, fb, depth + 1)
    }
}

fn eval_numeric_f64(expr: &Expr, var: &str, x: f64) -> f64 {
    match expr {
        Expr::Num(v) => rat_to_f64(v),
        Expr::Float(v) => v.to_f64(),
        Expr::Sym(name) => match name.as_str() {
            n if n == var => x,
            "pi" => std::f64::consts::PI,
            "e" => std::f64::consts::E,
            _ => f64::NAN,
        },
        Expr::Add(terms) => terms.iter().map(|t| eval_numeric_f64(t, var, x)).sum(),
        Expr::Mul(factors) => factors
            .iter()
            .fold(1.0, |p, f| p * eval_numeric_f64(f, var, x)),
        Expr::Pow(base, exp) => eval_numeric_f64(base, var, x).powf(eval_numeric_f64(exp, var, x)),
        Expr::Call(name, args) => {
            let values: Vec<f64> = args.iter().map(|a| eval_numeric_f64(a, var, x)).collect();
            let first = values.first().copied().unwrap_or(f64::NAN);
            match name.to_lowercase().as_str() {
                "sin" => first.sin(),
                "cos" => first.cos(),
                "tan" => first.tan(),
                "asin" => first.asin(),
                "acos" => first.acos(),
                "atan" => first.atan(),
                "sinh" => first.sinh(),
                "cosh" => first.cosh(),
                "tanh" => first.tanh(),
                "exp" => first.exp(),
                "ln" => first.ln(),
                "log10" => first.log10(),
                "log2" => first.log2(),
                "sqrt" => first.sqrt(),
                "abs" => first.abs(),
                _ => f64::NAN,
            }
        }
        _ => f64::NAN,
    }
}
