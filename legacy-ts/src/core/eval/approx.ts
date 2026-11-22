/**
 * 近似求值：把精确表达式转换成 BigFloat / 大精度复数。
 * 只有在 N(...)、近似显示或数值积分回退时才会用到。
 */

import { CalcError } from '../errors'
import {
  bfAcos,
  bfAsin,
  bfAtan,
  bfCos,
  bfCosh,
  bfExp,
  bfLn,
  bfLog10,
  bfLog2,
  bfPi,
  bfPow,
  bfSin,
  bfSinh,
  bfSqrt,
  bfTan,
  bfTanh,
  BigFloat
} from '../numeric/bigfloat'
import { Rational } from '../numeric/rational'
import {
  add,
  Expr,
  float,
  I,
  mul,
  num
} from '../symbolic/expr'

export interface CNum {
  re: BigFloat
  im: BigFloat
}

export function cnum(re: BigFloat, im: BigFloat): CNum {
  return { re, im }
}

function zero(p: number): BigFloat {
  return BigFloat.zero(p)
}

function one(p: number): BigFloat {
  return BigFloat.one(p)
}

export function cadd(a: CNum, b: CNum, p: number): CNum {
  return { re: a.re.add(b.re, p), im: a.im.add(b.im, p) }
}

export function csub(a: CNum, b: CNum, p: number): CNum {
  return { re: a.re.sub(b.re, p), im: a.im.sub(b.im, p) }
}

export function cmul(a: CNum, b: CNum, p: number): CNum {
  const re = a.re.mul(b.re, p).sub(a.im.mul(b.im, p), p)
  const im = a.re.mul(b.im, p).add(a.im.mul(b.re, p), p)
  return { re, im }
}

export function cdiv(a: CNum, b: CNum, p: number): CNum {
  const d = b.re.mul(b.re, p).add(b.im.mul(b.im, p), p)
  if (d.isZero) throw new CalcError('复数除法：除数为零')
  const re = a.re.mul(b.re, p).add(a.im.mul(b.im, p), p).div(d, p)
  const im = a.im.mul(b.re, p).sub(a.re.mul(b.im, p), p).div(d, p)
  return { re, im }
}

export function cneg(a: CNum): CNum {
  return { re: a.re.neg(), im: a.im.neg() }
}

export function cabs(a: CNum, p: number): BigFloat {
  const sum = a.re.mul(a.re, p).add(a.im.mul(a.im, p), p)
  return bfSqrt(sum, p)
}

export function cexp(a: CNum, p: number): CNum {
  const expRe = bfExp(a.re, p)
  return {
    re: expRe.mul(bfCos(a.im, p), p),
    im: expRe.mul(bfSin(a.im, p), p)
  }
}

function bfAtan2(y: BigFloat, x: BigFloat, p: number): BigFloat {
  const work = p + 8
  if (x.isPositive) return bfAtan(y.div(x, work), p)
  if (x.isNegative) {
    const base = bfAtan(y.div(x, work), work)
    const pi = bfPi(work)
    return y.isNegative ? base.sub(pi, p) : base.add(pi, p)
  }
  if (y.isPositive) return bfPi(work).div(BigFloat.fromInt(2n, work), p)
  if (y.isNegative) return bfPi(work).div(BigFloat.fromInt(2n, work), p).neg()
  return zero(p)
}

export function cln(a: CNum, p: number): CNum {
  const modulus = cabs(a, p + 8)
  if (modulus.isZero) throw new CalcError('复对数：零没有对数')
  return {
    re: bfLn(modulus, p),
    im: bfAtan2(a.im, a.re, p)
  }
}

export function csqrt(a: CNum, p: number): CNum {
  if (a.im.isZero) {
    if (!a.re.isNegative) return { re: bfSqrt(a.re, p), im: zero(p) }
    return { re: zero(p), im: bfSqrt(a.re.neg(), p) }
  }
  const work = p + 8
  const r = cabs(a, work)
  const sum = a.re.add(r, work)
  const re = bfSqrt(sum.div(BigFloat.fromInt(2n, work), work), p)
  const diff = r.sub(a.re, work)
  let im = bfSqrt(diff.div(BigFloat.fromInt(2n, work), work), p)
  if (a.im.isNegative) im = im.neg()
  return { re, im }
}

