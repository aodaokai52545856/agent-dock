import assert from 'node:assert/strict'
import type { LivePtyInfo } from '../types.ts'
import {
  describeChannel,
  inspectPty,
  preparePtyRun,
  ptyWaitTimeoutError,
  ptyWindowLabel,
  resolvePty,
  validateFlowPtyBindings
} from './channels.ts'
import { ROLE_CONTRACTS } from './roles.ts'
import type { FlowDef, FlowNode, PtyChannel } from './types.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

function grok(partial: Partial<LivePtyInfo> & Pick<LivePtyInfo, 'ptyId' | 'title'>): LivePtyInfo {
  return {
    key: partial.key || partial.ptyId,
    projectId: partial.projectId || 'proj',
    toolId: partial.toolId || 'grokbuild',
    sessionId: partial.sessionId ?? `sess-${partial.ptyId}`,
    alive: partial.alive ?? true,
    ...partial
  }
}

function channel(partial: Partial<PtyChannel> = {}): PtyChannel {
  return {
    kind: 'pty',
    toolId: 'grokbuild',
    sessionId: '',
    ptyId: '',
    ...partial
  }
}

const a = grok({ ptyId: 'pty-a', title: '登录限流' })
const b = grok({ ptyId: 'pty-b', title: '审查意见' })

test('bound ptyId picks that Grok window among two live ones', () => {
  const live = resolvePty(channel({ ptyId: 'pty-b', sessionId: 'sess-pty-b' }), [a, b], 'proj')
  assert.equal(live?.ptyId, 'pty-b')
})

test('unbound node does not fall back to the first Grok window', () => {
  const found = inspectPty(channel(), [a, b], 'proj')
  assert.equal(found.ok, false)
  if (found.ok) return
  assert.equal(found.reason, 'unbound')
  assert.equal(resolvePty(channel(), [a, b], 'proj'), null)
})

test('dead ptyId does not fall back to another live Grok', () => {
  const dead = grok({ ptyId: 'pty-dead', title: '旧窗', alive: false })
  const found = inspectPty(channel({ ptyId: 'pty-dead', sessionId: 'sess-pty-dead' }), [dead, a], 'proj')
  assert.equal(found.ok, false)
  if (found.ok) return
  assert.equal(found.reason, 'closed')
  assert.equal(resolvePty(channel({ ptyId: 'pty-dead' }), [dead, a], 'proj'), null)
})

test('ptyId for a different tool does not resolve', () => {
  const found = inspectPty(channel({ ptyId: 'pty-a', toolId: 'kimi' }), [a, b], 'proj')
  assert.equal(found.ok, false)
  if (found.ok) return
  assert.equal(found.reason, 'mismatch')
})

test('sessionId without ptyId matches exactly one live window', () => {
  const live = resolvePty(channel({ sessionId: 'sess-pty-b' }), [a, b], 'proj')
  assert.equal(live?.ptyId, 'pty-b')
})

test('ambiguous sessionId match fails instead of picking the first', () => {
  const twin = grok({ ptyId: 'pty-b2', title: '副本', sessionId: 'sess-pty-b' })
  const found = inspectPty(channel({ sessionId: 'sess-pty-b' }), [b, twin], 'proj')
  assert.equal(found.ok, false)
  if (found.ok) return
  assert.equal(found.reason, 'ambiguous')
})

test('describeChannel uses the live window title when ptyId is bound', () => {
  const text = describeChannel(channel({ ptyId: 'pty-a', sessionId: '' }), [a, b])
  assert.equal(text, 'Grok · 登录限流')
})

test('describeChannel says unbound when ptyId is missing', () => {
  assert.equal(describeChannel(channel({ sessionId: 'sess-pty-a' })), '未绑定的 Grok 窗口')
})

test('pty window label includes a short pty id', () => {
  const label = ptyWindowLabel(grok({ ptyId: 'abcd1234', title: '登录限流' }))
  assert.match(label, /Grok/)
  assert.match(label, /登录限流/)
  assert.match(label, /1234/)
})

function twoGrokFlow(first: PtyChannel, second: PtyChannel): FlowDef {
  return {
    id: 'f1',
    name: '双 Grok',
    nodes: [
      {
        id: 'n-a',
        title: '开发者',
        role: 'developer',
        roleContract: ROLE_CONTRACTS.developer,
        channel: first
      },
      {
        id: 'n-b',
        title: '审查者',
        role: 'reviewer',
        roleContract: ROLE_CONTRACTS.reviewer,
        channel: second
      }
    ],
    edges: [
      {
        id: 'e1',
        from: 'n-a',
        to: 'n-b',
        mode: 'auto',
        transform: 'roleWrap',
        gate: 'none'
      }
    ],
    maxRetries: 2,
    maxSteps: 12,
    slice: 1,
    draftTask: '修闸门'
  }
}

