/**
 * 符号表达式树与自动化简。
 *
 * 计算器的默认计算方式是“精确符号优先”：
 *   - 数字字面量一律是 Rational；
 *   - 加减乘除自动折叠、合并同类项、合并同底数幂；
 *   - sqrt(2) 保留为 2^(1/2)，sqrt(2)*sqrt(2) 自动变成 2；
 *   - 只有显式调用 N(...) 或要求近似显示时才转成 BigFloat。
 */

import { CalcError } from '../errors'
import { bigintGcd, Rational } from '../numeric/rational'
import { BigFloat } from '../numeric/bigfloat'

export type Expr =
  | NumExpr
  | FloatExpr
  | SymExpr
  | StrExpr
  | AddExpr
  | MulExpr
  | PowExpr
  | CallExpr
  | EqExpr
  | ListExpr

export interface NumExpr {
  readonly kind: 'num'
  readonly value: Rational
}
export interface FloatExpr {
  readonly kind: 'float'
  readonly value: BigFloat
}
export interface SymExpr {
  readonly kind: 'sym'
  readonly name: string
}
export interface StrExpr {
  readonly kind: 'str'
  readonly value: string
}
export interface AddExpr {
  readonly kind: 'add'
  readonly terms: readonly Expr[]
}
export interface MulExpr {
  readonly kind: 'mul'
  readonly factors: readonly Expr[]
}
export interface PowExpr {
  readonly kind: 'pow'
  readonly base: Expr
  readonly exp: Expr
}
export interface CallExpr {
  readonly kind: 'call'
  readonly name: string
  readonly args: readonly Expr[]
}
export interface EqExpr {
  readonly kind: 'eq'
  readonly lhs: Expr
  readonly rhs: Expr
}
export interface ListExpr {
  readonly kind: 'list'
  readonly items: readonly Expr[]
}

export const ZERO: Expr = { kind: 'num', value: Rational.ZERO }
export const ONE: Expr = { kind: 'num', value: Rational.ONE }
export const NEG_ONE: Expr = { kind: 'num', value: Rational.NEG_ONE }
export const TWO: Expr = { kind: 'num', value: Rational.TWO }
export const HALF: Expr = { kind: 'num', value: Rational.HALF }
export const I: Expr = { kind: 'sym', name: 'i' }
export const PI: Expr = { kind: 'sym', name: 'pi' }
export const E: Expr = { kind: 'sym', name: 'e' }

const symCache = new Map<string, Expr>()

export function num(value: Rational): Expr {
  if (value.isZero) return ZERO
  if (value.isOne) return ONE
  if (value.isMinusOne) return NEG_ONE
  if (value.d === 1n && value.n === 2n) return TWO
  if (value.n === 1n && value.d === 2n) return HALF
  return { kind: 'num', value }
}

export function integer(n: bigint | number): Expr {
  return num(Rational.of(typeof n === 'number' ? BigInt(Math.trunc(n)) : n))
}

export function rational(n: bigint | number, d: bigint | number = 1n): Expr {
  return num(Rational.of(n, d))
}

export function float(value: BigFloat): Expr {
  return { kind: 'float', value }
}

export function sym(name: string): Expr {
  const cached = symCache.get(name)
  if (cached) return cached
  const value: Expr = { kind: 'sym', name }
  symCache.set(name, value)
  return value
}

export function str(value: string): Expr {
  return { kind: 'str', value }
}

export function eq(lhs: Expr, rhs: Expr): Expr {
  return { kind: 'eq', lhs, rhs }
}

export function list(items: Expr[]): Expr {
  return { kind: 'list', items }
}

const keyCache = new WeakMap<object, string>()

export function key(expr: Expr): string {
  const cached = keyCache.get(expr)
  if (cached !== undefined) return cached
  let k: string
  switch (expr.kind) {
    case 'num':
      k = `n:${expr.value.toString()}`
      break
    case 'float':
      k = `f:${expr.value.toString()}`
      break
    case 'sym':
      k = `s:${expr.name}`
      break
    case 'str':
      k = `t:${expr.value}`
      break
    case 'add':
      k = `a(${expr.terms.map(key).join(',')})`
      break
    case 'mul':
      k = `m(${expr.factors.map(key).join(',')})`
      break
    case 'pow':
      k = `p(${key(expr.base)},${key(expr.exp)})`
      break
    case 'call':
      k = `c:${expr.name}(${expr.args.map(key).join(',')})`
      break
    case 'eq':
      k = `e(${key(expr.lhs)},${key(expr.rhs)})`
      break
    case 'list':
      k = `l:${expr.items.map(key).join('|')}`
      break
  }
  keyCache.set(expr, k)
  return k
}

