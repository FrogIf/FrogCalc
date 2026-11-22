import { describe, expect, it } from 'vitest'
import { Session } from '@core/index'

function session(): Session {
  return new Session()
}

function plain(s: Session, input: string): string {
  const results = s.execute(input)
  const last = results[results.length - 1]
  expect(last, `没有结果：${input}`).toBeDefined()
  expect(last.ok, last.error ?? '').toBe(true)
  return last.plain ?? ''
}

function errorOf(s: Session, input: string): string {
  const results = s.execute(input)
  const last = results[results.length - 1]
  expect(last.ok).toBe(false)
  return last.error ?? ''
}

describe('精确算术', () => {
  it('分数保持精确', () => {
    const s = session()
    expect(plain(s, 'a = 1 / 3')).toBe('1/3')
    expect(plain(s, 'b = 2 / 3')).toBe('2/3')
    expect(plain(s, 'a + b')).toBe('1')
    expect(plain(s, 'a * 3')).toBe('1')
    expect(plain(s, '1/3 + 1/6')).toBe('0.5')
  })

  it('小数以精确有理数存储', () => {
    const s = session()
    expect(plain(s, '0.1 + 0.2')).toBe('0.3')
    expect(plain(s, '0.1 + 0.2 == 0.3')).toBe('0.3 = 0.3')
  })

  it('根式精确化简', () => {
    const s = session()
    expect(plain(s, 'sqrt(2) * sqrt(2)')).toBe('2')
    expect(plain(s, 'sqrt(2)^2')).toBe('2')
    expect(plain(s, 'sqrt(8)')).toBe('2*sqrt(2)')
    expect(plain(s, 'sqrt(8) / sqrt(2)')).toBe('2')
    expect(plain(s, 'sqrt(4)')).toBe('2')
    expect(plain(s, 'sqrt(-4)')).toBe('2i')
  })

  it('大整数', () => {
    const s = session()
    expect(plain(s, '2^10000').length).toBe(3011)
    expect(plain(s, '1000!').length).toBe(2568)
  })

  it('复数四则运算', () => {
    const s = session()
    expect(plain(s, '(2 + 3i) * (4 - 5i)')).toBe('23 + 2i')
    expect(plain(s, 'i^2')).toBe('-1')
    expect(plain(s, 'i^3')).toBe('-i')
    expect(plain(s, '(1 + i) / (1 - i)')).toBe('i')
  })
})

describe('解析器', () => {
  it('优先级与结合性', () => {
    const s = session()
    expect(plain(s, '2 + 3 * 4')).toBe('14')
    expect(plain(s, '-2^2')).toBe('-4')
    expect(plain(s, '2^3^2')).toBe('512')
    expect(plain(s, '2(3 + 4)')).toBe('14')
    expect(plain(s, '2^3!')).toBe('64')
  })

  it('隐式乘法', () => {
    const s = session()
    expect(plain(s, '2x')).toBe('2*x')
    const factored = plain(s, '(x + 1)(x - 1)')
    expect(factored).toContain('(x + 1)')
    expect(factored).toContain('(x - 1)')
  })

  it('函数定义与调用', () => {
    const s = session()
    expect(plain(s, 'f(x) = x^2 + 1')).toContain('f(x) = x^2 + 1')
    expect(plain(s, 'f(3)')).toBe('10')
    expect(plain(s, 'g(x, y) = x * y + 1')).toContain('g(x, y)')
    expect(plain(s, 'g(2, 3)')).toBe('7')
  })

  it('错误定位', () => {
    const s = session()
    expect(errorOf(s, '1 + ')).toContain('这里需要')
  })

  it('括号内允许多行', () => {
    const s = session()
    const result = s.execute('diff(\n  x^3 * sin(x),\n  x\n)')[0]
    expect(result.ok).toBe(true)
    expect(result.plain).toContain('cos(x)')
  })
})

