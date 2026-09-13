import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import {
  appearanceFromSettings,
  chromeSurfaces,
  clampContrast,
  clampTerminalFontSize,
  clampUiFontSize,
  lookPatchFromDraft,
  colorLuminance,
  defaultAppearance,
  glassFill,
  inkTones,
  mixHex,
  parseAppearance,
  parseHex,
  parseTheme,
  resolvedTheme,
  themeDefaults,
  themePaint
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

test('look patch applies theme fonts terminal size and glass follow without a save click', () => {
  assert.equal(clampTerminalFontSize(9), 10)
  assert.equal(clampTerminalFontSize(30), 22)
  const patch = lookPatchFromDraft({
    uiTheme: 'light',
    uiFontSize: 15,
    uiFontFamily: 'Segoe UI',
    codeFontFamily: 'Consolas',
    uiContrast: 40,
    translucentSidebar: false,
    terminalFontSize: 18,
    grokFollowGlass: false
  })
  assert.equal(patch.uiTheme, 'light')
  assert.equal(patch.uiFontSize, 15)
  assert.equal(patch.uiFontFamily, 'Segoe UI')
  assert.equal(patch.codeFontFamily, 'Consolas')
  assert.equal(patch.terminalFontSize, 18)
  assert.equal(patch.grokFollowGlass, false)
  assert.equal(patch.translucentSidebar, false)
})

test('parseAppearance fills Codex-like defaults', () => {
  const next = parseAppearance({ theme: 'dark', accent: '#e6b450', contrast: 80, translucentSidebar: false })
  assert.equal(next.accent, '#E6B450')
  assert.equal(next.contrast, 80)
  assert.equal(next.translucentSidebar, false)
  assert.equal(themeDefaults('dark').background, '#0B0F13')
})

test('dark theme default is navy ink; custom colors stay empty so light can still resolve', () => {
  const look = defaultAppearance()
  assert.equal(look.theme, 'system')
  assert.equal(look.background, '')
  assert.equal(look.accent, '')
  assert.equal(look.foreground, '')
  assert.equal(themeDefaults('dark').background, '#0B0F13')
  assert.equal(themeDefaults('light').background, '#F3F3F3')
})

test('light theme does not reuse a stored dark background', () => {
  const light = appearanceFromSettings(
    { uiTheme: 'light', uiBackground: '#0B0F13', uiForeground: '' },
    true
  )
  assert.equal(light.background, '')
  assert.equal(themePaint(light.background, themeDefaults('light').background, 'light', 'surface'), '#F3F3F3')
  const dark = appearanceFromSettings({ uiTheme: 'dark', uiBackground: '#0B0F13' }, false)
  assert.equal(dark.background, '#0B0F13')
})

test('light palette stays independent of dark custom colors', () => {
  const look = appearanceFromSettings(
    { uiTheme: 'light', uiBackground: '#0B0F13', uiBackgroundLight: '#FAFAFA' },
    true
  )
  assert.equal(look.background, '#FAFAFA')
})

test('themePaint drops colors that would invert the theme', () => {
  assert.equal(themePaint('#0B0F13', '#F3F3F3', 'light', 'surface'), '#F3F3F3')
  assert.equal(themePaint('#0B0F13', '#0B0F13', 'dark', 'surface'), '#0B0F13')
  assert.equal(themePaint('#F3F3F3', '#0B0F13', 'dark', 'surface'), '#0B0F13')
  assert.equal(themePaint('#ECECEC', '#171717', 'light', 'ink'), '#171717')
  assert.equal(themePaint('#171717', '#ECECEC', 'dark', 'ink'), '#ECECEC')
})

test('glassFill keeps the ink and lets the window veil through', () => {
  assert.equal(glassFill('#0B0F13'), 'rgb(11 15 19 / var(--ad-veil))')
  assert.equal(glassFill('#F3F3F3'), 'rgb(243 243 243 / var(--ad-veil))')
})

test('chrome surfaces keep the veil so opacity and frost show through', () => {
  const light = chromeSurfaces('#F3F3F3', '#171717')
  const dark = chromeSurfaces('#0B0F13', '#ECECEC')
  for (const value of [...Object.values(light), ...Object.values(dark)]) {
    assert.match(value, /\/ var\(--ad-veil\)\)$/)
    assert.equal(value.startsWith('#'), false)
  }
})

test('light secondary ink stays darker than mid grey so glass still reads', () => {
  const tones = inkTones('#171717', '#F3F3F3', 60, 'light')
  assert.equal(colorLuminance(tones.muted) < colorLuminance('#6F6F6F'), true)
  assert.equal(colorLuminance(tones.faint) < colorLuminance('#8A8A8A'), true)
  assert.equal(colorLuminance(tones.muted) < colorLuminance(tones.faint), true)
})

test('dark secondary ink stays brighter than charcoal', () => {
  const tones = inkTones('#ECECEC', '#0B0F13', 60, 'dark')
  assert.equal(colorLuminance(tones.muted) > colorLuminance('#4A4A4A'), true)
  assert.equal(colorLuminance(tones.muted) > colorLuminance(tones.faint), true)
})

test('settings look controls commit live and do not revert on close', () => {
  const drawer = readFileSync(join(dirname(fileURLToPath(import.meta.url)), '..', 'components/SettingsDrawer.vue'), 'utf8')
  assert.match(drawer, /commitLookSettings/)
  assert.match(drawer, /flushLookSettings/)
  assert.match(drawer, /terminalFontSize/)
  assert.doesNotMatch(drawer, /if \(!open\) \{\s*applyUiGlass\(props\.settings/)
  assert.doesNotMatch(drawer, /点保存后写入本机/)
})
