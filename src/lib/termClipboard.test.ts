import assert from 'node:assert/strict'
import {
  attachTermClipboard,
  parseOsc52,
  resolveClaudePaste,
  shouldCopySelection,
  shouldInterceptPaste,
  type ClipKeyEvent
} from './termClipboard.ts'

function test(name: string, fn: () => void) {
  fn()
  console.log(`ok ${name}`)
}

function key(partial: Partial<ClipKeyEvent> & { code: string }): ClipKeyEvent {
  return {
    type: 'keydown',
    key: 'c',
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    altKey: false,
    ...partial
  }
}

test('OSC 52 write decodes UTF-8 clipboard payloads', () => {
  assert.deepEqual(parseOsc52('c;SGVsbG8='), { kind: 'write', text: 'Hello' })
  assert.deepEqual(parseOsc52('c;5L2g5aW9'), { kind: 'write', text: '你好' })
  assert.deepEqual(parseOsc52(';YQ=='), { kind: 'write', text: 'a' })
})

test('OSC 52 query and junk are ignored; empty payload clears', () => {
  assert.deepEqual(parseOsc52('c;?'), { kind: 'query' })
  assert.deepEqual(parseOsc52('c;'), { kind: 'clear' })
  assert.deepEqual(parseOsc52('c'), { kind: 'ignore' })
  assert.deepEqual(parseOsc52('c;!!!!'), { kind: 'ignore' })
})

test('Windows Ctrl+C copies only when the terminal has a selection', () => {
  const copy = key({ code: 'KeyC', ctrlKey: true })
  assert.equal(shouldCopySelection(copy, true, false), true)
  assert.equal(shouldCopySelection(copy, false, false), false)
  assert.equal(shouldCopySelection(key({ type: 'keyup', code: 'KeyC', ctrlKey: true }), true, false), false)
})

test('macOS uses Cmd+C for copy and leaves Ctrl+C as interrupt', () => {
  assert.equal(shouldCopySelection(key({ code: 'KeyC', metaKey: true }), true, true), true)
  assert.equal(shouldCopySelection(key({ code: 'KeyC', ctrlKey: true }), true, true), false)
})

test('Ctrl+Shift+C copies a selection on non-mac hosts', () => {
  const chord = key({ code: 'KeyC', ctrlKey: true, shiftKey: true })
  assert.equal(shouldCopySelection(chord, true, false), true)
  assert.equal(shouldCopySelection(chord, false, false), false)
  assert.equal(shouldCopySelection(chord, true, true), false)
})

test('Windows Ctrl+V is a paste chord and macOS leaves it alone', () => {
  const paste = key({ code: 'KeyV', key: 'v', ctrlKey: true })
  assert.equal(shouldInterceptPaste(paste, false), true)
  assert.equal(shouldInterceptPaste(paste, true), false)
  assert.equal(shouldInterceptPaste(key({ code: 'KeyV', key: 'v', ctrlKey: true, repeat: true }), false), false)
  assert.equal(shouldInterceptPaste(key({ type: 'keyup', code: 'KeyV', key: 'v', ctrlKey: true }), false), false)
  assert.equal(shouldInterceptPaste(key({ code: 'KeyV', key: 'v', ctrlKey: true, altKey: true }), false), false)
})

test('Claude paste sends text, or an image chord when the clipboard has only an image', () => {
  assert.deepEqual(resolveClaudePaste({ text: 'hello', hasImage: false }, true), { kind: 'text', text: 'hello' })
  assert.deepEqual(resolveClaudePaste({ text: 'hello', hasImage: true }, true), { kind: 'text', text: 'hello' })
  assert.deepEqual(resolveClaudePaste({ text: '  ', hasImage: true }, true), { kind: 'image', sequence: '\u001bv' })
  assert.deepEqual(resolveClaudePaste({ text: '', hasImage: true }, false), { kind: 'image', sequence: '\u0016' })
  assert.deepEqual(resolveClaudePaste({ text: '', hasImage: false }, true), { kind: 'skip' })
})

test('attachTermClipboard writes OSC 52 and host copy, and copies on mouseup', () => {
  const osc = new Map<number, (data: string) => boolean>()
  let keyHandler: ((event: ClipKeyEvent) => boolean) | undefined
  let selection = 'picked'
  const listeners = new Map<string, () => void>()
  const written: string[] = []
  const term = {
    parser: {
      registerOscHandler(id: number, callback: (data: string) => boolean) {
        osc.set(id, callback)
        return {
          dispose() {
            osc.delete(id)
          }
        }
      }
    },
    attachCustomKeyEventHandler(handler: (event: ClipKeyEvent) => boolean) {
      keyHandler = handler
    },
    getSelection() {
      return selection
    },
    hasSelection() {
      return selection.length > 0
    }
  }
  const el = {
    addEventListener(type: string, callback: () => void) {
      listeners.set(type, callback)
    },
    removeEventListener(type: string) {
      listeners.delete(type)
    }
  }
  const attached = attachTermClipboard(term, el, {
    write: (text) => written.push(text),
    mac: false
  })
  assert.equal(osc.get(52)?.('c;SGVsbG8='), true)
  assert.equal(keyHandler?.(key({ code: 'KeyC', ctrlKey: true })), false)
  listeners.get('mouseup')?.()
  selection = ''
  assert.equal(keyHandler?.(key({ code: 'KeyC', ctrlKey: true })), true)
  attached.dispose()
  assert.deepEqual(written, ['Hello', 'picked', 'picked'])
  assert.equal(osc.has(52), false)
  assert.equal(listeners.has('mouseup'), false)
})

test('Ctrl+V asks the host to paste only while that host owns the chord', () => {
  let keyHandler: ((event: ClipKeyEvent) => boolean) | undefined
  let pastes = 0
  let enabled = true
  const term = {
    parser: {
      registerOscHandler() {
        return { dispose() {} }
      }
    },
    attachCustomKeyEventHandler(handler: (event: ClipKeyEvent) => boolean) {
      keyHandler = handler
    },
    getSelection() {
      return ''
    },
    hasSelection() {
      return false
    }
  }
  const el = {
    addEventListener() {},
    removeEventListener() {}
  }
  const pasteKey = key({ code: 'KeyV', key: 'v', ctrlKey: true })
  attachTermClipboard(term, el, {
    write() {},
    mac: false,
    onPaste() {
      pastes += 1
    },
    pasteEnabled: () => enabled
  })
  assert.equal(keyHandler?.(pasteKey), false)
  enabled = false
  assert.equal(keyHandler?.(pasteKey), true)
  assert.equal(keyHandler?.(key({ type: 'keyup', code: 'KeyV', key: 'v', ctrlKey: true })), true)
  assert.equal(pastes, 1)
})
