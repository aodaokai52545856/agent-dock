<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import FlowChart from './FlowChart.vue'
import FlowNodeCard from './FlowNodeCard.vue'
import HandoffPanel from './HandoffPanel.vue'
import {
  addRoleNode,
  advanceSlice,
  clearSelection,
  markVerdict,
  persistCurrent,
  refreshGit,
  registerCustomRole,
  disconnectEdge,
  setEdgeBackTo,
  setEdgeGate,
  setEdgeMaxLoops,
  setEdgeMode,
  setPaneMode,
  startSelectedFlow,
  stopSelectedFlow
} from '../lib/flow/runtime.ts'
import { describeChannel } from '../lib/flow/channels.ts'
import { flowInspectorKind, flowInspectorTitle } from '../lib/flow/inspector.ts'
import { allRoles, roleLabel } from '../lib/flow/roles.ts'
import { FLOW_END, FLOW_START, type RunStep } from '../lib/flow/types.ts'
import { probeNote } from '../lib/pipeline'
import {
  currentFlow,
  currentPipeline,
  currentRun,
  projectHasLivePty,
  selectedProject,
  store
} from '../lib/store'
import { GATE_LABEL } from '../lib/types'

const busy = computed(() => currentRun.value?.status === 'running')
const waiting = computed(() => currentRun.value?.status === 'waiting')
const nextLocked = computed(() => currentPipeline.value?.gate !== 'passed')
const editing = computed(() => store.bridgePaneMode === 'edit')
const selectedNode = computed(
  () => currentFlow.value?.nodes.find((item) => item.id === store.bridgeSelectedNodeId) ?? null
)
const selectedEdge = computed(
  () => currentFlow.value?.edges.find((item) => item.id === store.bridgeSelectedEdgeId) ?? null
)
const activeNode = computed(
  () => currentFlow.value?.nodes.find((item) => item.id === currentRun.value?.currentNodeId) ?? null
)
const failedNodeId = computed(() => {
  const steps = currentRun.value?.steps || []
  return [...steps].reverse().find((step) => step.status === 'failed')?.nodeId || ''
})

const newRole = ref('')
const roles = computed(() => allRoles(store.customRoles))
const inspectorKind = computed(() =>
  flowInspectorKind(store.bridgePaneMode, store.bridgeSelectedNodeId, store.bridgeSelectedEdgeId)
)
const inspectorTitle = computed(() => flowInspectorTitle(inspectorKind.value))

const stageHint = computed(() => {
  if (editing.value) {
    return '开发接到审查，再从审查拉回开发，即成循环。点循环线可改最大次数。'
  }
  const run = currentRun.value
  if (run?.status === 'running') return '正在执行当前节点，图上高亮的是正在跑的角色。'
  if (run?.status === 'waiting') return '停在人工干预：改信封后再写入下一窗口。'
  if (run?.status === 'completed') return '本片已走完，可进入下一片。'
  if (run?.status === 'failed') return '未通过或失败，可标记结论或回到编排改图。'
  if (run?.status === 'canceled') return '本片已停止，可改图后重新运行。'
  return '填写本轮任务后运行。运行中可以在这里看预览并插手。'
})

function nodeTitle(nodeId: string) {
  const node = currentFlow.value?.nodes.find((item) => item.id === nodeId)
  return node?.title || (node ? roleLabel(node.role) : nodeId)
}

function stepStatus(step: RunStep) {
  if (step.status === 'working') return '进行中'
  if (step.status === 'completed') return '完成'
  if (step.status === 'failed') return '失败'
  if (step.status === 'canceled') return '已停止'
  if (step.status === 'input-required') return '待桥接'
  return step.status
}

function stepDuration(step: RunStep) {
  const end = step.endedAt || Date.now()
  const ms = Math.max(0, end - step.startedAt)
  if (ms < 1000) return `${ms} ms`
  return `${Math.round(ms / 1000)} s`
}

function addRole(id: string) {
  addRoleNode(id)
}

function submitNewRole() {
  const role = registerCustomRole(newRole.value)
  if (role) {
    newRole.value = ''
    addRoleNode(role.id)
  }
}

function goRun() {
  setPaneMode('run')
}

