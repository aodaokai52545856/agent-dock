import type { BridgePipeline } from './types'

const PIPELINE_STORAGE_KEY = 'ad-bridge-pipelines'

export function emptyPipeline(projectId: string): BridgePipeline {
  return {
    projectId,
    slice: 1,
    gate: 'idle',
    task: '',
    selectedThreadIds: [],
    targetKind: 'uncommittedChanges',
    commitSha: '',
    baseBranch: 'main',
    customInstructions: '',
    reviews: [],
    cursorAgentId: '',
    retryCount: 0,
    maxRetries: 2,
    lastError: '',
    git: null
  }
}

export function loadPipelines(): Record<string, BridgePipeline> {
  try {
    const raw = localStorage.getItem(PIPELINE_STORAGE_KEY)
    if (!raw) return {}
    const parsed = JSON.parse(raw) as Record<string, BridgePipeline>
    const out: Record<string, BridgePipeline> = {}
    for (const [id, row] of Object.entries(parsed ?? {})) {
      if (!id || !row || typeof row !== 'object') continue
      out[id] = { ...emptyPipeline(id), ...row, projectId: id }
    }
    return out
  } catch {
    return {}
  }
}

export function persistPipelines(pipelines: Record<string, BridgePipeline>) {
  try {
    const slim: Record<string, BridgePipeline> = {}
    for (const [id, row] of Object.entries(pipelines)) {
      slim[id] = {
        ...row,
        reviews: row.reviews.map((review) => ({
          ...review,
          text: review.text.length > 200_000 ? review.text.slice(0, 200_000) : review.text
        }))
      }
    }
    localStorage.setItem(PIPELINE_STORAGE_KEY, JSON.stringify(slim))
  } catch {
    /* ignore quota */
  }
}
