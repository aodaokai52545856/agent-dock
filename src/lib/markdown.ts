import { marked, Renderer } from 'marked'
import type { Tokens } from 'marked'

marked.setOptions({
  gfm: true,
  breaks: true
})

const DIAGRAM_LANGS = new Set([
  'mermaid',
  'flowchart',
  'flow',
  'graph',
  'sequence',
  'sequencediagram',
  'classdiagram',
  'statediagram',
  'statediagramv2',
  'erdiagram',
  'gantt',
  'pie',
  'gitgraph',
  'mindmap',
  'timeline',
  'journey',
  'quadrantchart',
  'requirementdiagram',
  'c4',
  'block',
  'blockbeta',
  'packet',
  'architecture',
  'kanban',
  'sankey',
  'xychart',
  'radar'
])

const DIAGRAM_STARTER =
  /^(flowchart|graph|sequenceDiagram|classDiagram|stateDiagram(?:-v2)?|erDiagram|gantt|pie|gitGraph|mindmap|timeline|journey|quadrantChart|requirementDiagram|C4Context|block(?:-beta)?|packet(?:-beta)?|architecture(?:-beta)?|kanban|sankey(?:-beta)?|xychart(?:-beta)?|radar(?:-beta)?)\b/i

function langKey(lang?: string | null) {
  return (lang || '').trim().toLowerCase().replace(/[\s_-]/g, '')
}

export function isDiagramLang(lang?: string | null) {
  if (!lang) return false
  return DIAGRAM_LANGS.has(langKey(lang))
}

export function mermaidSource(lang: string | undefined, text: string) {
  const body = (text || '').replace(/^\uFEFF/, '').trim()
  const key = langKey(lang)
  if (!key || key === 'mermaid') return body
  if (DIAGRAM_STARTER.test(body)) return body
  if (key === 'flowchart' || key === 'flow' || key === 'graph') {
    if (/^(TD|TB|BT|RL|LR)\b/i.test(body)) return `flowchart ${body}`
    return `flowchart TD\n${body}`
  }
  const prefix: Record<string, string> = {
    sequence: 'sequenceDiagram',
    sequencediagram: 'sequenceDiagram',
    classdiagram: 'classDiagram',
    statediagram: 'stateDiagram-v2',
    statediagramv2: 'stateDiagram-v2',
    erdiagram: 'erDiagram',
    gantt: 'gantt',
    pie: 'pie',
    gitgraph: 'gitGraph',
    mindmap: 'mindmap',
    timeline: 'timeline',
    journey: 'journey',
    quadrantchart: 'quadrantChart',
    requirementdiagram: 'requirementDiagram'
  }
  const head = prefix[key]
  return head ? `${head}\n${body}` : body
}

export function escapeHtml(value: string) {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

export function sanitizePreviewHtml(html: string) {
  return html
    .replace(/<script[\s\S]*?>[\s\S]*?<\/script>/gi, '')
    .replace(/<style[\s\S]*?>[\s\S]*?<\/style>/gi, '')
    .replace(/<\/?(iframe|object|embed|link|meta)\b[\s\S]*?>/gi, '')
    .replace(/\son\w+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)/gi, '')
    .replace(/javascript:/gi, '')
}

const renderer = new Renderer()
const defaultCode = renderer.code.bind(renderer)
renderer.code = (token: Tokens.Code) => {
  if (isDiagramLang(token.lang)) {
    const source = mermaidSource(token.lang, token.text)
    return `<pre class="ad-mermaid" data-source="${encodeURIComponent(source)}">${escapeHtml(source)}</pre>\n`
  }
  return defaultCode(token)
}
marked.use({ renderer })

export function renderMarkdown(source: string) {
  const html = marked.parse(source ?? '', { async: false })
  return sanitizePreviewHtml(typeof html === 'string' ? html : String(html))
}