export function equals(a: Expr, b: Expr): boolean {
  return a === b || key(a) === key(b)
}

function degreeOf(expr: Expr): number {
  switch (expr.kind) {
    case 'num':
      return 0
    case 'sym':
      return expr.name === 'pi' || expr.name === 'e' || expr.name === 'i' ? 0 : 1
    case 'pow': {
      if (expr.exp.kind === 'num' && expr.exp.value.isInteger) {
        const exp = Number(expr.exp.value.toBigInt())
        return Math.abs(exp) * degreeOf(expr.base)
      }
      return 0
    }
    case 'mul':
      return expr.factors.reduce((sum, f) => sum + degreeOf(f), 0)
    case 'add':
      return Math.max(0, ...expr.terms.map(degreeOf))
    default:
      return 0
  }
}

function factorRank(expr: Expr): number {
  switch (expr.kind) {
    case 'sym':
      return 0
    case 'pow':
      return 1
    case 'call':
      return 2
    case 'add':
      return 3
    default:
      return 4
  }
}

function extractCoefficient(term: Expr): { coeff: Rational; rest: Expr } {
  if (term.kind === 'num') return { coeff: term.value, rest: ONE }
  if (term.kind !== 'mul') return { coeff: Rational.ONE, rest: term }
  let coeff = Rational.ONE
  const rest: Expr[] = []
  for (const factor of term.factors) {
    if (factor.kind === 'num') coeff = coeff.mul(factor.value)
    else rest.push(factor)
  }
  let restExpr: Expr
  if (rest.length === 0) restExpr = ONE
  else if (rest.length === 1) restExpr = rest[0]
  else restExpr = mul(...rest)
  return { coeff, rest: restExpr }
}

export function add(...args: Expr[]): Expr {
  const groups = new Map<string, { coeff: Rational; rest: Expr }>()
  let constant = Rational.ZERO

  const collect = (term: Expr): void => {
    if (term.kind === 'add') {
      for (const t of term.terms) collect(t)
      return
    }
    if (term.kind === 'num') {
      constant = constant.add(term.value)
      return
    }
    const { coeff, rest } = extractCoefficient(term)
    const k = key(rest)
    const existing = groups.get(k)
    if (existing) existing.coeff = existing.coeff.add(coeff)
    else groups.set(k, { coeff, rest })
  }

  for (const arg of args) collect(arg)

  const terms: Expr[] = []
  for (const { coeff, rest } of groups.values()) {
    if (coeff.isZero) continue
    if (coeff.isOne) terms.push(rest)
    else terms.push(mul(num(coeff), rest))
  }
  terms.sort((a, b) => {
    const da = degreeOf(a)
    const db = degreeOf(b)
    if (da !== db) return db - da
    return key(a) < key(b) ? -1 : key(a) > key(b) ? 1 : 0
  })
  if (!constant.isZero) terms.push(num(constant))
  if (terms.length === 0) return ZERO
  if (terms.length === 1) return terms[0]
  return { kind: 'add', terms }
}

export function mul(...args: Expr[]): Expr {
  let coeff = Rational.ONE
  const powers = new Map<string, { base: Expr; exp: Expr }>()

  const collect = (factor: Expr): void => {
    if (factor.kind === 'num') {
      coeff = coeff.mul(factor.value)
      return
    }
    if (factor.kind === 'mul') {
      for (const f of factor.factors) collect(f)
      return
    }
    const base = factor.kind === 'pow' ? factor.base : factor
    const exp = factor.kind === 'pow' ? factor.exp : ONE
    const k = key(base)
    const existing = powers.get(k)
    if (existing) existing.exp = add(existing.exp, exp)
    else powers.set(k, { base, exp })
  }

  for (const arg of args) collect(arg)
  if (coeff.isZero) return ZERO

  const factors: Expr[] = []
  for (const { base, exp } of powers.values()) {
    if (exp.kind === 'num' && exp.value.isZero) continue
    if (exp.kind === 'num' && exp.value.isOne) {
      factors.push(base)
    } else {
      factors.push(pow(base, exp))
    }
  }
  factors.sort((a, b) => {
    const ra = factorRank(a)
    const rb = factorRank(b)
    if (ra !== rb) return ra - rb
    return key(a) < key(b) ? -1 : key(a) > key(b) ? 1 : 0
  })
  if (!coeff.isOne) factors.unshift(num(coeff))
  if (factors.length === 0) return num(coeff)
  if (factors.length === 1) return factors[0]
  return { kind: 'mul', factors }
}

