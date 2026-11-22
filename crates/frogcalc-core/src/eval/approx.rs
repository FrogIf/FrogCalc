use std::cmp::Ordering;

use num_traits::ToPrimitive;

use crate::error::CalcError;
use crate::numeric::bigfloat::{
    bf_acos, bf_asin, bf_atan, bf_cos, bf_cosh, bf_exp, bf_ln, bf_pi, bf_sin, bf_sinh, bf_sqrt,
    bf_tan, bf_tanh, BigFloat,
};
use crate::numeric::{rat_to_f64, Rat};
use crate::symbolic::{add, mul, Expr};

#[derive(Clone, Debug)]
pub struct CNum {
    pub re: BigFloat,
    pub im: BigFloat,
}

fn zero(p: usize) -> BigFloat {
    BigFloat::zero(p)
}

fn one(p: usize) -> BigFloat {
    BigFloat::one(p)
}

pub fn cadd(a: &CNum, b: &CNum, p: usize) -> CNum {
    CNum {
        re: a.re.add(&b.re, p),
        im: a.im.add(&b.im, p),
    }
}

pub fn csub(a: &CNum, b: &CNum, p: usize) -> CNum {
    CNum {
        re: a.re.sub(&b.re, p),
        im: a.im.sub(&b.im, p),
    }
}

pub fn cmul(a: &CNum, b: &CNum, p: usize) -> CNum {
    CNum {
        re: a.re.mul(&b.re, p).sub(&a.im.mul(&b.im, p), p),
        im: a.re.mul(&b.im, p).add(&a.im.mul(&b.re, p), p),
    }
}

pub fn cdiv(a: &CNum, b: &CNum, p: usize) -> Result<CNum, CalcError> {
    let d = b.re.mul(&b.re, p).add(&b.im.mul(&b.im, p), p);
    if d.is_zero() {
        return Err(CalcError::new("复数除法：除数为零"));
    }
    Ok(CNum {
        re: a.re.mul(&b.re, p).add(&a.im.mul(&b.im, p), p).div(&d, p),
        im: a.im.mul(&b.re, p).sub(&a.re.mul(&b.im, p), p).div(&d, p),
    })
}

pub fn cabs(a: &CNum, p: usize) -> BigFloat {
    let sum = a.re.mul(&a.re, p).add(&a.im.mul(&a.im, p), p);
    bf_sqrt(&sum, p).unwrap_or_else(|_| zero(p))
}

pub fn cexp(a: &CNum, p: usize) -> Result<CNum, CalcError> {
    let exp_re = bf_exp(&a.re, p).map_err(CalcError::new)?;
    Ok(CNum {
        re: exp_re.mul(&bf_cos(&a.im, p).map_err(CalcError::new)?, p),
        im: exp_re.mul(&bf_sin(&a.im, p).map_err(CalcError::new)?, p),
    })
}

fn atan2(y: &BigFloat, x: &BigFloat, p: usize) -> BigFloat {
    let work = p + 8;
    match (x.is_positive(), x.is_negative()) {
        (true, _) => bf_atan(&y.div(x, work), p).unwrap_or_else(|_| zero(p)),
        (false, true) => {
            let base = bf_atan(&y.div(x, work), work).unwrap_or_else(|_| zero(work));
            let pi = bf_pi(work);
            if y.is_negative() {
                base.sub(&pi, p)
            } else {
                base.add(&pi, p)
            }
        }
        _ => {
            if y.is_positive() {
                bf_pi(work).div(&BigFloat::from_u32(2, work), p)
            } else if y.is_negative() {
                bf_pi(work).div(&BigFloat::from_u32(2, work), p).neg()
            } else {
                zero(p)
            }
        }
    }
}

pub fn cln(a: &CNum, p: usize) -> Result<CNum, CalcError> {
    let modulus = cabs(a, p + 8);
    if modulus.is_zero() {
        return Err(CalcError::new("复对数：零没有对数"));
    }
    Ok(CNum {
        re: bf_ln(&modulus, p).map_err(CalcError::new)?,
        im: atan2(&a.im, &a.re, p),
    })
}