function goEdit() {
  setPaneMode('edit')
}

function onPaletteDrag(roleId: string, event: DragEvent) {
  event.dataTransfer?.setData('application/x-agent-dock-role', roleId)
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'copy'
}

function edgeFromLabel(edge: { from: string }) {
  if (edge.from === FLOW_START) return '开始'
  const node = currentFlow.value?.nodes.find((item) => item.id === edge.from)
  return node?.title || (node ? roleLabel(node.role) : '节点')
}

function edgeToLabel(edge: { to: string }) {
  if (edge.to === FLOW_END) return '结束'
  const node = currentFlow.value?.nodes.find((item) => item.id === edge.to)
  return node?.title || (node ? roleLabel(node.role) : '节点')
}

function onGate(edgeId: string, value: string) {
  const backTo = currentFlow.value?.nodes[0]?.id
  if (value === 'loop') setEdgeGate(edgeId, 'loop')
  else if (value === 'passFail') setEdgeGate(edgeId, 'passFail', backTo)
  else setEdgeGate(edgeId, 'none')
}

function onMaxLoops(edgeId: string, event: Event) {
  setEdgeMaxLoops(edgeId, Number((event.target as HTMLInputElement).value))
}

function onKey(event: KeyboardEvent) {
  if (event.key !== 'Escape') return
  if (!editing.value) return
  const target = event.target as HTMLElement | null
  if (target?.closest('input, textarea, select, [contenteditable="true"]')) return
  clearSelection()
}