test('start validation rejects two nodes bound to the same window', () => {
  const flow = twoGrokFlow(
    channel({ ptyId: 'pty-a', sessionId: 'sess-pty-a' }),
    channel({ ptyId: 'pty-a', sessionId: 'sess-pty-a' })
  )
  const error = validateFlowPtyBindings(flow, [a, b], 'proj')
  assert.match(error || '', /同一/)
})

test('start validation rejects an unbound Grok node', () => {
  const flow = twoGrokFlow(channel({ ptyId: 'pty-a', sessionId: 'sess-pty-a' }), channel())
  const error = validateFlowPtyBindings(flow, [a, b], 'proj')
  assert.match(error || '', /未绑定/)
})

test('start validation rejects auto Grok with no session id yet', () => {
  const newborn = grok({ ptyId: 'pty-new', title: '新窗', sessionId: '' })
  const flow = twoGrokFlow(
    channel({ ptyId: 'pty-a', sessionId: 'sess-pty-a' }),
    channel({ ptyId: 'pty-new', sessionId: '' })
  )
  const error = validateFlowPtyBindings(flow, [a, newborn], 'proj')
  assert.match(error || '', /至少一轮/)
})

test('two distinct bound Grok windows pass validation', () => {
  const flow = twoGrokFlow(
    channel({ ptyId: 'pty-a', sessionId: 'sess-pty-a' }),
    channel({ ptyId: 'pty-b', sessionId: 'sess-pty-b' })
  )
  assert.equal(validateFlowPtyBindings(flow, [a, b], 'proj'), null)
})

function grokNode(id: string, title: string, pty: PtyChannel): FlowNode {
  return {
    id,
    title,
    role: id === 'n-a' ? 'developer' : 'reviewer',
    roleContract: id === 'n-a' ? ROLE_CONTRACTS.developer : ROLE_CONTRACTS.reviewer,
    channel: pty
  }
}

test('preparePtyRun refuses an unbound node even when two Grok windows are live', () => {
  const ready = preparePtyRun(grokNode('n-a', '开发者', channel()), [a, b], 'proj', { send: true })
  assert.equal(ready.ok, false)
  if (ready.ok) return
  assert.match(ready.error, /未绑定/)
})

test('preparePtyRun points auto nodes at their own window', () => {
  const first = preparePtyRun(
    grokNode('n-a', '开发者', channel({ ptyId: 'pty-a', sessionId: 'sess-pty-a' })),
    [a, b],
    'proj',
    { send: true }
  )
  const second = preparePtyRun(
    grokNode('n-b', '审查者', channel({ ptyId: 'pty-b', sessionId: 'sess-pty-b' })),
    [a, b],
    'proj',
    { send: true }
  )
  assert.equal(first.ok, true)
  assert.equal(second.ok, true)
  if (!first.ok || !second.ok) return
  assert.equal(first.live.ptyId, 'pty-a')
  assert.equal(second.live.ptyId, 'pty-b')
  assert.equal(first.sessionId, 'sess-pty-a')
  assert.equal(second.sessionId, 'sess-pty-b')
})

test('preparePtyRun does not succeed without a session id when waiting', () => {
  const newborn = grok({ ptyId: 'pty-new', title: '新窗', sessionId: '' })
  const ready = preparePtyRun(
    grokNode('n-a', '开发者', channel({ ptyId: 'pty-new' })),
    [newborn],
    'proj',
    { send: true }
  )
  assert.equal(ready.ok, false)
  if (ready.ok) return
  assert.match(ready.error, /新窗/)
})

test('preparePtyRun allows paste-without-send on a bound window with no session yet', () => {
  const newborn = grok({ ptyId: 'pty-new', title: '新窗', sessionId: '' })
  const ready = preparePtyRun(
    grokNode('n-a', '开发者', channel({ ptyId: 'pty-new' })),
    [newborn],
    'proj',
    { send: false }
  )
  assert.equal(ready.ok, true)
  if (!ready.ok) return
  assert.equal(ready.waitForTurn, false)
  assert.equal(ready.live.ptyId, 'pty-new')
})

test('timeout error names the window', () => {
  assert.match(ptyWaitTimeoutError(a), /登录限流/)
})
