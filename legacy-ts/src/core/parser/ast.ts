import { Rational } from '../numeric/rational'
import type { SourcePosition } from './lexer'

export type Node =
  | NumberNode
  | StringNode
  | IdentNode
  | UnaryNode
  | BinaryNode
  | CallNode
  | EqNode
  | ListNode

export interface NumberNode {
  readonly type: 'number'
  readonly value: Rational
  readonly raw: string
  readonly pos: SourcePosition
}

export interface StringNode {
  readonly type: 'string'
  readonly value: string
  readonly pos: SourcePosition
}

export interface IdentNode {
  readonly type: 'ident'
  readonly name: string
  readonly pos: SourcePosition
}

export interface UnaryNode {
  readonly type: 'unary'
  readonly op: '+' | '-'
  readonly arg: Node
  readonly pos: SourcePosition
}

export interface BinaryNode {
  readonly type: 'binary'
  readonly op: '+' | '-' | '*' | '/' | '^'
  readonly left: Node
  readonly right: Node
  readonly pos: SourcePosition
}

export interface CallNode {
  readonly type: 'call'
  readonly name: string
  readonly args: readonly Node[]
  readonly pos: SourcePosition
}

export interface EqNode {
  readonly type: 'eq'
  readonly left: Node
  readonly right: Node
  readonly pos: SourcePosition
}

export interface ListNode {
  readonly type: 'list'
  readonly items: readonly Node[]
  readonly pos: SourcePosition
}

export type Statement =
  | ExprStatement
  | AssignStatement
  | FuncDefStatement

export interface ExprStatement {
  readonly type: 'expr'
  readonly expr: Node
  readonly suppress: boolean
  readonly pos: SourcePosition
}

export interface AssignStatement {
  readonly type: 'assign'
  readonly name: string
  readonly value: Node
  readonly suppress: boolean
  readonly pos: SourcePosition
}

export interface FuncDefStatement {
  readonly type: 'funcdef'
  readonly name: string
  readonly params: readonly string[]
  readonly body: Node
  readonly suppress: boolean
  readonly pos: SourcePosition
}

export interface Program {
  readonly statements: Statement[]
}
