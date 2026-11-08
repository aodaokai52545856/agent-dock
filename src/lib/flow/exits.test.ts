import assert from 'node:assert/strict'
import { afterOutcome, applyManualVerdict, cancelRun, expectsVerdict, isAwaitingHuman, startRun } from './engine.ts'
import { exitTicket, pickExit, readConclusion, validateExits } from './exits.ts'
import { normalizeFlow } from './model.ts'
import { finishStep, runRecord, type DockFs, type PreparedStep } from './taskRun.ts'
import type { FlowDef, FlowNode, FlowRun, NodeExit } from './types.ts'

const tests: [string, () => void | Promise<void>][] = []
const test = (name: string, fn: () => void | Promise<void>) => tests.push([name, fn])

function node(id: string, exits?: NodeExit[], extra: Partial<FlowNode> = {}): FlowNode {
  return { id, title: id, role: 'developer', roleContract: '', channel: { kind: 'pty', toolId: 'claude', sessionId: '', ptyId: '' }, exits, ...extra }
}

function flow(nodes: FlowNode[], extra: Partial<FlowDef> = {}): FlowDef {
  return { id: 'f', name: 'f', nodes, edges: [], maxRetries: 2, maxSteps: 20, slice: 1, draftTask: '', ...extra }
}

const review = flow([
  node('dev', [{ on: 'PASS', to: 'cr' }]),
  node('cr', [
    { on: 'PASS', to: 'te' },
    { on: 'REJECT', owner: 'dev', to: 'dev' },
    { on: 'REJECT', owner: 'upstream', to: '@human' },
    { on: 'BLOCK', to: '@human' }
  ]),
  node('te', [{ on: 'PASS', to: '@end' }])
])

const say = (text: string) => ({ ok: true, text })

function advance(def: FlowDef, replies: string[]) {
  let { run } = startRun(def, 'p', '任务')
  let event = startRun(def, 'p', '任务').event
  for (const text of replies) {
    const next = afterOutcome(def, run, say(text))
    run = next.run
    event = next.event
  }
  return { run, event }
}

const path = (run: FlowRun) => run.steps.map((step) => step.nodeId).join(' ')

test('reads a conclusion on its own line', () => {
  assert.deepEqual(readConclusion([], '做完了\nVERDICT: PASS'), { on: 'PASS' })
  assert.deepEqual(readConclusion([], 'VERDICT：reject dev'), { on: 'REJECT', owner: 'dev' })
  assert.deepEqual(readConclusion([], 'VERDICT: REJECT owner=upstream'), { on: 'REJECT', owner: 'upstream' })
  assert.deepEqual(readConclusion([], '**VERDICT: BLOCK**'), { on: 'BLOCK' })
})

test('a sentence that mentions a verdict is not one', () => {
  assert.equal(readConclusion([], '我不确定是否该写 VERDICT: PASS，你看着办'), null)
  assert.equal(readConclusion([], 'VERDICT: MAYBE'), null)
  assert.equal(readConclusion([], ''), null)
})

test('a pass with a caveat is not a pass', () => {
  for (const line of ['VERDICT: PASS 但是还有两个问题', 'VERDICT: PASS however', 'VERDICT: PASS dev', 'VERDICT: PASS, mostly']) {
    assert.equal(readConclusion([], line), null, line)
  }
})

test('a clean verdict cannot paper over a doubtful one', () => {
  const doubt = 'VERDICT: PASS 但是还有两个问题。'
  assert.equal(readConclusion([`## 结论\n${doubt}\nVERDICT: PASS\n`], ''), null)
  assert.equal(readConclusion([`## 结论\nVERDICT: PASS\n${doubt}\n`], 'VERDICT: PASS'), null)
  assert.equal(readConclusion(['## 结论\nVERDICT: PASS\n', `## 结论\n${doubt}\n`], ''), null)
  assert.equal(readConclusion([], `${doubt}\nVERDICT: PASS`), null)
})

test('prose around a verdict is fine, it only has to not claim to be one', () => {
  assert.deepEqual(readConclusion(['## 结论\n这一轮没有问题。\n\nVERDICT: PASS\n'], ''), { on: 'PASS' })
  assert.deepEqual(readConclusion(['## 结论\nVERDICTS 这个词不算\nVERDICT: PASS\n'], ''), { on: 'PASS' })
})

test('a verdict line quoted in another section does not count', () => {
  assert.deepEqual(readConclusion(['## 问题\nVERDICT: PASS 但是还有两个问题\n\n## 结论\nVERDICT: PASS\n'], ''), { on: 'PASS' })
})

