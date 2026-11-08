import assert from 'node:assert/strict'
import { afterOutcome, startRun } from './engine.ts'
import { passSkipped } from './exits.ts'
import { flowFileText, mergeRepoFlow, parseFlowFile } from './flowFiles.ts'
import { preflight, preflightMessage, selectedProfile, validateProfiles } from './preflight.ts'
import { runRecord } from './taskRun.ts'
import type { FlowDef, FlowNode, NodeExit, NodeInput } from './types.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

type Spec = { exits?: NodeExit[]; reads?: (string | NodeInput)[]; writes?: string[] }

function node(id: string, { exits, reads = [], writes = [] }: Spec = {}): FlowNode {
  return {
    id, title: id, role: 'developer', roleContract: '',
    channel: { kind: 'pty', toolId: 'claude', sessionId: '', ptyId: '' },
    exits,
    inputs: reads.map((row) => (typeof row === 'string' ? { ref: row, required: true } : row)),
    outputs: writes.map((path) => ({ path }))
  }
}

function flow(nodes: FlowNode[], extra: Partial<FlowDef> = {}): FlowDef {
  return { id: 'f', name: 'f', nodes, edges: [], maxRetries: 2, maxSteps: 20, slice: 1, draftTask: '', ...extra }
}

const pass = (to: string, ...rest: NodeExit[]): NodeExit[] => [{ on: 'PASS', to }, ...rest]
const full = () => flow([
  node('sa', { exits: pass('dev'), writes: ['task:design.md'] }),
  node('dev', { exits: pass('te'), reads: ['task:design.md'], writes: ['task:log.md'] }),
  node('te', { exits: pass('cr', { on: 'FAIL', to: 'dev' }), writes: ['task:test.md'] }),
  node('cr', { exits: pass('@end', { on: 'REJECT', to: 'dev' }), reads: ['task:log.md', 'task:test.md'], writes: ['task:review.md'] })
])

test('a flow whose inputs are all produced upstream passes', () => {
  assert.deepEqual(preflight(full()).problems, [])
})

test('an input nobody produces is caught before anything runs', () => {
  const def = flow([node('dev', { exits: pass('cr') }), node('cr', { exits: pass('@end'), reads: ['task:design.md'] })])
  assert.match(preflight(def).problems[0], /「cr」必读 task:design\.md/)
})

test('an input produced only downstream does not count', () => {
  const def = flow([node('a', { exits: pass('b'), reads: ['task:x.md'] }), node('b', { exits: pass('@end'), writes: ['task:x.md'] })])
  assert.equal(preflight(def).problems.length, 1)
})

test('one route that skips the producer is enough to fail', () => {
  const def = flow([
    node('a', { exits: pass('b', { on: 'REJECT', to: 'c' }) }),
    node('b', { exits: pass('c'), writes: ['task:x.md'] }),
    node('c', { exits: pass('@end'), reads: ['task:x.md'] })
  ])
  assert.equal(preflight(def).problems.length, 1)
})

test('optional inputs are not required', () => {
  const def = flow([node('a', { exits: pass('@end'), reads: [{ ref: 'task:x.md' }] })])
  assert.deepEqual(preflight(def).problems, [])
})

test('project files are not judged here', () => {
  const def = flow([node('a', { exits: pass('@end'), reads: ['project:memory/index.md'] })])
  assert.deepEqual(preflight(def).problems, [])
})

test('a rework input only has to be produced by someone', () => {
  const rework = { ref: 'task:review.md', required: true, when: 'reworked' as const }
  const good = flow([
    node('dev', { exits: pass('cr'), reads: [rework] }),
    node('cr', { exits: pass('@end', { on: 'REJECT', to: 'dev' }), writes: ['task:review.md'] })
  ])
  assert.deepEqual(preflight(good).problems, [])
  const bad = flow([node('dev', { exits: pass('@end'), reads: [rework] })])
  assert.equal(preflight(bad).problems.length, 1)
})

test('a loop ends the walk', () => {
  const def = flow([node('a', { exits: pass('b') }), node('b', { exits: pass('a', { on: 'FAIL', to: 'b' }) })])
  assert.deepEqual(preflight(def).reached.map((row) => row.id), ['a', 'b'])
})

test('old gated edges and back edges are walked too', () => {
  const def = flow([node('dev', { writes: ['task:log.md'] }), node('cr', { reads: ['task:log.md', 'task:gone.md'] })], {
    edges: [
      { id: 's', from: '__start__', to: 'dev', mode: 'auto', gate: 'none' },
      { id: 'a', from: 'dev', to: 'cr', mode: 'auto', gate: 'none' },
      { id: 'b', from: 'cr', to: '__end__', mode: 'auto', gate: 'passFail', backTo: 'dev' }
    ]
  })
  const { problems, reached } = preflight(def)
  assert.equal(problems.length, 1)
  assert.match(problems[0], /task:gone\.md/)
  assert.equal(reached.length, 2)
})

test('a flow with no nodes has nothing to check', () => {
  assert.deepEqual(preflight(flow([])).problems, [])
})

