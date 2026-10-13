import assert from 'node:assert/strict'
import {
  isDiagramLang,
  mermaidSource,
  renderMarkdown,
  sanitizePreviewHtml
} from './markdown.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('renders headings and code', () => {
  const html = renderMarkdown('# 标题\n\n用 `ptyWrite` 引用')
  assert.match(html, /<h1>/)
  assert.match(html, /ptyWrite/)
})

test('strips script and event handlers', () => {
  const dirty = '<p onclick="alert(1)">x</p><script>alert(2)</script>'
  const html = sanitizePreviewHtml(dirty)
  assert.equal(html.includes('script'), false)
  assert.equal(html.includes('onclick'), false)
  assert.match(html, /<p>x<\/p>/)
})

test('diagram langs cover mermaid and flowchart aliases', () => {
  assert.equal(isDiagramLang('mermaid'), true)
  assert.equal(isDiagramLang('flowchart'), true)
  assert.equal(isDiagramLang('sequence'), true)
  assert.equal(isDiagramLang('ts'), false)
})

test('flowchart fence gets a mermaid starter when missing', () => {
  assert.equal(mermaidSource('flowchart', 'A --> B'), 'flowchart TD\nA --> B')
  assert.equal(mermaidSource('flowchart', 'LR\nA --> B'), 'flowchart LR\nA --> B')
  assert.equal(
    mermaidSource('mermaid', 'flowchart TD\nA --> B'),
    'flowchart TD\nA --> B'
  )
})

test('mermaid fences become ad-mermaid blocks', () => {
  const html = renderMarkdown('```mermaid\nflowchart TD\nA --> B\n```\n')
  assert.match(html, /class="ad-mermaid"/)
  assert.match(html, /data-source="/)
  assert.equal(html.includes('<script'), false)
  const encoded = html.match(/data-source="([^"]+)"/)?.[1] || ''
  assert.match(decodeURIComponent(encoded), /flowchart TD/)
})

test('ordinary code fences stay pre code', () => {
  const html = renderMarkdown('```ts\nconst a = 1\n```\n')
  assert.match(html, /language-ts/)
  assert.equal(html.includes('ad-mermaid'), false)
})
