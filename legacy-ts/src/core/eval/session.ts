import { CalcError } from '../errors'
import { diff } from '../calculus/diff'
import { integrate, integrateDefinite } from '../calculus/integrate'
import { solveEquation } from '../solve/solve'
import {
  add,
  call,
  div,
  eq,
  Expr,
  list,
  mul,
  neg,
  num,
  pow,
  str,
  sub,
  sym,
  ZERO
} from '../symbolic/expr'
import { factor, expand, simplify } from '../symbolic/transform'
import {
  collectSymbols,
  hasFloat,
  isConstant,
  nodeCount,
  substitute
} from '../symbolic/utils'
import { Rational } from '../numeric/rational'
import { approxPreview, toApproxExpr } from './approx'
import { toLatex, toPlainText } from '../format/plain'
import type { Node, Program, Statement } from '../parser/ast'
import { parse } from '../parser/parser'
import {
  divideExactComplex,
  fromExactComplex,
  multiplyExactComplex,
  powExactComplex,
  toExactComplex
} from '../symbolic/complex'

export type AngleMode = 'rad' | 'deg'
export type DisplayMode = 'exact' | 'both' | 'approx'

export interface CalcSettings {
  precision: number
  angleMode: AngleMode
  displayMode: DisplayMode
}

export const DEFAULT_SETTINGS: CalcSettings = {
  precision: 50,
  angleMode: 'rad',
  displayMode: 'both'
}

export interface UserFunction {
  name: string
  params: string[]
  body: Node
}

export interface ExecResult {
  ok: boolean
  kind: 'expr' | 'assign' | 'funcdef' | 'error'
  name?: string
  value?: Expr
  plain?: string
  latex?: string
  approx?: string
  approxLatex?: string
  valueType?: string
  note?: string
  error?: string
  errorPos?: number
  elapsedMs: number
  suppressed?: boolean
}

export interface WorkspaceItem {
  name: string
  kind: 'variable' | 'function'
  signature?: string
  valueType?: string
  plain?: string
  latex?: string
}

const RESERVED = new Set([
  'pi',
  'e',
  'i',
  'ans',
  'inf',
  'sin',
  'cos',
  'tan',
  'asin',
  'acos',
  'atan',
  'sinh',
  'cosh',
  'tanh',
  'exp',
  'ln',
  'log',
  'log10',
  'log2',
  'sqrt',
  'cbrt',
  'root',
  'pow',
  'abs',
  'sign',
  'floor',
  'ceil',
  'round',
  'factorial',
  'binomial',
  'gcd',
  'lcm',
  'mod',
  'min',
  'max',
  'eq',
  'diff',
  'integrate',
  'solve',
  'simplify',
  'expand',
  'factor',
  'N',
  'approx',
  'help',
  'clear',
  'vars',
  'subs',
  'substitute',
  'setprecision',
  'setangle',
  'true',
  'false'
])

export class Session {
  settings: CalcSettings = { ...DEFAULT_SETTINGS }
  private variables = new Map<string, Expr>()
  private functions = new Map<string, UserFunction>()
  private ans: Expr = ZERO
  private notes: string[] = []
  private steps = 0
  private callDepth = 0

  getSettings(): CalcSettings {
    return { ...this.settings }
  }

  setSettings(settings: Partial<CalcSettings>): void {
    const requested = settings.precision ?? this.settings.precision
    const precision = Number.isFinite(requested)
      ? Math.min(500, Math.max(10, Math.round(requested)))
      : this.settings.precision
    this.settings = {
      ...this.settings,
      ...settings,
      precision
    }
  }

  execute(source: string): ExecResult[] {
    let program: Program
    try {
      program = parse(source)
    } catch (error) {
      return [this.errorResult(error, 0)]
    }

    const results: ExecResult[] = []
    for (const statement of program.statements) {
      const started = now()
      this.notes = []
      this.steps = 0
      try {
        const outcome = this.evalStatement(statement)
        const elapsedMs = now() - started
        if (outcome.suppress) {
          results.push({
            ok: true,
            kind: outcome.kind,
            name: outcome.name,
            value: outcome.value,
            elapsedMs,
            suppressed: true
          })
          continue
        }
        const formatted = this.formatValue(outcome.value, outcome.kind)
        results.push({
          ok: true,
          kind: outcome.kind,
          name: outcome.name,
          value: outcome.value,
          ...formatted,
          elapsedMs,
          suppressed: false
        })
      } catch (error) {
        results.push(this.errorResult(error, now() - started))
        break
      }
    }
    return results
  }

