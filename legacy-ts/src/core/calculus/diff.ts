import {
  add,
  call,
  div,
  eq,
  Expr,
  list,
  mul,
  num,
  pow,
  sub,
  sym,
  ZERO,
  ONE
} from '../symbolic/expr'
import { Rational } from '../numeric/rational'
import { simplify } from '../symbolic/transform'

export function diff(expr: Expr, varName: string, order = 1): Expr {
  if (!Number.isInteger(order) || order < 0) {
    throw new Error('求导阶数必须是非负整数')
  }
  let result = expr
  for (let i = 0; i < order; i++) {
    result = simplify(diffRaw(result, varName))
  }
  return result
}

function diffRaw(expr: Expr, v: string): Expr {
  switch (expr.kind) {
    case 'num':
    case 'float':
    case 'str':
      return ZERO
    case 'sym':
      return expr.name === v ? ONE : ZERO
    case 'add':
      return add(...expr.terms.map((t) => diffRaw(t, v)))
    case 'mul': {
      const terms: Expr[] = []
      for (let i = 0; i < expr.factors.length; i++) {
        const d = diffRaw(expr.factors[i], v)
        if (d.kind === 'num' && d.value.isZero) continue
        const others = expr.factors.filter((_, j) => j !== i)
        terms.push(mul(d, ...others))
      }
      return add(...terms)
    }
    case 'pow': {
      const db = diffRaw(expr.base, v)
      const de = diffRaw(expr.exp, v)
      if (db.kind === 'num' && db.value.isZero && de.kind === 'num' && de.value.isZero) {
        return ZERO
      }
      if (de.kind === 'num' && de.value.isZero) {
        // (u^c)' = c u^(c-1) u'
        return mul(expr.exp, pow(expr.base, sub(expr.exp, ONE)), db)
      }
      // u^v = e^(v ln u)
      return mul(
        expr,
        add(mul(de, call('ln', expr.base)), div(mul(expr.exp, db), expr.base))
      )
    }
    case 'call':
      return diffCall(expr, v)
    case 'eq':
      return eq(diffRaw(expr.lhs, v), diffRaw(expr.rhs, v))
    case 'list':
      return list(expr.items.map((i) => diffRaw(i, v)))
    default:
      return ZERO
  }
}

function diffCall(expr: Expr & { kind: 'call' }, v: string): Expr {
  const name = expr.name.toLowerCase()
  const arg = expr.args[0]
  if (!arg) return call('diff', expr, sym(v))
  const du = diffRaw(arg, v)
  if (du.kind === 'num' && du.value.isZero) return ZERO

  switch (name) {
    case 'sin':
      return mul(call('cos', arg), du)
    case 'cos':
      return mul(num(Rational.NEG_ONE), call('sin', arg), du)
    case 'tan':
      return div(du, pow(call('cos', arg), num(Rational.TWO)))
    case 'asin':
      return div(du, pow(sub(ONE, pow(arg, num(Rational.TWO))), num(Rational.HALF)))
    case 'acos':
      return mul(
        num(Rational.NEG_ONE),
        div(du, pow(sub(ONE, pow(arg, num(Rational.TWO))), num(Rational.HALF)))
      )
    case 'atan':
      return div(du, add(ONE, pow(arg, num(Rational.TWO))))
    case 'sinh':
      return mul(call('cosh', arg), du)
    case 'cosh':
      return mul(call('sinh', arg), du)
    case 'tanh':
      return mul(sub(ONE, pow(call('tanh', arg), num(Rational.TWO))), du)
    case 'exp':
      return mul(call('exp', arg), du)
    case 'ln':
      return div(du, arg)
    case 'log10':
      return div(du, mul(arg, call('ln', num(Rational.of(10)))))
    case 'log2':
      return div(du, mul(arg, call('ln', num(Rational.TWO))))
    case 'abs':
      return mul(du, div(arg, call('abs', arg)))
    case 'sign':
    case 'floor':
    case 'ceil':
    case 'round':
      return ZERO
    default:
      return call('diff', expr, sym(v))
  }
}
