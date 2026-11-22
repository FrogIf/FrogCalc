/**
 * 规则式符号积分。
 *
 * 覆盖：基本积分表、线性换元、多项式、有理函数（部分分式）、
 * 分部积分、常见换元。无法处理时返回未求值的 integrate(...) 节点，
 * 而不是给出错误结果。
 */

import { CalcError } from '../errors'
import { BigFloat } from '../numeric/bigfloat'
import { Rational } from '../numeric/rational'
import {
  add,
  call,
  div,
  Expr,
  float,
  HALF,
  mul,
  num,
  ONE,
  pow,
  sub,
  sym,
  ZERO
} from '../symbolic/expr'
import { simplify, toFraction, expand } from '../symbolic/transform'
import { occurs, substitute } from '../symbolic/utils'
import {
  factorPoly,
  polyDegree,
  polyDivMod,
  polyFromExpr,
  polyIsZero,
  polyMonic,
  polyMul,
  polyScale,
  polyToExpr,
  type Poly
} from '../polynomial/poly'
import { diff } from './diff'

const MAX_DEPTH = 24

export function integrate(expr: Expr, varName: string): Expr {
  return simplify(integrateRaw(expr, varName, 0))
}

function xOf(v: string): Expr {
  return sym(v)
}

function unevaluated(expr: Expr, v: string): Expr {
  return call('integrate', expr, sym(v))
}

function isUnevaluated(expr: Expr, v: string): boolean {
  if (expr.kind !== 'call' || expr.name.toLowerCase() !== 'integrate') return false
  if (expr.args.length < 2) return false
  return expr.args[1].kind === 'sym' && expr.args[1].name === v
}

function safeIntegrate(expr: Expr, v: string, depth: number): Expr | null {
  try {
    const result = integrateRaw(expr, v, depth + 1)
    if (isUnevaluated(result, v)) return null
    return result
  } catch {
    return null
  }
}

function integrateRaw(expr: Expr, v: string, depth: number): Expr {
  if (depth > MAX_DEPTH) throw new CalcError('积分层数过深')
  const x = xOf(v)

  if (!occurs(expr, v)) {
    return mul(expr, x)
  }

  if (expr.kind === 'sym') {
    if (expr.name !== v) return mul(expr, x)
    return div(pow(x, num(Rational.TWO)), num(Rational.TWO))
  }

  if (expr.kind === 'add') {
    return add(...expr.terms.map((t) => integrateRaw(t, v, depth + 1)))
  }

  if (expr.kind === 'mul') {
    return integrateMul(expr, v, depth)
  }

  if (expr.kind === 'pow') {
    return integratePow(expr, v, depth)
  }

  if (expr.kind === 'call') {
    return integrateCall(expr, v, depth)
  }

  return unevaluated(expr, v)
}

function integrateMul(expr: Expr & { kind: 'mul' }, v: string, depth: number): Expr {
  const constantFactors = expr.factors.filter((f) => !occurs(f, v))
  const variableFactors = expr.factors.filter((f) => occurs(f, v))

  if (constantFactors.length > 0) {
    const constant = mul(...constantFactors)
    const rest = variableFactors.length === 1 ? variableFactors[0] : mul(...variableFactors)
    return mul(constant, integrateRaw(rest, v, depth + 1))
  }

  if (variableFactors.length === 1) {
    return integrateRaw(variableFactors[0], v, depth + 1)
  }

  const substituted = trySubstitution(expr, v, depth)
  if (substituted) return substituted

  const rational = tryRational(expr, v, depth)
  if (rational) return rational

  const byParts = tryByParts(expr, v, depth)
  if (byParts) return byParts

  return unevaluated(expr, v)
}