  private evalStatement(statement: Statement): {
    kind: 'expr' | 'assign' | 'funcdef'
    name?: string
    value: Expr
    suppress: boolean
  } {
    if (statement.type === 'expr') {
      const value = this.evalNode(statement.expr)
      this.checkSize(value)
      this.ans = value
      return { kind: 'expr', value, suppress: statement.suppress }
    }

    if (statement.type === 'assign') {
      if (RESERVED.has(statement.name)) {
        throw new CalcError(`不能给保留名称 “${statement.name}” 赋值`, statement.pos.offset)
      }
      const value = this.evalNode(statement.value)
      this.variables.set(statement.name, value)
      this.functions.delete(statement.name)
      this.ans = value
      this.checkSize(value)
      return { kind: 'assign', name: statement.name, value, suppress: statement.suppress }
    }

    if (RESERVED.has(statement.name)) {
      throw new CalcError(`不能重新定义保留名称 “${statement.name}”`, statement.pos.offset)
    }
    if (new Set(statement.params).size !== statement.params.length) {
      throw new CalcError('函数参数名不能重复', statement.pos.offset)
    }
    const fn: UserFunction = {
      name: statement.name,
      params: [...statement.params],
      body: statement.body
    }
    this.functions.set(statement.name, fn)
    this.variables.delete(statement.name)
    const display = eq(
      call(statement.name, ...statement.params.map((p) => sym(p))),
      this.nodeToExpr(statement.body)
    )
    return {
      kind: 'funcdef',
      name: statement.name,
      value: display,
      suppress: statement.suppress
    }
  }

  private nodeToExpr(node: Node): Expr {
    switch (node.type) {
      case 'number':
        return num(node.value)
      case 'string':
        return str(node.value)
      case 'ident':
        return sym(node.name)
      case 'unary':
        return node.op === '-' ? neg(this.nodeToExpr(node.arg)) : this.nodeToExpr(node.arg)
      case 'binary': {
        const left = this.nodeToExpr(node.left)
        const right = this.nodeToExpr(node.right)
        switch (node.op) {
          case '+':
            return add(left, right)
          case '-':
            return sub(left, right)
          case '*':
            return mul(left, right)
          case '/':
            return div(left, right)
          case '^':
            return pow(left, right)
        }
        break
      }
      case 'call':
        return call(node.name, ...node.args.map((a) => this.nodeToExpr(a)))
      case 'eq':
        return eq(this.nodeToExpr(node.left), this.nodeToExpr(node.right))
      case 'list':
        return list(node.items.map((item) => this.nodeToExpr(item)))
    }
    return ZERO
  }

