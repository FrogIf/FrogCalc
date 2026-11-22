/**
 * FrogCalc 语言的 Pratt 解析器。
 *
 * 语言设计目标：直观、少特殊符号、接近数学书写。
 *   a = 1 / 3
 *   f(x) = x^2 + 1
 *   2x + 3(x - 1)
 *   sqrt(2) * sqrt(2)
 *   solve(x^2 - 2x - 3, x)
 *
 * 运算符优先级（从低到高）：
 *   ==  <  + -  <  * / 与隐式乘法  <  一元 + -  <  ^  <  函数调用/阶乘
 */

import { CalcError } from '../errors'
import { Rational } from '../numeric/rational'
import type {
  CallNode,
  EqNode,
  FuncDefStatement,
  Node,
  Program,
  Statement
} from './ast'
import { tokenize, type SourcePosition, type Token } from './lexer'

const BP_EQ = 5
const BP_ADD = 10
const BP_MUL = 20
const BP_UNARY = 30
const BP_POW = 40
const BP_POSTFIX = 50

export function parse(source: string): Program {
  return new Parser(tokenize(source)).parseProgram()
}

class Parser {
  private index = 0
  private parenDepth = 0

  constructor(private readonly tokens: Token[]) {}

  parseProgram(): Program {
    const statements: Statement[] = []
    this.skipNewlines()
    while (!this.atEnd()) {
      const statement = this.parseStatement()
      let suppress = false
      if (this.atOp(';')) {
        while (this.atOp(';')) this.index++
        suppress = true
      }
      statements.push({ ...statement, suppress } as Statement)
      this.skipNewlines()
    }
    return { statements }
  }

  private parseStatement(): Statement {
    this.skipNewlines()
    const first = this.current()

    if (first.type === 'ident') {
      const next = this.peek(1)
      if (next.type === 'op' && next.value === '=') {
        this.index++
        const name = first.value
        this.index++ // '='
        const value = this.parseExpression(0)
        return {
          type: 'assign',
          name,
          value,
          suppress: false,
          pos: first.pos
        }
      }

      if (next.type === 'op' && next.value === '(') {
        const saved = this.index
        const savedDepth = this.parenDepth
        const name = first.value
        this.index++
        this.index++ // '('
        this.parenDepth++
        const params: string[] = []
        let valid = true
        this.skipSoftNewlines()
        if (!this.atOp(')')) {
          while (true) {
            this.skipSoftNewlines()
            const token = this.current()
            if (token.type !== 'ident') {
              valid = false
              break
            }
            params.push(token.value)
            this.index++
            this.skipSoftNewlines()
            if (this.atOp(',')) {
              this.index++
              continue
            }
            break
          }
        }
        this.skipSoftNewlines()
        if (valid && this.atOp(')')) {
          this.index++
          this.parenDepth = savedDepth
          this.skipSoftNewlines()
          if (this.atOp('=')) {
            this.index++
            const body = this.parseExpression(0)
            if (new Set(params).size !== params.length) {
              throw new CalcError('函数参数名不能重复', first.pos.offset)
            }
            return {
              type: 'funcdef',
              name,
              params,
              body,
              suppress: false,
              pos: first.pos
            }
          }
        }
        this.parenDepth = savedDepth
        this.index = saved
      }
    }

    const expr = this.parseExpression(0)
    return {
      type: 'expr',
      expr,
      suppress: false,
      pos: first.pos
    }
  }

  private parseExpression(minBp: number): Node {
    this.skipSoftNewlines()
    let left = this.parsePrefix()

    while (true) {
      this.skipSoftNewlines()
      const token = this.current()

      // 后缀阶乘：n!
      if (token.type === 'op' && token.value === '!' && BP_POSTFIX > minBp) {
        this.index++
        left = {
          type: 'call',
          name: 'factorial',
          args: [left],
          pos: left.pos
        }
        continue
      }

      if (
        token.type === 'number' ||
        token.type === 'ident' ||
        (token.type === 'op' && token.value === '(')
      ) {
        if (BP_MUL > minBp) {
          const right = this.parseExpression(BP_MUL + 1)
          left = this.binary('*', left, right, token.pos)
          continue
        }
        break
      }

      if (token.type !== 'op') break

      let lbp = 0
      let rbp = 0
      let op: '+' | '-' | '*' | '/' | '^' | '=' | null = null
      switch (token.value) {
        case '==':
          lbp = BP_EQ
          rbp = BP_EQ + 1
          break
        case '+':
        case '-':
          lbp = BP_ADD
          rbp = BP_ADD + 1
          op = token.value
          break
        case '*':
        case '/':
          lbp = BP_MUL
          rbp = BP_MUL + 1
          op = token.value
          break
        case '^':
        case '**':
          lbp = BP_POW
          rbp = BP_POW - 1
          op = '^'
          break
        default:
          lbp = 0
      }

      if (lbp <= minBp) break

      if (token.value === '==') {
        this.index++
        const right = this.parseExpression(rbp)
        left = {
          type: 'eq',
          left,
          right,
          pos: token.pos
        } satisfies EqNode
        continue
      }

      if (op === null) break
      this.index++
      const right = this.parseExpression(rbp)
      left = this.binary(op, left, right, token.pos)
    }

    return left
  }

