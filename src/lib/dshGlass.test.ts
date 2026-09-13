import assert from 'node:assert/strict'
import {
  dshGlassApplyScript,
  dshGlassCss,
  dshGlassCssText,
  dshGlassHash,
  dshGlassMessage,
  dshGlassTheme,
  dshGlassVars,
  parseDshGlassHash,
  withDshGlassHash
} from './dshGlass.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

test('empty background uses the dark navy default and full frost', () => {
  const theme = dshGlassTheme({
    uiTheme: 'dark',
    uiBackground: '',
    uiForeground: '',
    uiOpacity: 0
  })
  assert.equal(theme.bg, '#0B0F13')
  assert.equal(theme.fg, '#ECECEC')
  assert.equal(theme.frost, 100)
})

test('light theme does not keep a stored dark canvas', () => {
  const theme = dshGlassTheme({
    uiTheme: 'light',
    uiBackground: '#0B0F13',
    uiForeground: '',
    uiOpacity: 0
  })
  assert.equal(theme.bg, '#F3F3F3')
  assert.equal(theme.fg, '#171717')
  assert.equal(theme.scheme, 'light')
})

test('theme uses settings background, veil, and frost', () => {
  const theme = dshGlassTheme({
    uiTheme: 'dark',
    uiBackground: '#1A1B26',
    uiForeground: '#ECECEC',
    uiOpacity: 0,
    uiFrost: 65
  })
  assert.equal(theme.bg, '#1A1B26')
  assert.equal(theme.fg, '#ECECEC')
  assert.equal(theme.scheme, 'dark')
  assert.equal(theme.veil, 0.9)
  assert.equal(theme.frost, 65)
})

test('hash round-trips into the iframe fragment', () => {
  const theme = dshGlassTheme({
    uiTheme: 'light',
    uiBackground: '#F3F3F3',
    uiForeground: '#171717',
    uiOpacity: 40
  })
  const parsed = parseDshGlassHash(dshGlassHash(theme))
  assert.ok(parsed)
  assert.equal(parsed?.bg, theme.bg)
  assert.equal(parsed?.fg, theme.fg)
  assert.equal(parsed?.scheme, 'light')
  assert.equal(parsed?.veil, theme.veil)
  assert.equal(parsed?.frost, theme.frost)
})

test('withDshGlassHash keeps the token query and only replaces the fragment', () => {
  const theme = dshGlassTheme({
    uiTheme: 'dark',
    uiBackground: '#0D0D0D',
    uiForeground: '#ECECEC',
    uiOpacity: 0
  })
  const url = withDshGlassHash('http://127.0.0.1:3080/?token=abc.def', theme)
  assert.ok(url.startsWith('http://127.0.0.1:3080/?token=abc.def#'))
  assert.ok(url.includes('ad=1'))
  assert.ok(url.includes('bg=0D0D0D'))
})

test('glass css uses the settings background at window veil, not a solid fill', () => {
  const theme = dshGlassTheme({
    uiTheme: 'dark',
    uiBackground: '#0D0D0D',
    uiForeground: '#ECECEC',
    uiOpacity: 0
  })
  const css = dshGlassCss(theme)
  assert.equal(css.transparent, 'transparent')
  assert.equal(css.canvas, 'rgba(13, 13, 13, 0.9)')
  assert.ok(css.lift.startsWith('rgba(13, 13, 13, '))
})

test('injected stylesheet punches the official #151517 canvas to veil + frost', () => {
  const theme = dshGlassTheme({
    uiTheme: 'dark',
    uiBackground: '#0D0D0D',
    uiForeground: '#ECECEC',
    uiOpacity: 0,
    uiFrost: 65
  })
  const css = dshGlassCssText(theme)
  assert.equal(css.includes('#151517'), false)
  assert.ok(css.includes('background: transparent !important'))
  assert.ok(css.includes('backdrop-filter: blur(27.3px)'))
  assert.ok(css.includes('rgba(13, 13, 13, 0.9)'))
})

test('full-bleed tokens stay clear so frame and chat do not stack a second veil', () => {
  const theme = dshGlassTheme({
    uiTheme: 'dark',
    uiBackground: '#0D0D0D',
    uiForeground: '#ECECEC',
    uiOpacity: 0,
    uiFrost: 65
  })
  const vars = dshGlassVars(theme)
  assert.equal(vars['--dsw-alias-bg-base'], 'transparent')
  assert.equal(vars['--dsw-alias-bg-layer-1'], 'transparent')
  assert.equal(vars['--dsw-specific-sidebar-fill'], 'transparent')
  assert.equal(vars['--dsh-boot-bg'], 'transparent')
  assert.equal(Object.values(vars).some((value) => value.includes('#151517')), false)
  const css = dshGlassCssText(theme)
  assert.ok(css.includes('--dsw-alias-bg-base: transparent !important'))
  assert.ok(css.includes('--dsw-specific-sidebar-fill: transparent !important'))
  assert.match(css, /#root[^{]*\{[^}]*background: rgba\(13, 13, 13, 0\.9\) !important/)
})

test('apply script is a self-contained IIFE that does not depend on hash or postMessage', () => {
  const theme = dshGlassTheme({
    uiTheme: 'dark',
    uiBackground: '#0D0D0D',
    uiForeground: '#ECECEC',
    uiOpacity: 0,
    uiFrost: 65
  })
  const script = dshGlassApplyScript(theme)
  assert.ok(script.trimStart().startsWith('(function'))
  assert.ok(script.includes("setProperty('--dsw-alias-bg-base'"))
  assert.ok(script.includes("'important'"))
  assert.ok(script.includes('ad-dsh-glass'))
  assert.ok(script.includes('rgba(13, 13, 13, 0.9)'))
  assert.equal(script.includes('postMessage'), false)
  assert.equal(script.includes('location.hash'), false)
  new Function(script)
})

test('postMessage payload is the iframe contract', () => {
  const theme = dshGlassTheme({
    uiTheme: 'dark',
    uiBackground: '#0D0D0D',
    uiForeground: '#ECECEC',
    uiOpacity: 0
  })
  assert.deepEqual(dshGlassMessage(theme), { type: 'ad-dsh-glass', theme })
})
