import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  DEFAULT_SETTINGS,
  Session,
  type CalcSettings
} from '@core/index'
import { TitleBar } from './components/TitleBar'
import { Sidebar } from './components/Sidebar'
import { HistoryFeed } from './components/HistoryFeed'
import { Composer, type Candidate, type ComposerHandle } from './components/Composer'
import { SettingsDialog } from './components/SettingsDialog'
import { toView } from './types'
import type { HistoryEntry, SavedState } from './types'

const STORAGE_KEY = 'frogcalc-state-v1'

const BUILTIN_FUNCTIONS = [
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
  'help',
  'clear',
  'vars',
  'subs',
  'setprecision',
  'setangle'
]

const COMPOSER_EXAMPLES = [
  '1/3 + 1/6',
  'sqrt(2) * sqrt(2)',
  'diff(x^3*sin(x), x)',
  'solve(x^2 - 2x - 3, x)'
]

export function App() {
  const sessionRef = useRef<Session | null>(null)
  if (!sessionRef.current) sessionRef.current = new Session()
  const session = sessionRef.current

  const [settings, setSettings] = useState<CalcSettings>(DEFAULT_SETTINGS)
  const [history, setHistory] = useState<HistoryEntry[]>([])
  const [workspace, setWorkspace] = useState(session.getWorkspace())
  const [input, setInput] = useState('')
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [toast, setToast] = useState<string | null>(null)
  const [ready, setReady] = useState(false)

  const composerRef = useRef<ComposerHandle>(null)
  const feedRef = useRef<HTMLDivElement>(null)
  const loadedRef = useRef(false)
  const toastTimer = useRef<number | null>(null)

  useEffect(() => {
    let cancelled = false

    const load = async (): Promise<void> => {
      let saved: SavedState | null = null
      try {
        if (window.frog) {
          const raw = await window.frog.loadState()
          if (raw && typeof raw === 'object') saved = raw as SavedState
        } else {
          const raw = localStorage.getItem(STORAGE_KEY)
          if (raw) saved = JSON.parse(raw) as SavedState
        }
      } catch {
        saved = null
      }
      if (cancelled) return

      if (saved && saved.version === 1 && Array.isArray(saved.history)) {
        session.setSettings({ ...DEFAULT_SETTINGS, ...(saved.settings ?? {}) })
        setSettings(session.getSettings())
        if (typeof saved.workspaceSource === 'string') {
          if (saved.workspaceSource.trim().length > 0) {
            session.execute(saved.workspaceSource)
          }
        } else {
          for (const entry of saved.history) {
            session.execute(entry.input)
          }
        }
        setHistory(saved.history.slice(-200))
        setWorkspace(session.getWorkspace())
      }

      loadedRef.current = true
      setReady(true)
    }

    void load()
    return () => {
      cancelled = true
    }
  }, [session])

  useEffect(() => {
    if (!loadedRef.current) return
    const timer = window.setTimeout(() => {
      const state: SavedState = {
        version: 1,
        history,
        settings,
        workspaceSource: session.getWorkspaceSource()
      }
      if (window.frog) {
        void window.frog.saveState(state)
      } else {
        try {
          localStorage.setItem(STORAGE_KEY, JSON.stringify(state))
        } catch {
          // 忽略存储失败（例如配额不足）
        }
      }
    }, 350)
    return () => window.clearTimeout(timer)
  }, [history, settings, workspace, session])

  useEffect(() => {
    if (!ready) return
    composerRef.current?.focus()
  }, [ready])

  useEffect(() => {
    const handler = (event: KeyboardEvent): void => {
      const mod = event.ctrlKey || event.metaKey
      if (mod && event.key === ',') {
        event.preventDefault()
        setSettingsOpen(true)
      }
      if (mod && event.key.toLowerCase() === 'k') {
        event.preventDefault()
        composerRef.current?.focus()
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [])

  const showToast = useCallback((message: string) => {
    setToast(message)
    if (toastTimer.current !== null) window.clearTimeout(toastTimer.current)
    toastTimer.current = window.setTimeout(() => setToast(null), 1800)
  }, [])

  const execute = useCallback(
    (rawInput: string) => {
      const text = rawInput.trim()
      if (!text) return
      const results = session.execute(text).map(toView)
      const entry: HistoryEntry = {
        id:
          typeof crypto !== 'undefined' && 'randomUUID' in crypto
            ? crypto.randomUUID()
            : `${Date.now()}-${Math.random()}`,
        input: text,
        results,
        at: Date.now()
      }
      setHistory((previous) => [...previous.slice(-199), entry])
      setWorkspace(session.getWorkspace())
      setInput('')
      requestAnimationFrame(() => {
        const feed = feedRef.current
        if (feed) feed.scrollTop = feed.scrollHeight
        composerRef.current?.focus()
      })
    },
    [session]
  )

  const copy = useCallback(
    async (text: string, label: string) => {
      if (!text) return
      try {
        await navigator.clipboard.writeText(text)
      } catch {
        const area = document.createElement('textarea')
        area.value = text
        area.style.position = 'fixed'
        area.style.opacity = '0'
        document.body.appendChild(area)
        area.select()
        try {
          document.execCommand('copy')
        } finally {
          document.body.removeChild(area)
        }
      }
      showToast(`已复制${label}`)
    },
    [showToast]
  )

  const insert = useCallback((text: string) => {
    composerRef.current?.insert(text)
    composerRef.current?.focus()
  }, [])

  const handleSettingsChange = useCallback(
    (patch: Partial<CalcSettings>) => {
      session.setSettings(patch)
      setSettings(session.getSettings())
    },
    [session]
  )

  const clearWorkspace = useCallback(() => {
    session.clearAll()
    setWorkspace([])
    showToast('工作区已清空')
  }, [session, showToast])

  const clearHistory = useCallback(() => {
    setHistory([])
    showToast('历史记录已清空')
  }, [showToast])

  const exportHistory = useCallback(async () => {
    const lines: string[] = [
      'FrogCalc 历史记录',
      `导出时间：${new Date().toLocaleString('zh-CN')}`,
      ''
    ]
    for (const entry of history) {
      lines.push(`> ${entry.input}`)
      for (const result of entry.results) {
        if (result.suppressed) continue
        if (!result.ok) {
          lines.push(`错误：${result.error ?? '未知错误'}`)
          continue
        }
        const prefix = result.kind === 'assign' && result.name ? `${result.name} = ` : ''
        lines.push(`${prefix}${result.plain ?? ''}`)
        if (result.approx) lines.push(`≈ ${result.approx}`)
        if (result.note) lines.push(`（${result.note}）`)
      }
      lines.push('')
    }
    const content = lines.join('\n')

    if (window.frog) {
      const result = await window.frog.exportFile('frogcalc-history.txt', content)
      if (!result.canceled) showToast('历史已导出')
      return
    }

    const blob = new Blob([content], { type: 'text/plain;charset=utf-8' })
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = 'frogcalc-history.txt'
    link.click()
    URL.revokeObjectURL(url)
    showToast('历史已导出')
  }, [history, showToast])

  const candidates = useMemo<Candidate[]>(() => {
    const map = new Map<string, Candidate>()
    for (const item of workspace) {
      map.set(item.name, {
        name: item.name,
        kind: item.kind === 'function' ? '函数' : '变量'
      })
    }
    for (const name of BUILTIN_FUNCTIONS) {
      if (!map.has(name)) map.set(name, { name, kind: '函数' })
    }
    for (const name of ['pi', 'e', 'i']) {
      if (!map.has(name)) map.set(name, { name, kind: '常量' })
    }
    return [...map.values()]
  }, [workspace])

  const historyInputs = useMemo(() => history.map((entry) => entry.input), [history])

  if (!ready) {
    return (
      <div className="boot-screen">
        <div className="boot-mark">ƒ</div>
        <div className="boot-name">FrogCalc</div>
        <div className="boot-hint">正在加载精确计算内核…</div>
      </div>
    )
  }

  return (
    <div className="app">
      <TitleBar
        precision={settings.precision}
        angleMode={settings.angleMode}
        onOpenSettings={() => setSettingsOpen(true)}
      />

      <div className="app-body">
        <Sidebar
          workspace={workspace}
          onInsert={insert}
          onClearWorkspace={clearWorkspace}
        />

        <main className="main">
          <div className="main-scroll" ref={feedRef}>
            <HistoryFeed
              entries={history}
              displayMode={settings.displayMode}
              onInsert={insert}
              onCopy={copy}
              onExample={execute}
            />
          </div>

          <Composer
            ref={composerRef}
            value={input}
            onChange={setInput}
            onSubmit={execute}
            candidates={candidates}
            historyInputs={historyInputs}
            examples={COMPOSER_EXAMPLES}
            onExample={execute}
          />
        </main>
      </div>

      <SettingsDialog
        open={settingsOpen}
        settings={settings}
        historyCount={history.length}
        onChange={handleSettingsChange}
        onClose={() => setSettingsOpen(false)}
        onClearWorkspace={clearWorkspace}
        onClearHistory={clearHistory}
        onExport={() => void exportHistory()}
      />

      {toast && <div className="toast">{toast}</div>}
    </div>
  )
}
