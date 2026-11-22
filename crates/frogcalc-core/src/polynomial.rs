use std::collections::BTreeSet;

use std::ops::Neg;

use num_bigint::BigInt;
use num_traits::{One, Signed, ToPrimitive, Zero};

use crate::error::{CalcError, CalcResult};
use crate::numeric::{rat, rat_to_f64, rational_gcd, BigFloat, Rat};
use crate::symbolic::{add, integer, key, mul, num, pow, sub, sym, Expr};

const MAX_DEGREE: usize = 500;

#[derive(Clone, Debug)]
pub struct Poly {
    pub var: String,
    pub coeffs: Vec<Rat>,
}

pub fn poly_zero(var: &str) -> Poly {
    Poly {
        var: var.to_string(),
        coeffs: Vec::new(),
    }
}

pub fn poly_constant(var: &str, value: Rat) -> Poly {
    if value.is_zero() {
        poly_zero(var)
    } else {
        Poly {
            var: var.to_string(),
            coeffs: vec![value],
        }
    }
}

pub fn poly_trim(poly: &Poly) -> Poly {
    let mut coeffs = poly.coeffs.clone();
    while coeffs.last().map(|v| v.is_zero()).unwrap_or(false) {
        coeffs.pop();
    }
    Poly {
        var: poly.var.clone(),
        coeffs,
    }
}

pub fn poly_is_zero(poly: &Poly) -> bool {
    poly.coeffs.iter().all(|v| v.is_zero())
}

pub fn poly_degree(poly: &Poly) -> Option<usize> {
    if poly_is_zero(poly) {
        None
    } else {
        Some(poly.coeffs.len() - 1)
    }
}

pub fn poly_add(a: &Poly, b: &Poly) -> Poly {
    let n = a.coeffs.len().max(b.coeffs.len());
    let mut coeffs = Vec::with_capacity(n);
    for i in 0..n {
        let x = a.coeffs.get(i).cloned().unwrap_or_else(Rat::zero);
        let y = b.coeffs.get(i).cloned().unwrap_or_else(Rat::zero);
        coeffs.push(x + y);
    }
    poly_trim(&Poly {
        var: a.var.clone(),
        coeffs,
    })
}

pub fn poly_scale(a: &Poly, k: &Rat) -> Poly {
    if k.is_zero() {
        return poly_zero(&a.var);
    }
    Poly {
        var: a.var.clone(),
        coeffs: a.coeffs.iter().map(|v| v * k).collect(),
    }
}

pub fn poly_sub(a: &Poly, b: &Poly) -> Poly {
    poly_add(a, &poly_scale(b, &rat(-1, 1)))
}

pub fn poly_mul(a: &Poly, b: &Poly) -> CalcResult<Poly> {
    if poly_is_zero(a) || poly_is_zero(b) {
        return Ok(poly_zero(&a.var));
    }
    if a.coeffs.len() + b.coeffs.len() - 1 > MAX_DEGREE + 1 {
        return Err(CalcError::new("多项式次数过高"));
    }
    let mut coeffs = vec![Rat::zero(); a.coeffs.len() + b.coeffs.len() - 1];
    for (i, x) in a.coeffs.iter().enumerate() {
        for (j, y) in b.coeffs.iter().enumerate() {
            coeffs[i + j] += x * y;
        }
    }
    Ok(poly_trim(&Poly {
        var: a.var.clone(),
        coeffs,
    }))
}

pub fn poly_pow(a: &Poly, exp: u32) -> CalcResult<Poly> {
    if exp > 100 {
        return Err(CalcError::new("多项式指数过大"));
    }
    let mut result = poly_constant(&a.var, Rat::one());
    let mut base = a.clone();
    let mut e = exp;
    while e > 0 {
        if e & 1 == 1 {
            result = poly_mul(&result, &base)?;
        }
        e >>= 1;
        if e > 0 {
            base = poly_mul(&base, &base)?;
        }
    }
    Ok(result)
}