export function neg(expr: Expr): Expr {
  return mul(NEG_ONE, expr)
}

export function sub(a: Expr, b: Expr): Expr {
  return add(a, neg(b))
}

export function div(a: Expr, b: Expr): Expr {
  if (b.kind === 'num' && b.value.isZero) {
    throw new CalcError('除数不能为零')
  }
  return mul(a, pow(b, NEG_ONE))
}

function rawPow(base: Expr, exp: Expr): Expr {
  return { kind: 'pow', base, exp }
}

/** 对非负整数 n，求其 q 次根的整数部分，并判断是否完全 q 次幂 */
function integerNthRoot(n: bigint, q: number): { root: bigint; exact: boolean } {
  if (n < 0n) throw new Error('负数不能求偶数次根')
  if (n === 0n) return { root: 0n, exact: true }
  if (q === 1) return { root: n, exact: true }
  if (q === 2) {
    if (n < 2n) return { root: n, exact: true }
    let x = BigInt(1) << BigInt(Math.ceil(n.toString(2).length / 2) + 1)
    while (true) {
      const y = (x + n / x) >> 1n
      if (y >= x) break
      x = y
    }
    while (x * x > n) x -= 1n
    while ((x + 1n) * (x + 1n) <= n) x += 1n
    return { root: x, exact: x * x === n }
  }
  const bits = n.toString(2).length
  let x = BigInt(1) << BigInt(Math.ceil(bits / q) + 1)
  const qBig = BigInt(q)
  while (true) {
    const xPow = x ** (qBig - 1n)
    const y = ((qBig - 1n) * x + n / xPow) / qBig
    if (y >= x) break
    x = y
  }
  while (x ** qBig > n) x -= 1n
  while ((x + 1n) ** qBig <= n) x += 1n
  return { root: x, exact: x ** qBig === n }
}

const SMALL_PRIMES: number[] = (() => {
  const primes: number[] = []
  for (let n = 2; n <= 10000; n++) {
    let ok = true
    for (const p of primes) {
      if (p * p > n) break
      if (n % p === 0) {
        ok = false
        break
      }
    }
    if (ok) primes.push(n)
  }
  return primes
})()

/** 把 n 分解成 out^q * inside 的形式，inside 不再含小素数的 q 次幂因子 */
function extractPower(n: bigint, q: number): { out: bigint; inside: bigint } {
  if (n <= 1n) return { out: 1n, inside: n }
  const perfect = integerNthRoot(n, q)
  if (perfect.exact) return { out: perfect.root, inside: 1n }
  let rest = n
  let out = 1n
  let inside = 1n
  for (const p of SMALL_PRIMES) {
    if (BigInt(p) * BigInt(p) > rest) break
    if (rest % BigInt(p) !== 0n) continue
    let e = 0
    const pBig = BigInt(p)
    while (rest % pBig === 0n) {
      rest /= pBig
      e++
    }
    const whole = Math.floor(e / q)
    const rem = e % q
    if (whole > 0) out *= pBig ** BigInt(whole)
    if (rem > 0) inside *= pBig ** BigInt(rem)
  }
  inside *= rest
  return { out, inside }
}

function iPower(exp: bigint): Expr {
  let e = exp % 4n
  if (e < 0n) e += 4n
  if (e === 0n) return ONE
  if (e === 1n) return I
  if (e === 2n) return NEG_ONE
  return neg(I)
}

/**
 * 化简 (base)^(p/q)（base 为有理数）。
 * 返回 null 表示没有可化简的结构，调用方保留原始 Pow 节点。
 */
