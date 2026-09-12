import assert from 'node:assert/strict'
import { defaultPtyChannel } from './channels.ts'
import {
  appendNode,
  createCustomNode,
  loadAllFlows,
  migratePipeline,
  persistAllFlows,
  removeNode,
  seedProjectFlows,
  type StorageLike
} from './model.ts'
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
  assert.ok(added.edges.some((edge) => edge.from === prev.id && edge.to === last.id))
})

test('removeNode stitches neighbors', () => {
  const flow = seedProjectFlows().flows[0]
  const extra = appendNode(flow, createCustomNode('pm', { kind: 'human' }))
  const mid = extra.nodes[1]
  const trimmed = removeNode(extra, mid.id)
  assert.equal(trimmed.nodes.length, extra.nodes.length - 1)
  assert.ok(!trimmed.nodes.some((node) => node.id === mid.id))
})

test('two nodes get a fail-back loop', () => {
  const one = {
    ...seedProjectFlows().flows[0],
    nodes: [createCustomNode('developer', { kind: 'cursorSdk', agentId: '' })],
    edges: []
  }
  const pair = appendNode(one, createCustomNode('reviewer', {
    kind: 'codexApp',
    threadIds: [],
    targetKind: 'uncommittedChanges',
    commitSha: '',
    baseBranch: 'main',
    customInstructions: ''
  }))
  assert.equal(pair.nodes.length, 2)
  assert.ok(pair.edges.some((edge) => edge.gate === 'passFail' && edge.backTo === pair.nodes[0].id))
})

test('pty default channel is grok', () => {
  assert.equal(defaultPtyChannel().toolId, 'grokbuild')
})
