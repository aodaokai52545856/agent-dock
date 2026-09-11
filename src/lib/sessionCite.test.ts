import assert from 'node:assert/strict'
import { formatCiteBlock, formatCiteHeader, wrapBracketedPaste } from './sessionCite.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('cite header uses the tool label', () => {
  assert.equal(formatCiteHeader('grokbuild', '修圆角'), '【引用自 Grok · 修圆角】')
  assert.equal(formatCiteHeader('kimi', '  '), '【引用自 Kimi · 会话】')
})

test('project docs cite the relative path first', () => {
  const text = formatCiteBlock({
    toolId: 'grokbuild',
    sessionTitle: '跨 CLI',
    relPath: 'docs/superpowers/plans/2026-09-11-cite-rail.md'
  })
  assert.equal(
    text,
    '【引用自 Grok · 跨 CLI】\n@docs/superpowers/plans/2026-09-11-cite-rail.md'
  )
})

test('session-local docs can paste the body', () => {
  const text = formatCiteBlock({
    toolId: 'kimi',
    sessionTitle: '状态栏',
    body: '# 状态栏\n一行文案'
  })
  assert.match(text, /【引用自 Kimi · 状态栏】/)
  assert.match(text, /# 状态栏/)
})

test('bracketed paste does not add a trailing enter', () => {
  const wrapped = wrapBracketedPaste('hello')
  assert.equal(wrapped.startsWith('\x1b[200~'), true)
  assert.equal(wrapped.endsWith('\x1b[201~'), true)
  assert.equal(wrapped.includes('\r'), false)
})