function simplifyRationalPower(base: Rational, exp: Rational): Expr | null {
  const p = exp.n
  const q = exp.d
  if (q > 1000) return null
  if (base.isZero) return null

  if (base.sign < 0) {
    const mag = base.abs()
    if (q % 2n === 1n) {
      const magResult = simplifyRationalPower(mag, exp)
      const magnitude = magResult ?? rawPow(num(mag), num(exp))
      const sign = p % 2n === 0n ? 1 : -1
      return sign === 1 ? magnitude : neg(magnitude)
    }
    const magResult = simplifyRationalPower(mag, exp)
    const magnitude = magResult ?? rawPow(num(mag), num(exp))
    return mul(magnitude, iPower(p))
  }

  const qn = Number(q)
  const numExtract = extractPower(base.n, qn)
  const denExtract = extractPower(base.d, qn)
  const out = Rational.of(numExtract.out, denExtract.out)
  const inside = Rational.of(numExtract.inside, denExtract.inside)

  // p = k*q + r
  const k = Math.floor(Number(p) / qn)
  const r = Number(p) - k * qn

  let coeff = out.powInt(Number(p))
  if (k !== 0) coeff = coeff.mul(inside.powInt(k))

  const residual = r === 0 || inside.isOne ? ONE : rawPow(num(inside), num(Rational.of(r, qn)))
  if (coeff.isOne) {
    if (residual.kind === 'num' && residual.value.isOne) return ONE
    if (equals(residual, rawPow(num(base), num(exp)))) return null
    return residual
  }
  return mul(num(coeff), residual)
}

export function pow(base: Expr, exp: Expr): Expr {
  if (exp.kind === 'num') {
    const e = exp.value
    if (e.isZero) return ONE
    if (e.isOne) return base

    if (base.kind === 'num') {
      if (e.isInteger) {
        const n = Number(e.toBigInt())
        if (Number.isFinite(n) && Math.abs(n) <= 100000) {
          return num(base.value.powInt(n))
        }
        throw new CalcError('指数过大（整数幂上限 100000）')
      }
      const simplified = simplifyRationalPower(base.value, e)
      if (simplified) return simplified
    }

    if (base.kind === 'sym' && base.name === 'i' && e.isInteger) {
      return iPower(e.toBigInt())
    }

    if (base.kind === 'pow') {
      const safe =
        e.isInteger ||
        (base.base.kind === 'num' && base.base.value.sign > 0) ||
        (base.exp.kind === 'num' && base.exp.value.isInteger && e.isInteger)
      if (safe) {
        return pow(base.base, mul(base.exp, exp))
      }
    }

    if (base.kind === 'mul' && e.isInteger) {
      const n = Number(e.toBigInt())
      if (Number.isFinite(n) && Math.abs(n) <= 100) {
        return mul(...base.factors.map((f) => pow(f, exp)))
      }
    }
  }

  if (base.kind === 'num' && base.value.isOne) return ONE
  if (base.kind === 'num' && base.value.isZero) {
    if (exp.kind === 'num' && exp.value.sign > 0) return ZERO
    if (exp.kind === 'num' && exp.value.sign < 0) {
      throw new CalcError('零不能做负数次幂')
    }
  }
  return rawPow(base, exp)
}

// ---------------------------------------------------------------------------
// 精确初等函数与三角函数值
// ---------------------------------------------------------------------------

const PI_TIMES_12 = Rational.of(12)

function piCoefficient(expr: Expr): Rational | null {
  switch (expr.kind) {
    case 'num':
      return expr.value.isZero ? Rational.ZERO : null
    case 'sym':
      return expr.name === 'pi' ? Rational.ONE : null
    case 'mul': {
      let coeff = Rational.ONE
      let found = false
      for (const f of expr.factors) {
        if (f.kind === 'num') {
          coeff = coeff.mul(f.value)
        } else if (f.kind === 'sym' && f.name === 'pi' && !found) {
          found = true
        } else {
          return null
        }
      }
      return found ? coeff : null
    }
    case 'add': {
      let sum = Rational.ZERO
      let any = false
      for (const t of expr.terms) {
        const c = piCoefficient(t)
        if (c === null) return null
        sum = sum.add(c)
        any = true
      }
      return any ? sum : null
    }
    default:
      return null
  }
}

interface TrigTable {
  sin: Expr[]
  cos: Expr[]
}

let trigTable: TrigTable | null = null

