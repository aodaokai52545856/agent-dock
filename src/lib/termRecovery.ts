// ConPTY can leave a long native TUI paint parked on an older viewport.
// A settled size round trip asks the application to publish its current screen.
// Count output only outside that round trip so the repaint cannot trigger itself.
export const RECOVERY_OUTPUT_CHARS = 16 * 1024
export const RECOVERY_SETTLE_MS = 300
export const RECOVERY_COOLDOWN_MS = 1500

// Cursor visibility, SGR and OSC housekeeping are not a new answer. They must
// not make an already repaired viewport eligible for another size round trip.
function hasDisplayText(data: string) {
  return data.replace(/\x1b\[[0-?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|[\x00-\x1f\x7f]/g, '').trim().length > 0
}

type RecoveryHooks = {
  canRecover: () => boolean
  recover: () => Promise<void>
  scheduleTimeout: (callback: () => void, ms: number) => number
  cancelTimeout: (id: number) => void
  now?: () => number
  onError?: (error: unknown) => void
}

export function createTermRecovery(hooks: RecoveryHooks) {
  let output = 0
  let timer: number | undefined
  let recovering = false
  let repainting = false
  let disposed = false
  let forced = false
  let lastRecovery = -Infinity
  const now = hooks.now ?? Date.now

  function cancel() {
    if (timer !== undefined) hooks.cancelTimeout(timer)
    timer = undefined
  }

  async function flush() {
    timer = undefined
    if (disposed || recovering) return
    if (repainting && !forced) {
      repainting = false
      return
    }
    if ((!forced && output < RECOVERY_OUTPUT_CHARS) || !hooks.canRecover()) return
    forced = false
    output = 0
    recovering = true
    repainting = true
    lastRecovery = now()
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
    const delay = forced ? Math.max(RECOVERY_SETTLE_MS, lastRecovery + RECOVERY_COOLDOWN_MS - now()) : RECOVERY_SETTLE_MS
    timer = hooks.scheduleTimeout(() => { void flush() }, delay)
  }

  return {
    noteOutput(data: string) {
      if (disposed || !data) return
      if (!repainting && hasDisplayText(data)) output = Math.min(RECOVERY_OUTPUT_CHARS, output + data.length)
      // Wait until the output (including the recovery's repaint) has drained.
      // A user's downward scroll must not be starved by cursor/spinner updates.
      if (!forced) schedule()
    },
    request(force = false) {
      if (disposed || recovering) return
      if (force && !forced) {
        // Force bypasses the size threshold, not the need for new output.
        // A cooldown alone still replays every 1.5s under an endless wheel.
        if (!output || repainting) return
        forced = true
        schedule()
        return
      }
      if (forced) {
        if (timer === undefined) schedule()
      } else if (!repainting && output >= RECOVERY_OUTPUT_CHARS) schedule()
    },
    dispose() {
      disposed = true
      cancel()
    }
  }
}
