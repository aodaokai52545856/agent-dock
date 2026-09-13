export const MIN_TERM_PX = 32

export function canMeasure(width: number, height: number): boolean {
  return width >= MIN_TERM_PX && height >= MIN_TERM_PX
}

export function proposeGrid(
  width: number,
  height: number,
  cellWidth: number,
  cellHeight: number
): { cols: number; rows: number } | null {
  if (!canMeasure(width, height) || cellWidth <= 0 || cellHeight <= 0) return null
  return {
    cols: Math.max(1, Math.floor(width / cellWidth)),
    rows: Math.max(1, Math.floor(height / cellHeight))
  }
}

export function rowsThatFit(containerHeight: number, screenHeight: number, rows: number) {
  if (rows <= 1 || screenHeight <= 0 || screenHeight <= containerHeight) return rows
  const cell = screenHeight / rows
  if (cell <= 0) return rows
  if (screenHeight - containerHeight < cell) return rows
  return Math.max(1, Math.floor(containerHeight / cell))
}

export type FitSchedulerHooks = {
  isBusy: () => boolean
  run: () => void
  scheduleFrame: (cb: () => void) => number
  scheduleTimeout: (cb: () => void, ms: number) => number
  cancelTimeout: (id: number) => void
  cancelFrame?: (id: number) => void
  settleMs?: number
}

export function createFitScheduler(hooks: FitSchedulerHooks) {
  let queued = false
  let frame = 0
  let settle = 0
  const settleMs = hooks.settleMs ?? 80

  function clearSettle() {
    if (!settle) return
    hooks.cancelTimeout(settle)
    settle = 0
  }

  function flush() {
    if (hooks.isBusy()) {
      queued = true
      return
    }
    queued = false
    hooks.run()
    clearSettle()
    settle = hooks.scheduleTimeout(() => {
      settle = 0
      if (!hooks.isBusy()) hooks.run()
    }, settleMs)
  }

  function request() {
    queued = true
    if (hooks.isBusy()) return
    if (frame) return
    frame = hooks.scheduleFrame(() => {
      frame = 0
      flush()
    })
  }

  function onBusyChange(busy: boolean) {
    if (!busy && queued) request()
  }

  function dispose() {
    queued = false
    if (frame && hooks.cancelFrame) hooks.cancelFrame(frame)
    frame = 0
    clearSettle()
  }

  return { request, onBusyChange, dispose }
}
