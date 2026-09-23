<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { endPaneAnim, layout, sidebarPaneWidth, toggleProjects } from '../lib/layout'
import { liveDotTitle, projectLiveCounts, projectLiveDot, type LiveDotKind } from '../lib/livePulse'
import {
  createFlowFromTemplate,
  deleteFlow,
  renameFlow,
  selectFlow
} from '../lib/flow/runtime.ts'
import { FLOW_TEMPLATES } from '../lib/flow/templates.ts'
import { inspectPty } from '../lib/flow/channels.ts'
import { currentFlow, currentRun, ensurePipeline, store } from '../lib/store'

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

function onSelectProject(id: string) {
  emit('select', id)
  ensurePipeline(id)
}

const now = ref(Date.now())
let pulseTimer = 0

function projectDot(projectId: string): LiveDotKind {
  return projectLiveDot(store.live, projectId, store.ptyDataAt, now.value)
}

function projectCounts(projectId: string) {
  return projectLiveCounts(store.live, projectId, store.ptyDataAt, now.value)
}

const flows = computed(() => store.projectFlows[store.selectedProjectId]?.flows ?? [])
const selectedFlowId = computed(() => store.projectFlows[store.selectedProjectId]?.selectedFlowId ?? '')
const renamingId = ref('')
const renameDraft = ref('')
const confirmDeleteId = ref('')
const templateOpen = ref(false)

function startRename(id: string, name: string) {
  renamingId.value = id
  renameDraft.value = name
}

function commitRename() {
  if (renamingId.value && renameDraft.value.trim()) {
    renameFlow(renamingId.value, renameDraft.value)
  }
  renamingId.value = ''
}

function onDelete(id: string) {
  if (confirmDeleteId.value === id) {
    deleteFlow(id)
    confirmDeleteId.value = ''
    return
  }
  confirmDeleteId.value = id
}

onMounted(() => {
  pulseTimer = window.setInterval(() => {
    now.value = Date.now()
  }, 500)
})

onUnmounted(() => {
  window.clearInterval(pulseTimer)
})

const runHint = computed(() => {
  const run = currentRun.value
  if (!run) return ''
  if (run.status === 'running') return '运行中'
  if (run.status === 'waiting') return '待桥接'
  if (run.status === 'failed') return '失败'
  if (run.status === 'canceled') return '已停止'
  if (run.status === 'completed') return '已完成'
  return ''
})

const flowHint = computed(() => {
  const flow = currentFlow.value
  if (!flow) return '一对一：开发者完成后交给审查者，未通过再回到开发者。'
  const unbound = flow.nodes.filter((node) => (
    node.channel.kind === 'pty' && !inspectPty(node.channel, store.live, store.selectedProjectId).ok
  )).length
  if (unbound) return `${unbound} 个窗口节点还没绑好。`
  if (flow.nodes.length === 2 && flow.edges.some((edge) => edge.gate === 'passFail')) {
    return '一对一：开发者完成后交给审查者，未通过再回到开发者。'
  }
  return `串行 ${flow.nodes.length} 个节点。`
})

