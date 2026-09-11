<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { relativeTime } from '../lib/format'
import { endPaneAnim, layout, sidebarPaneWidth, toggleProjects } from '../lib/layout'
import { liveDotTitle, projectLiveDot, type LiveDotKind } from '../lib/livePulse'
import { liveOfProject } from '../lib/livePty'
import { refreshCodexThreads, toggleBridgeThread } from '../lib/pipeline'
import { currentPipeline, ensurePipeline, store } from '../lib/store'
import { GATE_LABEL } from '../lib/types'

defineProps<{
  collapsed: boolean
}>()

const emit = defineEmits<{
  add: []
  edit: [id: string]
  remove: [id: string]
  select: [id: string]
}>()

function onPaneTransitionEnd(event: TransitionEvent) {
  if (event.propertyName !== 'width') return
  if (event.target !== event.currentTarget) return
  endPaneAnim()
}

function isSelected(id: string) {
  return currentPipeline.value?.selectedThreadIds.includes(id) ?? false
}

function onSelectProject(id: string) {
  emit('select', id)
  ensurePipeline(id)
}

const now = ref(Date.now())
let pulseTimer = 0

function projectDot(projectId: string): LiveDotKind {
  return projectLiveDot(store.live, projectId, store.ptyDataAt, now.value)
}

function projectLiveCount(projectId: string) {
  return liveOfProject(store.live, projectId).length
}

onMounted(() => {
  pulseTimer = window.setInterval(() => {
    now.value = Date.now()
  }, 500)
})

onUnmounted(() => {
  window.clearInterval(pulseTimer)
})

const gate = computed(() => currentPipeline.value?.gate ?? 'idle')
const slice = computed(() => currentPipeline.value?.slice ?? 1)
const retryCount = computed(() => currentPipeline.value?.retryCount ?? 0)
const maxRetries = computed(() => currentPipeline.value?.maxRetries ?? 0)

/** Thin progress: retry ratio when failing/retrying; gate-based otherwise. */
const gateProgress = computed(() => {
  const g = gate.value
  if (g === 'passed') return 100
  if (g === 'failed') {
    const max = Math.max(1, maxRetries.value)
    return Math.min(100, Math.round((retryCount.value / max) * 100))
  }
  if (g === 'reviewing' || g === 'developing') return 55
  if (g === 'idle') return 0
  return 25
})
</script>

