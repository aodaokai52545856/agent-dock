<script setup lang="ts">
import { computed, ref } from 'vue'
import FlowChart from './FlowChart.vue'
import FlowNodeCard from './FlowNodeCard.vue'
import HandoffPanel from './HandoffPanel.vue'
import {
  addRoleNode,
  advanceSlice,
  markVerdict,
  persistCurrent,
  refreshGit,
  registerCustomRole,
  setPaneMode,
  startSelectedFlow
} from '../lib/flow/runtime.ts'
import { allRoles } from '../lib/flow/roles.ts'
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
  () => currentFlow.value?.nodes.find((item) => item.id === store.bridgeSelectedNodeId) ?? currentFlow.value?.nodes[0] ?? null
)

const newRole = ref('')
const roles = computed(() => allRoles(store.customRoles))

const stageHint = computed(() => {
  if (editing.value) return '点角色放到图上。一对一：开发完成后审查，未通过回到开发者。'
  const run = currentRun.value
  if (run?.status === 'running') return '正在执行当前节点，图上高亮的是正在跑的角色。'
  if (run?.status === 'waiting') return '停在人工干预：改信封后再写入下一窗口。'
  if (run?.status === 'completed') return '本片已走完，可进入下一片。'
  if (run?.status === 'failed') return '未通过或失败，可标记结论或回到编排改图。'
  return '填写本轮任务后运行。运行中可以在这里看预览并插手。'
})

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
        <p class="badge" :data-gate="currentPipeline.gate">{{ GATE_LABEL[currentPipeline.gate] }}</p>
      </header>

      <p v-if="projectHasLivePty && !editing" class="banner">
        控制台里这个项目还有打开的终端。审查或自动改文件时，不要同时写同一棵树。
      </p>

      <div class="board" :class="{ 'board--run': !editing }">
        <div class="board-chart">
          <FlowChart :flow="currentFlow" :readonly="!editing && busy" />

          <div v-if="editing" class="palette">
            <span class="palette-label">角色库</span>
            <button v-for="role in roles" :key="role.id" type="button" class="chip" @click="addRole(role.id)">
              {{ role.label }}
            </button>
            <label class="new-role">
              <input v-model="newRole" maxlength="16" placeholder="新角色名" @keydown.enter="submitNewRole" />
              <button type="button" class="chip" :disabled="!newRole.trim()" @click="submitNewRole">添加</button>
            </label>
          </div>
        </div>

        <div class="board-side">
          <template v-if="editing">
            <p class="side-kicker">节点</p>
            <FlowNodeCard
              v-if="selectedNode"
              :node="selectedNode"
              :can-remove="currentFlow.nodes.length > 1"
            />
            <p v-else class="muted">点图上的框，在这里改通道和合同。</p>
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

            <section v-if="currentPipeline.reviews.length" class="timeline">
              <h2>审查记录</h2>
              <article
                v-for="(review, index) in currentPipeline.reviews"
                :key="review.reviewThreadId || index"
                class="review-card"
                :data-verdict="review.verdict"
              >
                <div class="review-head">
                  <h3>审查 {{ index + 1 }}</h3>
                  <span class="verdict" :data-verdict="review.verdict">{{
                    review.verdict === 'pass' ? '通过' : review.verdict === 'fail' ? '未通过' : '待判定'
                  }}</span>
                </div>
                <pre class="review-text">{{ review.text || '（无审查原文）' }}</pre>
              </article>
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
  overflow: auto;
  background: var(--ad-editor);
}

.empty,
.workbench {
  max-width: 1200px;
  margin: 0 auto;
  padding: 20px 24px 64px;
}

.empty {
  max-width: 560px;
  padding-top: 72px;
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
  margin-bottom: 16px;
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
  margin: 0 0 16px;
  padding: 10px 12px;
  border-radius: 10px;
  background: rgba(201, 162, 39, 0.12);
  color: #e6d08a;
  font-size: 13px;
}

.board {
  display: grid;
  grid-template-columns: minmax(0, 1.15fr) minmax(300px, 0.85fr);
  gap: 20px;
  align-items: start;
}

.board-chart {
  min-width: 0;
  border: 1px solid var(--ad-border);
  border-radius: 12px;
  background: var(--ad-harbor);
}

.palette {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
  padding: 10px 12px 12px;
  border-top: 1px solid var(--ad-border);
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
}

.chip:hover:not(:disabled) {
  color: var(--ad-text);
  background: var(--ad-hover);
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

.board-side {
  min-width: 0;
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
  .board {
    grid-template-columns: 1fr;
  }

  .stage {
    flex-wrap: wrap;
  }
}
</style>
