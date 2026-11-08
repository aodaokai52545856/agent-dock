import { ompCommand, ompSnapshot, runOmpPrompt } from '../omp'
import * as api from '../api.ts'
import { isPtyBusy } from '../livePulse.ts'
import { wrapBracketedPaste } from '../sessionCite.ts'
import { combineVerdicts } from '../reviewVerdict.ts'
import type { LivePtyInfo, SessionTurn, ToolId } from '../types.ts'
import { channelSupportsAuto, inspectPty, preparePtyRun, ptyBindError, ptyWaitTimeoutError } from './channels.ts'
import { codexInstructions, promptFromEnvelope, renderEnvelope } from './envelope.ts'
import { makeId } from './ids.ts'
import { detectNewAssistantTurn, extendDeadline, turnIds } from './ptyWait.ts'
import { resetOmpSession, resetWindow } from './session.ts'
import { SCRIPT_TIMEOUT_DEFAULT_SEC } from './scripts.ts'
import { PTY_WAIT_MS_DEFAULT, type AdapterOutcome, type Envelope, type FlowNode } from './types.ts'

export type AdapterContext = {
  projectId: string
  send: boolean
  verbatim?: boolean
  // Start this step in an empty conversation; the step fails if that cannot be confirmed.
  fresh?: boolean
  onFresh?: (sessionId: string) => void | Promise<void>
  live: LivePtyInfo[]
  ptyDataAt: Record<string, number>
  cursorApiKey: string
  isCurrent: () => boolean
  onInputRequired?: (required:boolean) => void
  now?: () => number
  sleep?: (ms: number) => Promise<void>
  listTurns?: (projectId: string, toolId: ToolId, sessionId: string) => Promise<SessionTurn[]>
  switchSession?: (ptyId: string, sessionId: string) => Promise<void>
}

const sleep = (ms: number) => new Promise<void>((resolve) => window.setTimeout(resolve, ms))

export async function runChannel(node: FlowNode, envelope: Envelope, ctx: AdapterContext): Promise<AdapterOutcome> {
  if (node.channel.kind === 'human') return { ok: false, text: '', error: '人工节点由审批推进，不走通道' }
  if (node.channel.kind === 'script') return runScript(node, ctx)
  if (node.channel.kind === 'cursorSdk') return runCursor(node, envelope, ctx)
  if (node.channel.kind === 'codexApp') return runCodex(node, envelope, ctx)
  if (node.channel.kind === 'ompRpc' || node.channel.kind === 'pty' && node.channel.toolId === 'omp') {
    const channel=node.channel
    const live=ctx.live.find(l=>l.ptyId===channel.ptyId && l.projectId===ctx.projectId && l.toolId==='omp' && l.kind==='ompRpc' && l.alive)
    if(!live) return {ok:false,text:'',error:'请先绑定已打开的 OMP 会话'}
    const text=renderEnvelope(envelope,{transform:ctx.verbatim?'verbatim':'roleWrap',contract:node.roleContract})
    try {
      if(ctx.fresh){
        const reset=await resetOmpSession({snapshot:()=>ompSnapshot(live.ptyId,true),newSession:()=>ompCommand(live.ptyId,{type:'new_session'})})
        if(!reset.ok)return {ok:false,text:'',error:reset.error}
        await ctx.onFresh?.(reset.sessionId)
      }
      if(!ctx.send){await api.writeSessionText(live.ptyId,text);return {ok:true,text:'已写入 OMP 草稿，未发送',unconfirmed:true}}
      return {ok:true,text:await runOmpPrompt(live.ptyId,text,ctx.isCurrent,undefined,ctx.onInputRequired)}
    } catch(e){return {ok:false,text:'',error:String(e)}}
  }
  return runPty(node, envelope, ctx)
}

async function runScript(node: FlowNode, ctx: AdapterContext): Promise<AdapterOutcome> {
  if (node.channel.kind !== 'script') return { ok: false, text: '', error: '不是脚本节点' }
  if (!api.isTauri) return { ok: false, text: '', error: '脚本节点要在桌面端运行' }
  const { command } = node.channel
  const timeoutSecs = node.channel.timeoutSec ?? SCRIPT_TIMEOUT_DEFAULT_SEC
  const runId = makeId('script')
  const watch = window.setInterval(() => {
    if (!ctx.isCurrent()) void api.flowScriptStop(runId)
  }, 1000)
  try {
    const result = await api.flowScriptRun({ runId, projectId: ctx.projectId, command, timeoutSecs })
    if (result.canceled) return { ok: false, text: '', error: '已取消' }
    if (result.timedOut) return { ok: false, text: result.output, error: `脚本超过 ${timeoutSecs} 秒，已终止：${command}` }
    if (result.exitCode === null) return { ok: false, text: result.output, error: `脚本被系统终止：${command}` }
    const passed = result.exitCode === 0
    const text = [`脚本 ${command} 的退出码是 ${result.exitCode}`, result.output].filter(Boolean).join('\n\n')
    return { ok: true, text, verdict: passed ? 'pass' : 'fail', conclusion: { on: passed ? 'PASS' : 'FAIL' } }
  } catch (err) {
    return { ok: false, text: '', error: err instanceof Error ? err.message : String(err) }
  } finally {
    window.clearInterval(watch)
  }
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
  const extra = codexInstructions(node, envelope)
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

async function clearWindow(node: FlowNode, ctx: AdapterContext): Promise<string | null> {
  if (node.channel.kind !== 'pty' && node.channel.kind !== 'ompRpc') return '不是窗口节点'
  const found = inspectPty(node.channel, ctx.live, ctx.projectId)
  if (!found.ok) return ptyBindError(node, found.reason)
  const { live } = found
  const reset = await resetWindow(live, {
    now: ctx.now ?? Date.now,
    sleep: ctx.sleep ?? sleep,
    isCurrent: ctx.isCurrent,
    lastOutputAt: () => ctx.ptyDataAt[live.ptyId],
    listSessions: async () => (await api.listSessions(ctx.projectId, live.toolId)).sessions,
    write: (data) => api.ptyWrite(live.ptyId, data)
  })
  if (!reset.ok) return reset.error
  try {
    await ctx.switchSession?.(live.ptyId, reset.sessionId)
    await ctx.onFresh?.(reset.sessionId)
  } catch (err) {
    return err instanceof Error ? err.message : String(err)
  }
  return null
}

async function runPty(node: FlowNode, envelope: Envelope, ctx: AdapterContext): Promise<AdapterOutcome> {
  if ((node.channel.kind !== 'pty' && node.channel.kind !== 'ompRpc')) return { ok: false, text: '', error: '不是窗口节点' }
  if (ctx.fresh && api.isTauri) {
    const error = await clearWindow(node, ctx)
    if (error) return { ok: false, text: '', error }
  }
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
    return { ok: true, text: ctx.send ? '已写入并发送' : '已写入目标终端，未发送', unconfirmed: true }
  }
  if (!channelSupportsAuto(node.channel) && ctx.send) {
    return { ok: true, text: '已发送。该通道还不能自动读回合，请在窗口里确认。', unconfirmed: true }
  }
  let deadline = now() + PTY_WAIT_MS_DEFAULT
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
    deadline = extendDeadline(deadline, ctx.ptyDataAt[live.ptyId], PTY_WAIT_MS_DEFAULT)
  }
  return { ok: false, text: '', error: ptyWaitTimeoutError(live) }
}
