/**
 * 任意精度十进制浮点数（BigFloat）。
 *
 * 表示：value = sign * digits * 10^exp
 * - digits 为十进制整数尾数（非负）
 * - exp 为十进制指数
 * - prec 为有效数字位数
 *
 * 所有运算都保留保护位，只在返回时按目标精度舍入，从而支持 50+ 位
 * 有效数字的近似计算。精确计算始终优先使用 Rational，BigFloat 只用于
 * 显式近似（N(...)）和超越函数的数值求值。
 */

import { Rational } from './rational'

const GUARD = 14

export class BigFloat {
  readonly sign: 1 | -1
  readonly digits: bigint
  readonly exp: number
  readonly prec: number

  private constructor(sign: 1 | -1, digits: bigint, exp: number, prec: number) {
    this.sign = sign
    this.digits = digits
    this.exp = exp
    this.prec = prec
  }

  static normalize(
    sign: 1 | -1,
    digits: bigint,
    exp: number,
    prec: number
  ): BigFloat {
    if (digits === 0n) return new BigFloat(1, 0n, 0, prec)
    if (!Number.isFinite(exp) || Math.abs(exp) > 1e15) {
      throw new Error('数值指数超出范围')
    }
    let s = digits.toString()
    let e = exp
    if (s.length > prec) {
      const keep = BigInt(s.slice(0, prec))
      const next = s.charCodeAt(prec) - 48
      let kept = keep
      e += s.length - prec
      if (next >= 5) kept += 1n
      if (kept.toString().length > prec) {
        kept /= 10n
        e += 1
      }
      digits = kept
    }
    while (digits !== 0n && digits % 10n === 0n) {
      digits /= 10n
      e += 1
    }
    return new BigFloat(sign, digits, e, prec)
  }

  static zero(prec = 30): BigFloat {
    return new BigFloat(1, 0n, 0, prec)
  }

  static one(prec = 30): BigFloat {
    return new BigFloat(1, 1n, 0, prec)
  }

  static fromInt(value: bigint, prec = 30): BigFloat {
    if (value === 0n) return BigFloat.zero(prec)
    const sign: 1 | -1 = value < 0n ? -1 : 1
    const digits = value < 0n ? -value : value
    return BigFloat.normalize(sign, digits, 0, prec)
  }

  static fromNumber(value: number, prec = 30): BigFloat {
    if (!Number.isFinite(value)) throw new Error('无法表示非有限数')
    return BigFloat.fromString(String(value), prec)
  }

  static fromString(text: string, prec = 30): BigFloat {
    const t = text.trim()
    const m = /^([+-]?)(\d*)(?:\.(\d*))?(?:[eE]([+-]?\d+))?$/.exec(t)
    if (!m) throw new Error(`不是合法的数字：${text}`)
    const sign: 1 | -1 = m[1] === '-' ? -1 : 1
    const intPart = m[2] || ''
    const fracPart = m[3] || ''
    const expPart = m[4] ? parseInt(m[4], 10) : 0
    if (intPart === '' && fracPart === '') throw new Error(`不是合法的数字：${text}`)
    const digits = BigInt((intPart + fracPart) || '0')
    return BigFloat.normalize(sign, digits, expPart - fracPart.length, prec)
  }

  static fromRational(r: Rational, prec = 30): BigFloat {
    if (r.isZero) return BigFloat.zero(prec)
    const sign: 1 | -1 = r.sign < 0 ? -1 : 1
    const a = r.n < 0n ? -r.n : r.n
    const b = r.d
    if (b === 1n) return BigFloat.fromInt(r.n, prec)
    const work = prec + GUARD
    const desired = work + 6
    const shift = desired - a.toString().length + b.toString().length - 1
    let q: bigint
    let exp: number
    if (shift >= 0) {
      q = (a * 10n ** BigInt(shift)) / b
      exp = -shift
    } else {
      q = a / (b * 10n ** BigInt(-shift))
      exp = -shift
    }
    return BigFloat.normalize(sign, q, exp, prec)
  }

