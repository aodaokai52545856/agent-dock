import { marked } from 'marked'

marked.setOptions({
  gfm: true,
  breaks: true
})

export function sanitizePreviewHtml(html: string) {
  return html
    .replace(/<script[\s\S]*?>[\s\S]*?<\/script>/gi, '')
    .replace(/<style[\s\S]*?>[\s\S]*?<\/style>/gi, '')
    .replace(/<\/?(iframe|object|embed|link|meta)\b[\s\S]*?>/gi, '')
    .replace(/\son\w+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)/gi, '')
    .replace(/javascript:/gi, '')
}

export function renderMarkdown(source: string) {
  const html = marked.parse(source ?? '', { async: false })
  return sanitizePreviewHtml(typeof html === 'string' ? html : String(html))
}
