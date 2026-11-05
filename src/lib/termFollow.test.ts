import assert from 'node:assert/strict'
import { createTermFollow } from './termFollow.ts'

let visible = true, selected = false, bottom = true, scrolls = 0
let serial = 0
const frames = new Map<number, () => void>()
const follow = createTermFollow({
  canFollow: () => visible && !selected,
  atBottom: () => bottom,
  scrollToBottom: () => { scrolls++; bottom = true },
  scheduleFrame: cb => { frames.set(++serial, cb); return serial },
  cancelFrame: id => { frames.delete(id) }
})
function flush() { const work = [...frames.values()]; frames.clear(); work.forEach(cb => cb()) }

// A TUI can move the viewport without the user asking to read history.
bottom = false
follow.refresh(); follow.refresh(); flush()
assert.equal(scrolls, 1)
assert.equal(bottom, true)
// Background painting cannot disturb DOM scroll geometry; return resumes follow.
visible = false
bottom = false
follow.refresh(); flush()
assert.equal(scrolls, 1)
visible = true
follow.refresh(); flush()
assert.equal(scrolls, 2)
// A real user scroll overrides subsequent paints and hide/show cycles.
follow.pause()
bottom = false
follow.refresh(); flush()
visible = false; follow.refresh(); flush()
visible = true; follow.refresh(); flush()
assert.equal(scrolls, 2)
assert.equal(follow.isFollowing(), false)
bottom = true
follow.userScroll(); flush()
assert.equal(scrolls, 3)
selected = true
follow.refresh(); flush()
assert.equal(scrolls, 3)
follow.refresh(); follow.dispose(); flush()
assert.equal(scrolls, 3)
console.log('ok native output follows across hidden panes and TUI shifts while respecting user history and selection')

{
  let position = 80, isBottom = true
  let nextFrame: (() => void) | undefined
  let marker: { line: number; dispose: () => void } | undefined
  const saved = createTermFollow({
    canFollow: () => true,
    atBottom: () => isBottom,
    scrollToBottom: () => { position = 80; isBottom = true },
    captureAnchor: () => { marker = { line: position, dispose() { this.line = -1 } }; return marker },
    scrollToLine: line => { position = line },
    scheduleFrame: cb => { nextFrame = cb; return 1 },
    cancelFrame: () => { nextFrame = undefined }
  })
  saved.suspend()
  position = 0; isBottom = false
  saved.userScroll() // A delayed pre-blur callback must not overwrite intent.
  saved.resume(); nextFrame!()
  assert.equal(position, 80)
  assert.equal(saved.isFollowing(), true)
  saved.pause()
  position = 32; isBottom = false
  saved.userScroll()
  saved.suspend()
  position = 0
  saved.resume(); nextFrame!()
  assert.equal(position, 32)
  assert.equal(saved.isFollowing(), false)
  // The marker tracks retained history when scrollback is trimmed in background.
  marker!.line = 22
  saved.suspend(); saved.resume(); nextFrame!()
  assert.equal(position, 22)
  saved.dispose()
  assert.equal(marker!.line, -1)
}
console.log('ok inactive callbacks cannot replace scroll intent; history markers restore and follow scrollback trimming')
