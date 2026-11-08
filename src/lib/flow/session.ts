import { isPtyBusy } from '../livePulse.ts'
import type { LivePtyInfo, SessionRow } from '../types.ts'
import { FRESH_COMMANDS, nodeSession, ptyToolLabel } from './channels.ts'
import type { FlowNode, FlowRun, RunStep } from './types.ts'

const QUIET_WAIT_MS = 30_000
const NEW_SESSION_WAIT_MS = 15_000
const POLL_MS = 500

export type Reset = { ok: true; sessionId: string } | { ok: false; error: string }

export type WindowResetIo = {
  now: () => number
  sleep: (ms: number) => Promise<void>
  isCurrent: () => boolean
  lastOutputAt: () => number | undefined
  listSessions: () => Promise<Pick<SessionRow, 'id' | 'account'>[]>
  write: (data: string) => Promise<void>
}

export type OmpResetIo = {
  snapshot: () => Promise<{ settled: boolean; messages: unknown[]; info: { sessionId?: string | null } }>
  newSession: () => Promise<{ data?: { cancelled?: boolean } }>
}

// Clear once per task: a rework keeps what the node already learned, and a lesson
// turn exists to recall it. Reset success is tracked, not attempts, so a retry
// after a failed reset still starts clean.
export function startsFresh(node: FlowNode, run: FlowRun, step?: RunStep) {
  if (step?.purpose === 'lesson' || nodeSession(node) !== 'fresh') return false
  return !run.freshNodes?.includes(node.id)
}

function windowName(live: LivePtyInfo) {
  return live.title.trim() || ptyToolLabel(live.toolId)
}

function message(err: unknown) {
  return err instanceof Error ? err.message : String(err)
}

export async function resetWindow(live: LivePtyInfo, io: WindowResetIo): Promise<Reset> {
  const name = windowName(live)
  const command = FRESH_COMMANDS[live.toolId]
  if (!command) return { ok: false, error: `「${name}」不能清空上下文` }
  const quietBy = io.now() + QUIET_WAIT_MS
  while (isPtyBusy(io.lastOutputAt(), io.now())) {
    if (!io.isCurrent()) return { ok: false, error: '已取消' }
    if (io.now() >= quietBy) return { ok: false, error: `「${name}」窗口还在工作，不能清空上下文` }
    await io.sleep(POLL_MS)
  }
  const wanted = (row: Pick<SessionRow, 'account'>) => !live.account?.id || !row.account?.id || row.account.id === live.account.id
  let before: Set<string>
  try {
    before = new Set((await io.listSessions()).map((row) => row.id))
    await io.write(`${command}\r`)
  } catch (err) {
    return { ok: false, error: `清空「${name}」的上下文失败：${message(err)}` }
  }
  const deadline = io.now() + NEW_SESSION_WAIT_MS
  while (io.now() < deadline) {
    await io.sleep(POLL_MS)
    if (!io.isCurrent()) return { ok: false, error: '已取消' }
    let created: Pick<SessionRow, 'id' | 'account'>[]
    try {
      created = (await io.listSessions()).filter((row) => !before.has(row.id) && wanted(row))
    } catch {
      continue
    }
    if (created.length > 1) return { ok: false, error: `同时出现了多个新会话，分不清哪个属于「${name}」，请重跑` }
    if (created.length === 1) return { ok: true, sessionId: created[0].id }
  }
  return { ok: false, error: `「${name}」窗口没有开始新会话，提示词未发送。请确认输入框是空的再重跑` }
}

export async function resetOmpSession(io: OmpResetIo): Promise<Reset> {
  const before = await io.snapshot()
  // new_session aborts whatever the session is doing.
  if (!before.settled) return { ok: false, error: 'OMP 会话还在工作，不能清空上下文' }
  const response = await io.newSession()
  if (response.data?.cancelled) return { ok: false, error: 'OMP 取消了新建会话，提示词未发送' }
  const after = await io.snapshot()
  const sessionId = after.info.sessionId
  if (!sessionId || sessionId === before.info.sessionId || after.messages.length) {
    return { ok: false, error: 'OMP 没有开始新会话，提示词未发送' }
  }
  return { ok: true, sessionId }
}
