import { Expr } from '../symbolic/expr'
import { toExactComplex, type ExactComplex } from '../symbolic/complex'

const PREC_EQ = 5
const PREC_ADD = 10
const PREC_MUL = 20
const PREC_POW = 30
const PREC_ATOM = 100

function precOf(expr: Expr): number {
  switch (expr.kind) {
    case 'add':
      return PREC_ADD
    case 'mul':
      return PREC_MUL
    case 'pow':
      return PREC_POW
    case 'eq':
    case 'list':
      return PREC_EQ
    default:
      return PREC_ATOM
  }
}

function signOf(expr: Expr): 1 | -1 {
  if (expr.kind === 'num') return expr.value.sign < 0 ? -1 : 1
  if (expr.kind === 'mul') {
    for (const f of expr.factors) {
      if (f.kind === 'num') return f.value.sign < 0 ? -1 : 1
    }
  }
  return 1
}

function absExpr(expr: Expr): Expr {
  if (expr.kind === 'num') {
    return expr.value.sign < 0 ? { kind: 'num', value: expr.value.abs() } : expr
  }
  if (expr.kind === 'mul') {
    return {
      kind: 'mul',
      factors: expr.factors.map((f) =>
        f.kind === 'num' ? { kind: 'num', value: f.value.abs() } : f
      )
    }
  }
  return expr
}

function splitSign(expr: Expr): { sign: 1 | -1; body: Expr } {
  const sign = signOf(expr)
  return { sign, body: sign < 0 ? absExpr(expr) : expr }
}

function paren(text: string, need: boolean, latex: boolean): string {
  if (!need) return text
  return latex ? `\\left(${text}\\right)` : `(${text})`
}

function isNegativePower(expr: Expr): boolean {
  return expr.kind === 'pow' && expr.exp.kind === 'num' && expr.exp.value.sign < 0
}

function positivePower(expr: Expr): Expr {
  if (expr.kind === 'pow' && expr.exp.kind === 'num') {
    return {
      kind: 'pow',
      base: expr.base,
      exp: { kind: 'num', value: expr.exp.value.abs() }
    }
  }
  return expr
}

// ---------------------------------------------------------------------------
// 纯文本
// ---------------------------------------------------------------------------

export function toPlainText(expr: Expr, parentPrec = 0): string {
  if (expr.kind === 'add' || expr.kind === 'mul' || expr.kind === 'pow') {
    const c = toExactComplex(expr)
    if (c && !c.im.isZero) return complexPlain(c, parentPrec)
  }
  switch (expr.kind) {
    case 'num':
      return expr.value.toDisplayString()
    case 'float':
      return expr.value.toString()
    case 'sym':
      return expr.name
    case 'str':
      return JSON.stringify(expr.value)
    case 'add': {
      let out = ''
      expr.terms.forEach((term, index) => {
        const { sign, body } = splitSign(term)
        if (index === 0) out += sign < 0 ? '-' : ''
        else out += sign < 0 ? ' - ' : ' + '
        out += toPlainText(body, PREC_ADD)
      })
      if (out === '') out = '0'
      return paren(out, PREC_ADD < parentPrec, false)
    }
    case 'mul':
      return paren(plainMul(expr.factors), PREC_MUL < parentPrec, false)
    case 'pow':
      return paren(plainPow(expr.base, expr.exp), PREC_POW < parentPrec, false)
    case 'call':
      return `${expr.name}(${expr.args.map((a) => toPlainText(a, 0)).join(', ')})`
    case 'eq':
      return paren(
        `${toPlainText(expr.lhs, 0)} = ${toPlainText(expr.rhs, 0)}`,
        PREC_EQ < parentPrec,
        false
      )
    case 'list':
      return `{${expr.items.map((i) => toPlainText(i, 0)).join(', ')}}`
    default:
      return ''
  }
}

function plainMul(factors: readonly Expr[]): string {
  let coeff: Expr | null = null
  const numerator: Expr[] = []
  const denominator: Expr[] = []
  for (const factor of factors) {
    if ((factor.kind === 'num' || factor.kind === 'float') && coeff === null) {
      coeff = factor
      continue
    }
    if (isNegativePower(factor)) denominator.push(positivePower(factor))
    else numerator.push(factor)
  }

  const factorText = (items: Expr[]): string =>
    items.length === 0 ? '1' : items.map((item) => toPlainText(item, PREC_MUL)).join('*')

  let numeratorText = factorText(numerator)
  if (coeff && (coeff.kind === 'num' || coeff.kind === 'float')) {
    const isOne =
      coeff.kind === 'num'
        ? coeff.value.isOne
        : coeff.value.digits === 1n && coeff.value.exp === 0 && !coeff.value.isNegative
    const isMinusOne =
      coeff.kind === 'num'
        ? coeff.value.isMinusOne
        : coeff.value.digits === 1n && coeff.value.exp === 0 && coeff.value.isNegative
    if (isMinusOne) numeratorText = `-${numeratorText}`
    else if (!isOne) numeratorText = `${toPlainText(coeff, PREC_MUL)}*${numeratorText}`
  }
  if (denominator.length === 0) return numeratorText
  const denominatorText = factorText(denominator)
  return `${paren(numeratorText, numeratorText.startsWith('-'), false)}/${paren(
    denominatorText,
    denominator.length > 1,
    false
  )}`
}

