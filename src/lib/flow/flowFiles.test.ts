import assert from 'node:assert/strict'
import test from 'node:test'
import { flowFilePath, flowFileText, mergeRepoFlow, mergeRepoFlows, parseFlowFile } from './flowFiles.ts'
import { FLOW_END, FLOW_START, type FlowDef } from './types.ts'

function flow(): FlowDef {
  return {
    id: 'flow-a',
    name: '开发审查',
    nodes: [
      {
        id: 'dev', title: '开发', role: 'developer', roleContract: '写代码', x: 10, y: 20,
        channel: { kind: 'pty', toolId: 'claude', sessionId: 'sess-1', ptyId: 'pty-1', profileId: 'acct-1' },
        inputs: [{ ref: 'project:memory/index.md', required: true, read: 'index-first' }],
        outputs: [{ path: 'task:dev-log.md', template: 'dev-log.md', sections: ['## 结论'] }]
      },
      {
        id: 'cr', title: '审查', role: 'reviewer', roleContract: '审代码', x: 10, y: 120,
        channel: { kind: 'codexApp', threadIds: ['t-1'], targetKind: 'baseBranch', commitSha: 'abc', baseBranch: 'dev', customInstructions: '看回退' }
      },
      { id: 'cur', title: '补丁', role: 'developer', roleContract: '打补丁', x: 10, y: 220, channel: { kind: 'cursorSdk', agentId: 'agent-1' } }
    ],
    edges: [
      { id: 'e0', from: FLOW_START, to: 'dev', mode: 'auto', gate: 'none' },
      { id: 'e1', from: 'dev', to: 'cr', mode: 'auto', gate: 'none' },
      { id: 'e2', from: 'cr', to: FLOW_END, mode: 'auto', gate: 'passFail', backTo: 'dev' }
    ],
    maxRetries: 3,
    maxSteps: 12,
    slice: 4,
    draftTask: '本机草稿'
  }
}

test('flow files leave out everything bound to this machine', () => {
  const text = flowFileText(flow())
  for (const local of ['sess-1', 'pty-1', 'acct-1', 't-1', 'abc', 'agent-1', '本机草稿', '"slice"']) {
    assert.equal(text.includes(local), false, local)
  }
  assert.match(text, /"toolId": "claude"/)
  assert.match(text, /"baseBranch": "dev"/)
  assert.match(text, /"customInstructions": "看回退"/)
  assert.ok(text.endsWith('}\n'))
})

test('a flow survives the round trip through its file', () => {
  const original = flow()
  const parsed = parseFlowFile('flow-a.json', flowFileText(original))
  assert.ok(parsed)
  const merged = mergeRepoFlow(parsed, original)
  assert.equal(flowFileText(merged), flowFileText(original))
  assert.deepEqual(merged.nodes[0].channel, original.nodes[0].channel)
  assert.deepEqual(merged.nodes[1].channel, original.nodes[1].channel)
  assert.deepEqual(merged.nodes[2].channel, original.nodes[2].channel)
  assert.deepEqual(merged.nodes[0].inputs?.[0], { ref: 'project:memory/index.md', required: true, when: undefined, read: 'index-first' })
  assert.equal(merged.draftTask, '本机草稿')
  assert.equal(merged.slice, 4)
})

test('the file name decides the flow id', () => {
  assert.equal(parseFlowFile('dev.json', flowFileText(flow()))?.id, 'dev')
  assert.equal(flowFilePath('dev'), 'flows/dev.json')
})

test('a hand written minimal flow loads with defaults', () => {
  const text = JSON.stringify({
    nodes: [{ id: 'a', channel: { kind: 'pty', toolId: 'claude' }, outputs: [{ path: 'task:x.md' }, { nope: true }] }],
    edges: []
  })
  const parsed = parseFlowFile('mini.json', text)
  assert.equal(parsed?.name, '未命名流程')
  assert.equal(parsed?.nodes[0].outputs?.length, 1)
  assert.equal(parsed?.nodes[0].inputs, undefined)
})

test('malformed flow files are rejected', () => {
  assert.equal(parseFlowFile('a.json', 'not json'), null)
  assert.equal(parseFlowFile('a.json', 'null'), null)
  assert.equal(parseFlowFile('a.json', '{"nodes":1}'), null)
})

test('the repository wins over the local definition but not the local bindings', () => {
  const local = flow()
  const repo = flow()
  repo.nodes[0].roleContract = '来自仓库'
  repo.nodes[0].channel = { kind: 'pty', toolId: 'claude', sessionId: '', ptyId: '' }
  const merged = mergeRepoFlow(repo, local)
  assert.equal(merged.nodes[0].roleContract, '来自仓库')
  assert.deepEqual(merged.nodes[0].channel, local.nodes[0].channel)
})

test('bindings are dropped when the channel changed', () => {
  const repo = flow()
  repo.nodes[0].channel = { kind: 'pty', toolId: 'grokbuild', sessionId: '', ptyId: '' }
  assert.equal((mergeRepoFlow(repo, flow()).nodes[0].channel as { ptyId: string }).ptyId, '')
})

test('merging keeps local-only flows and adds new repository flows', () => {
  const localOnly = { ...flow(), id: 'local' }
  const fromRepo = { ...flow(), id: 'repo' }
  const merged = mergeRepoFlows([localOnly], [fromRepo])
  assert.deepEqual(merged.map((item) => item.id), ['local', 'repo'])
  assert.equal(merged[0].draftTask, '本机草稿')
  assert.equal(merged[1].draftTask, '')
  assert.equal(merged[1].slice, 1)
})