test('words after a rejection are not an owner unless they could be one', () => {
  assert.equal(readConclusion([], 'VERDICT: REJECT 因为缺测试'), null)
  assert.deepEqual(readConclusion([], 'VERDICT: FAIL dev-2'), { on: 'FAIL', owner: 'dev-2' })
})

test('only the end of a reply counts', () => {
  const quoted = ['VERDICT: PASS', '一', '二', '三', '四', '五', '六'].join('\n')
  assert.equal(readConclusion([], quoted), null)
  assert.deepEqual(readConclusion([], 'VERDICT: PASS\n改主意了\nVERDICT: REJECT dev'), { on: 'REJECT', owner: 'dev' })
})

test('the document outranks the reply', () => {
  assert.deepEqual(readConclusion(['## 结论\nVERDICT: REJECT dev\n'], 'VERDICT: PASS'), { on: 'REJECT', owner: 'dev' })
})

test('a document without a conclusion falls back to the reply', () => {
  assert.deepEqual(readConclusion(['## 摘要\n一句话\n'], 'VERDICT: PASS'), { on: 'PASS' })
})

test('a document that contradicts itself goes to a person, the reply cannot rescue it', () => {
  assert.equal(readConclusion(['## 结论\nVERDICT: PASS\nVERDICT: FAIL\n'], 'VERDICT: PASS'), null)
  assert.equal(readConclusion(['## 结论\nVERDICT: PASS\n', '## 结论\nVERDICT: FAIL\n'], 'VERDICT: PASS'), null)
  assert.deepEqual(readConclusion(['## 结论\nVERDICT: PASS\nVERDICT: PASS\n'], ''), { on: 'PASS' })
})

test('an exact owner wins over a catch-all, and an unknown owner uses the catch-all', () => {
  const exits: NodeExit[] = [{ on: 'REJECT', to: 'a' }, { on: 'REJECT', owner: 'dev', to: 'b' }]
  assert.equal(pickExit(exits, { on: 'REJECT', owner: 'dev' })?.to, 'b')
  assert.equal(pickExit(exits, { on: 'REJECT', owner: 'qa' })?.to, 'a')
  assert.equal(pickExit(exits, { on: 'REJECT' })?.to, 'a')
})

test('an owner nobody listed matches nothing without a catch-all', () => {
  assert.equal(pickExit([{ on: 'REJECT', owner: 'dev', to: 'b' }], { on: 'REJECT', owner: 'qa' }), null)
  assert.equal(pickExit([{ on: 'REJECT', owner: 'dev', to: 'b' }], { on: 'REJECT' }), null)
})

test('the prompt lists exactly what the node may conclude', () => {
  const ticket = exitTicket(review.nodes[1])
  assert.match(ticket, /VERDICT: PASS、VERDICT: REJECT dev、VERDICT: REJECT upstream、VERDICT: BLOCK/)
})

test('a gate can sit in the middle of a flow', () => {
  const { run, event } = advance(review, ['VERDICT: PASS', 'VERDICT: PASS', 'VERDICT: PASS'])
  assert.equal(path(run), 'dev cr te')
  assert.equal(event.type, 'complete')
  assert.equal(run.status, 'completed')
})

test('a node with exits expects a verdict', () => {
  assert.equal(expectsVerdict(review, 'dev'), true)
})

test('a rejection goes where the owner says', () => {
  const { run, event } = advance(review, ['VERDICT: PASS', 'VERDICT: REJECT dev'])
  assert.equal(path(run), 'dev cr dev')
  assert.equal(event.type, 'execute')
  assert.equal(run.retryCount, 1)
})

test('a rejection of the upstream stops for a person', () => {
  const { run, event } = advance(review, ['VERDICT: PASS', 'VERDICT: REJECT upstream'])
  assert.equal(event.type, 'waitHuman')
  assert.equal(run.status, 'waiting')
  assert.equal(isAwaitingHuman(review, run), true)
  assert.match(run.lastError, /REJECT/)
})

test('an unreadable conclusion stops for a person, it never moves on', () => {
  const { run, event } = advance(review, ['都改好了'])
  assert.equal(event.type, 'waitHuman')
  assert.equal(path(run), 'dev')
  assert.match(run.exitPause || '', /读不出来/)
})

test('a conclusion with no exit stops for a person', () => {
  const { run } = advance(review, ['VERDICT: FAIL'])
  assert.match(run.exitPause || '', /FAIL 没有对应的出口/)
})

