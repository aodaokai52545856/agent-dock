import { parseReviewVerdict } from '../reviewVerdict.ts'
import type { ReviewVerdict } from '../types.ts'
import { channelSupportsAuto, ptyToolLabel } from './channels.ts'
import type { ChannelBinding, FlowDef, FlowNode, NodeOutput } from './types.ts'

export const DOCK_DIR = '.agentDock'
const CONCLUSION = '## 结论'
const PLACEHOLDER = /\{\{[^{}\n]*\}\}/

export type ResolvedRef = {
  // Relative to `.agentDock/`, as the file commands take it.
  rel: string
  // Relative to the project root, as an agent reads it.
  path: string
}

export type InputBlock = { path: string; read?: 'index-first'; text?: string }
export type OutputBlock = {
  path: string
  capture: 'file' | 'reply'
  sections: string[]
  template?: string
}

export function resolveRef(ref: string, taskDir: string): ResolvedRef | null {
  const match = /^(task|project):(.+)$/.exec(ref.trim())
  if (!match) return null
  const name = match[2]
  if (name.split(/[\\/]/).some((part) => !part || part === '.' || part === '..')) return null
  const rel = match[1] === 'task' ? `tasks/${taskDir}/${name}` : name
  return { rel, path: `${DOCK_DIR}/${rel}` }
}

export function nodeHasContract(node: FlowNode) {
  return Boolean(node.inputs?.length || node.outputs?.length)
}

export function flowHasContract(flow: FlowDef) {
  return Boolean(flow.experience) || flow.nodes.some(nodeHasContract)
}

// Unverified channels get file contents inlined and their reply captured, so a
// tool that cannot reach `.agentDock/` still works.
export function channelReadsFiles(channel: ChannelBinding) {
  return channel.kind !== 'cursorSdk'
}

export function outputCapture(channel: ChannelBinding, output: NodeOutput): 'file' | 'reply' {
  if (channel.kind !== 'pty' && channel.kind !== 'ompRpc') return 'reply'
  return output.capture ?? 'file'
}

export function templateRel(node: FlowNode, output: NodeOutput) {
  return output.template ? `roles/${node.role}/${output.template}` : ''
}

export function validateContract(flow: FlowDef): string | null {
  for (const node of flow.nodes) {
    if (node.channel.kind === 'human' && nodeHasContract(node)) {
      return `节点「${node.title}」是人工节点，不能声明输入输出`
    }
    if (node.outputs?.length && node.channel.kind === 'pty' && !channelSupportsAuto(node.channel)) {
      return `节点「${node.title}」用的 ${ptyToolLabel(node.channel.toolId)} 还不能确认一轮是否结束，不能声明输出`
    }
    for (const input of node.inputs ?? []) {
      if (!resolveRef(input.ref, 'task')) return `节点「${node.title}」的输入写法不对：${input.ref}`
    }
    for (const output of node.outputs ?? []) {
      if (!output.path.startsWith('task:') || !resolveRef(output.path, 'task')) {
        return `节点「${node.title}」的输出只能写到 task: 下：${output.path}`
      }
    }
  }
  return null
}

export function taskDirName(task: string, at: number, runId: string) {
  const date = new Date(at)
  const pad = (value: number) => String(value).padStart(2, '0')
  const stamp = `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}-${pad(date.getHours())}${pad(date.getMinutes())}`
  const slug = task.trim().replace(/[^\p{L}\p{N}]+/gu, '-').replace(/^-+/, '').slice(0, 32).replace(/-+$/, '')
  const suffix = runId.replace(/[^a-zA-Z0-9]/g, '').slice(-4)
  return [stamp, slug, suffix].filter(Boolean).join('-')
}

export function renderContractBlock(inputs: InputBlock[], outputs: OutputBlock[]) {
  const lines: string[] = []
  const listed = inputs.filter((item) => item.text === undefined)
  if (listed.length) {
    lines.push('开始前先读完这些文件：')
    for (const item of listed) {
      lines.push(`- ${item.path}${item.read === 'index-first' ? '（这是索引：先扫一遍，只读命中当前范围的条目）' : ''}`)
    }
  }
  for (const item of inputs.filter((row) => row.text !== undefined)) {
    lines.push(`文件 ${item.path} 的内容：`, '<file>', item.text ?? '', '</file>')
  }
  for (const item of outputs) {
    const required = item.sections.length ? `必须包含：${item.sections.join('、')}。` : ''
    const template = item.template ? `按模板 ${item.template} 的结构写。` : ''
    if (item.capture === 'file') {
      lines.push(`把产物写到 ${item.path}。${template}填完所有占位符，${required}`)
    } else {
      lines.push(`在回复里给出完整产物，Dock 会保存到 ${item.path}。${template}${required}`)
    }
  }
  return lines.join('\n')
}

export function sectionBody(text: string, heading: string) {
  const lines = text.split(/\r?\n/)
  const level = /^#+/.exec(heading)?.[0].length ?? 0
  const start = lines.findIndex((line) => line.trim() === heading)
  if (start < 0) return null
  const body: string[] = []
  for (const line of lines.slice(start + 1)) {
    const depth = /^(#+)\s/.exec(line)?.[1].length ?? 0
    if (depth && depth <= level) break
    body.push(line)
  }
  return body.join('\n').trim()
}

export function validateOutputText(path: string, text: string, sections: string[]): string | null {
  if (!text.trim()) return `产物 ${path} 是空的`
  const placeholder = PLACEHOLDER.exec(text)
  if (placeholder) return `产物 ${path} 还有模板占位符：${placeholder[0]}`
  for (const heading of sections) {
    const body = sectionBody(text, heading)
    if (body === null) return `产物 ${path} 缺少段落：${heading}`
    if (!body) return `产物 ${path} 的段落是空的：${heading}`
  }
  return null
}

export function verdictFromDocument(text: string): ReviewVerdict | undefined {
  const body = sectionBody(text, CONCLUSION)
  const verdict = body ? parseReviewVerdict(body) : 'unknown'
  return verdict === 'unknown' ? undefined : verdict
}
