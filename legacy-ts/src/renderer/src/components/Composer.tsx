import {
  forwardRef,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
  useState
} from 'react'
import { highlight } from '../highlight'
import { PlayIcon } from './Icons'

export interface Candidate {
  name: string
  kind: '变量' | '函数' | '常量'
}

export interface ComposerHandle {
  insert: (text: string) => void
  focus: () => void
}

interface ComposerProps {
  value: string
  onChange: (value: string) => void
  onSubmit: (input: string) => void
  candidates: Candidate[]
  historyInputs: string[]
  examples: string[]
  onExample: (input: string) => void
}

interface AutocompleteState {
  word: string
  start: number
  end: number
  items: Candidate[]
  index: number
}

export const Composer = forwardRef<ComposerHandle, ComposerProps>(function Composer(
  { value, onChange, onSubmit, candidates, historyInputs, examples, onExample },
  ref
) {
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const overlayRef = useRef<HTMLPreElement>(null)
  const [autocomplete, setAutocomplete] = useState<AutocompleteState | null>(null)
  const [historyIndex, setHistoryIndex] = useState(-1)
  const draftRef = useRef('')

  useImperativeHandle(ref, () => ({
    insert(text: string) {
      const el = textareaRef.current
      if (!el) {
        onChange(value + text)
        return
      }
      const start = el.selectionStart
      const end = el.selectionEnd
      const next = value.slice(0, start) + text + value.slice(end)
      onChange(next)
      requestAnimationFrame(() => {
        el.focus()
        const pos = start + text.length
        el.setSelectionRange(pos, pos)
      })
    },
    focus() {
      textareaRef.current?.focus()
    }
  }))

  useLayoutEffect(() => {
    const el = textareaRef.current
    if (!el) return
    el.style.height = 'auto'
    const height = Math.min(el.scrollHeight, 240)
    el.style.height = `${Math.max(56, height)}px`
  }, [value])

  const updateAutocomplete = (text: string, caret: number): void => {
    const before = text.slice(0, caret)
    const match = /([\p{L}_][\p{L}\p{N}_]*)$/u.exec(before)
    if (!match) {
      setAutocomplete(null)
      return
    }
    const word = match[1]
    if (word.length < 2) {
      setAutocomplete(null)
      return
    }
    const start = caret - word.length
    const items = candidates
      .filter(
        (candidate) =>
          candidate.name.toLowerCase().startsWith(word.toLowerCase()) &&
          candidate.name !== word
      )
      .slice(0, 8)
    if (items.length === 0) {
      setAutocomplete(null)
      return
    }
    setAutocomplete({ word, start, end: caret, items, index: 0 })
  }

  const handleChange = (text: string): void => {
    onChange(text)
    setHistoryIndex(-1)
    draftRef.current = ''
    const el = textareaRef.current
    updateAutocomplete(text, el ? el.selectionStart : text.length)
  }

  const applyCandidate = (candidate: Candidate): void => {
    if (!autocomplete || !textareaRef.current) return
    const isFunction = candidate.kind === '函数'
    const inserted = isFunction ? `${candidate.name}(` : candidate.name
    const next =
      value.slice(0, autocomplete.start) + inserted + value.slice(autocomplete.end)
    onChange(next)
    setAutocomplete(null)
    const caret = autocomplete.start + inserted.length
    requestAnimationFrame(() => {
      const el = textareaRef.current
      if (!el) return
      el.focus()
      el.setSelectionRange(caret, caret)
    })
  }

  const submit = (): void => {
    const text = value.trim()
    if (!text) return
    setAutocomplete(null)
    setHistoryIndex(-1)
    draftRef.current = ''
    onSubmit(text)
  }

  const navigateHistory = (direction: -1 | 1): void => {
    if (historyInputs.length === 0) return
    let nextIndex = historyIndex
    if (direction === -1) {
      if (historyIndex === -1) {
        draftRef.current = value
        nextIndex = historyInputs.length - 1
      } else {
        nextIndex = Math.max(0, historyIndex - 1)
      }
    } else {
      if (historyIndex === -1) return
      nextIndex = historyIndex + 1
      if (nextIndex >= historyInputs.length) {
        setHistoryIndex(-1)
        onChange(draftRef.current)
        return
      }
    }
    setHistoryIndex(nextIndex)
    onChange(historyInputs[nextIndex])
    requestAnimationFrame(() => {
      const el = textareaRef.current
      if (!el) return
      el.focus()
      el.setSelectionRange(el.value.length, el.value.length)
    })
  }

  const handleKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>): void => {
    const el = event.currentTarget

    if (autocomplete) {
      if (event.key === 'ArrowDown') {
        event.preventDefault()
        setAutocomplete({
          ...autocomplete,
          index: (autocomplete.index + 1) % autocomplete.items.length
        })
        return
      }
      if (event.key === 'ArrowUp') {
        event.preventDefault()
        setAutocomplete({
          ...autocomplete,
          index:
            (autocomplete.index - 1 + autocomplete.items.length) %
            autocomplete.items.length
        })
        return
      }
      if (event.key === 'Tab') {
        event.preventDefault()
        applyCandidate(autocomplete.items[autocomplete.index])
        return
      }
      if (event.key === 'Escape') {
        event.preventDefault()
        setAutocomplete(null)
        return
      }
    }

    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault()
      setAutocomplete(null)
      submit()
      return
    }

    if (event.key === 'ArrowUp' && el.selectionStart === 0 && el.selectionEnd === 0) {
      event.preventDefault()
      navigateHistory(-1)
      return
    }

    if (
      event.key === 'ArrowDown' &&
      el.selectionStart === el.value.length &&
      el.selectionEnd === el.value.length
    ) {
      event.preventDefault()
      navigateHistory(1)
      return
    }

    if (event.key === 'Tab') {
      event.preventDefault()
      const start = el.selectionStart
      const end = el.selectionEnd
      const next = `${value.slice(0, start)}  ${value.slice(end)}`
      onChange(next)
      requestAnimationFrame(() => {
        el.setSelectionRange(start + 2, start + 2)
      })
    }
  }

  const segments = highlight(value)
  const isEmpty = value.length === 0

  return (
    <div className="composer">
      {autocomplete && (
        <div className="autocomplete" role="listbox">
          <div className="autocomplete-head">补全</div>
          {autocomplete.items.map((item, index) => (
            <button
              key={`${item.kind}:${item.name}`}
              type="button"
              className={`autocomplete-item ${index === autocomplete.index ? 'active' : ''}`}
              onMouseDown={(event) => {
                event.preventDefault()
                applyCandidate(item)
              }}
            >
              <span className="autocomplete-name">{item.name}</span>
              <span className={`autocomplete-kind kind-${item.kind}`}>{item.kind}</span>
            </button>
          ))}
        </div>
      )}

      <div className="composer-shell">
        <span className="composer-prompt">›</span>
        <div className="composer-input-area">
          <pre ref={overlayRef} className="composer-highlight" aria-hidden="true">
            {segments.map((segment, index) => (
              <span key={index} className={`tok-${segment.type}`}>
                {segment.text}
              </span>
            ))}
            {value.endsWith('\n') ? '\n' : null}
          </pre>
          <textarea
            ref={textareaRef}
            value={value}
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
            placeholder="输入表达式，例如 1/3 + 1/6、diff(x^3*sin(x), x) 或 solve(x^2 - 2x - 3, x)"
            onChange={(event) => handleChange(event.target.value)}
            onKeyDown={handleKeyDown}
            onClick={(event) =>
              updateAutocomplete(event.currentTarget.value, event.currentTarget.selectionStart)
            }
            onScroll={(event) => {
              if (overlayRef.current) {
                overlayRef.current.scrollTop = event.currentTarget.scrollTop
                overlayRef.current.scrollLeft = event.currentTarget.scrollLeft
              }
            }}
          />
        </div>
        <button className="run-button" type="button" onClick={submit} title="计算 (Enter)">
          <PlayIcon size={16} />
          <span>计算</span>
        </button>
      </div>

      {isEmpty && examples.length > 0 && (
        <div className="example-row">
          <span className="example-label">试试</span>
          {examples.map((example) => (
            <button
              key={example}
              type="button"
              className="example-chip"
              onClick={() => {
                onExample(example)
                requestAnimationFrame(() => textareaRef.current?.focus())
              }}
            >
              {example}
            </button>
          ))}
        </div>
      )}

      <div className="composer-hint">
        <span>
          <kbd>Enter</kbd> 计算
        </span>
        <span>
          <kbd>Shift</kbd> + <kbd>Enter</kbd> 换行
        </span>
        <span>
          <kbd>↑</kbd> <kbd>↓</kbd> 历史
        </span>
        <span className="hint-accent">精确优先 · 大数 · 复数 · 符号运算</span>
      </div>
    </div>
  )
})