test('a node sent back too often stops for a person', () => {
  const def = flow([node('dev', [{ on: 'PASS', to: 'cr' }], { maxAttempts: 2 }), review.nodes[1]])
  const { run, event } = advance(def, ['VERDICT: PASS', 'VERDICT: REJECT dev', 'VERDICT: PASS', 'VERDICT: REJECT dev'])
  assert.equal(event.type, 'waitHuman')
  assert.equal(path(run), 'dev cr dev cr')
  assert.match(run.exitPause || '', /已尝试 2 次/)
})

test('without a limit the flow retry count applies, per node', () => {
  const def = flow(review.nodes, { maxRetries: 1 })
  const { run } = advance(def, ['VERDICT: PASS', 'VERDICT: REJECT dev', 'VERDICT: PASS', 'VERDICT: REJECT dev'])
  assert.match(run.exitPause || '', /已尝试 2 次/)
})

test('attempts from earlier runs of the task count', () => {
  const def = flow(review.nodes, { maxRetries: 1 })
  const { run } = startRun(def, 'p', '任务', 'cr')
  run.priorAttempts = { dev: 2 }
  assert.match(afterOutcome(def, run, say('VERDICT: REJECT dev')).run.exitPause || '', /已尝试 2 次/)
})

test('a person can pass a paused node along its PASS exit', () => {
  const { run } = advance(review, ['VERDICT: PASS', 'VERDICT: BLOCK'])
  const { run: after, event } = applyManualVerdict(review, run, 'pass')
  assert.equal(event.type, 'execute')
  assert.equal(path(after), 'dev cr te')
  assert.equal(after.exitPause, undefined)
  assert.equal(after.lastError, '')
  assert.equal(after.steps[1].status, 'completed')
})

test('a person can fail a paused node, which ends the run as failed', () => {
  const { run } = advance(review, ['VERDICT: PASS', 'VERDICT: BLOCK'])
  const { run: after, event } = applyManualVerdict(review, run, 'fail')
  assert.equal(event.type, 'fail')
  assert.equal(after.status, 'failed')
  assert.equal(after.steps[1].status, 'failed')
})

test('a FAIL exit is honoured when a person marks a failure', () => {
  const def = flow([node('dev', [{ on: 'PASS', to: '@end' }, { on: 'FAIL', to: 'fix' }]), node('fix', [{ on: 'PASS', to: '@end' }])])
  const { run } = advance(def, ['不知道'])
  assert.equal(applyManualVerdict(def, run, 'fail').run.currentNodeId, 'fix')
})

test('marking pass on a failed run cannot complete a node whose exits lead elsewhere', () => {
  const { run } = startRun(review, 'p', '任务')
  const failed = afterOutcome(review, run, { ok: false, text: '', error: '窗口没了' }).run
  assert.equal(failed.status, 'failed')
  const { run: after } = applyManualVerdict(review, failed, 'pass')
  assert.notEqual(after.status, 'completed')
  assert.equal(after.currentNodeId, 'cr')
})

test('a manual pass only ends the run through an exit that says so', () => {
  const def = flow([node('dev', [{ on: 'PASS', to: '@human' }])])
  const { run } = advance(def, ['VERDICT: PASS'])
  const { run: after } = applyManualVerdict(def, run, 'pass')
  assert.equal(after.status, 'failed')
})

test('a human node follows its exits too', () => {
  const human = node('gate', [{ on: 'PASS', to: 'te' }, { on: 'FAIL', to: 'dev' }], { channel: { kind: 'human' } })
  const def = flow([human, node('dev', [{ on: 'PASS', to: '@end' }]), node('te', [{ on: 'PASS', to: '@end' }])])
  const { run } = startRun(def, 'p', '任务')
  run.status = 'waiting'
  assert.equal(applyManualVerdict(def, run, 'fail').run.currentNodeId, 'dev')
  assert.equal(applyManualVerdict(def, run, 'pass').run.currentNodeId, 'te')
})

test('stopping clears the pause', () => {
  const { run } = advance(review, ['都改好了'])
  assert.equal(cancelRun(run).exitPause, undefined)
})

test('the step cap still applies', () => {
  const def = flow(review.nodes, { maxSteps: 2, maxRetries: 9 })
  const { run } = advance(def, ['VERDICT: PASS', 'VERDICT: REJECT dev'])
  assert.equal(run.status, 'failed')
})

test('nodes without exits keep the old edges', () => {
  const def = flow([node('a'), node('b')], { edges: [{ id: 'e', from: 'a', to: 'b', mode: 'auto', gate: 'none' }] })
  assert.equal(advance(def, ['随便说点什么']).run.currentNodeId, 'b')
})

test('the trace keeps the conclusion and its owner', () => {
  const { run } = advance(review, ['VERDICT: PASS', 'VERDICT: REJECT dev'])
  assert.deepEqual(runRecord(run).steps[1].conclusion, { on: 'REJECT', owner: 'dev' })
})

