import { entryNode } from './engine.ts'
import { EXIT_HUMAN, passSkipped } from './exits.ts'
import type { FlowDef, FlowNode } from './types.ts'

export const MAX_PREFLIGHT_PROBLEMS = 3

export function selectedProfile(flow: FlowDef) {
  const names = Object.keys(flow.profiles ?? {})
  return flow.draftProfile && names.includes(flow.draftProfile) ? flow.draftProfile : names[0]
}

export function skippedBy(flow: FlowDef, profile: string | undefined) {
  return (profile && flow.profiles?.[profile]?.skip) || []
}

// Skipping is only defined along exits: a skipped node hands over through its PASS exit.
export function validateProfiles(flow: FlowDef): string | null {
  const profiles = Object.entries(flow.profiles ?? {})
  if (!profiles.length) return null
  const plain = flow.nodes.find((node) => !node.exits?.length)
  if (plain) return `用 profile 时每个节点都要声明出口，节点「${plain.title || plain.id}」没有`
  for (const [name, profile] of profiles) {
    for (const id of profile.skip) {
      if (!flow.nodes.some((node) => node.id === id)) return `profile「${name}」跳过了不存在的节点：${id}`
    }
  }
  return null
}

function nextNodes(flow: FlowDef, skip: string[], node: FlowNode) {
  if (node.exits?.length) return node.exits.map((exit) => passSkipped(flow, skip, exit.to))
  return flow.edges.filter((edge) => edge.from === node.id).flatMap((edge) => [edge.to, edge.backTo ?? ''])
}

export type Preflight = { problems: string[]; reached: FlowNode[] }

// Walks every route the run could take, so a file that only some routes produce
// is caught too. Going back to a node already on the route is a rework: by then
// everything before it exists.
export function preflight(flow: FlowDef, profile?: string): Preflight {
  const skip = skippedBy(flow, profile)
  const problems = new Set<string>()
  const reached = new Map<string, FlowNode>()
  const seen = new Set<string>()
  const owed = new Set(flow.nodes.filter((node) => !skip.includes(node.id)).flatMap((node) => (node.outputs ?? []).map((row) => row.path)))

  const check = (node: FlowNode, produced: Set<string>) => {
    for (const input of node.inputs ?? []) {
      if (!input.required || !input.ref.startsWith('task:')) continue
      if (input.when === 'reworked' ? owed.has(input.ref) : produced.has(input.ref)) continue
      problems.add(`节点「${node.title || node.id}」必读 ${input.ref}，但没有上游节点产出它`)
    }
  }
  const visit = (id: string, route: string[], produced: Set<string>) => {
    const node = flow.nodes.find((item) => item.id === id)
    if (!node || route.includes(id)) return
    const key = `${id}|${[...produced].sort().join()}`
    if (seen.has(key)) return
    seen.add(key)
    reached.set(id, node)
    check(node, produced)
    const after = new Set([...produced, ...(node.outputs ?? []).map((row) => row.path)])
    for (const next of nextNodes(flow, skip, node)) visit(next, [...route, id], after)
  }

  const first = entryNode(flow)
  const start = first ? passSkipped(flow, skip, first.id) : ''
  if (start === EXIT_HUMAN || (first && !flow.nodes.some((node) => node.id === start))) {
    problems.add('这个 profile 没有可以开始的节点')
  }
  visit(start, [], new Set())
  return { problems: [...problems], reached: [...reached.values()] }
}

export function preflightMessage(problems: string[]) {
  const shown = problems.slice(0, MAX_PREFLIGHT_PROBLEMS).join('；')
  const rest = problems.length - MAX_PREFLIGHT_PROBLEMS
  return `启动前检查没通过：${shown}${rest > 0 ? `；还有 ${rest} 处` : ''}`
}
