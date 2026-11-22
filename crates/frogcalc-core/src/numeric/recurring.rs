//! 无限循环小数的精确解析。
//!
//! 支持形如 `0.1(3)`、`-0.0[3]`、`2.5` 的文本，其中括号（或方括号）内是循环节。
//! 结果直接构造为精确有理数，不经过浮点近似。

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Zero};

use super::Rat;

/// 数字总位数上限，避免极端输入耗尽内存。
const MAX_DIGITS: usize = 10_000;

/// 把循环小数文本解析为精确有理数。
///
/// 设整数部分为 `I`、非循环小数部分为 `N`（`n` 位）、循环节为 `R`（`m` 位），则
///
/// ```text
/// x = sign * (I + N/10^n + R/(10^n*(10^m-1)))
/// ```
pub fn rat_from_recurring(text: &str) -> Result<Rat, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("需要一个循环小数文本，例如 \"0.1(3)\"".into());
    }
    let (sign, body) = match trimmed.as_bytes()[0] {
        b'-' => (-1i32, &trimmed[1..]),
        b'+' => (1i32, &trimmed[1..]),
        _ => (1i32, trimmed),
    };
    let chars: Vec<char> = body.chars().collect();
    let mut index = 0usize;

    let mut integer_digits = String::new();
    while index < chars.len() && chars[index].is_ascii_digit() {
        integer_digits.push(chars[index]);
        index += 1;
    }

    let mut fraction_digits = String::new();
    let mut repeating_digits: Option<String> = None;
    let mut has_point = false;
    if index < chars.len() && chars[index] == '.' {
        has_point = true;
        index += 1;
        while index < chars.len() && chars[index].is_ascii_digit() {
            fraction_digits.push(chars[index]);
            index += 1;
        }
        if index < chars.len() && (chars[index] == '(' || chars[index] == '[') {
            let close = if chars[index] == '(' { ')' } else { ']' };
            let group_start = index;
            index += 1;
            let mut group = String::new();
            while index < chars.len() && chars[index] != close {
                if !chars[index].is_ascii_digit() {
                    return Err(format!(
                        "循环节中只能包含数字（第 {} 个字符）",
                        group_start + 2 + group.chars().count()
                    ));
                }
                group.push(chars[index]);
                index += 1;
            }
            if index >= chars.len() {
                return Err("循环节缺少右括号".into());
            }
            index += 1;
            if group.is_empty() {
                return Err("循环节不能为空".into());
            }
            repeating_digits = Some(group);
        }
    }

    if index < chars.len() {
        if repeating_digits.is_some() && chars[index].is_ascii_digit() {
            return Err("循环节之后不能再有数字".into());
        }
        if chars[index] == '(' || chars[index] == '[' {
            return Err("循环节必须写在小数点之后，例如 \"0.(3)\"".into());
        }
        return Err(format!(
            "无法识别的字符 “{}”（第 {} 个字符）",
            chars[index],
            index + 1
        ));
    }
    if integer_digits.is_empty()
        && fraction_digits.is_empty()
        && repeating_digits.is_none()
        && !has_point
    {
        return Err("需要形如 \"0.1(3)\" 的循环小数".into());
    }

    let total_digits = integer_digits.len()
        + fraction_digits.len()
        + repeating_digits
            .as_ref()
            .map(|digits| digits.len())
            .unwrap_or(0);
    if total_digits > MAX_DIGITS {
        return Err(format!("数字过长（上限 {MAX_DIGITS} 位）"));
    }

    let integer_value = parse_digits(&integer_digits);
    let non_repeating_value = parse_digits(&fraction_digits);
    let ten = BigInt::from(10u32);
    let place = ten.pow(fraction_digits.len() as u32);

    let value = match repeating_digits {
        None => BigRational::new(&integer_value * &place + &non_repeating_value, place),
        Some(repeating) => {
            let repeat_place = ten.pow(repeating.len() as u32);
            let repeat_base = &repeat_place - BigInt::one();
            let denominator = &place * &repeat_base;
            let numerator = &integer_value * &denominator
                + &non_repeating_value * &repeat_base
                + parse_digits(&repeating);
            BigRational::new(numerator, denominator)
        }
    };
    Ok(if sign < 0 { -value } else { value })
}

fn parse_digits(digits: &str) -> BigInt {
    if digits.is_empty() {
        return BigInt::zero();
    }
    BigInt::parse_bytes(digits.as_bytes(), 10).unwrap_or_else(BigInt::zero)
}
