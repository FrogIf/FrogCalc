import { describe, expect, it } from 'vitest'
import { Session } from '@core/index'

function run(input: string): { plain: string; session: Session } {
  const session = new Session()
  const results = session.execute(input)
  const last = results[results.length - 1]
  expect(last.ok, last.error ?? input).toBe(true)
  return { plain: last.plain ?? '', session }
}

describe('积分扩展', () => {
  it('基本积分表', () => {
    expect(run('integrate(1/x, x)').plain).toContain('ln')
    expect(run('integrate(1/(x^2 + 1), x)').plain).toBe('atan(x)')
    expect(run('integrate(1/(x^2 + 3), x)').plain).toContain('atan')
    expect(run('integrate(sin(x), x)').plain).toBe('-cos(x)')
    expect(run('integrate(cos(2x), x)').plain).toBe('0.5*sin(2*x)')
  })

  it('有理函数与分部积分', () => {
    expect(run('integrate(1/(x^2 - 1), x)').plain).toContain('ln')
    expect(run('integrate(x^2*exp(x), x)').plain).toContain('exp(x)')
    expect(run('integrate(x*cos(x), x)').plain).toContain('cos(x)')
    expect(run('integrate(ln(x), x)').plain).toContain('ln(x)')
    expect(run('integrate(2*x/(x^2 + 1), x)').plain).toContain('ln')
  })

  it('定积分', () => {
    expect(run('integrate(x^2, x, 0, 1)').plain).toBe('1/3')
    const piResult = run('integrate(sin(x), x, 0, pi)').plain
    expect(piResult).toBe('2')
  })
})

describe('方程扩展', () => {
  it('高次与复数根', () => {
    expect(run('solve(x^4 - 5x^2 + 4, x)').plain).toBe('{-2, -1, 1, 2}')
    expect(run('solve(x^2 + 1, x)').plain).toContain('i')
    expect(run('solve(x^3 - 1, x)').plain).toContain('i')
  })

  it('超越方程逆运算', () => {
    expect(run('solve(exp(x) == 2, x)').plain).toContain('ln(2)')
    expect(run('solve(ln(x) == 1, x)').plain).toContain('e')
    expect(run('solve(abs(x) == 3, x)').plain).toContain('3')
  })
})

describe('化简扩展', () => {
  it('分式通分', () => {
    const result = run('simplify(1/x + 1/y)').plain
    expect(result).toContain('x')
    expect(result).toContain('y')
    expect(result).toContain('/')
  })

  it('根式与因式分解', () => {
    expect(run('sqrt(12)').plain).toBe('2*sqrt(3)')
    expect(run('factor(x^2 + 2x + 1)').plain).toBe('(x + 1)^2')
    expect(run('expand((x + y)^2)').plain).toContain('2*x*y')
  })
})

describe('语言与设置', () => {
  it('角度制', () => {
    const session = new Session()
    session.execute('setangle("deg")')
    const result = session.execute('sin(30)')[0]
    expect(result.ok).toBe(true)
    expect(result.plain).toBe('0.5')
    const inverse = session.execute('asin(1/2)')[0]
    expect(inverse.plain).toBe('30')
  })

  it('替换与精度设置', () => {
    expect(run('subs(x^2 + 1, x, 3)').plain).toBe('10')
    const session = new Session()
    session.execute('setprecision(30)')
    const result = session.execute('N(pi)')[0]
    expect(result.ok).toBe(true)
    expect(result.plain).toMatch(/^3\.14159265358979323846264338328/)
  })

  it('分号抑制与多语句', () => {
    const session = new Session()
    const results = session.execute('a = 2; b = 3; a + b')
    expect(results.length).toBe(3)
    expect(results[0].suppressed).toBe(true)
    expect(results[1].suppressed).toBe(true)
    expect(results[2].plain).toBe('5')
  })

  it('工作区持久化文本', () => {
    const session = new Session()
    session.execute('a = 1/3\nf(x) = x^2 + 1')
    const source = session.getWorkspaceSource()
    const restored = new Session()
    restored.execute(source)
    const values = restored.getWorkspace()
    expect(values.find((v) => v.name === 'a')?.plain).toBe('1/3')
    expect(values.find((v) => v.name === 'f')?.signature).toBe('f(x)')
  })

  it('列表字面量', () => {
    const session = new Session()
    const result = session.execute('x = {1, 2, 3}')[0]
    expect(result.ok).toBe(true)
    expect(result.plain).toBe('{1, 2, 3}')
    const restored = new Session()
    restored.execute(session.getWorkspaceSource())
    expect(restored.getWorkspace().find((v) => v.name === 'x')?.plain).toBe(
      '{1, 2, 3}'
    )
  })
})