pub fn csqrt(a: &CNum, p: usize) -> Result<CNum, CalcError> {
    if a.im.is_zero() {
        return if a.re.is_negative() {
            Ok(CNum {
                re: zero(p),
                im: bf_sqrt(&a.re.neg(), p).map_err(CalcError::new)?,
            })
        } else {
            Ok(CNum {
                re: bf_sqrt(&a.re, p).map_err(CalcError::new)?,
                im: zero(p),
            })
        };
    }
    let work = p + 8;
    let r = cabs(a, work);
    let sum = a.re.add(&r, work);
    let re = bf_sqrt(&sum.div(&BigFloat::from_u32(2, work), work), p).map_err(CalcError::new)?;
    let diff = r.sub(&a.re, work);
    let mut im =
        bf_sqrt(&diff.div(&BigFloat::from_u32(2, work), work), p).map_err(CalcError::new)?;
    if a.im.is_negative() {
        im = im.neg();
    }
    Ok(CNum { re, im })
}

pub fn cpow(a: &CNum, b: &CNum, p: usize) -> Result<CNum, CalcError> {
    if b.im.is_zero() && b.re.is_integer() {
        if let Some(n) = b.re.to_bigint().and_then(|v| v.to_i64()) {
            if n == 0 {
                return Ok(CNum {
                    re: one(p),
                    im: zero(p),
                });
            }
            let work = p + 8;
            let mut base = CNum {
                re: a.re.with_precision(work),
                im: a.im.with_precision(work),
            };
            let negative = n < 0;
            let mut exp = n.unsigned_abs();
            let mut result = CNum {
                re: one(work),
                im: zero(work),
            };
            while exp > 0 {
                if exp & 1 == 1 {
                    result = cmul(&result, &base, work);
                }
                base = cmul(&base, &base, work);
                exp >>= 1;
            }
            if negative {
                result = cdiv(
                    &CNum {
                        re: one(work),
                        im: zero(work),
                    },
                    &result,
                    work,
                )?;
            }
            return Ok(CNum {
                re: result.re.with_precision(p),
                im: result.im.with_precision(p),
            });
        }
    }
    cexp(&cmul(b, &cln(a, p + 8)?, p + 8), p)
}

pub fn csin(a: &CNum, p: usize) -> Result<CNum, CalcError> {
    if a.im.is_zero() {
        return Ok(CNum {
            re: bf_sin(&a.re, p).map_err(CalcError::new)?,
            im: zero(p),
        });
    }
    let work = p + 8;
    Ok(CNum {
        re: bf_sin(&a.re, work)
            .map_err(CalcError::new)?
            .mul(&bf_cosh(&a.im, work).map_err(CalcError::new)?, p),
        im: bf_cos(&a.re, work)
            .map_err(CalcError::new)?
            .mul(&bf_sinh(&a.im, work).map_err(CalcError::new)?, p),
    })
}

pub fn ccos(a: &CNum, p: usize) -> Result<CNum, CalcError> {
    if a.im.is_zero() {
        return Ok(CNum {
            re: bf_cos(&a.re, p).map_err(CalcError::new)?,
            im: zero(p),
        });
    }
    let work = p + 8;
    Ok(CNum {
        re: bf_cos(&a.re, work)
            .map_err(CalcError::new)?
            .mul(&bf_cosh(&a.im, work).map_err(CalcError::new)?, p),
        im: bf_sin(&a.re, work)
            .map_err(CalcError::new)?
            .mul(&bf_sinh(&a.im, work).map_err(CalcError::new)?, p)
            .neg(),
    })
}

pub fn ctan(a: &CNum, p: usize) -> Result<CNum, CalcError> {
    if a.im.is_zero() {
        return Ok(CNum {
            re: bf_tan(&a.re, p).map_err(CalcError::new)?,
            im: zero(p),
        });
    }
    cdiv(&csin(a, p + 8)?, &ccos(a, p + 8)?, p)
}

fn nearly_zero(value: &BigFloat, p: usize) -> bool {
    value.is_zero() || value.order() < -(p as i64 - 2)
}