function plainPow(base: Expr, exp: Expr): string {
  const b = paren(
    toPlainText(base, 0),
    base.kind === 'add' || base.kind === 'mul' || base.kind === 'pow',
    false
  )
  if (exp.kind === 'num') {
    const v = exp.value
    if (v.isOne) return b
    if (v.n === 1n && v.d === 2n) return `sqrt(${toPlainText(base, 0)})`
    if (v.n === 1n && v.d === 3n) return `cbrt(${toPlainText(base, 0)})`
    if (v.n === -1n && v.d === 1n) return `1/${paren(toPlainText(base, 0), base.kind !== 'sym' && base.kind !== 'num', false)}`
    if (v.n === -1n && v.d === 2n) return `1/sqrt(${toPlainText(base, 0)})`
  }
  const e = paren(toPlainText(exp, 0), exp.kind === 'add' || exp.kind === 'mul', false)
  return `${b}^${e}`
}

// ---------------------------------------------------------------------------
// LaTeX
// ---------------------------------------------------------------------------

export function toLatex(expr: Expr): string {
  return latexText(expr, 0)
}

function latexText(expr: Expr, parentPrec = 0): string {
  if (expr.kind === 'add' || expr.kind === 'mul' || expr.kind === 'pow') {
    const c = toExactComplex(expr)
    if (c && !c.im.isZero) return complexLatex(c, parentPrec)
  }
  switch (expr.kind) {
    case 'num': {
      const v = expr.value
      const sign = v.n < 0n ? '-' : ''
      const absN = v.n < 0n ? -v.n : v.n
      if (v.d === 1n) return `${sign}${absN}`
      return `${sign}\\frac{${absN}}{${v.d}}`
    }
    case 'float': {
      const text = expr.value.toString()
      const m = /^(-?)(\d+(?:\.\d+)?)e([+-]?\d+)$/.exec(text)
      if (m) {
        return `${m[1]}${m[2]}\\times 10^{${m[3]}}`
      }
      return text
    }
    case 'sym':
      return latexSymbol(expr.name)
    case 'str':
      return `\\text{${escapeLatex(expr.value)}}`
    case 'add': {
      let out = ''
      expr.terms.forEach((term, index) => {
        const { sign, body } = splitSign(term)
        if (index === 0) out += sign < 0 ? '-' : ''
        else out += sign < 0 ? ' - ' : ' + '
        out += latexText(body, PREC_ADD)
      })
      if (out === '') out = '0'
      return paren(out, PREC_ADD < parentPrec, true)
    }
    case 'mul':
      return paren(latexMul(expr.factors), PREC_MUL < parentPrec, true)
    case 'pow':
      return paren(latexPow(expr.base, expr.exp), PREC_POW < parentPrec, true)
    case 'call':
      return latexCall(expr.name, expr.args)
    case 'eq':
      return `${latexText(expr.lhs, 0)} = ${latexText(expr.rhs, 0)}`
    case 'list':
      return `\\left\\{ ${expr.items
        .map((i) => latexText(i, 0))
        .join(',\\; ')} \\right\\}`
    default:
      return ''
  }
}

function latexMul(factors: readonly Expr[]): string {
  let coeff: Expr | null = null
  const numerator: Expr[] = []
  const denominator: Expr[] = []
  for (const factor of factors) {
    if ((factor.kind === 'num' || factor.kind === 'float') && coeff === null) {
      coeff = factor
      continue
    }
    if (isNegativePower(factor)) denominator.push(positivePower(factor))
    else numerator.push(factor)
  }

  const factorText = (items: Expr[]): string =>
    items.length === 0
      ? '1'
      : items
          .map((item) => {
            const text = latexText(item, PREC_MUL)
            return paren(text, precOf(item) < PREC_MUL, true)
          })
          .join('\\,')

  let numeratorText = factorText(numerator)
  if (coeff && (coeff.kind === 'num' || coeff.kind === 'float')) {
    const isOne =
      coeff.kind === 'num'
        ? coeff.value.isOne
        : coeff.value.digits === 1n && coeff.value.exp === 0 && !coeff.value.isNegative
    const isMinusOne =
      coeff.kind === 'num'
        ? coeff.value.isMinusOne
        : coeff.value.digits === 1n && coeff.value.exp === 0 && coeff.value.isNegative
    if (isMinusOne) numeratorText = `-${numeratorText}`
    else if (!isOne) {
      numeratorText =
        numerator.length === 0
          ? latexText(coeff, 0)
          : `${latexText(coeff, PREC_MUL)}\\,${numeratorText}`
    }
  }
  if (denominator.length === 0) return numeratorText
  return `\\frac{${numeratorText}}{${factorText(denominator)}}`
}

