export type FlowInspectorKind = 'node' | 'edge' | 'run' | 'flow' | ''

export function flowInspectorKind(
  paneMode: 'edit' | 'run',
  nodeId: string,
  edgeId: string,
  flowSettings = false
): FlowInspectorKind {
  if (paneMode === 'run') return 'run'
  if (edgeId) return 'edge'
  if (nodeId) return 'node'
  if (flowSettings) return 'flow'
  return ''
}

export function flowInspectorTitle(kind: FlowInspectorKind) {
  if (kind === 'edge') return '连线'
  if (kind === 'node') return '节点'
  if (kind === 'run') return '运行'
  if (kind === 'flow') return '流程设置'
  return ''
}
