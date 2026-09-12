import { parseReviewVerdict } from '../reviewVerdict.ts'
import { makeId } from './ids.ts'
import { roleLabel } from './roles.ts'
import {
  ARTIFACT_CHAR_CAP,
  type AdapterOutcome,
  type Envelope,
  type FlowDef,
  type FlowEdge,
  type FlowNode,
  type FlowRun,
  type RoleId
} from './types.ts'

function cap(text: string) {
  if (text.length <= ARTIFACT_CHAR_CAP) return text
  return text.slice(0, ARTIFACT_CHAR_CAP)
}

export function emptyEnvelope(partial: Partial<Envelope> & Pick<Envelope, 'runId' | 'toNode' | 'toRole'>): Envelope {
  return {
    id: partial.id || makeId('env'),
    runId: partial.runId,
    causationId: partial.causationId || '',
    fromNode: partial.fromNode || '',
    toNode: partial.toNode,
    fromRole: partial.fromRole || 'pm',
    toRole: partial.toRole,
    task: partial.task || '',
    done: partial.done || '',
    remaining: partial.remaining || '',
    blockers: partial.blockers ? [...partial.blockers] : [],
    artifacts: (partial.artifacts || []).map((item) => ({ ...item, body: cap(item.body) })),
    verdict: partial.verdict
  }
}

export function initialEnvelope(run: FlowRun, first: FlowNode): Envelope {
  return emptyEnvelope({
    runId: run.id,
    toNode: first.id,
    fromRole: 'pm',
    toRole: first.role,
    task: run.task,
    remaining: run.task,
    artifacts: run.task.trim() ? [{ kind: 'text', body: run.task }] : []
  })
}

export function envelopeFromOutcome(run: FlowRun, node: FlowNode, outcome: AdapterOutcome): Envelope {
  const text = cap(outcome.text || '')
  const verdict = outcome.verdict ?? (node.role === 'reviewer' ? parseReviewVerdict(text) : undefined)
  return emptyEnvelope({
    runId: run.id,
    causationId: run.lastEnvelope?.id || '',
    fromNode: node.id,
    toNode: node.id,
    fromRole: node.role,
    toRole: node.role,
    task: run.task,
    done: text,
    remaining: '',
    artifacts: text ? [{ kind: 'text', body: text }] : [],
    verdict
  })
}

export function transformEnvelope(_edge: FlowEdge, envelope: Envelope, flow: FlowDef, toNodeId: string): Envelope {
  const toNode = flow.nodes.find((item) => item.id === toNodeId)
  const toRole: RoleId = toNode?.role || envelope.toRole
  return emptyEnvelope({
    ...envelope,
    id: makeId('env'),
    causationId: envelope.id,
    fromNode: envelope.fromNode || envelope.toNode,
    toNode: toNodeId,
    fromRole: envelope.fromRole,
    toRole,
    remaining: envelope.remaining || envelope.task
  })
}

export function renderEnvelope(envelope: Envelope, opts?: { transform?: FlowEdge['transform']; contract?: string }) {
  if (opts?.transform === 'verbatim') {
    const body = envelope.artifacts.map((item) => item.body).join('\n\n').trim()
    return body || envelope.done || envelope.task
  }
  const from = envelope.fromNode ? roleLabel(envelope.fromRole) : '编排'
  const to = roleLabel(envelope.toRole)
  const lines = [`【编排接力 · ${from} → ${to}】`]
  if (envelope.task.trim()) {
    lines.push(`本轮任务：${envelope.task.trim()}`)
  }
  if (envelope.done.trim()) {
    lines.push(`上一节点做了：${envelope.done.trim()}`)
  }
  if (envelope.remaining.trim() && envelope.remaining.trim() !== envelope.task.trim()) {
    lines.push(`还剩：${envelope.remaining.trim()}`)
  }
  if (envelope.blockers.length) {
    lines.push(`阻塞：${envelope.blockers.join('；')}`)
  }
  if (envelope.verdict) {
    const ticket = envelope.verdict === 'pass' ? 'PASS' : envelope.verdict === 'fail' ? 'FAIL' : 'UNKNOWN'
    lines.push(`结论：${ticket}`)
  }
  if (opts?.contract?.trim()) {
    lines.push(`目标角色合同：${opts.contract.trim()}`)
  }
  const body = envelope.artifacts.map((item) => item.body).join('\n\n').trim()
  if (body && body !== envelope.done.trim()) {
    lines.push('---', body)
  } else if (body && !envelope.done.trim()) {
    lines.push('---', body)
  }
  return lines.join('\n')
}

export function promptFromEnvelope(node: FlowNode, envelope: Envelope) {
  const rendered = renderEnvelope(envelope, { transform: 'roleWrap', contract: node.roleContract })
  if (node.role === 'developer') {
    const body = [
      '你是本仓库的开发者。只改当前这一片需要的文件，做完后停下来，不要开新需求。',
      node.roleContract,
      '',
      '本轮任务：',
      envelope.task.trim() || '按工作区未提交改动继续推进，不要扩大范围。'
    ]
    if (envelope.done.trim()) {
      body.push('', '上一轮意见：', envelope.done.trim())
    }
    return body.join('\n')
  }
  if (node.role === 'reviewer') {
    return [node.roleContract, rendered].filter(Boolean).join('\n\n')
  }
  return rendered
}
