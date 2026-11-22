import { SettingsIcon } from './Icons'

interface TitleBarProps {
  precision: number
  angleMode: 'rad' | 'deg'
  onOpenSettings: () => void
}

export function TitleBar({ precision, angleMode, onOpenSettings }: TitleBarProps) {
  return (
    <header className="titlebar">
      <div className="brand">
        <div className="brand-mark">ƒ</div>
        <div className="brand-text">
          <span className="brand-name">FrogCalc</span>
          <span className="brand-sub">精确计算器 · CAS</span>
        </div>
      </div>

      <div className="titlebar-status">
        <span className="status-chip">{precision} 位</span>
        <span className="status-chip">{angleMode === 'deg' ? '角度制' : '弧度制'}</span>
        <span className="status-chip status-online">精确模式</span>
      </div>

      <div className="titlebar-actions">
        <button
          type="button"
          className="icon-button"
          onClick={onOpenSettings}
          title="设置"
        >
          <SettingsIcon size={18} />
        </button>
      </div>
    </header>
  )
}
