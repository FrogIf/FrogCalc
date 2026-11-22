pub mod bigfloat;
pub mod recurring;

pub use bigfloat::BigFloat;
pub use num_bigint::{BigInt, BigUint, Sign};
pub use num_rational::BigRational;
pub use num_traits::{One, Signed, ToPrimitive, Zero};
pub use recurring::rat_from_recurring;

use num_integer::Integer;

pub type Rat = BigRational;

pub fn rat(n: i64, d: i64) -> Rat {
    BigRational::new(BigInt::from(n), BigInt::from(d))
}

pub fn rat_int(n: impl Into<BigInt>) -> Rat {
    BigRational::from_integer(n.into())
}

pub fn rat_from_str(text: &str) -> Option<Rat> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let (sign, rest) = match text.as_bytes()[0] {
        b'-' => (-1i32, &text[1..]),
        b'+' => (1i32, &text[1..]),
        _ => (1i32, text),
    };
    let (mantissa, exp) = match rest.find(['e', 'E']) {
        Some(i) => (&rest[..i], rest[i + 1..].parse::<i64>().ok()?),
        None => (rest, 0i64),
    };
    let (int_part, frac_part) = match mantissa.find('.') {
        Some(i) => (&mantissa[..i], &mantissa[i + 1..]),
        None => (mantissa, ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    let digits: String = format!("{int_part}{frac_part}");
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut numerator = BigInt::parse_bytes(digits.as_bytes(), 10)?;
    if sign < 0 {
        numerator = -numerator;
    }
    let pow = exp - frac_part.len() as i64;
    if pow >= 0 {
        Some(BigRational::from_integer(
            numerator * BigInt::from(10u32).pow(pow as u32),
        ))
    } else {
        Some(BigRational::new(
            numerator,
            BigInt::from(10u32).pow((-pow) as u32),
        ))
    }
}

pub fn bigint_gcd(a: &BigInt, b: &BigInt) -> BigInt {
    a.gcd(b)
}

pub fn rational_gcd(a: &Rat, b: &Rat) -> Rat {
    if a.is_zero() {
        return b.abs();
    }
    if b.is_zero() {
        return a.abs();
    }
    let num = a.numer().gcd(b.numer());
    let den = a.denom().lcm(b.denom());
    BigRational::new(num, den)
}

pub fn floor_rat(value: &Rat) -> BigInt {
    value.numer().div_floor(value.denom())
}

pub fn ceil_rat(value: &Rat) -> BigInt {
    value.numer().div_ceil(value.denom())
}

pub fn round_rat(value: &Rat) -> BigInt {
    let abs = value.abs();
    let floor = floor_rat(&abs);
    let frac = &abs - BigRational::from_integer(floor.clone());
    let rounded = if frac >= rat(1, 2) { floor + 1 } else { floor };
    if value.is_negative() {
        -rounded
    } else {
        rounded
    }
}

pub fn rat_to_f64(value: &Rat) -> f64 {
    if let (Some(n), Some(d)) = (value.numer().to_f64(), value.denom().to_f64()) {
        if d != 0.0 {
            let v = n / d;
            if v.is_finite() {
                return v;
            }
        }
    }
    let sign = if value.is_negative() { -1.0 } else { 1.0 };
    let n = value.numer().abs().to_str_radix(10);
    let d = value.denom().to_str_radix(10);
    let order = n.len() as i64 - d.len() as i64;
    let n_head: f64 = n[..n.len().min(17)].parse().unwrap_or(1.0);
    let d_head: f64 = d[..d.len().min(17)].parse().unwrap_or(1.0);
    let n_scale = 10f64.powi(n.len().min(17) as i32 - 1);
    let d_scale = 10f64.powi(d.len().min(17) as i32 - 1);
    sign * (n_head / n_scale) / (d_head / d_scale) * 10f64.powi(order as i32)
}

pub fn rat_pow_int(base: &Rat, exp: i64) -> Option<Rat> {
    if exp.unsigned_abs() > 100_000 {
        return None;
    }
    if base.is_zero() {
        if exp < 0 {
            return None;
        }
        return Some(Rat::zero());
    }
    let negative = exp < 0;
    let mut e = exp.unsigned_abs();
    let mut factor = base.clone();
    let mut result = Rat::one();
    while e > 0 {
        if e & 1 == 1 {
            result *= &factor;
        }
        factor = &factor * &factor;
        e >>= 1;
    }
    if negative {
        Some(result.recip())
    } else {
        Some(result)
    }
}

pub fn rat_is_integer(value: &Rat) -> bool {
    value.is_integer()
}

pub fn rat_to_bigint(value: &Rat) -> Option<BigInt> {
    if value.is_integer() {
        Some(value.numer().clone())
    } else {
        None
    }
}

pub fn rat_display(value: &Rat) -> String {
    if value.is_integer() {
        value.numer().to_string()
    } else {
        format!("{}/{}", value.numer(), value.denom())
    }
}

/// 若有理数有不超过 max_digits 位小数的精确十进制表示，返回该十进制字符串。
pub fn rat_exact_decimal(value: &Rat, max_digits: usize) -> Option<String> {
    if value.is_integer() {
        return Some(value.numer().to_string());
    }
    let numer = value.numer().abs();
    let mut denom = value.denom().clone();
    let mut twos = 0usize;
    let mut fives = 0usize;
    let two = BigInt::from(2u32);
    let five = BigInt::from(5u32);
    while (&denom % &two).is_zero() {
        denom /= &two;
        twos += 1;
    }
    while (&denom % &five).is_zero() {
        denom /= &five;
        fives += 1;
    }
    if denom != BigInt::one() {
        return None;
    }
    let digits = twos.max(fives);
    if digits > max_digits {
        return None;
    }
    let scale = BigInt::from(10u32).pow(digits as u32);
    let scaled = (numer * scale) / value.denom();
    let mut s = scaled.to_string();
    if s.len() <= digits {
        s = format!("{}{}", "0".repeat(digits + 1 - s.len()), s);
    }
    let point = s.len() - digits;
    let int_part = &s[..point];
    let frac_part = s[point..].trim_end_matches('0');
    let body = if frac_part.is_empty() {
        int_part.to_string()
    } else {
        format!("{int_part}.{frac_part}")
    };
    if value.is_negative() {
        Some(format!("-{body}"))
    } else {
        Some(body)
    }
}

pub fn bigint_pow(base: &BigInt, exp: u32) -> BigInt {
    let mut result = BigInt::one();
    let mut factor = base.clone();
    let mut e = exp;
    while e > 0 {
        if e & 1 == 1 {
            result *= &factor;
        }
        factor = &factor * &factor;
        e >>= 1;
    }
    result
}