onMounted(() => window.addEventListener('keydown', onKey))
onUnmounted(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <section class="pane" aria-label="编排画布">
    <div v-if="!selectedProject" class="empty">
      <h1>先选一个项目</h1>
      <p>左边上面选项目，下面会出现已保存的流程。右边用来画流程图，或看着跑并插手。</p>
    </div>
    <div v-else-if="currentFlow && currentPipeline" class="workbench">
      <header class="stage">
        <div class="modes" role="tablist" aria-label="画布模式">
          <button type="button" class="mode" :class="{ 'is-on': editing }" @click="goEdit">编排</button>
          <button type="button" class="mode" :class="{ 'is-on': !editing }" @click="goRun">运行</button>
        </div>
        <div class="stage-main">
          <h1>{{ currentFlow.name }}</h1>
          <p class="stage-hint">{{ stageHint }}</p>
        </div>
        <button
          v-if="!editing && (busy || waiting)"
          type="button"
          class="soft-btn"
          @click="stopSelectedFlow"
        >
          停止本片
        </button>
        <p class="badge" :data-gate="currentPipeline.gate">{{ GATE_LABEL[currentPipeline.gate] }}</p>
      </header>

      <p v-if="projectHasLivePty && !editing" class="banner">
        控制台里这个项目还有打开的终端。审查或自动改文件时，不要同时写同一棵树。
      </p>

      <div class="board">
        <div class="board-chart">
          <div class="chart-wrap">
            <FlowChart :flow="currentFlow" :readonly="!editing" />
            <aside
              v-if="inspectorKind"
              class="ad-drawer inspector"
              :aria-label="inspectorTitle"
              @click.stop
              @pointerdown.stop
            >
              <header class="inspector-head">
                <p class="side-kicker">{{ inspectorTitle }}</p>
                <button
                  v-if="inspectorKind !== 'run'"
                  type="button"
                  class="text-btn"
                  @click="clearSelection"
                >
                  关闭
                </button>
              </header>
          <template v-if="editing">
            <template v-if="selectedEdge">
              <article class="edge-card">
                <p class="hint">
                  {{ edgeFromLabel(selectedEdge) }}
                  →
                  {{ edgeToLabel(selectedEdge) }}
                </p>
                <label class="field">
                  <span>完成后</span>
                  <select
                    :value="selectedEdge.mode"
                    @change="setEdgeMode(selectedEdge.id, ($event.target as HTMLSelectElement).value as 'auto' | 'manual')"
                  >
                    <option value="auto">自动</option>
                    <option value="manual">手动桥接</option>
                  </select>
                </label>
                <label class="field">
                  <span>闸</span>
                  <select
                    :value="selectedEdge.gate"
                    @change="onGate(selectedEdge.id, ($event.target as HTMLSelectElement).value)"
                  >
                    <option value="none">走下一节点</option>
                    <option value="passFail">通过才结束，未通过返回</option>
                    <option value="loop">循环回去</option>
                  </select>
                </label>
                <label v-if="selectedEdge.gate === 'passFail'" class="field">
                  <span>未通过回到</span>
                  <select
                    :value="selectedEdge.backTo || ''"
                    @change="setEdgeBackTo(selectedEdge.id, ($event.target as HTMLSelectElement).value)"
                  >
                    <option v-for="node in currentFlow.nodes" :key="node.id" :value="node.id">
                      {{ node.title || roleLabel(node.role) }}
                    </option>
                  </select>
                </label>
                <label v-if="selectedEdge.gate === 'loop'" class="field">
                  <span>最大循环次数</span>
                  <input
                    type="number"
                    min="1"
                    max="20"
                    :value="selectedEdge.maxLoops || 3"
                    @change="onMaxLoops(selectedEdge.id, $event)"
                  />
                </label>
                <p v-if="selectedEdge.gate === 'loop'" class="hint">
                  用尽后走该节点的另一条线（通常接到「结束」）。没有另一条线则结束本片。
                </p>
                <button type="button" class="soft-btn" @click="disconnectEdge(selectedEdge.id)">删除这条连线</button>
              </article>
            </template>
            <template v-else>
              <FlowNodeCard
                v-if="selectedNode"
                :node="selectedNode"
              />
            </template>
          </template>

          <template v-else>
            <label class="field">
              <span>本轮任务</span>
              <textarea
                v-model="currentFlow.draftTask"
                rows="4"
                maxlength="4000"
                placeholder="这一片要做的事。"
                @change="persistCurrent"
              />
            </label>

            <HandoffPanel />

            <div class="actions">
              <button type="button" class="btn btn-primary" :disabled="busy" @click="startSelectedFlow()">
                {{ busy ? '运行中…' : '运行本片' }}
              </button>
              <button
                v-if="failedNodeId"
                type="button"
                class="soft-btn"
                :disabled="busy"
                @click="startSelectedFlow(failedNodeId)"
              >
                从失败节点重跑
              </button>
              <button type="button" class="soft-btn" :disabled="busy" @click="markVerdict('pass')">标记通过</button>
              <button type="button" class="soft-btn" :disabled="busy" @click="markVerdict('fail')">标记未通过</button>
              <button type="button" class="soft-btn" :disabled="nextLocked || busy || waiting" @click="advanceSlice">
                进入下一片
              </button>
            </div>
            <p v-if="currentPipeline.lastError" class="error">{{ currentPipeline.lastError }}</p>
            <p v-if="currentPipeline.retryCount" class="meta">
              已返工 {{ currentPipeline.retryCount }} / {{ currentPipeline.maxRetries }} 次
            </p>

            <section v-if="currentRun" class="activity">
              <h2>活动</h2>
              <p v-if="activeNode" class="meta">
                当前：{{ activeNode.title || roleLabel(activeNode.role) }}
                · {{ describeChannel(activeNode.channel, store.live) }}
              </p>
              <p v-if="currentRun.lastEnvelope?.done" class="envelope-preview">{{ currentRun.lastEnvelope.done }}</p>
              <ol v-if="currentRun.steps.length" class="steps">
                <li v-for="step in currentRun.steps" :key="step.id" :data-status="step.status">
                  <div class="step-head">
                    <strong>{{ nodeTitle(step.nodeId) }}</strong>
                    <span>{{ stepStatus(step) }} · {{ stepDuration(step) }}</span>
                  </div>
                  <pre v-if="step.envelope?.done" class="review-text">{{ step.envelope.done }}</pre>
                  <p v-if="step.error" class="error">{{ step.error }}</p>
                </li>
              </ol>
            </section>

            <div class="probe-row">
              <span class="probe-dot" :class="{ 'is-ok': store.codexProbe?.ok }" />
              <div>
                <strong>Codex App Server</strong>
                <p class="meta">{{ probeNote(store.codexProbe) }}</p>
              </div>
            </div>
            <article class="side-card">
              <div class="card-head">
                <h2>工作区</h2>
                <button type="button" class="text-btn" @click="refreshGit">刷新 diff</button>
              </div>
              <p v-if="currentPipeline.git" class="git-line">
                {{ currentPipeline.git.branch }}
                · {{ currentPipeline.git.head.slice(0, 8) || '—' }}
                · {{ currentPipeline.git.dirty ? '有未提交改动' : '工作区干净' }}
              </p>
              <pre v-if="currentPipeline.git?.summary" class="diff">{{ currentPipeline.git.summary }}</pre>
              <p v-else class="meta">还没有 git 摘要。</p>
            </article>
          </template>
            </aside>
          </div>
          <div v-if="editing" class="palette">
            <span class="palette-label">角色库</span>
            <button
              v-for="role in roles"
              :key="role.id"
              type="button"
              class="chip"
              draggable="true"
              @click="addRole(role.id)"
              @dragstart="onPaletteDrag(role.id, $event)"
            >
              {{ role.label }}
            </button>
            <label class="new-role">
              <input v-model="newRole" maxlength="16" placeholder="新角色名" @keydown.enter="submitNewRole" />
              <button type="button" class="chip" :disabled="!newRole.trim()" @click="submitNewRole">添加</button>
            </label>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.pane {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  background: var(--ad-editor);
}

.empty,
.workbench {
  flex: 1;
  min-height: 0;
  width: 100%;
  padding: 16px 20px 16px;
}

.workbench {
  display: flex;
  flex-direction: column;
}

.empty {
  max-width: 560px;
  margin: 0 auto;
  padding-top: 72px;
  overflow: auto;
}

h1 {
  margin: 0;
  font-size: 18px;
  line-height: 26px;
  font-weight: 560;
}

h2,
h3 {
  margin: 0;
  font-size: 13px;
  line-height: 20px;
  font-weight: 560;
}

.stage {
  display: flex;
  align-items: flex-start;
  gap: 16px;
  margin-bottom: 12px;
  flex-shrink: 0;
}

.modes {
  display: flex;
  padding: 2px;
  border-radius: 8px;
  border: 1px solid var(--ad-border);
  background: var(--ad-raised);
  flex-shrink: 0;
}

.mode {
  height: 28px;
  padding: 0 12px;
  border-radius: 6px;
  font-size: 12px;
  color: var(--ad-muted);
}

.mode.is-on {
  background: var(--ad-selected);
  color: var(--ad-text);
}

.stage-main {
  min-width: 0;
  flex: 1;
}

.stage-hint,
.meta,
.muted {
  margin: 4px 0 0;
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

.badge {
  margin: 0;
  height: 28px;
  padding: 0 10px;
  border-radius: 999px;
  background: var(--ad-raised);
  border: 1px solid var(--ad-border);
  display: grid;
  place-items: center;
  font-size: 12px;
  flex-shrink: 0;
}

.badge[data-gate='passed'] {
  color: var(--ad-success);
}

.badge[data-gate='failed'] {
  color: var(--ad-error);
}

.badge[data-gate='reviewing'],
.badge[data-gate='developing'],
.badge[data-gate='pendingReview'] {
  color: var(--ad-warning);
}

.banner {
  flex-shrink: 0;
  margin: 0 0 16px;
  padding: 10px 12px;
  border-radius: 10px;
  background: rgba(201, 162, 39, 0.12);
  color: #e6d08a;
  font-size: 13px;
}

.board {
  flex: 1;
  min-height: 0;
  display: flex;
  min-width: 0;
}

.board-chart {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--ad-border);
  border-radius: 12px;
  background: var(--ad-harbor);
}

.chart-wrap {
  flex: 1;
  min-height: 0;
  position: relative;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.chart-wrap :deep(.chart) {
  flex: 1;
  min-height: 0;
}

.inspector {
  bottom: auto;
  width: min(380px, calc(100% - 24px));
  max-height: calc(100% - 24px);
  padding: 14px 16px 16px;
  z-index: 6;
}

.inspector-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 10px;
}

.inspector-head .side-kicker {
  margin: 0;
}

.inspector :deep(.card),
.inspector .edge-card {
  padding: 0;
  border: 0;
  border-radius: 0;
  background: transparent;
}

.palette {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
  padding: 10px 12px 12px;
  border-top: 1px solid var(--ad-border);
  flex-shrink: 0;
}

.palette-label,
.side-kicker {
  font-size: 12px;
  color: var(--ad-faint);
}

.chip {
  height: 28px;
  padding: 0 10px;
  border-radius: 8px;
  border: 1px solid var(--ad-border);
  color: var(--ad-muted);
  font-size: 12px;
  cursor: grab;
}

.chip:hover:not(:disabled) {
  color: var(--ad-text);
  background: var(--ad-hover);
}

.edge-card {
  padding: 12px 14px;
  border: 1px solid var(--ad-border);
  border-radius: 12px;
  background: var(--ad-harbor);
}

.edge-card .hint {
  margin: 0 0 10px;
  font-size: 12px;
  color: var(--ad-muted);
}

.new-role {
  display: inline-flex;
  gap: 6px;
  margin-left: auto;
}

.new-role input {
  width: 120px;
  height: 28px;
  padding: 0 8px;
}

.activity {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 16px 0;
}

.steps {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.steps li {
  padding: 8px 10px;
  border: 1px solid var(--ad-border);
  border-left-width: 3px;
  border-radius: 10px;
  background: var(--ad-harbor);
}

.steps li[data-status='completed'] {
  border-left-color: var(--ad-success);
}

.steps li[data-status='working'] {
  border-left-color: var(--ad-warning);
}

.steps li[data-status='failed'],
.steps li[data-status='canceled'] {
  border-left-color: var(--ad-error);
}

.step-head {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  font-size: 12px;
}

.envelope-preview {
  margin: 0;
  max-height: 72px;
  overflow: auto;
  font-size: 12px;
  color: var(--ad-muted);
  white-space: pre-wrap;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 12px;
}

.field > span {
  font-size: 12px;
  color: var(--ad-muted);
}

textarea {
  min-height: 96px;
  resize: vertical;
  padding: 10px 12px;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin: 12px 0;
}

.soft-btn,
.text-btn {
  height: 28px;
  padding: 0 10px;
  border-radius: 8px;
  font-size: 12px;
  color: var(--ad-muted);
}

.soft-btn:hover:not(:disabled),
.text-btn:hover {
  color: var(--ad-text);
  background: var(--ad-hover);
}

.error {
  color: var(--ad-error);
  font-size: 13px;
}

.timeline {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 16px 0;
}

.review-card {
  padding: 10px 12px;
  border: 1px solid var(--ad-border);
  border-left-width: 3px;
  border-radius: 10px;
  background: var(--ad-harbor);
}

.review-card[data-verdict='pass'] {
  border-left-color: var(--ad-success);
}

.review-card[data-verdict='fail'] {
  border-left-color: var(--ad-error);
}

.review-head {
  display: flex;
  justify-content: space-between;
  gap: 8px;
}

.verdict[data-verdict='pass'] {
  color: var(--ad-success);
}

.verdict[data-verdict='fail'] {
  color: var(--ad-error);
}

.review-text,
.diff {
  margin: 8px 0 0;
  max-height: 200px;
  overflow: auto;
  white-space: pre-wrap;
  font-family: var(--ad-mono);
  font-size: 12px;
  line-height: 18px;
}

.probe-row {
  display: flex;
  gap: 10px;
  padding: 10px 12px;
  margin-bottom: 10px;
  border: 1px solid var(--ad-border);
  border-radius: 10px;
}

.probe-dot {
  width: 8px;
  height: 8px;
  margin-top: 6px;
  border-radius: 50%;
  background: var(--ad-error);
}

.probe-dot.is-ok {
  background: var(--ad-success);
}

.side-card {
  padding: 12px 14px;
  border: 1px solid var(--ad-border);
  border-radius: 12px;
  background: var(--ad-harbor);
}

.card-head {
  display: flex;
  justify-content: space-between;
  margin-bottom: 8px;
}

.git-line {
  margin: 0;
  font-size: 13px;
}

@media (max-width: 900px) {
  .stage {
    flex-wrap: wrap;
  }

  .inspector {
    width: calc(100% - 24px);
  }
}
</style>