  withPrecision(prec: number): BigFloat {
    return BigFloat.normalize(this.sign, this.digits, this.exp, prec)
  }

  get isZero(): boolean {
    return this.digits === 0n
  }

  get isNegative(): boolean {
    return this.sign < 0 && this.digits !== 0n
  }

  get isPositive(): boolean {
    return this.sign > 0 && this.digits !== 0n
  }

  get isInteger(): boolean {
    if (this.isZero) return true
    if (this.exp >= 0) return true
    return this.digits % 10n ** BigInt(-this.exp) === 0n
  }

  get order(): number {
    // floor(log10(|x|))：value 位于 [10^(order), 10^(order+1))
    if (this.isZero) return 0
    return this.digits.toString().length - 1 + this.exp
  }

  abs(): BigFloat {
    return new BigFloat(1, this.digits, this.exp, this.prec)
  }

  neg(): BigFloat {
    return new BigFloat(this.sign === 1 ? -1 : 1, this.digits, this.exp, this.prec)
  }

  scale10(k: number, prec = this.prec): BigFloat {
    return BigFloat.normalize(this.sign, this.digits, this.exp + k, prec)
  }

  add(o: BigFloat, prec = Math.max(this.prec, o.prec)): BigFloat {
    const work = prec + GUARD
    const a = this.withPrecision(work)
    const b = o.withPrecision(work)
    if (a.isZero) return b.withPrecision(prec)
    if (b.isZero) return a.withPrecision(prec)
    if (a.exp - b.exp > work + 6) return a.withPrecision(prec)
    if (b.exp - a.exp > work + 6) return b.withPrecision(prec)
    const e = Math.min(a.exp, b.exp)
    const da = a.digits * 10n ** BigInt(a.exp - e) * BigInt(a.sign)
    const db = b.digits * 10n ** BigInt(b.exp - e) * BigInt(b.sign)
    const sum = da + db
    if (sum === 0n) return BigFloat.zero(prec)
    const sign: 1 | -1 = sum < 0n ? -1 : 1
    return BigFloat.normalize(sign, sum < 0n ? -sum : sum, e, prec)
  }

  sub(o: BigFloat, prec?: number): BigFloat {
    return this.add(o.neg(), prec)
  }

  mul(o: BigFloat, prec = Math.max(this.prec, o.prec)): BigFloat {
    if (this.isZero || o.isZero) return BigFloat.zero(prec)
    const work = prec + GUARD
    const a = this.withPrecision(work)
    const b = o.withPrecision(work)
    return BigFloat.normalize(
      a.sign === b.sign ? 1 : -1,
      a.digits * b.digits,
      a.exp + b.exp,
      prec
    )
  }

  div(o: BigFloat, prec = Math.max(this.prec, o.prec)): BigFloat {
    if (o.isZero) throw new Error('除数不能为零')
    if (this.isZero) return BigFloat.zero(prec)
    const work = prec + GUARD
    const a = this.withPrecision(work)
    const b = o.withPrecision(work)
    const desired = work + 6
    const shift = desired - a.digits.toString().length + b.digits.toString().length - 1
    let q: bigint
    let exp: number
    if (shift >= 0) {
      q = (a.digits * 10n ** BigInt(shift)) / b.digits
      exp = a.exp - b.exp - shift
    } else {
      q = a.digits / (b.digits * 10n ** BigInt(-shift))
      exp = a.exp - b.exp - shift
    }
    return BigFloat.normalize(a.sign === b.sign ? 1 : -1, q, exp, prec)
  }

  recip(prec = this.prec): BigFloat {
    return BigFloat.one(prec + GUARD).div(this, prec)
  }

  powInt(e: number, prec = this.prec): BigFloat {
    if (!Number.isInteger(e)) throw new Error('指数必须是整数')
    if (Math.abs(e) > 1000000) throw new Error('指数过大')
    if (e === 0) return BigFloat.one(prec)
    if (this.isZero) {
      if (e < 0) throw new Error('零不能做负数次幂')
      return BigFloat.zero(prec)
    }
    const work = prec + GUARD
    let base = this.withPrecision(work)
    let exp = Math.abs(e)
    let result = BigFloat.one(work)
    while (exp > 0) {
      if (exp & 1) result = result.mul(base, work)
      base = base.mul(base, work)
      exp >>= 1
    }
    return e < 0 ? result.recip(prec) : result.withPrecision(prec)
  }

