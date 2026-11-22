/**
 * 一元多项式（有理系数），用于因式分解、通分约分、方程求解与部分分式。
 * 系数数组按升幂排列：coeffs[i] 是 x^i 的系数。
 */

import { CalcError } from '../errors'
import { BigFloat } from '../numeric/bigfloat'
import { Rational } from '../numeric/rational'
import {
  add,
  Expr,
  float,
  HALF,
  I,
  key,
  mul,
  num,
  pow,
  rational,
  sub,
  sym
} from '../symbolic/expr'
import { occurs } from '../symbolic/utils'

export interface Poly {
  readonly varName: string
  readonly coeffs: readonly Rational[]
}

const MAX_DEGREE = 500

export function polyZero(varName: string): Poly {
  return { varName, coeffs: [] }
}

export function polyConstant(varName: string, value: Rational): Poly {
  return value.isZero ? polyZero(varName) : { varName, coeffs: [value] }
}

export function polyTrim(poly: Poly): Poly {
  const coeffs: Rational[] = []
  for (let i = 0; i < poly.coeffs.length; i++) {
    coeffs.push(poly.coeffs[i] ?? Rational.ZERO)
  }
  while (coeffs.length > 0 && coeffs[coeffs.length - 1].isZero) coeffs.pop()
  return { varName: poly.varName, coeffs }
}

export function polyDegree(poly: Poly): number {
  return poly.coeffs.length - 1
}

export function polyIsZero(poly: Poly): boolean {
  return poly.coeffs.length === 0
}

export function polyAdd(a: Poly, b: Poly): Poly {
  const n = Math.max(a.coeffs.length, b.coeffs.length)
  const out: Rational[] = []
  for (let i = 0; i < n; i++) {
    const x = a.coeffs[i] ?? Rational.ZERO
    const y = b.coeffs[i] ?? Rational.ZERO
    out.push(x.add(y))
  }
  return polyTrim({ varName: a.varName, coeffs: out })
}

export function polySub(a: Poly, b: Poly): Poly {
  return polyAdd(a, polyScale(b, Rational.NEG_ONE))
}

export function polyScale(a: Poly, k: Rational): Poly {
  if (k.isZero) return polyZero(a.varName)
  return {
    varName: a.varName,
    coeffs: a.coeffs.map((c) => c.mul(k))
  }
}

export function polyMul(a: Poly, b: Poly): Poly {
  if (polyIsZero(a) || polyIsZero(b)) return polyZero(a.varName)
  if (a.coeffs.length + b.coeffs.length - 1 > MAX_DEGREE + 1) {
    throw new CalcError('多项式次数过高')
  }
  const out = new Array<Rational>(a.coeffs.length + b.coeffs.length - 1).fill(
    Rational.ZERO
  )
  for (let i = 0; i < a.coeffs.length; i++) {
    for (let j = 0; j < b.coeffs.length; j++) {
      out[i + j] = out[i + j].add(a.coeffs[i].mul(b.coeffs[j]))
    }
  }
  return polyTrim({ varName: a.varName, coeffs: out })
}

export function polyPow(a: Poly, exp: number): Poly {
  if (exp < 0 || !Number.isInteger(exp)) throw new CalcError('多项式指数必须是非负整数')
  if (exp > 100) throw new CalcError('多项式指数过大')
  let result = polyConstant(a.varName, Rational.ONE)
  let base = a
  let e = exp
  while (e > 0) {
    if (e & 1) result = polyMul(result, base)
    e >>= 1
    if (e > 0) base = polyMul(base, base)
  }
  return result
}