function integratePow(expr: Expr & { kind: 'pow' }, v: string, depth: number): Expr {
  const x = xOf(v)
  const exp = expr.exp
  const base = expr.base

  if (exp.kind === 'num') {
    const n = exp.value

    // ∫ x^n dx
    if (base.kind === 'sym' && base.name === v) {
      if (n.isMinusOne) return call('ln', call('abs', x))
      return div(pow(x, num(n.add(Rational.ONE))), num(n.add(Rational.ONE)))
    }

    // ∫ (a x + b)^n dx / ∫ 1/(a x + b) dx
    const linear = polyFromExpr(base, v)
    if (linear && polyDegree(linear) === 1) {
      const a = linear.coeffs[1]
      if (n.isMinusOne) {
        return div(call('ln', call('abs', base)), num(a))
      }
      const next = n.add(Rational.ONE)
      return div(pow(base, num(next)), num(a.mul(next)))
    }

    // ∫ (p(x))^n dx，n 为非负整数：展开后逐项积分
    if (n.isInteger && n.sign > 0) {
      const e = Number(n.toBigInt())
      if (e <= 12) {
        const poly = polyFromExpr(base, v)
        if (poly) {
          return integrateRaw(expand(pow(base, exp)), v, depth + 1)
        }
      }
    }

    // ∫ 1/(1+x^2) dx = atan(x)
    if (n.isMinusOne) {
      const p = polyFromExpr(base, v)
      if (p && polyDegree(p) === 2) {
        const c = p.coeffs[0]
        const b = p.coeffs[1]
        const a = p.coeffs[2]
        if (b.isZero && a.isOne && c.isOne) {
          return call('atan', x)
        }
        if (b.isZero && a.sign > 0 && c.sign > 0) {
          const k = c.div(a)
          const sqrtK = pow(num(k), HALF)
          return mul(
            div(ONE, mul(sqrtK, num(a))),
            call('atan', div(x, sqrtK))
          )
        }
      }
    }

    // ∫ 1/sqrt(1-x^2) dx = asin(x)
    if (n.n === -1n && n.d === 2n) {
      const p = polyFromExpr(base, v)
      if (p && polyDegree(p) === 2 && p.coeffs[1].isZero) {
        const c = p.coeffs[0]
        const a = p.coeffs[2]
        if (a.isMinusOne && c.isOne) return call('asin', x)
      }
    }
  }

  const rational = tryRational(expr, v, depth)
  if (rational) return rational

  // 线性内层函数的换元
  const linear = polyFromExpr(base, v)
  if (linear && polyDegree(linear) === 1) {
    const a = linear.coeffs[1]
    const substituted = tryOuterAntiderivative(expr, base)
    if (substituted) return div(substituted, num(a))
  }

  return unevaluated(expr, v)
}

function integrateCall(expr: Expr & { kind: 'call' }, v: string, depth: number): Expr {
  const name = expr.name.toLowerCase()
  const arg = expr.args[0]
  if (!arg) return unevaluated(expr, v)
  const linear = polyFromExpr(arg, v)
  const a = linear && polyDegree(linear) === 1 ? linear.coeffs[1] : null

  const outer = tryOuterAntiderivative(expr, arg)
  if (outer) {
    if (!a) {
      // 非线性参数，尝试把整体当作 u，若导数与剩余部分匹配由 substitution 处理
      return unevaluated(expr, v)
    }
    return div(outer, num(a))
  }

  if (name === 'abs' && linear && polyDegree(linear) === 1) {
    return div(mul(arg, call('abs', arg)), num(a!.mul(Rational.TWO)))
  }

  return unevaluated(expr, v)
}

/** ∫ f(u) du 中 f 的原函数（以 u 表示） */
function tryOuterAntiderivative(f: Expr, u: Expr): Expr | null {
  if (f.kind === 'call') {
    switch (f.name.toLowerCase()) {
      case 'exp':
        return call('exp', u)
      case 'sin':
        return mul(num(Rational.NEG_ONE), call('cos', u))
      case 'cos':
        return call('sin', u)
      case 'tan':
        return mul(num(Rational.NEG_ONE), call('ln', call('abs', call('cos', u))))
      case 'sinh':
        return call('cosh', u)
      case 'cosh':
        return call('sinh', u)
      case 'tanh':
        return call('ln', call('cosh', u))
      case 'ln':
        return sub(mul(u, call('ln', u)), u)
      case 'asin':
        return add(mul(u, call('asin', u)), pow(sub(ONE, pow(u, num(Rational.TWO))), HALF))
      case 'acos':
        return sub(mul(u, call('acos', u)), pow(sub(ONE, pow(u, num(Rational.TWO))), HALF))
      case 'atan':
        return sub(
          mul(u, call('atan', u)),
          mul(HALF, call('ln', add(ONE, pow(u, num(Rational.TWO)))))
        )
      default:
        return null
    }
  }

  if (f.kind === 'pow' && f.base === u && f.exp.kind === 'num') {
    const n = f.exp.value
    if (n.isMinusOne) return call('ln', call('abs', u))
    const next = n.add(Rational.ONE)
    return div(pow(u, num(next)), num(next))
  }
  return null
}

