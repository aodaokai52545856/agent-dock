import { makeId } from './ids.ts'
import { FLOW_END, FLOW_START, MAX_LOOPS_DEFAULT, type FlowDef, type FlowNode } from './types.ts'

export const NODE_W = 248
export const NODE_H = 72
export const NODE_GAP_Y = 88
export const NODE_X = 48
export const GRID = 16
export const START_H = 40
export const END_H = 40

export function snap(n: number) {
  return Math.round(n / GRID) * GRID
}

export function defaultStartPoint() {
  return { x: NODE_X, y: 16 }
}

export function defaultNodePoint(index: number) {
  const start = defaultStartPoint()
  return {
    x: NODE_X,
    y: start.y + START_H + NODE_GAP_Y + index * (NODE_H + NODE_GAP_Y)
  }
}

function isLegacyColumn(flow: FlowDef) {
  if (Number.isFinite(flow.startY) || Number.isFinite(flow.startX)) return false
  if (!flow.nodes.length) return false
  return flow.nodes.every((node, index) => {
    if (!Number.isFinite(node.x) || !Number.isFinite(node.y)) return true
    const old = 24 + index * (NODE_H + NODE_GAP_Y)
    return node.x === NODE_X && node.y === old
  })
}

export function withNodePoints(flow: FlowDef): FlowDef {
  const remap = isLegacyColumn(flow)
  return {
    ...flow,
    nodes: flow.nodes.map((node, index) => {
      if (!remap && Number.isFinite(node.x) && Number.isFinite(node.y)) return node
      const point = defaultNodePoint(index)
      return { ...node, x: point.x, y: point.y }
    })
  }
}

export function placeNode(flow: FlowDef, id: string, x: number, y: number): FlowDef {
  return {
    ...flow,
    nodes: flow.nodes.map((node) => (
      node.id === id
        ? { ...node, x: Math.max(0, snap(x)), y: Math.max(0, snap(y)) }
        : node
    ))
  }
}

export function closesCycle(flow: FlowDef, fromId: string, toId: string) {
  if (!toId || toId === FLOW_END || toId === FLOW_START || fromId === toId) return false
  const seen = new Set<string>()
  const stack = [toId]
  while (stack.length) {
    const id = stack.pop() as string
    if (id === fromId) return true
    if (seen.has(id)) continue
    seen.add(id)
    for (const edge of flow.edges) {
      if (edge.from === id && edge.to && edge.to !== FLOW_END) stack.push(edge.to)
    }
  }
  return false
}

export function connectForward(flow: FlowDef, fromId: string, toId: string): FlowDef {
  if (!fromId || fromId === toId) return flow
  if (fromId === FLOW_END || toId === FLOW_START) return flow
  if (flow.edges.some((edge) => edge.from === fromId && edge.to === toId)) return flow
  if (closesCycle(flow, fromId, toId)) {
    const existingLoop = flow.edges.find((edge) => edge.from === fromId && edge.gate === 'loop')
    if (existingLoop) {
      return {
        ...flow,
        edges: flow.edges.map((edge) => (
          edge.id === existingLoop.id ? { ...edge, to: toId, gate: 'loop', maxLoops: edge.maxLoops || MAX_LOOPS_DEFAULT } : edge
        ))
      }
    }
    return {
      ...flow,
      edges: [
        ...flow.edges,
        {
          id: makeId('e'),
          from: fromId,
          to: toId,
          mode: 'auto',
          transform: 'roleWrap',
          gate: 'loop',
          maxLoops: MAX_LOOPS_DEFAULT
        }
      ]
    }
  }
  const forward = flow.edges.find((edge) => edge.from === fromId && edge.gate !== 'loop')
  if (forward) {
    return {
      ...flow,
      edges: flow.edges.map((edge) => (edge.id === forward.id ? { ...edge, to: toId } : edge))
    }
  }
  return {
    ...flow,
    edges: [
      ...flow.edges,
      {
        id: makeId('e'),
        from: fromId,
        to: toId,
        mode: 'auto',
        transform: 'roleWrap',
        gate: 'none'
      }
    ]
  }
}

export function deleteEdge(flow: FlowDef, edgeId: string): FlowDef {
  return { ...flow, edges: flow.edges.filter((edge) => edge.id !== edgeId) }
}

