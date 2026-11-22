import { MathView } from './MathView'
import type { HistoryEntry, ResultView } from '../types'
import { CopyIcon, PlayIcon, SparkIcon } from './Icons'

interface HistoryFeedProps {
  entries: HistoryEntry[]
  displayMode: 'exact' | 'both' | 'approx'
  onInsert: (text: string) => void
  onCopy: (text: string, label: string) => void
  onExample: (input: string) => void
}

const EXAMPLES = [
  {
    title: '精确分数',
    description: '1/3 与 2/3 相加，不损失任何精度',
    input: 'a = 1/3\nb = 2/3\na + b'
  },
  {
    title: '根式化简',
    description: '√2 · √2 自动化为 2',
    input: 'sqrt(2) * sqrt(2)'
  },
  {
    title: '符号微积分',
    description: '求导、积分、化简、因式分解',
    input: 'diff(x^3 * sin(x), x)'
  },
  {
    title: '解方程',
    description: '二次方程给出精确根',
    input: 'solve(x^2 - 2x - 3, x)'
  },
  {
    title: '大数计算',
    description: '1000! 的完整精确结果',
    input: '1000!'
  },
  {
    title: '复数运算',
    description: '精确复数四则运算',
    input: '(2 + 3i) * (4 - 5i)'
  }
]

export function HistoryFeed({
  entries,
  displayMode,
  onInsert,
  onCopy,
  onExample
}: HistoryFeedProps) {
  if (entries.length === 0) {
    return (
      <div className="history-feed empty-feed">
        <div className="hero">
          <div className="hero-mark">
            <SparkIcon size={22} />
          </div>
          <h1>
            精确计算，从 <span className="hero-accent">1/3</span> 开始
          </h1>
          <p>
            FrogCalc 是一个精确优先的桌面计算器：有理数、根式、复数、大整数全程不丢精度，
            同时支持符号求导、积分、化简与方程求解。
          </p>
          <div className="hero-features">
            <span>精确有理数</span>
            <span>任意大整数</span>
            <span>50 位可调精度</span>
            <span>自研 CAS</span>
            <span>中文界面</span>
          </div>
        </div>

        <div className="example-grid">
          {EXAMPLES.map((example) => (
            <button
              key={example.title}
              type="button"
              className="example-card"
              onClick={() => onExample(example.input)}
            >
              <span className="example-card-title">{example.title}</span>
              <span className="example-card-desc">{example.description}</span>
              <code className="example-card-code">{example.input.split('\n')[0]}</code>
              <span className="example-card-run">
                <PlayIcon size={12} />
                运行
              </span>
            </button>
          ))}
        </div>
      </div>
    )
  }

  return (
    <div className="history-feed">
      {entries.map((entry) => (
        <article key={entry.id} className="history-entry">
          <div className="entry-input">
            <span className="entry-prompt">›</span>
            <pre className="entry-source">{entry.input}</pre>
            <div className="entry-meta">
              <span>{formatTime(entry.at)}</span>
              <button
                type="button"
                className="icon-button tiny"
                title="复制输入"
                onClick={() => onCopy(entry.input, '输入')}
              >
                <CopyIcon size={13} />
              </button>
            </div>
          </div>

          <div className="entry-results">
            {entry.results.length === 0 && (
              <div className="result-card muted-card">没有结果</div>
            )}
            {entry.results.map((result, index) => (
              <ResultCard
                key={`${entry.id}-${index}`}
                result={result}
                input={entry.input}
                displayMode={displayMode}
                onInsert={onInsert}
                onCopy={onCopy}
              />
            ))}
          </div>
        </article>
      ))}
    </div>
  )
}

interface ResultCardProps {
  result: ResultView
  input: string
  displayMode: 'exact' | 'both' | 'approx'
  onInsert: (text: string) => void
  onCopy: (text: string, label: string) => void
}

function ResultCard({ result, input, displayMode, onInsert, onCopy }: ResultCardProps) {
  if (result.suppressed) {
    return (
      <div className="result-card suppressed-card">
        <span>已计算（分号抑制输出）</span>
        {result.kind === 'assign' && result.name && (
          <span className="suppressed-name">{result.name} 已更新</span>
        )}
      </div>
    )
  }

  if (!result.ok) {
    return (
      <div className="result-card error-card">
        <div className="error-head">计算出错</div>
        <div className="error-message">{result.error}</div>
        {typeof result.errorPos === 'number' && (
          <ErrorCaret input={input} position={result.errorPos} />
        )}
        <div className="result-foot">
          <span>{formatMs(result.elapsedMs)}</span>
        </div>
      </div>
    )
  }

  const prefix =
    result.kind === 'assign' && result.name ? `${result.name} = ` : ''
  const exactLatex = result.latex ? `${prefix}${result.latex}` : ''
  const exactPlain = result.plain ? `${prefix}${result.plain}` : ''
  const showApproxFirst = displayMode === 'approx' && Boolean(result.approxLatex)
  const primaryLatex = showApproxFirst ? result.approxLatex! : exactLatex
  const primaryPlain = showApproxFirst ? result.approx! : exactPlain

  return (
    <div className={`result-card ${showApproxFirst ? 'approx-primary' : ''}`}>
      <div className="result-head">
        <span className="result-type">{result.valueType ?? '结果'}</span>
        {showApproxFirst && <span className="approx-badge">近似</span>}
        <span className="result-time">{formatMs(result.elapsedMs)}</span>
      </div>

      <div className="result-math">
        {result.valueType === '文本' ? (
          <pre className="result-text">{primaryPlain}</pre>
        ) : primaryPlain.length > 2000 ? (
          <pre className="result-long-number">{primaryPlain}</pre>
        ) : (
          <MathView latex={primaryLatex} />
        )}
      </div>

      {!showApproxFirst && result.approxLatex && (
        <div className="result-approx">
          <span className="approx-symbol">≈</span>
          <MathView latex={result.approxLatex} display={false} />
        </div>
      )}

      {showApproxFirst && exactLatex && (
        <div className="result-exact-line">
          <span>精确值</span>
          <MathView latex={exactLatex} display={false} />
        </div>
      )}

      {result.note && <div className="result-note">{result.note}</div>}

      <div className="result-actions">
        <button
          type="button"
          className="text-button"
          onClick={() => onInsert(primaryPlain)}
        >
          插入
        </button>
        <button
          type="button"
          className="text-button"
          onClick={() => onCopy(primaryPlain, '结果')}
        >
          复制
        </button>
        <button
          type="button"
          className="text-button"
          onClick={() => onCopy(primaryLatex, 'LaTeX')}
        >
          LaTeX
        </button>
      </div>
    </div>
  )
}

function ErrorCaret({ input, position }: { input: string; position: number }) {
  const safe = Math.max(0, Math.min(position, input.length))
  const before = input.slice(0, safe)
  const lineStart = before.lastIndexOf('\n') + 1
  const column = safe - lineStart
  const lineEnd = input.indexOf('\n', safe)
  const line = input.slice(lineStart, lineEnd === -1 ? input.length : lineEnd)
  return (
    <pre className="error-caret">
      {line}
      {'\n'}
      {' '.repeat(column)}
      ^
    </pre>
  )
}

function formatTime(at: number): string {
  return new Date(at).toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit'
  })
}

function formatMs(ms: number): string {
  if (ms < 1) return '<1 ms'
  if (ms < 1000) return `${Math.round(ms)} ms`
  return `${(ms / 1000).toFixed(2)} s`
}
