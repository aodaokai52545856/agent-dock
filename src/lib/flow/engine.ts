import { envelopeFromOutcome, initialEnvelope, transformEnvelope } from './envelope.ts'
import { makeId } from './ids.ts'
import {
  FLOW_END,
  type AdapterOutcome,
  type EngineResult,
  type Envelope,
  type FlowDef,
  type FlowEdge,
  type FlowRun,
  type RunStep
} from './types.ts'

function cloneRun(run: FlowRun): FlowRun {
  return {
    ...run,
    steps: run.steps.map((step) => ({ ...step, envelope: step.envelope ? { ...step.envelope } : undefined })),
    lastEnvelope: run.lastEnvelope ? { ...run.lastEnvelope } : null,
    pendingHandoff: run.pendingHandoff ? { ...run.pendingHandoff } : null
  }
}

function findNode(flow: FlowDef, id: string) {
  return flow.nodes.find((item) => item.id === id) ?? null
}

function outgoing(flow: FlowDef, nodeId: string): FlowEdge | null {
  return flow.edges.find((edge) => edge.from === nodeId) ?? null
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
    stepCount: 0,
    lastError: '',
    cursorAgentId: '',
    git: null,
    createdAt: Date.now()
  }
}

export function startRun(flow: FlowDef, projectId: string, task: string, nodeId?: string): EngineResult {
  const run = createRun(projectId, flow, task)
  const first = (nodeId && findNode(flow, nodeId)) || flow.nodes[0]
  if (!first) return fail(run, '流程里还没有节点')
  const envelope = initialEnvelope(run, first)
  run.lastEnvelope = envelope
  run.status = 'running'
  run.currentNodeId = first.id
  run.stepCount = 1
  markWorking(run, first.id, envelope)
  return { run, event: { type: 'execute', nodeId: first.id, envelope, send: true, verbatim: false } }
}

function goTo(flow: FlowDef, run: FlowRun, edge: FlowEdge, envelope: Envelope, toId: string, mode = edge.mode): EngineResult {
  if (!toId || toId === FLOW_END) return complete(run)
  const nextNode = findNode(flow, toId)
  if (!nextNode) return fail(run, '找不到下一节点')
  const nextEnvelope = transformEnvelope(edge, envelope, flow, toId)
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
  const edge = outgoing(flow, run.currentNodeId)
  if (!edge) return complete(run)

  const verdict = envelope.verdict ?? 'unknown'
  if (edge.gate === 'passFail') {
    if (verdict === 'pass') {
      return goTo(flow, run, edge, envelope, edge.to, 'auto')
    }
    if (verdict === 'unknown') {
      const target = edge.to && edge.to !== FLOW_END ? edge.to : edge.backTo || FLOW_END
      const nextEnvelope = transformEnvelope(edge, envelope, flow, target)
      run.status = 'waiting'
      run.pendingHandoff = nextEnvelope
      const last = run.steps[run.steps.length - 1]
      if (last) last.status = 'input-required'
      run.lastError = '官方审查结论是文本，未能读出 PASS/FAIL。请人工标记或手动桥接。'
      return { run, event: { type: 'waitManual', envelope: nextEnvelope } }
    }
    const maxRetries = flow.maxRetries || 0
    if (run.retryCount >= maxRetries) {
      return fail(run, `已返工 ${run.retryCount} 次仍未通过，下一片保持锁定`)
    }
    if (edge.backTo) {
      run.retryCount += 1
      return goTo(flow, run, { ...edge, transform: 'roleWrap' }, envelope, edge.backTo, 'auto')
    }
    return fail(run, '审查未通过')
  }

  return goTo(flow, run, edge, envelope, edge.to)
}

export function afterOutcome(flow: FlowDef, run: FlowRun, outcome: AdapterOutcome): EngineResult {
  const next = cloneRun(run)
  const node = findNode(flow, next.currentNodeId)
  const step = next.steps[next.steps.length - 1]
  if (!node) return fail(next, '当前节点不存在')

  if (outcome.cursorAgentId) next.cursorAgentId = outcome.cursorAgentId

  if (!outcome.ok) {
    if (step) {
      step.status = 'failed'
      step.error = outcome.error || '节点失败'
      step.endedAt = Date.now()
    }
    return fail(next, outcome.error || '节点失败')
  }

  const envelope = envelopeFromOutcome(next, node, outcome)
  next.lastEnvelope = envelope
  next.lastError = ''
  if (step) {
    step.status = 'completed'
    step.endedAt = Date.now()
    step.envelope = envelope
  }
  return followEdge(flow, next, envelope)
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

export function applyManualVerdict(flow: FlowDef, run: FlowRun, verdict: 'pass' | 'fail'): EngineResult {
  const next = cloneRun(run)
  if (!next.lastEnvelope) return fail(next, '还没有可标记的结论')
  next.lastEnvelope = { ...next.lastEnvelope, verdict }
  const step = next.steps[next.steps.length - 1]
  if (step?.envelope) step.envelope = next.lastEnvelope
  next.lastError = ''
  return followEdge(flow, next, next.lastEnvelope)
}

export function isRunBusy(run: FlowRun | null | undefined) {
  return run?.status === 'running'
}