pub fn poly_div_mod(a: &Poly, b: &Poly) -> CalcResult<(Poly, Poly)> {
    if poly_is_zero(b) {
        return Err(CalcError::new("多项式除法：除式不能为零"));
    }
    let mut r = poly_trim(a);
    let db = poly_degree(b).unwrap();
    let lb = &b.coeffs[db];
    let mut q: Vec<Rat> = Vec::new();
    while let Some(dr) = poly_degree(&r) {
        if dr < db {
            break;
        }
        let k = &r.coeffs[dr] / lb;
        let shift = dr - db;
        while q.len() <= shift {
            q.push(Rat::zero());
        }
        q[shift] += &k;
        let mut sub_coeffs = vec![Rat::zero(); shift];
        for i in 0..=db {
            sub_coeffs.push(&b.coeffs[i] * &k);
        }
        r = poly_sub(
            &r,
            &Poly {
                var: r.var.clone(),
                coeffs: sub_coeffs,
            },
        );
    }
    while q.last().map(|v| v.is_zero()).unwrap_or(false) {
        q.pop();
    }
    Ok((
        Poly {
            var: a.var.clone(),
            coeffs: q,
        },
        r,
    ))
}

pub fn poly_derivative(a: &Poly) -> Poly {
    if a.coeffs.len() <= 1 {
        return poly_zero(&a.var);
    }
    let mut coeffs = Vec::new();
    for i in 1..a.coeffs.len() {
        coeffs.push(&a.coeffs[i] * Rat::from_integer(BigInt::from(i)));
    }
    poly_trim(&Poly {
        var: a.var.clone(),
        coeffs,
    })
}

pub fn poly_eval(a: &Poly, x: &Rat) -> Rat {
    let mut result = Rat::zero();
    for coeff in a.coeffs.iter().rev() {
        result = result * x + coeff;
    }
    result
}

pub fn poly_monic(a: &Poly) -> Poly {
    match poly_degree(a) {
        None => a.clone(),
        Some(d) => poly_scale(a, &a.coeffs[d].recip()),
    }
}

pub fn poly_major_coeff(a: &Poly) -> Option<Rat> {
    poly_degree(a).map(|d| a.coeffs[d].clone())
}

pub fn poly_gcd(a: &Poly, b: &Poly) -> CalcResult<Poly> {
    let mut x = poly_trim(a);
    let mut y = poly_trim(b);
    while !poly_is_zero(&y) {
        let (_, r) = poly_div_mod(&x, &y)?;
        x = y;
        y = r;
    }
    Ok(poly_monic(&x))
}

pub fn poly_to_expr(poly: &Poly) -> Expr {
    let x = sym(&poly.var);
    let mut terms = Vec::new();
    for (i, coeff) in poly.coeffs.iter().enumerate() {
        if coeff.is_zero() {
            continue;
        }
        let term = if i == 0 {
            num(coeff.clone())
        } else if i == 1 {
            mul(vec![num(coeff.clone()), x.clone()])
        } else {
            mul(vec![num(coeff.clone()), pow(x.clone(), integer(i as i64))])
        };
        terms.push(term);
    }
    add(terms)
}

pub fn poly_from_expr(expr: &Expr, var: &str) -> Option<Poly> {
    fn visit(expr: &Expr, var: &str) -> Option<Poly> {
        match expr {
            Expr::Num(v) => Some(poly_constant(var, v.clone())),
            Expr::Sym(name) if name == var => Some(Poly {
                var: var.to_string(),
                coeffs: vec![Rat::zero(), Rat::one()],
            }),
            Expr::Add(terms) => {
                let mut acc = poly_zero(var);
                for term in terms {
                    acc = poly_add(&acc, &visit(term, var)?);
                }
                Some(acc)
            }
            Expr::Mul(factors) => {
                let mut acc = poly_constant(var, Rat::one());
                for factor in factors {
                    acc = poly_mul(&acc, &visit(factor, var)?).ok()?;
                }
                Some(acc)
            }
            Expr::Pow(base, exp) => {
                let e = match &**exp {
                    Expr::Num(v) if v.is_integer() => v.numer().to_u32()?,
                    _ => return None,
                };
                if e > 100 {
                    return None;
                }
                let base = visit(base, var)?;
                poly_pow(&base, e).ok()
            }
            _ => None,
        }
    }
    let p = visit(expr, var)?;
    Some(poly_trim(&p))
}

fn divisors(n: &BigInt) -> Vec<BigInt> {
    let abs = n.abs();
    if abs.is_zero() {
        return vec![BigInt::zero()];
    }
    let mut out = BTreeSet::new();
    let mut i = BigInt::one();
    let limit = BigInt::from(1_000_000u32);
    while &i * &i <= abs && i <= limit {
        if (&abs % &i).is_zero() {
            out.insert(i.clone());
            out.insert(&abs / &i);
        }
        i += 1u32;
    }
    out.insert(BigInt::one());
    out.insert(abs);
    out.into_iter().collect()
}

