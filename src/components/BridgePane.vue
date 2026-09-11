<script setup lang="ts">
import { computed, watch } from 'vue'
import {
  advanceSlice,
  markManualVerdict,
  persistCurrent,
  probeNote,
  refreshGitSnapshot,
  runDeveloperLoop,
  submitManualReview
} from '../lib/pipeline'
import { currentPipeline, projectHasLivePty, selectedProject, store } from '../lib/store'
import { GATE_LABEL, type ReviewTargetKind } from '../lib/types'

const busy = computed(() => {
  const gate = currentPipeline.value?.gate
  return gate === 'reviewing' || gate === 'developing'
})

const nextLocked = computed(() => currentPipeline.value?.gate !== 'passed')

const targets: { id: ReviewTargetKind; label: string; hint: string }[] = [
  { id: 'uncommittedChanges', label: '未提交改动', hint: '审工作区 diff' },
  { id: 'commit', label: '指定 commit', hint: '按 sha 审' },
  { id: 'baseBranch', label: '相对分支', hint: '对比基线分支' },
  { id: 'custom', label: '自定义说明', hint: '自由审查指令' }
]

function setTarget(kind: ReviewTargetKind) {
  const pipe = currentPipeline.value
  if (!pipe) return
  pipe.targetKind = kind
  persistCurrent()
}

watch(
  currentPipeline,
  () => {
    persistCurrent()
  },
  { deep: true }
)

const probe = computed(() => store.codexProbe)

const stageHint = computed(() => {
  const gate = currentPipeline.value?.gate
  if (gate === 'reviewing') return 'Codex 正在审查本轮改动'
  if (gate === 'developing') return 'Cursor 正在自动开发'
  if (gate === 'passed') return '本片已通过，可进入下一片'
  if (gate === 'failed') return '未通过，可返工或手动标记'
  return '半自动闸：写完再审；自动开发另计额度'
})
</script>

