import { EXIT_END, EXIT_HUMAN } from './exits.ts'
import { FLOW_END, FLOW_START, type ConclusionOn, type FlowDef, type FlowEdge, type FlowNode, type NodeExit } from './types.ts'

export const CONCLUSION_OPTIONS: ConclusionOn[] = ['PASS', 'FAIL', 'REJECT', 'BLOCK']

// A flow routes by exits unless a node still hands over along an old edge. A new,
// empty flow starts with exits.
export function routesByExits(flow: FlowDef) {
  return !flow.edges.some((edge) => edge.from !== FLOW_START && !flow.nodes.find((node) => node.id === edge.from)?.exits?.length)
}

function withExits(flow: FlowDef, nodeId: string, exits: NodeExit[]): FlowDef {
  return { ...flow, nodes: flow.nodes.map((node) => (node.id === nodeId ? { ...node, exits: exits.length ? exits : undefined } : node)) }
}

// Dragging a wire adds the first exit the node lacks: PASS, then the ways back.
export function connectExit(flow: FlowDef, fromId: string, toId: string): FlowDef {
  const node = flow.nodes.find((item) => item.id === fromId)
  if (!node || fromId === toId) return flow
  const to = toId === FLOW_END ? EXIT_END : toId
  const exits = node.exits ?? []
  if (exits.some((exit) => exit.to === to)) return flow
  const on = exits.some((exit) => exit.on === 'PASS') ? CONCLUSION_OPTIONS.find((item) => item !== 'PASS' && !exits.some((exit) => exit.on === item && !exit.owner)) : 'PASS'
  if (!on || (to === EXIT_END && on !== 'PASS')) return flow
  return withExits(flow, fromId, [...exits, { on, to }])
}

export function setNodeExits(flow: FlowDef, nodeId: string, exits: NodeExit[]): FlowDef {
  return withExits(flow, nodeId, exits)
}

// Removing a node takes every reference to it along, or the flow would be refused.
export function forgetNode(flow: FlowDef, nodeId: string): FlowDef {
  const nodes = flow.nodes.map((node) => {
    const exits = node.exits?.filter((exit) => exit.to !== nodeId)
    return node.exits ? { ...node, exits: exits?.length ? exits : undefined } : node
  })
  const profiles = flow.profiles
    ? Object.fromEntries(Object.entries(flow.profiles).map(([name, row]) => [name, { skip: row.skip.filter((id) => id !== nodeId) }]))
    : undefined
  const experience = flow.experience
    ? {
        ...flow.experience,
        authors: flow.experience.authors.filter((id) => id !== nodeId),
        judge: flow.experience.judge === nodeId ? undefined : flow.experience.judge
      }
    : undefined
  return { ...flow, nodes, profiles, experience }
}

export type ExitWire = {
  id: string
  fromId: string
  // A node id or FLOW_END. Exits to a person have no wire.
  to: string
  label: string
  back: boolean
}

export function exitLabel(exit: NodeExit) {
  return [exit.on, exit.owner].filter(Boolean).join(' ')
}

export function exitWires(flow: FlowDef): ExitWire[] {
  const order = new Map(flow.nodes.map((node, index) => [node.id, index]))
  return flow.nodes.flatMap((node, index) => (node.exits ?? []).flatMap((exit, slot) => {
    if (exit.to === EXIT_HUMAN) return []
    const to = exit.to === EXIT_END ? FLOW_END : exit.to
    if (to !== FLOW_END && !order.has(to)) return []
    const back = to !== FLOW_END && (order.get(to) ?? 0) <= index
    return [{ id: `exit:${node.id}:${slot}`, fromId: node.id, to, label: exitLabel(exit), back }]
  }))
}

export function humanExits(node: FlowNode) {
  return (node.exits ?? []).filter((exit) => exit.to === EXIT_HUMAN).map(exitLabel)
}

export type Conversion = { flow: FlowDef; notes: string[] } | { problems: string[] }

function nodeName(node: FlowNode) {
  return `「${node.title || node.id}」`
}

function exitsFromEdges(node: FlowNode, out: FlowEdge[]): NodeExit[] | string {
  if (out.some((edge) => edge.mode === 'manual')) return `${nodeName(node)}有手动桥接的连线，出口没有手动确认这一步`
  if (out.some((edge) => edge.gate === 'loop')) return `${nodeName(node)}有循环连线，出口不能按次数循环后再往前走`
  if (out.length > 1) return `${nodeName(node)}有多条往前的连线`
  const edge = out[0]
  if (!edge) return [{ on: 'PASS', to: EXIT_END }]
  const to = edge.to === FLOW_END || !edge.to ? EXIT_END : edge.to
  const exits: NodeExit[] = [{ on: 'PASS', to }]
  if (edge.gate === 'passFail' && edge.backTo) exits.push({ on: 'FAIL', to: edge.backTo })
  return exits
}

// Only shapes that keep their meaning are converted; anything else is reported
// and the flow stays as it was.
export function convertToExits(flow: FlowDef): Conversion {
  const problems: string[] = []
  const exits = new Map<string, NodeExit[]>()
  for (const node of flow.nodes) {
    if (node.exits?.length) continue
    const row = exitsFromEdges(node, flow.edges.filter((edge) => edge.from === node.id))
    if (typeof row === 'string') problems.push(row)
    else exits.set(node.id, row)
  }
  if (problems.length) return { problems }
  const notes = ['转换后每个节点都要写结论（VERDICT 一行），读不出结论会暂停等人工，不再走手动桥接']
  if (flow.edges.some((edge) => edge.gate === 'passFail')) {
    notes.push(`返工次数改为按节点计算，每个节点最多 ${(flow.maxRetries || 0) + 1} 次，用完暂停等人工，不再直接失败`)
  }
  return {
    notes,
    flow: {
      ...flow,
      nodes: flow.nodes.map((node) => (exits.has(node.id) ? { ...node, exits: exits.get(node.id) } : node)),
      edges: flow.edges.filter((edge) => edge.from === FLOW_START)
    }
  }
}