pub fn rational_roots(poly: &Poly) -> Vec<Rat> {
    let p = poly_trim(poly);
    let Some(deg) = poly_degree(&p) else {
        return Vec::new();
    };
    if deg == 0 {
        return Vec::new();
    }
    let c0 = &p.coeffs[0];
    if c0.is_zero() {
        let rest = Poly {
            var: p.var.clone(),
            coeffs: p.coeffs[1..].to_vec(),
        };
        let mut roots = vec![Rat::zero()];
        roots.extend(rational_roots(&rest));
        return roots;
    }
    let leading = &p.coeffs[deg];
    let ps = divisors(c0.numer());
    let qs = divisors(leading.numer());
    let mut seen = BTreeSet::new();
    let mut roots = Vec::new();
    for a in &ps {
        for b in &qs {
            for sign in [1i32, -1] {
                if b.is_zero() {
                    continue;
                }
                let mut candidate = Rat::new(a.clone(), b.clone());
                if sign < 0 {
                    candidate = -candidate;
                }
                let key = candidate.to_string();
                if !seen.insert(key) {
                    continue;
                }
                if poly_eval(&p, &candidate).is_zero() {
                    roots.push(candidate);
                }
            }
        }
    }
    roots
}

fn sqrt_expr(value: &Rat) -> Expr {
    if value.is_zero() {
        num(Rat::zero())
    } else {
        pow(num(value.clone()), num(rat(1, 2)))
    }
}

fn quadratic_roots(p: &Poly) -> Vec<Expr> {
    let a = &p.coeffs[2];
    let b = &p.coeffs[1];
    let c = &p.coeffs[0];
    let disc = b * b - a * c * rat(4, 1);
    let denom = a * rat(2, 1);
    if disc.is_zero() {
        return vec![num(-b / &denom)];
    }
    if disc.is_positive() {
        let s = sqrt_expr(&disc);
        let real = num(-b / &denom);
        let term = mul(vec![s, num(denom.recip())]);
        vec![add(vec![real.clone(), term.clone()]), sub(real, term)]
    } else {
        let s = sqrt_expr(&disc.neg());
        let real = num(-b / &denom);
        let imag = mul(vec![s, num(denom.recip()), sym("i")]);
        vec![add(vec![real.clone(), imag.clone()]), sub(real, imag)]
    }
}

fn dedupe(items: Vec<Expr>) -> Vec<Expr> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for item in items {
        let k = key(&item);
        if seen.insert(k) {
            out.push(item);
        }
    }
    out
}

#[derive(Clone, Copy, Debug)]
struct Cx {
    re: f64,
    im: f64,
}

impl Cx {
    fn add(self, other: Cx) -> Cx {
        Cx {
            re: self.re + other.re,
            im: self.im + other.im,
        }
    }
    fn sub(self, other: Cx) -> Cx {
        Cx {
            re: self.re - other.re,
            im: self.im - other.im,
        }
    }
    fn mul(self, other: Cx) -> Cx {
        Cx {
            re: self.re * other.re - self.im * other.im,
            im: self.re * other.im + self.im * other.re,
        }
    }
    fn div(self, other: Cx) -> Cx {
        let d = other.re * other.re + other.im * other.im;
        Cx {
            re: (self.re * other.re + self.im * other.im) / d,
            im: (self.im * other.re - self.re * other.im) / d,
        }
    }
    fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }
}

