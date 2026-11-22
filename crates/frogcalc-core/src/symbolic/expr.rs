use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::OnceLock;

use num_bigint::{BigInt, BigUint, Sign};
use num_integer::Integer;
use num_traits::{One, Signed, ToPrimitive, Zero};

use crate::numeric::{
    bigint_pow, rat, rat_display, rat_int, rat_pow_int, rat_to_f64, rational_gcd, BigFloat, Rat,
};

use crate::numeric::bigfloat::bf_pi;

#[derive(Clone, Debug)]
pub enum Expr {
    Num(Rat),
    Float(BigFloat),
    Sym(String),
    Str(String),
    Add(Vec<Expr>),
    Mul(Vec<Expr>),
    Pow(Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Eq(Box<Expr>, Box<Expr>),
    List(Vec<Expr>),
}

pub fn num(value: Rat) -> Expr {
    Expr::Num(value)
}

pub fn integer(value: i64) -> Expr {
    Expr::Num(rat_int(BigInt::from(value)))
}

pub fn rational(n: i64, d: i64) -> Expr {
    Expr::Num(rat(n, d))
}

pub fn sym(name: &str) -> Expr {
    Expr::Sym(name.to_string())
}

pub fn str_expr(value: &str) -> Expr {
    Expr::Str(value.to_string())
}

pub fn one() -> Expr {
    Expr::Num(Rat::one())
}

pub fn zero() -> Expr {
    Expr::Num(Rat::zero())
}

pub fn list(items: Vec<Expr>) -> Expr {
    Expr::List(items)
}

pub fn eq_expr(lhs: Expr, rhs: Expr) -> Expr {
    Expr::Eq(Box::new(lhs), Box::new(rhs))
}

pub fn raw_pow(base: Expr, exp: Expr) -> Expr {
    Expr::Pow(Box::new(base), Box::new(exp))
}

pub fn is_zero(expr: &Expr) -> bool {
    matches!(expr, Expr::Num(v) if v.is_zero())
}

pub fn is_one(expr: &Expr) -> bool {
    matches!(expr, Expr::Num(v) if v.is_one())
}

pub fn key(expr: &Expr) -> String {
    match expr {
        Expr::Num(v) => format!("n:{}", v),
        Expr::Float(v) => format!("f:{v}"),
        Expr::Sym(name) => format!("s:{name}"),
        Expr::Str(text) => format!("t:{text}"),
        Expr::Add(terms) => format!("a({})", terms.iter().map(key).collect::<Vec<_>>().join(",")),
        Expr::Mul(factors) => format!(
            "m({})",
            factors.iter().map(key).collect::<Vec<_>>().join(",")
        ),
        Expr::Pow(base, exp) => format!("p({},{})", key(base), key(exp)),
        Expr::Call(name, args) => format!(
            "c:{name}({})",
            args.iter().map(key).collect::<Vec<_>>().join(",")
        ),
        Expr::Eq(lhs, rhs) => format!("e({},{})", key(lhs), key(rhs)),
        Expr::List(items) => format!("l:{}", items.iter().map(key).collect::<Vec<_>>().join("|")),
    }
}

pub fn equals(a: &Expr, b: &Expr) -> bool {
    key(a) == key(b)
}

fn degree_of(expr: &Expr) -> i64 {
    match expr {
        Expr::Num(_) | Expr::Float(_) | Expr::Str(_) => 0,
        Expr::Sym(name) => {
            if matches!(name.as_str(), "pi" | "e" | "i") {
                0
            } else {
                1
            }
        }
        Expr::Pow(base, exp) => match &**exp {
            Expr::Num(v) if v.is_integer() => v
                .numer()
                .to_i64()
                .map(|e| e.unsigned_abs() as i64 * degree_of(base))
                .unwrap_or(0),
            _ => 0,
        },
        Expr::Mul(factors) => factors.iter().map(degree_of).sum(),
        Expr::Add(terms) => terms.iter().map(degree_of).max().unwrap_or(0),
        _ => 0,
    }
}

fn factor_rank(expr: &Expr) -> u8 {
    match expr {
        Expr::Sym(_) => 0,
        Expr::Pow(_, _) => 1,
        Expr::Call(_, _) => 2,
        Expr::Add(_) => 3,
        _ => 4,
    }
}

fn extract_coefficient(term: &Expr) -> (Rat, Expr) {
    match term {
        Expr::Num(v) => (v.clone(), one()),
        Expr::Mul(factors) => {
            let mut coeff = Rat::one();
            let mut rest = Vec::new();
            for factor in factors {
                if let Expr::Num(v) = factor {
                    coeff *= v;
                } else {
                    rest.push(factor.clone());
                }
            }
            let rest_expr = match rest.len() {
                0 => one(),
                1 => rest.pop().unwrap(),
                _ => mul(rest),
            };
            (coeff, rest_expr)
        }
        _ => (Rat::one(), term.clone()),
    }
}

pub fn add(terms: Vec<Expr>) -> Expr {
    let mut groups: HashMap<String, (Rat, Expr)> = HashMap::new();
    let mut constant = Rat::zero();
    fn collect(term: Expr, groups: &mut HashMap<String, (Rat, Expr)>, constant: &mut Rat) {
        match term {
            Expr::Add(inner) => {
                for t in inner {
                    collect(t, groups, constant);
                }
            }
            Expr::Num(v) => *constant += v,
            other => {
                let (coeff, rest) = extract_coefficient(&other);
                let k = key(&rest);
                if let Some(entry) = groups.get_mut(&k) {
                    entry.0 += coeff;
                } else {
                    groups.insert(k, (coeff, rest));
                }
            }
        }
    }
    for term in terms {
        collect(term, &mut groups, &mut constant);
    }
    let mut out: Vec<Expr> = groups
        .into_values()
        .filter(|(c, _)| !c.is_zero())
        .map(|(c, rest)| {
            if c.is_one() {
                rest
            } else {
                mul(vec![num(c), rest])
            }
        })
        .collect();
    out.sort_by(|a, b| {
        let da = degree_of(a);
        let db = degree_of(b);
        db.cmp(&da).then_with(|| key(a).cmp(&key(b)))
    });
    if !constant.is_zero() {
        out.push(num(constant));
    }
    match out.len() {
        0 => zero(),
        1 => out.pop().unwrap(),
        _ => Expr::Add(out),
    }
}

pub fn mul(factors: Vec<Expr>) -> Expr {
    let mut coeff = Rat::one();
    let mut powers: HashMap<String, (Expr, Expr)> = HashMap::new();
    fn collect(factor: Expr, coeff: &mut Rat, powers: &mut HashMap<String, (Expr, Expr)>) {
        match factor {
            Expr::Num(v) => *coeff *= v,
            Expr::Mul(inner) => {
                for f in inner {
                    collect(f, coeff, powers);
                }
            }
            Expr::Pow(base, exp) => {
                let k = key(&base);
                if let Some(entry) = powers.get_mut(&k) {
                    entry.1 = add(vec![entry.1.clone(), *exp]);
                } else {
                    powers.insert(k, (*base, *exp));
                }
            }
            other => {
                let k = key(&other);
                if let Some(entry) = powers.get_mut(&k) {
                    entry.1 = add(vec![entry.1.clone(), one()]);
                } else {
                    powers.insert(k, (other, one()));
                }
            }
        }
    }
    for factor in factors {
        collect(factor, &mut coeff, &mut powers);
    }
    if coeff.is_zero() {
        return zero();
    }
    let mut out: Vec<Expr> = powers
        .into_values()
        .filter_map(|(base, exp)| {
            if matches!(&exp, Expr::Num(v) if v.is_zero()) {
                None
            } else if matches!(&exp, Expr::Num(v) if v.is_one()) {
                Some(base)
            } else {
                Some(pow(base, exp))
            }
        })
        .collect();
    out.sort_by(|a, b| {
        factor_rank(a)
            .cmp(&factor_rank(b))
            .then_with(|| key(a).cmp(&key(b)))
    });
    if !coeff.is_one() {
        out.insert(0, num(coeff));
    }
    match out.len() {
        0 => one(),
        1 => out.pop().unwrap(),
        _ => Expr::Mul(out),
    }
}

pub fn neg(expr: Expr) -> Expr {
    mul(vec![integer(-1), expr])
}

pub fn sub(a: Expr, b: Expr) -> Expr {
    add(vec![a, neg(b)])
}

pub fn div(a: Expr, b: Expr) -> Expr {
    mul(vec![a, pow(b, integer(-1))])
}

fn i_power(exp: &BigInt) -> Expr {
    let mut e = exp.mod_floor(&BigInt::from(4u32)).to_i64().unwrap_or(0);
    if e < 0 {
        e += 4;
    }
    match e {
        0 => one(),
        1 => sym("i"),
        2 => integer(-1),
        _ => neg(sym("i")),
    }
}

fn small_primes() -> &'static [u32] {
    static PRIMES: OnceLock<Vec<u32>> = OnceLock::new();
    PRIMES.get_or_init(|| {
        let mut prims = Vec::new();
        for n in 2..=10_000u32 {
            if prims.iter().all(|p: &u32| n % *p != 0 || *p * *p > n) {
                prims.push(n);
            }
        }
        prims
    })
}

