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
let pollTimer = 0
let loadSeq = 0

const railSession = computed(() => {
  if (store.focusedSession && !isPendingSessionId(store.focusedSession.sessionId)) {
    return store.focusedSession
  }
  const live = store.live.find(
    (item) =>
      item.projectId === store.selectedProjectId &&
      item.alive &&
      item.sessionId &&
      !isPendingSessionId(item.sessionId)
  )
  if (live?.sessionId) {
    return { toolId: live.toolId, sessionId: live.sessionId, title: live.title }
  }
  return null
})

const canCiteTurns = computed(() => {
  const session = railSession.value
  return Boolean(session && (session.toolId === 'grokbuild' || session.toolId === 'kimi'))
})

async function loadDocs() {
  const project = selectedProject.value
  const session = railSession.value
  if (!project || !session) {
    docs.value = []
    status.value = 'idle'
    error.value = ''
    return
  }
  if (session.toolId === 'dsh') {
    docs.value = []
    status.value = 'empty'
    error.value = ''
    return
  }
  const seq = ++loadSeq
  status.value = docs.value.length ? 'ready' : 'loading'
  try {
    const rows = await api.listSessionDocs(project.id, session.toolId, session.sessionId)
    if (seq !== loadSeq) return
    docs.value = rows
    status.value = rows.length ? 'ready' : 'empty'
    error.value = ''
  } catch (err) {
    if (seq !== loadSeq) return
    error.value = err instanceof Error ? err.message : String(err)
    if (!docs.value.length) status.value = 'error'
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

const emptyCopy = computed(() => {
  if (!selectedProject.value) return '先选一个项目。'
  if (!railSession.value && !activeLive.value) return '打开会话后，这里会列出这次写出的 Markdown 和最后总结。'
  if (railSession.value?.toolId === 'dsh') {
    return 'DeepSeek Web 会话没有可预览文档。'
  }
  return '这次会话还没有写出 Markdown，也还没有可展示的总结。'
})
</script>

<template>
  <aside class="rail" aria-label="会话文档">
    <div class="body" :style="{ width: layout.docRailWidth + 'px' }" :aria-hidden="collapsed">
      <div class="head">
        <p class="kicker">文档</p>
        <button type="button" class="icon-btn" title="收起文档栏" @click="toggleDocRail">›</button>
      </div>

      <div v-if="railSession" class="session">
        <span class="session-tool">{{ toolLabel(railSession.toolId) }}</span>
        <span class="session-title" :title="railSession.title">{{ railSession.title }}</span>
      </div>

      <div class="toolbar">
        <button type="button" class="text-btn" :disabled="!canCiteTurns" @click="openCite">引用片段</button>
        <button type="button" class="text-btn" :disabled="status === 'loading'" @click="loadDocs">刷新</button>
      </div>

      <div v-if="status === 'loading'" class="state muted">正在读取文档</div>
      <div v-else-if="status === 'error'" class="state">
        <p>{{ error }}</p>
        <button type="button" class="btn btn-ghost btn-small" @click="loadDocs">重试</button>
      </div>
      <div v-else-if="status === 'idle' || status === 'empty'" class="state muted">{{ emptyCopy }}</div>
      <ul v-else class="docs">
        <li v-for="doc in docs" :key="doc.path">
          <button type="button" class="doc" @click="emit('open', doc)">
            <span class="doc-top">
              <span class="ad-tag" :class="{ 'is-summary': doc.kind === 'summary' }">{{ kindLabel(doc.kind) }}</span>
              <span class="doc-time">{{ relativeTime(doc.updatedAt) }}</span>
            </span>
            <span class="doc-title" :title="doc.title">{{ doc.title }}</span>
            <span class="doc-path" :title="doc.relPath || doc.path">{{
              doc.kind === 'summary' ? '来自会话记录' : doc.relPath || '会话文件'
            }}</span>
          </button>
        </li>
      </ul>
    </div>
  </aside>
</template>

<style scoped>
.rail {
  position: relative;
  flex: 1;
  min-width: 0;
  height: 100%;
  background: var(--ad-sidebar);
  overflow: hidden;
}

.body {
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 36px;
  padding: 0 8px 0 12px;
}

.kicker {
  margin: 0;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.session {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  padding: 0 12px 8px;
}

.session-tool {
  flex-shrink: 0;
  font-size: 12px;
  color: var(--ad-muted);
}

.session-title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
}

.toolbar {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  padding: 0 12px 8px;
}

.text-btn {
  padding: 0;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.text-btn:hover:not(:disabled) {
  color: var(--ad-text);
}

.docs {
  flex: 1;
  min-height: 0;
  overflow: auto;
  margin: 0;
  padding: 0 8px 16px;
  list-style: none;
}

.doc {
  display: flex;
  flex-direction: column;
  gap: 2px;
  width: 100%;
  padding: 8px 10px;
  margin-bottom: 4px;
  border-radius: 10px;
  text-align: left;
}

.doc:hover {
  background: var(--ad-hover);
}

.doc-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.doc-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ad-tag.is-summary {
  color: var(--ad-text);
  background: var(--ad-selected);
}

.doc-path,
.doc-time {
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

.doc-path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.state {
  padding: 16px 12px;
  font-size: 13px;
  line-height: 20px;
}

.state p {
  margin: 0 0 12px;
}

.icon-btn {
  width: 28px;
  height: 28px;
  color: var(--ad-muted);
  border-radius: 8px;
}

.icon-btn:hover {
  color: var(--ad-text);
  background: var(--ad-hover);
}
</style>
