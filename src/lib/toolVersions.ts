import { TOOLS, type ToolId, type ToolVersionInfo } from './types.ts'

export const VERSION_IDLE = '未检测'
export const VERSION_CHECKING = '检测中'

export function idleVersionRow(toolId: ToolId, name?: string): ToolVersionInfo {
  return {
    toolId,
    name: name || TOOLS.find((tool) => tool.id === toolId)?.label || toolId,
    found: false,
    localVersion: '-',
    latestVersion: '-',
    compare: VERSION_IDLE,
    checks: []
  }
}

export function seedVersionRows(): ToolVersionInfo[] {
  return TOOLS.map((tool) => idleVersionRow(tool.id, tool.label))
}

export function markVersionChecking(row: ToolVersionInfo): ToolVersionInfo {
  return {
    ...row,
    localVersion: VERSION_CHECKING,
    latestVersion: VERSION_CHECKING,
    compare: VERSION_CHECKING,
    checks: []
  }
}

export function mergeVersionRows(next: ToolVersionInfo[]): ToolVersionInfo[] {
  return TOOLS.map((tool) => {
    return (
      next.find((row) => row.toolId === tool.id) ?? {
        toolId: tool.id,
        name: tool.label,
        found: false,
        localVersion: '查询失败',
        latestVersion: '查询失败',
        compare: '无法对比',
        checks: []
      }
    )
  })
}

export function patchVersionRow(rows: ToolVersionInfo[], next: ToolVersionInfo): ToolVersionInfo[] {
  return rows.map((row) => (row.toolId === next.toolId ? next : row))
}

export function isVersionIdle(compare: string) {
  return compare === VERSION_IDLE
}

export function isVersionChecking(compare: string) {
  return compare === VERSION_CHECKING
}
