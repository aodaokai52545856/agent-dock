import assert from 'node:assert/strict'
import { afterOutcome, startRun } from './engine.ts'
import { validateExits } from './exits.ts'
import { removeNode } from './model.ts'
import { preflight } from './preflight.ts'
import { connectExit, convertToExits, exitWires, forgetNode, humanExits, routesByExits, setNodeExits } from './routing.ts'
import { createBlankFlow, createCodexGrokFlow, createDevReviewFlow } from './templates.ts'
import { FLOW_END, FLOW_START, type FlowDef, type FlowEdge, type FlowNode, type NodeExit } from './types.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

const node = (id: string, exits?: NodeExit[]): FlowNode => ({ id, title: id, role: 'developer', roleContract: '', channel: { kind: 'human' }, exits })
const edge = (from: string, to: string, extra: Partial<FlowEdge> = {}): FlowEdge => ({ id: `${from}-${to}`, from, to, mode: 'auto', gate: 'none', ...extra })
const flow = (nodes: FlowNode[], edges: FlowEdge[] = [], extra: Partial<FlowDef> = {}): FlowDef => ({ id: 'f', name: 'f', nodes, edges, maxRetries: 2, maxSteps: 12, slice: 1, draftTask: '', ...extra })

test('a new empty flow routes by exits', () => {
  assert.equal(routesByExits(flow([])), true)
  assert.equal(routesByExits(flow([node('a')], [edge(FLOW_START, 'a')])), true)
})

test('an old edge between nodes keeps the flow on edges', () => {
  assert.equal(routesByExits(flow([node('a'), node('b')], [edge('a', 'b')])), false)
})

test('the first wire from a node is its PASS exit', () => {
  const next = connectExit(flow([node('a'), node('b')]), 'a', 'b')
  assert.deepEqual(next.nodes[0].exits, [{ on: 'PASS', to: 'b' }])
})

test('later wires add the ways back, one each', () => {
  let def = flow([node('a'), node('b', [{ on: 'PASS', to: '@end' }])])
  def = connectExit(def, 'b', 'a')
  assert.deepEqual(def.nodes[1].exits?.[1], { on: 'FAIL', to: 'a' })
  def = connectExit(def, 'b', 'b')
  assert.equal(def.nodes[1].exits?.length, 2)
})

test('a wire to the end is PASS, and only PASS may end the flow', () => {
  assert.deepEqual(connectExit(flow([node('a')]), 'a', FLOW_END).nodes[0].exits, [{ on: 'PASS', to: '@end' }])
  const passed = flow([node('a', [{ on: 'PASS', to: 'b' }]), node('b')])
  assert.equal(connectExit(passed, 'a', FLOW_END), passed)
})

test('the same target twice is ignored', () => {
  const def = flow([node('a', [{ on: 'PASS', to: 'b' }]), node('b')])
  assert.equal(connectExit(def, 'a', 'b'), def)
})

test('once every conclusion is used, another wire is ignored', () => {
  const full: NodeExit[] = [{ on: 'PASS', to: 'b' }, { on: 'FAIL', to: 'a' }, { on: 'REJECT', to: 'a' }, { on: 'BLOCK', to: '@human' }]
  const def = flow([node('a', full), node('b'), node('c')])
  assert.equal(connectExit(def, 'a', 'c'), def)
})

test('clearing the exits leaves none rather than an empty list', () => {
  assert.equal(setNodeExits(flow([node('a', [{ on: 'PASS', to: '@end' }])]), 'a', []).nodes[0].exits, undefined)
})

test('removing a node clears exits, skips and lesson roles that named it', () => {
  const def = flow([node('a', [{ on: 'PASS', to: 'b' }, { on: 'FAIL', to: '@human' }]), node('b', [{ on: 'PASS', to: '@end' }])], [], {
    profiles: { quick: { skip: ['b', 'a'] } },
    experience: { policy: 'model', authors: ['b', 'a'], judge: 'b' }
  })
  const next = removeNode(def, 'b')
  assert.deepEqual(next.nodes.map((n) => n.id), ['a'])
  assert.deepEqual(next.nodes[0].exits, [{ on: 'FAIL', to: '@human' }])
  assert.deepEqual(next.profiles, { quick: { skip: ['a'] } })
  assert.deepEqual(next.experience, { policy: 'model', authors: ['a'], judge: undefined })
})