function getTrigTable(): TrigTable {
  if (trigTable) return trigTable
  const q = (n: number, d: number): Expr => num(Rational.of(n, d))
  const rad = (n: number): Expr => rawPow(num(Rational.of(n)), HALF)
  const sqrt2 = rad(2)
  const sqrt3 = rad(3)
  const sqrt6 = rad(6)
  const sin: Expr[] = []
  const cos: Expr[] = []
  sin[0] = ZERO
  sin[1] = mul(q(1, 4), add(sqrt6, neg(sqrt2)))
  sin[2] = q(1, 2)
  sin[3] = mul(q(1, 2), sqrt2)
  sin[4] = mul(q(1, 2), sqrt3)
  sin[5] = mul(q(1, 4), add(sqrt6, sqrt2))
  sin[6] = ONE
  cos[0] = ONE
  cos[1] = mul(q(1, 4), add(sqrt6, sqrt2))
  cos[2] = mul(q(1, 2), sqrt3)
  cos[3] = mul(q(1, 2), sqrt2)
  cos[4] = q(1, 2)
  cos[5] = mul(q(1, 4), add(sqrt6, neg(sqrt2)))
  cos[6] = ZERO
  for (let n = 7; n <= 11; n++) {
    sin[n] = sin[12 - n]
    cos[n] = neg(cos[12 - n])
  }
  for (let n = 12; n <= 23; n++) {
    sin[n] = neg(sin[n - 12])
    cos[n] = neg(cos[n - 12])
  }
  trigTable = { sin, cos }
  return trigTable
}

function exactTrig(name: 'sin' | 'cos' | 'tan', arg: Expr): Expr | null {
  const coeff = piCoefficient(arg)
  if (coeff === null) return null
  const nRat = coeff.mul(PI_TIMES_12)
  if (!nRat.isInteger) return null
  let n = Number(nRat.toBigInt() % 24n)
  if (n < 0) n += 24
  const table = getTrigTable()
  if (name === 'sin') return table.sin[n]
  if (name === 'cos') return table.cos[n]
  const c = table.cos[n]
  if (c.kind === 'num' && c.value.isZero) return null
  return div(table.sin[n], c)
}

function isNegativeMul(expr: Expr): boolean {
  return (
    expr.kind === 'mul' &&
    expr.factors.some((f) => f.kind === 'num' && f.value.sign < 0)
  )
}

function exactAsin(arg: Expr): Expr | null {
  if (arg.kind === 'num') {
    const v = arg.value
    if (v.isZero) return ZERO
    if (v.isOne) return mul(HALF, PI)
    if (v.isMinusOne) return neg(mul(HALF, PI))
    if (v.equals(Rational.HALF)) return mul(num(Rational.of(1, 6)), PI)
    if (v.equals(Rational.of(-1, 2))) return neg(mul(num(Rational.of(1, 6)), PI))
    return null
  }
  const table = getTrigTable()
  // sqrt(2)/2, sqrt(3)/2 以及相反数
  const halfSqrt2 = mul(HALF, rawPow(num(Rational.of(2)), HALF))
  const halfSqrt3 = mul(HALF, rawPow(num(Rational.of(3)), HALF))
  if (equals(arg, halfSqrt2)) return mul(num(Rational.of(1, 4)), PI)
  if (equals(arg, halfSqrt3)) return mul(num(Rational.of(1, 3)), PI)
  if (equals(arg, neg(halfSqrt2))) return neg(mul(num(Rational.of(1, 4)), PI))
  if (equals(arg, neg(halfSqrt3))) return neg(mul(num(Rational.of(1, 3)), PI))
  void table
  return null
}

function exactAtan(arg: Expr): Expr | null {
  if (arg.kind === 'num') {
    if (arg.value.isZero) return ZERO
    if (arg.value.isOne) return mul(num(Rational.of(1, 4)), PI)
    if (arg.value.isMinusOne) return neg(mul(num(Rational.of(1, 4)), PI))
  }
  return null
}

function floorRational(r: Rational): bigint {
  if (r.n >= 0n) return r.n / r.d
  return -((-r.n + r.d - 1n) / r.d)
}

function roundRational(r: Rational): bigint {
  const sign = r.sign < 0 ? -1n : 1n
  const abs = r.abs()
  const q = abs.n / abs.d
  const rem = abs.n % abs.d
  return sign * (rem * 2n >= abs.d ? q + 1n : q)
}

