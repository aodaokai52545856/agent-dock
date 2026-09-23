export const PTY_DONE_IDLE_MS = 6000
export const PTY_DONE_MIN_BUSY_MS = 4000
export const PTY_DONE_STARTUP_MS = 15000

export type PtyDonePhase = 'busy' | 'idle'

export type PtyDoneWatch = {
  busySince: number
  lastAt: number
  cycles: number
  phase: PtyDonePhase
}

export type DoneNoticeTarget = 'skip' | 'in-app' | 'native'

export function beginOrExtendBusy(watch: PtyDoneWatch | undefined, at: number): PtyDoneWatch {
  if (!watch || watch.phase === 'idle') {
    return {
      busySince: at,
      lastAt: at,
      cycles: watch?.cycles ?? 0,
      phase: 'busy'
    }
  }
  return { ...watch, lastAt: at }
}

export function settleIdle(
  watch: PtyDoneWatch,
  now: number,
  opts?: { idleMs?: number; minBusyMs?: number; startupMs?: number }
): { watch: PtyDoneWatch; notify: boolean } {
  const idleMs = opts?.idleMs ?? PTY_DONE_IDLE_MS
  const minBusyMs = opts?.minBusyMs ?? PTY_DONE_MIN_BUSY_MS
  const startupMs = opts?.startupMs ?? PTY_DONE_STARTUP_MS
  if (watch.phase !== 'busy') return { watch, notify: false }
  if (now - watch.lastAt < idleMs) return { watch, notify: false }
  const span = watch.lastAt - watch.busySince
  const notify = span >= minBusyMs && (watch.cycles >= 1 || span >= startupMs)
  return {
    watch: { ...watch, phase: 'idle', cycles: watch.cycles + 1 },
    notify
  }
}

export function doneNoticeTarget(opts: { watching: boolean; windowFocused: boolean }): DoneNoticeTarget {
  if (opts.watching && opts.windowFocused) return 'skip'
  if (opts.windowFocused) return 'in-app'
  return 'native'
}

export function doneNoticeCopy(input: { toolLabel: string; title: string; projectName: string }) {
  const session = input.title.trim() || '会话'
  const tool = input.toolLabel.trim() || '窗口'
  return {
    title: `${tool} 已完成`,
    body: input.projectName.trim() ? `${input.projectName.trim()} · ${session}` : session
  }
}
