import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import {
  glassifySpan,
  isOscQuery,
  isSurfaceFill,
  osc11Reply,
  oscRgbFromHex,
  parseCssRgb,
  paletteIndex,
  shouldClearFill,
  termSchemeFromInk,
  ansiPaletteBlack,
  ansiPaletteWhite,
  canvasPad,
  ptyCanvasInk,
  ptyForcesDark,
  termThemeBackground
} from './termGlass.ts'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('parseCssRgb reads hex and rgb()', () => {
  assert.deepEqual(parseCssRgb('#0d0d0d'), { r: 13, g: 13, b: 13 })
  assert.deepEqual(parseCssRgb('rgb(42, 42, 42)'), { r: 42, g: 42, b: 42 })
  assert.deepEqual(parseCssRgb('rgb(42 42 42)'), { r: 42, g: 42, b: 42 })
  assert.equal(parseCssRgb('transparent'), null)
})

test('surface fills are near-neutral theme greys', () => {
  assert.equal(isSurfaceFill({ r: 13, g: 13, b: 13 }, true), true)
  assert.equal(isSurfaceFill({ r: 42, g: 42, b: 48 }, true), true)
  assert.equal(isSurfaceFill({ r: 243, g: 243, b: 243 }, false), true)
  assert.equal(isSurfaceFill({ r: 180, g: 40, b: 40 }, true), false)
  assert.equal(isSurfaceFill({ r: 40, g: 120, b: 60 }, true), false)
})

test('palette 0 and dark greyscale map to glass', () => {
  assert.equal(paletteIndex('xterm-bg-0 xterm-fg-7'), 0)
  assert.equal(shouldClearFill({ className: 'xterm-bg-0' }, true), true)
  assert.equal(shouldClearFill({ className: 'xterm-bg-236' }, true), true)
  assert.equal(shouldClearFill({ className: 'xterm-bg-1' }, true), false)
  assert.equal(shouldClearFill({ backgroundColor: '#2a2a2a' }, true), true)
  assert.equal(shouldClearFill({ backgroundColor: 'rgb(120, 32, 32)' }, true), false)
})

test('glassifySpan clears only surface backgrounds', () => {
  const surface = { style: { backgroundColor: '#1c1c1c' }, className: '' }
  glassifySpan(surface, true)
  assert.equal(surface.style.backgroundColor, 'transparent')
  const diff = { style: { backgroundColor: 'rgb(90, 24, 24)' }, className: '' }
  glassifySpan(diff, true)
  assert.equal(diff.style.backgroundColor, 'rgb(90, 24, 24)')
})

test('OSC 11 query reports ink and set is treated as a query only when marked', () => {
  assert.equal(isOscQuery('?'), true)
  assert.equal(isOscQuery('rgb:1a1a/1a1a/1a1a'), false)
  assert.equal(oscRgbFromHex('#0D0D0D'), 'rgb:0D0D/0D0D/0D0D')
  assert.equal(osc11Reply('#0d0d0d'), '\x1b]11;rgb:0d0d/0d0d/0d0d\x1b\\')
})

test('pty theme background is one xterm-parseable rgba veil, not transparent', () => {
  const css = termThemeBackground('#0B0F13', 0)
  assert.equal(css, 'rgba(11, 15, 19, 0.900)')
  assert.match(css, /^rgba\(\d{1,3}, \d{1,3}, \d{1,3}, 0\.\d+\)$/)
  assert.equal(termThemeBackground('#0B0F13', 100), 'rgba(11, 15, 19, 0.120)')
})

test('light ink keeps ANSI black actually black so TUIs can paint', () => {
  assert.equal(termSchemeFromInk('#F3F3F3'), 'light')
  assert.equal(ansiPaletteBlack('#F3F3F3'), '#171717')
  assert.equal(ansiPaletteWhite('#171717', '#F3F3F3'), '#ECECEC')
  assert.equal(termSchemeFromInk('#0B0F13'), 'dark')
  assert.equal(ansiPaletteBlack('#0B0F13'), '#0B0F13')
})