  private parsePrefix(): Node {
    this.skipSoftNewlines()
    const token = this.current()

    if (token.type === 'number') {
      this.index++
      try {
        return {
          type: 'number',
          value: Rational.fromDecimalString(token.value),
          raw: token.value,
          pos: token.pos
        }
      } catch (error) {
        throw new CalcError(
          error instanceof Error ? error.message : '数字格式错误',
          token.pos.offset
        )
      }
    }

    if (token.type === 'string') {
      this.index++
      return {
        type: 'string',
        value: token.value,
        pos: token.pos
      }
    }

    if (token.type === 'ident') {
      this.index++
      if (this.atOp('(')) {
        this.index++
        this.parenDepth++
        const args: Node[] = []
        this.skipSoftNewlines()
        if (!this.atOp(')')) {
          while (true) {
            args.push(this.parseExpression(0))
            this.skipSoftNewlines()
            if (this.atOp(',')) {
              this.index++
              continue
            }
            break
          }
        }
        this.skipSoftNewlines()
        this.expectOp(')')
        this.parenDepth--
        return {
          type: 'call',
          name: token.value,
          args,
          pos: token.pos
        } satisfies CallNode
      }
      return {
        type: 'ident',
        name: token.value,
        pos: token.pos
      }
    }

    if (token.type === 'op' && (token.value === '+' || token.value === '-')) {
      this.index++
      const arg = this.parseExpression(BP_UNARY)
      return {
        type: 'unary',
        op: token.value,
        arg,
        pos: token.pos
      }
    }

    if (token.type === 'op' && token.value === '(') {
      this.index++
      this.parenDepth++
      const expr = this.parseExpression(0)
      this.skipSoftNewlines()
      this.expectOp(')')
      this.parenDepth--
      return expr
    }

    if (token.type === 'op' && token.value === '{') {
      this.index++
      this.parenDepth++
      const items: Node[] = []
      this.skipSoftNewlines()
      if (!this.atOp('}')) {
        while (true) {
          items.push(this.parseExpression(0))
          this.skipSoftNewlines()
          if (this.atOp(',')) {
            this.index++
            continue
          }
          break
        }
      }
      this.skipSoftNewlines()
      this.expectOp('}')
      this.parenDepth--
      return {
        type: 'list',
        items,
        pos: token.pos
      }
    }

    throw new CalcError(
      `这里需要一个数字、变量或左括号，但遇到 “${token.value || '结尾'}”`,
      token.pos.offset
    )
  }

  private binary(
    op: '+' | '-' | '*' | '/' | '^',
    left: Node,
    right: Node,
    pos: SourcePosition
  ): Node {
    return { type: 'binary', op, left, right, pos }
  }

  private current(): Token {
    return this.tokens[this.index]
  }

  private peek(offset: number): Token {
    return this.tokens[Math.min(this.index + offset, this.tokens.length - 1)]
  }

  private atEnd(): boolean {
    return this.current().type === 'eof'
  }

  private atOp(value: string): boolean {
    const token = this.current()
    return token.type === 'op' && token.value === value
  }

  private expectOp(value: string): void {
    const token = this.current()
    if (token.type === 'op' && token.value === value) {
      this.index++
      return
    }
    throw new CalcError(
      `这里需要 “${value}”，但遇到 “${token.value || '结尾'}”`,
      token.pos.offset
    )
  }

  private skipNewlines(): void {
    while (this.current().type === 'newline') this.index++
  }

  private skipSoftNewlines(): void {
    if (this.parenDepth > 0) {
      while (this.current().type === 'newline') this.index++
    }
  }
}