function onCreate(id: (typeof FLOW_TEMPLATES)[number]['id']) {
  templateOpen.value = false
  createFlowFromTemplate(id)
}
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
                <span v-if="projectCounts(project.id).open" class="project-counts">
                  <span
                    class="project-live"
                    :title="projectCounts(project.id).open + ' 个已打开窗口'"
                  >
                    {{ projectCounts(project.id).open }}
                  </span>
                  <span
                    v-if="projectCounts(project.id).busy"
                    class="project-busy"
                    :title="projectCounts(project.id).busy + ' 个正在运行'"
                  >
                    {{ projectCounts(project.id).busy }}
                  </span>
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

      <div class="block block--flows">
        <div class="block-head">
          <p class="kicker">已保存的流程</p>
          <div class="new-wrap">
            <button
              type="button"
              class="text-btn"
              :disabled="!store.selectedProjectId"
              @click="templateOpen = !templateOpen"
            >
              新建
            </button>
            <div v-if="templateOpen" class="templates" role="menu">
              <button
                v-for="item in FLOW_TEMPLATES"
                :key="item.id"
                type="button"
                class="template"
                @click="onCreate(item.id)"
              >
                <strong>{{ item.name }}</strong>
                <span>{{ item.hint }}</span>
              </button>
            </div>
          </div>
        </div>
        <p class="hint">{{ flowHint }}</p>
        <ul v-if="flows.length" class="flows">
          <li v-for="item in flows" :key="item.id">
            <div class="flow" :class="{ 'flow--on': item.id === selectedFlowId }">
              <button type="button" class="flow-main" @click="selectFlow(item.id)" @dblclick="startRename(item.id, item.name)">
                <span class="flow-name" :title="item.name">
                  <input
                    v-if="renamingId === item.id"
                    v-model="renameDraft"
                    class="rename"
                    @click.stop
                    @keydown.enter="commitRename"
                    @blur="commitRename"
                  />
                  <template v-else>{{ item.name }}</template>
                </span>
                <span v-if="item.id === selectedFlowId && runHint" class="flow-run">{{ runHint }}</span>
              </button>
              <div class="project-ops">
                <button type="button" class="row-btn" @click.stop="startRename(item.id, item.name)">改名</button>
                <button
                  type="button"
                  class="row-btn row-btn-danger"
                  @click.stop="onDelete(item.id)"
                >
                  {{ confirmDeleteId === item.id ? '确认删' : '删除' }}
                </button>
              </div>
            </div>
          </li>
        </ul>
        <p v-else class="muted pad">还没有流程。点新建会放好「开发 ↔ 审查」。</p>
        <p v-if="currentFlow" class="meta pad">当前：{{ currentFlow.name }} · {{ currentFlow.nodes.length }} 个节点</p>
        <p v-if="currentRun?.lastError && currentRun.status !== 'running'" class="meta pad run-error">
          {{ currentRun.lastError }}
        </p>
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

.block--flows {
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

.new-wrap {
  position: relative;
}

.templates {
  position: absolute;
  top: 100%;
  right: 0;
  z-index: 8;
  width: 220px;
  padding: 6px;
  border: 1px solid var(--ad-border);
  border-radius: 10px;
  background: var(--ad-raised);
  box-shadow: var(--ad-shadow-menu);
}

.template {
  display: flex;
  flex-direction: column;
  gap: 2px;
  width: 100%;
  padding: 8px;
  text-align: left;
  border-radius: 8px;
  color: var(--ad-text);
}

.template:hover {
  background: var(--ad-hover);
}

.template span {
  font-size: 11px;
  color: var(--ad-muted);
}

.run-error {
  color: var(--ad-error);
}

.projects {
  list-style: none;
  margin: 0;
  padding: 0 8px;
  max-height: 168px;
  overflow: auto;
}

.flows {
  list-style: none;
  margin: 0;
  padding: 0 8px 12px;
  overflow: auto;
  flex: 1;
}

.project,
.flow {
  display: flex;
  align-items: center;
  gap: 2px;
  margin-bottom: 2px;
  border-radius: 8px;
}

.project:hover,
.project:focus-within,
.flow:hover,
.flow:focus-within {
  background: var(--ad-hover);
}

.project--active,
.flow--on {
  background: var(--ad-selected);
}

.project-main,
.flow-main {
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

.project-counts {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}

.project-live,
.project-busy,
.flow-run {
  min-width: 16px;
  height: 16px;
  padding: 0 5px;
  border-radius: 999px;
  font-size: 11px;
  line-height: 16px;
  text-align: center;
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}

.project-live {
  background: color-mix(in srgb, var(--ad-success) 16%, transparent);
  color: var(--ad-success);
}

.project-busy {
  background: color-mix(in srgb, var(--ad-busy) 18%, transparent);
  color: var(--ad-busy);
}

.flow-run {
  background: color-mix(in srgb, var(--ad-warning) 16%, transparent);
  color: var(--ad-warning);
}

.project-body {
  min-width: 0;
  display: flex;
  flex-direction: column;
  flex: 1;
}

.project-name,
.flow-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--ad-text);
  flex: 1;
  min-width: 0;
}

.rename {
  width: 100%;
  height: 24px;
  padding: 0 6px;
  border: 1px solid var(--ad-border);
  border-radius: 6px;
  background: var(--ad-raised);
  color: var(--ad-text);
  font-size: 12px;
}

.muted,
.hint,
.meta,
.pad {
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

.pad {
  padding: 0 12px 8px;
}

.hint {
  margin: 0 12px 8px;
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
</style>
