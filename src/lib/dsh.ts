export const DSH_PLATFORM_URL = 'https://platform.deepseek.com'
export const DSH_DOCS_URL = 'https://deepseek-harness.github.io/deepseek-harness/'
export const DSH_KEY_NAME = 'DEEPSEEK_API_KEY'
export const DSH_WEB_CONFLICT_MARK = 'DeepSeek Web 进程冲突'

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
