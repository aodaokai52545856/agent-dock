<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import * as api from '../lib/api'
import { relativeTime } from '../lib/format'
import { layout, toggleDocRail } from '../lib/layout'
import { kindLabel } from '../lib/sessionCite'
import { isPendingSessionId } from '../lib/liveBind'
import { activeLive, selectedProject, store } from '../lib/store'
import { toolLabel, type SessionDoc, type ToolId } from '../lib/types'

const emit = defineEmits<{
  open: [doc: SessionDoc]
  cite: [payload: { sessionId: string; toolId: ToolId; title: string }]
}>()

defineProps<{
  collapsed: boolean
}>()

const docs = ref<SessionDoc[]>([])
const status = ref<'idle' | 'loading' | 'ready' | 'empty' | 'error'>('idle')
const error = ref('')
const query = ref('')
const tab = ref<'files' | 'summaries'>('files')
const refreshing = ref(false)
let loadedKey = ''
let pollTimer = 0
let loadSeq = 0

const railSession = computed(() => {
  if (store.focusedSession && !isPendingSessionId(store.focusedSession.sessionId)) {
    return store.focusedSession
  }
  const live = activeLive.value
  if (!live || live.projectId !== store.selectedProjectId || !live.alive) return null
  if (live.sessionId && !isPendingSessionId(live.sessionId)) {
    return { toolId: live.toolId, sessionId: live.sessionId, title: live.title }
  }
  return null
})

const canCiteTurns = computed(() => {
  const session = railSession.value
  return Boolean(session && (session.toolId === 'grokbuild' || session.toolId === 'kimi'))
})

async function loadDocs() {
  const seq = ++loadSeq
  const project = selectedProject.value
  const session = railSession.value
  const key = project && session ? `${project.id}|${session.toolId}|${session.sessionId}` : ''
  if (key !== loadedKey) { docs.value = []; query.value = ''; loadedKey = key }
  if (!project || !session) {
    refreshing.value = false
    docs.value = []
    status.value = 'idle'
    error.value = ''
    return
  }
  if (session.toolId === 'dsh') {
    refreshing.value = false
    docs.value = []
    status.value = 'empty'
    error.value = ''
    return
  }
  refreshing.value = true
  status.value = docs.value.length ? 'ready' : 'loading'
  try {
    const rows = await api.listSessionDocs(project.id, session.toolId, session.sessionId)
    if (seq !== loadSeq) return
    docs.value = rows
    if (tab.value === 'files' && !rows.some(doc => doc.kind !== 'summary') && rows.some(doc => doc.kind === 'summary')) tab.value = 'summaries'
    status.value = rows.length ? 'ready' : 'empty'
    error.value = ''
  } catch (err) {
    if (seq !== loadSeq) return
    error.value = err instanceof Error ? err.message : String(err)
    if (!docs.value.length) status.value = 'error'
  } finally {
    if (seq === loadSeq) refreshing.value = false
  }
}

function startPoll() {
  stopPoll()
  pollTimer = window.setInterval(() => {
    if (layout.docRailCollapsed) return
    void loadDocs()
  }, 15_000)
}

function stopPoll() {
  if (pollTimer) {
    window.clearInterval(pollTimer)
    pollTimer = 0
  }
}

watch(
  () => [
    store.appMode,
    store.selectedProjectId,
    railSession.value?.sessionId,
    railSession.value?.toolId,
    layout.docRailCollapsed
  ],
  () => {
    void loadDocs()
    if (store.appMode === 'console' && !layout.docRailCollapsed) startPoll()
    else stopPoll()
  },
  { immediate: true }
)

onUnmounted(() => {
  loadSeq += 1
  stopPoll()
})

function openCite() {
  const session = railSession.value
  if (!session) return
  emit('cite', session)
}

function tagClass(kind: string) {
  if (kind === 'summary') return 'is-summary'
  if (kind === 'plan') return 'is-plan'
  if (kind === 'spec') return 'is-spec'
  return 'is-doc'
}

const emptyCopy = computed(() => {
  if (!selectedProject.value) return '先选一个项目。'
  if (!railSession.value && !activeLive.value) return '选择一个会话，查看提到的 Markdown 文档和每一轮总结。'
  if (railSession.value?.toolId === 'dsh') {
    return 'DeepSeek Web 会话没有可预览文档。'
  }
  return '这次会话还没有提到 Markdown 文档，也还没有总结输出。'
})

