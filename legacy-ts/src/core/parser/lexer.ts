export interface SourcePosition {
  readonly offset: number
  readonly line: number
  readonly column: number
}

export type TokenType = 'number' | 'ident' | 'string' | 'op' | 'newline' | 'eof'

export interface Token {
  readonly type: TokenType
  readonly value: string
  readonly pos: SourcePosition
}

const OP_CHARS = new Set(['+', '-', '*', '/', '^', '=', '(', ')', ',', ';', '=', '!', '{', '}'])

function isIdentStart(ch: string): boolean {
  return /[\p{L}_]/u.test(ch)
}

function isIdentPart(ch: string): boolean {
  return /[\p{L}\p{N}_]/u.test(ch)
}

export function tokenize(source: string): Token[] {
  const tokens: Token[] = []
  let i = 0
  let line = 1
  let column = 1

  const pos = (): SourcePosition => ({ offset: i, line, column })

  const advance = (): string => {
    const ch = source[i]
    i++
    if (ch === '\n') {
      line++
      column = 1
    } else {
      column++
    }
    return ch
  }

  while (i < source.length) {
    const ch = source[i]

    if (ch === ' ' || ch === '\t' || ch === '\r') {
      advance()
      continue
    }

    if (ch === '\n') {
      tokens.push({ type: 'newline', value: '\n', pos: pos() })
      advance()
      continue
    }

    if (ch === '#') {
      while (i < source.length && source[i] !== '\n') advance()
      continue
    }

    if (ch === '"') {
      const start = pos()
      advance()
      let value = ''
      let closed = false
      while (i < source.length) {
        const c = advance()
        if (c === '"') {
          closed = true
          break
        }
        if (c === '\\') {
          const esc = advance()
          if (esc === 'n') value += '\n'
          else if (esc === 't') value += '\t'
          else if (esc === '"') value += '"'
          else if (esc === '\\') value += '\\'
          else value += esc
        } else {
          value += c
        }
      }
      if (!closed) {
        throw new Error(`字符串未闭合（第 ${start.line} 行第 ${start.column} 列）`)
      }
      tokens.push({ type: 'string', value, pos: start })
      continue
    }

    if (/[0-9]/.test(ch) || (ch === '.' && /[0-9]/.test(source[i + 1] ?? ''))) {
      const start = pos()
      let raw = ''
      while (i < source.length && /[0-9_]/.test(source[i])) {
        raw += advance()
      }
      if (source[i] === '.') {
        raw += advance()
        while (i < source.length && /[0-9_]/.test(source[i])) raw += advance()
      }
      if ((source[i] === 'e' || source[i] === 'E') && i + 1 < source.length) {
        let j = i + 1
        if (source[j] === '+' || source[j] === '-') j++
        if (/[0-9]/.test(source[j] ?? '')) {
          raw += advance() // e
          if (source[i] === '+' || source[i] === '-') raw += advance()
          while (i < source.length && /[0-9_]/.test(source[i])) raw += advance()
        }
      }
      tokens.push({ type: 'number', value: raw.replace(/_/g, ''), pos: start })
      continue
    }

    if (isIdentStart(ch)) {
      const start = pos()
      let name = ''
      while (i < source.length && isIdentPart(source[i])) name += advance()
      tokens.push({ type: 'ident', value: name, pos: start })
      continue
    }

    if (ch === '=' && source[i + 1] === '=') {
      const start = pos()
      advance()
      advance()
      tokens.push({ type: 'op', value: '==', pos: start })
      continue
    }

    if (ch === '*' && source[i + 1] === '*') {
      const start = pos()
      advance()
      advance()
      tokens.push({ type: 'op', value: '**', pos: start })
      continue
    }

    if (OP_CHARS.has(ch)) {
      const start = pos()
      advance()
      tokens.push({ type: 'op', value: ch, pos: start })
      continue
    }

    throw new Error(
      `无法识别的字符 “${ch}”（第 ${line} 行第 ${column} 列）`
    )
  }

  tokens.push({ type: 'eof', value: '', pos: pos() })
  return tokens
}