  private evalNode(node: Node, locals?: Map<string, Expr>, depth = 0): Expr {
    this.steps++
    if (this.steps > 500000) {
      throw new CalcError('计算步骤过多，已中止（可能存在过大的展开或递归）', node.pos.offset)
    }
    if (depth > 400) {
      throw new CalcError('表达式嵌套过深', node.pos.offset)
    }

    switch (node.type) {
      case 'number':
        return num(node.value)
      case 'string':
        return str(node.value)
      case 'ident': {
        if (locals && locals.has(node.name)) return locals.get(node.name)!
        if (node.name === 'ans') return this.ans
        if (this.variables.has(node.name)) return this.variables.get(node.name)!
        if (node.name === 'pi' || node.name === 'e' || node.name === 'i') {
          return sym(node.name)
        }
        return sym(node.name)
      }
      case 'unary':
        if (node.op === '+') return this.evalNode(node.arg, locals, depth + 1)
        return neg(this.evalNode(node.arg, locals, depth + 1))
      case 'binary': {
        const left = this.evalNode(node.left, locals, depth + 1)
        const right = this.evalNode(node.right, locals, depth + 1)
        try {
          switch (node.op) {
            case '+':
              return add(left, right)
            case '-':
              return sub(left, right)
            case '*': {
              const a = toExactComplex(left)
              const b = toExactComplex(right)
              if (a && b) return fromExactComplex(multiplyExactComplex(a, b))
              return mul(left, right)
            }
            case '/': {
              const a = toExactComplex(left)
              const b = toExactComplex(right)
              if (a && b) {
                const result = divideExactComplex(a, b)
                if (result) return fromExactComplex(result)
              }
              return div(left, right)
            }
            case '^': {
              const a = toExactComplex(left)
              if (a && right.kind === 'num' && right.value.isInteger) {
                const n = Number(right.value.toBigInt())
                const result = powExactComplex(a, n)
                if (result) return fromExactComplex(result)
              }
              return pow(left, right)
            }
          }
        } catch (error) {
          throw this.wrapError(error, node.pos.offset)
        }
        break
      }
      case 'eq':
        return eq(
          this.evalNode(node.left, locals, depth + 1),
          this.evalNode(node.right, locals, depth + 1)
        )
      case 'list':
        return list(node.items.map((item) => this.evalNode(item, locals, depth + 1)))
      case 'call':
        return this.evalCall(node, locals, depth)
    }
    throw new CalcError('无法计算的表达式')
  }

  private evalCall(node: Node & { type: 'call' }, locals: Map<string, Expr> | undefined, depth: number): Expr {
    const lower = node.name.toLowerCase()

    // 需要保持参数为“变量名”的特殊形式
    if (lower === 'diff') return this.evalDiff(node, locals, depth)
    if (lower === 'integrate') return this.evalIntegrate(node, locals, depth)
    if (lower === 'solve') return this.evalSolve(node, locals, depth)
    if (lower === 'simplify') {
      this.requireArgs(node, 1)
      return simplify(this.evalNode(node.args[0], locals, depth + 1))
    }
    if (lower === 'expand') {
      this.requireArgs(node, 1)
      return expand(this.evalNode(node.args[0], locals, depth + 1))
    }
    if (lower === 'factor') {
      this.requireArgs(node, 1)
      return factor(this.evalNode(node.args[0], locals, depth + 1))
    }
    if (lower === 'n' || lower === 'approx') {
      if (node.args.length < 1 || node.args.length > 2) {
        throw new CalcError('N 需要 1 到 2 个参数', node.pos.offset)
      }
      const value = this.evalNode(node.args[0], locals, depth + 1)
      let digits = this.settings.precision
      if (node.args.length === 2) {
        const d = this.evalNode(node.args[1], locals, depth + 1)
        if (d.kind !== 'num' || !d.value.isInteger) {
          throw new CalcError('N 的第二个参数必须是整数位数', node.pos.offset)
        }
        digits = Number(d.value.toBigInt())
        if (digits < 10 || digits > 500) {
          throw new CalcError('精度位数应在 10 到 500 之间', node.pos.offset)
        }
      }
      try {
        return toApproxExpr(value, digits)
      } catch (error) {
        throw this.wrapError(error, node.pos.offset)
      }
    }
    if (lower === 'help') {
      return this.evalHelp(node, locals, depth)
    }
    if (lower === 'clear') {
      return this.evalClear(node, locals, depth)
    }
    if (lower === 'vars') {
      return str(this.workspaceSummary())
    }
    if (lower === 'subs' || lower === 'substitute') {
      this.requireArgs(node, 3)
      const target = this.evalNode(node.args[0], locals, depth + 1)
      const varName = this.resolveVarName(node.args[1])
      const value = this.evalNode(node.args[2], locals, depth + 1)
      return substitute(target, varName, value)
    }
    if (lower === 'setprecision') {
      this.requireArgs(node, 1)
      const value = this.evalNode(node.args[0], locals, depth + 1)
      if (value.kind !== 'num' || !value.value.isInteger) {
        throw new CalcError('setprecision 需要一个整数', node.pos.offset)
      }
      this.setSettings({ precision: Number(value.value.toBigInt()) })
      return str(`精度已设置为 ${this.settings.precision} 位有效数字`)
    }
    if (lower === 'setangle') {
      this.requireArgs(node, 1)
      const value = this.evalNode(node.args[0], locals, depth + 1)
      const text = value.kind === 'str' ? value.value.toLowerCase() : ''
      if (text !== 'deg' && text !== 'rad') {
        throw new CalcError('setangle 的参数应为 "deg" 或 "rad"', node.pos.offset)
      }
      this.setSettings({ angleMode: text === 'deg' ? 'deg' : 'rad' })
      return str(text === 'deg' ? '角度制已启用' : '弧度制已启用')
    }

    // 用户函数
    const userFn = this.functions.get(node.name)
    if (userFn) {
      if (userFn.params.length !== node.args.length) {
        throw new CalcError(
          `函数 ${node.name} 需要 ${userFn.params.length} 个参数，实际收到 ${node.args.length} 个`,
          node.pos.offset
        )
      }
      if (this.callDepth >= 200) {
        throw new CalcError('函数调用层数过深（可能存在无限递归）', node.pos.offset)
      }
      const args = node.args.map((a) => this.evalNode(a, locals, depth + 1))
      const frame = new Map<string, Expr>()
      userFn.params.forEach((p, i) => frame.set(p, args[i]))
      this.callDepth++
      try {
        return this.evalNode(userFn.body, frame, depth + 1)
      } finally {
        this.callDepth--
      }
    }

    // 内置数学函数
    const args = node.args.map((a) => this.evalNode(a, locals, depth + 1))

    // 变量后面跟括号时视为隐式乘法，例如 x(y + 1)
    if (this.variables.has(node.name) && args.length === 1) {
      return mul(this.variables.get(node.name)!, args[0])
    }

    try {
      return this.applyBuiltinCall(lower, node.name, args, node.pos.offset)
    } catch (error) {
      throw this.wrapError(error, node.pos.offset)
    }
  }