<template>
  <aside
    class="rail"
    :class="{ 'rail--collapsed': collapsed }"
    :style="{ width: sidebarPaneWidth() + 'px' }"
    aria-label="编排工作区"
    @transitionend="onPaneTransitionEnd"
  >
    <div class="body" :style="{ width: layout.sidebarWidth + 'px' }" :aria-hidden="collapsed">
      <div class="head">
        <p class="kicker">编排</p>
      </div>

      <div class="block">
        <div class="block-head">
          <button type="button" class="fold-btn" :aria-expanded="!layout.projectsCollapsed" @click="toggleProjects">
            <svg class="fold-caret" :class="{ 'is-closed': layout.projectsCollapsed }" viewBox="0 0 16 16" aria-hidden="true">
              <path d="M6 4.5 10 8l-4 3.5" />
            </svg>
            <span class="kicker">项目</span>
          </button>
          <button type="button" class="text-btn" @click="emit('add')">添加</button>
        </div>
        <ul v-if="!layout.projectsCollapsed && store.projects.length" class="projects">
          <li v-for="project in store.projects" :key="project.id">
            <div
              class="project"
              :class="{
                'project--active': store.selectedProjectId === project.id,
                'project--live': projectDot(project.id) !== 'off',
                'project--busy': projectDot(project.id) === 'busy'
              }"
              :title="liveDotTitle(projectDot(project.id), 'project') || undefined"
            >
              <button type="button" class="project-main" @click="onSelectProject(project.id)">
                <svg class="glyph" viewBox="0 0 16 16" aria-hidden="true">
                  <path d="M2.5 5h4.1l1.1 1.3H13.5V12.5H2.5z" />
                </svg>
                <span class="project-body">
                  <span class="project-name" :title="project.path">{{ project.name }}</span>
                </span>
                <span
                  v-if="projectLiveCount(project.id)"
                  class="project-live"
                  :title="projectLiveCount(project.id) + ' 个已打开会话'"
                >
                  {{ projectLiveCount(project.id) }}
                </span>
              </button>
              <div class="project-ops">
                <button type="button" class="row-btn" @click.stop="emit('edit', project.id)">编辑</button>
                <button type="button" class="row-btn row-btn-danger" @click.stop="emit('remove', project.id)">
                  移除
                </button>
              </div>
            </div>
          </li>
        </ul>
        <p v-else-if="!layout.projectsCollapsed" class="muted pad">还没有项目</p>
      </div>

      <div class="block gate-block">
        <div class="gate-card" :data-gate="gate">
          <div class="gate-card-top">
            <span class="gate-dot" :data-gate="gate" />
            <span class="gate-label">{{ GATE_LABEL[gate] }}</span>
            <span class="gate-slice">第 {{ slice }} 片</span>
          </div>
          <p v-if="retryCount" class="gate-retry">
            已返工 {{ retryCount }} / {{ maxRetries }} 次
          </p>
          <div class="gate-bar" role="progressbar" :aria-valuenow="gateProgress" aria-valuemin="0" aria-valuemax="100">
            <span class="gate-bar-fill" :style="{ width: gateProgress + '%' }" />
          </div>
        </div>
      </div>

      <div class="block block--threads">
        <div class="block-head">
          <p class="kicker">Codex 线程</p>
          <button type="button" class="text-btn" :disabled="store.codexStatus === 'loading'" @click="refreshCodexThreads">
            {{ store.codexStatus === 'loading' ? '读取中' : '刷新' }}
          </button>
        </div>
        <p class="hint">勾选后作为审查上下文。不会往桌面正在聊的 turn 里塞字。</p>
        <ul v-if="store.codexThreads.length" class="threads">
          <li v-for="thread in store.codexThreads" :key="thread.id">
            <label class="thread" :class="{ 'thread--on': isSelected(thread.id) }">
              <input type="checkbox" :checked="isSelected(thread.id)" @change="toggleBridgeThread(thread.id)" />
              <span class="thread-body">
                <span class="thread-name">{{ thread.name }}</span>
                <span class="thread-preview">{{ thread.preview || thread.id }}</span>
              </span>
              <span class="thread-meta">
                <span v-if="thread.isPinned" class="pin">钉</span>
                <span>{{ relativeTime(thread.updatedAt) }}</span>
              </span>
            </label>
          </li>
        </ul>
        <p v-else-if="store.codexStatus === 'loading'" class="muted pad">正在列出本机线程库…</p>
        <p v-else-if="store.codexError" class="muted pad">{{ store.codexError }}</p>
        <p v-else class="muted pad">这个目录还没有可桥接的线程。不勾选则会新开一条专用审查线程。</p>
      </div>
    </div>

  </aside>
</template>

<style scoped>
.rail {
  position: relative;
  flex-shrink: 0;
  background: var(--ad-sidebar);
  min-width: 0;
  overflow: hidden;
}

.body {
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.head,
.block-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.head {
  height: 36px;
  padding: 0 8px 0 12px;
}

.block {
  padding: 0 0 8px;
  border-bottom: 1px solid var(--ad-border);
}

.block--threads {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  border-bottom: none;
}

.block-head {
  padding: 0 12px;
  height: 32px;
}

.fold-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  height: 28px;
  padding: 0 4px 0 0;
  color: inherit;
}

.fold-caret {
  width: 12px;
  height: 12px;
  flex-shrink: 0;
  fill: none;
  stroke: var(--ad-muted);
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
  transform: rotate(90deg);
}

.fold-caret.is-closed {
  transform: rotate(0deg);
}

.kicker {
  margin: 0;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-faint);
}

