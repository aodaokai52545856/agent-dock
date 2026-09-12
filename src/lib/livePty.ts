import { pendingSessionId } from './liveBind.ts'
import type { FocusedSession, LivePtyInfo, ToolId } from './types'

export type LivePtyGroup = {
  projectId: string
  projectName: string
  items: LivePtyInfo[]
}

export function isLivePty(item: LivePtyInfo) {
  return item.alive !== false
}

export function isDshWeb(item: LivePtyInfo | null | undefined) {
  if (!item || item.toolId !== 'dsh') return false
  return item.kind !== 'pty'
}

export function liveOfProject(live: LivePtyInfo[], projectId: string) {
  return live.filter((item) => item.projectId === projectId && isLivePty(item))
}

export function pickPtyForProject(
  live: LivePtyInfo[],
  projectId: string,
  preferredId?: string | null
): LivePtyInfo | null {
  const ofProject = liveOfProject(live, projectId)
  if (!ofProject.length) return null
  if (preferredId) {
    const preferred = ofProject.find((item) => item.ptyId === preferredId)
    if (preferred) return preferred
  }
  return ofProject[0] ?? null
}

export function isCurrentSession(
  sessionId: string,
  toolId: ToolId,
  opts: {
    activePtyId: string
    focused: FocusedSession | null
    live: LivePtyInfo | null
  }
) {
  if (opts.focused?.sessionId === sessionId && opts.focused.toolId === toolId) return true
  if (opts.live && opts.activePtyId) return opts.live.ptyId === opts.activePtyId
  if (opts.activePtyId) return false
  return false
}

export function focusedFromLive(item: LivePtyInfo): FocusedSession | null {
  if (item.sessionId) {
    return {
      toolId: item.toolId,
      sessionId: item.sessionId,
      title: item.title
    }
  }
  return {
    toolId: item.toolId,
    sessionId: pendingSessionId(item.ptyId),
    title: item.title?.trim() || '新会话'
  }
}

export function groupLiveByProject(
  live: LivePtyInfo[],
  projects: { id: string; name: string }[]
): LivePtyGroup[] {
  const names = new Map(projects.map((project) => [project.id, project.name]))
  const order: string[] = []
  const buckets = new Map<string, LivePtyInfo[]>()
  for (const item of live) {
    if (!isLivePty(item)) continue
    let bucket = buckets.get(item.projectId)
    if (!bucket) {
      bucket = []
      buckets.set(item.projectId, bucket)
      order.push(item.projectId)
    }
    bucket.push(item)
  }
  return order.map((projectId) => ({
    projectId,
    projectName: names.get(projectId) ?? projectId,
    items: buckets.get(projectId) ?? []
  }))
}

export function rememberPty(
  lastByProject: Record<string, string>,
  projectId: string,
  ptyId: string
) {
  if (!projectId || !ptyId) return lastByProject
  lastByProject[projectId] = ptyId
  return lastByProject
}

export function forgetPty(lastByProject: Record<string, string>, ptyId: string) {
  for (const [projectId, lastId] of Object.entries(lastByProject)) {
    if (lastId === ptyId) delete lastByProject[projectId]
  }
  return lastByProject
}

export function projectSwitchView(
  live: LivePtyInfo[],
  fromProjectId: string,
  toProjectId: string,
  activePtyId: string,
  lastByProject: Record<string, string>
): { live: LivePtyInfo[]; lastByProject: Record<string, string>; activePtyId: string } {
  const last = { ...lastByProject }
  if (fromProjectId && activePtyId) last[fromProjectId] = activePtyId
  const next = pickPtyForProject(live, toProjectId, last[toProjectId])
  return {
    live,
    lastByProject: last,
    activePtyId: next?.ptyId ?? ''
  }
}

export function fallbackPtyAfterExit(
  live: LivePtyInfo[],
  deadPtyId: string,
  deadProjectId?: string | null,
  activePtyId?: string | null
): LivePtyInfo | null {
  if (activePtyId && activePtyId !== deadPtyId) {
    return live.find((item) => item.ptyId === activePtyId && isLivePty(item)) ?? null
  }
  if (deadProjectId) return pickPtyForProject(live, deadProjectId)
  return null
}

export function toolTint(toolId: ToolId) {
  if (toolId === 'opencode') return 'var(--ad-opencode)'
  if (toolId === 'kimi') return 'var(--ad-kimi)'
  if (toolId === 'claude') return 'var(--ad-claude)'
  if (toolId === 'pi') return 'var(--ad-pi)'
  if (toolId === 'dsh') return 'var(--ad-dsh)'
  return 'var(--ad-grok)'
}