const profiled = () => flow(full().nodes, { profiles: { full: { skip: [] }, quick: { skip: ['te'] }, docs: { skip: ['dev', 'te'] } } })

test('skipping a node follows its PASS exit', () => {
  assert.equal(passSkipped(profiled(), ['te'], 'te'), 'cr')
  assert.equal(passSkipped(profiled(), ['dev', 'te'], 'dev'), 'cr')
  assert.equal(passSkipped(profiled(), ['te'], 'cr'), 'cr')
})

test('a skip that loops gives up and asks a person', () => {
  const def = flow([node('a', { exits: pass('b') }), node('b', { exits: pass('a') })])
  assert.equal(passSkipped(def, ['a', 'b'], 'a'), '@human')
})

test('skipping the producer of a required input fails preflight for that profile only', () => {
  const def = profiled()
  assert.deepEqual(preflight(def, 'full').problems, [])
  assert.match(preflight(def, 'quick').problems[0], /task:test\.md/)
})

test('the same flow can be made to work for a shorter profile', () => {
  const def = profiled()
  def.nodes[3].inputs = [{ ref: 'task:design.md', required: true }]
  assert.deepEqual(preflight(def, 'docs').problems, [])
  assert.deepEqual(preflight(def, 'docs').reached.map((row) => row.id), ['sa', 'cr'])
})

test('a profile that skips everything has nowhere to start', () => {
  const def = flow([node('a', { exits: pass('@human') })], { profiles: { none: { skip: ['a'] } } })
  assert.match(preflight(def, 'none').problems[0], /没有可以开始的节点/)
})

const reply = (text: string) => ({ ok: true, text })

test('a run skips what its profile says', () => {
  const def = profiled()
  let { run } = startRun(def, 'p', '任务', undefined, 'docs')
  run = afterOutcome(def, run, reply('VERDICT: PASS')).run
  assert.deepEqual(run.steps.map((step) => step.nodeId), ['sa', 'cr'])
  assert.equal(runRecord(run).profile, 'docs')
})

test('a run that starts on a skipped node starts at the next one', () => {
  const def = flow(full().nodes, { profiles: { late: { skip: ['sa'] } } })
  assert.equal(startRun(def, 'p', '任务', undefined, 'late').run.currentNodeId, 'dev')
})

test('rerunning a named node ignores the skip list', () => {
  const def = profiled()
  assert.equal(startRun(def, 'p', '任务', 'te', 'docs').run.currentNodeId, 'te')
})

test('a run cannot start when everything is skipped', () => {
  const def = flow([node('a', { exits: pass('@human') })], { profiles: { none: { skip: ['a'] } } })
  assert.equal(startRun(def, 'p', '任务', undefined, 'none').run.status, 'failed')
})

test('a rejection into a skipped node carries on past it', () => {
  const def = flow(full().nodes, { profiles: { quick: { skip: ['dev'] } } })
  const { run } = startRun(def, 'p', '任务', 'cr', 'quick')
  assert.equal(afterOutcome(def, run, reply('VERDICT: REJECT')).run.currentNodeId, 'te')
})

test('profiles need exits on every node', () => {
  const def = flow([node('a')], { profiles: { p: { skip: [] } } })
  assert.match(validateProfiles(def) || '', /每个节点都要声明出口/)
})

test('a profile cannot skip a node that does not exist', () => {
  const def = flow(full().nodes, { profiles: { p: { skip: ['ghost'] } } })
  assert.match(validateProfiles(def) || '', /不存在的节点：ghost/)
  assert.equal(validateProfiles(profiled()), null)
  assert.equal(validateProfiles(full()), null)
})

test('the first profile is the default, the local choice wins while it exists', () => {
  const def = profiled()
  assert.equal(selectedProfile(def), 'full')
  assert.equal(selectedProfile({ ...def, draftProfile: 'docs' }), 'docs')
  assert.equal(selectedProfile({ ...def, draftProfile: 'gone' }), 'full')
  assert.equal(selectedProfile(full()), undefined)
})

test('profiles are saved in the flow file, the choice stays on this machine', () => {
  const def = { ...profiled(), draftProfile: 'docs' }
  const text = flowFileText(def)
  assert.equal(text.includes('draftProfile'), false)
  const loaded = parseFlowFile('f.json', text) as FlowDef
  assert.deepEqual(loaded.profiles?.quick, { skip: ['te'] })
  assert.equal(mergeRepoFlow(loaded, def).draftProfile, 'docs')
})

test('a bad skip list in a file is dropped to an empty one', () => {
  const load = (profiles: unknown) => parseFlowFile('f.json', JSON.stringify({ nodes: [], edges: [], profiles })) as FlowDef
  assert.deepEqual(load({ a: { skip: 'dev' } }).profiles, { a: { skip: [] } })
  assert.equal(load('nope').profiles, undefined)
})

test('the message shows the first problems and counts the rest', () => {
  assert.equal(preflightMessage(['a', 'b', 'c', 'd', 'e']), '启动前检查没通过：a；b；c；还有 2 处')
  assert.equal(preflightMessage(['a']), '启动前检查没通过：a')
})
