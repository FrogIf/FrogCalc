const PREC_EQ: i32 = 5;
const PREC_ADD: i32 = 10;
const PREC_MUL: i32 = 20;
const PREC_POW: i32 = 30;

fn sign_of(expr: &Expr) -> i8 {
    match expr {
        Expr::Num(v) => {
            if v.is_negative() {
                -1
            } else {
                1
            }
        }
        Expr::Mul(factors) => {
            for factor in factors {
                if let Expr::Num(v) = factor {
                    return if v.is_negative() { -1 } else { 1 };
                }
            }
            1
        }
        _ => 1,
    }
}

fn abs_expr(expr: &Expr) -> Expr {
    match expr {
        Expr::Num(v) => num(v.abs()),
        Expr::Mul(factors) => mul(
            factors
                .iter()
                .map(|f| match f {
                    Expr::Num(v) => num(v.abs()),
                    other => other.clone(),
                })
                .collect(),
        ),
        _ => expr.clone(),
    }
}

fn split_sign(expr: &Expr) -> (i8, Expr) {
    let sign = sign_of(expr);
    if sign < 0 {
        (sign, abs_expr(expr))
    } else {
        (sign, expr.clone())
    }
}

fn paren(text: String, need: bool, latex: bool) -> String {
    if !need {
        text
    } else if latex {
        format!("\\left({text}\\right)")
    } else {
        format!("({text})")
    }
}

fn is_negative_power(expr: &Expr) -> bool {
    matches!(expr, Expr::Pow(_, exp) if matches!(&**exp, Expr::Num(v) if v.is_negative()))
}

fn positive_power(expr: &Expr) -> Expr {
    match expr {
        Expr::Pow(base, exp) => match &**exp {
            Expr::Num(v) => raw_pow((**base).clone(), num(v.abs())),
            _ => expr.clone(),
        },
        _ => expr.clone(),
    }
}