  compare(o: BigFloat): number {
    if (this.isZero && o.isZero) return 0
    if (this.isZero) return o.isNegative ? 1 : -1
    if (o.isZero) return this.isNegative ? -1 : 1
    if (this.sign !== o.sign) return this.sign < o.sign ? -1 : 1
    const s = this.sign
    const orderA = this.order
    const orderB = o.order
    if (orderA !== orderB) {
      return (orderA < orderB ? -1 : 1) * s
    }
    const e = Math.min(this.exp, o.exp)
    const da = this.digits * 10n ** BigInt(this.exp - e)
    const db = o.digits * 10n ** BigInt(o.exp - e)
    const c = da < db ? -1 : da > db ? 1 : 0
    return c * s
  }

  equals(o: BigFloat): boolean {
    return (
      this.sign === o.sign &&
      this.digits === o.digits &&
      this.exp === o.exp &&
      this.isZero === o.isZero
    )
  }

  toNumber(): number {
    const n = Number(this.toString())
    if (!Number.isFinite(n)) {
      return this.sign < 0 ? -Infinity : Infinity
    }
    return n
  }

  toBigInt(): bigint {
    if (!this.isInteger) throw new Error('不是整数')
    if (this.exp >= 0) return BigInt(this.sign) * this.digits * 10n ** BigInt(this.exp)
    return BigInt(this.sign) * (this.digits / 10n ** BigInt(-this.exp))
  }

  floor(): BigFloat {
    return this.roundMode('floor')
  }

  ceil(): BigFloat {
    return this.roundMode('ceil')
  }

  round(): BigFloat {
    return this.roundMode('round')
  }

  private roundMode(mode: 'floor' | 'ceil' | 'round'): BigFloat {
    if (this.isZero || this.exp >= 0) return this
    const divisor = 10n ** BigInt(-this.exp)
    const q = this.digits / divisor
    const rem = this.digits % divisor
    let mag = q
    if (mode === 'floor') {
      if (this.sign < 0 && rem !== 0n) mag += 1n
    } else if (mode === 'ceil') {
      if (this.sign > 0 && rem !== 0n) mag += 1n
    } else {
      if (rem * 2n >= divisor) mag += 1n
    }
    const value = BigInt(this.sign) * mag
    return BigFloat.fromInt(value, this.prec)
  }

  toString(): string {
    if (this.isZero) return '0'
    const s = this.digits.toString()
    const order = s.length - 1 + this.exp
    const sign = this.sign < 0 ? '-' : ''
    if (order >= 16 || order <= -7) {
      const mantissa = s.length > 1 ? `${s[0]}.${s.slice(1)}` : s
      return `${sign}${mantissa}e${order >= 0 ? '+' : ''}${order}`
    }
    const point = s.length + this.exp
    if (point >= s.length) {
      return sign + s + '0'.repeat(point - s.length)
    }
    if (point > 0) {
      return sign + s.slice(0, point) + '.' + s.slice(point)
    }
    return sign + '0.' + '0'.repeat(-point) + s
  }

  toPrecisionString(sig: number): string {
    return this.withPrecision(Math.max(1, sig)).toString()
  }
}

function bf(n: number | bigint | string, prec: number): BigFloat {
  if (typeof n === 'bigint') return BigFloat.fromInt(n, prec)
  if (typeof n === 'number') return BigFloat.fromNumber(n, prec)
  return BigFloat.fromString(n, prec)
}

function epsilon(work: number): BigFloat {
  return BigFloat.fromString(`1e-${work + 4}`, work)
}

const piCache = new Map<number, BigFloat>()
const ln10Cache = new Map<number, BigFloat>()
const ln2Cache = new Map<number, BigFloat>()