test('a node whose only exit pointed at the removed one ends up with none', () => {
  assert.equal(forgetNode(flow([node('a', [{ on: 'PASS', to: 'b' }]), node('b')]), 'b').nodes[0].exits, undefined)
})

test('wires go forward or back by position, people get no wire', () => {
  const def = flow([
    node('dev', [{ on: 'PASS', to: 'cr' }]),
    node('cr', [{ on: 'PASS', to: '@end' }, { on: 'REJECT', owner: 'dev', to: 'dev' }, { on: 'BLOCK', to: '@human' }])
  ])
  assert.deepEqual(exitWires(def).map((w) => [w.fromId, w.to, w.label, w.back]), [
    ['dev', 'cr', 'PASS', false],
    ['cr', FLOW_END, 'PASS', false],
    ['cr', 'dev', 'REJECT dev', true]
  ])
  assert.deepEqual(humanExits(def.nodes[1]), ['BLOCK'])
})

test('an exit to a node that is gone draws nothing', () => {
  assert.deepEqual(exitWires(flow([node('a', [{ on: 'PASS', to: 'ghost' }])])), [])
})

test('a reviewed pair converts to PASS and FAIL exits', () => {
  const def = flow([node('dev'), node('cr')], [edge(FLOW_START, 'dev'), edge('dev', 'cr'), edge('cr', FLOW_END, { gate: 'passFail', backTo: 'dev' })])
  const result = convertToExits(def)
  assert.ok('flow' in result)
  assert.deepEqual(result.flow.nodes.map((n) => n.exits), [
    [{ on: 'PASS', to: 'cr' }],
    [{ on: 'PASS', to: '@end' }, { on: 'FAIL', to: 'dev' }]
  ])
  assert.deepEqual(result.flow.edges.map((e) => e.from), [FLOW_START])
  assert.equal(validateExits(result.flow), null)
  assert.ok(result.notes.some((note) => /写结论/.test(note)))
  assert.ok(result.notes.some((note) => /最多 3 次/.test(note)))
})

test('a converted flow runs the same route', () => {
  const def = flow([node('dev'), node('cr')], [edge(FLOW_START, 'dev'), edge('dev', 'cr'), edge('cr', FLOW_END, { gate: 'passFail', backTo: 'dev' })])
  const converted = (convertToExits(def) as { flow: FlowDef }).flow
  let { run } = startRun(converted, 'p', '任务')
  for (const text of ['VERDICT: PASS', 'VERDICT: FAIL', 'VERDICT: PASS', 'VERDICT: PASS']) run = afterOutcome(converted, run, { ok: true, text }).run
  assert.deepEqual(run.steps.map((s) => s.nodeId), ['dev', 'cr', 'dev', 'cr'])
  assert.equal(run.status, 'completed')
})

test('a node with no edge out ends the flow when converted', () => {
  const result = convertToExits(flow([node('a')], [edge(FLOW_START, 'a')]))
  assert.ok('flow' in result)
  assert.deepEqual(result.flow.nodes[0].exits, [{ on: 'PASS', to: '@end' }])
})

test('a manual hand-over is refused, the flow is left alone', () => {
  const result = convertToExits(flow([node('a'), node('b')], [edge('a', 'b', { mode: 'manual' })]))
  assert.ok('problems' in result)
  assert.match(result.problems[0], /手动桥接/)
})

test('a loop is refused', () => {
  const result = convertToExits(flow([node('a'), node('b')], [edge('a', 'b'), edge('b', 'a', { gate: 'loop', maxLoops: 3 })]))
  assert.ok('problems' in result)
  assert.match(result.problems.join(), /循环/)
})

test('every problem is listed, not just the first', () => {
  const result = convertToExits(flow([node('a'), node('b')], [edge('a', 'b', { mode: 'manual' }), edge('b', 'a', { gate: 'loop' })]))
  assert.equal('problems' in result && result.problems.length, 2)
})

test('the review template starts with exits and passes its own checks', () => {
  const def = createDevReviewFlow()
  assert.equal(routesByExits(def), true)
  assert.equal(validateExits(def), null)
  assert.deepEqual(preflight(def).problems, [])
  assert.deepEqual(def.nodes[1].exits?.map((e) => e.on), ['PASS', 'FAIL'])
})

test('the blank template routes by exits, the manual hand-over template keeps its edge', () => {
  assert.equal(routesByExits(createBlankFlow()), true)
  assert.equal(routesByExits(createCodexGrokFlow()), false)
})