.text-btn {
  padding: 0 4px;
  height: 28px;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.text-btn:hover:not(:disabled) {
  color: var(--ad-text);
}

.projects {
  list-style: none;
  margin: 0;
  padding: 0 8px;
  max-height: 168px;
  overflow: auto;
}

.project {
  display: flex;
  align-items: center;
  gap: 2px;
  margin-bottom: 2px;
  border-radius: 8px;
}

.project:hover,
.project:focus-within {
  background: var(--ad-hover);
}

.project--active {
  background: var(--ad-selected);
}

.project-main {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  text-align: left;
}

.glyph {
  width: 15px;
  height: 15px;
  flex-shrink: 0;
  fill: none;
  stroke: var(--ad-muted);
  stroke-width: 1.2;
  stroke-linejoin: round;
}

.project--active .glyph {
  stroke: var(--ad-text);
}

.project-live {
  min-width: 16px;
  height: 16px;
  padding: 0 5px;
  border-radius: 999px;
  background: rgba(74, 222, 128, 0.16);
  color: #4ade80;
  font-size: 11px;
  line-height: 16px;
  text-align: center;
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}

.project-body,
.thread-body {
  min-width: 0;
  display: flex;
  flex-direction: column;
  flex: 1;
}

.project-name,
.thread-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--ad-text);
}

.muted,
.hint,
.thread-preview,
.thread-meta,
.collapsed-label {
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

.thread-preview {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.project-ops {
  display: flex;
  gap: 2px;
  padding-right: 6px;
}

.row-btn {
  height: 24px;
  padding: 0 6px;
  font-size: 12px;
  color: var(--ad-muted);
  border-radius: 6px;
}

.row-btn:hover {
  color: var(--ad-text);
  background: rgba(255, 255, 255, 0.06);
}

.row-btn-danger:hover {
  color: var(--ad-error);
}

.pad {
  padding: 0 12px 8px;
}

.hint {
  margin: 0 12px 8px;
}

.gate-block {
  padding: 8px 12px 10px;
}

.gate-card {
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid var(--ad-border);
  background: var(--ad-harbor);
}

.gate-card-top {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.gate-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--ad-faint);
  flex-shrink: 0;
}

.gate-dot[data-gate='passed'] {
  background: var(--ad-success);
}

.gate-dot[data-gate='failed'] {
  background: var(--ad-error);
}

.gate-dot[data-gate='reviewing'],
.gate-dot[data-gate='developing'] {
  background: var(--ad-warning);
}

.gate-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--ad-text);
  font-size: 13px;
  line-height: 20px;
  font-weight: 560;
}

.gate-slice {
  flex-shrink: 0;
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
  font-variant-numeric: tabular-nums;
}

.gate-retry {
  margin: 6px 0 0;
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

.gate-bar {
  margin-top: 8px;
  height: 3px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.08);
  overflow: hidden;
}

.gate-bar-fill {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--ad-muted);
  transition: width var(--ad-transition);
}

.gate-card[data-gate='passed'] .gate-bar-fill {
  background: var(--ad-success);
}

.gate-card[data-gate='failed'] .gate-bar-fill {
  background: var(--ad-error);
}

.gate-card[data-gate='reviewing'] .gate-bar-fill,
.gate-card[data-gate='developing'] .gate-bar-fill {
  background: var(--ad-warning);
}

.threads {
  list-style: none;
  margin: 0;
  padding: 0 8px 16px;
  overflow: auto;
  flex: 1;
}

.thread {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px;
  margin-bottom: 2px;
  border-radius: 8px;
}

.thread:hover,
.thread--on {
  background: var(--ad-hover);
}

.thread input {
  margin: 0;
  accent-color: #ececec;
}

.thread-meta {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 2px;
  flex-shrink: 0;
}

.pin {
  font-size: 11px;
  color: var(--ad-text);
}

.icon-btn {
  width: 28px;
  height: 28px;
  border-radius: 8px;
  color: var(--ad-muted);
}

.icon-btn:hover {
  color: var(--ad-text);
  background: var(--ad-hover);
}

.collapsed-label {
  writing-mode: vertical-rl;
  font-size: 12px;
  line-height: 20px;
  letter-spacing: 2px;
}
</style>
