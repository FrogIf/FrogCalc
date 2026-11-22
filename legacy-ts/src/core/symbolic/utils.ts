import {
  add,
  call,
  eq,
  Expr,
  list,
  mul,
  pow,
  str,
  sub,
  sym
} from './expr'

export function occurs(expr: Expr, name: string): boolean {
  switch (expr.kind) {
    case 'sym':
      return expr.name === name
    case 'add':
      return expr.terms.some((t) => occurs(t, name))
    case 'mul':
      return expr.factors.some((f) => occurs(f, name))
    case 'pow':
      return occurs(expr.base, name) || occurs(expr.exp, name)
    case 'call':
      return expr.args.some((a) => occurs(a, name))
    case 'eq':
      return occurs(expr.lhs, name) || occurs(expr.rhs, name)
    case 'list':
      return expr.items.some((i) => occurs(i, name))
    default:
      return false
  }
}

export function collectSymbols(expr: Expr, out = new Set<string>()): Set<string> {
  switch (expr.kind) {
    case 'sym':
      out.add(expr.name)
      break
    case 'add':
      for (const t of expr.terms) collectSymbols(t, out)
      break
    case 'mul':
      for (const f of expr.factors) collectSymbols(f, out)
      break
    case 'pow':
      collectSymbols(expr.base, out)
      collectSymbols(expr.exp, out)
      break
    case 'call':
      for (const a of expr.args) collectSymbols(a, out)
      break
    case 'eq':
      collectSymbols(expr.lhs, out)
      collectSymbols(expr.rhs, out)
      break
    case 'list':
      for (const i of expr.items) collectSymbols(i, out)
      break
  }
  return out
}

export function substitute(expr: Expr, name: string, replacement: Expr): Expr {
  switch (expr.kind) {
    case 'sym':
      return expr.name === name ? replacement : expr
    case 'add':
      return add(...expr.terms.map((t) => substitute(t, name, replacement)))
    case 'mul':
      return mul(...expr.factors.map((f) => substitute(f, name, replacement)))
    case 'pow':
      return pow(
        substitute(expr.base, name, replacement),
        substitute(expr.exp, name, replacement)
      )
    case 'call':
      return call(expr.name, ...expr.args.map((a) => substitute(a, name, replacement)))
    case 'eq':
      return eq(
        substitute(expr.lhs, name, replacement),
        substitute(expr.rhs, name, replacement)
      )
    case 'list':
      return list(expr.items.map((i) => substitute(i, name, replacement)))
    default:
      return expr
  }
}

export function substituteMany(
  expr: Expr,
  bindings: ReadonlyMap<string, Expr>
): Expr {
  switch (expr.kind) {
    case 'sym': {
      const value = bindings.get(expr.name)
      return value ?? expr
    }
    case 'add':
      return add(...expr.terms.map((t) => substituteMany(t, bindings)))
    case 'mul':
      return mul(...expr.factors.map((f) => substituteMany(f, bindings)))
    case 'pow':
      return pow(
        substituteMany(expr.base, bindings),
        substituteMany(expr.exp, bindings)
      )
    case 'call':
      return call(expr.name, ...expr.args.map((a) => substituteMany(a, bindings)))
    case 'eq':
      return eq(
        substituteMany(expr.lhs, bindings),
        substituteMany(expr.rhs, bindings)
      )
    case 'list':
      return list(expr.items.map((i) => substituteMany(i, bindings)))
    default:
      return expr
  }
}

export function nodeCount(expr: Expr): number {
  switch (expr.kind) {
    case 'add':
      return 1 + expr.terms.reduce((n, t) => n + nodeCount(t), 0)
    case 'mul':
      return 1 + expr.factors.reduce((n, f) => n + nodeCount(f), 0)
    case 'pow':
      return 1 + nodeCount(expr.base) + nodeCount(expr.exp)
    case 'call':
      return 1 + expr.args.reduce((n, a) => n + nodeCount(a), 0)
    case 'eq':
      return 1 + nodeCount(expr.lhs) + nodeCount(expr.rhs)
    case 'list':
      return 1 + expr.items.reduce((n, i) => n + nodeCount(i), 0)
    default:
      return 1
  }
}

export function hasFloat(expr: Expr): boolean {
  switch (expr.kind) {
    case 'float':
      return true
    case 'add':
      return expr.terms.some(hasFloat)
    case 'mul':
      return expr.factors.some(hasFloat)
    case 'pow':
      return hasFloat(expr.base) || hasFloat(expr.exp)
    case 'call':
      return expr.args.some(hasFloat)
    case 'eq':
      return hasFloat(expr.lhs) || hasFloat(expr.rhs)
    case 'list':
      return expr.items.some(hasFloat)
    default:
      return false
  }
}

export function isConstant(expr: Expr): boolean {
  switch (expr.kind) {
    case 'num':
    case 'float':
    case 'str':
      return true
    case 'sym':
      return expr.name === 'pi' || expr.name === 'e' || expr.name === 'i'
    case 'add':
      return expr.terms.every(isConstant)
    case 'mul':
      return expr.factors.every(isConstant)
    case 'pow':
      return isConstant(expr.base) && isConstant(expr.exp)
    case 'call':
      return expr.args.every(isConstant)
    default:
      return false
  }
}

export function makeSymbol(name: string): Expr {
  return sym(name)
}
