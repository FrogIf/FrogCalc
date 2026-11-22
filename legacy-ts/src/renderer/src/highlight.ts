export type TokenClass =
  | 'comment'
  | 'string'
  | 'number'
  | 'identifier'
  | 'function'
  | 'constant'
  | 'operator'
  | 'plain'

export interface Segment {
  type: TokenClass
  text: string
}

const CONSTANTS = new Set(['pi', 'e', 'i', 'inf', 'true', 'false'])

function isIdentStart(ch: string): boolean {
  return /[\p{L}_]/u.test(ch)
}

function isIdentPart(ch: string): boolean {
  return /[\p{L}\p{N}_]/u.test(ch)
}

function isDigit(ch: string): boolean {
  return /[0-9]/.test(ch)
}

export function highlight(source: string): Segment[] {
  const segments: Segment[] = []
  let i = 0

  const push = (type: TokenClass, text: string): void => {
    if (!text) return
    const last = segments[segments.length - 1]
    if (last && last.type === type) last.text += text
    else segments.push({ type, text })
  }

  while (i < source.length) {
    const ch = source[i]

    if (ch === '#') {
      let j = i + 1
      while (j < source.length && source[j] !== '\n') j++
      push('comment', source.slice(i, j))
      i = j
      continue
    }

    if (ch === '"') {
      let j = i + 1
      while (j < source.length) {
        if (source[j] === '\\') {
          j += 2
          continue
        }
        if (source[j] === '"') {
          j++
          break
        }
        j++
      }
      push('string', source.slice(i, j))
      i = j
      continue
    }

    if (isDigit(ch) || (ch === '.' && isDigit(source[i + 1] ?? ''))) {
      let j = i
      while (j < source.length && /[0-9_]/.test(source[j])) j++
      if (source[j] === '.') {
        j++
        while (j < source.length && /[0-9_]/.test(source[j])) j++
      }
      if ((source[j] === 'e' || source[j] === 'E') && j + 1 < source.length) {
        let k = j + 1
        if (source[k] === '+' || source[k] === '-') k++
        if (isDigit(source[k] ?? '')) {
          j++
          if (source[j] === '+' || source[j] === '-') j++
          while (j < source.length && /[0-9_]/.test(source[j])) j++
        }
      }
      push('number', source.slice(i, j))
      i = j
      continue
    }

    if (isIdentStart(ch)) {
      let j = i + 1
      while (j < source.length && isIdentPart(source[j])) j++
      const word = source.slice(i, j)
      let look = j
      while (look < source.length && (source[look] === ' ' || source[look] === '\t')) look++
      if (source[look] === '(') push('function', word)
      else if (CONSTANTS.has(word)) push('constant', word)
      else push('identifier', word)
      i = j
      continue
    }

    if (ch === '=' && source[i + 1] === '=') {
      push('operator', '==')
      i += 2
      continue
    }

    if (ch === '*' && source[i + 1] === '*') {
      push('operator', '**')
      i += 2
      continue
    }

    if ('+-*/^=(),;!{}'.includes(ch)) {
      push('operator', ch)
      i++
      continue
    }

    push('plain', ch)
    i++
  }

  return segments
}
