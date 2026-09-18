import assert from 'node:assert/strict'
import {
  START_H,
  connectForward,
  defaultNodePoint,
  defaultStartPoint,
  deleteEdge,
  placeNode,
  snap,
  terminals,
  withNodePoints
} from './chartLayout.ts'
import { createCustomNode, seedProjectFlows } from './model.ts'
import { FLOW_END, FLOW_START } from './types.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('default node points share an x so the spine stays aligned', () => {
  const a = defaultNodePoint(0)
  const b = defaultNodePoint(1)
  assert.equal(a.x, b.x)
  assert.ok(b.y - a.y >= 140)
})

test('start sits above the first node, end sits below the last', () => {
  const flow = withNodePoints(seedProjectFlows().flows[0])
  const { start, end } = terminals(flow)
  const first = defaultNodePoint(0)
  const last = defaultNodePoint(1)
  assert.equal(start.x, first.x)
  assert.ok(start.y + START_H < first.y)
  assert.equal(end.x, last.x)
  assert.ok(end.y >= last.y + 72)
  assert.equal(defaultStartPoint().y < first.y, true)
})

test('connectForward can start from the start terminal', () => {
  const flow = seedProjectFlows().flows[0]
  const wired = connectForward(flow, FLOW_START, flow.nodes[0].id)
  const edge = wired.edges.find((item) => item.from === FLOW_START)
  assert.equal(edge?.to, flow.nodes[0].id)
})

test('deleteEdge removes only that wire', () => {
  const flow = seedProjectFlows().flows[0]
  const edgeId = flow.edges[0].id
  const next = deleteEdge(flow, edgeId)
  assert.equal(next.edges.some((item) => item.id === edgeId), false)
  assert.equal(next.edges.length, flow.edges.length - 1)
})

test('snap lands on the 16px grid', () => {
  assert.equal(snap(0), 0)
  assert.equal(snap(17), 16)
  assert.equal(snap(24), 32)
  assert.equal(snap(25), 32)
})

test('withNodePoints fills missing coordinates without moving placed nodes', () => {
  const flow = seedProjectFlows().flows[0]
  flow.nodes[0].x = 80
  flow.nodes[0].y = 40
  const placed = withNodePoints(flow)
  assert.equal(placed.nodes[0].x, 80)
  assert.equal(placed.nodes[0].y, 40)
  assert.equal(typeof placed.nodes[1].x, 'number')
  assert.equal(typeof placed.nodes[1].y, 'number')
})

test('placeNode snaps the dragged node', () => {
  const flow = withNodePoints(seedProjectFlows().flows[0])
  const moved = placeNode(flow, flow.nodes[0].id, 19, 27)
  assert.equal(moved.nodes[0].x, 16)
  assert.equal(moved.nodes[0].y, 32)
})

test('connectForward rewires the existing outgoing edge', () => {
  const flow = seedProjectFlows().flows[0]
  const extra = createCustomNode('pm', { kind: 'human' })
  extra.id = 'n-extra'
  const next = { ...flow, nodes: [...flow.nodes, extra] }
  const wired = connectForward(next, flow.nodes[1].id, extra.id)
  const edge = wired.edges.find((item) => item.from === flow.nodes[1].id && item.gate !== 'loop')
  assert.equal(edge?.to, extra.id)
  assert.ok(!wired.edges.some((item) => item.from === flow.nodes[1].id && item.to === FLOW_END))
})

test('connecting back to an upstream node adds a loop and keeps the exit', () => {
  const flow = seedProjectFlows().flows[0]
  const dev = flow.nodes[0]
  const rev = flow.nodes[1]
  const wired = connectForward(flow, rev.id, dev.id)
  const loop = wired.edges.find((item) => item.from === rev.id && item.to === dev.id)
  assert.equal(loop?.gate, 'loop')
  assert.equal(loop?.maxLoops, 3)
  assert.ok(wired.edges.some((item) => item.from === rev.id && item.to === FLOW_END))
})
