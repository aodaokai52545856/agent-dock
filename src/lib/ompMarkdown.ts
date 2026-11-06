import { Marked, Renderer } from 'marked'
import hljs from 'highlight.js/lib/common'
import { escapeHtml } from './markdown.ts'

const AUTO_LANGUAGES = ['typescript', 'javascript', 'python', 'json', 'bash', 'sql', 'yaml', 'xml', 'css', 'diff']

export function highlightOmpCode(source: string, language = ''): string {
  // Very large tool output stays readable without blocking the stream on highlighting.
  if (source.length > 80_000 || /^(text|plaintext|txt|log)$/i.test(language)) return escapeHtml(source)
  const key = language.trim().split(/\s+/)[0].toLowerCase()
  if (key && !hljs.getLanguage(key)) return escapeHtml(source)
  try {
    return key ? hljs.highlight(source, {language:key, ignoreIllegals:true}).value :
      hljs.highlightAuto(source, AUTO_LANGUAGES).value
  } catch { return escapeHtml(source) }
}

/** Convert SGR color codes to a fixed set of classes; all output is escaped. */
export function renderOmpTerminal(source: string): string {
  const colors = ['black','red','green','yellow','blue','magenta','cyan','white']
  let color = '', bold = false, html = '', offset = 0
  // Strip OSC sequences (including hyperlinks) and non-SGR terminal controls.
  const clean = source.replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g, '').replace(/\x1b\[(?![\d;]*m)[\d;?]*[ -/]*[@-~]/g, '')
  const append = (text: string) => {
    const classes = [color ? `omp-ansi-${color}` : '', bold ? 'omp-ansi-bold' : ''].filter(Boolean).join(' ')
    const escaped = escapeHtml(text)
    // rg/grep omit ANSI in pipes; file:line output still deserves a readable hierarchy.
    html += classes ? `<span class="${classes}">${escaped}</span>` : escaped.replace(
      /^((?:[A-Za-z]:)?[^:\r\n]+\.[\w]+):(\d+)(?=:)/gm,
      '<span class="omp-output-path">$1</span>:<span class="omp-output-line">$2</span>'
    )
  }
  for (const match of clean.matchAll(/\x1b\[([\d;]*)m/g)) {
    append(clean.slice(offset, match.index))
    const codes = (match[1] || '0').split(';').map(Number)
    for (let i = 0; i < codes.length; i++) {
      const code = codes[i]
      if (code === 0) { color = ''; bold = false }
      else if (code === 1) bold = true
      else if (code === 22) bold = false
      else if (code === 39) color = ''
      else if (code >= 30 && code <= 37) color = colors[code - 30]
      else if (code >= 90 && code <= 97) color = colors[code - 90]
      // Skip extended-color parameters instead of treating them as SGR controls.
      else if (code === 38 || code === 48) i += codes[i + 1] === 2 ? 4 : codes[i + 1] === 5 ? 2 : 0
    }
    offset = match.index! + match[0].length
  }
  append(clean.slice(offset))
  return html
}

const renderer = new Renderer()
renderer.code = ({text, lang}) => {
  const language = (lang || '').split(/\s+/)[0]
  return `<section class="omp-code-block"><div class="omp-code-toolbar"><span>${escapeHtml(language || 'code')}</span><button type="button" class="omp-code-copy" aria-label="复制代码">复制</button></div><pre tabindex="0"><code class="hljs language-${escapeHtml(language || 'text')}">${highlightOmpCode(text, language)}</code></pre></section>\n`
}
// Model-generated HTML is content, never UI or executable markup.
renderer.html = ({text}) => escapeHtml(text)
renderer.link = function ({href, tokens, title}) {
  const label = this.parser.parseInline(tokens)
  if (!/^(https?:\/\/|mailto:|#)/i.test(href)) return label
  return `<a href="${escapeHtml(href)}"${title ? ` title="${escapeHtml(title)}"` : ''}>${label}</a>`
}
renderer.image = ({href, text, title}) => /^(https?:\/\/|data:image\/(png|jpeg|gif|webp);base64,)/i.test(href) ?
  `<img src="${escapeHtml(href)}" alt="${escapeHtml(text)}"${title ? ` title="${escapeHtml(title)}"` : ''} loading="lazy" />` : escapeHtml(text)
const table = renderer.table
renderer.table = function (token) {
  return `<div class="ad-table-scroll" role="region" aria-label="表格，可横向滚动" tabindex="0">${table.call(this, token)}</div>\n`
}
const markdown = new Marked({gfm:true, breaks:true, renderer})
export function renderOmpMarkdown(source: string) {
  return markdown.parse(source || '', {async:false}) as string
}