test('Grok and Claude follow dock light ink; OpenCode and Kimi stay dark', () => {
  assert.equal(ptyCanvasInk('#F3F3F3', 'grokbuild'), '#F3F3F3')
  assert.equal(ptyCanvasInk('#F3F3F3', 'claude'), '#F3F3F3')
  assert.equal(ptyCanvasInk('#F3F3F3', 'opencode'), '#0B0F13')
  assert.equal(ptyCanvasInk('#F3F3F3', 'kimi'), '#0B0F13')
  assert.equal(ptyCanvasInk('#F3F3F3', 'pi'), '#0B0F13')
  assert.equal(ptyForcesDark('kimi'), true)
  assert.equal(ptyForcesDark('grokbuild'), false)
})

test('OpenCode leftover pad stays opaque dark so dock chrome cannot show through', () => {
  assert.equal(canvasPad('#F3F3F3', 50, 'opencode'), '#0B0F13')
  assert.equal(canvasPad('#F3F3F3', 50, 'kimi'), '#0B0F13')
  assert.equal(canvasPad('#F3F3F3', 50, 'pi'), '#0B0F13')
  assert.match(canvasPad('#F3F3F3', 50, 'grokbuild'), /^rgba\(243, 243, 243, /)
})

test('Pi keeps a dark inset so the TUI is not flush against the sidebar', () => {
  const pane = readFileSync(join(root, 'components/TerminalPane.vue'), 'utf8')
  assert.match(pane, /dataset\.tool/)
  assert.match(pane, /\[data-tool='pi'\] \.xterm\s*\{[^}]*padding:\s*8px/)
})

test('pty host css punches extra editor fills so they do not stack on the xterm veil', () => {
  const pane = readFileSync(join(root, 'components/TerminalPane.vue'), 'utf8')
  assert.match(pane, /background:\s*termThemeBackground\(/)
  assert.match(pane, /\.term-host\s*\{[^}]*background:\s*transparent/)
  assert.match(pane, /\.xterm-bg-0\s*\{[^}]*background-color:\s*transparent/)
  assert.doesNotMatch(
    pane,
    /\.xterm-viewport\s*\{[^}]*background-color:\s*transparent\s*!important/
  )
  assert.doesNotMatch(pane, /\.term-host\s*\{[^}]*--ad-editor/)
  assert.doesNotMatch(pane, /\.xterm-bg-0\s*\{[^}]*--ad-editor/)
  assert.match(pane, /style\.background = hostPad\(ptyId\)/)
})

test('pty canvas stays clipped to the tool pane so it cannot paint into the session strip or status bar', () => {
  const pane = readFileSync(join(root, 'components/TerminalPane.vue'), 'utf8')
  const host = pane.match(/\.term-host\s*\{[^}]+\}/)
  assert.ok(host, 'TerminalPane should define .term-host')
  assert.match(host[0], /overflow:\s*hidden/)
  assert.match(host[0], /contain:[^;]*paint/)
  assert.match(host[0], /translateZ\(0\)/)
  const xterm = pane.match(/\.term-host \.xterm\s*\{[^}]+\}/)
  assert.ok(xterm, 'TerminalPane should size .xterm to the host')
  assert.match(xterm[0], /width:\s*100%/)
  assert.match(xterm[0], /height:\s*100%/)
  assert.match(xterm[0], /padding:\s*8px/)
  assert.match(pane, /\[data-scheme='dark'\] \.xterm\s*\{[^}]*padding:\s*0/)
  const strip = readFileSync(join(root, 'components/LaunchStrip.vue'), 'utf8')
  assert.match(strip, /\.strip\s*\{[^}]*z-index:\s*[2-9]/)
  const bar = readFileSync(join(root, 'components/StatusBar.vue'), 'utf8')
  assert.match(bar, /\.bar\s*\{[^}]*z-index:\s*[2-9]/)
})

test('empty homepage keeps its own editor veil', () => {
  const pane = readFileSync(join(root, 'components/TerminalPane.vue'), 'utf8')
  const empty = pane.match(/\.empty\s*\{[^}]+\}/)
  assert.ok(empty, 'TerminalPane should define .empty')
  assert.match(empty[0], /background:\s*var\(--ad-editor\)/)
})

test('console editor column does not paint a veil under the pty', () => {
  const app = readFileSync(join(root, 'App.vue'), 'utf8')
  const main = app.match(/\.main\s*\{[^}]+\}/)
  assert.ok(main, 'App.vue should define .main')
  assert.match(main[0], /background:\s*transparent/)
})
