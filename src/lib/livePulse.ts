import { isLivePty, liveOfProject } from './livePty.ts'
import type { LivePtyInfo, ToolId } from './types.ts'

export const PTY_BUSY_MS = 2800
export const PTY_BUSY_MIN_CHUNK = 12

export type LiveDotKind = 'off' | 'open' | 'busy'

export function isSignificantPtyChunk(data: string) {
  return data.length >= PTY_BUSY_MIN_CHUNK
}

export function isPtyBusy(lastDataAt: number | undefined, now: number, windowMs = PTY_BUSY_MS) {
  return Boolean(lastDataAt && now - lastDataAt < windowMs)
}

export function liveDotForPty(item: LivePtyInfo | null | undefined, lastDataAt: number | undefined, now: number): LiveDotKind {
  if (!item || !isLivePty(item)) return 'off'
  return isPtyBusy(lastDataAt, now) ? 'busy' : 'open'
}

export function sessionLiveDot(
  live: LivePtyInfo[],
  sessionId: string,
  toolId: ToolId,
  projectId: string,
  lastDataAt: Record<string, number>,
  now: number
): LiveDotKind {
  const item = live.find(
    (row) =>
      isLivePty(row) &&
      row.projectId === projectId &&
      row.toolId === toolId &&
      row.sessionId === sessionId
  )
  return liveDotForPty(item, item ? lastDataAt[item.ptyId] : undefined, now)
}

export function projectLiveDot(
  live: LivePtyInfo[],
  projectId: string,
  lastDataAt: Record<string, number>,
  now: number
): LiveDotKind {
  const items = liveOfProject(live, projectId)
  if (!items.length) return 'off'
  if (items.some((item) => isPtyBusy(lastDataAt[item.ptyId], now))) return 'busy'
  return 'open'
}

export function projectLiveCounts(
  live: LivePtyInfo[],
  projectId: string,
  lastDataAt: Record<string, number>,
  now: number
) {
  const items = liveOfProject(live, projectId)
  return {
    open: items.length,
    busy: items.filter((item) => isPtyBusy(lastDataAt[item.ptyId], now)).length
  }
}

export function liveDotTitle(kind: LiveDotKind, scope: 'session' | 'project') {
  if (kind === 'busy') return scope === 'project' ? '有会话正在执行' : '正在执行'
  if (kind === 'open') return scope === 'project' ? '有打开的会话' : '已打开'
  return ''
}
