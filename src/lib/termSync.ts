export const SYNC_OUTPUT_MODE = 2026

export type CsiParams = Array<number | number[]>

export type SyncTerm = {
  parser: {
    registerCsiHandler: (
      id: { prefix?: string; intermediates?: string; final: string },
      callback: (params: CsiParams) => boolean
    ) => { dispose: () => void }
  }
}

export function flattenCsiParams(params: CsiParams): number[] {
  const out: number[] = []
  for (const value of params) {
    if (Array.isArray(value)) out.push(...value)
    else out.push(value)
  }
  return out
}

export function csiParamsInclude(params: CsiParams, mode: number) {
  return flattenCsiParams(params).includes(mode)
}

export function decrqmReply(mode: number, status = 2) {
  return `\x1b[?${mode};${status}$y`
}

export type RefreshGateHooks = {
  refresh: () => void
  scheduleFrame: (cb: () => void) => number
  cancelFrame: (id: number) => void
}

export function createRefreshGate(hooks: RefreshGateHooks) {
  let frame = 0

  function request() {
    if (frame) return
    frame = hooks.scheduleFrame(() => {
      frame = 0
      hooks.refresh()
    })
  }

  function dispose() {
    if (!frame) return
    hooks.cancelFrame(frame)
    frame = 0
  }

  return { request, dispose }
}

export function attachSynchronizedOutput(
  term: SyncTerm,
  hooks: { send: (data: string) => void; refresh: () => void }
) {
  const offReset = term.parser.registerCsiHandler({ prefix: '?', final: 'l' }, (params) => {
    if (csiParamsInclude(params, SYNC_OUTPUT_MODE)) hooks.refresh()
    return false
  })
  const offQuery = term.parser.registerCsiHandler(
    { prefix: '?', intermediates: '$', final: 'p' },
    (params) => {
      if (!csiParamsInclude(params, SYNC_OUTPUT_MODE)) return false
      hooks.send(decrqmReply(SYNC_OUTPUT_MODE))
      return true
    }
  )
  return {
    dispose() {
      offReset.dispose()
      offQuery.dispose()
    }
  }
}