fn integer_nth_root(n: &BigUint, q: u32) -> (BigUint, bool) {
    if n.is_zero() {
        return (BigUint::zero(), true);
    }
    if q == 1 {
        return (n.clone(), true);
    }
    if q == 2 {
        let mut x = BigUint::one() << ((n.bits() as usize + 1) / 2 + 1);
        loop {
            let y = (&x + n / &x) >> 1u32;
            if y >= x {
                break;
            }
            x = y;
        }
        while &x * &x > *n {
            x -= 1u32;
        }
        while (&x + 1u32) * (&x + 1u32) <= *n {
            x += 1u32;
        }
        let exact = &x * &x == *n;
        return (x, exact);
    }
    let q_big = BigUint::from(q);
    let bits = n.bits() as usize;
    let mut x = BigUint::one() << ((bits + q as usize - 1) / q as usize + 1);
    loop {
        let x_pow = x.pow(q - 1);
        let y = ((&q_big - 1u32) * &x + n / &x_pow) / &q_big;
        if y >= x {
            break;
        }
        x = y;
    }
    while x.pow(q) > *n {
        x -= 1u32;
    }
    while (x.clone() + 1u32).pow(q) <= *n {
        x += 1u32;
    }
    let exact = x.pow(q) == *n;
    (x, exact)
}

