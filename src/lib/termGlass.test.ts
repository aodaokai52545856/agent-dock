import assert from 'node:assert/strict'
import {
  glassifySpan,
  isOscQuery,
  isSurfaceFill,
  osc11Reply,
  oscRgbFromHex,
  parseCssRgb,
  paletteIndex,
  shouldClearFill
} from './termGlass.ts'

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