fn plain(expr: &Expr, parent: i32) -> String {
    if let Some(text) = complex_plain(expr, parent) {
        return text;
    }
    match expr {
        Expr::Num(v) => rat_display(v),
        Expr::Float(v) => v.to_string(),
        Expr::Sym(name) => name.clone(),
        Expr::Str(text) => format!("{text:?}"),
        Expr::Add(terms) => {
            let mut out = String::new();
            for (i, term) in terms.iter().enumerate() {
                let (sign, body) = split_sign(term);
                if i == 0 {
                    if sign < 0 {
                        out.push('-');
                    }
                } else if sign < 0 {
                    out.push_str(" - ");
                } else {
                    out.push_str(" + ");
                }
                out.push_str(&plain(&body, PREC_ADD));
            }
            if out.is_empty() {
                out.push('0');
            }
            paren(out, PREC_ADD < parent, false)
        }
        Expr::Mul(factors) => paren(plain_mul(factors), PREC_MUL < parent, false),
        Expr::Pow(base, exp) => paren(plain_pow(base, exp), PREC_POW < parent, false),
        Expr::Call(name, args) => format!(
            "{name}({})",
            args.iter()
                .map(|a| plain(a, 0))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Eq(lhs, rhs) => paren(
            format!("{} = {}", plain(lhs, 0), plain(rhs, 0)),
            PREC_EQ < parent,
            false,
        ),
        Expr::List(items) => format!(
            "{{{}}}",
            items
                .iter()
                .map(|i| plain(i, 0))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn plain_mul(factors: &[Expr]) -> String {
    let mut coeff: Option<&Expr> = None;
    let mut numerator = Vec::new();
    let mut denominator = Vec::new();
    for factor in factors {
        if matches!(factor, Expr::Num(_) | Expr::Float(_)) && coeff.is_none() {
            coeff = Some(factor);
            continue;
        }
        if is_negative_power(factor) {
            denominator.push(positive_power(factor));
        } else {
            numerator.push(factor.clone());
        }
    }
    let factor_text = |items: &[Expr]| {
        if items.is_empty() {
            "1".to_string()
        } else {
            items
                .iter()
                .map(|i| plain(i, PREC_MUL))
                .collect::<Vec<_>>()
                .join("*")
        }
    };
    let mut numerator_text = factor_text(&numerator);
    if let Some(coeff_expr) = coeff {
        let is_one = match coeff_expr {
            Expr::Num(v) => v.is_one(),
            Expr::Float(v) => v.digits == BigUint::one() && v.exp == 0 && !v.is_negative(),
            _ => false,
        };
        let is_minus_one = match coeff_expr {
            Expr::Num(v) => v == &rat(-1, 1),
            Expr::Float(v) => v.digits == BigUint::one() && v.exp == 0 && v.is_negative(),
            _ => false,
        };
        if is_minus_one {
            numerator_text = format!("-{numerator_text}");
        } else if !is_one {
            numerator_text = format!("{}*{}", plain(coeff_expr, PREC_MUL), numerator_text);
        }
    }
    if denominator.is_empty() {
        numerator_text
    } else {
        let denom_text = factor_text(&denominator);
        format!(
            "{}/{}",
            paren(numerator_text.clone(), numerator_text.starts_with('-'), false),
            paren(denom_text, denominator.len() > 1, false)
        )
    }
}

fn plain_pow(base: &Expr, exp: &Expr) -> String {
    let b = paren(
        plain(base, 0),
        matches!(base, Expr::Add(_) | Expr::Mul(_) | Expr::Pow(_, _)),
        false,
    );
    if let Expr::Num(v) = exp {
        if v.is_one() {
            return b;
        }
        if v == &rat(1, 2) {
            return format!("sqrt({})", plain(base, 0));
        }
        if v == &rat(1, 3) {
            return format!("cbrt({})", plain(base, 0));
        }
        if v == &rat(-1, 1) {
            return format!(
                "1/{}",
                paren(
                    plain(base, 0),
                    !matches!(base, Expr::Sym(_) | Expr::Num(_)),
                    false
                )
            );
        }
        if v == &rat(-1, 2) {
            return format!("1/sqrt({})", plain(base, 0));
        }
    }
    let e = paren(
        plain(exp, 0),
        matches!(exp, Expr::Add(_) | Expr::Mul(_)),
        false,
    );
    format!("{b}^{e}")
}

fn unicode(expr: &Expr, parent: i32) -> String {
    if let Some(text) = complex_unicode(expr, parent) {
        return text;
    }
    match expr {
        Expr::Num(v) => rat_display(v),
        Expr::Float(v) => v.to_string(),
        Expr::Sym(name) => match name.as_str() {
            "pi" => "π".to_string(),
            "e" => "e".to_string(),
            "i" => "i".to_string(),
            _ => name.clone(),
        },
        Expr::Str(text) => text.clone(),
        Expr::Add(terms) => {
            let mut out = String::new();
            for (i, term) in terms.iter().enumerate() {
                let (sign, body) = split_sign(term);
                if i == 0 {
                    if sign < 0 {
                        out.push('-');
                    }
                } else if sign < 0 {
                    out.push_str(" − ");
                } else {
                    out.push_str(" + ");
                }
                out.push_str(&unicode(&body, PREC_ADD));
            }
            if out.is_empty() {
                out.push('0');
            }
            paren(out, PREC_ADD < parent, false)
        }
        Expr::Mul(factors) => paren(unicode_mul(factors), PREC_MUL < parent, false),
        Expr::Pow(base, exp) => paren(unicode_pow(base, exp), PREC_POW < parent, false),
        Expr::Call(name, args) => format!(
            "{name}({})",
            args.iter()
                .map(|a| unicode(a, 0))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Eq(lhs, rhs) => format!("{} = {}", unicode(lhs, 0), unicode(rhs, 0)),
        Expr::List(items) => format!(
            "{{{}}}",
            items
                .iter()
                .map(|i| unicode(i, 0))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn unicode_mul(factors: &[Expr]) -> String {
    let mut coeff: Option<&Expr> = None;
    let mut numerator = Vec::new();
    let mut denominator = Vec::new();
    for factor in factors {
        if matches!(factor, Expr::Num(_) | Expr::Float(_)) && coeff.is_none() {
            coeff = Some(factor);
            continue;
        }
        if is_negative_power(factor) {
            denominator.push(positive_power(factor));
        } else {
            numerator.push(factor.clone());
        }
    }
    let join = |items: &[Expr]| {
        if items.is_empty() {
            "1".to_string()
        } else {
            items
                .iter()
                .map(|i| unicode(i, PREC_MUL))
                .collect::<Vec<_>>()
                .join("·")
        }
    };
    let mut num_text = join(&numerator);
    if let Some(c) = coeff {
        let is_one = match c {
            Expr::Num(v) => v.is_one(),
            Expr::Float(v) => v.digits == BigUint::one() && v.exp == 0 && !v.is_negative(),
            _ => false,
        };
        let is_minus_one = match c {
            Expr::Num(v) => v == &rat(-1, 1),
            Expr::Float(v) => v.digits == BigUint::one() && v.exp == 0 && v.is_negative(),
            _ => false,
        };
        if is_minus_one {
            num_text = format!("-{num_text}");
        } else if !is_one {
            num_text = format!("{}·{}", unicode(c, PREC_MUL), num_text);
        }
    }
    if denominator.is_empty() {
        num_text
    } else {
        format!(
            "{}/{}",
            paren(num_text.clone(), num_text.contains(' ') || num_text.starts_with('-'), false),
            paren(join(&denominator), denominator.len() > 1, false)
        )
    }
}

fn superscript(n: i64) -> Option<String> {
    let map = |d: char| match d {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '-' => '⁻',
        _ => '?',
    };
    let mut out = String::new();
    for ch in n.to_string().chars() {
        let c = map(ch);
        if c == '?' {
            return None;
        }
        out.push(c);
    }
    Some(out)
}

fn unicode_pow(base: &Expr, exp: &Expr) -> String {
    let b = paren(
        unicode(base, 0),
        matches!(base, Expr::Add(_) | Expr::Mul(_) | Expr::Pow(_, _)),
        false,
    );
    if let Expr::Num(v) = exp {
        if v.is_one() {
            return b;
        }
        if v == &rat(1, 2) {
            return format!("√({})", unicode(base, 0));
        }
        if v == &rat(1, 3) {
            return format!("∛({})", unicode(base, 0));
        }
        if v == &rat(-1, 1) {
            return format!("1/{}", b);
        }
        if v.is_integer() {
            if let Some(n) = v.numer().to_i64() {
                if let Some(sup) = superscript(n) {
                    return format!("{b}{sup}");
                }
            }
        }
    }
    let e = paren(
        unicode(exp, 0),
        matches!(exp, Expr::Add(_) | Expr::Mul(_)),
        false,
    );
    format!("{b}^{e}")
}

fn latex(expr: &Expr, parent: i32) -> String {
    if let Some(text) = complex_latex(expr, parent) {
        return text;
    }
    match expr {
        Expr::Num(v) => {
            let sign = if v.is_negative() { "-" } else { "" };
            let abs = v.abs();
            if abs.is_integer() {
                format!("{sign}{}", abs.numer())
            } else {
                format!("{sign}\\frac{{{}}}{{{}}}", abs.numer(), abs.denom())
            }
        }
        Expr::Float(v) => {
            let text = v.to_string();
            if let Some(idx) = text.find('e') {
                let (m, e) = text.split_at(idx);
                format!("{m}\\times 10^{{{}}}", &e[1..])
            } else {
                text
            }
        }
        Expr::Sym(name) => match name.as_str() {
            "pi" => "\\pi".to_string(),
            "e" => "e".to_string(),
            "i" => "i".to_string(),
            _ => {
                if name.chars().count() == 1 {
                    name.clone()
                } else {
                    format!("\\mathrm{{{name}}}")
                }
            }
        },
        Expr::Str(text) => format!("\\text{{{text}}}"),
        Expr::Add(terms) => {
            let mut out = String::new();
            for (i, term) in terms.iter().enumerate() {
                let (sign, body) = split_sign(term);
                if i == 0 {
                    if sign < 0 {
                        out.push('-');
                    }
                } else if sign < 0 {
                    out.push_str(" - ");
                } else {
                    out.push_str(" + ");
                }
                out.push_str(&latex(&body, PREC_ADD));
            }
            paren(out, PREC_ADD < parent, true)
        }
        Expr::Mul(factors) => paren(latex_mul(factors), PREC_MUL < parent, true),
        Expr::Pow(base, exp) => paren(latex_pow(base, exp), PREC_POW < parent, true),
        Expr::Call(name, args) => format!(
            "{}\\left({}\\right)",
            name,
            args.iter()
                .map(|a| latex(a, 0))
                .collect::<Vec<_>>()
                .join(",\\; ")
        ),
        Expr::Eq(lhs, rhs) => format!("{} = {}", latex(lhs, 0), latex(rhs, 0)),
        Expr::List(items) => format!(
            "\\left\\{{ {} \\right\\}}",
            items
                .iter()
                .map(|i| latex(i, 0))
                .collect::<Vec<_>>()
                .join(",\\; ")
        ),
    }
}

fn latex_mul(factors: &[Expr]) -> String {
    factors
        .iter()
        .map(|f| latex(f, PREC_MUL))
        .collect::<Vec<_>>()
        .join("\\,")
}

fn latex_pow(base: &Expr, exp: &Expr) -> String {
    let b = paren(latex(base, 0), matches!(base, Expr::Add(_) | Expr::Mul(_) | Expr::Pow(_, _)), true);
    if let Expr::Num(v) = exp {
        if v == &rat(1, 2) {
            return format!("\\sqrt{{{}}}", latex(base, 0));
        }
        if v == &rat(1, 3) {
            return format!("\\sqrt[3]{{{}}}", latex(base, 0));
        }
        if v.is_integer() {
            return format!("{b}^{{{}}}", v.numer());
        }
    }
    format!("{b}^{{{}}}", latex(exp, 0))
}

fn complex_plain(expr: &Expr, parent: i32) -> Option<String> {
    if !matches!(expr, Expr::Add(_) | Expr::Mul(_) | Expr::Pow(_, _)) {
        return None;
    }
    let c = to_exact_complex(expr)?;
    if c.im.is_zero() {
        return None;
    }
    let im_abs = c.im.abs();
    let im_text = if im_abs.is_one() {
        "i".to_string()
    } else {
        format!("{}i", rat_display(&im_abs))
    };
    let out = if c.re.is_zero() {
        if c.im.is_negative() {
            format!("-{im_text}")
        } else {
            im_text
        }
    } else {
        format!(
            "{} {} {}",
            rat_display(&c.re),
            if c.im.is_negative() { "-" } else { "+" },
            im_text
        )
    };
    Some(paren(out, PREC_ADD < parent, false))
}

fn complex_unicode(expr: &Expr, parent: i32) -> Option<String> {
    if !matches!(expr, Expr::Add(_) | Expr::Mul(_) | Expr::Pow(_, _)) {
        return None;
    }
    let c = to_exact_complex(expr)?;
    if c.im.is_zero() {
        return None;
    }
    let im_abs = c.im.abs();
    let im_text = if im_abs.is_one() {
        "i".to_string()
    } else {
        format!("{}i", rat_display(&im_abs))
    };
    let out = if c.re.is_zero() {
        if c.im.is_negative() {
            format!("-{im_text}")
        } else {
            im_text
        }
    } else {
        format!(
            "{} {} {}",
            rat_display(&c.re),
            if c.im.is_negative() { "−" } else { "+" },
            im_text
        )
    };
    Some(paren(out, PREC_ADD < parent, false))
}

fn complex_latex(expr: &Expr, parent: i32) -> Option<String> {
    if !matches!(expr, Expr::Add(_) | Expr::Mul(_) | Expr::Pow(_, _)) {
        return None;
    }
    let c = to_exact_complex(expr)?;
    if c.im.is_zero() {
        return None;
    }
    let im_abs = c.im.abs();
    let im_text = if im_abs.is_one() {
        "i".to_string()
    } else {
        format!("{}i", latex(&Expr::Num(im_abs), 0))
    };
    let out = if c.re.is_zero() {
        if c.im.is_negative() {
            format!("-{im_text}")
        } else {
            im_text
        }
    } else {
        format!(
            "{} {} {}",
            latex(&Expr::Num(c.re.clone()), 0),
            if c.im.is_negative() { "-" } else { "+" },
            im_text
        )
    };
    Some(paren(out, PREC_ADD < parent, true))
}
