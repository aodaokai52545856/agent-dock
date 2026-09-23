import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import {
  DSH_EMBED_EDGE,
  DSH_EMBED_GUTTER,
  DSH_STATUS_BAR,
  clampDshEmbedBounds,
  dshEmbedBlocked,
  toastOverlayTop
} from './dshEmbed.ts'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

const pane = { left: 280, top: 80, width: 1000, height: 700 }

test('keeps a window-edge strip so frameless resize still works', () => {
  const bounds = clampDshEmbedBounds(pane, {
    windowWidth: 1280,
    windowHeight: 840,
    docRailWidth: 0,
    maximized: false
  })
  assert.ok(bounds)
  assert.equal(bounds!.x, 280 + DSH_EMBED_GUTTER)
  assert.equal(bounds!.x + bounds!.width, 1280 - DSH_EMBED_EDGE)
  assert.equal(bounds!.y + bounds!.height, 780)
  assert.ok(bounds!.y + bounds!.height <= 840 - DSH_EMBED_EDGE)
})

test('leaves the sidebar gutter uncovered', () => {
  const bounds = clampDshEmbedBounds(pane, {
    windowWidth: 1280,
    windowHeight: 840,
    docRailWidth: 0,
    maximized: false
  })
  assert.ok(bounds)
  assert.ok(bounds!.x > pane.left)
})

test('leaves the doc rail gutter uncovered', () => {
  const bounds = clampDshEmbedBounds(pane, {
    windowWidth: 1280,
    windowHeight: 840,
    docRailWidth: 260,
    maximized: false
  })
  assert.ok(bounds)
  assert.equal(bounds!.width, 1000 - DSH_EMBED_GUTTER - 260 - DSH_EMBED_GUTTER)
})

test('maximized windows do not keep the outer resize strip', () => {
  const bounds = clampDshEmbedBounds(pane, {
    windowWidth: 1280,
    windowHeight: 840,
    docRailWidth: 0,
    maximized: true
  })
  assert.ok(bounds)
  assert.equal(bounds!.x + bounds!.width, 1280)
})

test('embed stops above the status bar so DeepSeek cannot cover it', () => {
  const tall = { left: 280, top: 40, width: 1000, height: 800 }
  const bounds = clampDshEmbedBounds(tall, {
    windowWidth: 1280,
    windowHeight: 840,
    docRailWidth: 0,
    maximized: false
  })
  assert.ok(bounds)
  assert.equal(bounds!.y + bounds!.height, 840 - DSH_STATUS_BAR)
  assert.ok(bounds!.y + bounds!.height <= 840 - DSH_STATUS_BAR)
})

test('title-bar menus and dialogs block the native embed', () => {
  const embed = { left: 280, top: 80, width: 1000, height: 700 }
  function fakeRoot(opts: { mask?: boolean; menus?: Box[] }) {
    return {
      querySelector: (selector: string) => (selector.includes('ad-mask') && opts.mask ? {} : null),
      querySelectorAll: (selector: string) => {
        if (!selector.includes('ad-menu')) return []
        return (opts.menus ?? []).map((box) => ({ getBoundingClientRect: () => box }))
      }
    }
  }
  assert.equal(dshEmbedBlocked(fakeRoot({}), embed), false)
  assert.equal(dshEmbedBlocked(fakeRoot({ mask: true }), embed), true)
  assert.equal(
    dshEmbedBlocked(fakeRoot({ menus: [{ left: 40, top: 200, width: 228, height: 226 }] }), embed),
    false,
    'sidebar context menus stay in the rail and must not hide DeepSeek'
  )
  assert.equal(
    dshEmbedBlocked(fakeRoot({ menus: [{ left: 300, top: 90, width: 180, height: 120 }] }), embed),
    true,
    'menus that overlap the embed still hide it'
  )
  assert.equal(
    dshEmbedBlocked(fakeRoot({ menus: [{ left: 300, top: 90, width: 180, height: 120 }] })),
    false,
    'without embed bounds, only a full-window mask should hide DeepSeek'
  )
})

