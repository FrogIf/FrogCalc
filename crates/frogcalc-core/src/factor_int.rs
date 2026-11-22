//! 整系数一元多项式的 Berlekamp–Zassenhaus 因式分解。
//!
//! 流程：选择合适的小素数 p → 在 F_p 上用 Berlekamp 算法分解 → 用线性同余方程组
//! 逐位做 Hensel 提升到超过系数界 → 枚举模因子子集做 Zassenhaus 重组。
//! 只处理本原、平方自由（square-free）的多项式；调用方负责内容与重因子。

use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, ToPrimitive, Zero};

use crate::numeric::Rat;

const MAX_DEGREE: usize = 28;
const MAX_COEFF_BITS: u64 = 4096;
const MAX_RECOMBINE_ATTEMPTS: usize = 200_000;
const MAX_MOD_FACTORS: usize = 16;

const SMALL_PRIMES: [u64; 24] = [
    3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
];

/// 分解本原整系数多项式（系数低次到高次，内容为 1，首项为正）。
///
/// 返回若干本原因子，乘积等于输入（相差一个有理常数时同样可接受，调用方会校正内容）。
pub fn factor_squarefree_integer(coeffs: &[BigInt]) -> Option<Vec<Vec<BigInt>>> {
    let f = trim_big(coeffs);
    let degree = f.len().checked_sub(1)?;
    if degree <= 1 {
        return Some(vec![f]);
    }
    if degree > MAX_DEGREE {
        return None;
    }
    if f.iter().any(|c| c.bits() > MAX_COEFF_BITS) {
        return None;
    }
    let leading = f[degree].clone();
    if leading.is_zero() {
        return None;
    }
    let monic = if leading.is_one() {
        f.clone()
    } else {
        let mut g = Vec::with_capacity(degree + 1);
        for i in 0..degree {
            let mut coeff = f[i].clone();
            for _ in 0..(degree - 1 - i) {
                coeff *= &leading;
            }
            g.push(coeff);
        }
        g.push(BigInt::one());
        g
    };
    let monic_factors = factor_monic(&monic)?;
    let mut factors = Vec::new();
    if leading.is_one() {
        factors = monic_factors;
    } else {
        for h in monic_factors {
            let mut scaled = Vec::with_capacity(h.len());
            let mut power = BigInt::one();
            for coeff in &h {
                scaled.push(coeff * &power);
                power *= &leading;
            }
            factors.push(primitive_int(&scaled));
        }
    }
    if !product_matches(&f, &factors) {
        return None;
    }
    Some(factors)
}

/// 校验若干本原因子的乘积与目标多项式只相差一个有理常数，并据此接受结果。
fn product_matches(target: &[BigInt], factors: &[Vec<BigInt>]) -> bool {
    if factors.is_empty() {
        return false;
    }
    let mut product = vec![BigInt::one()];
    for factor in factors {
        product = mul_big(&product, factor);
    }
    if product.len() != target.len() {
        return false;
    }
    let target_lead = target.last().cloned().unwrap_or_else(BigInt::one);
    let product_lead = product.last().cloned().unwrap_or_else(BigInt::one);
    if product_lead.is_zero() {
        return false;
    }
    let ratio = Rat::new(target_lead, product_lead);
    product.iter().enumerate().all(|(i, coeff)| {
        let scaled = ratio.clone() * Rat::from_integer(coeff.clone());
        scaled == Rat::from_integer(target[i].clone())
    })
}

// ---------------------------------------------------------------------------
// 单变量：模 p 与 Hensel 提升
// ---------------------------------------------------------------------------