  private applyBuiltinCall(lower: string, original: string, args: Expr[], pos: number): Expr {
    if (lower === 'sin' || lower === 'cos' || lower === 'tan') {
      const arg = this.settings.angleMode === 'deg' ? this.degToRad(args[0]) : args[0]
      return call(lower, arg)
    }
    if (lower === 'asin' || lower === 'acos' || lower === 'atan') {
      const result = call(lower, args[0])
      return this.settings.angleMode === 'deg' ? this.radToDeg(result) : result
    }
    return call(original, ...args)
  }

  private degToRad(expr: Expr): Expr {
    return mul(expr, num(Rational.of(1, 180)), sym('pi'))
  }

  private radToDeg(expr: Expr): Expr {
    return mul(expr, num(Rational.of(180)), pow(sym('pi'), num(Rational.NEG_ONE)))
  }

  private evalDiff(node: Node & { type: 'call' }, locals: Map<string, Expr> | undefined, depth: number): Expr {
    if (node.args.length < 2 || node.args.length > 3) {
      throw new CalcError('diff 需要 2 到 3 个参数：diff(表达式, 变量[, 阶数])', node.pos.offset)
    }
    const expr = this.evalNode(node.args[0], locals, depth + 1)
    const varName = this.resolveVarName(node.args[1])
    let order = 1
    if (node.args.length === 3) {
      const o = this.evalNode(node.args[2], locals, depth + 1)
      if (o.kind !== 'num' || !o.value.isInteger || o.value.sign < 0) {
        throw new CalcError('求导阶数必须是非负整数', node.pos.offset)
      }
      order = Number(o.value.toBigInt())
    }
    return diff(expr, varName, order)
  }

