import assert from 'node:assert/strict'
import { afterOutcome, applyManualVerdict, resumeManual, startRun } from './engine.ts'
import { ROLE_CONTRACTS } from './roles.ts'
import { FLOW_END, type FlowDef } from './types.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

function chain(mode: 'auto' | 'manual', gate: 'none' | 'passFail' = 'none'): FlowDef {
  return {
    id: 'f1',
    name: 't',
    nodes: [
      {
        id: 'n-a',
        title: '审查者',
        role: 'reviewer',
        roleContract: ROLE_CONTRACTS.reviewer,
        channel: {
          kind: 'codexApp',
          threadIds: [],
          targetKind: 'uncommittedChanges',
          commitSha: '',
          baseBranch: 'main',
          customInstructions: ''
        }
      },
      {
        id: 'n-b',
        title: '开发者',
        role: 'developer',
        roleContract: ROLE_CONTRACTS.developer,
        channel: { kind: 'pty', toolId: 'grokbuild', sessionId: 's1', ptyId: 'p1' }
      }
    ],
    edges: [
      {
        id: 'e1',
        from: 'n-a',
        to: 'n-b',
        mode,
        transform: 'roleWrap',
        gate,
        backTo: gate === 'passFail' ? 'n-a' : undefined
      }
    ],
    maxRetries: 2,
    maxSteps: 12,
    slice: 1,
    draftTask: '修闸门'
  }
}

test('startRun executes the first node', () => {
  const flow = chain('auto')
  const started = startRun(flow, 'proj', '修闸门')
  assert.equal(started.event.type, 'execute')
  if (started.event.type !== 'execute') return
  assert.equal(started.event.nodeId, 'n-a')
  assert.equal(started.run.status, 'running')
  assert.equal(started.run.task, '修闸门')
})

test('startRun can begin at a later node', () => {
  const flow = chain('manual')
  const started = startRun(flow, 'proj', '修闸门', 'n-b')
  assert.equal(started.event.type, 'execute')
  if (started.event.type !== 'execute') return
  assert.equal(started.event.nodeId, 'n-b')
})

test('auto edge advances to the next execute', () => {
  const flow = chain('auto')
  const started = startRun(flow, 'proj', '修闸门')
  const next = afterOutcome(flow, started.run, { ok: true, text: '审查完成\nVERDICT: PASS', verdict: 'pass' })
  assert.equal(next.event.type, 'execute')
  if (next.event.type !== 'execute') return
  assert.equal(next.event.nodeId, 'n-b')
  assert.equal(next.event.send, true)
  assert.equal(next.run.currentNodeId, 'n-b')
})

test('manual edge stops at waitManual', () => {
  const flow = chain('manual')
  const started = startRun(flow, 'proj', '修闸门')
  const next = afterOutcome(flow, started.run, { ok: true, text: '请开发者返工', verdict: 'fail' })
  assert.equal(next.event.type, 'waitManual')
  assert.equal(next.run.status, 'waiting')
  assert.equal(next.run.pendingHandoff?.toNode, 'n-b')
})

test('resumeManual executes the target node', () => {
  const flow = chain('manual')
  const started = startRun(flow, 'proj', '修闸门')
  const waiting = afterOutcome(flow, started.run, { ok: true, text: '意见', verdict: 'fail' })
  assert.equal(waiting.event.type, 'waitManual')
  if (waiting.event.type !== 'waitManual') return
  const resumed = resumeManual(flow, waiting.run, waiting.event.envelope, false)
  assert.equal(resumed.event.type, 'execute')
  if (resumed.event.type !== 'execute') return
  assert.equal(resumed.event.nodeId, 'n-b')
  assert.equal(resumed.event.send, false)
  assert.equal(resumed.run.pendingHandoff, null)
})

