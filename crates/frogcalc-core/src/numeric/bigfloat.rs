use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{One, Signed, ToPrimitive, Zero};

use super::{rat_to_f64, Rat};

const GUARD: usize = 14;

/// 十进制任意精度浮点数：value = sign * digits * 10^exp
#[derive(Clone, Debug)]
pub struct BigFloat {
    pub sign: i8,
    pub digits: BigUint,
    pub exp: i64,
    pub prec: usize,
}

fn pow10(n: usize) -> BigUint {
    BigUint::from(10u32).pow(n.min(1_000_000) as u32)
}

fn digits_len(v: &BigUint) -> usize {
    v.to_str_radix(10).len()
}

impl BigFloat {
    pub fn normalize(sign: i8, mut digits: BigUint, mut exp: i64, prec: usize) -> Self {
        let prec = prec.max(1);
        if digits.is_zero() {
            return Self {
                sign: 1,
                digits: BigUint::zero(),
                exp: 0,
                prec,
            };
        }
        let sign = if sign < 0 { -1 } else { 1 };
        let s = digits.to_str_radix(10);
        if s.len() > prec {
            let keep = BigUint::parse_bytes(s[..prec].as_bytes(), 10).unwrap_or_else(BigUint::zero);
            let next = s.as_bytes()[prec] as i32 - b'0' as i32;
            let mut kept = keep;
            exp += (s.len() - prec) as i64;
            if next >= 5 {
                kept += 1u32;
            }
            if digits_len(&kept) > prec {
                kept /= 10u32;
                exp += 1;
            }
            digits = kept;
        }
        let ten = BigUint::from(10u32);
        while !digits.is_zero() && (&digits % &ten).is_zero() {
            digits /= &ten;
            exp += 1;
        }
        Self {
            sign,
            digits,
            exp,
            prec,
        }
    }

    pub fn zero(prec: usize) -> Self {
        Self::normalize(1, BigUint::zero(), 0, prec)
    }

    pub fn one(prec: usize) -> Self {
        Self::normalize(1, BigUint::one(), 0, prec)
    }

    pub fn from_bigint(value: &BigInt, prec: usize) -> Self {
        Self::normalize(
            if value.is_negative() { -1 } else { 1 },
            value.magnitude().clone(),
            0,
            prec,
        )
    }

    pub fn from_i64(value: i64, prec: usize) -> Self {
        Self::from_bigint(&BigInt::from(value), prec)
    }

    pub fn from_u32(value: u32, prec: usize) -> Self {
        Self::normalize(1, BigUint::from(value), 0, prec)
    }

    pub fn from_rat(value: &Rat, prec: usize) -> Self {
        if value.is_zero() {
            return Self::zero(prec);
        }
        let sign = if value.is_negative() { -1 } else { 1 };
        let a = value.numer().magnitude().clone();
        let b = value.denom().magnitude().clone();
        if b == BigUint::one() {
            return Self::normalize(sign, a, 0, prec);
        }
        let work = prec + GUARD;
        let desired = work + 6;
        let shift = desired as i64 + digits_len(&b) as i64 - digits_len(&a) as i64 - 1;
        let (q, exp) = if shift >= 0 {
            ((a * pow10(shift as usize)) / b, -(shift as i64))
        } else {
            (a / (b * pow10((-shift) as usize)), -(shift as i64))
        };
        Self::normalize(sign, q, exp, prec)
    }

    pub fn from_f64(value: f64, prec: usize) -> Option<Self> {
        if !value.is_finite() {
            return None;
        }
        Self::from_str(&format!("{value:?}"), prec)
    }