pub fn to_cnum(expr: &Expr, precision: usize) -> Result<CNum, CalcError> {
    let p = precision.max(10);
    match expr {
        Expr::Num(v) => Ok(CNum {
            re: BigFloat::from_rat(v, p),
            im: zero(p),
        }),
        Expr::Float(v) => Ok(CNum {
            re: v.with_precision(p),
            im: zero(p),
        }),
        Expr::Sym(name) => match name.as_str() {
            "pi" => Ok(CNum {
                re: bf_pi(p),
                im: zero(p),
            }),
            "e" => Ok(CNum {
                re: bf_exp(&one(p), p).map_err(CalcError::new)?,
                im: zero(p),
            }),
            "i" => Ok(CNum {
                re: zero(p),
                im: one(p),
            }),
            _ => Err(CalcError::new(format!("无法近似：{name} 不是已知常数"))),
        },
        Expr::Add(terms) => {
            let mut acc = CNum {
                re: zero(p),
                im: zero(p),
            };
            for term in terms {
                acc = cadd(&acc, &to_cnum(term, p)?, p);
            }
            Ok(acc)
        }
        Expr::Mul(factors) => {
            let mut acc = CNum {
                re: one(p),
                im: zero(p),
            };
            for factor in factors {
                acc = cmul(&acc, &to_cnum(factor, p)?, p);
            }
            Ok(acc)
        }
        Expr::Pow(base, exp) => cpow(&to_cnum(base, p)?, &to_cnum(exp, p)?, p),
        Expr::Call(name, args) => eval_call(name, args, p),
        Expr::Eq(_, _) => Err(CalcError::new("方程不能直接近似，请先求解")),
        Expr::List(_) => Err(CalcError::new("解集不能直接近似")),
        Expr::Str(_) => Err(CalcError::new("文本不能近似")),
    }
}

fn eval_call(name: &str, args: &[Expr], p: usize) -> Result<CNum, CalcError> {
    let values: Result<Vec<CNum>, CalcError> = args.iter().map(|a| to_cnum(a, p)).collect();
    let values = values?;
    let a = values
        .first()
        .cloned()
        .ok_or_else(|| CalcError::new("函数缺少参数"))?;
    match name.to_lowercase().as_str() {
        "sqrt" => csqrt(&a, p),
        "exp" => cexp(&a, p),
        "ln" => cln(&a, p),
        "log10" => {
            let ln = cln(&a, p + 8)?;
            let denom = bf_ln(&BigFloat::from_i64(10, p + 8), p + 8).map_err(CalcError::new)?;
            Ok(CNum {
                re: ln.re.div(&denom, p),
                im: ln.im.div(&denom, p),
            })
        }
        "log2" => {
            let ln = cln(&a, p + 8)?;
            let denom = bf_ln(&BigFloat::from_i64(2, p + 8), p + 8).map_err(CalcError::new)?;
            Ok(CNum {
                re: ln.re.div(&denom, p),
                im: ln.im.div(&denom, p),
            })
        }
        "sin" => csin(&a, p),
        "cos" => ccos(&a, p),
        "tan" => ctan(&a, p),
        "asin" => {
            if !a.im.is_zero() {
                return Err(CalcError::new("复数的 asin 暂不支持"));
            }
            Ok(CNum {
                re: bf_asin(&a.re, p).map_err(CalcError::new)?,
                im: zero(p),
            })
        }
        "acos" => {
            if !a.im.is_zero() {
                return Err(CalcError::new("复数的 acos 暂不支持"));
            }
            Ok(CNum {
                re: bf_acos(&a.re, p).map_err(CalcError::new)?,
                im: zero(p),
            })
        }
        "atan" => {
            if !a.im.is_zero() {
                return Err(CalcError::new("复数的 atan 暂不支持"));
            }
            Ok(CNum {
                re: bf_atan(&a.re, p).map_err(CalcError::new)?,
                im: zero(p),
            })
        }
        "sinh" => {
            if !a.im.is_zero() {
                return Err(CalcError::new("复数的 sinh 暂不支持"));
            }
            Ok(CNum {
                re: bf_sinh(&a.re, p).map_err(CalcError::new)?,
                im: zero(p),
            })
        }
        "cosh" => {
            if !a.im.is_zero() {
                return Err(CalcError::new("复数的 cosh 暂不支持"));
            }
            Ok(CNum {
                re: bf_cosh(&a.re, p).map_err(CalcError::new)?,
                im: zero(p),
            })
        }
        "tanh" => {
            if !a.im.is_zero() {
                return Err(CalcError::new("复数的 tanh 暂不支持"));
            }
            Ok(CNum {
                re: bf_tanh(&a.re, p).map_err(CalcError::new)?,
                im: zero(p),
            })
        }
        "abs" => Ok(CNum {
            re: cabs(&a, p),
            im: zero(p),
        }),
        "sign" => {
            if !a.im.is_zero() {
                return Err(CalcError::new("复数的 sign 暂不支持"));
            }
            Ok(CNum {
                re: if a.re.is_zero() {
                    zero(p)
                } else {
                    a.re.signum_float(p)
                },
                im: zero(p),
            })
        }
        "floor" => {
            if !a.im.is_zero() {
                return Err(CalcError::new("复数的 floor 暂不支持"));
            }
            Ok(CNum {
                re: a.re.floor().with_precision(p),
                im: zero(p),
            })
        }
        "ceil" => {
            if !a.im.is_zero() {
                return Err(CalcError::new("复数的 ceil 暂不支持"));
            }
            Ok(CNum {
                re: a.re.ceil().with_precision(p),
                im: zero(p),
            })
        }
        "round" => {
            if !a.im.is_zero() {
                return Err(CalcError::new("复数的 round 暂不支持"));
            }
            Ok(CNum {
                re: a.re.round().with_precision(p),
                im: zero(p),
            })
        }
        _ => Err(CalcError::new(format!("函数 {name} 暂不支持近似求值"))),
    }
}