<template>
  <section class="pane" aria-label="编排画布">
    <div v-if="!selectedProject" class="empty">
      <h1>先选一个项目</h1>
      <p>编排和本机 Codex 线程库按项目目录对齐。控制台里已开的终端不会被关掉。</p>
    </div>
    <div v-else-if="currentPipeline" class="workbench">
      <header class="stage">
        <div class="stage-main">
          <h1>{{ selectedProject.name }} · 第 {{ currentPipeline.slice }} 片</h1>
          <p class="stage-hint">{{ stageHint }}</p>
        </div>
        <p class="badge" :data-gate="currentPipeline.gate">{{ GATE_LABEL[currentPipeline.gate] }}</p>
      </header>

      <p v-if="projectHasLivePty" class="banner">
        控制台里这个项目还有打开的终端。审查或 Cursor 改文件时，不要同时在终端里写同一棵树。
      </p>

      <div class="cols">
        <div class="col col--primary">
          <label class="field">
            <span>本轮任务</span>
            <textarea
              v-model="currentPipeline.task"
              rows="5"
              maxlength="4000"
              placeholder="这一片要 Cursor 做或你自己刚做完的事。半自动闸只审，不替你改代码。"
            />
          </label>

          <div class="actions actions--primary">
            <button type="button" class="btn btn-primary" :disabled="busy" @click="submitManualReview">
              {{ currentPipeline.gate === 'reviewing' ? '审查中…' : '提交本轮审查' }}
            </button>
            <button type="button" class="btn btn-ghost" :disabled="busy" @click="runDeveloperLoop">
              {{ currentPipeline.gate === 'developing' ? '开发中…' : '开始自动开发' }}
            </button>
          </div>

          <div class="actions actions--soft">
            <button type="button" class="soft-btn" :disabled="busy" @click="markManualVerdict('pass')">
              标记通过
            </button>
            <button type="button" class="soft-btn" :disabled="busy" @click="markManualVerdict('fail')">
              标记未通过
            </button>
            <button type="button" class="soft-btn" :disabled="nextLocked || busy" @click="advanceSlice">
              进入下一片
            </button>
          </div>

          <p class="help">
            半自动：你在 Cursor IDE 里写完再点审查。自动开发会调 Cursor SDK，额度与 IDE 分开。未通过时「进入下一片」保持锁定。
          </p>
          <p v-if="currentPipeline.lastError" class="error">{{ currentPipeline.lastError }}</p>
          <p v-if="currentPipeline.retryCount" class="meta">
            已返工 {{ currentPipeline.retryCount }} / {{ currentPipeline.maxRetries }} 次
          </p>

          <section v-if="currentPipeline.reviews.length" class="timeline" aria-label="审查时间线">
            <h2 class="section-title">审查记录</h2>
            <article
              v-for="(review, index) in currentPipeline.reviews"
              :key="review.reviewThreadId || index"
              class="review-card"
              :data-verdict="review.verdict"
            >
              <div class="review-head">
                <h3>Codex 审查 {{ index + 1 }}</h3>
                <span class="verdict" :data-verdict="review.verdict">{{
                  review.verdict === 'pass' ? '通过' : review.verdict === 'fail' ? '未通过' : '待判定'
                }}</span>
              </div>
              <p class="meta">
                源线程 {{ review.sourceThreadId || '新开' }} · 审查线程 {{ review.reviewThreadId || '—' }}
              </p>
              <pre class="review-text">{{ review.text || '（无审查原文）' }}</pre>
            </article>
          </section>
        </div>

        <aside class="col col--secondary">
          <div class="probe-row">
            <span class="probe-dot" :class="{ 'is-ok': probe?.ok }" />
            <div class="probe-body">
              <strong>本机 App Server</strong>
              <p>{{ probeNote(probe) }}</p>
              <p v-if="probe" class="meta">
                {{ probe.ok ? '已握手' : '未接通' }}
                · 列出 {{ probe.listed }} 条
                · 本目录 {{ probe.cwdMatched }} 条
                · {{ probe.writerSafe ? '未抢 writer' : '存在 writer 风险' }}
              </p>
            </div>
          </div>

          <article class="side-card">
            <h2>审查目标</h2>
            <div class="targets">
              <button
                v-for="item in targets"
                :key="item.id"
                type="button"
                class="target"
                :class="{ 'is-on': currentPipeline.targetKind === item.id }"
                @click="setTarget(item.id)"
              >
                <strong>{{ item.label }}</strong>
                <span>{{ item.hint }}</span>
              </button>
            </div>
            <label v-if="currentPipeline.targetKind === 'commit'" class="field tight">
              <span>Commit SHA</span>
              <input v-model="currentPipeline.commitSha" placeholder="完整或短 sha" />
            </label>
            <label v-if="currentPipeline.targetKind === 'baseBranch'" class="field tight">
              <span>基线分支</span>
              <input v-model="currentPipeline.baseBranch" placeholder="main" />
            </label>
            <label v-if="currentPipeline.targetKind === 'custom'" class="field tight">
              <span>审查说明</span>
              <textarea
                v-model="currentPipeline.customInstructions"
                rows="3"
                placeholder="请审什么，结尾请写 VERDICT: PASS 或 FAIL"
              />
            </label>
          </article>

          <article class="side-card">
            <div class="card-head">
              <h2>工作区</h2>
              <button type="button" class="text-btn" @click="refreshGitSnapshot">刷新 diff</button>
            </div>
            <p v-if="currentPipeline.git" class="git-line">
              {{ currentPipeline.git.branch }}
              · {{ currentPipeline.git.head.slice(0, 8) || '—' }}
              · {{ currentPipeline.git.dirty ? '有未提交改动' : '工作区干净' }}
            </p>
            <pre v-if="currentPipeline.git?.summary" class="diff">{{ currentPipeline.git.summary }}</pre>
            <p v-else class="meta">还没有 git 摘要。</p>
          </article>
        </aside>
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
  max-width: 1120px;
  margin: 0 auto;
  padding: 28px 24px 64px;
}

.empty {
  max-width: 560px;
  padding-top: 72px;
}

h1 {
  margin: 0;
  font-size: 20px;
  line-height: 28px;
  font-weight: 560;
}

h2,
.section-title {
  margin: 0;
  font-size: 13px;
  line-height: 20px;
  font-weight: 600;
}

h3 {
  margin: 0;
  font-size: 13px;
  line-height: 20px;
  font-weight: 560;
}

.stage {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 16px;
}