function atanInv(n: number, work: number): BigFloat {
  const z = BigFloat.one(work).div(bf(n, work), work)
  const z2 = z.mul(z, work)
  let term = z
  let sum = z
  for (let k = 1; k < 100000; k++) {
    term = term.mul(z2, work).neg()
    const contribution = term.div(bf(2 * k + 1, work), work)
    sum = sum.add(contribution, work)
    if (contribution.abs().compare(epsilon(work)) < 0) break
  }
  return sum
}

export function bfPi(prec: number): BigFloat {
  const cached = piCache.get(prec)
  if (cached) return cached
  const work = prec + 18
  const a = atanInv(5, work).mul(bf(16, work), work)
  const b = atanInv(239, work).mul(bf(4, work), work)
  const value = a.sub(b, prec)
  piCache.set(prec, value)
  return value
}

function atanhSeries(z: BigFloat, work: number): BigFloat {
  const z2 = z.mul(z, work)
  let term = z
  let sum = z
  for (let k = 1; k < 100000; k++) {
    term = term.mul(z2, work)
    const contribution = term.div(bf(2 * k + 1, work), work)
    sum = sum.add(contribution, work)
    if (contribution.abs().compare(epsilon(work)) < 0) break
  }
  return sum
}

export function bfLn2(prec: number): BigFloat {
  const cached = ln2Cache.get(prec)
  if (cached) return cached
  const work = prec + 18
  const z = BigFloat.one(work).div(bf(3, work), work)
  const value = atanhSeries(z, work).mul(bf(2, work), prec)
  ln2Cache.set(prec, value)
  return value
}

export function bfLn10(prec: number): BigFloat {
  const cached = ln10Cache.get(prec)
  if (cached) return cached
  const work = prec + 18
  const ln2 = bfLn2(work)
  const z = BigFloat.one(work).div(bf(9, work), work)
  const ln5over4 = atanhSeries(z, work).mul(bf(2, work), work)
  const value = ln2.mul(bf(3, work), work).add(ln5over4, prec)
  ln10Cache.set(prec, value)
  return value
}

export function bfLn(x: BigFloat, prec = x.prec): BigFloat {
  if (x.isZero || x.isNegative) throw new Error('对数要求正数')
  const work = prec + GUARD + 8
  const xw = x.withPrecision(work)
  if (xw.digits === 1n && xw.exp === 0) return BigFloat.zero(prec)
  const order = xw.order
  const s = xw.digits.toString()
  // m = x / 10^order，位于 [1, 10)
  let m = BigFloat.normalize(1, xw.digits, xw.exp - order, work)
  // 多次开方，把 m 拉近 1，加快 atanh 级数收敛
  for (let i = 0; i < 5; i++) {
    m = bfSqrt(m, work)
  }
  const z = m.sub(BigFloat.one(work), work).div(m.add(BigFloat.one(work), work), work)
  const lnM = atanhSeries(z, work).mul(bf(2, work), work).mul(bf(32, work), work)
  const ln10 = bfLn10(work)
  const result = lnM.add(ln10.mul(bf(order, work), work), prec)
  return result
}

export function bfExp(x: BigFloat, prec = x.prec): BigFloat {
  const work = prec + GUARD + 8
  const xw = x.withPrecision(work)
  if (xw.isZero) return BigFloat.one(prec)
  if (Math.abs(xw.order) > 15) throw new Error('指数函数的参数过大')
  const ln10 = bfLn10(work)
  const kFloat = xw.div(ln10, work)
  const k = Math.round(kFloat.toNumber())
  let r = xw.sub(ln10.mul(bf(k, work), work), work)
  // 泰勒级数
  let term = BigFloat.one(work)
  let sum = BigFloat.one(work)
  const eps = epsilon(work)
  for (let i = 1; i < 100000; i++) {
    term = term.mul(r, work).div(bf(i, work), work)
    sum = sum.add(term, work)
    if (term.abs().compare(eps) < 0) break
  }
  // 乘以 10^k（精确的十进制指数平移）
  let result = sum.scale10(k, work)
  if (Math.abs(result.order) > 1e7) throw new Error('结果数量级过大')
  return result.withPrecision(prec)
}

