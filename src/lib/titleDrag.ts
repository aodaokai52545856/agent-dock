export function isTitlebarDoubleClick(event: { button: number; detail: number }) {
  return event.button === 0 && event.detail >= 2
}
