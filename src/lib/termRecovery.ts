// ConPTY can leave a long native TUI paint parked on an older viewport.
// A settled size round trip asks the application to publish its current screen.
// Count output only outside that round trip so the repaint cannot trigger itself.
export const RECOVERY_OUTPUT_CHARS = 16 * 1024
export const RECOVERY_SETTLE_MS = 300

type RecoveryHooks = {
  canRecover: () => boolean
  recover: () => Promise<void>
  scheduleTimeout: (callback: () => void, ms: number) => number
  cancelTimeout: (id: number) => void
  onError?: (error: unknown) => void
}

export function createTermRecovery(hooks: RecoveryHooks) {
  let output = 0
  let timer: number | undefined
  let recovering = false
  let repainting = false
  let disposed = false

  function cancel() {
    if (timer !== undefined) hooks.cancelTimeout(timer)
    timer = undefined
  }

  async function flush() {
    timer = undefined
    if (disposed || recovering) return
    if (repainting) {
      repainting = false
      return
    }
    if (output < RECOVERY_OUTPUT_CHARS || !hooks.canRecover()) return
    output = 0
    recovering = true
    repainting = true
    try {
      await hooks.recover()
    } catch (error) {
      hooks.onError?.(error)
    } finally {
      recovering = false
      if (!disposed) schedule()
    }
  }

  function schedule() {
    cancel()
    timer = hooks.scheduleTimeout(() => { void flush() }, RECOVERY_SETTLE_MS)
  }

  return {
    noteOutput(data: string) {
      if (disposed || !data) return
      if (!repainting) output = Math.min(RECOVERY_OUTPUT_CHARS, output + data.length)
      // Wait until the output (including the recovery's repaint) has drained.
      schedule()
    },
    request() {
      if (!disposed && !recovering && !repainting && output >= RECOVERY_OUTPUT_CHARS) schedule()
    },
    dispose() {
      disposed = true
      cancel()
    }
  }
}