export function bfSqrt(x: BigFloat, prec = x.prec): BigFloat {
  if (x.isNegative) throw new Error('负数不能开平方（精确模式请使用复数）')
  if (x.isZero) return BigFloat.zero(prec)
  const work = prec + GUARD + 10
  const xw = x.withPrecision(work)
  const order = xw.order
  const h = Math.floor(order / 2)
  const m = BigFloat.normalize(1, xw.digits, xw.exp - 2 * h, work)
  // 用双精度数给一个约 15 位的初值，后续牛顿迭代翻倍收敛
  let g = BigFloat.fromNumber(Math.sqrt(m.toNumber()), work).scale10(h, work)
  const two = bf(2, work)
  for (let i = 0; i < 8; i++) {
    g = g.add(xw.div(g, work), work).div(two, work)
  }
  return g.withPrecision(prec)
}

export function bfAtan(x: BigFloat, prec = x.prec): BigFloat {
  const work = prec + GUARD + 10
  const xw = x.withPrecision(work)
  if (xw.isZero) return BigFloat.zero(prec)
  const sign = xw.isNegative ? -1 : 1
  let a = xw.abs()
  if (a.compare(BigFloat.one(work)) > 0) {
    const pi = bfPi(work)
    const inv = BigFloat.one(work).div(a, work)
    const inner = bfAtan(inv, work)
    const result = pi.div(bf(2, work), work).sub(inner, work)
    return (sign < 0 ? result.neg() : result).withPrecision(prec)
  }
  // atan(x) = 2 atan(x / (1 + sqrt(1 + x^2)))，连续三次缩小参数
  const one = BigFloat.one(work)
  for (let i = 0; i < 3; i++) {
    const root = bfSqrt(one.add(a.mul(a, work), work), work)
    a = a.div(one.add(root, work), work)
  }
  const a2 = a.mul(a, work)
  let term = a
  let sum = a
  const eps = epsilon(work)
  for (let k = 1; k < 100000; k++) {
    term = term.mul(a2, work).neg()
    const contribution = term.div(bf(2 * k + 1, work), work)
    sum = sum.add(contribution, work)
    if (contribution.abs().compare(eps) < 0) break
  }
  sum = sum.mul(bf(8, work), work) // 2^3
  const result = sign < 0 ? sum.neg() : sum
  return result.withPrecision(prec)
}

function reduceAngle(x: BigFloat, work: number): BigFloat {
  const pi = bfPi(work + 6)
  const twoPi = pi.mul(bf(2, work), work)
  const qFloat = x.div(twoPi, work)
  const qNum = qFloat.toNumber()
  if (!Number.isFinite(qNum) || Math.abs(qNum) > 1e14) {
    throw new Error('三角函数的参数过大，无法可靠规约')
  }
  const q = Math.round(qNum)
  let r = x.sub(twoPi.mul(bf(q, work), work), work)
  if (r.compare(pi) > 0) r = r.sub(twoPi, work)
  if (r.compare(pi.neg()) < 0) r = r.add(twoPi, work)
  return r
}

export function bfSin(x: BigFloat, prec = x.prec): BigFloat {
  const work = prec + GUARD + 8
  let r = reduceAngle(x.withPrecision(work), work)
  const pi = bfPi(work)
  const halfPi = pi.div(bf(2, work), work)
  if (r.compare(halfPi) > 0) r = pi.sub(r, work)
  else if (r.compare(halfPi.neg()) < 0) r = pi.neg().sub(r, work)
  const r2 = r.mul(r, work)
  let term = r
  let sum = r
  const eps = epsilon(work)
  for (let k = 1; k < 100000; k++) {
    term = term.mul(r2, work).neg().div(bf((2 * k) * (2 * k + 1), work), work)
    sum = sum.add(term, work)
    if (term.abs().compare(eps) < 0) break
  }
  return sum.withPrecision(prec)
}

