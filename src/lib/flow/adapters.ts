import * as api from '../api.ts'
import { isPtyBusy } from '../livePulse.ts'
import { wrapBracketedPaste } from '../sessionCite.ts'
import { combineVerdicts } from '../reviewVerdict.ts'
import type { LivePtyInfo, SessionTurn, ToolId } from '../types.ts'
import { channelSupportsAuto, preparePtyRun, ptyWaitTimeoutError } from './channels.ts'
import { promptFromEnvelope, renderEnvelope } from './envelope.ts'
import { detectNewAssistantTurn, turnIds } from './ptyWait.ts'
import { PTY_WAIT_MS_DEFAULT, type AdapterOutcome, type Envelope, type FlowNode } from './types.ts'

export type AdapterContext = {
  projectId: string
  send: boolean
  verbatim?: boolean
  live: LivePtyInfo[]
  ptyDataAt: Record<string, number>
  cursorApiKey: string
  isCurrent: () => boolean
  now?: () => number
  sleep?: (ms: number) => Promise<void>
  listTurns?: (projectId: string, toolId: ToolId, sessionId: string) => Promise<SessionTurn[]>
}

const sleep = (ms: number) => new Promise<void>((resolve) => window.setTimeout(resolve, ms))

export async function runChannel(node: FlowNode, envelope: Envelope, ctx: AdapterContext): Promise<AdapterOutcome> {
  if (node.channel.kind === 'human') {
    const text = envelope.done.trim() || envelope.task.trim() || '已确认'
    return { ok: true, text }
  }
  if (node.channel.kind === 'cursorSdk') return runCursor(node, envelope, ctx)
  if (node.channel.kind === 'codexApp') return runCodex(node, envelope, ctx)
  return runPty(node, envelope, ctx)
}

async function runCursor(node: FlowNode, envelope: Envelope, ctx: AdapterContext): Promise<AdapterOutcome> {
  const prompt = promptFromEnvelope(node, envelope)
  if (!api.isTauri) {
    await (ctx.sleep ?? sleep)(300)
    return { ok: true, text: '预览：已完成本片开发。', cursorAgentId: node.channel.kind === 'cursorSdk' ? node.channel.agentId || 'preview-agent' : 'preview-agent' }
  }
  if (!ctx.cursorApiKey.trim()) {
    return { ok: false, text: '', error: '编排自动开发需要 Cursor API Key，请打开设置填写' }
  }
  try {
    const agentId = node.channel.kind === 'cursorSdk' ? node.channel.agentId : ''
    const result = await api.cursorDevRun({
      projectId: ctx.projectId,
      prompt,
      agentId: agentId || null
    })
    if (!result.ok) {
      return { ok: false, text: result.text || '', error: result.text || `Cursor 开发失败：${result.status}` }
    }
    return { ok: true, text: result.text || '已完成本片开发。', cursorAgentId: result.agentId }
  } catch (err) {
    return { ok: false, text: '', error: err instanceof Error ? err.message : String(err) }
  }
}

async function runCodex(node: FlowNode, envelope: Envelope, ctx: AdapterContext): Promise<AdapterOutcome> {
  if (node.channel.kind !== 'codexApp') return { ok: false, text: '', error: '不是 Codex 节点' }
  const extra = [node.channel.customInstructions, envelope.task, envelope.done].filter((item) => item?.trim()).join('\n\n')
  if (!api.isTauri) {
    await (ctx.sleep ?? sleep)(400)
    const fail = extra.includes('故意失败')
    const text = fail ? '预览：缺测试。\nVERDICT: FAIL' : '预览闭环审查通过。\nVERDICT: PASS'
    return { ok: true, text, verdict: fail ? 'fail' : 'pass' }
  }
  try {
    const result = await api.startCodexReview({
      projectId: ctx.projectId,
      threadIds: [...node.channel.threadIds],
      targetKind: node.channel.targetKind,
      commitSha: node.channel.commitSha,
      baseBranch: node.channel.baseBranch,
      customInstructions: extra
    })
    const text = result.reviews.map((item) => item.text).join('\n\n')
    const verdict = result.verdict || combineVerdicts(result.reviews.map((item) => item.verdict))
    return { ok: true, text, verdict }
  } catch (err) {
    return { ok: false, text: '', error: err instanceof Error ? err.message : String(err) }
  }
}

async function runPty(node: FlowNode, envelope: Envelope, ctx: AdapterContext): Promise<AdapterOutcome> {
  if (node.channel.kind !== 'pty') return { ok: false, text: '', error: '不是窗口节点' }
  const ready = preparePtyRun(node, ctx.live, ctx.projectId, { send: ctx.send })
  if (!ready.ok) return { ok: false, text: '', error: ready.error }
  const rendered = renderEnvelope(envelope, {
    transform: ctx.verbatim ? 'verbatim' : 'roleWrap',
    contract: node.roleContract
  })
  if (!api.isTauri) {
    await (ctx.sleep ?? sleep)(300)
    return { ok: true, text: '预览：窗口已收到接力。' }
  }
  const { live, waitForTurn, sessionId } = ready
  const now = ctx.now ?? Date.now
  if (isPtyBusy(ctx.ptyDataAt[live.ptyId], now())) {
    await (ctx.sleep ?? sleep)(2800)
    if (!ctx.isCurrent()) return { ok: false, text: '', error: '已取消' }
  }
  let before: string[] = []
  if (waitForTurn && sessionId) {
    try {
      const listed = await (ctx.listTurns
        ? ctx.listTurns(ctx.projectId, live.toolId, sessionId)
        : api.listSessionTurns(ctx.projectId, live.toolId, sessionId))
      before = turnIds(listed)
    } catch {
      before = []
    }
  }
  const payload = ctx.send ? wrapBracketedPaste(rendered) + '\r' : wrapBracketedPaste(rendered)
  try {
    await api.ptyWrite(live.ptyId, payload)
  } catch (err) {
    return { ok: false, text: '', error: err instanceof Error ? err.message : String(err) }
  }
  if (!waitForTurn || !sessionId) {
    return { ok: true, text: ctx.send ? '已写入并发送' : '已写入目标终端，未发送' }
  }
  if (!channelSupportsAuto(node.channel) && ctx.send) {
    return { ok: true, text: '已发送。该通道还不能自动读回合，请在窗口里确认。' }
  }
  const deadline = now() + PTY_WAIT_MS_DEFAULT
  const wait = ctx.sleep ?? sleep
  while (now() < deadline) {
    if (!ctx.isCurrent()) return { ok: false, text: '', error: '已取消' }
    await wait(1500)
    if (!ctx.isCurrent()) return { ok: false, text: '', error: '已取消' }
    try {
      const listed = await (ctx.listTurns
        ? ctx.listTurns(ctx.projectId, live.toolId, sessionId)
        : api.listSessionTurns(ctx.projectId, live.toolId, sessionId))
      const found = detectNewAssistantTurn(before, listed)
      if (found) return { ok: true, text: found.text }
    } catch {
      /* keep polling */
    }
  }
  return { ok: false, text: '', error: ptyWaitTimeoutError(live) }
}
