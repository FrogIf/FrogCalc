import { CalcError } from '../errors'
import { Rational } from '../numeric/rational'
import {
  add,
  div,
  Expr,
  mul,
  num,
  ONE,
  pow,
  ZERO
} from './expr'
import { collectSymbols } from './utils'
import {
  factorPoly,
  polyDegree,
  polyDivMod,
  polyFromExpr,
  polyGcd,
  polyIsZero,
  polyToExpr,
  type Poly
} from '../polynomial/poly'

const MAX_EXPAND_TERMS = 8000

/** 展开：把乘积分配到加法上，并展开非负整数幂 */
export function expand(expr: Expr, depth = 0): Expr {
  if (depth > 80) throw new CalcError('展开层数过深')
  switch (expr.kind) {
    case 'add': {
      const terms = expr.terms.map((t) => expand(t, depth + 1))
      return add(...terms)
    }
    case 'mul': {
      let terms: Expr[][] = [[]]
      for (const factor of expr.factors) {
        const expanded = expand(factor, depth + 1)
        const factorTerms = expanded.kind === 'add' ? expanded.terms : [expanded]
        const next: Expr[][] = []
        for (const prefix of terms) {
          for (const term of factorTerms) {
            next.push([...prefix, term])
            if (next.length > MAX_EXPAND_TERMS) {
              throw new CalcError('展开结果过大')
            }
          }
        }
        terms = next
      }
      return add(...terms.map((factors) => mul(...factors)))
    }
    case 'pow': {
      const base = expand(expr.base, depth + 1)
      if (
        expr.exp.kind === 'num' &&
        expr.exp.value.isInteger &&
        expr.exp.value.sign > 0
      ) {
        const n = Number(expr.exp.value.toBigInt())
        if (n <= 20 && base.kind === 'add') {
          let result: Expr = ONE
          for (let i = 0; i < n; i++) {
            result = expandMul(result, base, depth + 1)
          }
          return result
        }
      }
      return pow(base, expand(expr.exp, depth + 1))
    }
    case 'call':
      return {
        ...expr,
        args: expr.args.map((a) => expand(a, depth + 1))
      }
    case 'eq':
      return {
        ...expr,
        lhs: expand(expr.lhs, depth + 1),
        rhs: expand(expr.rhs, depth + 1)
      }
    default:
      return expr
  }
}

function expandMul(a: Expr, b: Expr, depth: number): Expr {
  const left = expand(a, depth + 1)
  const right = expand(b, depth + 1)
  const leftTerms = left.kind === 'add' ? left.terms : [left]
  const rightTerms = right.kind === 'add' ? right.terms : [right]
  if (leftTerms.length * rightTerms.length > MAX_EXPAND_TERMS) {
    throw new CalcError('展开结果过大')
  }
  const out: Expr[] = []
  for (const l of leftTerms) {
    for (const r of rightTerms) {
      out.push(mul(l, r))
    }
  }
  return add(...out)
}

export interface FractionParts {
  n: Expr
  d: Expr
}

export function toFraction(expr: Expr): FractionParts {
  switch (expr.kind) {
    case 'add': {
      let acc: FractionParts = { n: ZERO, d: ONE }
      for (const term of expr.terms) {
        const f = toFraction(term)
        acc = {
          n: add(mul(acc.n, f.d), mul(f.n, acc.d)),
          d: mul(acc.d, f.d)
        }
      }
      return acc
    }
    case 'mul': {
      let n: Expr = ONE
      let d: Expr = ONE
      for (const factor of expr.factors) {
        const f = toFraction(factor)
        n = mul(n, f.n)
        d = mul(d, f.d)
      }
      return { n, d }
    }
    case 'pow': {
      if (expr.exp.kind === 'num' && expr.exp.value.isInteger) {
        const e = Number(expr.exp.value.toBigInt())
        if (Math.abs(e) <= 100) {
          const base = toFraction(expr.base)
          if (e >= 0) {
            return { n: pow(base.n, num(Rational.of(e))), d: pow(base.d, num(Rational.of(e))) }
          }
          return {
            n: pow(base.d, num(Rational.of(-e))),
            d: pow(base.n, num(Rational.of(-e)))
          }
        }
      }
      return { n: expr, d: ONE }
    }
    default:
      return { n: expr, d: ONE }
  }
}

function cancelFraction(parts: FractionParts): FractionParts {
  let { n, d } = parts
  if (d.kind === 'num' && d.value.isOne) return { n, d }
  if (n.kind === 'num' && n.value.isZero) return { n: ZERO, d: ONE }

  const symbols = new Set([...collectSymbols(n), ...collectSymbols(d)])
  for (const name of symbols) {
    if (name === 'pi' || name === 'e' || name === 'i') continue
    const pn = polyFromExpr(n, name)
    const pd = polyFromExpr(d, name)
    if (!pn || !pd) continue
    if (polyIsZero(pn) || polyIsZero(pd)) continue
    const gcd = polyGcd(pn, pd)
    if (polyDegree(gcd) <= 0) continue
    const qn = polyDivMod(pn, gcd)
    const qd = polyDivMod(pd, gcd)
    if (!polyIsZero(qn.r) || !polyIsZero(qd.r)) continue
    n = polyToExpr(qn.q)
    d = polyToExpr(qd.q)
    if (d.kind === 'num' && d.value.isOne) break
  }
  return { n, d }
}

/** 化简：先合并分式、约去多项式公因子，再做结构化简 */
export function simplify(expr: Expr): Expr {
  switch (expr.kind) {
    case 'add': {
      const simplified = expr.terms.map(simplify)
      return simplifyFraction(add(...simplified))
    }
    case 'mul': {
      const simplified = expr.factors.map(simplify)
      return simplifyFraction(mul(...simplified))
    }
    case 'pow': {
      return simplifyFraction(pow(simplify(expr.base), simplify(expr.exp)))
    }
    case 'call': {
      return simplifyFraction({
        ...expr,
        args: expr.args.map(simplify)
      })
    }
    case 'eq':
      return { ...expr, lhs: simplify(expr.lhs), rhs: simplify(expr.rhs) }
    case 'list':
      return { ...expr, items: expr.items.map(simplify) }
    default:
      return expr
  }
}

function simplifyFraction(expr: Expr): Expr {
  const parts = cancelFraction(toFraction(expr))
  if (parts.d.kind === 'num' && parts.d.value.isOne) return parts.n
  return div(parts.n, parts.d)
}

/** 因式分解（优先在一元多项式上完成） */
export function factor(expr: Expr): Expr {
  if (expr.kind === 'num') return expr
  const symbols = [...collectSymbols(expr)]
  let best: { name: string; poly: Poly } | null = null
  for (const name of symbols) {
    if (name === 'pi' || name === 'e' || name === 'i') continue
    const poly = polyFromExpr(expr, name)
    if (poly && polyDegree(poly) > 0 && (!best || polyDegree(poly) > polyDegree(best.poly))) {
      best = { name, poly }
    }
  }
  if (!best) return expr
  const result = factorPoly(best.poly)
  if (result.factors.length === 0) return num(result.content)
  const parts: Expr[] = []
  if (!result.content.isOne) parts.push(num(result.content))
  for (const f of result.factors) {
    const p = polyToExpr(f.poly)
    parts.push(f.mult === 1 ? p : pow(p, num(Rational.of(f.mult))))
  }
  if (parts.length === 1 && result.content.isOne) {
    // 没有真正分解出东西
    if (result.factors.length === 1 && result.factors[0].mult === 1) {
      return expr
    }
  }
  return mul(...parts)
}
