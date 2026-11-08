import type { FlowDef } from './types.ts'

export const SCRIPT_APPROVAL_KEY = 'ad-flow-script-approvals'
export const SCRIPT_TIMEOUT_DEFAULT_SEC = 600

export type ScriptRow = { nodeId: string; title: string; command: string }
export type Approvals = Record<string, string[]>
type Storage = Pick<globalThis.Storage, 'getItem' | 'setItem'>

export function flowScripts(flow: FlowDef): ScriptRow[] {
  return flow.nodes.flatMap((node) => (
    node.channel.kind === 'script'
      ? [{ nodeId: node.id, title: node.title || node.id, command: node.channel.command }]
      : []
  ))
}

// Approvals live with the app, never in the repository: a cloned project must not
// be able to mark its own commands as already reviewed. The exact text is what
// was reviewed, so any edit asks again.
export function isScriptApproved(approvals: Approvals, projectId: string, command: string) {
  return Boolean(approvals[projectId]?.includes(command))
}

export function unapprovedScripts(flow: FlowDef, approvals: Approvals, projectId: string) {
  return flowScripts(flow).filter((row) => !isScriptApproved(approvals, projectId, row.command))
}

export function withApprovedScripts(approvals: Approvals, projectId: string, commands: string[]): Approvals {
  return { ...approvals, [projectId]: [...new Set([...(approvals[projectId] ?? []), ...commands])] }
}

export function loadApprovals(storage: Storage): Approvals {
  try {
    const row = JSON.parse(storage.getItem(SCRIPT_APPROVAL_KEY) || '{}')
    return row && typeof row === 'object' ? row : {}
  } catch {
    return {}
  }
}

export function saveApprovals(storage: Storage, approvals: Approvals) {
  try {
    storage.setItem(SCRIPT_APPROVAL_KEY, JSON.stringify(approvals))
  } catch {
    /* an approval that was not saved is asked for again next time */
  }
}

// A failing exit code has to go somewhere, or a broken build would pass silently.
export function validateScripts(flow: FlowDef): string | null {
  for (const node of flow.nodes) {
    if (node.channel.kind !== 'script') continue
    const name = `节点「${node.title || node.id}」`
    if (!node.channel.command.trim()) return `${name}是脚本节点，但没有写命令`
    const gated = flow.edges.some((edge) => edge.from === node.id && edge.gate === 'passFail')
    if (!node.exits?.length && !gated) return `${name}是脚本节点，要声明出口，否则失败的退出码没有去处`
  }
  return null
}
