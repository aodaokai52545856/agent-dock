import { emptyEnvelope, envelopeFromOutcome, initialEnvelope, transformEnvelope } from './envelope.ts'
import { EXIT_END, EXIT_HUMAN, passSkipped, pickExit, readConclusion } from './exits.ts'
import { makeId } from './ids.ts'
import { attemptOf } from './taskRun.ts'
import {
  FLOW_END,
  FLOW_START,
  type AdapterOutcome,
  type Conclusion,
  type EngineResult,
  type Envelope,
  type FlowDef,
  type FlowEdge,
  type FlowNode,
  type FlowRun,
  type LessonDraft,
  type LessonRecord,
  type NodeExit,
  type RunStep
} from './types.ts'

function cloneRun(run: FlowRun): FlowRun {
  return {
    ...run,
    steps: run.steps.map((step) => ({ ...step, envelope: step.envelope ? { ...step.envelope } : undefined })),
    lastEnvelope: run.lastEnvelope ? { ...run.lastEnvelope } : null,
    pendingHandoff: run.pendingHandoff ? { ...run.pendingHandoff } : null,
    loopCounts: { ...(run.loopCounts || {}) }
  }
}

function findNode(flow: FlowDef, id: string) {
  return flow.nodes.find((item) => item.id === id) ?? null
}

export function expectsVerdict(flow: FlowDef, nodeId: string) {
  if (findNode(flow, nodeId)?.exits?.length) return true
  return flow.edges.some((edge) => edge.from === nodeId && edge.gate === 'passFail')
}

export function entryNode(flow: FlowDef, nodeId?: string) {
  if (nodeId) return findNode(flow, nodeId)
  const fromStart = flow.edges.find((edge) => edge.from === FLOW_START)
  if (fromStart?.to && fromStart.to !== FLOW_END) return findNode(flow, fromStart.to)
  return flow.nodes[0] ?? null
}

function outgoingAll(flow: FlowDef, nodeId: string) {
  return flow.edges.filter((edge) => edge.from === nodeId)
}

function takeLoop(run: FlowRun, edge: FlowEdge) {
  const cap = Math.min(20, Math.max(1, edge.maxLoops || 3))
  const used = run.loopCounts[edge.id] || 0
  if (used >= cap) return false
  run.loopCounts[edge.id] = used + 1
  return true
}

function fail(run: FlowRun, error: string): EngineResult {
  run.status = 'failed'
  run.lastError = error
  run.pendingHandoff = null
  const current = run.steps[run.steps.length - 1]
  if (current && current.status === 'working') {
    current.status = 'failed'
    current.error = error
    current.endedAt = Date.now()
  }
  return { run, event: { type: 'fail', error } }
}

function complete(run: FlowRun): EngineResult {
  run.status = 'completed'
  run.lastError = ''
  run.pendingHandoff = null
  return { run, event: { type: 'complete' } }
}

function markWorking(run: FlowRun, nodeId: string, envelope: Envelope): RunStep {
  const step: RunStep = {
    id: makeId('step'),
    nodeId,
    status: 'working',
    startedAt: Date.now(),
    envelope
  }
  run.steps.push(step)
  return step
}

function tooManySteps(flow: FlowDef, run: FlowRun) {
  return run.stepCount >= (flow.maxSteps || 12)
}

export function createRun(projectId: string, flow: FlowDef, task: string): FlowRun {
  const first = flow.nodes[0]
  return {
    id: makeId('run'),
    projectId,
    flowId: flow.id,
    status: 'idle',
    currentNodeId: first?.id || '',
    task: task.trim(),
    steps: [],
    lastEnvelope: null,
    pendingHandoff: null,
    retryCount: 0,
    loopCounts: {},
    stepCount: 0,
    lastError: '',
    cursorAgentId: '',
    git: null,
    createdAt: Date.now()
  }
}

function skipOf(flow: FlowDef, run: FlowRun) {
  return (run.profile && flow.profiles?.[run.profile]?.skip) || []
}

export function startRun(flow: FlowDef, projectId: string, task: string, nodeId?: string, profile?: string): EngineResult {
  const run = createRun(projectId, flow, task)
  run.profile = profile
  const named = entryNode(flow, nodeId)
  if (!named) return fail(run, '流程里还没有节点，或开始还没连到第一个角色')
  // Naming a node is a deliberate rerun of it, so the skip list does not apply.
  const first = nodeId ? named : findNode(flow, passSkipped(flow, skipOf(flow, run), named.id))
  if (!first) return fail(run, '这个 profile 没有可以开始的节点')
  const envelope = initialEnvelope(run, first)
  run.lastEnvelope = envelope
  run.status = 'running'
  run.currentNodeId = first.id
  run.stepCount = 1
  markWorking(run, first.id, envelope)
  return { run, event: { type: 'execute', nodeId: first.id, envelope, send: true, verbatim: false } }
}

