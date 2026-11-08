import { sectionBody } from './sections.ts'
import type { Conclusion, ConclusionOn, FlowDef, FlowNode, NodeExit } from './types.ts'

export const EXIT_HUMAN = '@human'
export const EXIT_END = '@end'

const CONCLUSIONS: ConclusionOn[] = ['PASS', 'FAIL', 'REJECT', 'BLOCK']
const CONCLUSION_HEADING = '## 结论'
// PASS takes no owner: anything after it is a caveat, and a caveat is not a pass.
const VERDICT_LINE = /^VERDICT\s*[:：]\s*(?:(PASS)|(FAIL|REJECT|BLOCK)(?:\s+(?:owner\s*[=:：]\s*)?([\w-]+))?)$/i
const VERDICT_START = /^VERDICT\b/i
const OWNER = /^[\w-]+$/
const REPLY_TAIL_LINES = 5

function parseLine(line: string): Conclusion | null {
  const match = VERDICT_LINE.exec(line)
  if (!match) return null
  const on = (match[1] ?? match[2]).toUpperCase() as ConclusionOn
  return match[3] ? { on, owner: match[3] } : { on }
}

// Every line that means to state a verdict, null for one that cannot be read. An
// unreadable one must not be dropped: next to a clean PASS it is the agent's doubt.
function statedVerdicts(text: string) {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim().replace(/^[`*_\s]+|[`*_\s]+$/g, ''))
    .filter((line) => VERDICT_START.test(line))
    .map(parseLine)
}

const key = (row: Conclusion) => `${row.on} ${row.owner ?? ''}`

// A document may repeat itself, but it cannot say two different things. Null means
// nobody can tell what was concluded, which only a person can settle.
export function readConclusion(documents: string[], reply: string): Conclusion | null {
  const written = documents.flatMap((text) => statedVerdicts(sectionBody(text, CONCLUSION_HEADING) ?? ''))
  if (written.length) {
    const readable = written.filter((row): row is Conclusion => Boolean(row))
    return readable.length === written.length && new Set(readable.map(key)).size === 1 ? readable[0] : null
  }
  const tail = reply.split(/\r?\n/).filter((line) => line.trim()).slice(-REPLY_TAIL_LINES).join('\n')
  const said = statedVerdicts(tail)
  return said.includes(null) ? null : said.at(-1) ?? null
}

export function pickExit(exits: NodeExit[], conclusion: Conclusion): NodeExit | null {
  const same = exits.filter((exit) => exit.on === conclusion.on)
  return same.find((exit) => exit.owner && exit.owner === conclusion.owner) ?? same.find((exit) => !exit.owner) ?? null
}

// A skipped node hands over through its PASS exit. Nodes that only skip into each
// other have nowhere left to go, which is a person's call.
export function passSkipped(flow: FlowDef, skip: string[], to: string) {
  const seen = new Set<string>()
  let next = to
  while (skip.includes(next)) {
    if (seen.has(next)) return EXIT_HUMAN
    seen.add(next)
    next = flow.nodes.find((node) => node.id === next)?.exits?.find((exit) => exit.on === 'PASS')?.to ?? EXIT_HUMAN
  }
  return next
}

export function exitTicket(node: FlowNode) {
  const rows = (node.exits ?? []).map((exit) => `VERDICT: ${exit.on}${exit.owner ? ` ${exit.owner}` : ''}`)
  return `结束时单独一行写结论，只能是其中之一：${[...new Set(rows)].join('、')}。`
}

function exitProblem(flow: FlowDef, exit: NodeExit): string | null {
  if (!CONCLUSIONS.includes(exit.on)) return `结论只能是 ${CONCLUSIONS.join('、')}：${exit.on}`
  if (exit.owner && exit.on === 'PASS') return `PASS 不能带责任方：${exit.owner}`
  if (exit.owner && !OWNER.test(exit.owner)) return `责任方只能用字母、数字、下划线或连字符：${exit.owner}`
  if (exit.to === EXIT_END) return exit.on === 'PASS' ? null : `只有 PASS 能走到 ${EXIT_END}：${exit.on}`
  if (exit.to === EXIT_HUMAN) return null
  return flow.nodes.some((item) => item.id === exit.to) ? null : `出口指向不存在的节点：${exit.to}`
}

export function validateExits(flow: FlowDef): string | null {
  for (const node of flow.nodes) {
    const exits = node.exits
    if (!exits?.length) continue
    const name = `节点「${node.title || node.id}」`
    if (flow.edges.some((edge) => edge.from === node.id)) return `${name}同时有出口和连线，只能留一种`
    if (!exits.some((exit) => exit.on === 'PASS')) return `${name}没有 PASS 出口`
    if (new Set(exits.map(key)).size !== exits.length) return `${name}有重复的出口`
    for (const exit of exits) {
      const problem = exitProblem(flow, exit)
      if (problem) return `${name}${problem}`
    }
  }
  return null
}