fn factor_monic(f: &[BigInt]) -> Option<Vec<Vec<BigInt>>> {
    let degree = f.len() - 1;
    if degree <= 1 {
        return Some(vec![f.to_vec()]);
    }
    let bound = coefficient_bound(f);
    let mut attempts = 0usize;
    for &p in &SMALL_PRIMES {
        let reduced = reduce_to_mod(f, p)?;
        if reduced.len() != degree + 1 || reduced[degree] != 1 {
            continue;
        }
        let derivative = derivative_u(&reduced, p);
        let gcd = gcd_u(&reduced, &derivative, p);
        if gcd.len() != 1 {
            continue;
        }
        let mod_factors = match berlekamp(&reduced, p) {
            Some(factors) => factors,
            None => continue,
        };
        if mod_factors.len() <= 1 {
            // 模 p 不可约 ⇒ 在 Z 上不可约。
            return Some(vec![f.to_vec()]);
        }
        if mod_factors.len() > MAX_MOD_FACTORS {
            continue;
        }
        let (lifted, modulus) = hensel_lift(f, &mod_factors, p, &bound)?;
        attempts += 1;
        if let Some(factors) = recombine(f, &lifted, &modulus, &mut attempts) {
            if factors.len() > 1 || (factors.len() == 1 && factors[0].len() < f.len()) {
                return Some(factors);
            }
        }
        if attempts > MAX_RECOMBINE_ATTEMPTS {
            // 重组预算耗尽：按未能分解处理，调用方会用旧路径兜底。
            return Some(vec![f.to_vec()]);
        }
        if attempts > MAX_RECOMBINE_ATTEMPTS * 2 {
            break;
        }
    }
    // 找不到分解时按不可约处理，调用方用旧路径（有理根 + 数值根）兜底。
    Some(vec![f.to_vec()])
}

fn coefficient_bound(f: &[BigInt]) -> BigInt {
    let degree = f.len() - 1;
    let max = f.iter().map(|c| c.abs()).max().unwrap_or_else(BigInt::one);
    let terms = BigInt::from(degree + 1);
    let sqrt_terms = integer_sqrt(&terms) + 1;
    // 2^n * ceil(sqrt(n+1)) * max|a_i| 是因子系数的安全上界。
    (max * sqrt_terms * (BigInt::one() << degree)) + 1
}

fn integer_sqrt(value: &BigInt) -> BigInt {
    if value <= &BigInt::one() {
        return value.clone();
    }
    let mut x = value.clone();
    let mut y = (&x + value / &x) >> 1;
    while y < x {
        x = y;
        y = (&x + value / &x) >> 1;
    }
    x
}

// ---------------------------------------------------------------------------
// F_p 上的多项式
// ---------------------------------------------------------------------------

fn trim_u(poly: &mut Vec<u64>, p: u64) {
    while poly.last().map(|c| *c == 0).unwrap_or(false) {
        poly.pop();
    }
    let _ = p;
}

fn normalize_u(mut poly: Vec<u64>, p: u64) -> Vec<u64> {
    for coeff in poly.iter_mut() {
        *coeff %= p;
    }
    trim_u(&mut poly, p);
    poly
}

