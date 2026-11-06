import type { SessionTurn } from '../types.ts'

// An opening remark arrives long before the work is done, so only the latest
// text can end the wait.
export function detectNewAssistantTurn(beforeIds: string[], turns: SessionTurn[]): SessionTurn | null {
  const seen = new Set(beforeIds)
  const fresh = turns.filter((item) => item.role === 'assistant' && !seen.has(item.id))
  const last = fresh[fresh.length - 1]
  return last && !last.working ? last : null
}

// Long turns are fine as long as the window keeps printing; only silence counts.
export function extendDeadline(deadline: number, lastOutputAt: number | undefined, patience: number) {
  return Math.max(deadline, (lastOutputAt ?? 0) + patience)
}

export function turnIds(turns: SessionTurn[]) {
  return turns.map((item) => item.id)
}
