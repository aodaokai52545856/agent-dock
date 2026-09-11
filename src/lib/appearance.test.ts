import assert from 'node:assert/strict'
import {
  clampContrast,
  clampUiFontSize,
  mixHex,
  parseAppearance,
  parseHex,
  parseTheme,
  resolvedTheme,
  themeDefaults
} from './appearance.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('parseTheme only accepts system light dark', () => {
  assert.equal(parseTheme('light'), 'light')
  assert.equal(parseTheme('dark'), 'dark')
  assert.equal(parseTheme('system'), 'system')
  assert.equal(parseTheme('other'), 'system')
  assert.equal(parseTheme(undefined), 'system')
})

test('ui font size clamps to 11-18', () => {
  assert.equal(clampUiFontSize(13), 13)
  assert.equal(clampUiFontSize(10), 11)
  assert.equal(clampUiFontSize(22), 18)
  assert.equal(clampUiFontSize(Number.NaN), 13)
})

test('system theme follows the preference flag', () => {
  assert.equal(resolvedTheme('light', true), 'light')
  assert.equal(resolvedTheme('dark', true), 'dark')
  assert.equal(resolvedTheme('system', true), 'light')
  assert.equal(resolvedTheme('system', false), 'dark')
})

test('parseHex accepts 3 and 6 digit colors', () => {
  assert.equal(parseHex('#e6b'), '#EE66BB')
  assert.equal(parseHex('e6b450'), '#E6B450')
  assert.equal(parseHex('nope'), '')
})

test('contrast mixes foreground toward background', () => {
  assert.equal(clampContrast(60), 60)
  assert.equal(clampContrast(-4), 0)
  assert.equal(mixHex('#FFFFFF', '#000000', 0.5), '#808080')
})

test('parseAppearance fills Codex-like defaults', () => {
  const next = parseAppearance({ theme: 'dark', accent: '#e6b450', contrast: 80, translucentSidebar: false })
  assert.equal(next.accent, '#E6B450')
  assert.equal(next.contrast, 80)
  assert.equal(next.translucentSidebar, false)
  assert.equal(themeDefaults('dark').background, '#0D0D0D')
})