test('passFail fail with backTo re-executes the source', () => {
  const flow: FlowDef = {
    ...chain('auto', 'passFail'),
    nodes: [
      {
        id: 'n-dev',
        title: '开发者',
        role: 'developer',
        roleContract: ROLE_CONTRACTS.developer,
        channel: { kind: 'cursorSdk', agentId: '' }
      },
      {
        id: 'n-rev',
        title: '审查者',
        role: 'reviewer',
        roleContract: ROLE_CONTRACTS.reviewer,
        channel: {
          kind: 'codexApp',
          threadIds: [],
          targetKind: 'uncommittedChanges',
          commitSha: '',
          baseBranch: 'main',
          customInstructions: ''
        }
      }
    ],
    edges: [
      {
        id: 'e-dev',
        from: 'n-dev',
        to: 'n-rev',
        mode: 'auto',
        transform: 'roleWrap',
        gate: 'none'
      },
      {
        id: 'e-rev',
        from: 'n-rev',
        to: FLOW_END,
        mode: 'auto',
        transform: 'roleWrap',
        gate: 'passFail',
        backTo: 'n-dev'
      }
    ]
  }
  let state = startRun(flow, 'proj', '修闸门')
  state = afterOutcome(flow, state.run, { ok: true, text: '已改完' })
  assert.equal(state.event.type, 'execute')
  state = afterOutcome(flow, state.run, { ok: true, text: '缺测试\nVERDICT: FAIL', verdict: 'fail' })
  assert.equal(state.event.type, 'execute')
  if (state.event.type !== 'execute') return
  assert.equal(state.event.nodeId, 'n-dev')
  assert.equal(state.run.retryCount, 1)
})

test('passFail pass completes the run', () => {
  const flow: FlowDef = {
    ...chain('auto'),
    nodes: [chain('auto').nodes[0]],
    edges: [
      {
        id: 'e-end',
        from: 'n-a',
        to: FLOW_END,
        mode: 'auto',
        transform: 'roleWrap',
        gate: 'passFail'
      }
    ]
  }
  const started = startRun(flow, 'proj', '修闸门')
  const next = afterOutcome(flow, started.run, { ok: true, text: 'VERDICT: PASS', verdict: 'pass' })
  assert.equal(next.event.type, 'complete')
  assert.equal(next.run.status, 'completed')
})

test('unknown verdict fail-opens to manual', () => {
  const flow: FlowDef = {
    ...chain('auto'),
    nodes: [chain('auto').nodes[0]],
    edges: [
      {
        id: 'e-end',
        from: 'n-a',
        to: FLOW_END,
        mode: 'auto',
        transform: 'roleWrap',
        gate: 'passFail',
        backTo: 'n-a'
      }
    ]
  }
  const started = startRun(flow, 'proj', '修闸门')
  const next = afterOutcome(flow, started.run, { ok: true, text: '看起来还行', verdict: 'unknown' })
  assert.equal(next.event.type, 'waitManual')
  assert.equal(next.run.status, 'waiting')
  assert.match(next.run.lastError, /未能读出/)
})

test('manual pass after unknown follows the pass path', () => {
  const flow: FlowDef = {
    ...chain('auto'),
    nodes: [chain('auto').nodes[0]],
    edges: [
      {
        id: 'e-end',
        from: 'n-a',
        to: FLOW_END,
        mode: 'auto',
        transform: 'roleWrap',
        gate: 'passFail'
      }
    ]
  }
  const started = startRun(flow, 'proj', '修闸门')
  const waiting = afterOutcome(flow, started.run, { ok: true, text: '散文', verdict: 'unknown' })
  const marked = applyManualVerdict(flow, waiting.run, 'pass')
  assert.equal(marked.event.type, 'complete')
})

test('retry cap fails the run', () => {
  const flow: FlowDef = {
    ...chain('auto', 'passFail'),
    maxRetries: 0,
    nodes: [chain('auto').nodes[0]],
    edges: [
      {
        id: 'e-end',
        from: 'n-a',
        to: FLOW_END,
        mode: 'auto',
        transform: 'roleWrap',
        gate: 'passFail',
        backTo: 'n-a'
      }
    ]
  }
  const started = startRun(flow, 'proj', '修闸门')
  const next = afterOutcome(flow, started.run, { ok: true, text: 'VERDICT: FAIL', verdict: 'fail' })
  assert.equal(next.event.type, 'fail')
  assert.equal(next.run.status, 'failed')
})

test('adapter error fails the run', () => {
  const flow = chain('auto')
  const started = startRun(flow, 'proj', '修闸门')
  const next = afterOutcome(flow, started.run, { ok: false, text: '', error: 'Codex 断开' })
  assert.equal(next.event.type, 'fail')
  assert.equal(next.run.lastError, 'Codex 断开')
})

test('max steps melts the loop', () => {
  const flow = { ...chain('auto'), maxSteps: 1 }
  const started = startRun(flow, 'proj', '修闸门')
  const next = afterOutcome(flow, started.run, { ok: true, text: 'ok', verdict: 'pass' })
  assert.equal(next.event.type, 'fail')
  assert.match(next.run.lastError, /上限/)
})