export function bfCos(x: BigFloat, prec = x.prec): BigFloat {
  const work = prec + GUARD + 8
  let r = reduceAngle(x.withPrecision(work), work)
  const pi = bfPi(work)
  const halfPi = pi.div(bf(2, work), work)
  let sign = 1
  if (r.compare(halfPi) > 0) {
    r = pi.sub(r, work)
    sign = -1
  } else if (r.compare(halfPi.neg()) < 0) {
    r = pi.neg().sub(r, work)
    sign = -1
  }
  const r2 = r.mul(r, work)
  let term = BigFloat.one(work)
  let sum = BigFloat.one(work)
  const eps = epsilon(work)
  for (let k = 1; k < 100000; k++) {
    term = term.mul(r2, work).neg().div(bf((2 * k - 1) * (2 * k), work), work)
    sum = sum.add(term, work)
    if (term.abs().compare(eps) < 0) break
  }
  return (sign < 0 ? sum.neg() : sum).withPrecision(prec)
}

export function bfTan(x: BigFloat, prec = x.prec): BigFloat {
  const work = prec + GUARD + 8
  const s = bfSin(x, work)
  const c = bfCos(x, work)
  if (c.abs().compare(epsilon(work)) < 0) throw new Error('正切在该点无定义')
  return s.div(c, prec)
}

export function bfAsin(x: BigFloat, prec = x.prec): BigFloat {
  const work = prec + GUARD + 8
  const xw = x.withPrecision(work)
  if (xw.compare(BigFloat.one(work)) > 0 || xw.compare(BigFloat.one(work).neg()) < 0) {
    throw new Error('asin 的定义域为 [-1, 1]')
  }
  const one = BigFloat.one(work)
  const sq = one.sub(xw.mul(xw, work), work)
  if (sq.compare(epsilon(work)) <= 0) {
    const halfPi = bfPi(work).div(bf(2, work), work)
    return (xw.isNegative ? halfPi.neg() : halfPi).withPrecision(prec)
  }
  const denom = bfSqrt(sq, work)
  return bfAtan(xw.div(denom, work), prec)
}

export function bfAcos(x: BigFloat, prec = x.prec): BigFloat {
  const work = prec + GUARD + 8
  const halfPi = bfPi(work).div(bf(2, work), work)
  return halfPi.sub(bfAsin(x, work), prec)
}

export function bfSinh(x: BigFloat, prec = x.prec): BigFloat {
  const work = prec + GUARD + 8
  const a = bfExp(x, work)
  const b = bfExp(x.neg(), work)
  return a.sub(b, work).div(bf(2, work), prec)
}

export function bfCosh(x: BigFloat, prec = x.prec): BigFloat {
  const work = prec + GUARD + 8
  const a = bfExp(x, work)
  const b = bfExp(x.neg(), work)
  return a.add(b, work).div(bf(2, work), prec)
}

export function bfTanh(x: BigFloat, prec = x.prec): BigFloat {
  const work = prec + GUARD + 8
  const e = bfExp(x.mul(bf(2, work), work), work)
  return e.sub(BigFloat.one(work), work).div(e.add(BigFloat.one(work), work), prec)
}

export function bfPow(a: BigFloat, b: BigFloat, prec = Math.max(a.prec, b.prec)): BigFloat {
  if (a.isZero) {
    if (b.isZero) return BigFloat.one(prec)
    if (b.isNegative) throw new Error('零不能做负数次幂')
    return BigFloat.zero(prec)
  }
  if (b.isInteger) {
    const n = b.toNumber()
    if (Number.isSafeInteger(n) && Math.abs(n) <= 1000000) {
      return a.powInt(n, prec)
    }
  }
  if (a.isNegative) throw new Error('负数的非整数次幂在实数范围内无定义')
  return bfExp(b.mul(bfLn(a, prec + 10), prec + 10), prec)
}

export function bfLog10(x: BigFloat, prec = x.prec): BigFloat {
  const work = prec + 10
  return bfLn(x, work).div(bfLn10(work), prec)
}

export function bfLog2(x: BigFloat, prec = x.prec): BigFloat {
  const work = prec + 10
  return bfLn(x, work).div(bfLn2(work), prec)
}