export function polyDivMod(a: Poly, b: Poly): { q: Poly; r: Poly } {
  if (polyIsZero(b)) throw new CalcError('多项式除法：除式不能为零')
  let r = polyTrim(a)
  const q: Rational[] = []
  const db = polyDegree(b)
  const lb = b.coeffs[db]
  while (!polyIsZero(r) && polyDegree(r) >= db) {
    const dr = polyDegree(r)
    const k = r.coeffs[dr].div(lb)
    const shift = dr - db
    while (q.length <= shift) q.push(Rational.ZERO)
    q[shift] = q[shift].add(k)
    const sub = new Array<Rational>(shift).fill(Rational.ZERO)
    for (let i = 0; i <= db; i++) sub.push(b.coeffs[i].mul(k))
    r = polySub(r, { varName: r.varName, coeffs: sub })
  }
  while (q.length > 0 && q[q.length - 1].isZero) q.pop()
  return {
    q: { varName: a.varName, coeffs: q },
    r
  }
}

export function polyDerivative(a: Poly): Poly {
  if (a.coeffs.length <= 1) return polyZero(a.varName)
  const out: Rational[] = []
  for (let i = 1; i < a.coeffs.length; i++) {
    out.push(a.coeffs[i].mul(Rational.of(i)))
  }
  return polyTrim({ varName: a.varName, coeffs: out })
}

export function polyEvaluate(a: Poly, x: Rational): Rational {
  let result = Rational.ZERO
  for (let i = a.coeffs.length - 1; i >= 0; i--) {
    result = result.mul(x).add(a.coeffs[i])
  }
  return result
}

export function polyMonic(a: Poly): Poly {
  if (polyIsZero(a)) return a
  const lead = a.coeffs[polyDegree(a)]
  return polyScale(a, lead.inv())
}

export function polyGcd(a: Poly, b: Poly): Poly {
  let x = polyTrim(a)
  let y = polyTrim(b)
  while (!polyIsZero(y)) {
    const { r } = polyDivMod(x, y)
    x = y
    y = r
  }
  return polyMonic(x)
}

export function polyToExpr(poly: Poly): Expr {
  const terms: Expr[] = []
  const x = sym(poly.varName)
  for (let i = 0; i < poly.coeffs.length; i++) {
    const c = poly.coeffs[i]
    if (c.isZero) continue
    if (i === 0) terms.push(num(c))
    else if (i === 1) terms.push(mul(num(c), x))
    else terms.push(mul(num(c), pow(x, rational(i))))
  }
  return add(...terms)
}

export function polyFromExpr(expr: Expr, varName: string): Poly | null {
  const zero = polyZero(varName)
  const visit = (node: Expr): Poly | null => {
    switch (node.kind) {
      case 'num':
        return polyConstant(varName, node.value)
      case 'sym':
        if (node.name === varName) {
          return { varName, coeffs: [Rational.ZERO, Rational.ONE] }
        }
        return null
      case 'add': {
        let acc = zero
        for (const t of node.terms) {
          const p = visit(t)
          if (!p) return null
          acc = polyAdd(acc, p)
        }
        return acc
      }
      case 'mul': {
        let acc = polyConstant(varName, Rational.ONE)
        for (const f of node.factors) {
          const p = visit(f)
          if (!p) return null
          acc = polyMul(acc, p)
        }
        return acc
      }
      case 'pow': {
        if (node.exp.kind !== 'num' || !node.exp.value.isInteger) return null
        const e = Number(node.exp.value.toBigInt())
        if (e < 0 || e > 100) return null
        const base = visit(node.base)
        if (!base) return null
        return polyPow(base, e)
      }
      default:
        if (!occurs(node, varName)) return null
        return null
    }
  }
  const result = visit(expr)
  return result ? polyTrim(result) : null
}

function divisors(n: bigint): bigint[] {
  const abs = n < 0n ? -n : n
  if (abs === 0n) return [0n]
  const out = new Set<bigint>()
  for (let i = 1n; i * i <= abs && i <= 1000000n; i++) {
    if (abs % i === 0n) {
      out.add(i)
      out.add(abs / i)
    }
  }
  out.add(1n)
  out.add(abs)
  return [...out]
}

