import type { WorkspaceView } from '../types'
import { FunctionIcon, TrashIcon, VariableIcon } from './Icons'

interface SidebarProps {
  workspace: WorkspaceView[]
  onInsert: (text: string) => void
  onClearWorkspace: () => void
}

const CONSTANTS = [
  { name: 'pi', label: '圆周率' },
  { name: 'e', label: '自然常数' },
  { name: 'i', label: '虚数单位' }
]

function badgeClass(type?: string): string {
  if (!type) return 'badge'
  if (type === '整数') return 'badge badge-integer'
  if (type === '有理数') return 'badge badge-rational'
  if (type === '实数') return 'badge badge-real'
  if (type === '复数') return 'badge badge-complex'
  if (type === '函数') return 'badge badge-function'
  if (type === '解集') return 'badge badge-set'
  return 'badge badge-symbolic'
}

export function Sidebar({ workspace, onInsert, onClearWorkspace }: SidebarProps) {
  const variables = workspace.filter((item) => item.kind === 'variable')
  const functions = workspace.filter((item) => item.kind === 'function')

  return (
    <aside className="sidebar">
      <div className="sidebar-section">
        <div className="sidebar-heading">
          <span>工作区</span>
          <span className="sidebar-count">{workspace.length}</span>
        </div>

        {workspace.length === 0 ? (
          <div className="workspace-empty">
            赋值后会显示在这里。
            <br />
            例如 <code>a = 1 / 3</code>
          </div>
        ) : (
          <div className="workspace-list">
            {variables.map((item) => (
              <button
                key={item.name}
                type="button"
                className="workspace-item"
                title={item.plain}
                onClick={() => onInsert(item.name)}
              >
                <span className="workspace-icon">
                  <VariableIcon size={15} />
                </span>
                <span className="workspace-main">
                  <span className="workspace-name">{item.name}</span>
                  <span className="workspace-value">{item.plain}</span>
                </span>
                <span className={badgeClass(item.valueType)}>{item.valueType}</span>
              </button>
            ))}
            {functions.map((item) => (
              <button
                key={item.name}
                type="button"
                className="workspace-item"
                title={item.plain}
                onClick={() => onInsert(`${item.name}(`)}
              >
                <span className="workspace-icon">
                  <FunctionIcon size={15} />
                </span>
                <span className="workspace-main">
                  <span className="workspace-name">{item.signature}</span>
                  <span className="workspace-value">用户函数</span>
                </span>
                <span className={badgeClass(item.valueType)}>{item.valueType}</span>
              </button>
            ))}
          </div>
        )}

        {workspace.length > 0 && (
          <button type="button" className="sidebar-action" onClick={onClearWorkspace}>
            <TrashIcon size={14} />
            清空工作区
          </button>
        )}
      </div>

      <div className="sidebar-section">
        <div className="sidebar-heading">
          <span>常量</span>
        </div>
        <div className="constant-list">
          {CONSTANTS.map((constant) => (
            <button
              key={constant.name}
              type="button"
              className="constant-chip"
              onClick={() => onInsert(constant.name)}
            >
              <span className="constant-symbol">{constant.name}</span>
              <span className="constant-label">{constant.label}</span>
            </button>
          ))}
        </div>
      </div>

      <div className="sidebar-section sidebar-tips">
        <div className="sidebar-heading">
          <span>语法速记</span>
        </div>
        <ul>
          <li>
            <code>a = 1/3</code> 精确分数
          </li>
          <li>
            <code>f(x) = x^2 + 1</code> 定义函数
          </li>
          <li>
            <code>2x + 3(x - 1)</code> 隐式乘法
          </li>
          <li>
            <code>help("syntax")</code> 完整帮助
          </li>
        </ul>
      </div>
    </aside>
  )
}