  private evalIntegrate(
    node: Node & { type: 'call' },
    locals: Map<string, Expr> | undefined,
    depth: number
  ): Expr {
    if (node.args.length < 2 || node.args.length > 4) {
      throw new CalcError(
        'integrate 需要 2 到 4 个参数：integrate(表达式, 变量[, 下限, 上限])',
        node.pos.offset
      )
    }
    const expr = this.evalNode(node.args[0], locals, depth + 1)
    const varName = this.resolveVarName(node.args[1])
    if (node.args.length === 2) {
      const result = integrate(expr, varName)
      if (containsCall(result, 'integrate')) {
        this.notes.push('未能找到初等原函数，已保留未求值形式')
      } else {
        this.notes.push('不定积分（省略常数 C）')
      }
      return result
    }
    if (node.args.length === 4) {
      const a = this.evalNode(node.args[2], locals, depth + 1)
      const b = this.evalNode(node.args[3], locals, depth + 1)
      const result = integrateDefinite(expr, varName, a, b)
      if (containsCall(result, 'integrate')) {
        this.notes.push('定积分未能求出，已保留未求值形式')
      } else {
        this.notes.push('定积分已代入上下限')
      }
      return result
    }
    throw new CalcError('integrate 的下限和上限必须同时给出', node.pos.offset)
  }

  private evalSolve(node: Node & { type: 'call' }, locals: Map<string, Expr> | undefined, depth: number): Expr {
    this.requireArgs(node, 2)
    const expr = this.evalNode(node.args[0], locals, depth + 1)
    const varName = this.resolveVarName(node.args[1])
    const result = solveEquation(expr, varName)
    if (result.unsupported) {
      this.notes.push('未能解析求解，已保留未求值形式')
      return call('solve', expr, sym(varName))
    }
    if (result.note) this.notes.push(result.note)
    if (result.solutions.length === 0) this.notes.push('没有找到解')
    result.solutions.sort((a, b) => {
      if (a.kind === 'num' && b.kind === 'num') return a.value.compare(b.value)
      if (a.kind === 'num') return -1
      if (b.kind === 'num') return 1
      return 0
    })
    return list(result.solutions)
  }

  private evalHelp(node: Node & { type: 'call' }, locals: Map<string, Expr> | undefined, depth: number): Expr {
    if (node.args.length !== 1) {
      throw new CalcError('help 需要 1 个参数，例如 help("sin")', node.pos.offset)
    }
    let name = ''
    if (node.args[0].type === 'string') name = node.args[0].value
    else if (node.args[0].type === 'ident') name = node.args[0].name
    else {
      const value = this.evalNode(node.args[0], locals, depth + 1)
      if (value.kind === 'sym') name = value.name
      else if (value.kind === 'str') name = value.value
    }
    return str(this.helpText(name))
  }

  private evalClear(node: Node & { type: 'call' }, locals: Map<string, Expr> | undefined, depth: number): Expr {
    if (node.args.length === 0) {
      this.variables.clear()
      this.functions.clear()
      this.ans = ZERO
      return str('工作区已清空')
    }
    const names: string[] = []
    for (const arg of node.args) {
      if (arg.type === 'ident') names.push(arg.name)
      else if (arg.type === 'string') names.push(arg.value)
      else {
        const value = this.evalNode(arg, locals, depth + 1)
        if (value.kind === 'sym') names.push(value.name)
        else if (value.kind === 'str') names.push(value.value)
        else throw new CalcError('clear 的参数应为变量名或字符串', node.pos.offset)
      }
    }
    for (const name of names) {
      this.variables.delete(name)
      this.functions.delete(name)
    }
    return str(`已清除：${names.join(', ')}`)
  }

  private resolveVarName(node: Node): string {
    if (node.type === 'ident') return node.name
    if (node.type === 'string') return node.value
    throw new CalcError('这里需要变量名，例如 x', node.pos.offset)
  }

  private requireArgs(node: Node & { type: 'call' }, count: number): void {
    if (node.args.length !== count) {
      throw new CalcError(
        `${node.name} 需要 ${count} 个参数，实际收到 ${node.args.length} 个`,
        node.pos.offset
      )
    }
  }

