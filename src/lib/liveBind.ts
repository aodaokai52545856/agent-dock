import type { LivePtyInfo, SessionRow, ToolId } from './types'

export const PENDING_SESSION_PREFIX = '__pending:'
export const LIVE_BIND_SLACK_MS = 15_000
export const NEW_SESSION_TITLE = '新会话'

export function keepLiveTitle(liveTitle: string | null | undefined, diskTitle: string) {
  const custom = liveTitle?.trim() ?? ''
  if (custom && custom !== NEW_SESSION_TITLE && custom !== diskTitle) return custom
  return diskTitle
}

export type LiveBind = {
  ptyId: string
  sessionId: string
  title: string
  toolId: ToolId
}

export type OpenTarget =
  | { action: 'switch'; ptyId: string }
  | { action: 'bind-and-switch'; ptyId: string; sessionId: string }
  | { action: 'open'; sessionId: string | null }

export function pendingSessionId(ptyId: string) {
  return `${PENDING_SESSION_PREFIX}${ptyId}`
}

export function isPendingSessionId(sessionId: string) {
  return sessionId.startsWith(PENDING_SESSION_PREFIX)
}

export function ptyIdFromPending(sessionId: string) {
  return isPendingSessionId(sessionId) ? sessionId.slice(PENDING_SESSION_PREFIX.length) : ''
}

export function liveKey(projectId: string, toolId: ToolId, sessionId: string) {
  return `${projectId}|${toolId}|${sessionId}`
}

function sameProjectTool(item: LivePtyInfo, projectId: string, toolId: ToolId) {
  return item.projectId === projectId && item.toolId === toolId && item.alive !== false
}

export function matchUnboundLiveToSessions(
  live: LivePtyInfo[],
  sessions: SessionRow[],
  projectId: string,
  slackMs = LIVE_BIND_SLACK_MS
): LiveBind[] {
  const claimed = new Set(
    live
      .filter((item) => item.projectId === projectId && item.sessionId)
      .map((item) => `${item.toolId}:${item.sessionId}`)
  )
  const usedSession = new Set<string>()
  const binds: LiveBind[] = []
  const tools: ToolId[] = ['opencode', 'grokbuild', 'kimi', 'claude', 'pi', 'dsh']

  for (const toolId of tools) {
    const unbound = live
      .filter((item) => sameProjectTool(item, projectId, toolId) && !item.sessionId)
      .sort((a, b) => (b.openedAt ?? 0) - (a.openedAt ?? 0))
    if (!unbound.length) continue

    const candidates = sessions
      .filter((row) => row.toolId === toolId && !claimed.has(`${toolId}:${row.id}`))
      .sort((a, b) => b.updatedAt - a.updatedAt)

    for (const pty of unbound) {
      const openedAt = pty.openedAt ?? 0
      if (!openedAt) continue
      const session = candidates.find(
        (row) =>
          !usedSession.has(`${toolId}:${row.id}`) && row.updatedAt >= openedAt - slackMs
      )
      if (!session) continue
      usedSession.add(`${toolId}:${session.id}`)
      binds.push({
        ptyId: pty.ptyId,
        sessionId: session.id,
        title: session.title,
        toolId
      })
    }
  }
  return binds
}

export function resolveOpenTarget(
  live: LivePtyInfo[],
  opts: {
    projectId: string
    toolId: ToolId
    sessionId: string | null
    sessionUpdatedAt?: number
  }
): OpenTarget {
  const { projectId, toolId, sessionId } = opts
  if (sessionId && isPendingSessionId(sessionId)) {
    const ptyId = ptyIdFromPending(sessionId)
    const pending = live.find((item) => item.ptyId === ptyId && item.alive !== false)
    if (pending) return { action: 'switch', ptyId }
  }

  const same = live.filter((item) => sameProjectTool(item, projectId, toolId))
  if (sessionId) {
    const exact = same.find((item) => item.sessionId === sessionId)
    if (exact) return { action: 'switch', ptyId: exact.ptyId }

    if (opts.sessionUpdatedAt != null) {
      const probe: SessionRow = {
        toolId,
        id: sessionId,
        title: sessionId,
        cwd: '',
        updatedAt: opts.sessionUpdatedAt,
        renameKind: 'overlay'
      }
      const hit = matchUnboundLiveToSessions(live, [probe], projectId).find(
        (item) => item.sessionId === sessionId
      )
      if (hit) return { action: 'bind-and-switch', ptyId: hit.ptyId, sessionId }
    }
    return { action: 'open', sessionId }
  }

  return { action: 'open', sessionId: null }
}

export function pickActivePtyForProject(
  live: LivePtyInfo[],
  projectId: string,
  currentPtyId: string
) {
  const here = live.filter((item) => item.projectId === projectId && item.alive !== false)
  if (here.some((item) => item.ptyId === currentPtyId)) return currentPtyId
  if (!here.length) return ''
  return here.reduce((best, item) =>
    (item.openedAt ?? 0) >= (best.openedAt ?? 0) ? item : best
  ).ptyId
}

export function pendingSessionRows(
  live: LivePtyInfo[],
  sessions: SessionRow[],
  projectId: string
): SessionRow[] {
  return live
    .filter((item) => item.projectId === projectId && item.alive !== false)
    .filter(
      (item) =>
        !item.sessionId ||
        !sessions.some((row) => row.toolId === item.toolId && row.id === item.sessionId)
    )
    .map((item) => ({
      toolId: item.toolId,
      id: item.sessionId || pendingSessionId(item.ptyId),
      title: item.title?.trim() || NEW_SESSION_TITLE,
      cwd: '',
      updatedAt: item.openedAt ?? Date.now(),
      renameKind: 'overlay' as const
    }))
}

export function mergeLiveFromServer(server: LivePtyInfo[], local: LivePtyInfo[]): LivePtyInfo[] {
  return server.map((item) => {
    const prev = local.find((row) => row.ptyId === item.ptyId)
    if (!prev) return item
    if (prev.sessionId && !item.sessionId) {
      return {
        ...item,
        sessionId: prev.sessionId,
        title: prev.title || item.title,
        key: prev.key || item.key,
        openedAt: item.openedAt ?? prev.openedAt
      }
    }
    return {
      ...item,
      openedAt: item.openedAt ?? prev.openedAt
    }
  })
}


