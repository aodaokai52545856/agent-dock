import { isFixedDshSession } from './dsh.ts'
import { isPendingSessionId } from './liveBind.ts'

export function sessionSelectKey(toolId: string, sessionId: string) {
  return `${toolId}:${sessionId}`
}

/** Sessions the existing delete command can remove from disk. DeepSeek's fixed entry and not-yet-written rows cannot. */
export function canPhysicallyDeleteSession(toolId: string, sessionId: string) {
  return !isFixedDshSession(toolId, sessionId) && !isPendingSessionId(sessionId)
}

export function toggleSessionKey(keys: readonly string[], key: string) {
  return keys.includes(key) ? keys.filter((item) => item !== key) : [...keys, key]
}

export function pruneSessionKeys(keys: readonly string[], alive: readonly string[]) {
  const keep = new Set(alive)
  return keys.filter((key) => keep.has(key))
}

export function deletableSessionKeys(rows: readonly { toolId: string; id: string }[]) {
  return rows
    .filter((row) => canPhysicallyDeleteSession(row.toolId, row.id))
    .map((row) => sessionSelectKey(row.toolId, row.id))
}

export function allKeysSelected(keys: readonly string[], target: readonly string[]) {
  return target.length > 0 && target.every((key) => keys.includes(key))
}

export function toggleVisibleSelection(keys: readonly string[], visible: readonly string[]) {
  if (allKeysSelected(keys, visible)) {
    const drop = new Set(visible)
    return keys.filter((key) => !drop.has(key))
  }
  const next = new Set(keys)
  for (const key of visible) next.add(key)
  return [...next]
}