function trySubstitution(expr: Expr & { kind: 'mul' }, v: string, depth: number): Expr | null {
  const factors = expr.factors
  for (let i = 0; i < factors.length; i++) {
    const factor = factors[i]
    let u: Expr | null = null
    if (factor.kind === 'call' && factor.args.length === 1) u = factor.args[0]
    if (factor.kind === 'pow') u = factor.base
    if (!u || !occurs(u, v)) continue

    const outer = tryOuterAntiderivative(factor, u)
    if (!outer) continue

    const rest = mul(...factors.filter((_, j) => j !== i))
    const du = diff(u, v)
    const quotient = simplify(div(rest, du))
    if (!occurs(quotient, v)) {
      return mul(quotient, outer)
    }
  }
  return null
}

function tryByParts(expr: Expr & { kind: 'mul' }, v: string, depth: number): Expr | null {
  if (depth > MAX_DEPTH - 4) return null
  const factors = expr.factors

  // ln(x) 作为 u 通常更有效
  for (let i = 0; i < factors.length; i++) {
    const factor = factors[i]
    if (factor.kind !== 'call' || factor.name.toLowerCase() !== 'ln') continue
    const rest = mul(...factors.filter((_, j) => j !== i))
    const vInt = safeIntegrate(rest, v, depth)
    if (!vInt) continue
    const du = diff(factor, v)
    const inner = safeIntegrate(mul(vInt, du), v, depth)
    if (!inner) continue
    return sub(mul(factor, vInt), inner)
  }

  // 多项式因子作为 u
  for (let i = 0; i < factors.length; i++) {
    const p = polyFromExpr(factors[i], v)
    if (!p || polyDegree(p) <= 0) continue
    const rest = mul(...factors.filter((_, j) => j !== i))
    const vInt = safeIntegrate(rest, v, depth)
    if (!vInt) continue
    const du = diff(factors[i], v)
    const inner = safeIntegrate(mul(vInt, du), v, depth)
    if (!inner) continue
    return sub(mul(factors[i], vInt), inner)
  }

  return null
}

// ---------------------------------------------------------------------------
// 有理函数与部分分式
// ---------------------------------------------------------------------------

function tryRational(expr: Expr, v: string, depth: number): Expr | null {
  const parts = toFraction(expr)
  const pn = polyFromExpr(parts.n, v)
  const pd = polyFromExpr(parts.d, v)
  if (!pn || !pd) return null
  if (polyIsZero(pd) || polyDegree(pd) <= 0) return null

  let numerator = pn
  let denominator = pd
  let polynomialPart: Poly | null = null
  if (polyDegree(numerator) >= polyDegree(denominator)) {
    const { q, r } = polyDivMod(numerator, denominator)
    polynomialPart = q
    numerator = r
    if (polyIsZero(numerator)) {
      return integratePoly(q, v)
    }
  }

  const proper = integrateProperFraction(numerator, denominator, v)
  if (!proper) return null
  if (polynomialPart) {
    return add(integratePoly(polynomialPart, v), proper)
  }
  return proper
}

function integratePoly(poly: Poly, v: string): Expr {
  const x = xOf(v)
  const terms: Expr[] = []
  for (let i = 0; i < poly.coeffs.length; i++) {
    const c = poly.coeffs[i]
    if (c.isZero) continue
    const power = i + 1
    terms.push(
      div(mul(num(c), pow(x, num(Rational.of(power)))), num(Rational.of(power)))
    )
  }
  return add(...terms)
}

