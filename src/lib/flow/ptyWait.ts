import type { SessionTurn } from '../types.ts'

export function detectNewAssistantTurn(beforeIds: string[], turns: SessionTurn[]): SessionTurn | null {
  const seen = new Set(beforeIds)
  return turns.find((item) => item.role === 'assistant' && !seen.has(item.id)) ?? null
}

export function turnIds(turns: SessionTurn[]) {
  return turns.map((item) => item.id)
}