function factorialExpr(n: bigint): Expr {
  if (n < 0n) throw new CalcError('阶乘要求非负整数')
  if (n > 20000n) throw new CalcError('阶乘参数过大（上限 20000）')
  let result = 1n
  for (let i = 2n; i <= n; i++) result *= i
  return num(Rational.of(result))
}

function binomialExpr(n: bigint, k: bigint): Expr {
  if (n < 0n || k < 0n || k > n) throw new CalcError('二项式系数要求 0 <= k <= n')
  if (n > 20000n) throw new CalcError('二项式系数参数过大（上限 20000）')
  const kk = k > n - k ? n - k : k
  let result = 1n
  for (let i = 1n; i <= kk; i++) {
    result = (result * (n - kk + i)) / i
  }
  return num(Rational.of(result))
}

function rawCall(name: string, args: Expr[]): Expr {
  return { kind: 'call', name, args }
}

function callAbs(arg: Expr): Expr {
  if (arg.kind === 'num') return num(arg.value.abs())
  if (arg.kind === 'mul') {
    const coeff = arg.factors.find((f) => f.kind === 'num')
    if (coeff && coeff.kind === 'num' && coeff.value.sign < 0) {
      const rest = mul(...arg.factors.filter((f) => f !== coeff), num(coeff.value.abs()))
      return callAbs(rest)
    }
  }
  return rawCall('abs', [arg])
}