export function rationalRoots(poly: Poly): Rational[] {
  const p = polyTrim(poly)
  if (polyDegree(p) < 1) return []
  const c0 = p.coeffs[0]
  let leading = p.coeffs[polyDegree(p)]
  if (c0.isZero) {
    // x=0 是根；先把它提出来
    const rest = polyTrim({
      varName: p.varName,
      coeffs: p.coeffs.slice(1)
    })
    return [Rational.ZERO, ...rationalRoots(rest)]
  }
  const ps = divisors(c0.n)
  const qs = divisors(leading.n)
  const candidates = new Set<string>()
  const roots: Rational[] = []
  for (const a of ps) {
    for (const b of qs) {
      for (const sign of [1n, -1n]) {
        const candidate = Rational.of(sign * a, b)
        const k = candidate.toString()
        if (candidates.has(k)) continue
        candidates.add(k)
        if (polyEvaluate(p, candidate).isZero) roots.push(candidate)
      }
    }
  }
  return roots
}

/** 精确求解低次多项式；高次剩余部分返回数值根 */
export function solvePoly(poly: Poly): Expr[] {
  const p = polyTrim(poly)
  const degree = polyDegree(p)
  if (degree < 0) return []
  if (degree === 0) return []
  const x = sym(p.varName)

  if (degree === 1) {
    return [num(p.coeffs[0].neg().div(p.coeffs[1]))]
  }
  if (degree === 2) {
    return quadraticRoots(p, x)
  }

  let current = p
  const exact: Expr[] = []
  for (const r of rationalRoots(p)) {
    if (polyDegree(current) <= 0) break
    const factor: Poly = {
      varName: p.varName,
      coeffs: [r.neg(), Rational.ONE]
    }
    const { q, r: rem } = polyDivMod(current, factor)
    if (polyIsZero(rem)) {
      exact.push(num(r))
      current = polyTrim(q)
    }
  }
  if (polyDegree(current) === 0) return dedupe(exact)
  if (polyDegree(current) === 1) {
    exact.push(num(current.coeffs[0].neg().div(current.coeffs[1])))
    return dedupe(exact)
  }
  if (polyDegree(current) === 2) {
    return dedupe([...exact, ...quadraticRoots(current, x)])
  }
  return dedupe([...exact, ...numericRoots(current)])
}

function dedupe(items: Expr[]): Expr[] {
  const seen = new Set<string>()
  const out: Expr[] = []
  for (const item of items) {
    const k = key(item)
    if (seen.has(k)) continue
    seen.add(k)
    out.push(item)
  }
  return out
}

function sqrtExpr(r: Rational): Expr {
  if (r.isZero) return num(Rational.ZERO)
  return pow(num(r), HALF)
}

function quadraticRoots(p: Poly, _x: Expr): Expr[] {
  const a = p.coeffs[2]
  const b = p.coeffs[1]
  const c = p.coeffs[0]
  const disc = b.mul(b).sub(a.mul(c).mul(Rational.of(4)))
  if (disc.isZero) {
    return [num(b.neg().div(a.mul(Rational.TWO)))]
  }
  const denominator = a.mul(Rational.TWO)
  if (disc.sign > 0) {
    const s = sqrtExpr(disc)
    const real = num(b.neg().div(denominator))
    const term = mul(s, num(denominator.inv()))
    return [add(real, term), sub(real, term)]
  }
  const s = sqrtExpr(disc.neg())
  const real = num(b.neg().div(denominator))
  const imag = mul(s, num(denominator.inv()), I)
  return [add(real, imag), sub(real, imag)]
}

interface Cx {
  re: number
  im: number
}

function cadd(a: Cx, b: Cx): Cx {
  return { re: a.re + b.re, im: a.im + b.im }
}
function csub(a: Cx, b: Cx): Cx {
  return { re: a.re - b.re, im: a.im - b.im }
}
function cmul(a: Cx, b: Cx): Cx {
  return { re: a.re * b.re - a.im * b.im, im: a.re * b.im + a.im * b.re }
}
function cdiv(a: Cx, b: Cx): Cx {
  const d = b.re * b.re + b.im * b.im
  return {
    re: (a.re * b.re + a.im * b.im) / d,
    im: (a.im * b.re - a.re * b.im) / d
  }
}
function cabs(a: Cx): number {
  return Math.hypot(a.re, a.im)
}

