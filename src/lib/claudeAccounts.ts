import type { ClaudeAccount } from './types'

export function claudeStatus(status: ClaudeAccount['status']): string {
  return ({ ready: '本地已登录', refreshable: '待续期', signedOut: '未登录', legacy: '待迁移', broken: '需检查', conflict: '配置冲突', unverified: '待核验' })[status] || '未知状态'
}

export function canUseClaudeAccount(account: Pick<ClaudeAccount, 'status'>): boolean {
  return account.status === 'ready' || account.status === 'refreshable'
}

export function proxyLabel(proxy: string): string {
  return proxy === 'inherit' ? '跟随项目 / 应用代理' : proxy === 'off' ? '直连' : proxy
}