function integrateProperFraction(numerator: Poly, denominator: Poly, v: string): Expr | null {
  const leading = denominator.coeffs[polyDegree(denominator)]
  const monic = polyMonic(denominator)
  const scaled = polyScale(numerator, leading.inv())

  const factored = factorPoly(monic)
  if (factored.factors.length === 0) return null
  if (factored.factors.some((f) => f.mult > 1)) return null

  const factors = factored.factors.map((f) => f.poly)
  const total = polyDegree(monic)
  const columns: Array<{ poly: Poly; factor: Poly; power: number }> = []

  for (const factor of factors) {
    const degree = polyDegree(factor)
    const quotient = polyDivMod(monic, factor).q
    for (let j = 0; j < degree; j++) {
      const monomial: Poly = {
        varName: monic.varName,
        coeffs: [...new Array<Rational>(j).fill(Rational.ZERO), Rational.ONE]
      }
      columns.push({ poly: polyMul(monomial, quotient), factor, power: j })
    }
  }

  if (columns.length !== total) return null

  const matrix: Rational[][] = []
  const rhs: Rational[] = []
  for (let k = 0; k <= total - 1; k++) {
    matrix.push(columns.map((c) => c.poly.coeffs[k] ?? Rational.ZERO))
    rhs.push(scaled.coeffs[k] ?? Rational.ZERO)
  }

  const solution = solveLinearSystem(matrix, rhs)
  if (!solution) return null

  const terms: Expr[] = []
  for (const factor of factors) {
    const degree = polyDegree(factor)
    const numeratorCoeffs: Rational[] = []
    for (let j = 0; j < degree; j++) {
      const index = columns.findIndex((c) => c.factor === factor && c.power === j)
      numeratorCoeffs.push(solution[index])
    }
    const term = integrateFactorFraction(factor, numeratorCoeffs, v)
    if (!term) return null
    terms.push(term)
  }
  return add(...terms)
}

function solveLinearSystem(A: Rational[][], b: Rational[]): Rational[] | null {
  const n = b.length
  const m = A.length ? A[0].length : 0
  if (n !== m) return null
  const augmented = A.map((row, i) => [...row, b[i]])

  for (let col = 0; col < n; col++) {
    let pivot = -1
    for (let row = col; row < n; row++) {
      if (!augmented[row][col].isZero) {
        pivot = row
        break
      }
    }
    if (pivot === -1) return null
    if (pivot !== col) {
      const tmp = augmented[col]
      augmented[col] = augmented[pivot]
      augmented[pivot] = tmp
    }
    const p = augmented[col][col]
    for (let j = col; j <= n; j++) augmented[col][j] = augmented[col][j].div(p)
    for (let row = 0; row < n; row++) {
      if (row === col) continue
      const factor = augmented[row][col]
      if (factor.isZero) continue
      for (let j = col; j <= n; j++) {
        augmented[row][j] = augmented[row][j].sub(factor.mul(augmented[col][j]))
      }
    }
  }
  return augmented.map((row) => row[n])
}

function integrateFactorFraction(
  factor: Poly,
  numerator: Rational[],
  v: string
): Expr | null {
  const degree = polyDegree(factor)
  const x = xOf(v)
  const factorExpr = polyToExpr(factor)

  if (degree === 1) {
    const a = factor.coeffs[1]
    const A = numerator[0] ?? Rational.ZERO
    if (A.isZero) return ZERO
    return mul(num(A.div(a)), call('ln', call('abs', factorExpr)))
  }

  if (degree === 2) {
    const a = factor.coeffs[2]
    const b = factor.coeffs[1]
    const c = factor.coeffs[0]
    const A = numerator[1] ?? Rational.ZERO
    const B = numerator[0] ?? Rational.ZERO
    const disc = a.mul(c).mul(Rational.of(4)).sub(b.mul(b))
    if (disc.sign <= 0) return null
    const sqrtDisc = pow(num(disc), HALF)
    const terms: Expr[] = []
    if (!A.isZero) {
      terms.push(
        mul(
          num(A.div(a.mul(Rational.TWO))),
          call('ln', call('abs', factorExpr))
        )
      )
    }
    const rem = B.sub(A.mul(b).div(a.mul(Rational.TWO)))
    if (!rem.isZero) {
      const atanArg = div(
        add(mul(num(a.mul(Rational.TWO)), x), num(b)),
        sqrtDisc
      )
      terms.push(
        mul(
          div(num(rem.mul(Rational.TWO)), sqrtDisc),
          call('atan', atanArg)
        )
      )
    }
    return add(...terms)
  }

  return null
}

