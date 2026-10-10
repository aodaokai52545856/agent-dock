import { parseSessionToolFilter, type SessionToolFilter } from './types.ts'

export const SESSION_TOOL_FILTER_KEY = 'ad-session-tool-filter'
export const SESSION_LIVE_ONLY_KEY = 'ad-session-live-only'

export type SessionViewSnapshot = {
  toolFilter: SessionToolFilter
  liveOnly: boolean
}

export type SessionViewStorage = {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
  removeItem(key: string): void
}

export function projectSessionViewKey(base: string, projectId: string) {
  return `${base}:${projectId}`
}

function readItem(storage: SessionViewStorage | null | undefined, key: string): string | null {
  if (!storage) return null
  try {
    return storage.getItem(key)
  } catch {
    return null
  }
}

function writeItem(storage: SessionViewStorage | null | undefined, key: string, value: string) {
  if (!storage) return
  try {
    storage.setItem(key, value)
  } catch {
    /* ignore quota */
  }
}

export function readGlobalSessionToolFilter(storage: SessionViewStorage | null | undefined): SessionToolFilter {
  return parseSessionToolFilter(readItem(storage, SESSION_TOOL_FILTER_KEY))
}

export function writeGlobalSessionToolFilter(
  storage: SessionViewStorage | null | undefined,
  filter: SessionToolFilter
) {
  writeItem(storage, SESSION_TOOL_FILTER_KEY, filter)
}

export function readGlobalSessionLiveOnly(storage: SessionViewStorage | null | undefined) {
  return readItem(storage, SESSION_LIVE_ONLY_KEY) === '1'
}

export function writeGlobalSessionLiveOnly(storage: SessionViewStorage | null | undefined, on: boolean) {
  writeItem(storage, SESSION_LIVE_ONLY_KEY, on ? '1' : '0')
}

export function readProjectSessionView(
  storage: SessionViewStorage | null | undefined,
  projectId: string,
  fallback: SessionViewSnapshot
): SessionViewSnapshot {
  if (!projectId) return { ...fallback }
  const filterRaw = readItem(storage, projectSessionViewKey(SESSION_TOOL_FILTER_KEY, projectId))
  const liveRaw = readItem(storage, projectSessionViewKey(SESSION_LIVE_ONLY_KEY, projectId))
  return {
    toolFilter: filterRaw == null ? fallback.toolFilter : parseSessionToolFilter(filterRaw),
    liveOnly: liveRaw == null ? fallback.liveOnly : liveRaw === '1'
  }
}

export function writeProjectSessionView(
  storage: SessionViewStorage | null | undefined,
  projectId: string,
  view: SessionViewSnapshot
) {
  if (!projectId) return
  writeItem(storage, projectSessionViewKey(SESSION_TOOL_FILTER_KEY, projectId), view.toolFilter)
  writeItem(storage, projectSessionViewKey(SESSION_LIVE_ONLY_KEY, projectId), view.liveOnly ? '1' : '0')
}

export function forgetProjectSessionView(storage: SessionViewStorage | null | undefined, projectId: string) {
  if (!storage || !projectId) return
  try {
    storage.removeItem(projectSessionViewKey(SESSION_TOOL_FILTER_KEY, projectId))
    storage.removeItem(projectSessionViewKey(SESSION_LIVE_ONLY_KEY, projectId))
  } catch {
    /* ignore quota */
  }
}
