import { TOOLS, toolLabel, type ToolId, type SessionRow } from './types.ts'

export const SESSION_GROUP_COLLAPSE_KEY = 'ad-session-group-collapsed'

const TOOL_IDS = new Set(TOOLS.map((tool) => tool.id))

export type SessionGroupId = ToolId | `claude:${string}`

export function compareSessionRecency(a: SessionRow, b: SessionRow) {
  return b.updatedAt - a.updatedAt || a.toolId.localeCompare(b.toolId) || a.id.localeCompare(b.id)
}

export function accountSessionGroups(toolId: ToolId, sessions: SessionRow[]) {
  if (toolId !== 'claude' || !sessions.length) {
    return [{ id: toolId as SessionGroupId, label: toolLabel(toolId), sessions }]
  }
  const buckets = new Map<string, SessionRow[]>()
  for (const session of sessions) {
    const id = session.account?.id || 'unknown'
    if (!buckets.has(id)) buckets.set(id, [])
    buckets.get(id)!.push(session)
  }
  // Account groups stay put when messages arrive, terminals open, or the default account changes.
  return [...buckets].sort(([a], [b]) =>
    a === b ? 0 : a === 'system' ? -1 : b === 'system' ? 1 : a.localeCompare(b)
  ).map(([id, rows]) => ({
    id: `claude:${id}` as SessionGroupId,
    label: `Claude · ${rows[0].account?.name || '归属未知'}`,
    sessions: [...rows].sort(compareSessionRecency)
  }))
}

export function parseCollapsedGroups(raw: string | null | undefined): Set<SessionGroupId> {
  if (!raw?.trim()) return new Set()
  try {
    const value = JSON.parse(raw)
    if (!Array.isArray(value)) return new Set()
    return new Set(value.filter((id): id is SessionGroupId => typeof id === 'string' && (
      TOOL_IDS.has(id as ToolId) || (id.startsWith('claude:') && id.length > 7 && id.length <= 200)
    )))
  } catch {
    return new Set()
  }
}

export function serializeCollapsedGroups(ids: Iterable<SessionGroupId>) {
  return JSON.stringify([...ids])
}

export function toggleCollapsedGroup(ids: Set<SessionGroupId>, toolId: SessionGroupId): Set<SessionGroupId> {
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