function numericRoots(p: Poly): Expr[] {
  const degree = polyDegree(p)
  const coeffs = p.coeffs.map((c) => c.toNumber())
  const lead = coeffs[degree]
  if (!Number.isFinite(lead) || lead === 0) return []
  for (let i = 0; i <= degree; i++) coeffs[i] /= lead
  const roots: Cx[] = []
  for (let i = 0; i < degree; i++) {
    const angle = (2 * Math.PI * i) / degree + 0.35
    roots.push({ re: 0.9 * Math.cos(angle), im: 0.9 * Math.sin(angle) })
  }
  const evaluate = (z: Cx): Cx => {
    let result: Cx = { re: 0, im: 0 }
    for (let i = degree; i >= 0; i--) {
      result = cadd(cmul(result, z), { re: coeffs[i], im: 0 })
    }
    return result
  }
  for (let iter = 0; iter < 3000; iter++) {
    let maxDelta = 0
    for (let i = 0; i < degree; i++) {
      let denom: Cx = { re: 1, im: 0 }
      for (let j = 0; j < degree; j++) {
        if (i !== j) denom = cmul(denom, csub(roots[i], roots[j]))
      }
      if (cabs(denom) < 1e-24) continue
      const delta = cdiv(evaluate(roots[i]), denom)
      roots[i] = csub(roots[i], delta)
      maxDelta = Math.max(maxDelta, cabs(delta))
    }
    if (maxDelta < 1e-14) break
  }
  const out: Expr[] = []
  for (const root of roots) {
    if (Math.abs(root.im) < 1e-9) {
      out.push(float(BigFloat.fromNumber(root.re, 17)))
    } else {
      const re = float(BigFloat.fromNumber(root.re, 17))
      const im = float(BigFloat.fromNumber(Math.abs(root.im), 17))
      const iTerm = mul(im, I)
      out.push(add(re, root.im >= 0 ? iTerm : negExpr(iTerm)))
    }
  }
  return out
}

function negExpr(e: Expr): Expr {
  return mul(num(Rational.NEG_ONE), e)
}

export function factorPoly(poly: Poly): {
  content: Rational
  factors: Array<{ poly: Poly; mult: number }>
} {
  let p = polyTrim(poly)
  if (polyIsZero(p)) return { content: Rational.ONE, factors: [] }
  const content = p.coeffs.reduce(
    (g, c) => rationalGcd(g, c),
    Rational.ZERO
  )
  const normalizedContent = content.isZero ? Rational.ONE : content.abs()
  p = polyScale(p, normalizedContent.inv())

  const factors: Array<{ poly: Poly; mult: number }> = []
  for (const root of rationalRoots(p)) {
    const factor: Poly = {
      varName: p.varName,
      coeffs: [root.neg(), Rational.ONE]
    }
    let mult = 0
    while (true) {
      const { q, r } = polyDivMod(p, factor)
      if (!polyIsZero(r)) break
      p = polyTrim(q)
      mult++
    }
    if (mult > 0) factors.push({ poly: factor, mult })
  }
  if (polyDegree(p) > 0) {
    factors.push({ poly: p, mult: 1 })
  }
  return { content: normalizedContent, factors }
}

function bigintGcdAbs(a: bigint, b: bigint): bigint {
  let x = a < 0n ? -a : a
  let y = b < 0n ? -b : b
  while (y !== 0n) {
    const t = x % y
    x = y
    y = t
  }
  return x
}

function rationalGcd(a: Rational, b: Rational): Rational {
  if (a.isZero) return b.abs()
  if (b.isZero) return a.abs()
  const n = bigintGcdAbs(a.n, b.n)
  const d = (a.d / bigintGcdAbs(a.d, b.d)) * b.d
  return Rational.of(n, d)
}
