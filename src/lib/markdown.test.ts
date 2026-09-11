import assert from 'node:assert/strict'
import { renderMarkdown, sanitizePreviewHtml } from './markdown.ts'

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