fn poly_mul_u(a: &[u64], b: &[u64], p: u64) -> Vec<u64> {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    let mut out = vec![0u64; a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() {
        if *x == 0 {
            continue;
        }
        for (j, y) in b.iter().enumerate() {
            out[i + j] = (out[i + j] + mulmod(*x, *y, p)) % p;
        }
    }
    normalize_u(out, p)
}

fn poly_divmod_u(a: &[u64], b: &[u64], p: u64) -> (Vec<u64>, Vec<u64>) {
    let mut remainder = normalize_u(a.to_vec(), p);
    let divisor = normalize_u(b.to_vec(), p);
    if divisor.is_empty() {
        return (Vec::new(), remainder);
    }
    let db = divisor.len() - 1;
    let lead_inv = mod_inverse(divisor[db], p).unwrap_or(0);
    let mut quotient = vec![0u64; remainder.len().max(db + 1)];
    while remainder.len() >= divisor.len() && !remainder.is_empty() {
        let dr = remainder.len() - 1;
        let shift = dr - db;
        let factor = mulmod(remainder[dr], lead_inv, p);
        if factor == 0 {
            break;
        }
        quotient[shift] = factor;
        for i in 0..=db {
            let sub = mulmod(divisor[i], factor, p);
            remainder[i + shift] = (remainder[i + shift] + p - sub) % p;
        }
        trim_u(&mut remainder, p);
    }
    (normalize_u(quotient, p), remainder)
}

fn poly_mulmod(a: &[u64], b: &[u64], modulus: &[u64], p: u64) -> Vec<u64> {
    let product = poly_mul_u(a, b, p);
    poly_divmod_u(&product, modulus, p).1
}

fn poly_powmod(base: &[u64], exp: u64, modulus: &[u64], p: u64) -> Vec<u64> {
    let mut result = vec![1u64];
    let mut factor = normalize_u(base.to_vec(), p);
    let mut e = exp;
    while e > 0 {
        if e & 1 == 1 {
            result = poly_mulmod(&result, &factor, modulus, p);
        }
        factor = poly_mulmod(&factor, &factor, modulus, p);
        e >>= 1;
    }
    result
}

fn monic_u(mut poly: Vec<u64>, p: u64) -> Vec<u64> {
    poly = normalize_u(poly, p);
    let Some(&lead) = poly.last() else {
        return poly;
    };
    if lead == 0 || lead == 1 {
        return poly;
    }
    let inv = mod_inverse(lead, p).unwrap_or(1);
    for coeff in poly.iter_mut() {
        *coeff = mulmod(*coeff, inv, p);
    }
    poly
}

fn gcd_u(a: &[u64], b: &[u64], p: u64) -> Vec<u64> {
    let mut x = normalize_u(a.to_vec(), p);
    let mut y = normalize_u(b.to_vec(), p);
    while !y.is_empty() {
        let (_, r) = poly_divmod_u(&x, &y, p);
        x = y;
        y = r;
    }
    monic_u(x, p)
}

fn derivative_u(a: &[u64], p: u64) -> Vec<u64> {
    if a.len() <= 1 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(a.len() - 1);
    for (i, coeff) in a.iter().enumerate().skip(1) {
        out.push(mulmod(*coeff, (i as u64) % p, p));
    }
    normalize_u(out, p)
}

fn mulmod(a: u64, b: u64, p: u64) -> u64 {
    ((a as u128 * b as u128) % p as u128) as u64
}

fn mod_inverse(value: u64, p: u64) -> Option<u64> {
    let value = value % p;
    if value == 0 {
        return None;
    }
    let (mut old_r, mut r) = (p as i128, value as i128);
    let (mut old_s, mut s) = (0i128, 1i128);
    while r != 0 {
        let q = old_r / r;
        let next_r = old_r - q * r;
        old_r = r;
        r = next_r;
        let next_s = old_s - q * s;
        old_s = s;
        s = next_s;
    }
    if old_r != 1 {
        return None;
    }
    let mut result = old_s % p as i128;
    if result < 0 {
        result += p as i128;
    }
    Some(result as u64)
}

/// Berlekamp：返回模 p 下首一不可约因子。
fn berlekamp(f: &[u64], p: u64) -> Option<Vec<Vec<u64>>> {
    let mut out = Vec::new();
    let mut queue = vec![normalize_u(f.to_vec(), p)];
    let mut guard = 0;
    while let Some(poly) = queue.pop() {
        guard += 1;
        if guard > 500 {
            return None;
        }
        let degree = poly.len().saturating_sub(1);
        if degree <= 1 {
            out.push(poly);
            continue;
        }
        let basis = berlekamp_basis(&poly, p)?;
        if basis.len() <= 1 {
            out.push(poly);
            continue;
        }
        if let Some((left, right)) = split_with_basis(&poly, &basis, p)? {
            queue.push(left);
            queue.push(right);
        } else {
            out.push(poly);
        }
    }
    Some(out)
}

fn berlekamp_basis(f: &[u64], p: u64) -> Option<Vec<Vec<u64>>> {
    let degree = f.len() - 1;
    let x = vec![0u64, 1u64];
    let xp = poly_powmod(&x, p, f, p);
    let mut powers = Vec::with_capacity(degree);
    let mut current = vec![1u64];
    for _ in 0..degree {
        powers.push(current.clone());
        current = poly_mulmod(&current, &xp, f, p);
    }
    // 矩阵 M 的第 j 列是 x^(p*j) mod f 的系数；A = M - I。
    let mut matrix = vec![vec![0u64; degree]; degree];
    for (j, power) in powers.iter().enumerate() {
        for i in 0..degree {
            let value = power.get(i).copied().unwrap_or(0);
            matrix[i][j] = if i == j { (value + p - 1) % p } else { value };
        }
    }
    Some(nullspace_mod(matrix, p))
}

fn nullspace_mod(mut matrix: Vec<Vec<u64>>, p: u64) -> Vec<Vec<u64>> {
    let n = matrix.len();
    let mut pivot_row = vec![usize::MAX; n];
    let mut row = 0usize;
    for col in 0..n {
        let Some(found) = (row..n).find(|r| matrix[*r][col] % p != 0) else {
            continue;
        };
        matrix.swap(row, found);
        let inv = mod_inverse(matrix[row][col], p).unwrap_or(1);
        for j in col..n {
            matrix[row][j] = mulmod(matrix[row][j], inv, p);
        }
        for r in 0..n {
            if r == row {
                continue;
            }
            let factor = matrix[r][col] % p;
            if factor == 0 {
                continue;
            }
            for j in col..n {
                let sub = mulmod(factor, matrix[row][j], p);
                matrix[r][j] = (matrix[r][j] + p - sub) % p;
            }
        }
        pivot_row[col] = row;
        row += 1;
        if row == n {
            break;
        }
    }
    let mut basis = Vec::new();
    let free: Vec<usize> = (0..n).filter(|c| pivot_row[*c] == usize::MAX).collect();
    for free_col in free {
        let mut vector = vec![0u64; n];
        vector[free_col] = 1;
        for col in 0..n {
            let pivot = pivot_row[col];
            if pivot == usize::MAX {
                continue;
            }
            vector[col] = (p - matrix[pivot][free_col] % p) % p;
        }
        basis.push(vector);
    }
    basis
}

fn basis_poly(vector: &[u64]) -> Vec<u64> {
    let mut poly = vector.to_vec();
    while poly.last().map(|c| *c == 0).unwrap_or(false) {
        poly.pop();
    }
    poly
}

fn split_with_basis(f: &[u64], basis: &[Vec<u64>], p: u64) -> Option<Option<(Vec<u64>, Vec<u64>)>> {
    for vector in basis {
        let candidate = basis_poly(vector);
        if candidate.len() <= 1 {
            continue;
        }
        for s in 0..p {
            let mut shifted = candidate.clone();
            shifted[0] = (shifted[0] + p - s) % p;
            let divisor = gcd_u(f, &shifted, p);
            if divisor.len() > 1 && divisor.len() < f.len() {
                let (quotient, remainder) = poly_divmod_u(f, &divisor, p);
                if remainder.is_empty() && quotient.len() > 1 {
                    return Some(Some((divisor, quotient)));
                }
            }
        }
    }
    // 组合多个基向量提高成功率。
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    for _ in 0..24 {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let mut vector = vec![0u64; f.len() - 1];
        for (i, base) in basis.iter().enumerate() {
            let weight = (seed >> (i % 32)) % p;
            for (j, value) in base.iter().enumerate() {
                vector[j] = (vector[j] + mulmod(*value, weight, p)) % p;
            }
        }
        let candidate = basis_poly(&vector);
        if candidate.len() <= 1 {
            continue;
        }
        for s in 0..p {
            let mut shifted = candidate.clone();
            shifted[0] = (shifted[0] + p - s) % p;
            let divisor = gcd_u(f, &shifted, p);
            if divisor.len() > 1 && divisor.len() < f.len() {
                let (quotient, remainder) = poly_divmod_u(f, &divisor, p);
                if remainder.is_empty() && quotient.len() > 1 {
                    return Some(Some((divisor, quotient)));
                }
            }
        }
    }
    Some(None)
}

fn reduce_to_mod(f: &[BigInt], p: u64) -> Option<Vec<u64>> {
    let modulus = BigInt::from(p);
    let mut out = Vec::with_capacity(f.len());
    for coeff in f {
        let value = coeff.mod_floor(&modulus).to_u64()?;
        out.push(value % p);
    }
    Some(normalize_u(out, p))
}

// ---------------------------------------------------------------------------
// Hensel 提升（用线性同余方程组逐位求解修正量，避免多项式扩展欧几里得）
// ---------------------------------------------------------------------------

fn hensel_lift(
    f: &[BigInt],
    mod_factors: &[Vec<u64>],
    p: u64,
    bound: &BigInt,
) -> Option<(Vec<Vec<BigInt>>, BigInt)> {
    let degree = f.len() - 1;
    let target = bound * BigInt::from(2u32);
    let mut modulus = BigInt::from(p);
    let mut factors: Vec<Vec<BigInt>> = mod_factors
        .iter()
        .map(|poly| poly.iter().map(|c| BigInt::from(*c)).collect())
        .collect();
    let mut rounds = 0usize;
    while modulus <= target {
        rounds += 1;
        if rounds > 64 {
            return None;
        }
        let mut product = vec![BigInt::one()];
        for factor in &factors {
            product = mul_big(&product, factor);
        }
        if product.len() != f.len() {
            return None;
        }
        let diff = sub_big(f, &product);
        let correction = div_exact_big(&diff, &modulus)?;
        let correction = reduce_big(&correction, &modulus);

        let mut cofactors = Vec::with_capacity(factors.len());
        for i in 0..factors.len() {
            let mut cofactor = vec![BigInt::one()];
            for (j, factor) in factors.iter().enumerate() {
                if i != j {
                    cofactor = mul_big(&cofactor, factor);
                }
            }
            cofactors.push(reduce_big(&cofactor, &modulus));
        }

        let mut matrix = vec![vec![BigInt::zero(); degree]; degree];
        let mut columns = Vec::with_capacity(degree);
        let mut column = 0usize;
        for (i, cofactor) in cofactors.iter().enumerate() {
            let width = factors[i].len() - 1;
            for shift in 0..width {
                for row in 0..degree {
                    if row >= shift {
                        let coeff = cofactor
                            .get(row - shift)
                            .cloned()
                            .unwrap_or_else(BigInt::zero);
                        matrix[row][column] = mod_value(&coeff, &modulus);
                    }
                }
                columns.push((i, shift));
                column += 1;
            }
        }
        if column != degree {
            return None;
        }
        let mut rhs = vec![BigInt::zero(); degree];
        for (i, value) in correction.iter().enumerate().take(degree) {
            rhs[i] = mod_value(value, &modulus);
        }
        let solution = solve_linear_mod(matrix, rhs, &modulus)?;
        for (index, (factor_index, shift)) in columns.iter().enumerate() {
            let delta = &solution[index];
            if delta.is_zero() {
                continue;
            }
            let factor = &mut factors[*factor_index];
            while factor.len() <= *shift {
                factor.push(BigInt::zero());
            }
            factor[*shift] += &modulus * delta;
        }
        modulus = &modulus * &modulus;
        factors = factors
            .iter()
            .map(|factor| reduce_big(factor, &modulus))
            .collect();
    }
    Some((factors, modulus))
}

fn is_unit(value: &BigInt, modulus: &BigInt) -> bool {
    !value.is_zero() && value.gcd(modulus).is_one()
}

fn mod_value(value: &BigInt, modulus: &BigInt) -> BigInt {
    value.mod_floor(modulus)
}

fn reduce_big(poly: &[BigInt], modulus: &BigInt) -> Vec<BigInt> {
    trim_big(
        &poly
            .iter()
            .map(|c| mod_value(c, modulus))
            .collect::<Vec<_>>(),
    )
}

fn solve_linear_mod(
    mut matrix: Vec<Vec<BigInt>>,
    mut rhs: Vec<BigInt>,
    modulus: &BigInt,
) -> Option<Vec<BigInt>> {
    let n = rhs.len();
    if matrix.len() != n || matrix.iter().any(|row| row.len() != n) {
        return None;
    }
    for col in 0..n {
        let pivot = (col..n).find(|row| is_unit(&matrix[*row][col], modulus))?;
        matrix.swap(col, pivot);
        rhs.swap(col, pivot);
        let inverse = mod_inverse_big(&matrix[col][col], modulus)?;
        for j in col..n {
            matrix[col][j] = mod_value(&(&matrix[col][j] * &inverse), modulus);
        }
        rhs[col] = mod_value(&(&rhs[col] * &inverse), modulus);
        for row in 0..n {
            if row == col {
                continue;
            }
            let factor = matrix[row][col].clone();
            if factor.is_zero() {
                continue;
            }
            for j in col..n {
                let sub = mod_value(&(&factor * &matrix[col][j]), modulus);
                let value = mod_value(&(&matrix[row][j] + modulus - sub), modulus);
                matrix[row][j] = value;
            }
            let sub = mod_value(&(&factor * &rhs[col]), modulus);
            rhs[row] = mod_value(&(&rhs[row] + modulus - sub), modulus);
        }
    }
    Some(rhs)
}

fn mod_inverse_big(value: &BigInt, modulus: &BigInt) -> Option<BigInt> {
    let (mut old_r, mut r) = (modulus.clone(), mod_value(value, modulus));
    let (mut old_s, mut s) = (BigInt::zero(), BigInt::one());
    while !r.is_zero() {
        let q = &old_r / &r;
        let next_r = &old_r - &q * &r;
        old_r = r;
        r = next_r;
        let next_s = &old_s - &q * &s;
        old_s = s;
        s = next_s;
    }
    if !old_r.is_one() {
        return None;
    }
    Some(mod_value(&old_s, modulus))
}

// ---------------------------------------------------------------------------
// Zassenhaus 重组
// ---------------------------------------------------------------------------

fn recombine(
    f: &[BigInt],
    lifted: &[Vec<BigInt>],
    modulus: &BigInt,
    attempts: &mut usize,
) -> Option<Vec<Vec<BigInt>>> {
    let symmetric: Vec<Vec<BigInt>> = lifted
        .iter()
        .map(|poly| symmetrize(poly, modulus))
        .collect();
    let mut remaining: Vec<usize> = (0..symmetric.len()).collect();
    let mut target = f.to_vec();
    let mut found_factors: Vec<Vec<BigInt>> = Vec::new();
    loop {
        let target_degree = target.len().saturating_sub(1);
        if target_degree == 0 {
            break;
        }
        // 枚举所有真子集：真因子一定对应模因子的某个子集，其补集也对应另一个子集，
        // 因此只有单个剩余因子时才跳过。
        let max_size = remaining.len().saturating_sub(1);
        let mut found = None;
        'search: for size in 1..=max_size {
            let combinations = subset_combinations(remaining.len(), size);
            for combo in combinations {
                *attempts += 1;
                if *attempts > MAX_RECOMBINE_ATTEMPTS {
                    return None;
                }
                let mut candidate = vec![BigInt::one()];
                for &index in &combo {
                    candidate = mul_big(&candidate, &symmetric[remaining[index]]);
                }
                // 子集乘积只是“模 m 意义下”等于真因子，必须先折回对称区间再试除。
                let candidate = primitive_int(&symmetric_poly(&candidate, &modulus));
                if candidate.len() - 1 > target_degree {
                    continue;
                }
                if let Some(quotient) = divide_exact(&target, &candidate) {
                    found = Some((combo, candidate, quotient));
                    break 'search;
                }
            }
        }
        match found {
            Some((combo, factor, quotient)) => {
                found_factors.push(factor);
                target = quotient;
                let mut index = 0usize;
                remaining.retain(|_| {
                    let keep = !combo.contains(&index);
                    index += 1;
                    keep
                });
            }
            None => {
                break;
            }
        }
    }
    if target.len() > 1 {
        found_factors.push(target);
    }
    if found_factors.is_empty() {
        return None;
    }
    Some(found_factors)
}

