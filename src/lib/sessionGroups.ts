import { TOOLS, type ToolId } from './types.ts'

export const SESSION_GROUP_COLLAPSE_KEY = 'ad-session-group-collapsed'

const TOOL_IDS = new Set(TOOLS.map((tool) => tool.id))

export function parseCollapsedGroups(raw: string | null | undefined): Set<ToolId> {
  if (!raw?.trim()) return new Set()
  try {
    const value = JSON.parse(raw)
    if (!Array.isArray(value)) return new Set()
    return new Set(value.filter((id): id is ToolId => typeof id === 'string' && TOOL_IDS.has(id as ToolId)))
  } catch {
    return new Set()
  }
}

export function serializeCollapsedGroups(ids: Iterable<ToolId>) {
  return JSON.stringify([...ids])
}

export function toggleCollapsedGroup(ids: Set<ToolId>, toolId: ToolId): Set<ToolId> {
  const next = new Set(ids)
  if (next.has(toolId)) next.delete(toolId)
  else next.add(toolId)
  return next
}

export function groupBodyHidden(collapsed: boolean, query = '') {
  if (query.trim()) return false
  return collapsed
}

export function shouldShowSessionGroupHead(groupCount: number, liveOnly = false) {
  return liveOnly || groupCount > 1
}

export function sessionLiveFilterLabel(liveOnly: boolean) {
  return liveOnly ? '已开对话' : '全部会话'
}
