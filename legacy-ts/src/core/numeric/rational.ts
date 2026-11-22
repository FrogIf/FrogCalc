/**
 * 精确有理数：分子/分母均为任意精度 BigInt，自动约分，分母恒为正。
 * 计算器的所有数字字面量都以 Rational 存储，从源头避免浮点误差。
 */

function gcd(a: bigint, b: bigint): bigint {
  a = a < 0n ? -a : a
  b = b < 0n ? -b : b
  while (b !== 0n) {
    const t = a % b
    a = b
    b = t
  }
  return a
}

export class Rational {
  readonly n: bigint
  readonly d: bigint

  private constructor(n: bigint, d: bigint) {
    this.n = n
    this.d = d
  }

  static readonly ZERO = new Rational(0n, 1n)
  static readonly ONE = new Rational(1n, 1n)
  static readonly NEG_ONE = new Rational(-1n, 1n)
  static readonly TWO = new Rational(2n, 1n)
  static readonly HALF = new Rational(1n, 2n)

  static of(n: bigint | number | string, d: bigint | number | string = 1n): Rational {
    let nn = typeof n === 'bigint' ? n : BigInt(n)
    let dd = typeof d === 'bigint' ? d : BigInt(d)
    if (dd === 0n) throw new Error('分母不能为零')
    if (dd < 0n) {
      nn = -nn
      dd = -dd
    }
    const g = gcd(nn, dd)
    if (g > 1n) {
      nn /= g
      dd /= g
    }
    return new Rational(nn, dd)
  }

  static fromInteger(n: bigint): Rational {
    return new Rational(n, 1n)
  }

  /** 解析十进制字面量为精确分数，例如 "0.1" -> 1/10，"1.5e-3" -> 3/2000 */
  static fromDecimalString(text: string): Rational {
    const m = /^([+-]?)(\d*)(?:\.(\d*))?(?:[eE]([+-]?\d+))?$/.exec(text.trim())
    if (!m) throw new Error(`不是合法的数字：${text}`)
    const sign = m[1] === '-' ? -1n : 1n
    const intPart = m[2] || ''
    const fracPart = m[3] || ''
    const expPart = m[4] ? parseInt(m[4], 10) : 0
    if (intPart === '' && fracPart === '') throw new Error(`不是合法的数字：${text}`)
    let digits = BigInt((intPart + fracPart) || '0')
    let exp = expPart - fracPart.length
    digits *= sign
    if (exp >= 0) return Rational.of(digits * 10n ** BigInt(exp))
    return Rational.of(digits, 10n ** BigInt(-exp))
  }

  get isZero(): boolean {
    return this.n === 0n
  }

  get isOne(): boolean {
    return this.n === 1n && this.d === 1n
  }

  get isMinusOne(): boolean {
    return this.n === -1n && this.d === 1n
  }

  get isInteger(): boolean {
    return this.d === 1n
  }

  get sign(): -1 | 0 | 1 {
    if (this.n === 0n) return 0
    return this.n < 0n ? -1 : 1
  }

  toBigInt(): bigint {
    if (this.d !== 1n) throw new Error('不是整数')
    return this.n
  }

  toNumber(): number {
    if (this.d === 1n) return Number(this.n)
    const a = Number(this.n)
    const b = Number(this.d)
    if (Number.isFinite(a) && Number.isFinite(b)) return a / b
    // 超大数时按数量级换算，避免直接溢出
    const sign = this.sign
    const an = this.n < 0n ? -this.n : this.n
    const sa = an.toString()
    const sb = this.d.toString()
    const scale = Math.min(300, Math.max(0, sa.length - sb.length))
    const a2 = Number(sa.slice(0, 17)) / 10 ** Math.min(16, sa.length - 1)
    const b2 = Number(sb.slice(0, 17)) / 10 ** Math.min(16, sb.length - 1)
    return sign * (a2 / b2) * 10 ** (sa.length - sb.length)
  }

  add(o: Rational): Rational {
    return Rational.of(this.n * o.d + o.n * this.d, this.d * o.d)
  }

  sub(o: Rational): Rational {
    return Rational.of(this.n * o.d - o.n * this.d, this.d * o.d)
  }

  mul(o: Rational): Rational {
    return Rational.of(this.n * o.n, this.d * o.d)
  }

  div(o: Rational): Rational {
    if (o.n === 0n) throw new Error('除数不能为零')
    return Rational.of(this.n * o.d, this.d * o.n)
  }

  neg(): Rational {
    return Rational.of(-this.n, this.d)
  }

  abs(): Rational {
    return this.n < 0n ? this.neg() : this
  }

  inv(): Rational {
    if (this.n === 0n) throw new Error('零没有倒数')
    return Rational.of(this.d, this.n)
  }

  powInt(e: number | bigint): Rational {
    let exp = typeof e === 'bigint' ? e : BigInt(e)
    if (exp === 0n) return Rational.ONE
    const limit = 100000
    if (exp > BigInt(limit) || exp < BigInt(-limit)) {
      throw new Error(`指数过大（上限 ${limit}）`)
    }
    if (this.isZero) {
      if (exp < 0n) throw new Error('零不能做负数次幂')
      return Rational.ZERO
    }
    const neg = exp < 0n
    if (neg) exp = -exp
    let base: Rational = this
    let result = Rational.ONE
    while (exp > 0n) {
      if (exp & 1n) result = result.mul(base)
      base = base.mul(base)
      exp >>= 1n
    }
    return neg ? result.inv() : result
  }

  compare(o: Rational): number {
    const left = this.n * o.d
    const right = o.n * this.d
    return left < right ? -1 : left > right ? 1 : 0
  }

  equals(o: Rational): boolean {
    return this.n === o.n && this.d === o.d
  }

  /** 用于显示：能有限小数表示时给出小数，否则给出分数 */
  toDisplayString(): string {
    if (this.d === 1n) return this.n.toString()
    const den = this.d
    let temp = den
    let twos = 0
    let fives = 0
    while (temp % 2n === 0n) {
      temp /= 2n
      twos++
    }
    while (temp % 5n === 0n) {
      temp /= 5n
      fives++
    }
    if (temp !== 1n) return `${this.n}/${this.d}`
    const digits = Math.max(twos, fives)
    if (digits > 20) return `${this.n}/${this.d}`
    const scaled = (this.n < 0n ? -this.n : this.n) * 10n ** BigInt(digits) / this.d
    let s = scaled.toString().padStart(digits + 1, '0')
    const intPart = s.slice(0, s.length - digits)
    let fracPart = s.slice(s.length - digits).replace(/0+$/, '')
    s = fracPart.length > 0 ? `${intPart}.${fracPart}` : intPart
    return this.n < 0n ? `-${s}` : s
  }

  toString(): string {
    return this.d === 1n ? this.n.toString() : `${this.n}/${this.d}`
  }
}

export { gcd as bigintGcd }

export function bigintPow(base: bigint, exp: bigint): bigint {
  if (exp < 0n) throw new Error('大整数指数不能为负')
  let result = 1n
  let b = base
  let e = exp
  while (e > 0n) {
    if (e & 1n) result *= b
    b *= b
    e >>= 1n
  }
  return result
}