fn symmetrize(poly: &[BigInt], modulus: &BigInt) -> Vec<BigInt> {
    let half = modulus / BigInt::from(2u32);
    poly.iter()
        .map(|coeff| {
            let value = mod_value(coeff, modulus);
            if value > half {
                value - modulus
            } else {
                value
            }
        })
        .collect()
}

fn symmetric_poly(poly: &[BigInt], modulus: &BigInt) -> Vec<BigInt> {
    trim_big(&symmetrize(poly, modulus))
}

fn subset_combinations(count: usize, size: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    if size == 0 || size > count {
        return out;
    }
    let mut indices: Vec<usize> = (0..size).collect();
    loop {
        out.push(indices.clone());
        if out.len() > 500_000 {
            break;
        }
        let mut i = size;
        loop {
            if i == 0 {
                return out;
            }
            i -= 1;
            if indices[i] != i + count - size {
                indices[i] += 1;
                for j in (i + 1)..size {
                    indices[j] = indices[j - 1] + 1;
                }
                break;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// BigInt 多项式工具
// ---------------------------------------------------------------------------

fn trim_big(poly: &[BigInt]) -> Vec<BigInt> {
    let mut out = poly.to_vec();
    while out.last().map(|c| c.is_zero()).unwrap_or(false) {
        out.pop();
    }
    out
}

fn mul_big(a: &[BigInt], b: &[BigInt]) -> Vec<BigInt> {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    let mut out = vec![BigInt::zero(); a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() {
        if x.is_zero() {
            continue;
        }
        for (j, y) in b.iter().enumerate() {
            out[i + j] += x * y;
        }
    }
    trim_big(&out)
}

fn sub_big(a: &[BigInt], b: &[BigInt]) -> Vec<BigInt> {
    let n = a.len().max(b.len());
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let x = a.get(i).cloned().unwrap_or_else(BigInt::zero);
        let y = b.get(i).cloned().unwrap_or_else(BigInt::zero);
        out.push(x - y);
    }
    trim_big(&out)
}

fn div_exact_big(a: &[BigInt], divisor: &BigInt) -> Option<Vec<BigInt>> {
    let mut out = Vec::with_capacity(a.len());
    for coeff in a {
        let (quotient, remainder) = coeff.div_rem(divisor);
        if !remainder.is_zero() {
            return None;
        }
        out.push(quotient);
    }
    Some(trim_big(&out))
}

/// 首一除式的整除：返回商，余式非零时返回 `None`。
fn divide_exact(target: &[BigInt], divisor: &[BigInt]) -> Option<Vec<BigInt>> {
    let divisor = trim_big(divisor);
    let lead = divisor.last()?;
    if !lead.is_one() {
        return None;
    }
    let db = divisor.len() - 1;
    let mut remainder = trim_big(target);
    let mut quotient = vec![BigInt::zero(); remainder.len().saturating_sub(db).max(1)];
    while remainder.len() > db {
        let dr = remainder.len() - 1;
        let shift = dr - db;
        let factor = remainder[dr].clone();
        quotient[shift] = factor.clone();
        for i in 0..=db {
            remainder[i + shift] -= &factor * &divisor[i];
        }
        remainder = trim_big(&remainder);
    }
    if remainder.is_empty() {
        Some(trim_big(&quotient))
    } else {
        None
    }
}

fn primitive_int(poly: &[BigInt]) -> Vec<BigInt> {
    let mut out = trim_big(poly);
    if out.is_empty() {
        return out;
    }
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
    out
}
