import assert from 'node:assert/strict'
import {
  COLLAPSED_WIDTH,
  DOCRAIL_MIN,
  LAYOUT_VERSION,
  SIDEBAR_MIN,
  beginWindowMove,
  docRailPaneWidth,
  endWindowMove,
  isLayoutBusy,
  noteWindowMove,
  layout,
  migrateStoredLayout,
  resizeDocRail,
  resizeSidebar,
  sidebarPaneWidth,
  sidebarHeadCompact,
  sidebarToolFilterUsesMark,
  SIDEBAR_HEAD_COMPACT,
  clampOverlayBox,
  toggleDocRail,
  toggleProjects,
  windowMoving
} from './layout.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

const snapshot = { ...layout }

function restore() {
  Object.assign(layout, snapshot)
}

test('fresh layout keeps the console full width by collapsing the doc rail', () => {
  assert.equal(snapshot.docRailCollapsed, true)
})

test('window move is layout-busy until it is released', () => {
  try {
    assert.equal(windowMoving.value, false)
    beginWindowMove()
    assert.equal(windowMoving.value, true)
    assert.equal(isLayoutBusy(), true)
    endWindowMove()
    assert.equal(windowMoving.value, false)
  } finally {
    endWindowMove()
  }
})

test('pointer up does not start a window-move cycle', () => {
  try {
    assert.equal(windowMoving.value, false)
    endWindowMove()
    assert.equal(windowMoving.value, false)
    assert.equal(isLayoutBusy(), false)
  } finally {
    endWindowMove()
  }
})

test('move events do not start a busy cycle unless a drag already began', () => {
  try {
    assert.equal(windowMoving.value, false)
    noteWindowMove()
    assert.equal(windowMoving.value, false)
    beginWindowMove()
    noteWindowMove()
    assert.equal(windowMoving.value, true)
  } finally {
    endWindowMove()
  }
})

test('collapsed doc rail takes no layout width so the PTY is not squeezed', () => {
  layout.docRailCollapsed = true
  layout.docRailWidth = 260
  try {
    assert.equal(docRailPaneWidth(), 0)
  } finally {
    restore()
  }
})

test('expanded doc rail reports its own width, not the sidebar collapsed strip', () => {
  layout.docRailCollapsed = false
  layout.docRailWidth = 260
  try {
    assert.equal(docRailPaneWidth(), 260)
    assert.notEqual(docRailPaneWidth(), COLLAPSED_WIDTH)
  } finally {
    restore()
  }
})

test('session menu stays inside the sidebar so a child webview cannot cover it', () => {
  const box = clampOverlayBox(
    { x: 250, y: 400, width: 228, height: 226 },
    { windowWidth: 1280, windowHeight: 840, containRight: 280 }
  )
  assert.equal(box.x + 228, 280)
  assert.ok(box.x < 250)
  assert.equal(box.y, 400)
})

test('session menu does not overflow the window bottom', () => {
  const box = clampOverlayBox(
    { x: 40, y: 800, width: 228, height: 226 },
    { windowWidth: 1280, windowHeight: 840 }
  )
  assert.equal(box.y + 226, 832)
})

test('narrow sidebar compacts the session head so 会话/刷新 do not wrap', () => {
  assert.equal(sidebarHeadCompact(SIDEBAR_MIN), true)
  assert.equal(sidebarHeadCompact(SIDEBAR_HEAD_COMPACT), true)
  assert.equal(sidebarHeadCompact(280), false)
})

test('narrow session filter uses the official mark when the tool name would wrap', () => {
  assert.equal(sidebarToolFilterUsesMark(SIDEBAR_MIN, 'Claude Code'), true)
  assert.equal(sidebarToolFilterUsesMark(280, 'Claude Code'), true)
  assert.equal(sidebarToolFilterUsesMark(360, 'Claude Code'), false)
})

test('short session filter labels stay text even in a narrow sidebar', () => {
  assert.equal(sidebarToolFilterUsesMark(SIDEBAR_MIN, '全部'), false)
  assert.equal(sidebarToolFilterUsesMark(SIDEBAR_MIN, 'Pi'), false)
  assert.equal(sidebarToolFilterUsesMark(280, 'Grok'), false)
})

test('workspace width is only the dragged size, never a collapsed strip', () => {
  layout.sidebarCollapsed = true
  layout.sidebarWidth = 280
  try {
    assert.equal(sidebarPaneWidth(), 280)
    resizeSidebar(SIDEBAR_MIN - 80)
    assert.equal(layout.sidebarWidth, SIDEBAR_MIN)
    assert.equal(sidebarPaneWidth(), SIDEBAR_MIN)
  } finally {
    restore()
  }
})

test('project list fold does not change workspace width', () => {
  layout.projectsCollapsed = false
  layout.sidebarWidth = 280
  try {
    toggleProjects()
    assert.equal(layout.projectsCollapsed, true)
    assert.equal(sidebarPaneWidth(), 280)
    toggleProjects()
    assert.equal(layout.projectsCollapsed, false)
  } finally {
    restore()
  }
})

test('toggleDocRail only flips collapsed and does not steal a collapsed strip', () => {
  layout.docRailCollapsed = true
  try {
    toggleDocRail()
    assert.equal(layout.docRailCollapsed, false)
    assert.equal(docRailPaneWidth(), layout.docRailWidth)
    toggleDocRail()
    assert.equal(layout.docRailCollapsed, true)
    assert.equal(docRailPaneWidth(), 0)
  } finally {
    restore()
  }
})

test('old flex-column layouts collapse the doc rail once so the console gets its width back', () => {
  const next = migrateStoredLayout({
    sidebarWidth: 280,
    docRailWidth: 260,
    docRailCollapsed: false
  })
  assert.equal(next.docRailCollapsed, true)
  assert.equal(next.layoutVersion, LAYOUT_VERSION)
  assert.equal(next.sidebarWidth, 280)
})

test('layouts already on the overlay version keep an opened doc rail', () => {
  const next = migrateStoredLayout({
    docRailCollapsed: false,
    layoutVersion: LAYOUT_VERSION
  })
  assert.equal(next.docRailCollapsed, false)
})

test('v2 layouts keep the overlay rail and stop collapsing the workspace', () => {
  const next = migrateStoredLayout({
    sidebarCollapsed: true,
    docRailCollapsed: false,
    layoutVersion: 2
  })
  assert.equal(next.sidebarCollapsed, false)
  assert.equal(next.docRailCollapsed, false)
  assert.equal(next.layoutVersion, LAYOUT_VERSION)
})

test('dragging the doc rail below the minimum collapses it to zero width', () => {
  layout.docRailCollapsed = false
  layout.docRailWidth = 260
  try {
    resizeDocRail(DOCRAIL_MIN - 40)
    assert.equal(layout.docRailCollapsed, true)
    assert.equal(docRailPaneWidth(), 0)
  } finally {
    restore()
  }
})