fn extract_power(n: &BigUint, q: u32) -> (BigUint, BigUint) {
    if n <= &BigUint::one() {
        return (BigUint::one(), n.clone());
    }
    let (root, exact) = integer_nth_root(n, q);
    if exact {
        return (root, BigUint::one());
    }
    let mut rest = n.clone();
    let mut out = BigUint::one();
    let mut inside = BigUint::one();
    for &p in small_primes() {
        let p_big = BigUint::from(p);
        if &p_big * &p_big > rest {
            break;
        }
        let mut e = 0u32;
        while (&rest % &p_big).is_zero() {
            rest /= &p_big;
            e += 1;
        }
        let whole = e / q;
        let rem = e % q;
        if whole > 0 {
            out *= p_big.pow(whole);
        }
        if rem > 0 {
            inside *= p_big.pow(rem);
        }
    }
    inside *= rest;
    (out, inside)
}

fn simplify_rational_power(base: &Rat, exp: &Rat) -> Option<Expr> {
    let q = exp.denom().to_u32()?;
    let p = exp.numer().to_i64()?;
    if q > 1000 || p.unsigned_abs() > 100_000 {
        return None;
    }
    if base.is_zero() {
        return None;
    }
    if base.is_negative() {
        let mag = base.abs();
        if q % 2 == 1 {
            let mag_result = simplify_rational_power(&mag, exp)
                .unwrap_or_else(|| raw_pow(num(mag), num(exp.clone())));
            return Some(if p % 2 == 0 {
                mag_result
            } else {
                neg(mag_result)
            });
        }
        let mag_result = simplify_rational_power(&mag, exp)
            .unwrap_or_else(|| raw_pow(num(mag), num(exp.clone())));
        return Some(mul(vec![mag_result, i_power(&BigInt::from(p))]));
    }
    let (out_n, inside_n) = extract_power(base.numer().magnitude(), q);
    let (out_d, inside_d) = extract_power(base.denom().magnitude(), q);
    let out = Rat::new(
        BigInt::from_biguint(Sign::Plus, out_n),
        BigInt::from_biguint(Sign::Plus, out_d),
    );
    let inside = Rat::new(
        BigInt::from_biguint(Sign::Plus, inside_n),
        BigInt::from_biguint(Sign::Plus, inside_d),
    );
    let k = p.div_euclid(q as i64);
    let r = p.rem_euclid(q as i64);
    let mut coeff = rat_pow_int(&out, p)?;
    if k != 0 {
        coeff *= rat_pow_int(&inside, k)?;
    }
    let residual = if r == 0 || inside.is_one() {
        one()
    } else {
        raw_pow(num(inside.clone()), num(rat(r, q as i64)))
    };
    if coeff.is_one() && out.is_one() && inside == *base {
        return None;
    }
    Some(mul(vec![num(coeff), residual]))
}

pub fn pow(base: Expr, exp: Expr) -> Expr {
    if let Expr::Num(e) = &exp {
        if e.is_zero() {
            return one();
        }
        if e.is_one() {
            return base;
        }
        if let Expr::Num(b) = &base {
            if e.is_integer() {
                if let Some(n) = e.numer().to_i64() {
                    if let Some(value) = rat_pow_int(b, n) {
                        return num(value);
                    }
                }
            } else if let Some(simplified) = simplify_rational_power(b, e) {
                return simplified;
            }
        }
        if let Expr::Sym(name) = &base {
            if name == "i" && e.is_integer() {
                return i_power(e.numer());
            }
        }
        if let Expr::Pow(inner_base, inner_exp) = &base {
            let safe = e.is_integer()
                || matches!(&**inner_base, Expr::Num(v) if v.is_positive())
                || (matches!(&**inner_exp, Expr::Num(v) if v.is_integer()) && e.is_integer());
            if safe {
                return pow(
                    (**inner_base).clone(),
                    mul(vec![(**inner_exp).clone(), exp]),
                );
            }
        }
        if let Expr::Mul(factors) = &base {
            if let Some(n) = e.numer().to_i64() {
                if n.unsigned_abs() <= 100 {
                    return mul(factors
                        .iter()
                        .map(|f| pow(f.clone(), exp.clone()))
                        .collect());
                }
            }
        }
    }
    if let Expr::Num(v) = &base {
        if v.is_one() {
            return one();
        }
        if v.is_zero() {
            if let Expr::Num(e) = &exp {
                if e.is_positive() {
                    return zero();
                }
            }
        }
    }
    raw_pow(base, exp)
}

