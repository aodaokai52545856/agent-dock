import assert from 'node:assert/strict'
import { defaultPtyChannel } from './channels.ts'
import {
  appendNode,
  createCustomNode,
  ensurePairLoop,
  loadAllFlows,
  migratePipeline,
  persistAllFlows,
  removeNode,
  seedProjectFlows,
  type StorageLike
} from './model.ts'
import { FLOW_END } from './types.ts'
import { FLOW_TEMPLATES, flowFromTemplate } from './templates.ts'
import { emptyPipeline } from '../pipelineModel.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

function memory(): StorageLike {
  const map = new Map<string, string>()
  return {
    getItem(key) {
      return map.get(key) ?? null
    },
    setItem(key, value) {
      map.set(key, value)
    }
  }
}

test('migrate pipeline keeps slice threads and retries', () => {
  const pipeline = emptyPipeline('p1')
  pipeline.slice = 4
  pipeline.task = '修侧栏'
  pipeline.selectedThreadIds = ['thr_a']
  pipeline.maxRetries = 5
  pipeline.cursorAgentId = 'agent-9'
  pipeline.targetKind = 'commit'
  pipeline.commitSha = 'abc123'
  const flow = migratePipeline(pipeline)
  assert.equal(flow.slice, 4)
  assert.equal(flow.draftTask, '修侧栏')
  assert.equal(flow.maxRetries, 5)
  const cursor = flow.nodes.find((node) => node.channel.kind === 'cursorSdk')
  assert.equal(cursor?.channel.kind === 'cursorSdk' && cursor.channel.agentId, 'agent-9')
  const codex = flow.nodes.find((node) => node.channel.kind === 'codexApp')
  assert.ok(codex && codex.channel.kind === 'codexApp')
  if (codex && codex.channel.kind === 'codexApp') {
    assert.deepEqual(codex.channel.threadIds, ['thr_a'])
    assert.equal(codex.channel.targetKind, 'commit')
    assert.equal(codex.channel.commitSha, 'abc123')
  }
})

test('empty storage seeds the developer-reviewer pair', () => {
  const seeded = seedProjectFlows()
  assert.equal(seeded.flows.length, 1)
  assert.equal(seeded.flows[0].templateId, 'dev-review')
  assert.equal(seeded.flows[0].nodes.length, 2)
  assert.ok(seeded.flows[0].edges.some((edge) => edge.backTo === seeded.flows[0].nodes[0].id))
})

test('old pipeline storage migrates on load', () => {
  const storage = memory()
  storage.setItem(
    'ad-bridge-pipelines',
    JSON.stringify({
      p1: {
        ...emptyPipeline('p1'),
        slice: 3,
        selectedThreadIds: ['thr_1']
      }
    })
  )
  const all = loadAllFlows(storage)
  assert.equal(all.p1.flows[0].slice, 3)
  const codex = all.p1.flows[0].nodes.find((node) => node.channel.kind === 'codexApp')
  assert.ok(codex && codex.channel.kind === 'codexApp')
  if (codex && codex.channel.kind === 'codexApp') {
    assert.deepEqual(codex.channel.threadIds, ['thr_1'])
  }
  assert.equal(all.p1.flows.length, 1)
})

test('persist roundtrip keeps selected flow', () => {
  const storage = memory()
  const seeded = seedProjectFlows()
  persistAllFlows({ p1: seeded }, storage)
  const loaded = loadAllFlows(storage)
  assert.equal(loaded.p1.selectedFlowId, seeded.selectedFlowId)
  assert.equal(loaded.p1.flows.length, 1)
})

test('appendNode on a pair adds a third node after the reviewer', () => {
  const flow = seedProjectFlows().flows[0]
  const added = appendNode(flow, createCustomNode('pm', { kind: 'human' }))
  assert.equal(added.nodes.length, 3)
  const last = added.nodes[added.nodes.length - 1]
  const prev = added.nodes[added.nodes.length - 2]
  assert.equal(added.edges.some((edge) => edge.from === prev.id && edge.to === last.id), false)
})

