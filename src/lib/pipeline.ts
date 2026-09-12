import * as api from './api'
import { persistCurrent as persistFlow, toggleCodexThread } from './flow/runtime.ts'
import { selectedProject, store } from './store'
import type { CodexProbe, CodexThread } from './types'

const PREVIEW_THREADS: CodexThread[] = [
  {
    id: 'thr_preview_desktop',
    name: '桌面审查人设',
    preview: 'Continue the sidebar layout review',
    cwd: 'D:\\idea_jidian_projects\\aitools',
    sourceKind: 'appServer',
    isPinned: true,
    createdAt: Date.now() - 3_600_000,
    updatedAt: Date.now() - 120_000,
    modelProvider: 'openai'
  },
  {
    id: 'thr_preview_cli',
    name: 'CLI 同目录',
    preview: 'Fix the review gate copy',
    cwd: 'D:\\idea_jidian_projects\\aitools',
    sourceKind: 'cli',
    isPinned: false,
    createdAt: Date.now() - 86_400_000,
    updatedAt: Date.now() - 7_200_000,
    modelProvider: 'openai'
  }
]

function toMs(value: number) {
  if (!value) return 0
  return value < 1_000_000_000_000 ? value * 1000 : value
}

function normalizeThread(row: CodexThread): CodexThread {
  return {
    ...row,
    name: row.name || row.preview || row.id,
    preview: row.preview || '',
    createdAt: toMs(row.createdAt),
    updatedAt: toMs(row.updatedAt || row.createdAt)
  }
}

export async function probeCodex() {
  const project = selectedProject.value
  if (!project) {
    store.codexProbe = null
    return
  }
  if (!api.isTauri) {
    store.codexProbe = {
      ok: true,
      binary: 'preview',
      initialized: true,
      listed: PREVIEW_THREADS.length,
      cwdMatched: PREVIEW_THREADS.length,
      threads: PREVIEW_THREADS,
      writerSafe: true,
      note: '浏览器预览：未连接本机 Codex。切到桌面端后会走 thread/list，审查用 detached。'
    }
    return
  }
  try {
    store.codexProbe = await api.codexProbe(project.id)
  } catch (err) {
    store.codexProbe = {
      ok: false,
      binary: '',
      initialized: false,
      listed: 0,
      cwdMatched: 0,
      threads: [],
      writerSafe: true,
      note: err instanceof Error ? err.message : String(err)
    }
  }
}

export async function refreshCodexThreads() {
  const project = selectedProject.value
  if (!project) {
    store.codexThreads = []
    store.codexStatus = 'idle'
    store.codexError = ''
    store.codexProbe = null
    return
  }
  store.codexStatus = 'loading'
  store.codexError = ''
  await probeCodex()
  if (!api.isTauri) {
    store.codexThreads = PREVIEW_THREADS
    store.codexStatus = 'ready'
    return
  }
  try {
    const listed = await api.listCodexThreads(project.id)
    store.codexThreads = listed.threads.map(normalizeThread)
    store.codexStatus = 'ready'
    if (!store.codexThreads.length && listed.note) {
      store.codexError = listed.note
    }
  } catch (err) {
    store.codexThreads = store.codexProbe?.threads.map(normalizeThread) ?? []
    store.codexStatus = 'error'
    store.codexError = err instanceof Error ? err.message : String(err)
  }
}

export function toggleBridgeThread(threadId: string) {
  toggleCodexThread(threadId)
}

export function persistCurrent() {
  persistFlow()
}

export function probeNote(probe: CodexProbe | null) {
  if (!probe) return '尚未探测本机 Codex App Server。'
  return probe.note
}
