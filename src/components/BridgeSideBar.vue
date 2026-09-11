<script setup lang="ts">
import { pathTail, relativeTime } from '../lib/format'
import { endPaneAnim, layout, paneDragging, sidebarPaneWidth, toggleSidebar } from '../lib/layout'
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
</script>

<template>
  <aside
    class="rail"
    :class="{ 'rail--collapsed': collapsed, 'rail--static': paneDragging }"
    :style="{ width: sidebarPaneWidth() + 'px' }"
    aria-label="编排工作区"
    @transitionend="onPaneTransitionEnd"
  >
    <div class="body" :style="{ width: layout.sidebarWidth + 'px' }" :aria-hidden="collapsed">
      <div class="head">
        <p class="kicker">编排</p>
        <button type="button" class="icon-btn" title="收起工作区" @click="toggleSidebar">‹</button>
      </div>

      <div class="block">
        <div class="block-head">
          <p class="kicker">项目</p>
          <button type="button" class="text-btn" @click="emit('add')">添加</button>
        </div>
        <ul v-if="store.projects.length" class="projects">
          <li v-for="project in store.projects" :key="project.id">
            <div
              class="project"
              :class="{
                'project--active': store.selectedProjectId === project.id,
                'project--proxy': project.proxyEnabled
              }"
            >
              <button type="button" class="project-main" @click="onSelectProject(project.id)">
                <svg class="glyph" viewBox="0 0 16 16" aria-hidden="true">
                  <path d="M2.5 5h4.1l1.1 1.3H13.5V12.5H2.5z" />
                </svg>
                <span class="project-body">
                  <span class="project-name">{{ project.name }}</span>
                  <span class="project-path" :title="project.path">{{ pathTail(project.path) }}</span>
                </span>
                <span v-if="project.proxyEnabled" class="proxy-dot" title="代理开" />
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
        <p v-else class="muted pad">还没有项目</p>
      </div>

      <div class="block gate">
        <p class="kicker">闸门</p>
        <p class="gate-line">
          <span class="gate-dot" :data-gate="currentPipeline?.gate ?? 'idle'" />
          {{ GATE_LABEL[currentPipeline?.gate ?? 'idle'] }}
          <span class="faint">第 {{ currentPipeline?.slice ?? 1 }} 片</span>
        </p>
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

    <div class="strip" :aria-hidden="!collapsed">
      <button type="button" class="icon-btn" title="展开工作区" @click="toggleSidebar">›</button>
      <span class="collapsed-label">编排</span>
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
  transition: width var(--ad-pane-move);
}

.rail--static {
  transition: none;
}

.body {
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
  opacity: 1;
  transition: opacity var(--ad-pane-fade);
}

.rail--collapsed .body {
  opacity: 0;
  pointer-events: none;
}

.strip {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 8px 0;
  opacity: 0;
  pointer-events: none;
  transition: opacity var(--ad-pane-fade);
}

.rail--collapsed .strip {
  opacity: 1;
  pointer-events: auto;
  transition-delay: 60ms;
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

.block-head,
.gate {
  padding: 0 12px;
}

.block-head {
  height: 32px;
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
.project:focus-within,
.project--active {
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

.proxy-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ad-success);
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

.project-path,
.muted,
.hint,
.thread-preview,
.faint,
.thread-meta,
.collapsed-label {
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

.project-path,
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

.gate-line {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0 0 10px;
  color: var(--ad-text);
}

.gate-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--ad-faint);
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
}
</style>