fn pi_coefficient(expr: &Expr) -> Option<Rat> {
    match expr {
        Expr::Num(v) if v.is_zero() => Some(Rat::zero()),
        Expr::Num(_) => None,
        Expr::Sym(name) if name == "pi" => Some(Rat::one()),
        Expr::Mul(factors) => {
            let mut coeff = Rat::one();
            let mut found = false;
            for factor in factors {
                match factor {
                    Expr::Num(v) => coeff *= v,
                    Expr::Sym(name) if name == "pi" && !found => found = true,
                    _ => return None,
                }
            }
            if found {
                Some(coeff)
            } else {
                None
            }
        }
        Expr::Add(terms) => {
            let mut sum = Rat::zero();
            let mut any = false;
            for term in terms {
                sum += pi_coefficient(term)?;
                any = true;
            }
            if any {
                Some(sum)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn trig_table() -> &'static (Vec<Expr>, Vec<Expr>) {
    static TABLE: OnceLock<(Vec<Expr>, Vec<Expr>)> = OnceLock::new();
    TABLE.get_or_init(|| {
        let q = |n: i64, d: i64| num(rat(n, d));
        let rad = |n: i64| raw_pow(num(rat_int(BigInt::from(n))), num(rat(1, 2)));
        let sqrt2 = rad(2);
        let sqrt3 = rad(3);
        let sqrt6 = rad(6);
        let mut sin = Vec::with_capacity(24);
        let mut cos = Vec::with_capacity(24);
        for _ in 0..24 {
            sin.push(zero());
            cos.push(zero());
        }
        sin[0] = zero();
        sin[1] = mul(vec![q(1, 4), add(vec![sqrt6.clone(), neg(sqrt2.clone())])]);
        sin[2] = q(1, 2);
        sin[3] = mul(vec![q(1, 2), sqrt2.clone()]);
        sin[4] = mul(vec![q(1, 2), sqrt3.clone()]);
        sin[5] = mul(vec![q(1, 4), add(vec![sqrt6.clone(), sqrt2.clone()])]);
        sin[6] = one();
        cos[0] = one();
        cos[1] = mul(vec![q(1, 4), add(vec![sqrt6.clone(), sqrt2.clone()])]);
        cos[2] = mul(vec![q(1, 2), sqrt3.clone()]);
        cos[3] = mul(vec![q(1, 2), sqrt2.clone()]);
        cos[4] = q(1, 2);
        cos[5] = mul(vec![q(1, 4), add(vec![sqrt6, neg(sqrt2)])]);
        cos[6] = zero();
        for n in 7..=11 {
            sin[n] = sin[12 - n].clone();
            cos[n] = neg(cos[12 - n].clone());
        }
        for n in 12..=23 {
            sin[n] = neg(sin[n - 12].clone());
            cos[n] = neg(cos[n - 12].clone());
        }
        (sin, cos)
    })
}

fn exact_trig(name: &str, arg: &Expr) -> Option<Expr> {
    let coeff = pi_coefficient(arg)?;
    let times = coeff * rat(12, 1);
    if !times.is_integer() {
        return None;
    }
    let n = times.numer().mod_floor(&BigInt::from(24u32)).to_i64()?;
    let n = if n < 0 { n + 24 } else { n };
    let (sin, cos) = trig_table();
    match name {
        "sin" => Some(sin[n as usize].clone()),
        "cos" => Some(cos[n as usize].clone()),
        "tan" => {
            let c = &cos[n as usize];
            if is_zero(c) {
                None
            } else {
                Some(div(sin[n as usize].clone(), c.clone()))
            }
        }
        _ => None,
    }
}

fn is_negative_mul(expr: &Expr) -> bool {
    matches!(expr, Expr::Mul(factors) if factors.iter().any(|f| matches!(f, Expr::Num(v) if v.is_negative())))
}

fn exact_asin(arg: &Expr) -> Option<Expr> {
    match arg {
        Expr::Num(v) if v.is_zero() => Some(zero()),
        Expr::Num(v) if v.is_one() => Some(mul(vec![num(rat(1, 2)), sym("pi")])),
        Expr::Num(v) if v == &rat(-1, 1) => Some(neg(mul(vec![num(rat(1, 2)), sym("pi")]))),
        Expr::Num(v) if v == &rat(1, 2) => Some(mul(vec![num(rat(1, 6)), sym("pi")])),
        Expr::Num(v) if v == &rat(-1, 2) => Some(neg(mul(vec![num(rat(1, 6)), sym("pi")]))),
        _ => {
            let half_sqrt2 = mul(vec![num(rat(1, 2)), raw_pow(integer(2), num(rat(1, 2)))]);
            let half_sqrt3 = mul(vec![num(rat(1, 2)), raw_pow(integer(3), num(rat(1, 2)))]);
            if equals(arg, &half_sqrt2) {
                Some(mul(vec![num(rat(1, 4)), sym("pi")]))
            } else if equals(arg, &half_sqrt3) {
                Some(mul(vec![num(rat(1, 3)), sym("pi")]))
            } else if equals(arg, &neg(half_sqrt2)) {
                Some(neg(mul(vec![num(rat(1, 4)), sym("pi")])))
            } else if equals(arg, &neg(half_sqrt3)) {
                Some(neg(mul(vec![num(rat(1, 3)), sym("pi")])))
            } else {
                None
            }
        }
    }
}

fn exact_atan(arg: &Expr) -> Option<Expr> {
    match arg {
        Expr::Num(v) if v.is_zero() => Some(zero()),
        Expr::Num(v) if v.is_one() => Some(mul(vec![num(rat(1, 4)), sym("pi")])),
        Expr::Num(v) if v == &rat(-1, 1) => Some(neg(mul(vec![num(rat(1, 4)), sym("pi")]))),
        _ => None,
    }
}

fn exact_abs(arg: Expr) -> Expr {
    match arg {
        Expr::Num(v) => num(v.abs()),
        Expr::Mul(mut factors) => {
            if let Some(pos) = factors
                .iter()
                .position(|f| matches!(f, Expr::Num(v) if v.is_negative()))
            {
                if let Expr::Num(v) = factors.remove(pos) {
                    factors.push(num(v.abs()));
                }
                return exact_abs(mul(factors));
            }
            Expr::Call("abs".into(), vec![mul(factors)])
        }
        other => Expr::Call("abs".into(), vec![other]),
    }
}

fn factorial_expr(n: &BigInt) -> Expr {
    if n.is_negative() || n > &BigInt::from(20_000u32) {
        return Expr::Call("factorial".into(), vec![num(rat_int(n.clone()))]);
    }
    let mut result = BigInt::one();
    let mut i = BigInt::from(2u32);
    while &i <= n {
        result *= &i;
        i += 1u32;
    }
    num(rat_int(result))
}

fn binomial_expr(n: &BigInt, k: &BigInt) -> Expr {
    if n.is_negative() || k.is_negative() || k > n || n > &BigInt::from(20_000u32) {
        return Expr::Call(
            "binomial".into(),
            vec![num(rat_int(n.clone())), num(rat_int(k.clone()))],
        );
    }
    let kk = std::cmp::min(k.clone(), n - k);
    let mut result = BigInt::one();
    let mut i = BigInt::one();
    while i <= kk {
        result = (result * (n - &kk + &i)) / &i;
        i += 1u32;
    }
    num(rat_int(result))
}

fn floor_rat_big(value: &Rat) -> BigInt {
    use num_integer::Integer;
    value.numer().div_floor(value.denom())
}

fn ceil_rat_big(value: &Rat) -> BigInt {
    use num_integer::Integer;
    value.numer().div_ceil(value.denom())
}

fn round_rat_big(value: &Rat) -> BigInt {
    let sign = if value.is_negative() { -1 } else { 1 };
    let abs = value.abs();
    let floor = floor_rat_big(&abs);
    let rem = &abs - Rat::from_integer(floor.clone());
    let rounded = if rem >= rat(1, 2) { floor + 1 } else { floor };
    if sign < 0 {
        -rounded
    } else {
        rounded
    }
}

fn dispatch_call(name: &str, args: Vec<Expr>) -> Expr {
    let lower = name.to_lowercase();
    let arity = |n: usize| args.len() == n;
    match lower.as_str() {
        "sqrt" if arity(1) => pow(args[0].clone(), num(rat(1, 2))),
        "cbrt" if arity(1) => pow(args[0].clone(), num(rat(1, 3))),
        "root" if arity(2) => {
            if let Expr::Num(n) = &args[1] {
                if n.is_integer() && n.numer().is_positive() {
                    if let Some(deg) = n.numer().to_i64() {
                        if deg <= 1000 {
                            return pow(args[0].clone(), num(rat(1, deg)));
                        }
                    }
                }
            }
            Expr::Call(name.to_string(), args)
        }
        "pow" if arity(2) => pow(args[0].clone(), args[1].clone()),
        "abs" if arity(1) => exact_abs(args[0].clone()),
        "sign" if arity(1) => {
            if let Expr::Num(v) = &args[0] {
                integer(v.signum().to_i64().unwrap_or(0))
            } else {
                Expr::Call("sign".into(), args)
            }
        }
        "floor" if arity(1) => {
            if let Expr::Num(v) = &args[0] {
                num(rat_int(floor_rat_big(v)))
            } else {
                Expr::Call("floor".into(), args)
            }
        }
        "ceil" if arity(1) => {
            if let Expr::Num(v) = &args[0] {
                num(rat_int(ceil_rat_big(v)))
            } else {
                Expr::Call("ceil".into(), args)
            }
        }
        "round" if arity(1) => {
            if let Expr::Num(v) = &args[0] {
                num(rat_int(round_rat_big(v)))
            } else {
                Expr::Call("round".into(), args)
            }
        }
        "exp" if arity(1) => match &args[0] {
            Expr::Num(v) if v.is_zero() => one(),
            Expr::Num(v) if v.is_one() => sym("e"),
            Expr::Call(n, inner) if n == "ln" => inner[0].clone(),
            _ => Expr::Call("exp".into(), args),
        },
        "ln" | "log" if lower == "log" && arity(2) => mul(vec![
            call_expr("ln", vec![args[0].clone()]),
            pow(call_expr("ln", vec![args[1].clone()]), integer(-1)),
        ]),
        "ln" | "log" if arity(1) => match &args[0] {
            Expr::Num(v) if v.is_one() => zero(),
            Expr::Sym(n) if n == "e" => one(),
            Expr::Pow(base, exp) if equals(base, &sym("e")) => (**exp).clone(),
            Expr::Call(n, inner) if n == "exp" => inner[0].clone(),
            _ => Expr::Call("ln".into(), args),
        },
        "log10" if arity(1) => mul(vec![
            call_expr("ln", vec![args[0].clone()]),
            pow(call_expr("ln", vec![integer(10)]), integer(-1)),
        ]),
        "log2" if arity(1) => mul(vec![
            call_expr("ln", vec![args[0].clone()]),
            pow(call_expr("ln", vec![integer(2)]), integer(-1)),
        ]),
        "sin" | "cos" | "tan" if arity(1) => {
            if let Some(exact) = exact_trig(lower.as_str(), &args[0]) {
                return exact;
            }
            if is_negative_mul(&args[0]) {
                let positive = mul(vec![args[0].clone(), integer(-1)]);
                if lower == "cos" {
                    return call_expr("cos", vec![positive]);
                }
                return neg(call_expr(lower.as_str(), vec![positive]));
            }
            Expr::Call(lower, args)
        }
        "asin" if arity(1) => {
            exact_asin(&args[0]).unwrap_or_else(|| Expr::Call("asin".into(), args))
        }
        "acos" if arity(1) => {
            if let Some(asin) = exact_asin(&args[0]) {
                sub(mul(vec![num(rat(1, 2)), sym("pi")]), asin)
            } else {
                Expr::Call("acos".into(), args)
            }
        }
        "atan" if arity(1) => {
            exact_atan(&args[0]).unwrap_or_else(|| Expr::Call("atan".into(), args))
        }
        "sinh" | "cosh" | "tanh" if arity(1) => {
            if let Expr::Num(v) = &args[0] {
                if v.is_zero() {
                    return if lower == "cosh" { one() } else { zero() };
                }
            }
            Expr::Call(lower, args)
        }
        "factorial" if arity(1) => {
            if let Expr::Num(v) = &args[0] {
                if v.is_integer() {
                    return factorial_expr(v.numer());
                }
            }
            Expr::Call("factorial".into(), args)
        }
        "binomial" if arity(2) => {
            if let (Expr::Num(n), Expr::Num(k)) = (&args[0], &args[1]) {
                if n.is_integer() && k.is_integer() {
                    return binomial_expr(n.numer(), k.numer());
                }
            }
            Expr::Call("binomial".into(), args)
        }
        "gcd" | "lcm" if args.len() >= 2 => {
            if args
                .iter()
                .all(|a| matches!(a, Expr::Num(v) if v.is_integer()))
            {
                let mut result = match &args[0] {
                    Expr::Num(v) => v.numer().abs(),
                    _ => BigInt::zero(),
                };
                for arg in &args[1..] {
                    if let Expr::Num(v) = arg {
                        let value = v.numer().abs();
                        if lower == "gcd" {
                            result = result.gcd(&value);
                        } else {
                            result = if result.is_zero() || value.is_zero() {
                                BigInt::zero()
                            } else {
                                (&result / result.gcd(&value)) * &value
                            };
                        }
                    }
                }
                num(rat_int(result))
            } else {
                Expr::Call(lower, args)
            }
        }
        "mod" if arity(2) => {
            if let (Expr::Num(a), Expr::Num(b)) = (&args[0], &args[1]) {
                if b.is_integer() {
                    let divisor = b.numer();
                    if divisor.is_zero() {
                        return Expr::Call("mod".into(), args);
                    }
                    if a.is_integer() {
                        use num_integer::Integer;
                        let mut r = a.numer().mod_floor(divisor);
                        if r.is_negative() {
                            r += divisor.abs();
                        }
                        return num(rat_int(r));
                    }
                    let q = floor_rat_big(&(a / b));
                    return num(a - b * Rat::from_integer(q));
                }
            }
            Expr::Call("mod".into(), args)
        }
        "min" | "max" if !args.is_empty() => {
            if args.iter().all(|a| matches!(a, Expr::Num(_))) {
                let mut best = match &args[0] {
                    Expr::Num(v) => v.clone(),
                    _ => Rat::zero(),
                };
                for arg in &args[1..] {
                    if let Expr::Num(v) = arg {
                        if (lower == "min" && v < &best) || (lower == "max" && v > &best) {
                            best = v.clone();
                        }
                    }
                }
                num(best)
            } else {
                Expr::Call(lower, args)
            }
        }
        "eq" if arity(2) => eq_expr(args[0].clone(), args[1].clone()),
        _ => Expr::Call(name.to_string(), args),
    }
}

pub fn call_expr(name: &str, args: Vec<Expr>) -> Expr {
    dispatch_call(name, args)
}

pub fn call(name: &str, args: Vec<Expr>) -> Expr {
    dispatch_call(name, args)
}

// ---------------------------------------------------------------------------
// 精确复数
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ExactComplex {
    pub re: Rat,
    pub im: Rat,
}

pub fn to_exact_complex(expr: &Expr) -> Option<ExactComplex> {
    match expr {
        Expr::Num(v) => Some(ExactComplex {
            re: v.clone(),
            im: Rat::zero(),
        }),
        Expr::Sym(name) if name == "i" => Some(ExactComplex {
            re: Rat::zero(),
            im: Rat::one(),
        }),
        Expr::Add(terms) => {
            let mut re = Rat::zero();
            let mut im = Rat::zero();
            for term in terms {
                let c = to_exact_complex(term)?;
                re += c.re;
                im += c.im;
            }
            Some(ExactComplex { re, im })
        }
        Expr::Mul(factors) => {
            let mut acc = ExactComplex {
                re: Rat::one(),
                im: Rat::zero(),
            };
            for factor in factors {
                acc = multiply_complex(&acc, &to_exact_complex(factor)?);
            }
            Some(acc)
        }
        Expr::Pow(base, exp) => {
            let n = match &**exp {
                Expr::Num(v) if v.is_integer() => v.numer().to_i64()?,
                _ => return None,
            };
            let base = to_exact_complex(base)?;
            pow_complex(&base, n)
        }
        _ => None,
    }
}

pub fn multiply_complex(a: &ExactComplex, b: &ExactComplex) -> ExactComplex {
    ExactComplex {
        re: &a.re * &b.re - &a.im * &b.im,
        im: &a.re * &b.im + &a.im * &b.re,
    }
}

fn reciprocal_complex(a: &ExactComplex) -> Option<ExactComplex> {
    let denom = &a.re * &a.re + &a.im * &a.im;
    if denom.is_zero() {
        return None;
    }
    Some(ExactComplex {
        re: &a.re / &denom,
        im: -&a.im / &denom,
    })
}

pub fn divide_complex(a: &ExactComplex, b: &ExactComplex) -> Option<ExactComplex> {
    Some(multiply_complex(a, &reciprocal_complex(b)?))
}

pub fn pow_complex(a: &ExactComplex, n: i64) -> Option<ExactComplex> {
    if n == 0 {
        return Some(ExactComplex {
            re: Rat::one(),
            im: Rat::zero(),
        });
    }
    let negative = n < 0;
    let mut exp = n.unsigned_abs();
    let mut base = a.clone();
    let mut result = ExactComplex {
        re: Rat::one(),
        im: Rat::zero(),
    };
    while exp > 0 {
        if exp & 1 == 1 {
            result = multiply_complex(&result, &base);
        }
        base = multiply_complex(&base, &base);
        exp >>= 1;
    }
    if negative {
        reciprocal_complex(&result)
    } else {
        Some(result)
    }
}

pub fn from_complex(value: &ExactComplex) -> Expr {
    if value.im.is_zero() {
        return num(value.re.clone());
    }
    let im_part = if value.im.is_one() {
        sym("i")
    } else {
        mul(vec![num(value.im.clone()), sym("i")])
    };
    if value.re.is_zero() {
        im_part
    } else {
        add(vec![num(value.re.clone()), im_part])
    }
}

// ---------------------------------------------------------------------------
// 结构遍历
// ---------------------------------------------------------------------------

pub fn occurs(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Sym(n) => n == name,
        Expr::Add(terms) => terms.iter().any(|t| occurs(t, name)),
        Expr::Mul(factors) => factors.iter().any(|t| occurs(t, name)),
        Expr::Pow(b, e) => occurs(b, name) || occurs(e, name),
        Expr::Call(_, args) => args.iter().any(|t| occurs(t, name)),
        Expr::Eq(l, r) => occurs(l, name) || occurs(r, name),
        Expr::List(items) => items.iter().any(|t| occurs(t, name)),
        _ => false,
    }
}

pub fn collect_symbols(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Sym(n) => {
            if !out.contains(n) {
                out.push(n.clone());
            }
        }
        Expr::Add(terms) => terms.iter().for_each(|t| collect_symbols(t, out)),
        Expr::Mul(factors) => factors.iter().for_each(|t| collect_symbols(t, out)),
        Expr::Pow(b, e) => {
            collect_symbols(b, out);
            collect_symbols(e, out);
        }
        Expr::Call(_, args) => args.iter().for_each(|t| collect_symbols(t, out)),
        Expr::Eq(l, r) => {
            collect_symbols(l, out);
            collect_symbols(r, out);
        }
        Expr::List(items) => items.iter().for_each(|t| collect_symbols(t, out)),
        _ => {}
    }
}

pub fn substitute(expr: &Expr, name: &str, replacement: &Expr) -> Expr {
    match expr {
        Expr::Sym(n) if n == name => replacement.clone(),
        Expr::Add(terms) => add(terms
            .iter()
            .map(|t| substitute(t, name, replacement))
            .collect()),
        Expr::Mul(factors) => mul(factors
            .iter()
            .map(|t| substitute(t, name, replacement))
            .collect()),
        Expr::Pow(b, e) => pow(
            substitute(b, name, replacement),
            substitute(e, name, replacement),
        ),
        Expr::Call(n, args) => call_expr(
            n,
            args.iter()
                .map(|t| substitute(t, name, replacement))
                .collect(),
        ),
        Expr::Eq(l, r) => eq_expr(
            substitute(l, name, replacement),
            substitute(r, name, replacement),
        ),
        Expr::List(items) => Expr::List(
            items
                .iter()
                .map(|t| substitute(t, name, replacement))
                .collect(),
        ),
        other => other.clone(),
    }
}

pub fn substitute_many(expr: &Expr, bindings: &HashMap<String, Expr>) -> Expr {
    match expr {
        Expr::Sym(n) => bindings.get(n).cloned().unwrap_or_else(|| expr.clone()),
        Expr::Add(terms) => add(terms.iter().map(|t| substitute_many(t, bindings)).collect()),
        Expr::Mul(factors) => mul(factors
            .iter()
            .map(|t| substitute_many(t, bindings))
            .collect()),
        Expr::Pow(b, e) => pow(substitute_many(b, bindings), substitute_many(e, bindings)),
        Expr::Call(n, args) => call_expr(
            n,
            args.iter().map(|t| substitute_many(t, bindings)).collect(),
        ),
        Expr::Eq(l, r) => eq_expr(substitute_many(l, bindings), substitute_many(r, bindings)),
        Expr::List(items) => {
            Expr::List(items.iter().map(|t| substitute_many(t, bindings)).collect())
        }
        other => other.clone(),
    }
}

pub fn node_count(expr: &Expr) -> usize {
    match expr {
        Expr::Add(terms) => 1 + terms.iter().map(node_count).sum::<usize>(),
        Expr::Mul(factors) => 1 + factors.iter().map(node_count).sum::<usize>(),
        Expr::Pow(b, e) => 1 + node_count(b) + node_count(e),
        Expr::Call(_, args) => 1 + args.iter().map(node_count).sum::<usize>(),
        Expr::Eq(l, r) => 1 + node_count(l) + node_count(r),
        Expr::List(items) => 1 + items.iter().map(node_count).sum::<usize>(),
        _ => 1,
    }
}

pub fn has_float(expr: &Expr) -> bool {
    match expr {
        Expr::Float(_) => true,
        Expr::Add(terms) => terms.iter().any(has_float),
        Expr::Mul(factors) => factors.iter().any(has_float),
        Expr::Pow(b, e) => has_float(b) || has_float(e),
        Expr::Call(_, args) => args.iter().any(has_float),
        Expr::Eq(l, r) => has_float(l) || has_float(r),
        Expr::List(items) => items.iter().any(has_float),
        _ => false,
    }
}

pub fn is_constant(expr: &Expr) -> bool {
    match expr {
        Expr::Num(_) | Expr::Float(_) | Expr::Str(_) => true,
        Expr::Sym(n) => matches!(n.as_str(), "pi" | "e" | "i"),
        Expr::Add(terms) => terms.iter().all(is_constant),
        Expr::Mul(factors) => factors.iter().all(is_constant),
        Expr::Pow(b, e) => is_constant(b) && is_constant(e),
        Expr::Call(_, args) => args.iter().all(is_constant),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// 输出
// ---------------------------------------------------------------------------

include!("format_expr.rs");

// 便于其他模块使用
pub fn to_plain(expr: &Expr) -> String {
    plain(expr, 0)
}

pub fn to_unicode(expr: &Expr) -> String {
    unicode(expr, 0)
}

pub fn to_latex(expr: &Expr) -> String {
    latex(expr, 0)
}

pub fn rat_expr_display(value: &Rat) -> String {
    rat_display(value)
}

pub fn float_from_f64(value: f64, prec: usize) -> Option<BigFloat> {
    BigFloat::from_f64(value, prec)
}

pub fn pi_float(prec: usize) -> BigFloat {
    bf_pi(prec)
}

pub fn rational_gcd_expr(a: &Rat, b: &Rat) -> Rat {
    rational_gcd(a, b)
}

pub fn bigint_pow_expr(base: &BigInt, exp: u32) -> BigInt {
    bigint_pow(base, exp)
}

pub fn rat_to_float(value: &Rat) -> f64 {
    rat_to_f64(value)
}

pub fn rat_to_integer(value: &Rat) -> Option<BigInt> {
    if value.is_integer() {
        Some(value.numer().clone())
    } else {
        None
    }
}

pub fn num_from_ratio(value: Rat) -> Expr {
    num(value)
}

pub fn ratio_zero() -> Rat {
    Rat::zero()
}

pub fn ratio_one() -> Rat {
    Rat::one()
}

pub fn clone_expr(expr: &Expr) -> Expr {
    expr.clone()
}

pub fn add_one(expr: Expr) -> Expr {
    add(vec![expr, one()])
}

pub fn sub_one(expr: Expr) -> Expr {
    add(vec![expr, integer(-1)])
}

pub fn neg_one() -> Expr {
    integer(-1)
}

pub fn as_num(expr: &Expr) -> Option<&Rat> {
    match expr {
        Expr::Num(v) => Some(v),
        _ => None,
    }
}

pub fn ordering_key(a: &Expr, b: &Expr) -> Ordering {
    key(a).cmp(&key(b))
}
