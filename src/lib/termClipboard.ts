export const OSC_CLIPBOARD = 52
const MAX_CLIP_CHARS = 1_048_576

export type Osc52Action =
  | { kind: 'write'; text: string }
  | { kind: 'clear' }
  | { kind: 'query' }
  | { kind: 'ignore' }

export type ClipKeyEvent = {
  type: string
  key: string
  code: string
  ctrlKey: boolean
  metaKey: boolean
  shiftKey: boolean
  altKey: boolean
  repeat?: boolean
  preventDefault?: () => void
}

export type ClipboardPaste = {
  text: string
  hasImage: boolean
  files?: string[]
}

export type ResolvedPaste =
  | { kind: 'text'; text: string }
  | { kind: 'image'; sequence: string }
  | { kind: 'skip' }

export type ClipTerm = {
  parser: {
    registerOscHandler: (
      ident: number,
      callback: (data: string) => boolean
    ) => { dispose: () => void }
  }
  attachCustomKeyEventHandler: (handler: (event: ClipKeyEvent) => boolean) => void
  getSelection: () => string
  hasSelection: () => boolean
}

export type ClipSurface = {
  addEventListener: (type: string, callback: () => void) => void
  removeEventListener: (type: string, callback: () => void) => void
}

function padBase64(value: string) {
  const pad = (4 - (value.length % 4)) % 4
  return pad ? value + '='.repeat(pad) : value
}

function decodeBase64Utf8(payload: string): string | null {
  const compact = payload.replace(/\s/g, '')
  if (!compact || !/^[A-Za-z0-9+/]*={0,2}$/.test(compact)) return null
  try {
    const binary = atob(padBase64(compact))
    const bytes = new Uint8Array(binary.length)
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i)
    const text = new TextDecoder().decode(bytes)
    if (text.length > MAX_CLIP_CHARS) return null
    return text
  } catch {
    return null
  }
}

export function parseOsc52(data: string): Osc52Action {
  const sep = data.indexOf(';')
  if (sep === -1) return { kind: 'ignore' }
  const payload = data.slice(sep + 1)
  if (payload === '?') return { kind: 'query' }
  if (!payload) return { kind: 'clear' }
  const text = decodeBase64Utf8(payload)
  if (text == null) return { kind: 'ignore' }
  return { kind: 'write', text }
}

export function shouldInterceptPaste(event: ClipKeyEvent, mac: boolean) {
  if (mac || event.repeat) return false
  if (event.type !== 'keydown' || event.altKey || event.metaKey || event.shiftKey) return false
  if (!event.ctrlKey) return false
  return event.code === 'KeyV' || event.key === 'v' || event.key === 'V'
}

export function shouldInterceptAntigravityPaste(event: ClipKeyEvent, mac: boolean) {
  if (event.type !== 'keydown' || event.altKey) return false
  const modifier = mac ? event.ctrlKey !== event.metaKey : event.ctrlKey && !event.metaKey
  return modifier && (event.code === 'KeyV' || event.key.toLowerCase() === 'v')
}

// Windows Claude binds image paste to alt+v. Terminals send that as ESC v, and
// Claude treats the meta flag as alt. Other platforms bind ctrl+v (0x16).
export function claudeImagePasteSequence(windows: boolean) {
  return windows ? '\u001bv' : '\u0016'
}

export function resolveClaudePaste(input: ClipboardPaste, windows: boolean): ResolvedPaste {
  if (input.hasImage && input.text.trim() === '') {
    return { kind: 'image', sequence: claudeImagePasteSequence(windows) }
  }
  if (input.text) return { kind: 'text', text: input.text }
  return { kind: 'skip' }
}

export function resolveAntigravityPaste(input: ClipboardPaste): ResolvedPaste | { kind: 'files'; paths: string[] } {
  if (input.files?.length) return { kind: 'files', paths: input.files }
  if (input.text && (!input.hasImage || input.text.trim())) return { kind: 'text', text: input.text }
  // The CLI can read native media formats absent from the WebView clipboard.
  return { kind: 'image', sequence: '\u0016' }
}

const antigravityMediaPath = /\.(png|jpe?g|gif|webp|bmp|tiff?|svg|mp4|mov|webm|avi)$/i

export function antigravityFileInputs(paths: string[], windows: boolean): string[] {
  return paths.filter(path => path && !/[\u0000-\u001f\u007f]/.test(path)).map(path => {
    const normalized = windows ? path.replaceAll('\\', '/') : path
    const quoted = /\s|"/.test(normalized) ? JSON.stringify(normalized) : normalized
    // Native paste recognizes image and video paths as attachments.
    return antigravityMediaPath.test(normalized) ? quoted : `@${quoted} `
  })
}

export function shouldCopySelection(event: ClipKeyEvent, hasSelection: boolean, mac: boolean) {
  if (event.type !== 'keydown' || event.altKey) return false
  const copyKey = event.code === 'KeyC' || event.key === 'c' || event.key === 'C'
  if (!copyKey || !hasSelection) return false
  if (mac) return event.metaKey && !event.ctrlKey
  if (!event.ctrlKey || event.metaKey) return false
  return true
}

export function attachTermClipboard(
  term: ClipTerm,
  el: ClipSurface,
  hooks: {
    write: (text: string) => void
    mac: boolean
    onPaste?: () => void
    interceptPaste?: (event: ClipKeyEvent) => boolean
  }
) {
  let disposed = false
  const offOsc = term.parser.registerOscHandler(OSC_CLIPBOARD, (data) => {
    if (disposed) return true
    const action = parseOsc52(data)
    if (action.kind === 'write') hooks.write(action.text)
    return true
  })
  term.attachCustomKeyEventHandler((event) => {
    if (disposed) return true
    if (shouldCopySelection(event, term.hasSelection(), hooks.mac)) {
      event.preventDefault?.()
      const text = term.getSelection()
      if (text) hooks.write(text)
      return false
    }
    if (hooks.onPaste && hooks.interceptPaste?.(event)) {
      event.preventDefault?.()
      if (!event.repeat) hooks.onPaste()
      return false
    }
    return true
  })
  const onMouseUp = () => {
    if (disposed) return
    const text = term.getSelection()
    if (text) hooks.write(text)
  }
  el.addEventListener('mouseup', onMouseUp)
  return {
    dispose() {
      disposed = true
      offOsc.dispose()
      el.removeEventListener('mouseup', onMouseUp)
    }
  }
}