describe('符号运算', () => {
  it('求导', () => {
    const s = session()
    expect(plain(s, 'diff(x^3 * sin(x), x)')).toContain('x^3*cos(x)')
    expect(plain(s, 'diff(x^3 * sin(x), x)')).toContain('3*x^2*sin(x)')
    expect(plain(s, 'diff(sin(x^2), x)')).toBe('2*x*cos(x^2)')
  })

  it('积分', () => {
    const s = session()
    expect(plain(s, 'integrate(x^2, x)')).toBe('1/3*x^3')
    expect(plain(s, 'integrate(x * exp(x), x)')).toContain('exp(x)')
    expect(plain(s, 'integrate(1/(x^2 - 1), x)')).toContain('ln')
    expect(plain(s, 'integrate(2*x*cos(x^2), x)')).toBe('sin(x^2)')
  })

  it('化简、展开、因式分解', () => {
    const s = session()
    expect(plain(s, 'simplify((x^2 - 1)/(x - 1))')).toBe('x + 1')
    expect(plain(s, 'expand((x + 1)^5)')).toBe('x^5 + 5*x^4 + 10*x^3 + 10*x^2 + 5*x + 1')
    expect(plain(s, 'factor(x^4 - 1)')).toBe('(x^2 + 1)*(x - 1)*(x + 1)')
  })

  it('解方程', () => {
    const s = session()
    expect(plain(s, 'solve(x^2 - 2x - 3, x)')).toBe('{-1, 3}')
    expect(plain(s, 'solve(sin(x) == 0, x)')).toBe('{0, pi}')
    expect(plain(s, 'solve(2*sin(3x + 1) == 1/2, x)')).toContain('asin(0.25)')
  })
})

describe('近似计算', () => {
  it('圆周率与常数', () => {
    const s = session()
    expect(plain(s, 'N(pi, 50)')).toMatch(/^3\.14159265358979323846/)
    expect(plain(s, 'N(e, 50)')).toMatch(/^2\.71828182845904523536/)
    expect(plain(s, 'N(sqrt(2), 50)')).toMatch(/^1\.41421356237309504880/)
    expect(plain(s, 'N(ln(2), 50)')).toMatch(/^0\.69314718055994530941/)
    expect(plain(s, 'N(sin(1), 50)')).toMatch(/^0\.84147098480789650665/)
  })

  it('100 位精度', () => {
    const s = session()
    expect(plain(s, 'N(pi, 100)')).toBe(
      '3.141592653589793238462643383279502884197169399375105820974944592307816406286208998628034825342117068'
    )
  })

  it('默认显示精确值并附带近似值', () => {
    const s = session()
    const results = s.execute('1/3')
    expect(results[0].plain).toBe('1/3')
    expect(results[0].approx).toMatch(/^0\.333333/)
  })

  it('复杂表达式近似', () => {
    const s = session()
    expect(plain(s, 'N(exp(i*pi), 30)')).toMatch(/^-1/)
    expect(plain(s, 'N(2i, 20)')).toMatch(/^2\*i$/)
    expect(plain(s, 'N(3 + 2i, 20)')).toMatch(/^3 \+ 2\*i$/)
  })
})

describe('LaTeX 输出', () => {
  it('基本结构', () => {
    const s = session()
    const result = s.execute('sqrt(2)/2 + x^2')[0]
    expect(result.latex).toContain('\\frac')
    expect(result.latex).toContain('x^{2}')
  })
})

describe('健壮性', () => {
  it('未求值的积分与方程', () => {
    const s = session()
    const integral = s.execute('integrate(exp(-x^2), x)')[0]
    expect(integral.ok).toBe(true)
    expect(integral.plain).toContain('integrate')
    expect(integral.note).toContain('未求值')

    const equation = s.execute('solve(x^x == 2, x)')[0]
    expect(equation.ok).toBe(true)
    expect(equation.plain).toContain('solve')
    expect(equation.note).toContain('未求值')
  })

  it('错误与 ans', () => {
    const s = session()
    expect(errorOf(s, '1/0')).toContain('除数')
    expect(plain(s, '2 + 3')).toBe('5')
    expect(plain(s, 'ans * 2')).toBe('10')
  })

  it('帮助与清空', () => {
    const s = session()
    expect(plain(s, 'help("syntax")')).toContain('FrogCalc 语言')
    s.execute('a = 1')
    expect(s.getWorkspace().length).toBe(1)
    s.execute('clear()')
    expect(s.getWorkspace().length).toBe(0)
  })
})
