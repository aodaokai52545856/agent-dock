import { defaultChannel, defaultCodexChannel, defaultPtyChannel } from './channels.ts'
import { makeId } from './ids.ts'
import { ROLE_CONTRACTS } from './roles.ts'
import {
  FLOW_END,
  MAX_RETRIES_DEFAULT,
  MAX_STEPS_DEFAULT,
  type FlowDef,
  type FlowNode,
  type FlowTemplateId
} from './types.ts'

function node(role: FlowNode['role'], title: string, channel: FlowNode['channel']): FlowNode {
  return {
    id: makeId('n'),
    title,
    role,
    roleContract: ROLE_CONTRACTS[role],
    channel
  }
}

export function emptyFlow(name: string, templateId?: FlowTemplateId): FlowDef {
  return {
    id: makeId('flow'),
    name,
    templateId,
    nodes: [],
    edges: [],
    maxRetries: MAX_RETRIES_DEFAULT,
    maxSteps: MAX_STEPS_DEFAULT,
    slice: 1,
    draftTask: ''
  }
}

export function createDevReviewFlow(): FlowDef {
  const dev = node('developer', '开发者', defaultChannel('developer'))
  const rev = node('reviewer', '审查者', defaultCodexChannel())
  const flow = emptyFlow('开发 ↔ 审查', 'dev-review')
  flow.nodes = [dev, rev]
  flow.edges = [
    {
      id: makeId('e'),
      from: dev.id,
      to: rev.id,
      mode: 'auto',
      transform: 'roleWrap',
      gate: 'none'
    },
    {
      id: makeId('e'),
      from: rev.id,
      to: FLOW_END,
      mode: 'auto',
      transform: 'roleWrap',
      gate: 'passFail',
      backTo: dev.id
    }
  ]
  return flow
}

export function createCodexGrokFlow(): FlowDef {
  const rev = node('reviewer', '审查者', defaultCodexChannel())
  const grok = node('developer', '开发者', defaultPtyChannel('grokbuild'))
  const flow = emptyFlow('Codex → Grok', 'codex-grok')
  flow.nodes = [rev, grok]
  flow.edges = [
    {
      id: makeId('e'),
      from: rev.id,
      to: grok.id,
      mode: 'manual',
      transform: 'roleWrap',
      gate: 'none'
    }
  ]
  return flow
}

export function createBlankFlow(): FlowDef {
  const start = node('pm', '项目经理', defaultChannel('pm'))
  const flow = emptyFlow('空白流程', 'blank')
  flow.nodes = [start]
  return flow
}

export function flowFromTemplate(id: FlowTemplateId): FlowDef {
  if (id === 'codex-grok') return createCodexGrokFlow()
  if (id === 'blank') return createBlankFlow()
  return createDevReviewFlow()
}

export const FLOW_TEMPLATES: { id: FlowTemplateId; name: string; hint: string }[] = [
  { id: 'dev-review', name: '开发 ↔ 审查', hint: '开发完成后审查，未通过回到开发者' }
]