function latexPow(base: Expr, exp: Expr): string {
  const wrapBase =
    base.kind === 'add' || base.kind === 'mul' || base.kind === 'pow'
  const b = paren(latexText(base, 0), wrapBase, true)
  if (exp.kind === 'num') {
    const v = exp.value
    if (v.isOne) return b
    if (v.n === 1n && v.d === 2n) return `\\sqrt{${latexText(base, 0)}}`
    if (v.n === 1n && v.d === 3n) return `\\sqrt[3]{${latexText(base, 0)}}`
    if (v.isMinusOne) return `\\frac{1}{${b}}`
    if (v.n === -1n && v.d === 2n) {
      return `\\frac{1}{\\sqrt{${latexText(base, 0)}}}`
    }
  }
  const e = paren(
    latexText(exp, 0),
    exp.kind === 'add' || exp.kind === 'mul',
    true
  )
  return `${b}^{${e}}`
}

function latexSymbol(name: string): string {
  if (name === 'pi') return '\\pi'
  if (name === 'e') return 'e'
  if (name === 'i') return 'i'
  if (name === 'inf') return '\\infty'
  if (/^[A-Za-z]$/.test(name)) return name
  return `\\mathrm{${escapeLatex(name)}}`
}

function escapeLatex(text: string): string {
  return text.replace(/[\\{}_%#$&^~]/g, (ch) => `\\${ch}`)
}

function complexPlain(c: ExactComplex, parentPrec: number): string {
  const imAbs = c.im.abs()
  const imCoeff = imAbs.isOne ? '' : imAbs.toDisplayString()
  const imPart = `${imCoeff}i`
  let out: string
  if (c.re.isZero) {
    out = c.im.sign < 0 ? `-${imPart}` : imPart
  } else {
    out = `${c.re.toDisplayString()} ${c.im.sign < 0 ? '-' : '+'} ${imPart}`
  }
  return paren(out, PREC_ADD < parentPrec, false)
}

function latexRationalValue(value: ExactComplex['re']): string {
  const sign = value.n < 0n ? '-' : ''
  const absN = value.n < 0n ? -value.n : value.n
  if (value.d === 1n) return `${sign}${absN}`
  return `${sign}\\frac{${absN}}{${value.d}}`
}

function complexLatex(c: ExactComplex, parentPrec: number): string {
  const imAbs = c.im.abs()
  const imCoeff = imAbs.isOne ? '' : latexRationalValue(imAbs)
  const imPart = `${imCoeff}i`
  let out: string
  if (c.re.isZero) {
    out = c.im.sign < 0 ? `-${imPart}` : imPart
  } else {
    out = `${latexRationalValue(c.re)} ${c.im.sign < 0 ? '-' : '+'} ${imPart}`
  }
  return paren(out, PREC_ADD < parentPrec, true)
}

const LATEX_FUNCTIONS: Record<string, string> = {
  sin: '\\sin',
  cos: '\\cos',
  tan: '\\tan',
  asin: '\\arcsin',
  acos: '\\arccos',
  atan: '\\arctan',
  sinh: '\\sinh',
  cosh: '\\cosh',
  tanh: '\\tanh',
  exp: '\\exp',
  ln: '\\ln',
  log: '\\log',
  log10: '\\log_{10}',
  log2: '\\log_{2}',
  sign: '\\operatorname{sgn}',
  round: '\\operatorname{round}',
  factorial: '\\operatorname{factorial}',
  gcd: '\\gcd',
  lcm: '\\operatorname{lcm}',
  mod: '\\operatorname{mod}'
}

function latexCall(name: string, args: readonly Expr[]): string {
  const lower = name.toLowerCase()
  if (lower === 'abs' && args.length === 1) {
    return `\\left|${latexText(args[0], 0)}\\right|`
  }
  if (lower === 'floor' && args.length === 1) {
    return `\\left\\lfloor ${latexText(args[0], 0)} \\right\\rfloor`
  }
  if (lower === 'ceil' && args.length === 1) {
    return `\\left\\lceil ${latexText(args[0], 0)} \\right\\rceil`
  }
  if (lower === 'binomial' && args.length === 2) {
    return `\\binom{${latexText(args[0], 0)}}{${latexText(args[1], 0)}}`
  }
  const fn = LATEX_FUNCTIONS[lower]
  const argText = args.map((a) => latexText(a, 0)).join(',\\; ')
  if (fn) return `${fn}\\left(${argText}\\right)`
  return `\\operatorname{${escapeLatex(name)}}\\left(${argText}\\right)`
}