export function terminals(flow: FlowDef) {
  const start = {
    x: Number.isFinite(flow.startX) ? flow.startX as number : defaultStartPoint().x,
    y: Number.isFinite(flow.startY) ? flow.startY as number : defaultStartPoint().y
  }
  let bottom = start.y + START_H
  const placed = withNodePoints(flow)
  for (const [index, node] of placed.nodes.entries()) {
    const point = nodePoint(node, index)
    bottom = Math.max(bottom, point.y + NODE_H)
  }
  const end = {
    x: Number.isFinite(flow.endX) ? flow.endX as number : NODE_X,
    y: Number.isFinite(flow.endY) ? flow.endY as number : bottom + NODE_GAP_Y
  }
  return { start, end, w: NODE_W, h: START_H }
}

export function placeTerminal(flow: FlowDef, which: 'start' | 'end', x: number, y: number): FlowDef {
  const next = { ...flow }
  const sx = Math.max(0, snap(x))
  const sy = Math.max(0, snap(y))
  if (which === 'start') {
    next.startX = sx
    next.startY = sy
  } else {
    next.endX = sx
    next.endY = sy
  }
  return next
}

export function hitTerminal(flow: FlowDef, x: number, y: number): 'start' | 'end' | null {
  const { start, end, w, h } = terminals(flow)
  if (x >= start.x && x <= start.x + w && y >= start.y && y <= start.y + h) return 'start'
  if (x >= end.x && x <= end.x + w && y >= end.y && y <= end.y + END_H) return 'end'
  return null
}

export function startAnchor(flow: FlowDef) {
  const { start, w } = terminals(flow)
  return { x: start.x + w / 2, y: start.y + START_H }
}

export function endAnchor(flow: FlowDef) {
  const { end, w } = terminals(flow)
  return { x: end.x + w / 2, y: end.y }
}

export function nodePoint(node: FlowNode, index = 0) {
  const fallback = defaultNodePoint(index)
  return {
    x: Number.isFinite(node.x) ? node.x as number : fallback.x,
    y: Number.isFinite(node.y) ? node.y as number : fallback.y
  }
}

export function bottomAnchor(node: FlowNode, index = 0) {
  const point = nodePoint(node, index)
  return { x: point.x + NODE_W / 2, y: point.y + NODE_H }
}

export function topAnchor(node: FlowNode, index = 0) {
  const point = nodePoint(node, index)
  return { x: point.x + NODE_W / 2, y: point.y }
}

export function rightAnchor(node: FlowNode, index = 0) {
  const point = nodePoint(node, index)
  return { x: point.x + NODE_W, y: point.y + NODE_H / 2 }
}

export function hitNode(flow: FlowDef, x: number, y: number) {
  const placed = withNodePoints(flow)
  for (let i = placed.nodes.length - 1; i >= 0; i -= 1) {
    const node = placed.nodes[i]
    const point = nodePoint(node, i)
    if (x >= point.x && x <= point.x + NODE_W && y >= point.y && y <= point.y + NODE_H) {
      return node
    }
  }
  return null
}

export function canvasSize(flow: FlowDef) {
  const placed = withNodePoints(flow)
  const { start, end } = terminals(placed)
  let width = NODE_X + NODE_W + 160
  let height = 200
  for (const [index, node] of placed.nodes.entries()) {
    const point = nodePoint(node, index)
    width = Math.max(width, point.x + NODE_W + 120)
    height = Math.max(height, point.y + NODE_H + 80)
  }
  width = Math.max(width, start.x + NODE_W + 80, end.x + NODE_W + 80)
  height = Math.max(height, start.y + START_H + 80, end.y + END_H + 80)
  return { width, height }
}

export function forwardPath(from: { x: number; y: number }, to: { x: number; y: number }) {
  const midY = (from.y + to.y) / 2
  return `M ${from.x} ${from.y} C ${from.x} ${midY}, ${to.x} ${midY}, ${to.x} ${to.y}`
}

export function backPath(from: { x: number; y: number }, to: { x: number; y: number }) {
  const bulge = Math.max(from.x, to.x) + 56
  return `M ${from.x} ${from.y} C ${bulge} ${from.y}, ${bulge} ${to.y}, ${to.x} ${to.y}`
}

export function pathMid(from: { x: number; y: number }, to: { x: number; y: number }) {
  return { x: (from.x + to.x) / 2, y: (from.y + to.y) / 2 }
}