function goTo(flow: FlowDef, run: FlowRun, envelope: Envelope, toId: string, mode: FlowEdge['mode'] = 'auto'): EngineResult {
  if (!toId || toId === FLOW_END) return complete(run)
  const nextNode = findNode(flow, toId)
  if (!nextNode) return fail(run, '找不到下一节点')
  const nextEnvelope = transformEnvelope(envelope, flow, toId)
  if (mode === 'manual') {
    run.status = 'waiting'
    run.pendingHandoff = nextEnvelope
    const last = run.steps[run.steps.length - 1]
    if (last) last.status = 'input-required'
    return { run, event: { type: 'waitManual', envelope: nextEnvelope } }
  }
  if (tooManySteps(flow, run)) return fail(run, `已达 ${flow.maxSteps} 步上限，已熔断`)
  run.currentNodeId = toId
  run.status = 'running'
  run.pendingHandoff = null
  run.lastEnvelope = nextEnvelope
  run.stepCount += 1
  markWorking(run, toId, nextEnvelope)
  return { run, event: { type: 'execute', nodeId: toId, envelope: nextEnvelope, send: true, verbatim: false } }
}

export function followEdge(flow: FlowDef, run: FlowRun, envelope: Envelope): EngineResult {
  const outs = outgoingAll(flow, run.currentNodeId)
  if (!outs.length) return complete(run)
  const loop = outs.find((edge) => edge.gate === 'loop')
  const forward = outs.find((edge) => edge.gate !== 'loop') || null
  const edge = forward || loop
  if (!edge) return complete(run)

  const verdict = envelope.verdict ?? 'unknown'
  if (forward?.gate === 'passFail') {
    if (verdict === 'pass') {
      return goTo(flow, run, envelope, forward.to)
    }
    if (verdict === 'unknown') {
      const target = loop?.to || forward.backTo || (forward.to !== FLOW_END ? forward.to : FLOW_END)
      const nextEnvelope = transformEnvelope(envelope, flow, target)
      run.status = 'waiting'
      run.pendingHandoff = nextEnvelope
      const last = run.steps[run.steps.length - 1]
      if (last) last.status = 'input-required'
      run.lastError = '官方审查结论是文本，未能读出 PASS/FAIL。请人工标记或手动桥接。'
      return { run, event: { type: 'waitManual', envelope: nextEnvelope } }
    }
    if (loop && takeLoop(run, loop)) {
      run.retryCount += 1
      return goTo(flow, run, envelope, loop.to)
    }
    const maxRetries = flow.maxRetries || 0
    if (forward.backTo && run.retryCount < maxRetries) {
      run.retryCount += 1
      return goTo(flow, run, envelope, forward.backTo)
    }
    if (loop) return fail(run, `已循环 ${loop.maxLoops || 3} 次仍未通过，下一片保持锁定`)
    return fail(run, run.retryCount ? `已返工 ${run.retryCount} 次仍未通过，下一片保持锁定` : '审查未通过')
  }

  if (loop && takeLoop(run, loop)) {
    return goTo(flow, run, envelope, loop.to, loop.mode)
  }
  if (forward) return goTo(flow, run, envelope, forward.to, forward.mode)
  return complete(run)
}

function withConclusion(node: FlowNode, outcome: AdapterOutcome): AdapterOutcome {
  if (!node.exits?.length) return outcome
  const conclusion = outcome.conclusion === undefined ? readConclusion([], outcome.text) : outcome.conclusion
  const verdict = !conclusion ? 'unknown' : conclusion.on === 'PASS' ? 'pass' : 'fail'
  return { ...outcome, conclusion, verdict }
}

function pauseAtExit(run: FlowRun, reason: string): EngineResult {
  run.status = 'waiting'
  run.exitPause = reason
  run.lastError = reason
  const step = run.steps[run.steps.length - 1]
  if (step) step.status = 'input-required'
  return { run, event: { type: 'waitHuman', nodeId: run.currentNodeId } }
}

