import * as api from './api'
import { persistPipelines } from './pipelineModel'
import { canAdvanceSlice, combineVerdicts, parseReviewVerdict } from './reviewVerdict'
import { ensurePipeline, selectedProject, showToast, store } from './store'
import type { BridgeReview, CodexProbe, CodexThread, ReviewVerdict } from './types'

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

function save() {
  persistPipelines(store.pipelines)
}

function pipeline() {
  const id = store.selectedProjectId
  if (!id) return null
  return ensurePipeline(id)
}

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

export async function refreshGitSnapshot() {
  const pipe = pipeline()
  const project = selectedProject.value
  if (!pipe || !project) return
  if (!api.isTauri) {
    pipe.git = {
      branch: 'main',
      head: 'preview000',
      dirty: true,
      summary: ' M agent-dock/src/App.vue'
    }
    save()
    return
  }
  try {
    pipe.git = await api.gitSnapshot(project.id)
    pipe.lastError = ''
  } catch (err) {
    pipe.git = null
    pipe.lastError = err instanceof Error ? err.message : String(err)
  }
  save()
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
    await refreshGitSnapshot()
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
  await refreshGitSnapshot()
}

export function toggleBridgeThread(threadId: string) {
  const pipe = pipeline()
  if (!pipe) return
  const idx = pipe.selectedThreadIds.indexOf(threadId)
  if (idx >= 0) pipe.selectedThreadIds.splice(idx, 1)
  else pipe.selectedThreadIds.push(threadId)
  save()
}

function applyReviews(pipe: NonNullable<ReturnType<typeof pipeline>>, reviews: BridgeReview[]) {
  if (!pipe) return
  const normalized = reviews.map((item) => ({
    ...item,
    verdict: item.verdict === 'pass' || item.verdict === 'fail' ? item.verdict : parseReviewVerdict(item.text)
  }))
  pipe.reviews = normalized
  const verdict = combineVerdicts(normalized.map((item) => item.verdict))
  pipe.gate = verdict === 'pass' ? 'passed' : 'failed'
  pipe.lastError = verdict === 'unknown' ? '官方审查结论是文本，未能读出 PASS/FAIL。请人工标记。' : ''
  save()
  return verdict
}

function reviewPayload() {
  const pipe = pipeline()
  const project = selectedProject.value
  if (!pipe || !project) return null
  return {
    projectId: project.id,
    threadIds: [...pipe.selectedThreadIds],
    targetKind: pipe.targetKind,
    commitSha: pipe.commitSha,
    baseBranch: pipe.baseBranch,
    customInstructions: pipe.customInstructions
  }
}

export async function submitManualReview() {
  const pipe = pipeline()
  const payload = reviewPayload()
  if (!pipe || !payload) {
    showToast('先选一个项目')
    return
  }
  if (pipe.gate === 'reviewing' || pipe.gate === 'developing') {
    showToast('这一片还在跑')
    return
  }
  pipe.gate = 'reviewing'
  pipe.lastError = ''
  save()
  if (!api.isTauri) {
    await new Promise((resolve) => window.setTimeout(resolve, 400))
    applyReviews(pipe, [
      {
        sourceThreadId: pipe.selectedThreadIds[0] ?? 'thr_preview_new',
        reviewThreadId: 'thr_preview_review',
        text: '预览审查：结构清楚。\nVERDICT: PASS',
        verdict: 'pass'
      }
    ])
    showToast('预览：本轮审查已通过')
    return
  }
  try {
    const result = await api.startCodexReview(payload)
    applyReviews(pipe, result.reviews)
    if (result.verdict === 'pass') showToast('审查通过，可以进入下一片')
    else if (result.verdict === 'fail') showToast('审查未通过，下一片仍锁定')
    else showToast('未能读出通过票，请人工标记')
  } catch (err) {
    pipe.gate = 'failed'
    pipe.lastError = err instanceof Error ? err.message : String(err)
    save()
    showToast(pipe.lastError)
  }
}

export function markManualVerdict(verdict: ReviewVerdict) {
  const pipe = pipeline()
  if (!pipe) return
  if (verdict !== 'pass' && verdict !== 'fail') return
  if (!pipe.reviews.length) {
    pipe.reviews = [
      {
        sourceThreadId: pipe.selectedThreadIds[0] ?? '',
        reviewThreadId: '',
        text: verdict === 'pass' ? '人工标记：通过' : '人工标记：未通过',
        verdict
      }
    ]
  } else {
    pipe.reviews = pipe.reviews.map((item) => ({ ...item, verdict }))
  }
  pipe.gate = verdict === 'pass' ? 'passed' : 'failed'
  pipe.lastError = ''
  save()
  showToast(verdict === 'pass' ? '已标记通过' : '已标记未通过，下一片仍锁定')
}