  private formatValue(value: Expr, kind: 'expr' | 'assign' | 'funcdef'): {
    plain: string
    latex: string
    approx?: string
    approxLatex?: string
    valueType: string
    note?: string
  } {
    const plain = value.kind === 'str' ? value.value : toPlainText(value)
    const latex = toLatex(value)
    let approx: string | undefined
    let approxLatex: string | undefined
    if (
      this.settings.displayMode !== 'exact' &&
      value.kind !== 'list' &&
      value.kind !== 'eq' &&
      value.kind !== 'str' &&
      !hasFloat(value)
    ) {
      const previewPrecision =
        this.settings.displayMode === 'approx'
          ? this.settings.precision
          : Math.min(20, this.settings.precision)
      try {
        const preview = approxPreview(value, previewPrecision)
        if (preview !== null && preview !== plain) {
          approx = preview
          const approxExpr = toApproxExpr(value, previewPrecision)
          approxLatex = toLatex(approxExpr)
        }
      } catch {
        // 含自由符号时无法近似，忽略
      }
    }
    const note = this.notes.length > 0 ? this.notes.join('；') : undefined
    return {
      plain,
      latex,
      approx,
      approxLatex,
      valueType: kind === 'funcdef' ? '函数定义' : typeLabel(value),
      note
    }
  }

  private checkSize(value: Expr): void {
    if (nodeCount(value) > 300000) {
      throw new CalcError('结果表达式过大，已中止')
    }
  }

  getWorkspace(): WorkspaceItem[] {
    const items: WorkspaceItem[] = []
    for (const [name, value] of this.variables) {
      items.push({
        name,
        kind: 'variable',
        valueType: typeLabel(value),
        plain: toPlainText(value),
        latex: toLatex(value)
      })
    }
    for (const [name, fn] of this.functions) {
      items.push({
        name,
        kind: 'function',
        signature: `${name}(${fn.params.join(', ')})`,
        valueType: '函数',
        plain: `${name}(${fn.params.join(', ')})`
      })
    }
    items.sort((a, b) => a.name.localeCompare(b.name))
    return items
  }

  /** 可重新执行的变量与函数定义，用于持久化工作区 */
  getWorkspaceSource(): string {
    const lines: string[] = []
    for (const [name, value] of this.variables) {
      if (value.kind === 'eq') {
        lines.push(
          `${name} = eq(${toPlainText(value.lhs)}, ${toPlainText(value.rhs)})`
        )
      } else {
        lines.push(`${name} = ${toPlainText(value)}`)
      }
    }
    for (const [name, fn] of this.functions) {
      lines.push(
        `${name}(${fn.params.join(', ')}) = ${toPlainText(this.nodeToExpr(fn.body))}`
      )
    }
    return lines.join('\n')
  }

  removeWorkspaceItem(name: string): void {
    this.variables.delete(name)
    this.functions.delete(name)
  }

  clearAll(): void {
    this.variables.clear()
    this.functions.clear()
    this.ans = ZERO
  }

  private workspaceSummary(): string {
    const lines: string[] = []
    for (const [name, value] of this.variables) {
      lines.push(`${name} = ${toPlainText(value)}`)
    }
    for (const [name, fn] of this.functions) {
      lines.push(`${name}(${fn.params.join(', ')}) = 用户函数`)
    }
    if (lines.length === 0) return '工作区为空'
    return lines.join('\n')
  }