export function cpow(a: CNum, b: CNum, p: number): CNum {
  if (b.im.isZero && b.re.isInteger) {
    const n = b.re.toNumber()
    if (Number.isSafeInteger(n) && Math.abs(n) <= 100000) {
      if (n === 0) return { re: one(p), im: zero(p) }
      const work = p + 8
      let base: CNum = { re: a.re.withPrecision(work), im: a.im.withPrecision(work) }
      let exp = Math.abs(n)
      let result: CNum = { re: one(work), im: zero(work) }
      while (exp > 0) {
        if (exp & 1) result = cmul(result, base, work)
        base = cmul(base, base, work)
        exp >>= 1
      }
      if (n < 0) {
        result = cdiv({ re: one(work), im: zero(work) }, result, work)
      }
      return {
        re: result.re.withPrecision(p),
        im: result.im.withPrecision(p)
      }
    }
  }
  return cexp(cmul(b, cln(a, p + 8), p + 8), p)
}

export function csin(a: CNum, p: number): CNum {
  if (a.im.isZero) return { re: bfSin(a.re, p), im: zero(p) }
  const work = p + 8
  return {
    re: bfSin(a.re, work).mul(bfCosh(a.im, work), p),
    im: bfCos(a.re, work).mul(bfSinh(a.im, work), p)
  }
}

export function ccos(a: CNum, p: number): CNum {
  if (a.im.isZero) return { re: bfCos(a.re, p), im: zero(p) }
  const work = p + 8
  return {
    re: bfCos(a.re, work).mul(bfCosh(a.im, work), p),
    im: bfSin(a.re, work).mul(bfSinh(a.im, work), p).neg()
  }
}

export function ctan(a: CNum, p: number): CNum {
  if (a.im.isZero) return { re: bfTan(a.re, p), im: zero(p) }
  return cdiv(csin(a, p + 8), ccos(a, p + 8), p)
}

function nearlyZero(value: BigFloat, p: number): boolean {
  return value.isZero || value.order < -(p - 2)
}

export function toCNum(expr: Expr, precision: number): CNum {
  const p = Math.max(10, precision)
  switch (expr.kind) {
    case 'num':
      return { re: BigFloat.fromRational(expr.value, p), im: zero(p) }
    case 'float':
      return { re: expr.value.withPrecision(p), im: zero(p) }
    case 'sym':
      if (expr.name === 'pi') return { re: bfPi(p), im: zero(p) }
      if (expr.name === 'e') return { re: bfExp(one(p), p), im: zero(p) }
      if (expr.name === 'i') return { re: zero(p), im: one(p) }
      throw new CalcError(`无法近似：${expr.name} 不是已知常数`)
    case 'add': {
      let acc: CNum = { re: zero(p), im: zero(p) }
      for (const t of expr.terms) acc = cadd(acc, toCNum(t, p), p)
      return acc
    }
    case 'mul': {
      let acc: CNum = { re: one(p), im: zero(p) }
      for (const f of expr.factors) acc = cmul(acc, toCNum(f, p), p)
      return acc
    }
    case 'pow':
      return cpow(toCNum(expr.base, p), toCNum(expr.exp, p), p)
    case 'call':
      return evalCall(expr, p)
    case 'eq':
      throw new CalcError('方程不能直接近似，请先求解')
    case 'list':
      throw new CalcError('解集不能直接近似')
    case 'str':
      throw new CalcError('文本不能近似')
  }
}