const fsOf = (text: string): DockFs => ({
  read: async () => ({ text, sha256: 'new' }),
  write: async () => 'new',
  exists: async () => true
} as unknown as DockFs)

const prepared: PreparedStep = { block: '', inputs: [], before: {}, placeholders: {} }

test('finishing a step reads the conclusion from its document', async () => {
  const target = node('cr', [{ on: 'PASS', to: '@end' }], { outputs: [{ path: 'task:review.md' }] })
  const { run } = startRun(flow([target]), 'p', '任务')
  run.taskDir = 't'
  const { outcome } = await finishStep(fsOf('## 结论\nVERDICT: REJECT dev\n'), target, run, prepared, say('VERDICT: PASS'), true)
  assert.deepEqual(outcome.conclusion, { on: 'REJECT', owner: 'dev' })
})

test('a conclusion the channel already knows is not overruled by the document', async () => {
  const target = node('sh', [{ on: 'PASS', to: '@end' }], { outputs: [{ path: 'task:log.md' }] })
  const { run } = startRun(flow([target]), 'p', '任务')
  run.taskDir = 't'
  const given = { ...say('跑完了'), conclusion: { on: 'FAIL' as const } }
  const { outcome } = await finishStep(fsOf('## 结论\nVERDICT: PASS\n'), target, run, prepared, given, true)
  assert.deepEqual(outcome.conclusion, { on: 'FAIL' })
})

test('an unreadable document pauses instead of falling through to an empty conclusion', async () => {
  const target = node('cr', [{ on: 'PASS', to: '@end' }], { outputs: [{ path: 'task:review.md' }] })
  const def = flow([target])
  const { run } = startRun(def, 'p', '任务')
  run.taskDir = 't'
  const { outcome } = await finishStep(fsOf('## 结论\n看起来还行\n'), target, run, prepared, say('好了'), true)
  assert.equal(outcome.conclusion, null)
  assert.equal(afterOutcome(def, run, outcome).event.type, 'waitHuman')
})

const invalid = (exits: NodeExit[], edges: FlowDef['edges'] = []) => validateExits(flow([node('a', exits), node('b')], { edges }))

test('valid exits pass', () => {
  assert.equal(validateExits(review), null)
})

test('exits that could misroute a run are refused before it starts', () => {
  assert.match(invalid([{ on: 'REJECT', to: 'b' }]) || '', /没有 PASS 出口/)
  assert.match(invalid([{ on: 'PASS', to: 'nowhere' }]) || '', /不存在的节点/)
  assert.match(invalid([{ on: 'PASS', to: 'b' }, { on: 'MAYBE' as never, to: 'b' }]) || '', /结论只能是/)
  assert.match(invalid([{ on: 'PASS', to: 'b' }, { on: 'PASS', to: '@human' }]) || '', /重复/)
  assert.match(invalid([{ on: 'PASS', owner: 'dev', to: 'b' }]) || '', /PASS 不能带责任方/)
  assert.match(invalid([{ on: 'PASS', to: 'b' }, { on: 'REJECT', owner: '开发', to: 'b' }]) || '', /责任方只能用/)
  assert.match(invalid([{ on: 'PASS', to: 'b' }], [{ id: 'e', from: 'a', to: 'b', mode: 'auto', gate: 'none' }]) || '', /同时有出口和连线/)
})

test('only PASS may end a flow, a run that completes unlocks the next slice', () => {
  assert.match(invalid([{ on: 'PASS', to: 'b' }, { on: 'FAIL', to: '@end' }]) || '', /只有 PASS/)
})

test('flow files keep exits as written, bad ones included, so the run refuses them', () => {
  const load = (exits: unknown, maxAttempts?: unknown) => normalizeFlow({ id: 'f', name: 'f', nodes: [{ id: 'a', role: 'developer', channel: { kind: 'human' }, exits, maxAttempts }], edges: [] } as unknown as FlowDef) as FlowDef
  assert.deepEqual(load([{ on: 'reject', owner: 'dev', to: 'a' }]).nodes[0].exits, [{ on: 'REJECT', owner: 'dev', to: 'a' }])
  assert.match(validateExits(load([{ on: 'nope', to: 'a' }])) || '', /没有 PASS|结论只能是/)
  assert.equal(load([]).nodes[0].exits, undefined)
  assert.equal(load(undefined, 3).nodes[0].maxAttempts, 3)
  assert.equal(load(undefined, 0).nodes[0].maxAttempts, undefined)
})

for (const [name, fn] of tests) {
  await fn()
  console.log(`ok ${name}`)
}