  private helpText(name: string): string {
    const key = name.toLowerCase()
    const docs: Record<string, string> = {
      syntax:
        'FrogCalc 语言：\n' +
        '  a = 1 / 3           赋值，除以整数得到精确分数\n' +
        '  f(x) = x^2 + 1      定义函数\n' +
        '  2x + 3(x - 1)       支持隐式乘法\n' +
        '  sqrt(2) * sqrt(2)   精确化简为 2\n' +
        '  diff / integrate / solve / simplify / expand / factor\n' +
        '  N(表达式, 位数)      近似求值\n' +
        '  help("函数名")       查看帮助\n' +
        '  # 这是注释\n' +
        '  分号 ; 可分隔语句并抑制输出',
      diff: 'diff(表达式, 变量[, 阶数])：求导。例如 diff(x^3*sin(x), x)',
      integrate:
        'integrate(表达式, 变量[, 下限, 上限])：积分。\n' +
        '例如 integrate(x*exp(x), x) 或 integrate(1/(x^2-1), x)。\n' +
        '找不到初等原函数时保留未求值形式。',
      solve: 'solve(方程或表达式, 变量)：解方程。\n例如 solve(x^2 - 2x - 3, x) 或 solve(sin(x) == 0, x)。',
      simplify: 'simplify(表达式)：化简、通分、约去公因子。',
      expand: 'expand(表达式)：展开多项式与乘积。',
      factor: 'factor(表达式)：对一元多项式做因式分解。',
      sqrt: 'sqrt(x)：平方根。对完全平方数自动精确化简。',
      exp: 'exp(x)：自然指数函数。',
      ln: 'ln(x)：自然对数。',
      log: 'log(x) 为自然对数；log(x, b) 为以 b 为底的对数。',
      sin: 'sin(x)：正弦。默认弧度制，可用 setangle("deg") 切换。',
      cos: 'cos(x)：余弦。',
      tan: 'tan(x)：正切。',
      asin: 'asin(x)：反正弦。',
      acos: 'acos(x)：反余弦。',
      atan: 'atan(x)：反正切。',
      abs: 'abs(x)：绝对值。',
      factorial: 'factorial(n) 或 n!：阶乘。',
      binomial: 'binomial(n, k)：二项式系数。',
      gcd: 'gcd(a, b, ...)：最大公约数。',
      lcm: 'lcm(a, b, ...)：最小公倍数。',
      mod: 'mod(a, b)：取模。',
      N: 'N(表达式[, 位数])：按指定位数做近似求值。',
      help: 'help("名称")：显示帮助。',
      clear: 'clear() 清空工作区；clear("x") 删除变量 x。',
      vars: 'vars()：列出工作区变量与函数。',
      subs: 'subs(表达式, 变量, 值)：符号替换。',
      setprecision: 'setprecision(n)：设置近似计算的精度位数。',
      setangle: 'setangle("deg") 或 setangle("rad")：切换角度制。'
    }
    return docs[key] ?? `没有找到 “${name}” 的帮助。试试 help("syntax")。`
  }

  private errorResult(error: unknown, elapsedMs: number): ExecResult {
    if (error instanceof CalcError) {
      return {
        ok: false,
        kind: 'error',
        error: error.message,
        errorPos: error.pos,
        elapsedMs
      }
    }
    if (error instanceof Error) {
      return {
        ok: false,
        kind: 'error',
        error: error.message,
        elapsedMs
      }
    }
    return {
      ok: false,
      kind: 'error',
      error: String(error),
      elapsedMs
    }
  }

  private wrapError(error: unknown, pos: number): CalcError {
    if (error instanceof CalcError) {
      return error.pos === undefined ? new CalcError(error.message, pos) : error
    }
    if (error instanceof Error) return new CalcError(error.message, pos)
    return new CalcError(String(error), pos)
  }
}

function now(): number {
  return typeof performance !== 'undefined' ? performance.now() : Date.now()
}

function typeLabel(value: Expr): string {
  if (value.kind === 'str') return '文本'
  if (value.kind === 'list') return '解集'
  if (value.kind === 'eq') return '方程'
  if (value.kind === 'float') return '实数'
  if (value.kind === 'num') {
    return value.value.isInteger ? '整数' : '有理数'
  }
  const symbols = collectSymbols(value)
  if (symbols.has('i')) return '复数'
  if (isConstant(value)) return '精确常数'
  if (hasFloat(value)) return '实数'
  return '符号式'
}

function containsCall(expr: Expr, name: string): boolean {
  switch (expr.kind) {
    case 'call':
      return expr.name.toLowerCase() === name || expr.args.some((a) => containsCall(a, name))
    case 'add':
      return expr.terms.some((t) => containsCall(t, name))
    case 'mul':
      return expr.factors.some((f) => containsCall(f, name))
    case 'pow':
      return containsCall(expr.base, name) || containsCall(expr.exp, name)
    case 'eq':
      return containsCall(expr.lhs, name) || containsCall(expr.rhs, name)
    case 'list':
      return expr.items.some((i) => containsCall(i, name))
    default:
      return false
  }
}
