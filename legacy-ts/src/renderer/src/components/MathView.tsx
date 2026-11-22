import { useEffect, useRef } from 'react'
import katex from 'katex'

interface MathViewProps {
  latex: string
  display?: boolean
  className?: string
}

export function MathView({ latex, display = true, className }: MathViewProps) {
  const ref = useRef<HTMLSpanElement>(null)

  useEffect(() => {
    if (!ref.current) return
    try {
      katex.render(latex, ref.current, {
        throwOnError: false,
        displayMode: display,
        strict: false,
        trust: false,
        output: 'html'
      })
    } catch {
      ref.current.textContent = latex
    }
  }, [latex, display])

  return <span ref={ref} className={className} />
}