    pub fn from_str(text: &str, prec: usize) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        let (sign, rest) = match text.as_bytes()[0] {
            b'-' => (-1i8, &text[1..]),
            b'+' => (1i8, &text[1..]),
            _ => (1i8, text),
        };
        let (mantissa, exp) = match rest.find(['e', 'E']) {
            Some(i) => (&rest[..i], rest[i + 1..].parse::<i64>().ok()?),
            None => (rest, 0i64),
        };
        let (int_part, frac_part) = match mantissa.find('.') {
            Some(i) => (&mantissa[..i], &mantissa[i + 1..]),
            None => (mantissa, ""),
        };
        let digits_text = format!("{int_part}{frac_part}");
        if digits_text.is_empty() || !digits_text.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let digits = BigUint::parse_bytes(digits_text.as_bytes(), 10)?;
        Some(Self::normalize(
            sign,
            digits,
            exp - frac_part.len() as i64,
            prec,
        ))
    }

    pub fn with_precision(&self, prec: usize) -> Self {
        Self::normalize(self.sign, self.digits.clone(), self.exp, prec)
    }

    pub fn is_zero(&self) -> bool {
        self.digits.is_zero()
    }

    pub fn is_negative(&self) -> bool {
        self.sign < 0 && !self.is_zero()
    }

    pub fn is_positive(&self) -> bool {
        self.sign > 0 && !self.is_zero()
    }

    pub fn is_integer(&self) -> bool {
        if self.is_zero() || self.exp >= 0 {
            return true;
        }
        (&self.digits % pow10((-self.exp) as usize)).is_zero()
    }

    pub fn order(&self) -> i64 {
        if self.is_zero() {
            0
        } else {
            digits_len(&self.digits) as i64 - 1 + self.exp
        }
    }

    pub fn abs(&self) -> Self {
        Self {
            sign: 1,
            digits: self.digits.clone(),
            exp: self.exp,
            prec: self.prec,
        }
    }

    pub fn neg(&self) -> Self {
        Self {
            sign: -self.sign,
            digits: self.digits.clone(),
            exp: self.exp,
            prec: self.prec,
        }
    }

    pub fn scale10(&self, k: i64, prec: usize) -> Self {
        Self::normalize(self.sign, self.digits.clone(), self.exp + k, prec)
    }

    pub fn add(&self, other: &Self, prec: usize) -> Self {
        let work = prec + GUARD;
        let a = self.with_precision(work);
        let b = other.with_precision(work);
        if a.is_zero() {
            return b.with_precision(prec);
        }
        if b.is_zero() {
            return a.with_precision(prec);
        }
        if a.exp - b.exp > work as i64 + 6 {
            return a.with_precision(prec);
        }
        if b.exp - a.exp > work as i64 + 6 {
            return b.with_precision(prec);
        }
        let e = a.exp.min(b.exp);
        let da = a.digits.clone() * pow10((a.exp - e) as usize);
        let db = b.digits.clone() * pow10((b.exp - e) as usize);
        let mut sum = BigInt::from_biguint(Sign::Plus, da);
        if a.sign < 0 {
            sum = -sum;
        }
        let mut rhs = BigInt::from_biguint(Sign::Plus, db);
        if b.sign < 0 {
            rhs = -rhs;
        }
        sum += rhs;
        if sum.is_zero() {
            return Self::zero(prec);
        }
        let sign = if sum.is_negative() { -1 } else { 1 };
        Self::normalize(sign, sum.magnitude().clone(), e, prec)
    }

    pub fn sub(&self, other: &Self, prec: usize) -> Self {
        self.add(&other.neg(), prec)
    }

    pub fn mul(&self, other: &Self, prec: usize) -> Self {
        if self.is_zero() || other.is_zero() {
            return Self::zero(prec);
        }
        let work = prec + GUARD;
        let a = self.with_precision(work);
        let b = other.with_precision(work);
        Self::normalize(
            if a.sign == b.sign { 1 } else { -1 },
            a.digits * b.digits,
            a.exp + b.exp,
            prec,
        )
    }

    pub fn div(&self, other: &Self, prec: usize) -> Self {
        assert!(!other.is_zero(), "不能除以零");
        if self.is_zero() {
            return Self::zero(prec);
        }
        let work = prec + GUARD;
        let a = self.with_precision(work);
        let b = other.with_precision(work);
        let desired = work + 6;
        let shift =
            desired as i64 + digits_len(&b.digits) as i64 - digits_len(&a.digits) as i64 - 1;
        let (q, exp) = if shift >= 0 {
            (
                (a.digits * pow10(shift as usize)) / b.digits,
                a.exp - b.exp - shift,
            )
        } else {
            (
                a.digits / (b.digits * pow10((-shift) as usize)),
                a.exp - b.exp - shift,
            )
        };
        Self::normalize(if a.sign == b.sign { 1 } else { -1 }, q, exp, prec)
    }

    pub fn recip(&self, prec: usize) -> Self {
        Self::one(prec + GUARD).div(self, prec)
    }

    pub fn pow_int(&self, exp: i64, prec: usize) -> Option<Self> {
        if exp.unsigned_abs() > 1_000_000 {
            return None;
        }
        if exp == 0 {
            return Some(Self::one(prec));
        }
        if self.is_zero() {
            if exp < 0 {
                return None;
            }
            return Some(Self::zero(prec));
        }
        let work = prec + GUARD;
        let mut base = self.with_precision(work);
        let mut e = exp.unsigned_abs();
        let mut result = Self::one(work);
        while e > 0 {
            if e & 1 == 1 {
                result = result.mul(&base, work);
            }
            base = base.mul(&base, work);
            e >>= 1;
        }
        if exp < 0 {
            Some(result.recip(prec))
        } else {
            Some(result.with_precision(prec))
        }
    }

    pub fn compare(&self, other: &Self) -> Ordering {
        if self.is_zero() && other.is_zero() {
            return Ordering::Equal;
        }
        if self.is_zero() {
            return if other.is_negative() {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }
        if other.is_zero() {
            return if self.is_negative() {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        if self.sign != other.sign {
            return if self.sign < other.sign {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        let order_a = self.order();
        let order_b = other.order();
        if order_a != order_b {
            let cmp = order_a.cmp(&order_b);
            return if self.sign < 0 { cmp.reverse() } else { cmp };
        }
        let e = self.exp.min(other.exp);
        let da = self.digits.clone() * pow10((self.exp - e) as usize);
        let db = other.digits.clone() * pow10((other.exp - e) as usize);
        let cmp = da.cmp(&db);
        if self.sign < 0 {
            cmp.reverse()
        } else {
            cmp
        }
    }

    pub fn to_f64(&self) -> f64 {
        self.to_string().parse::<f64>().unwrap_or(f64::NAN)
    }

    pub fn to_bigint(&self) -> Option<BigInt> {
        if !self.is_integer() {
            return None;
        }
        if self.is_zero() {
            return Some(BigInt::zero());
        }
        let magnitude = if self.exp >= 0 {
            self.digits.clone() * pow10(self.exp as usize)
        } else {
            self.digits.clone() / pow10((-self.exp) as usize)
        };
        Some(BigInt::from_biguint(
            if self.sign < 0 {
                Sign::Minus
            } else {
                Sign::Plus
            },
            magnitude,
        ))
    }

    pub fn floor(&self) -> Self {
        self.round_mode(0)
    }

    pub fn ceil(&self) -> Self {
        self.round_mode(1)
    }

    pub fn round(&self) -> Self {
        self.round_mode(2)
    }

    fn round_mode(&self, mode: u8) -> Self {
        if self.is_zero() || self.exp >= 0 {
            return self.clone();
        }
        let divisor = pow10((-self.exp) as usize);
        let q = &self.digits / &divisor;
        let rem = &self.digits % &divisor;
        let mut magnitude = q;
        match mode {
            0 => {
                if self.sign < 0 && !rem.is_zero() {
                    magnitude += 1u32;
                }
            }
            1 => {
                if self.sign > 0 && !rem.is_zero() {
                    magnitude += 1u32;
                }
            }
            _ => {
                if rem.clone() * 2u32 >= divisor {
                    magnitude += 1u32;
                }
            }
        }
        let value = BigInt::from_biguint(Sign::Plus, magnitude);
        let value = if self.sign < 0 { -value } else { value };
        Self::from_bigint(&value, self.prec)
    }

    pub fn to_string(&self) -> String {
        if self.is_zero() {
            return "0".to_string();
        }
        let s = self.digits.to_str_radix(10);
        let order = s.len() as i64 - 1 + self.exp;
        let sign = if self.sign < 0 { "-" } else { "" };
        if order >= 16 || order <= -7 {
            let mantissa = if s.len() > 1 {
                format!("{}.{}", &s[..1], &s[1..])
            } else {
                s
            };
            let exp_sign = if order >= 0 { "+" } else { "" };
            return format!("{sign}{mantissa}e{exp_sign}{order}");
        }
        let point = s.len() as i64 + self.exp;
        if point >= s.len() as i64 {
            format!(
                "{sign}{}{}",
                s,
                "0".repeat((point - s.len() as i64) as usize)
            )
        } else if point > 0 {
            format!("{sign}{}.{}", &s[..point as usize], &s[point as usize..])
        } else {
            format!("{sign}0.{}{}", "0".repeat((-point) as usize), s)
        }
    }

    pub fn to_precision_string(&self, sig: usize) -> String {
        self.with_precision(sig.max(1)).to_string()
    }
}

impl std::fmt::Display for BigFloat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string())
    }
}

fn epsilon(work: usize) -> BigFloat {
    BigFloat::from_str(&format!("1e-{}", work + 4), work).unwrap()
}

fn cache() -> &'static Mutex<HashMap<(usize, &'static str), BigFloat>> {
    static CACHE: OnceLock<Mutex<HashMap<(usize, &'static str), BigFloat>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cached(name: &'static str, prec: usize, f: impl FnOnce(usize) -> BigFloat) -> BigFloat {
    let key = (prec, name);
    if let Some(v) = cache().lock().unwrap().get(&key) {
        return v.clone();
    }
    let value = f(prec);
    cache().lock().unwrap().insert(key, value.clone());
    value
}

fn atan_inv(n: u32, work: usize) -> BigFloat {
    let z = BigFloat::one(work).div(&BigFloat::from_u32(n, work), work);
    let z2 = z.mul(&z, work);
    let mut term = z.clone();
    let mut sum = z;
    for k in 1..100_000u32 {
        term = term.mul(&z2, work).neg();
        let contribution = term.div(&BigFloat::from_u32(2 * k + 1, work), work);
        sum = sum.add(&contribution, work);
        if contribution.abs().compare(&epsilon(work)) == Ordering::Less {
            break;
        }
    }
    sum
}

pub fn bf_pi(prec: usize) -> BigFloat {
    cached("pi", prec, |p| {
        let work = p + 18;
        let a = atan_inv(5, work).mul(&BigFloat::from_u32(16, work), work);
        let b = atan_inv(239, work).mul(&BigFloat::from_u32(4, work), work);
        a.sub(&b, p)
    })
}

fn atanh_series(z: &BigFloat, work: usize) -> BigFloat {
    let z2 = z.mul(z, work);
    let mut term = z.clone();
    let mut sum = z.clone();
    for k in 1..100_000u32 {
        term = term.mul(&z2, work);
        let contribution = term.div(&BigFloat::from_u32(2 * k + 1, work), work);
        sum = sum.add(&contribution, work);
        if contribution.abs().compare(&epsilon(work)) == Ordering::Less {
            break;
        }
    }
    sum
}

pub fn bf_ln2(prec: usize) -> BigFloat {
    cached("l2", prec, |p| {
        let work = p + 18;
        let z = BigFloat::one(work).div(&BigFloat::from_u32(3, work), work);
        atanh_series(&z, work).mul(&BigFloat::from_u32(2, work), p)
    })
}

pub fn bf_ln10(prec: usize) -> BigFloat {
    cached("l10", prec, |p| {
        let work = p + 18;
        let ln2 = bf_ln2(work);
        let z = BigFloat::one(work).div(&BigFloat::from_u32(9, work), work);
        let ln5over4 = atanh_series(&z, work).mul(&BigFloat::from_u32(2, work), work);
        ln2.mul(&BigFloat::from_u32(3, work), work)
            .add(&ln5over4, p)
    })
}

pub fn bf_ln(x: &BigFloat, prec: usize) -> Result<BigFloat, String> {
    if x.is_zero() || x.is_negative() {
        return Err("对数要求正数".into());
    }
    let work = prec + GUARD + 8;
    let xw = x.with_precision(work);
    if xw.digits == BigUint::one() && xw.exp == 0 {
        return Ok(BigFloat::zero(prec));
    }
    let order = xw.order();
    let mut m = BigFloat::normalize(1, xw.digits.clone(), xw.exp - order, work);
    for _ in 0..5 {
        m = bf_sqrt(&m, work)?;
    }
    let one = BigFloat::one(work);
    let z = m.sub(&one, work).div(&m.add(&one, work), work);
    let ln_m = atanh_series(&z, work)
        .mul(&BigFloat::from_u32(2, work), work)
        .mul(&BigFloat::from_u32(32, work), work);
    let ln10 = bf_ln10(work);
    Ok(ln_m.add(&ln10.mul(&BigFloat::from_i64(order, work), work), prec))
}

pub fn bf_exp(x: &BigFloat, prec: usize) -> Result<BigFloat, String> {
    let work = prec + GUARD + 8;
    let xw = x.with_precision(work);
    if xw.is_zero() {
        return Ok(BigFloat::one(prec));
    }
    if xw.order().abs() > 15 {
        return Err("指数函数的参数过大".into());
    }
    let ln10 = bf_ln10(work);
    let k = xw.div(&ln10, work).to_f64().round() as i64;
    let r = xw.sub(&ln10.mul(&BigFloat::from_i64(k, work), work), work);
    let mut term = BigFloat::one(work);
    let mut sum = BigFloat::one(work);
    let eps = epsilon(work);
    for i in 1..100_000u32 {
        term = term.mul(&r, work).div(&BigFloat::from_u32(i, work), work);
        sum = sum.add(&term, work);
        if term.abs().compare(&eps) == Ordering::Less {
            break;
        }
    }
    let result = sum.scale10(k, work);
    if result.order().abs() > 10_000_000 {
        return Err("结果数量级过大".into());
    }
    Ok(result.with_precision(prec))
}

pub fn bf_sqrt(x: &BigFloat, prec: usize) -> Result<BigFloat, String> {
    if x.is_negative() {
        return Err("负数不能开平方".into());
    }
    if x.is_zero() {
        return Ok(BigFloat::zero(prec));
    }
    let work = prec + GUARD + 10;
    let xw = x.with_precision(work);
    let order = xw.order();
    let h = order.div_euclid(2);
    let m = BigFloat::normalize(1, xw.digits.clone(), xw.exp - 2 * h, work);
    let mut g = BigFloat::from_f64(m.to_f64().sqrt(), work)
        .ok_or_else(|| "开方初值失败".to_string())?
        .scale10(h, work);
    let two = BigFloat::from_u32(2, work);
    for _ in 0..8 {
        g = g.add(&xw.div(&g, work), work).div(&two, work);
    }
    Ok(g.with_precision(prec))
}

pub fn bf_atan(x: &BigFloat, prec: usize) -> Result<BigFloat, String> {
    let work = prec + GUARD + 10;
    let xw = x.with_precision(work);
    if xw.is_zero() {
        return Ok(BigFloat::zero(prec));
    }
    let sign = if xw.is_negative() { -1 } else { 1 };
    let mut a = xw.abs();
    let one = BigFloat::one(work);
    if a.compare(&one) == Ordering::Greater {
        let pi = bf_pi(work);
        let inner = bf_atan(&one.div(&a, work), work)?;
        let result = pi.div(&BigFloat::from_u32(2, work), work).sub(&inner, work);
        return Ok(if sign < 0 {
            result.neg().with_precision(prec)
        } else {
            result.with_precision(prec)
        });
    }
    for _ in 0..3 {
        let root = bf_sqrt(&one.add(&a.mul(&a, work), work), work)?;
        a = a.div(&one.add(&root, work), work);
    }
    let a2 = a.mul(&a, work);
    let mut term = a.clone();
    let mut sum = a;
    let eps = epsilon(work);
    for k in 1..100_000u32 {
        term = term.mul(&a2, work).neg();
        let contribution = term.div(&BigFloat::from_u32(2 * k + 1, work), work);
        sum = sum.add(&contribution, work);
        if contribution.abs().compare(&eps) == Ordering::Less {
            break;
        }
    }
    sum = sum.mul(&BigFloat::from_u32(8, work), work);
    let result = if sign < 0 { sum.neg() } else { sum };
    Ok(result.with_precision(prec))
}

fn reduce_angle(x: &BigFloat, work: usize) -> Result<BigFloat, String> {
    let pi = bf_pi(work + 6);
    let two_pi = pi.mul(&BigFloat::from_u32(2, work), work);
    let q = x.div(&two_pi, work).to_f64();
    if !q.is_finite() || q.abs() > 1e14 {
        return Err("三角函数的参数过大，无法可靠规约".into());
    }
    let q = q.round() as i64;
    let mut r = x.sub(&two_pi.mul(&BigFloat::from_i64(q, work), work), work);
    if r.compare(&pi) == Ordering::Greater {
        r = r.sub(&two_pi, work);
    }
    if r.compare(&pi.neg()) == Ordering::Less {
        r = r.add(&two_pi, work);
    }
    Ok(r)
}

pub fn bf_sin(x: &BigFloat, prec: usize) -> Result<BigFloat, String> {
    let work = prec + GUARD + 8;
    let mut r = reduce_angle(&x.with_precision(work), work)?;
    let pi = bf_pi(work);
    let half_pi = pi.div(&BigFloat::from_u32(2, work), work);
    if r.compare(&half_pi) == Ordering::Greater {
        r = pi.sub(&r, work);
    } else if r.compare(&half_pi.neg()) == Ordering::Less {
        r = pi.neg().sub(&r, work);
    }
    let r2 = r.mul(&r, work);
    let mut term = r.clone();
    let mut sum = r;
    let eps = epsilon(work);
    for k in 1..100_000u32 {
        term = term
            .mul(&r2, work)
            .neg()
            .div(&BigFloat::from_u32(2 * k * (2 * k + 1), work), work);
        sum = sum.add(&term, work);
        if term.abs().compare(&eps) == Ordering::Less {
            break;
        }
    }
    Ok(sum.with_precision(prec))
}

pub fn bf_cos(x: &BigFloat, prec: usize) -> Result<BigFloat, String> {
    let work = prec + GUARD + 8;
    let mut r = reduce_angle(&x.with_precision(work), work)?;
    let pi = bf_pi(work);
    let half_pi = pi.div(&BigFloat::from_u32(2, work), work);
    let mut sign = 1;
    if r.compare(&half_pi) == Ordering::Greater {
        r = pi.sub(&r, work);
        sign = -1;
    } else if r.compare(&half_pi.neg()) == Ordering::Less {
        r = pi.neg().sub(&r, work);
        sign = -1;
    }
    let r2 = r.mul(&r, work);
    let mut term = BigFloat::one(work);
    let mut sum = BigFloat::one(work);
    let eps = epsilon(work);
    for k in 1..100_000u32 {
        term = term
            .mul(&r2, work)
            .neg()
            .div(&BigFloat::from_u32((2 * k - 1) * (2 * k), work), work);
        sum = sum.add(&term, work);
        if term.abs().compare(&eps) == Ordering::Less {
            break;
        }
    }
    Ok(if sign < 0 {
        sum.neg().with_precision(prec)
    } else {
        sum.with_precision(prec)
    })
}

pub fn bf_tan(x: &BigFloat, prec: usize) -> Result<BigFloat, String> {
    let work = prec + GUARD + 8;
    let s = bf_sin(x, work)?;
    let c = bf_cos(x, work)?;
    if c.abs().compare(&epsilon(work)) == Ordering::Less {
        return Err("正切在该点无定义".into());
    }
    Ok(s.div(&c, prec))
}

pub fn bf_asin(x: &BigFloat, prec: usize) -> Result<BigFloat, String> {
    let work = prec + GUARD + 8;
    let xw = x.with_precision(work);
    let one = BigFloat::one(work);
    if xw.compare(&one) == Ordering::Greater || xw.compare(&one.neg()) == Ordering::Less {
        return Err("asin 的定义域为 [-1, 1]".into());
    }
    let sq = one.sub(&xw.mul(&xw, work), work);
    if sq.compare(&epsilon(work)) != Ordering::Greater {
        let half_pi = bf_pi(work).div(&BigFloat::from_u32(2, work), work);
        return Ok(if xw.is_negative() {
            half_pi.neg().with_precision(prec)
        } else {
            half_pi.with_precision(prec)
        });
    }
    let denom = bf_sqrt(&sq, work)?;
    bf_atan(&xw.div(&denom, work), prec)
}

pub fn bf_acos(x: &BigFloat, prec: usize) -> Result<BigFloat, String> {
    let work = prec + GUARD + 8;
    let half_pi = bf_pi(work).div(&BigFloat::from_u32(2, work), work);
    Ok(half_pi.sub(&bf_asin(x, work)?, prec))
}

pub fn bf_sinh(x: &BigFloat, prec: usize) -> Result<BigFloat, String> {
    let work = prec + GUARD + 8;
    let a = bf_exp(x, work)?;
    let b = bf_exp(&x.neg(), work)?;
    Ok(a.sub(&b, work).div(&BigFloat::from_u32(2, work), prec))
}

pub fn bf_cosh(x: &BigFloat, prec: usize) -> Result<BigFloat, String> {
    let work = prec + GUARD + 8;
    let a = bf_exp(x, work)?;
    let b = bf_exp(&x.neg(), work)?;
    Ok(a.add(&b, work).div(&BigFloat::from_u32(2, work), prec))
}

pub fn bf_tanh(x: &BigFloat, prec: usize) -> Result<BigFloat, String> {
    let work = prec + GUARD + 8;
    let e = bf_exp(&x.mul(&BigFloat::from_u32(2, work), work), work)?;
    Ok(e.sub(&BigFloat::one(work), work)
        .div(&e.add(&BigFloat::one(work), work), prec))
}

pub fn bf_pow(a: &BigFloat, b: &BigFloat, prec: usize) -> Result<BigFloat, String> {
    if a.is_zero() {
        if b.is_zero() {
            return Ok(BigFloat::one(prec));
        }
        if b.is_negative() {
            return Err("零不能做负数次幂".into());
        }
        return Ok(BigFloat::zero(prec));
    }
    if b.is_integer() {
        if let Some(e) = b.to_bigint().and_then(|v| v.to_i64()) {
            if let Some(result) = a.pow_int(e, prec) {
                return Ok(result);
            }
        }
    }
    if a.is_negative() {
        return Err("负数的非整数次幂在实数范围内无定义".into());
    }
    let work = prec + 10;
    Ok(bf_exp(&b.mul(&bf_ln(a, work)?, work), prec)?)
}

pub fn bf_log10(x: &BigFloat, prec: usize) -> Result<BigFloat, String> {
    let work = prec + 10;
    Ok(bf_ln(x, work)?.div(&bf_ln10(work), prec))
}

pub fn bf_log2(x: &BigFloat, prec: usize) -> Result<BigFloat, String> {
    let work = prec + 10;
    Ok(bf_ln(x, work)?.div(&bf_ln2(work), prec))
}

pub fn bf_from_rat_f64(value: &Rat, prec: usize) -> BigFloat {
    BigFloat::from_f64(rat_to_f64(value), prec).unwrap_or_else(|| BigFloat::zero(prec))
}
