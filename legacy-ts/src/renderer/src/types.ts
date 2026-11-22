import type { CalcSettings, ExecResult, WorkspaceItem } from '@core/index'

export interface ResultView {
  ok: boolean
  kind: 'expr' | 'assign' | 'funcdef' | 'error'
  name?: string
  plain?: string
  latex?: string
  approx?: string
  approxLatex?: string
  valueType?: string
  note?: string
  error?: string
  errorPos?: number
  elapsedMs: number
  suppressed?: boolean
}

export interface HistoryEntry {
  id: string
  input: string
  results: ResultView[]
  at: number
}

export interface SavedState {
  version: number
  history: HistoryEntry[]
  settings: CalcSettings
  workspaceSource?: string
}

export type WorkspaceView = WorkspaceItem

export function toView(result: ExecResult): ResultView {
  return {
    ok: result.ok,
    kind: result.kind,
    name: result.name,
    plain: result.plain,
    latex: result.latex,
    approx: result.approx,
    approxLatex: result.approxLatex,
    valueType: result.valueType,
    note: result.note,
    error: result.error,
    errorPos: result.errorPos,
    elapsedMs: result.elapsedMs,
    suppressed: result.suppressed
  }
}

declare global {
  interface Window {
    frog?: {
      loadState: () => Promise<unknown>
      saveState: (state: unknown) => Promise<void>
      exportFile: (
        name: string,
        content: string
      ) => Promise<{ canceled: boolean; path?: string }>
    }
  }
}
