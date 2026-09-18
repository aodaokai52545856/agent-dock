export type FlowInspectorKind = 'node' | 'edge' | 'run' | ''

export function flowInspectorKind(
  paneMode: 'edit' | 'run',
  nodeId: string,
  edgeId: string
): FlowInspectorKind {
  if (paneMode === 'run') return 'run'
  if (edgeId) return 'edge'
  if (nodeId) return 'node'
  return ''
}

export function flowInspectorTitle(kind: FlowInspectorKind) {
  if (kind === 'edge') return '连线'
  if (kind === 'node') return '节点'
  if (kind === 'run') return '运行'
  return ''
}
