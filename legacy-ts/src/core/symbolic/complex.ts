import { Rational } from '../numeric/rational'
import {
  add,
  Expr,
  I,
  mul,
  num,
  pow,
  sym
} from './expr'

export interface ExactComplex {
  re: Rational
  im: Rational
}

/** 把由有理数和 i 组成的表达式识别为精确复数；遇到 pi/e/函数返回 null */
export function toExactComplex(expr: Expr): ExactComplex | null {
  switch (expr.kind) {
    case 'num':
      return { re: expr.value, im: Rational.ZERO }
    case 'sym':
      return expr.name === 'i' ? { re: Rational.ZERO, im: Rational.ONE } : null
    case 'add': {
      let re = Rational.ZERO
      let im = Rational.ZERO
      for (const term of expr.terms) {
        const c = toExactComplex(term)
        if (!c) return null
        re = re.add(c.re)
        im = im.add(c.im)
      }
      return { re, im }
    }
    case 'mul': {
      let acc: ExactComplex = { re: Rational.ONE, im: Rational.ZERO }
      for (const factor of expr.factors) {
        const c = toExactComplex(factor)
        if (!c) return null
        acc = multiply(acc, c)
      }
      return acc
    }
    case 'pow': {
      if (expr.exp.kind !== 'num' || !expr.exp.value.isInteger) return null
      const base = toExactComplex(expr.base)
      if (!base) return null
      const n = expr.exp.value
      if (n.isZero) return { re: Rational.ONE, im: Rational.ZERO }
      const neg = n.sign < 0
      const exp = Number(n.abs().toBigInt())
      if (!Number.isFinite(exp) || exp > 100000) return null
      let result: ExactComplex = { re: Rational.ONE, im: Rational.ZERO }
      let factor = base
      let e = exp
      while (e > 0) {
        if (e & 1) result = multiply(result, factor)
        factor = multiply(factor, factor)
        e >>= 1
      }
      return neg ? reciprocal(result) : result
    }
    default:
      return null
  }
}

function multiply(a: ExactComplex, b: ExactComplex): ExactComplex {
  return {
    re: a.re.mul(b.re).sub(a.im.mul(b.im)),
    im: a.re.mul(b.im).add(a.im.mul(b.re))
  }
}

export function multiplyExactComplex(a: ExactComplex, b: ExactComplex): ExactComplex {
  return multiply(a, b)
}

function reciprocal(a: ExactComplex): ExactComplex | null {
  const denom = a.re.mul(a.re).add(a.im.mul(a.im))
  if (denom.isZero) return null
  return {
    re: a.re.div(denom),
    im: a.im.neg().div(denom)
  }
}

export function fromExactComplex(value: ExactComplex): Expr {
  if (value.im.isZero) return num(value.re)
  const imPart = value.im.isOne ? I : mul(num(value.im), I)
  if (value.re.isZero) return imPart
  return add(num(value.re), imPart)
}

export function divideExactComplex(a: ExactComplex, b: ExactComplex): ExactComplex | null {
  const r = reciprocal(b)
  return r ? multiply(a, r) : null
}

export function powExactComplex(a: ExactComplex, n: number): ExactComplex | null {
  if (!Number.isInteger(n)) return null
  if (n === 0) return { re: Rational.ONE, im: Rational.ZERO }
  const neg = n < 0
  let base = a
  let exp = Math.abs(n)
  let result: ExactComplex = { re: Rational.ONE, im: Rational.ZERO }
  while (exp > 0) {
    if (exp & 1) result = multiply(result, base)
    base = multiply(base, base)
    exp >>= 1
  }
  return neg ? reciprocal(result) : result
}

void sym