function takeExit(flow: FlowDef, run: FlowRun, node: FlowNode, exit: NodeExit, envelope: Envelope): EngineResult {
  const to = passSkipped(flow, skipOf(flow, run), exit.to)
  if (to === EXIT_END) return complete(run)
  if (to === EXIT_HUMAN) return pauseAtExit(run, `节点「${node.title}」的结论是 ${[exit.on, exit.owner].filter(Boolean).join(' ')}，等人工决定`)
  const target = findNode(flow, to)
  if (!target) return fail(run, '找不到下一节点')
  const limit = target.maxAttempts ?? (flow.maxRetries || 0) + 1
  const attempts = attemptOf(run, target.id)
  if (attempts >= limit) return pauseAtExit(run, `节点「${target.title}」已尝试 ${attempts} 次，等人工决定`)
  if (attempts) run.retryCount += 1
  return goTo(flow, run, envelope, target.id)
}

function followExits(flow: FlowDef, run: FlowRun, node: FlowNode, envelope: Envelope): EngineResult {
  const conclusion = envelope.conclusion
  if (!conclusion) return pauseAtExit(run, `节点「${node.title}」的结论读不出来，等人工决定`)
  const exit = pickExit(node.exits ?? [], conclusion)
  const said = [conclusion.on, conclusion.owner].filter(Boolean).join(' ')
  if (!exit) return pauseAtExit(run, `节点「${node.title}」的结论 ${said} 没有对应的出口，等人工决定`)
  return takeExit(flow, run, node, exit, envelope)
}

// Marking a step by hand must never complete a run the exits did not lead out of.
function decideExit(flow: FlowDef, run: FlowRun, node: FlowNode, verdict: 'pass' | 'fail'): EngineResult {
  const conclusion: Conclusion = { on: verdict === 'pass' ? 'PASS' : 'FAIL' }
  const exit = pickExit(node.exits ?? [], conclusion)
  const step = run.steps[run.steps.length - 1]
  const envelope = { ...(step?.envelope ?? run.lastEnvelope ?? emptyEnvelope({ runId: run.id, toNode: node.id, toRole: node.role })), verdict, conclusion }
  run.exitPause = undefined
  run.lastError = ''
  if (!exit || exit.to === EXIT_HUMAN) {
    if (step) step.status = 'working'
    return fail(run, verdict === 'pass' ? `节点「${node.title}」没有可走的 PASS 出口` : '人工判定未通过')
  }
  if (step) {
    step.status = 'completed'
    step.envelope = envelope
  }
  run.lastEnvelope = envelope
  return takeExit(flow, run, node, exit, envelope)
}

export function afterOutcome(flow: FlowDef, run: FlowRun, outcome: AdapterOutcome): EngineResult {
  const next = cloneRun(run)
  const node = findNode(flow, next.currentNodeId)
  const step = next.steps[next.steps.length - 1]
  if (!node) return fail(next, '当前节点不存在')

  if (outcome.cursorAgentId) next.cursorAgentId = outcome.cursorAgentId
  if (step?.purpose === 'lesson') return finishLessonTurn(next, step, outcome)

  if (!outcome.ok) {
    if (step) {
      step.status = 'failed'
      step.error = outcome.error || '节点失败'
      step.endedAt = Date.now()
    }
    return fail(next, outcome.error || '节点失败')
  }

  const envelope = envelopeFromOutcome(next, node, withConclusion(node, outcome), expectsVerdict(flow, node.id))
  next.lastEnvelope = envelope
  next.lastError = ''
  if (step) {
    step.status = 'completed'
    step.endedAt = Date.now()
    step.envelope = envelope
  }
  return node.exits?.length ? followExits(flow, next, node, envelope) : followEdge(flow, next, envelope)
}

export function resumeManual(flow: FlowDef, run: FlowRun, envelope: Envelope, send: boolean): EngineResult {
  const next = cloneRun(run)
  if (!envelope.toNode || envelope.toNode === FLOW_END) {
    next.pendingHandoff = null
    next.lastEnvelope = envelope
    return complete(next)
  }
  if (tooManySteps(flow, next)) return fail(next, `已达 ${flow.maxSteps} 步上限，已熔断`)
  next.pendingHandoff = null
  next.currentNodeId = envelope.toNode
  next.status = 'running'
  next.lastEnvelope = envelope
  next.lastError = ''
  next.stepCount += 1
  markWorking(next, envelope.toNode, envelope)
  return { run: next, event: { type: 'execute', nodeId: envelope.toNode, envelope, send, verbatim: true } }
}

export function awaitHuman(run: FlowRun, nodeId: string): EngineResult {
  const next = cloneRun(run)
  next.status = 'waiting'
  const step = next.steps[next.steps.length - 1]
  if (step) step.status = 'input-required'
  return { run: next, event: { type: 'waitHuman', nodeId } }
}

