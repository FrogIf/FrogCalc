import { useEffect } from 'react'
import type { CalcSettings } from '@core/index'
import { CloseIcon, ExportIcon, TrashIcon } from './Icons'

interface SettingsDialogProps {
  open: boolean
  settings: CalcSettings
  historyCount: number
  onChange: (settings: Partial<CalcSettings>) => void
  onClose: () => void
  onClearWorkspace: () => void
  onClearHistory: () => void
  onExport: () => void
}

export function SettingsDialog({
  open,
  settings,
  historyCount,
  onChange,
  onClose,
  onClearWorkspace,
  onClearHistory,
  onExport
}: SettingsDialogProps) {
  useEffect(() => {
    if (!open) return
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [open, onClose])

  if (!open) return null

  return (
    <div
      className="modal-backdrop"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose()
      }}
    >
      <div className="modal" role="dialog" aria-modal="true" aria-label="设置">
        <div className="modal-head">
          <div>
            <h2>设置</h2>
            <p>调整计算与显示行为</p>
          </div>
          <button type="button" className="icon-button" onClick={onClose} title="关闭">
            <CloseIcon />
          </button>
        </div>

        <div className="modal-body">
          <section className="setting-group">
            <div className="setting-label">
              <span>近似精度</span>
              <span className="setting-value">{settings.precision} 位有效数字</span>
            </div>
            <input
              className="range"
              type="range"
              min={10}
              max={200}
              step={5}
              value={Math.min(200, settings.precision)}
              onChange={(event) => onChange({ precision: Number(event.target.value) })}
            />
            <div className="setting-inline">
              <input
                className="number-input"
                type="number"
                min={10}
                max={500}
                value={settings.precision}
                onChange={(event) => {
                  const value = Number(event.target.value)
                  if (Number.isFinite(value)) onChange({ precision: value })
                }}
              />
              <span className="setting-hint">
                精确计算不受此设置影响，只影响 N(...) 与近似显示。
              </span>
            </div>
          </section>

          <section className="setting-group">
            <div className="setting-label">
              <span>角度制</span>
            </div>
            <div className="segmented">
              <button
                type="button"
                className={settings.angleMode === 'rad' ? 'active' : ''}
                onClick={() => onChange({ angleMode: 'rad' })}
              >
                弧度 rad
              </button>
              <button
                type="button"
                className={settings.angleMode === 'deg' ? 'active' : ''}
                onClick={() => onChange({ angleMode: 'deg' })}
              >
                角度 deg
              </button>
            </div>
          </section>

          <section className="setting-group">
            <div className="setting-label">
              <span>结果显示</span>
            </div>
            <div className="segmented">
              <button
                type="button"
                className={settings.displayMode === 'exact' ? 'active' : ''}
                onClick={() => onChange({ displayMode: 'exact' })}
              >
                仅精确
              </button>
              <button
                type="button"
                className={settings.displayMode === 'both' ? 'active' : ''}
                onClick={() => onChange({ displayMode: 'both' })}
              >
                精确 + 近似
              </button>
              <button
                type="button"
                className={settings.displayMode === 'approx' ? 'active' : ''}
                onClick={() => onChange({ displayMode: 'approx' })}
              >
                近似优先
              </button>
            </div>
          </section>

          <section className="setting-group">
            <div className="setting-label">
              <span>数据</span>
              <span className="setting-value">{historyCount} 条历史</span>
            </div>
            <div className="setting-actions">
              <button
                type="button"
                className="secondary-button"
                onClick={() => {
                  if (window.confirm('确定清空工作区变量与函数吗？')) onClearWorkspace()
                }}
              >
                <TrashIcon size={15} />
                清空工作区
              </button>
              <button
                type="button"
                className="secondary-button"
                onClick={() => {
                  if (window.confirm('确定清空全部历史记录吗？')) onClearHistory()
                }}
              >
                <TrashIcon size={15} />
                清空历史
              </button>
              <button
                type="button"
                className="secondary-button"
                onClick={onExport}
                disabled={historyCount === 0}
              >
                <ExportIcon size={15} />
                导出历史
              </button>
            </div>
          </section>

          <section className="about-block">
            <div>
              <strong>FrogCalc</strong>
              <span>v0.1.0 · 精确优先的桌面计算器</span>
            </div>
            <p>
              自研词法/语法解析器、任意精度数值内核、符号代数、微积分与方程求解。
              不依赖任何第三方数学引擎。
            </p>
          </section>
        </div>
      </div>
    </div>
  )
}