fn numeric_roots(p: &Poly) -> Vec<Expr> {
    let Some(degree) = poly_degree(p) else {
        return Vec::new();
    };
    if degree == 0 {
        return Vec::new();
    }
    let mut coeffs: Vec<f64> = p.coeffs.iter().map(rat_to_f64).collect();
    let lead = coeffs[degree];
    if lead == 0.0 || !lead.is_finite() {
        return Vec::new();
    }
    for c in &mut coeffs {
        *c /= lead;
    }
    let mut roots = Vec::with_capacity(degree);
    for i in 0..degree {
        let angle = 2.0 * std::f64::consts::PI * i as f64 / degree as f64 + 0.35;
        roots.push(Cx {
            re: 0.9 * angle.cos(),
            im: 0.9 * angle.sin(),
        });
    }
    let evaluate = |z: Cx| {
        let mut result = Cx { re: 0.0, im: 0.0 };
        for i in (0..=degree).rev() {
            result = Cx {
                re: coeffs[i],
                im: 0.0,
            }
            .add(result.mul(z));
        }
        result
    };
    for _ in 0..3000 {
        let mut max_delta: f64 = 0.0;
        for i in 0..degree {
            let mut denom = Cx { re: 1.0, im: 0.0 };
            for j in 0..degree {
                if i != j {
                    denom = denom.mul(roots[i].sub(roots[j]));
                }
            }
            if denom.abs() < 1e-24 {
                continue;
            }
            let delta = evaluate(roots[i]).div(denom);
            roots[i] = roots[i].sub(delta);
            max_delta = max_delta.max(delta.abs());
        }
        if max_delta < 1e-14 {
            break;
        }
    }
    roots
        .into_iter()
        .map(|root| {
            if root.im.abs() < 1e-9 {
                Expr::Float(BigFloat::from_f64(root.re, 17).unwrap_or_else(|| BigFloat::zero(17)))
            } else {
                let re = Expr::Float(
                    BigFloat::from_f64(root.re, 17).unwrap_or_else(|| BigFloat::zero(17)),
                );
                let im = Expr::Float(
                    BigFloat::from_f64(root.im.abs(), 17).unwrap_or_else(|| BigFloat::zero(17)),
                );
                let imag = mul(vec![im, sym("i")]);
                if root.im >= 0.0 {
                    add(vec![re, imag])
                } else {
                    sub(re, imag)
                }
            }
        })
        .collect()
}

pub fn solve_poly(poly: &Poly) -> CalcResult<Vec<Expr>> {
    let p = poly_trim(poly);
    let Some(degree) = poly_degree(&p) else {
        return Ok(Vec::new());
    };
    if degree == 0 {
        return Ok(Vec::new());
    }
    if degree == 1 {
        return Ok(vec![num(-&p.coeffs[0] / &p.coeffs[1])]);
    }
    if degree == 2 {
        return Ok(quadratic_roots(&p));
    }
    let mut current = p.clone();
    let mut exact = Vec::new();
    for root in rational_roots(&p) {
        if poly_degree(&current).unwrap_or(0) == 0 {
            break;
        }
        let factor = Poly {
            var: p.var.clone(),
            coeffs: vec![-root.clone(), Rat::one()],
        };
        if let Ok((q, r)) = poly_div_mod(&current, &factor) {
            if poly_is_zero(&r) {
                exact.push(num(root));
                current = poly_trim(&q);
            }
        }
    }
    match poly_degree(&current) {
        None => Ok(dedupe(exact)),
        Some(0) => Ok(dedupe(exact)),
        Some(1) => {
            exact.push(num(-&current.coeffs[0] / &current.coeffs[1]));
            Ok(dedupe(exact))
        }
        Some(2) => {
            exact.extend(quadratic_roots(&current));
            Ok(dedupe(exact))
        }
        _ => {
            exact.extend(numeric_roots(&current));
            Ok(dedupe(exact))
        }
    }
}

pub struct FactorResult {
    pub content: Rat,
    pub factors: Vec<(Poly, u32)>,
}

pub fn factor_poly(poly: &Poly) -> CalcResult<FactorResult> {
    let mut p = poly_trim(poly);
    if poly_is_zero(&p) {
        return Ok(FactorResult {
            content: Rat::one(),
            factors: Vec::new(),
        });
    }
    let content = p
        .coeffs
        .iter()
        .fold(Rat::zero(), |g, c| rational_gcd(&g, c));
    let content = if content.is_zero() {
        Rat::one()
    } else {
        content.abs()
    };
    p = poly_scale(&p, &content.recip());
    let mut factors = Vec::new();
    for root in rational_roots(&p) {
        let factor = Poly {
            var: p.var.clone(),
            coeffs: vec![-root, Rat::one()],
        };
        let mut mult = 0;
        while let Ok((q, r)) = poly_div_mod(&p, &factor) {
            if !poly_is_zero(&r) {
                break;
            }
            p = poly_trim(&q);
            mult += 1;
        }
        if mult > 0 {
            factors.push((factor, mult));
        }
    }
    if poly_degree(&p).unwrap_or(0) > 0 {
        factors.push((p, 1));
    }
    Ok(FactorResult { content, factors })
}

pub fn numeric_square_root_rational(value: &Rat) -> Option<Rat> {
    if value.is_negative() {
        return None;
    }
    let n = value.numer().sqrt();
    let d = value.denom().sqrt();
    if &n * &n == *value.numer() && &d * &d == *value.denom() {
        Some(Rat::new(n, d))
    } else {
        None
    }
}
