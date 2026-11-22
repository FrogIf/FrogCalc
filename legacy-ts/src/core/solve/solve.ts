/**
 * 方程求解。
 * 优先使用精确多项式求根；其次尝试“变量只出现一次”的逆运算求解；
 * 无法处理时返回 unsupported，由调用方保留未求值形式。
 */

import { Rational } from '../numeric/rational'
import {
  add,
  call,
  div,
  Expr,
  HALF,
  key,
  mul,
  neg,
  num,
  ONE,
  pow,
  sub,
  sym,
  ZERO
} from '../symbolic/expr'
import { hasFloat, occurs } from '../symbolic/utils'
import { polyDegree, polyFromExpr, solvePoly } from '../polynomial/poly'

export interface SolveResult {
  solutions: Expr[]
  note?: string
  unsupported?: boolean
}

export function solveEquation(expr: Expr, varName: string): SolveResult {
  const normalized = expr.kind === 'eq' ? sub(expr.lhs, expr.rhs) : expr
  const x = sym(varName)

  if (!occurs(normalized, varName)) {
    if (normalized.kind === 'num' && normalized.value.isZero) {
      return { solutions: [x], note: '等式恒成立，任意 x 都是解' }
    }
    return { solutions: [], note: '等式与 x 无关，无解' }
  }

  const poly = polyFromExpr(normalized, varName)
  if (poly && polyDegree(poly) > 0) {
    const solutions = solvePoly(poly)
    if (solutions.length > 0) {
      const numeric = solutions.some((s) => hasFloat(s))
      return {
        solutions,
        note: numeric
          ? '高次多项式没有全部精确根，部分结果为数值近似根'
          : undefined
      }
    }
    return { solutions: [], note: '多项式没有找到根' }
  }

  const candidates = invert(normalized, varName, ZERO, 0)
  if (candidates && candidates.length > 0) {
    const solutions: Expr[] = []
    const seen = new Set<string>()
    for (const c of candidates) {
      const k = key(c)
      if (seen.has(k)) continue
      seen.add(k)
      solutions.push(c)
    }
    return {
      solutions,
      note: '使用逆运算求得主值解；周期方程（如三角方程）可能还有更多分支'
    }
  }

  return { solutions: [], unsupported: true }
}

/**
 * 把 expr = target 化成 x = ...。
 * 返回 null 表示无法隔离变量。
 */
function invert(expr: Expr, v: string, target: Expr, depth: number): Expr[] | null {
  if (depth > 40) return null
  if (expr.kind === 'sym' && expr.name === v) return [target]
  if (!occurs(expr, v)) return null

  switch (expr.kind) {
    case 'add': {
      const withVar = expr.terms.filter((t) => occurs(t, v))
      if (withVar.length !== 1) return null
      const others = expr.terms.filter((t) => !occurs(t, v))
      return invert(withVar[0], v, sub(target, add(...others)), depth + 1)
    }
    case 'mul': {
      const withVar = expr.factors.filter((f) => occurs(f, v))
      if (withVar.length !== 1) return null
      const others = expr.factors.filter((f) => !occurs(f, v))
      return invert(withVar[0], v, div(target, mul(...others)), depth + 1)
    }
    case 'pow': {
      if (occurs(expr.exp, v)) return null
      if (expr.exp.kind !== 'num') return null
      const root = pow(target, pow(expr.exp, num(Rational.NEG_ONE)))
      if (expr.exp.value.isInteger && expr.exp.value.n % 2n === 0n) {
        const a = invert(expr.base, v, root, depth + 1)
        const b = invert(expr.base, v, neg(root), depth + 1)
        return merge(a, b)
      }
      return invert(expr.base, v, root, depth + 1)
    }
    case 'call': {
      const name = expr.name.toLowerCase()
      if (expr.args.length !== 1) return null
      const arg = expr.args[0]
      const inverseTargets = inverseFunction(name, target)
      if (!inverseTargets) return null
      const results: Expr[] = []
      for (const t of inverseTargets) {
        const r = invert(arg, v, t, depth + 1)
        if (r) results.push(...r)
      }
      return results.length > 0 ? results : null
    }
    default:
      return null
  }
}

function merge(a: Expr[] | null, b: Expr[] | null): Expr[] | null {
  if (!a && !b) return null
  return [...(a ?? []), ...(b ?? [])]
}

function inverseFunction(name: string, t: Expr): Expr[] | null {
  switch (name) {
    case 'sin':
      return [call('asin', t), sub(mul(sym('pi'), ONE), call('asin', t))]
    case 'cos':
      return [call('acos', t), neg(call('acos', t))]
    case 'tan':
      return [call('atan', t)]
    case 'asin':
      return [call('sin', t)]
    case 'acos':
      return [call('cos', t)]
    case 'atan':
      return [call('tan', t)]
    case 'exp':
      return [call('ln', t)]
    case 'ln':
      return [call('exp', t)]
    case 'sqrt':
      return [pow(t, num(Rational.TWO))]
    case 'abs':
      return [t, neg(t)]
    case 'sinh':
      return [call('ln', add(t, pow(add(pow(t, num(Rational.TWO)), ONE), HALF)))]
    case 'cosh': {
      const principal = call(
        'ln',
        add(t, pow(sub(pow(t, num(Rational.TWO)), ONE), HALF))
      )
      return [principal, neg(principal)]
    }
    case 'tanh':
      return [
        mul(
          HALF,
          call('ln', div(add(ONE, t), sub(ONE, t)))
        )
      ]
    default:
      return null
  }
}