test('adding a node does not auto-wire it', () => {
  const flow = seedProjectFlows().flows[0]
  const before = flow.edges.length
  const added = appendNode(flow, createCustomNode('pm', { kind: 'human' }))
  assert.equal(added.nodes.length, flow.nodes.length + 1)
  assert.equal(added.edges.length, before)
  const last = added.nodes[added.nodes.length - 1]
  assert.equal(added.edges.some((edge) => edge.from === last.id || edge.to === last.id), false)
})

test('load repairs passFail gates that sat on a forward hop', () => {
  const storage = memory()
  const seeded = seedProjectFlows()
  const flow = seeded.flows[0]
  const extra = createCustomNode('pm', { kind: 'human' })
  extra.id = 'n-extra'
  flow.nodes.push(extra)
  flow.edges = [
    { id: 'e1', from: flow.nodes[0].id, to: flow.nodes[1].id, mode: 'auto', transform: 'roleWrap', gate: 'none' },
    {
      id: 'e2',
      from: flow.nodes[1].id,
      to: extra.id,
      mode: 'auto',
      transform: 'roleWrap',
      gate: 'passFail',
      backTo: flow.nodes[0].id
    }
  ]
  persistAllFlows({ p1: { ...seeded, flows: [flow] } }, storage)
  const loaded = loadAllFlows(storage).p1.flows[0]
  const hop = loaded.edges.find((edge) => edge.from === flow.nodes[1].id && edge.to === extra.id)
  assert.equal(hop?.gate, 'none')
  const terminal = loaded.edges.find((edge) => edge.to === FLOW_END)
  assert.equal(terminal?.gate, 'passFail')
})

test('appending onto a passFail end edge does not leave the gate on the forward hop', () => {
  const flow = seedProjectFlows().flows[0]
  const added = appendNode(flow, createCustomNode('pm', { kind: 'human' }))
  const mid = added.nodes[1]
  const last = added.nodes[2]
  const forward = added.edges.find((edge) => edge.from === mid.id && edge.to === last.id)
  assert.equal(forward, undefined)
  const terminal = added.edges.find((edge) => edge.from === mid.id)
  assert.equal(terminal?.to, FLOW_END)
  assert.equal(terminal?.gate, 'passFail')
  assert.equal(added.edges.some((edge) => edge.from === last.id), false)
})

test('removeNode drops the node and its wires without stitching', () => {
  const flow = seedProjectFlows().flows[0]
  const extra = appendNode(flow, createCustomNode('pm', { kind: 'human' }))
  const mid = extra.nodes[1]
  const trimmed = removeNode(extra, mid.id)
  assert.equal(trimmed.nodes.length, extra.nodes.length - 1)
  assert.ok(!trimmed.nodes.some((node) => node.id === mid.id))
  assert.equal(trimmed.edges.some((edge) => edge.from === mid.id || edge.to === mid.id), false)
})

test('removeNode can clear the last role node', () => {
  const flow = seedProjectFlows().flows[0]
  let next = flow
  for (const node of [...flow.nodes]) next = removeNode(next, node.id)
  assert.equal(next.nodes.length, 0)
})

test('two nodes get a fail-back loop', () => {
  const one = {
    ...seedProjectFlows().flows[0],
    nodes: [createCustomNode('developer', { kind: 'cursorSdk', agentId: '' })],
    edges: []
  }
  const pair = ensurePairLoop(appendNode(one, createCustomNode('reviewer', {
    kind: 'codexApp',
    threadIds: [],
    targetKind: 'uncommittedChanges',
    commitSha: '',
    baseBranch: 'main',
    customInstructions: ''
  })))
  assert.equal(pair.nodes.length, 2)
  assert.ok(pair.edges.some((edge) => edge.gate === 'passFail' && edge.backTo === pair.nodes[0].id))
})

test('pty default channel is grok', () => {
  assert.equal(defaultPtyChannel().toolId, 'grokbuild')
})

test('all flow templates are listed', () => {
  const ids = FLOW_TEMPLATES.map((item) => item.id)
  assert.deepEqual(ids, ['dev-review', 'codex-grok', 'blank'])
  assert.equal(flowFromTemplate('codex-grok').templateId, 'codex-grok')
  assert.equal(flowFromTemplate('blank').nodes.length, 1)
})
