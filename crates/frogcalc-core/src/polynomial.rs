use std::collections::BTreeSet;

use std::ops::Neg;

use num_bigint::BigInt;
use num_traits::{One, Signed, ToPrimitive, Zero};

use crate::error::{CalcError, CalcResult};
use crate::factor_int::factor_squarefree_integer;
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
    // 先做因式分解：一次/二次因子给出精确根，剩下的高次因子才退回数值求根。
    let mut roots = Vec::new();
    match factor_poly(&p) {
        Ok(result) if !result.factors.is_empty() => {
            for (factor, _) in &result.factors {
                match poly_degree(factor) {
                    Some(0) | None => {}
                    Some(1) => roots.push(num(-&factor.coeffs[0] / &factor.coeffs[1])),
                    Some(2) => roots.extend(quadratic_roots(factor)),
                    Some(_) => roots.extend(numeric_roots(factor)),
                }
            }
        }
        _ => roots.extend(numeric_roots(&p)),
    }
    if roots.is_empty() {
        roots.extend(numeric_roots(&p));
    }
    Ok(dedupe(roots))
}

pub struct FactorResult {
    pub content: Rat,
    pub factors: Vec<(Poly, u32)>,
}

pub fn factor_poly(poly: &Poly) -> CalcResult<FactorResult> {
    let original = poly_trim(poly);
    if poly_is_zero(&original) {
        return Ok(FactorResult {
            content: Rat::one(),
            factors: Vec::new(),
        });
    }
    let content = original
        .coeffs
        .iter()
        .fold(Rat::zero(), |g, c| rational_gcd(&g, c));
    let content = if content.is_zero() {
        Rat::one()
    } else {
        content.abs()
    };
    let scaled = poly_scale(&original, &content.recip());
    let mut factors: Vec<(Poly, u32)> = Vec::new();
    for (part, multiplicity) in squarefree_parts(&scaled) {
        let mut remaining = part;
        let (root_factors, rest) = extract_rational_roots(&remaining);
        factors.extend(root_factors.into_iter().map(|f| (f, multiplicity)));
        remaining = rest;
        if poly_degree(&remaining).unwrap_or(0) <= 0 {
            continue;
        }
        let integer = match poly_to_integers(&remaining) {
            Some(value) => value,
            None => {
                factors.push((remaining, multiplicity));
                continue;
            }
        };
        match factor_squarefree_integer(&integer) {
            Some(parts) if !parts.is_empty() => {
                for factor in parts {
                    let poly = Poly {
                        var: remaining.var.clone(),
                        coeffs: factor
                            .iter()
                            .map(|c| Rat::from_integer(c.clone()))
                            .collect(),
                    };
                    factors.push((poly_trim(&poly), multiplicity));
                }
            }
            _ => factors.push((remaining, multiplicity)),
        }
    }
    factors.retain(|(f, m)| poly_degree(f).unwrap_or(0) > 0 && *m > 0);
    if factors.is_empty() {
        return Ok(FactorResult {
            content,
            factors: Vec::new(),
        });
    }
    let mut product = poly_constant(&original.var, Rat::one());
    for (factor, multiplicity) in &factors {
        for _ in 0..*multiplicity {
            product = poly_mul(&product, factor)?;
        }
    }
    let (quotient, remainder) = poly_div_mod(&scaled, &product)?;
    let constant = if poly_is_zero(&remainder) && poly_degree(&quotient).unwrap_or(0) == 0 {
        quotient.coeffs.first().cloned().unwrap_or_else(Rat::one)
    } else {
        // 结果无法验证时退回“不分解”，保证正确性优先。
        return Ok(FactorResult {
            content,
            factors: vec![(scaled, 1)],
        });
    };
    Ok(FactorResult {
        content: content * constant,
        factors,
    })
}

/// Yun 算法：把一元多项式拆成 square-free 部分，附带重数。
pub fn squarefree_parts(poly: &Poly) -> Vec<(Poly, u32)> {
    let f = poly_monic(poly);
    if poly_degree(&f).unwrap_or(0) <= 0 {
        return Vec::new();
    }
    let derivative = poly_derivative(&f);
    let Ok(mut g) = poly_gcd(&f, &derivative) else {
        return vec![(f, 1)];
    };
    let Ok((mut w, _)) = poly_div_mod(&f, &g) else {
        return vec![(f, 1)];
    };
    let mut out = Vec::new();
    let mut multiplicity = 1u32;
    while poly_degree(&w).unwrap_or(0) > 0 {
        let Ok(y) = poly_gcd(&w, &g) else { break };
        let Ok((z, _)) = poly_div_mod(&w, &y) else {
            break;
        };
        if poly_degree(&z).unwrap_or(0) > 0 {
            out.push((poly_monic(&z), multiplicity));
        }
        w = y.clone();
        g = match poly_div_mod(&g, &y) {
            Ok((quotient, _)) => quotient,
            _ => break,
        };
        multiplicity += 1;
        if multiplicity > 200 {
            break;
        }
    }
    if out.is_empty() {
        out.push((f, 1));
    }
    out
}

fn extract_rational_roots(poly: &Poly) -> (Vec<Poly>, Poly) {
    let mut remaining = poly_trim(poly);
    let mut factors = Vec::new();
    for root in rational_roots(&remaining) {
        let factor = Poly {
            var: remaining.var.clone(),
            coeffs: vec![-root, Rat::one()],
        };
        loop {
            let Ok((quotient, remainder)) = poly_div_mod(&remaining, &factor) else {
                break;
            };
            if !poly_is_zero(&remainder) {
                break;
            }
            factors.push(factor.clone());
            remaining = poly_trim(&quotient);
        }
    }
    (factors, remaining)
}

fn poly_to_integers(poly: &Poly) -> Option<Vec<BigInt>> {
    use num_integer::Integer;
    let mut denominator_lcm = BigInt::one();
    for coeff in &poly.coeffs {
        denominator_lcm = denominator_lcm.lcm(coeff.denom());
    }
    let mut out: Vec<BigInt> = poly
        .coeffs
        .iter()
        .map(|coeff| {
            let scaled = coeff * Rat::from_integer(denominator_lcm.clone());
            scaled.numer().clone()
        })
        .collect();
    let mut gcd = BigInt::zero();
    for coeff in &out {
        gcd = gcd.gcd(coeff);
    }
    if !gcd.is_zero() && !gcd.is_one() {
        for coeff in out.iter_mut() {
            *coeff /= &gcd;
        }
    }
    if out.last().map(|c| c.is_negative()).unwrap_or(false) {
        for coeff in out.iter_mut() {
            *coeff = -coeff.clone();
        }
    }
    while out.last().map(|c| c.is_zero()).unwrap_or(false) {
        out.pop();
    }
    Some(out)
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