export function isAwaitingHuman(flow: FlowDef | null | undefined, run: FlowRun | null | undefined) {
  if (!flow || !run || run.status !== 'waiting' || run.pendingHandoff) return false
  return Boolean(run.exitPause) || findNode(flow, run.currentNodeId)?.channel.kind === 'human'
}

function decideHuman(flow: FlowDef, run: FlowRun, verdict: 'pass' | 'fail'): EngineResult {
  if (verdict === 'fail' && !expectsVerdict(flow, run.currentNodeId)) {
    const step = run.steps[run.steps.length - 1]
    if (step) step.status = 'working'
    return fail(run, '人工审批未通过')
  }
  const envelope = run.lastEnvelope
  const text = envelope?.done.trim() || envelope?.task.trim() || run.task || '已确认'
  const conclusion = findNode(flow, run.currentNodeId)?.exits?.length ? { on: verdict === 'pass' ? 'PASS' as const : 'FAIL' as const } : undefined
  return afterOutcome(flow, run, { ok: true, text, verdict, conclusion })
}

export function applyManualVerdict(flow: FlowDef, run: FlowRun, verdict: 'pass' | 'fail'): EngineResult {
  const next = cloneRun(run)
  const node = findNode(flow, next.currentNodeId)
  if (node?.exits?.length && (next.exitPause || node.channel.kind !== 'human')) return decideExit(flow, next, node, verdict)
  if (isAwaitingHuman(flow, next)) return decideHuman(flow, next, verdict)
  if (!next.lastEnvelope) return fail(next, '还没有可标记的结论')
  next.lastEnvelope = { ...next.lastEnvelope, verdict }
  const step = next.steps[next.steps.length - 1]
  if (step?.envelope) step.envelope = next.lastEnvelope
  next.lastError = ''
  return followEdge(flow, next, next.lastEnvelope)
}

// The work already passed, so a lesson that could not be written does not undo it.
function finishLessonTurn(run: FlowRun, step: RunStep, outcome: AdapterOutcome): EngineResult {
  step.status = outcome.ok ? 'completed' : 'failed'
  step.error = outcome.ok ? undefined : outcome.error || '补写经验失败'
  step.endedAt = Date.now()
  if (outcome.ok && step.envelope) step.envelope = { ...step.envelope, done: outcome.text, artifacts: [] }
  return complete(run)
}

export function startLessonTurn(flow: FlowDef, run: FlowRun, nodeId: string): EngineResult {
  const next = cloneRun(run)
  const node = findNode(flow, nodeId)
  if (!node) return fail(next, '找不到经验作者节点')
  const envelope = emptyEnvelope({
    runId: next.id,
    causationId: next.lastEnvelope?.id,
    toNode: nodeId,
    toRole: node.role,
    task: `补写这次任务的可复用经验，不改代码。原任务：${next.task}`,
    done: `判定节点认为值得沉淀：${next.lessonWanted || ''}`
  })
  next.lessonWanted = undefined
  next.currentNodeId = nodeId
  next.status = 'running'
  next.lastEnvelope = envelope
  markWorking(next, nodeId, envelope).purpose = 'lesson'
  return { run: next, event: { type: 'execute', nodeId, envelope, send: true, verbatim: false } }
}

export function awaitLessons(run: FlowRun, lessons: LessonDraft[]): EngineResult {
  const next = cloneRun(run)
  next.status = 'waiting'
  next.pendingLessons = lessons
  return { run: next, event: { type: 'waitLessons' } }
}

export function isAwaitingLessons(run: FlowRun | null | undefined) {
  return run?.status === 'waiting' && Boolean(run.pendingLessons?.length)
}

export function finishLessons(run: FlowRun, records: LessonRecord[]): FlowRun {
  const { run: done } = complete(cloneRun(run))
  done.pendingLessons = undefined
  done.lessons = records
  return done
}

export function isRunBusy(run: FlowRun | null | undefined) {
  return run?.status === 'running'
}

export function cancelRun(run: FlowRun): FlowRun {
  const next = cloneRun(run)
  next.status = 'canceled'
  next.lastError = '已停止'
  next.pendingHandoff = null
  next.pendingLessons = undefined
  next.exitPause = undefined
  const current = next.steps[next.steps.length - 1]
  if (current && (current.status === 'working' || current.status === 'input-required')) {
    current.status = 'canceled'
    current.error = '已停止'
    current.endedAt = Date.now()
  }
  return next
}