// ---------------------------------------------------------------------------
// 定积分
// ---------------------------------------------------------------------------

export function integrateDefinite(
  expr: Expr,
  varName: string,
  a: Expr,
  b: Expr
): Expr {
  const antiderivative = integrate(expr, varName)
  if (!isUnevaluated(antiderivative, varName)) {
    const upper = substitute(antiderivative, varName, b)
    const lower = substitute(antiderivative, varName, a)
    return simplify(sub(upper, lower))
  }
  const numeric = numericDefinite(expr, varName, a, b)
  if (numeric !== null) return numeric
  return unevaluated(expr, varName)
}

function numericDefinite(
  expr: Expr,
  varName: string,
  a: Expr,
  b: Expr
): Expr | null {
  if (a.kind !== 'num' || b.kind !== 'num') return null
  const lo = a.value.toNumber()
  const hi = b.value.toNumber()
  if (!Number.isFinite(lo) || !Number.isFinite(hi)) return null

  const f = (x: number): number | null => {
    const value = evalNumericJS(expr, varName, x)
    return Number.isFinite(value) ? value : null
  }

  const simpson = (left: number, right: number, fl: number, fm: number, fr: number): number => {
    const mid = (left + right) / 2
    const lm = (left + mid) / 2
    const rm = (mid + right) / 2
    const flm = f(lm)
    const frm = f(rm)
    if (flm === null || frm === null) return NaN
    const whole = ((right - left) / 6) * (fl + 4 * fm + fr)
    const half = ((right - left) / 12) * (fl + 4 * flm + 2 * fm + 4 * frm + fr)
    if (Math.abs(half - whole) < 1e-12 * Math.max(1, Math.abs(half))) return half
    return (
      simpson(left, mid, fl, flm, fm) +
      simpson(mid, right, fm, frm, fr)
    )
  }

  const fm = f((lo + hi) / 2)
  const fa = f(lo)
  const fb = f(hi)
  if (fm === null || fa === null || fb === null) return null
  const value = simpson(lo, hi, fa, fm, fb)
  if (!Number.isFinite(value)) return null
  return float(BigFloat.fromNumber(value, 15))
}

function evalNumericJS(expr: Expr, v: string, x: number): number {
  switch (expr.kind) {
    case 'num':
      return expr.value.toNumber()
    case 'float':
      return expr.value.toNumber()
    case 'sym':
      if (expr.name === v) return x
      if (expr.name === 'pi') return Math.PI
      if (expr.name === 'e') return Math.E
      return NaN
    case 'add':
      return expr.terms.reduce((s, t) => s + evalNumericJS(t, v, x), 0)
    case 'mul':
      return expr.factors.reduce((p, f) => p * evalNumericJS(f, v, x), 1)
    case 'pow': {
      const base = evalNumericJS(expr.base, v, x)
      const exp = evalNumericJS(expr.exp, v, x)
      return Math.pow(base, exp)
    }
    case 'call': {
      const args = expr.args.map((a) => evalNumericJS(a, v, x))
      switch (expr.name.toLowerCase()) {
        case 'sin':
          return Math.sin(args[0])
        case 'cos':
          return Math.cos(args[0])
        case 'tan':
          return Math.tan(args[0])
        case 'asin':
          return Math.asin(args[0])
        case 'acos':
          return Math.acos(args[0])
        case 'atan':
          return Math.atan(args[0])
        case 'sinh':
          return Math.sinh(args[0])
        case 'cosh':
          return Math.cosh(args[0])
        case 'tanh':
          return Math.tanh(args[0])
        case 'exp':
          return Math.exp(args[0])
        case 'ln':
          return Math.log(args[0])
        case 'log10':
          return Math.log10(args[0])
        case 'log2':
          return Math.log2(args[0])
        case 'sqrt':
          return Math.sqrt(args[0])
        case 'abs':
          return Math.abs(args[0])
        default:
          return NaN
      }
    }
    default:
      return NaN
  }
}
