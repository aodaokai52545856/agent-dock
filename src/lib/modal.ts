import type { ObjectDirective } from 'vue'

type ModalState = {
  el: HTMLElement
  restore: HTMLElement | null
  close: () => void
  onKey: (event: KeyboardEvent) => void
  onFocus: (event: FocusEvent) => void
}

const stack: ModalState[] = []
const states = new WeakMap<HTMLElement, ModalState>()
const focusable = 'button, input, select, textarea, a[href], [tabindex]'

function controls(el: HTMLElement) {
  return [...el.querySelectorAll<HTMLElement>(focusable)].filter(item =>
    item.tabIndex >= 0 && !item.matches(':disabled, [inert], [aria-hidden="true"]') &&
    item.getClientRects().length > 0 && getComputedStyle(item).visibility !== 'hidden'
  )
}

function focus(el: HTMLElement) {
  el.focus({ preventScroll: true })
}

// Shared by modal dialogs only; popovers and the canvas inspector stay non-modal.
export const modal: ObjectDirective<HTMLElement, () => void> = {
  mounted(el, binding) {
    if (!el.hasAttribute('tabindex')) el.tabIndex = -1
    const state: ModalState = {
      el,
      restore: document.activeElement instanceof HTMLElement ? document.activeElement : null,
      close: binding.value,
      onKey(event) {
        if (stack.at(-1) !== state || event.defaultPrevented || event.isComposing) return
        if (event.key === 'Escape') {
          event.preventDefault()
          event.stopImmediatePropagation()
          state.close()
        } else if (event.key === 'Tab') {
          const items = controls(el)
          const first = items[0]
          const last = items.at(-1)
          const current = document.activeElement
          if (!first || !el.contains(current) || current === el ||
            (event.shiftKey ? current === first : current === last)) {
            event.preventDefault()
            focus((event.shiftKey ? last : first) ?? el)
          }
        }
      },
      onFocus(event) {
        if (stack.at(-1) === state && !el.contains(event.target as Node)) focus(el)
      }
    }
    states.set(el, state)
    stack.push(state)
    window.addEventListener('keydown', state.onKey)
    document.addEventListener('focusin', state.onFocus)
    queueMicrotask(() => {
      if (stack.at(-1) === state && el.isConnected) {
        focus(el.querySelector<HTMLElement>('[data-modal-initial]') ?? el)
      }
    })
  },
  updated(el, binding) {
    const state = states.get(el)
    if (state) state.close = binding.value
  },
  unmounted(el) {
    const state = states.get(el)
    if (!state) return
    const wasTop = stack.at(-1) === state
    stack.splice(stack.indexOf(state), 1)
    states.delete(el)
    window.removeEventListener('keydown', state.onKey)
    document.removeEventListener('focusin', state.onFocus)
    if (!wasTop) return
    queueMicrotask(() => {
      const parent = stack.at(-1)?.el
      if (state.restore?.isConnected && (!parent || parent.contains(state.restore))) focus(state.restore)
      else if (parent) focus(parent)
    })
  }
}
