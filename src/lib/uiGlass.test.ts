import assert from 'node:assert/strict'
import {
  UI_FROST_DEFAULT,
  UI_OPACITY_DEFAULT,
  clampUiFrost,
  clampUiOpacity,
  glassRgba,
  inkBoostPercent,
  veilAlpha
} from './uiGlass.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('opacity and frost clamp to 0-100', () => {
  assert.equal(UI_FROST_DEFAULT, 100)
  assert.equal(clampUiOpacity(Number.NaN), UI_OPACITY_DEFAULT)
  assert.equal(clampUiOpacity(-4), 0)
  assert.equal(clampUiOpacity(80), 80)
  assert.equal(clampUiOpacity(140), 100)
  assert.equal(clampUiFrost(Number.NaN), UI_FROST_DEFAULT)
  assert.equal(clampUiFrost(0), 0)
  assert.equal(clampUiFrost(65), 65)
  assert.equal(clampUiFrost(200), 100)
})

test('glass rgba matches the CSS veil and never goes fully black-transparent', () => {
  assert.equal(veilAlpha(0), 0.9)
  assert.equal(Number(veilAlpha(100).toFixed(2)), 0.12)
  assert.equal(glassRgba('#0B0F13', 0), 'rgba(11, 15, 19, 0.900)')
  assert.equal(glassRgba('#1A1B26', 0).startsWith('rgba(26, 27, 38, '), true)
  assert.equal(glassRgba('nope', 0), 'rgba(11, 15, 19, 0.900)')
})

test('secondary ink jumps toward body text by half glass so 50% opacity still reads', () => {
  assert.equal(inkBoostPercent(0), 0)
  assert.equal(inkBoostPercent(50) >= 80, true)
  assert.equal(inkBoostPercent(100), 95)
})