function dispatchCall(name: string, args: Expr[]): Expr {
  const lower = name.toLowerCase()
  const arity = (n: number): void => {
    if (args.length !== n) {
      throw new CalcError(`${name} 需要 ${n} 个参数，实际收到 ${args.length} 个`)
    }
  }

  switch (lower) {
    case 'sqrt': {
      arity(1)
      return pow(args[0], HALF)
    }
    case 'cbrt': {
      arity(1)
      return pow(args[0], num(Rational.of(1, 3)))
    }
    case 'root': {
      arity(2)
      const n = args[1]
      if (n.kind === 'num' && n.value.isInteger && n.value.n > 0n && n.value.n <= 1000n) {
        return pow(args[0], num(Rational.of(1n, n.value.n)))
      }
      throw new CalcError('root 的第二个参数必须是正整数')
    }
    case 'pow': {
      arity(2)
      return pow(args[0], args[1])
    }
    case 'abs':
      arity(1)
      return callAbs(args[0])
    case 'sign': {
      arity(1)
      if (args[0].kind === 'num') return num(Rational.of(args[0].value.sign))
      return rawCall('sign', [args[0]])
    }
    case 'floor': {
      arity(1)
      if (args[0].kind === 'num') return num(Rational.of(floorRational(args[0].value)))
      return rawCall('floor', [args[0]])
    }
    case 'ceil': {
      arity(1)
      if (args[0].kind === 'num') {
        const v = args[0].value
        return num(Rational.of(-floorRational(v.neg())))
      }
      return rawCall('ceil', [args[0]])
    }
    case 'round': {
      arity(1)
      if (args[0].kind === 'num') return num(Rational.of(roundRational(args[0].value)))
      return rawCall('round', [args[0]])
    }
    case 'exp': {
      arity(1)
      const a = args[0]
      if (a.kind === 'num' && a.value.isZero) return ONE
      if (a.kind === 'num' && a.value.isOne) return E
      if (a.kind === 'call' && a.name === 'ln') return a.args[0]
      return rawCall('exp', [a])
    }
    case 'ln':
    case 'log': {
      if (lower === 'log' && args.length === 2) {
        return mul(call('ln', args[0]), pow(call('ln', args[1]), NEG_ONE))
      }
      arity(1)
      const a = args[0]
      if (a.kind === 'num' && a.value.isOne) return ZERO
      if (a.kind === 'sym' && a.name === 'e') return ONE
      if (a.kind === 'pow' && equals(a.base, E)) return a.exp
      if (a.kind === 'call' && a.name === 'exp') return a.args[0]
      return rawCall('ln', [a])
    }
    case 'log10': {
      arity(1)
      return mul(call('ln', args[0]), pow(call('ln', num(Rational.of(10))), NEG_ONE))
    }
    case 'log2': {
      arity(1)
      return mul(call('ln', args[0]), pow(call('ln', TWO), NEG_ONE))
    }
    case 'sin':
    case 'cos':
    case 'tan': {
      arity(1)
      const a = args[0]
      const exact = exactTrig(lower, a)
      if (exact !== null) return exact
      if (isNegativeMul(a)) {
        const positive = mul(a, NEG_ONE)
        if (lower === 'cos') return call('cos', positive)
        return neg(call(lower, positive))
      }
      return rawCall(lower, [a])
    }
    case 'asin': {
      arity(1)
      const exact = exactAsin(args[0])
      return exact ?? rawCall('asin', [args[0]])
    }
    case 'acos': {
      arity(1)
      const asin = exactAsin(args[0])
      if (asin !== null) return sub(mul(HALF, PI), asin)
      return rawCall('acos', [args[0]])
    }
    case 'atan': {
      arity(1)
      const exact = exactAtan(args[0])
      return exact ?? rawCall('atan', [args[0]])
    }
    case 'sinh': {
      arity(1)
      if (args[0].kind === 'num' && args[0].value.isZero) return ZERO
      return rawCall('sinh', [args[0]])
    }
    case 'cosh': {
      arity(1)
      if (args[0].kind === 'num' && args[0].value.isZero) return ONE
      return rawCall('cosh', [args[0]])
    }
    case 'tanh': {
      arity(1)
      if (args[0].kind === 'num' && args[0].value.isZero) return ZERO
      return rawCall('tanh', [args[0]])
    }
    case 'factorial': {
      arity(1)
      if (args[0].kind === 'num' && args[0].value.isInteger) {
        return factorialExpr(args[0].value.toBigInt())
      }
      return rawCall('factorial', [args[0]])
    }
    case 'binomial': {
      arity(2)
      if (
        args[0].kind === 'num' &&
        args[0].value.isInteger &&
        args[1].kind === 'num' &&
        args[1].value.isInteger
      ) {
        return binomialExpr(args[0].value.toBigInt(), args[1].value.toBigInt())
      }
      return rawCall('binomial', args)
    }
    case 'gcd':
    case 'lcm': {
      if (args.length < 2) throw new CalcError(`${name} 至少需要 2 个参数`)
      if (args.every((a) => a.kind === 'num' && a.value.isInteger)) {
        let result = (args[0] as NumExpr).value.toBigInt()
        for (let i = 1; i < args.length; i++) {
          const v = (args[i] as NumExpr).value.toBigInt()
          if (lower === 'gcd') result = bigintGcd(result, v)
          else result = result === 0n || v === 0n ? 0n : (result / bigintGcd(result, v)) * v
        }
        return num(Rational.of(result < 0n ? -result : result))
      }
      return rawCall(lower, args)
    }
    case 'mod': {
      arity(2)
      if (
        args[0].kind === 'num' &&
        args[1].kind === 'num' &&
        args[1].value.isInteger
      ) {
        const b = args[1].value.toBigInt()
        if (b === 0n) throw new CalcError('mod 的除数不能为零')
        if (args[0].value.isInteger) {
          let r = args[0].value.toBigInt() % b
          if (r < 0n) r += b < 0n ? -b : b
          return num(Rational.of(r))
        }
        const q = floorRational(args[0].value.div(args[1].value))
        return num(args[0].value.sub(args[1].value.mul(Rational.of(q))))
      }
      return rawCall('mod', args)
    }
    case 'min':
    case 'max': {
      if (args.length === 0) throw new CalcError(`${name} 需要至少 1 个参数`)
      if (args.every((a) => a.kind === 'num')) {
        let best = (args[0] as NumExpr).value
        for (const a of args.slice(1)) {
          const v = (a as NumExpr).value
          if (lower === 'min' ? v.compare(best) < 0 : v.compare(best) > 0) best = v
        }
        return num(best)
      }
      return rawCall(lower, args)
    }
    case 'eq': {
      arity(2)
      return eqExpr(args[0], args[1])
    }
    default:
      return rawCall(name, args)
  }
}

function eqExpr(lhs: Expr, rhs: Expr): Expr {
  return { kind: 'eq', lhs, rhs }
}

export function call(name: string, ...args: Expr[]): Expr {
  return dispatchCall(name, args)
}