export function advanceSlice() {
  const pipe = pipeline()
  if (!pipe) return
  if (!canAdvanceSlice(pipe.gate)) {
    showToast('审查未通过，不能进入下一片')
    return
  }
  pipe.slice += 1
  pipe.gate = 'idle'
  pipe.task = ''
  pipe.reviews = []
  pipe.retryCount = 0
  pipe.lastError = ''
  save()
  showToast(`已解锁第 ${pipe.slice} 片`)
}

function developerPrompt(task: string, reviewText?: string) {
  const body = [
    '你是本仓库的开发者。只改当前这一片需要的文件，做完后停下来，不要开新需求。',
    '',
    '本轮任务：',
    task.trim() || '按工作区未提交改动继续推进，不要扩大范围。'
  ]
  if (reviewText?.trim()) {
    body.push('', '上一轮 Codex 审查未通过，必须按意见返工：', reviewText.trim())
  }
  return body.join('\n')
}

async function runCursorOnce(prompt: string) {
  const pipe = pipeline()
  const project = selectedProject.value
  if (!pipe || !project) throw new Error('先选一个项目')
  if (!api.isTauri) {
    await new Promise((resolve) => window.setTimeout(resolve, 300))
    pipe.cursorAgentId = pipe.cursorAgentId || 'preview-agent'
    return { ok: true, agentId: pipe.cursorAgentId, status: 'finished', text: '预览：已完成本片开发。' }
  }
  const result = await api.cursorDevRun({
    projectId: project.id,
    prompt,
    agentId: pipe.cursorAgentId || null
  })
  if (result.agentId) pipe.cursorAgentId = result.agentId
  if (!result.ok) {
    throw new Error(result.text || `Cursor 开发失败：${result.status}`)
  }
  return result
}

export async function runDeveloperLoop() {
  const pipe = pipeline()
  if (!pipe) {
    showToast('先选一个项目')
    return
  }
  if (!pipe.task.trim() && !store.settings.cursorApiKey && api.isTauri) {
    showToast('先写本轮任务，并在设置里填写 Cursor API Key')
    return
  }
  if (pipe.gate === 'reviewing' || pipe.gate === 'developing') {
    showToast('这一片还在跑')
    return
  }
  if (api.isTauri && !store.settings.cursorApiKey.trim()) {
    showToast('编排自动开发需要 Cursor API Key，请打开设置填写')
    return
  }

  pipe.retryCount = 0
  pipe.lastError = ''
  let feedback = ''

  while (pipe.retryCount <= pipe.maxRetries) {
    pipe.gate = 'developing'
    save()
    try {
      await runCursorOnce(developerPrompt(pipe.task, feedback))
    } catch (err) {
      pipe.gate = 'failed'
      pipe.lastError = err instanceof Error ? err.message : String(err)
      save()
      showToast(pipe.lastError)
      return
    }

    pipe.gate = 'reviewing'
    save()
    try {
      if (!api.isTauri) {
        applyReviews(pipe, [
          {
            sourceThreadId: pipe.selectedThreadIds[0] ?? 'thr_preview_new',
            reviewThreadId: 'thr_preview_review',
            text: pipe.retryCount === 0 && pipe.task.includes('故意失败')
              ? '预览：缺测试。\nVERDICT: FAIL'
              : '预览闭环审查通过。\nVERDICT: PASS',
            verdict: pipe.retryCount === 0 && pipe.task.includes('故意失败') ? 'fail' : 'pass'
          }
        ])
      } else {
        const payload = reviewPayload()
        if (!payload) throw new Error('先选一个项目')
        const result = await api.startCodexReview(payload)
        applyReviews(pipe, result.reviews)
      }
    } catch (err) {
      pipe.gate = 'failed'
      pipe.lastError = err instanceof Error ? err.message : String(err)
      save()
      showToast(pipe.lastError)
      return
    }

    if (combineVerdicts(pipe.reviews.map((item) => item.verdict)) === 'pass') {
      showToast('开发并审查通过，可以进入下一片')
      return
    }

    feedback = pipe.reviews.map((item) => item.text).join('\n\n')
    if (pipe.retryCount >= pipe.maxRetries) {
      showToast(`已返工 ${pipe.retryCount} 次仍未通过，下一片保持锁定`)
      return
    }
    pipe.retryCount += 1
    showToast(`审查未通过，带意见返工（${pipe.retryCount}/${pipe.maxRetries}）`)
  }
}

export function persistCurrent() {
  persistPipelines(store.pipelines)
}

export function probeNote(probe: CodexProbe | null) {
  if (!probe) return '尚未探测本机 Codex App Server。'
  return probe.note
}