test('toast band insets the embed so the native page cannot cover it', () => {
  const bounds = clampDshEmbedBounds(pane, {
    windowWidth: 1280,
    windowHeight: 840,
    docRailWidth: 0,
    maximized: false,
    overlayTop: 92
  })
  assert.ok(bounds)
  assert.equal(bounds!.y, 92)
  assert.equal(bounds!.height, 780 - 92)
  assert.equal(bounds!.y + bounds!.height, 780)
})

test('toast overlay top includes a gutter below the pill', () => {
  assert.equal(toastOverlayTop(null), 0)
  assert.equal(
    toastOverlayTop({ getBoundingClientRect: () => ({ bottom: 86 }) }),
    86 + DSH_EMBED_GUTTER
  )
})

test('unmounting DeepSeek closes the native embed so a dead localhost page cannot cover the dock', () => {
  const pane = readFileSync(join(root, 'components/DshWebPane.vue'), 'utf8')
  assert.match(pane, /onUnmounted\(/)
  assert.match(pane, /dshEmbedClose/)
  assert.match(pane, /dshEmbedPaused/)
  const unmount = pane.slice(pane.indexOf('onUnmounted'))
  assert.doesNotMatch(
    unmount.slice(0, 400),
    /dshEmbedSetVisible\(false\)/,
    'unmount must close the webview, not merely hide it'
  )
})

test('closing sessions pauses the embed before the confirm mask goes away', () => {
  const app = readFileSync(join(root, 'App.vue'), 'utf8')
  assert.match(app, /pauseDshEmbed/)
  assert.match(app, /dshEmbedClose/)
  const confirm = app.slice(app.indexOf('async function onConfirm'))
  const dismiss = confirm.indexOf('confirmOpen.value = false')
  assert.ok(dismiss > 0)
  assert.match(confirm.slice(0, dismiss), /pauseDshEmbed/)
  assert.match(confirm.slice(0, dismiss), /dshEmbedClose/)
})

test('the close button goes home instead of adopting another live session', () => {
  const app = readFileSync(join(root, 'App.vue'), 'utf8')
  assert.match(app, /markPtyExit\(id, \{ userClosed: true \}\)/)
  const pane = readFileSync(join(root, 'components/TerminalPane.vue'), 'utf8')
  assert.match(
    pane,
    /watch\(\s*\(\) => store\.activePtyId[\s\S]{0,220}?immediate:\s*true/
  )
})

test('DeepSeek overlay does not unmount the terminal so a live Grok TUI is not wiped', () => {
  const app = readFileSync(join(root, 'App.vue'), 'utf8')
  const stack = app.slice(app.indexOf('class="term-stack"'), app.indexOf('class="doc-layer"'))
  assert.match(stack, /<TerminalPane/)
  assert.match(stack, /<DshWebPane/)
  assert.doesNotMatch(
    stack,
    /v-else/,
    'TerminalPane must stay mounted while DeepSeek is showing'
  )
  assert.match(app, /term-under/)
  assert.match(app, /is-covered/)
  assert.match(app, /dsh-over/)
})

test('opening a listed session does not rescan or retarget the sidebar filter', () => {
  const app = readFileSync(join(root, 'App.vue'), 'utf8')
  const start = app.indexOf('async function openSession')
  const end = app.indexOf('function onAppContextMenu')
  assert.ok(start >= 0 && end > start)
  const open = app.slice(start, end)
  assert.doesNotMatch(open, /setSessionToolFilter/)
  assert.doesNotMatch(open, /refreshSessions/)
  assert.doesNotMatch(open, /正在切换会话/)
})

test('xterm hosts skip DeepSeek web and refresh after they become visible', () => {
  const pane = readFileSync(join(root, 'components/TerminalPane.vue'), 'utf8')
  assert.match(pane, /isDshWeb/)
  assert.match(pane, /host\.term\.refresh/)
  assert.match(pane, /v-show="store\.activePtyId \|\| loading"/)
})
