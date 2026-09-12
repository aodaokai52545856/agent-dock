export type TermMenuAction = 'copy' | 'paste' | 'selectAll'

export type TermMenuItem = {
  id: TermMenuAction
  label: string
  hint: string
  enabled: boolean
}

const FORBIDDEN = new Set(['书写方向', '检查', '语音输入'])

export function termMenuForbidden(label: string) {
  return FORBIDDEN.has(label)
}

export function termMenuItems(opts: { hasSelection: boolean; mac: boolean }): TermMenuItem[] {
  const mod = opts.mac ? '⌘' : 'Ctrl'
  return [
    { id: 'copy', label: '复制', hint: `${mod}+C`, enabled: opts.hasSelection },
    { id: 'paste', label: '粘贴', hint: `${mod}+V`, enabled: true },
    { id: 'selectAll', label: '全选', hint: `${mod}+A`, enabled: true }
  ]
}