const files = computed(() => docs.value.filter(doc => doc.kind !== 'summary'))
const summaries = computed(() => docs.value.filter(doc => doc.kind === 'summary'))
const shown = computed(() => (tab.value === 'files' ? files.value : summaries.value).filter(doc =>
  !query.value.trim() || `${doc.title} ${doc.relPath || ''} ${doc.excerpt || ''}`.toLowerCase().includes(query.value.trim().toLowerCase())
))
const excerpt = (doc: SessionDoc) => (doc.excerpt || '').replace(/[#*`>]/g, '').trim()
const title = (doc: SessionDoc) => doc.kind === 'summary' ? doc.title.replace(/^第 \d+ 轮 · /, '') : doc.title
</script>

<template>
  <aside class="rail" aria-label="会话文档">
    <div class="body" :aria-hidden="collapsed">
      <div class="head"><div><h2>会话文档</h2><p>{{ files.length }} 份文档 · {{ summaries.length }} 轮总结</p></div><button type="button" class="icon-btn" title="收起文档栏" aria-label="收起文档栏" @click="toggleDocRail">›</button></div>
      <div v-if="railSession" class="session"><span class="session-tool">{{ toolLabel(railSession.toolId) }}</span><span class="session-title" :title="railSession.title">{{ railSession.title }}</span></div>
      <div class="tabs" role="tablist" aria-label="文档分类"><button id="doc-files-tab" type="button" role="tab" :aria-selected="tab === 'files'" aria-controls="session-doc-list" @click="tab = 'files'">Markdown 文档<span>{{ files.length }}</span></button><button id="doc-summaries-tab" type="button" role="tab" :aria-selected="tab === 'summaries'" aria-controls="session-doc-list" @click="tab = 'summaries'">每轮总结<span>{{ summaries.length }}</span></button></div>
      <div class="toolbar"><input v-model="query" type="search" :placeholder="tab === 'files' ? '搜索文件名或路径' : '搜索问题或总结内容'" aria-label="搜索会话文档" /><button type="button" class="text-btn" :disabled="refreshing || !railSession" @click="loadDocs">{{ refreshing ? '刷新中' : '刷新' }}</button></div>
      <div v-if="status === 'loading'" class="state muted" role="status">正在整理会话文档和总结…</div>
      <div v-else-if="status === 'error'" class="state"><p>{{ error }}</p><button type="button" class="btn btn-ghost btn-small" @click="loadDocs">重试</button></div>
      <div v-else-if="status === 'idle' || status === 'empty'" class="state empty-state"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 3h8l4 4v14H6zM14 3v5h5M9 12h6m-6 4h6" /></svg><h3>{{ railSession ? '暂无会话资料' : '选择会话后查看文档' }}</h3><p>{{ emptyCopy }}</p></div>
      <div v-else-if="!shown.length" class="state muted" role="status">{{ query.trim() ? '未找到匹配结果' : tab === 'files' ? '暂未识别到 Markdown 文档，可查看每轮总结。' : '此会话暂无总结输出。' }}</div>
      <ul v-else id="session-doc-list" class="docs" role="tabpanel" :aria-labelledby="tab === 'files' ? 'doc-files-tab' : 'doc-summaries-tab'">
        <li v-for="doc in shown" :key="doc.path"><button type="button" class="doc" :class="{ 'doc-summary': doc.kind === 'summary', 'doc-missing': doc.exists === false }" @click="emit('open', doc)">
          <span class="doc-top"><span class="ad-tag" :class="tagClass(doc.kind)">{{ doc.kind === 'summary' && doc.round ? `第 ${doc.round} 轮` : kindLabel(doc.kind) }}</span><span class="doc-time" :title="doc.updatedAt ? new Date(doc.updatedAt).toLocaleString() : ''">{{ relativeTime(doc.updatedAt) }}</span></span>
          <span class="doc-main"><svg v-if="doc.kind !== 'summary'" class="doc-glyph" viewBox="0 0 16 16" aria-hidden="true"><path d="M4.5 2.5h4.2L12.5 6.3V13.5H4.5zM8.6 2.5V6.3H12.5" /></svg><span class="doc-title" :title="title(doc)">{{ title(doc) }}</span><span class="open-arrow" aria-hidden="true">↗</span></span>
          <span v-if="excerpt(doc)" class="doc-excerpt">{{ excerpt(doc) }}</span>
          <span class="doc-path" :title="doc.relPath || doc.path">{{ doc.kind === 'summary' ? '已保存到本机 · 点击阅读全文' : doc.relPath || doc.path }}</span><span v-if="doc.exists === false" class="missing-label">已提及，文件尚未生成或已移动</span>
        </button></li>
      </ul>
      <footer><span>{{ tab === 'summaries' ? '每轮独立保存，重新打开会话可回看' : '包含会话中提到、读取和写入的 Markdown' }}</span><button v-if="canCiteTurns" type="button" class="text-btn" @click="openCite">引用片段</button></footer>
    </div>
  </aside>
</template>

<style scoped>
.rail { position: relative; flex: 1; min-width: 0; height: 100%; background: var(--ad-float-inset); overflow: hidden; }
.body { width: 100%; height: 100%; display: flex; flex-direction: column; min-height: 0; }
.head { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 20px 16px 12px; flex-shrink: 0; }
h2 { margin: 0; font-size: 15px; line-height: 24px; font-weight: 600; }.head p { margin: 4px 0 0; font-size: 11px; color: var(--ad-muted); }
.session { display: flex; align-items: center; gap: 8px; min-width: 0; padding: 0 16px 16px; flex-shrink: 0; }.session-tool { flex-shrink: 0; font-size: 11px; color: var(--ad-claude); }.session-title { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; color: var(--ad-muted); }
.tabs { display: flex; gap: 20px; padding: 0 16px; border-bottom: 1px solid var(--ad-border); flex-shrink: 0; }.tabs button { display: inline-flex; align-items: center; gap: 8px; height: 40px; border-bottom: 2px solid transparent; font-size: 12px; color: var(--ad-muted); }.tabs button[aria-selected='true'] { color: var(--ad-text); border-bottom-color: var(--ad-text); }.tabs span { font-size: 10px; background: var(--ad-hover); border-radius: 4px; padding: 0 5px; line-height: 18px; font-variant-numeric: tabular-nums; }
.toolbar { display: flex; align-items: center; gap: 12px; padding: 12px 16px; flex-shrink: 0; }.toolbar input { width: 100%; min-width: 0; height: 32px; font-size: 12px; }.text-btn { flex-shrink: 0; padding: 0; font-size: 12px; line-height: 24px; color: var(--ad-muted); }.text-btn:hover:not(:disabled) { color: var(--ad-text); }
.docs { flex: 1; min-height: 0; overflow: auto; scrollbar-gutter: stable; margin: 0; padding: 0 12px 16px; list-style: none; }
.doc { display: flex; flex-direction: column; gap: 8px; width: 100%; padding: 14px 12px; margin-bottom: 8px; border: 1px solid var(--ad-border); border-radius: 6px; background: var(--ad-harbor); text-align: left; }.doc:hover { background: var(--ad-hover); border-color: var(--ad-border-strong); }.doc-summary { border-left: 2px solid var(--ad-claude); }
.doc-top { display: flex; align-items: center; justify-content: space-between; gap: 8px; }.doc-main { display: flex; align-items: flex-start; gap: 8px; min-width: 0; }.doc-glyph { flex-shrink: 0; width: 16px; height: 18px; fill: none; stroke: var(--ad-muted); stroke-width: 1.2; }.doc-title { min-width: 0; overflow: hidden; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow-wrap: anywhere; font-size: 13px; line-height: 20px; font-weight: 600; color: var(--ad-text); }.open-arrow { margin-left: auto; color: var(--ad-muted); font-size: 12px; }.doc-excerpt { color: var(--ad-muted); font-size: 12px; line-height: 20px; display: -webkit-box; -webkit-line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; overflow-wrap: anywhere; }
.ad-tag.is-summary { color: var(--ad-claude); background: color-mix(in srgb, var(--ad-claude) 12%, transparent); }.ad-tag.is-plan { color: var(--ad-kimi); background: color-mix(in srgb, var(--ad-kimi) 12%, transparent); }.ad-tag.is-spec { color: var(--ad-warning); background: color-mix(in srgb, var(--ad-warning) 12%, transparent); }.ad-tag.is-doc { color: var(--ad-muted); background: var(--ad-hover); }
.doc-path,.doc-time { font-size: 11px; line-height: 18px; color: var(--ad-muted); }.doc-path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.doc-missing { opacity: .75; }.missing-label { font-size: 11px; color: var(--ad-warning); }
.state { padding: 24px 16px; font-size: 13px; line-height: 22px; color: var(--ad-muted); }.state p { margin: 0 0 12px; }.empty-state { margin: 24px 0; text-align: center; }.empty-state svg { width: 32px; height: 32px; fill: none; stroke: var(--ad-muted); stroke-width: 1.2; margin-bottom: 12px; }.empty-state h3 { margin: 0 0 8px; color: var(--ad-text); font-size: 13px; font-weight: 500; }.empty-state p { max-width: 280px; margin: 0 auto; font-size: 12px; }
.icon-btn { width: 28px; height: 28px; color: var(--ad-muted); border-radius: 4px; font-size: 20px; }.icon-btn:hover { color: var(--ad-text); background: var(--ad-hover); }
footer { display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 8px; padding: 12px 16px; margin-top: auto; border-top: 1px solid var(--ad-border); flex-shrink: 0; font-size: 11px; color: var(--ad-muted); }
</style>