impl BigFloat {
    fn signum_float(&self, prec: usize) -> BigFloat {
        if self.is_zero() {
            BigFloat::zero(prec)
        } else if self.is_negative() {
            BigFloat::from_i64(-1, prec)
        } else {
            BigFloat::one(prec)
        }
    }
}

pub fn format_cnum(value: &CNum, precision: usize) -> String {
    let p = precision.max(10);
    let real = if nearly_zero(&value.re, p) {
        zero(p)
    } else {
        value.re.clone()
    };
    let imag = if nearly_zero(&value.im, p) {
        zero(p)
    } else {
        value.im.clone()
    };
    if imag.is_zero() {
        return real.to_string();
    }
    let im_abs = imag.abs();
    let im_text = if im_abs.to_string() == "1" {
        "i".to_string()
    } else {
        format!("{im_abs}i")
    };
    if real.is_zero() {
        return if imag.is_negative() {
            format!("-{im_text}")
        } else {
            im_text
        };
    }
    let sign = if imag.is_negative() { "-" } else { "+" };
    format!("{real} {sign} {im_text}")
}

pub fn to_approx_expr(expr: &Expr, precision: usize) -> Result<Expr, CalcError> {
    let p = precision.max(10);
    let value = to_cnum(expr, p)?;
    let real = if nearly_zero(&value.re, p) {
        zero(p)
    } else {
        value.re.clone()
    };
    let imag = if nearly_zero(&value.im, p) {
        zero(p)
    } else {
        value.im.clone()
    };
    if imag.is_zero() {
        return Ok(Expr::Float(real));
    }
    if real.is_zero() {
        return Ok(mul(vec![Expr::Float(imag), crate::symbolic::sym("i")]));
    }
    Ok(add(vec![
        Expr::Float(real),
        mul(vec![Expr::Float(imag), crate::symbolic::sym("i")]),
    ]))
}

pub fn approx_preview(expr: &Expr, precision: usize) -> Option<String> {
    to_cnum(expr, precision.max(10))
        .ok()
        .map(|value| format_cnum(&value, precision))
}

pub fn rat_to_decimal(value: &Rat, precision: usize) -> String {
    BigFloat::from_rat(value, precision).to_string()
}

pub fn compare_float(a: &BigFloat, b: &BigFloat) -> Ordering {
    a.compare(b)
}

pub fn float_to_f64(value: &BigFloat) -> f64 {
    rat_to_f64(&crate::numeric::rat(0, 1)) * 0.0 + value.to_f64()
}
