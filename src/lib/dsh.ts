import type { DshKeyBundle, DshKeyMeta } from './types'

export const DSH_PLATFORM_URL = 'https://platform.deepseek.com'
export const DSH_DOCS_URL = 'https://deepseek-harness.github.io/deepseek-harness/'
export const DSH_KEY_NAME = 'DEEPSEEK_API_KEY'
export const DSH_WEB_CONFLICT_MARK = 'DeepSeek Web 进程冲突'
export const DSH_FIXED_SESSION_ID = 'deepseek'
export const DSH_FIXED_SESSION_TITLE = 'deepseek'
export const DSH_LIVE_KEY_ID = '__live__'

export function displayDshKeys(bundle: Pick<DshKeyBundle, 'status' | 'keys'>): DshKeyMeta[] {
  if (bundle.keys.length) return bundle.keys
  if (!bundle.status.configured || !bundle.status.masked) return []
  return [
    {
      id: DSH_LIVE_KEY_ID,
      name: '当前正在使用',
      masked: bundle.status.masked,
      updatedAt: '',
      active: true,
      managed: false
    }
  ]
}

export function showDshKeyEmpty(keys: DshKeyMeta[], error: string): boolean {
  return keys.length === 0 && !error.trim()
}

export function isManagedDshKey(item: DshKeyMeta): boolean {
  return item.managed !== false && item.id !== DSH_LIVE_KEY_ID
}

export function isFixedDshSession(toolId: string, sessionId: string) {
  return toolId === 'dsh' && sessionId === DSH_FIXED_SESSION_ID
}

export function isDshWebConflict(message: string) {
  return message.includes(DSH_WEB_CONFLICT_MARK)
}

export type DshKeySource = 'env' | 'file' | 'project-env' | 'user-env' | 'none' | string

export function sourceLabel(source: DshKeySource): string {
  switch (source) {
    case 'env':
      return '启动环境变量'
    case 'file':
      return 'Harness 凭据文件'
    case 'project-env':
      return '项目 .env'
    case 'user-env':
      return '~/.dsh/.env'
    case 'none':
      return '未配置'
    default:
      return source
  }
}

export function maskSecret(value: string): string {
  const trimmed = value.trim()
  if (!trimmed) return ''
  if (trimmed.length <= 8) return '••••'
  return `${trimmed.slice(0, 4)}…${trimmed.slice(-4)}`
}