function evalCall(expr: Expr & { kind: 'call' }, p: number): CNum {
  const name = expr.name.toLowerCase()
  const args = expr.args.map((a) => toCNum(a, p))
  const a = args[0]
  switch (name) {
    case 'sqrt':
      return csqrt(a, p)
    case 'exp':
      return cexp(a, p)
    case 'ln':
      return cln(a, p)
    case 'log10': {
      const ln = cln(a, p + 8)
      const denom = bfLn(BigFloat.fromInt(10n, p + 8), p + 8)
      return { re: ln.re.div(denom, p), im: ln.im.div(denom, p) }
    }
    case 'log2': {
      const ln = cln(a, p + 8)
      const denom = bfLn(BigFloat.fromInt(2n, p + 8), p + 8)
      return { re: ln.re.div(denom, p), im: ln.im.div(denom, p) }
    }
    case 'sin':
      return csin(a, p)
    case 'cos':
      return ccos(a, p)
    case 'tan':
      return ctan(a, p)
    case 'asin':
      if (!a.im.isZero) throw new CalcError('复数的 asin 暂不支持')
      return { re: bfAsin(a.re, p), im: zero(p) }
    case 'acos':
      if (!a.im.isZero) throw new CalcError('复数的 acos 暂不支持')
      return { re: bfAcos(a.re, p), im: zero(p) }
    case 'atan':
      if (!a.im.isZero) throw new CalcError('复数的 atan 暂不支持')
      return { re: bfAtan(a.re, p), im: zero(p) }
    case 'sinh':
      if (!a.im.isZero) throw new CalcError('复数的 sinh 暂不支持')
      return { re: bfSinh(a.re, p), im: zero(p) }
    case 'cosh':
      if (!a.im.isZero) throw new CalcError('复数的 cosh 暂不支持')
      return { re: bfCosh(a.re, p), im: zero(p) }
    case 'tanh':
      if (!a.im.isZero) throw new CalcError('复数的 tanh 暂不支持')
      return { re: bfTanh(a.re, p), im: zero(p) }
    case 'abs':
      return { re: cabs(a, p), im: zero(p) }
    case 'sign':
      if (!a.im.isZero) throw new CalcError('复数的 sign 暂不支持')
      return {
        re: a.re.isZero ? zero(p) : BigFloat.fromInt(a.re.isNegative ? -1n : 1n, p),
        im: zero(p)
      }
    case 'floor':
      if (!a.im.isZero) throw new CalcError('复数的 floor 暂不支持')
      return { re: a.re.floor().withPrecision(p), im: zero(p) }
    case 'ceil':
      if (!a.im.isZero) throw new CalcError('复数的 ceil 暂不支持')
      return { re: a.re.ceil().withPrecision(p), im: zero(p) }
    case 'round':
      if (!a.im.isZero) throw new CalcError('复数的 round 暂不支持')
      return { re: a.re.round().withPrecision(p), im: zero(p) }
    default:
      throw new CalcError(`函数 ${expr.name} 暂不支持近似求值`)
  }
}

export function formatCNum(value: CNum, precision: number): string {
  const p = Math.max(10, precision)
  const real = nearlyZero(value.re, p) ? zero(p) : value.re
  const imag = nearlyZero(value.im, p) ? zero(p) : value.im
  if (imag.isZero) return real.toString()
  if (real.isZero) return `${imag.toString()}i`
  const sign = imag.isNegative ? '-' : '+'
  return `${real.toString()} ${sign} ${imag.abs().toString()}i`
}

export function toApproxExpr(expr: Expr, precision: number): Expr {
  const p = Math.max(10, precision)
  const value = toCNum(expr, p)
  const real = nearlyZero(value.re, p) ? zero(p) : value.re
  const imag = nearlyZero(value.im, p) ? zero(p) : value.im
  if (imag.isZero) return float(real)
  if (real.isZero) return mul(float(imag), I)
  return add(float(real), mul(float(imag), I))
}

export function approxPreview(expr: Expr, precision = 20): string | null {
  try {
    return formatCNum(toCNum(expr, Math.max(10, precision)), precision)
  } catch {
    return null
  }
}

export function rationalToDecimal(r: Rational, precision = 50): string {
  return BigFloat.fromRational(r, precision).toString()
}

void num