.stage-hint,
.help,
.meta {
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
  color: var(--ad-text);
  flex-shrink: 0;
  font-size: 12px;
}

.badge[data-gate='passed'] {
  color: var(--ad-success);
}

.badge[data-gate='failed'] {
  color: var(--ad-error);
}

.badge[data-gate='reviewing'],
.badge[data-gate='developing'] {
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

.cols {
  display: grid;
  grid-template-columns: minmax(0, 1.45fr) minmax(280px, 0.9fr);
  gap: 20px;
  align-items: start;
}

.col--primary,
.col--secondary {
  min-width: 0;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 14px;
}

.field.tight {
  margin: 12px 0 0;
}

.field > span {
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

textarea {
  min-height: 108px;
  height: auto;
  padding: 10px 12px;
  resize: vertical;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.actions--primary {
  margin-bottom: 8px;
}

.actions--soft {
  margin-bottom: 10px;
}

.soft-btn {
  height: 28px;
  padding: 0 10px;
  border-radius: 8px;
  font-size: 12px;
  color: var(--ad-muted);
  border: 1px solid transparent;
}

.soft-btn:hover:not(:disabled) {
  color: var(--ad-text);
  background: var(--ad-hover);
  border-color: var(--ad-border);
}

.help {
  margin-bottom: 8px;
}

.error {
  margin: 0 0 8px;
  color: var(--ad-error);
  font-size: 13px;
}

.timeline {
  margin-top: 20px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.section-title {
  margin-bottom: 2px;
  color: var(--ad-faint);
  font-weight: 500;
}

.review-card {
  padding: 12px 14px 12px 12px;
  border: 1px solid var(--ad-border);
  border-left-width: 3px;
  border-radius: 10px;
  background: var(--ad-harbor);
  border-left-color: var(--ad-muted);
}

.review-card[data-verdict='pass'] {
  border-left-color: var(--ad-success);
}

.review-card[data-verdict='fail'] {
  border-left-color: var(--ad-error);
}

.review-card[data-verdict='unknown'] {
  border-left-color: var(--ad-warning);
}

.review-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 4px;
}

.verdict[data-verdict='pass'] {
  color: var(--ad-success);
}

.verdict[data-verdict='fail'] {
  color: var(--ad-error);
}

.verdict[data-verdict='unknown'] {
  color: var(--ad-warning);
}

.review-text,
.diff {
  margin: 8px 0 0;
  max-height: 280px;
  overflow: auto;
  white-space: pre-wrap;
  font-family: var(--ad-mono);
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-text);
}

.probe-row {
  display: flex;
  gap: 10px;
  align-items: flex-start;
  margin-bottom: 12px;
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid var(--ad-border);
  background: rgba(255, 255, 255, 0.02);
}

.probe-dot {
  width: 8px;
  height: 8px;
  margin-top: 6px;
  border-radius: 50%;
  background: var(--ad-error);
  flex-shrink: 0;
}

.probe-dot.is-ok {
  background: var(--ad-success);
}

.probe-body {
  min-width: 0;
  flex: 1;
}

.probe-body strong {
  font-size: 13px;
  font-weight: 560;
}

.probe-body p {
  margin: 2px 0 0;
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

.side-card {
  margin-bottom: 12px;
  padding: 12px 14px;
  border: 1px solid var(--ad-border);
  border-radius: 12px;
  background: var(--ad-harbor);
}

.card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
}

.side-card > h2 {
  margin-bottom: 10px;
}

.text-btn {
  height: 28px;
  padding: 0 6px;
  color: var(--ad-muted);
  font-size: 12px;
}

.text-btn:hover {
  color: var(--ad-text);
}

.targets {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

.target {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid var(--ad-border);
  text-align: left;
  color: var(--ad-muted);
}

.target.is-on {
  background: var(--ad-selected);
  color: var(--ad-text);
}

.target strong {
  font-weight: 560;
  color: inherit;
}

.git-line {
  margin: 0;
  font-size: 13px;
  line-height: 20px;
}

@media (max-width: 900px) {
  .cols {
    grid-template-columns: 1fr;
  }

  .targets {
    grid-template-columns: 1fr 1fr;
  }
}

@media (max-width: 520px) {
  .targets {
    grid-template-columns: 1fr;
  }
}
</style>
