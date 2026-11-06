// Presentation only: keep the original RPC messages intact for orchestration.
type RecordValue = Record<string, any>
export type OmpToolView = {
  id: string
  name: string
  status: 'running' | 'complete' | 'error' | 'ended'
  label: string
  description: string
  command: string
  path: string
  arguments: string
  output: string
  outputLanguage: string
  images: RecordValue[]
}

const textContent = (content: any): string => typeof content === 'string' ? content :
  Array.isArray(content) ? content.filter(p => p.type === 'text').map(p => p.text || '').join('\n') : ''

export function languageForPath(path: string): string {
  const extension = path.split('.').pop()?.toLowerCase() || ''
  return ({ts:'typescript',tsx:'typescript',js:'javascript',jsx:'javascript',mjs:'javascript',cjs:'javascript',
    py:'python',rs:'rust',java:'java',go:'go',json:'json',jsonl:'json',yaml:'yaml',yml:'yaml',
    css:'css',scss:'scss',html:'xml',vue:'xml',xml:'xml',sh:'bash',bash:'bash',ps1:'powershell',
    sql:'sql',md:'markdown',diff:'diff',patch:'diff',toml:'ini'} as Record<string,string>)[extension] || 'plaintext'
}

export function presentOmpTool(part: RecordValue, event: RecordValue | undefined, result: RecordValue | undefined, settled: boolean): OmpToolView {
  const args = part.arguments && typeof part.arguments === 'object' ? part.arguments : {}
  // History has toolResult messages, while a live run also has execution events.
  // Prefer the history result once available, so refreshing does not lose completion.
  const payload = result || event?.result || event?.partialResult
  const failed = result?.isError || event?.isError || payload?.isError
  const complete = Boolean(result || event?.type === 'tool_execution_end')
  const status = failed ? 'error' : complete ? 'complete' : settled ? 'ended' : 'running'
  const name = String(part.name || result?.toolName || '工具')
  const command = typeof args.command === 'string' ? args.command : ''
  const path = String(args.path || args.file_path || args.filePath || '')
  let output = textContent(payload?.content ?? payload)
  if (!output && typeof payload?.output === 'string') output = payload.output
  return {
    id: String(part.id || result?.toolCallId || ''), name, status,
    label: {running:'执行中',complete:'已完成',error:'失败',ended:'已结束'}[status],
    description: String(args.description || args.i || ''), command, path,
    arguments: JSON.stringify(part.arguments ?? {}, null, 2), output,
    outputLanguage: /^(read|read_file|cat)$/i.test(name) ? languageForPath(path) : /^(edit|apply_patch|patch)$/i.test(name) ? 'diff' : 'plaintext',
    images: Array.isArray(payload?.content) ? payload.content.filter((p: RecordValue) => p.type === 'image' && p.data) : []
  }
}

export function presentOmpMessages(messages: RecordValue[], tools: Record<string, RecordValue>, settled: boolean): (RecordValue & {parts:RecordValue[]})[] {
  const results = new Map<string, RecordValue>()
  const calls = new Set<string>()
  for (const message of messages) {
    if (message.role === 'toolResult' && message.toolCallId) results.set(message.toolCallId, message)
    if (Array.isArray(message.content)) for (const part of message.content) {
      if (part.type === 'toolCall' && part.id) calls.add(part.id)
    }
  }
  return messages.filter(message => message.role !== 'toolResult' || !calls.has(message.toolCallId)).map(message => {
    const content = message.role === 'compactionSummary' ? message.summary || textContent(message.content) : message.content
    const parts: RecordValue[] = typeof content === 'string' ? [{type:'text', text:content}] : Array.isArray(content) ? content : []
    return {...message, parts: parts.map(part => part.type === 'toolCall' ?
      {...part, tool:presentOmpTool(part, tools[part.id], results.get(part.id), settled)} : part)}
  })
}
