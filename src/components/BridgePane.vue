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
</script>

<template>
  <section class="pane" aria-label="编排画布">
    <div v-if="!selectedProject" class="empty">
      <h1>先选一个项目</h1>
      <p>编排和本机 Codex 线程库按项目目录对齐。控制台里已开的终端不会被关掉。</p>
    </div>
    <div v-else-if="currentPipeline" class="canvas">
      <header class="hero">
        <div>
          <p class="kicker">桥接编排</p>
          <h1>{{ selectedProject.name }} · 第 {{ currentPipeline.slice }} 片</h1>
        </div>
        <p class="badge" :data-gate="currentPipeline.gate">{{ GATE_LABEL[currentPipeline.gate] }}</p>
      </header>

      <p v-if="projectHasLivePty" class="banner">
        控制台里这个项目还有打开的终端。审查或 Cursor 改文件时，不要同时在终端里写同一棵树。
      </p>

      <article class="card">
        <h2>本机 App Server</h2>
        <p>{{ probeNote(probe) }}</p>
        <p v-if="probe" class="meta">
          {{ probe.ok ? '已握手' : '未接通' }}
          · 列出 {{ probe.listed }} 条
          · 本目录 {{ probe.cwdMatched }} 条
          · {{ probe.writerSafe ? '未抢 writer' : '存在 writer 风险' }}
        </p>
      </article>

      <label class="field">
        <span>本轮任务</span>
        <textarea
          v-model="currentPipeline.task"
          rows="4"
          maxlength="4000"
          placeholder="这一片要 Cursor 做或你自己刚做完的事。半自动闸只审，不替你改代码。"
        />
      </label>

      <div class="grid">
        <article class="card">
          <div class="card-head">
            <h2>工作区</h2>
            <button type="button" class="text-btn" @click="refreshGitSnapshot">刷新 diff</button>
          </div>
          <p v-if="currentPipeline.git">
            {{ currentPipeline.git.branch }}
            · {{ currentPipeline.git.head.slice(0, 8) || '—' }}
            · {{ currentPipeline.git.dirty ? '有未提交改动' : '工作区干净' }}
          </p>
          <pre v-if="currentPipeline.git?.summary" class="diff">{{ currentPipeline.git.summary }}</pre>
          <p v-else class="muted">还没有 git 摘要。</p>
        </article>

        <article class="card">
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
            <textarea v-model="currentPipeline.customInstructions" rows="3" placeholder="请审什么，结尾请写 VERDICT: PASS 或 FAIL" />
          </label>
        </article>
      </div>

      <div class="actions">
        <button type="button" class="btn btn-primary" :disabled="busy" @click="submitManualReview">
          {{ currentPipeline.gate === 'reviewing' ? '审查中…' : '提交本轮审查' }}
        </button>
        <button type="button" class="btn btn-ghost" :disabled="busy" @click="runDeveloperLoop">
          {{ currentPipeline.gate === 'developing' ? '开发中…' : '开始自动开发' }}
        </button>
        <button type="button" class="btn btn-ghost" :disabled="busy" @click="markManualVerdict('pass')">
          标记通过
        </button>
        <button type="button" class="btn btn-ghost" :disabled="busy" @click="markManualVerdict('fail')">
          标记未通过
        </button>
        <button type="button" class="btn btn-ghost" :disabled="nextLocked || busy" @click="advanceSlice">
          进入下一片
        </button>
      </div>
      <p class="muted">
        半自动：你在 Cursor IDE 里写完再点审查。自动开发会调 Cursor SDK，额度与 IDE 分开。未通过时「进入下一片」保持锁定。
      </p>
      <p v-if="currentPipeline.lastError" class="error">{{ currentPipeline.lastError }}</p>
      <p v-if="currentPipeline.retryCount" class="muted">已返工 {{ currentPipeline.retryCount }} / {{ currentPipeline.maxRetries }} 次</p>

      <article v-for="(review, index) in currentPipeline.reviews" :key="review.reviewThreadId || index" class="card review">
        <div class="card-head">
          <h2>Codex 审查 {{ index + 1 }}</h2>
          <span class="verdict" :data-verdict="review.verdict">{{
            review.verdict === 'pass' ? '通过' : review.verdict === 'fail' ? '未通过' : '待判定'
          }}</span>
        </div>
        <p class="meta">源线程 {{ review.sourceThreadId || '新开' }} · 审查线程 {{ review.reviewThreadId || '—' }}</p>
        <pre class="review-text">{{ review.text || '（无审查原文）' }}</pre>
      </article>
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
.canvas {
  max-width: 880px;
  margin: 0 auto;
  padding: 36px 28px 64px;
}

h1 {
  margin: 6px 0 0;
  font-size: 22px;
  line-height: 30px;
  font-weight: 560;
}

h2 {
  margin: 0;
  font-size: 13px;
  line-height: 20px;
  font-weight: 600;
}

.kicker,
.muted,
.meta {
  margin: 0;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.hero {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 20px;
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
}

.badge[data-gate='passed'] {
  color: var(--ad-success);
}

.badge[data-gate='failed'] {
  color: var(--ad-error);
}

.banner {
  margin: 0 0 16px;
  padding: 10px 12px;
  border-radius: 10px;
  background: rgba(201, 162, 39, 0.12);
  color: #e6d08a;
  font-size: 13px;
}

.card {
  margin-bottom: 16px;
  padding: 14px 16px;
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

.grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 16px;
}

.field.tight {
  margin: 12px 0 0;
}

textarea {
  min-height: 92px;
  height: auto;
  padding: 10px 12px;
  resize: vertical;
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

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin: 4px 0 12px;
}

.text-btn {
  height: 28px;
  padding: 0 6px;
  color: var(--ad-muted);
}

.text-btn:hover {
  color: var(--ad-text);
}

.diff,
.review-text {
  margin: 8px 0 0;
  max-height: 280px;
  overflow: auto;
  white-space: pre-wrap;
  font-family: var(--ad-mono);
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-text);
}

.error {
  color: var(--ad-error);
  font-size: 13px;
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

@media (max-width: 900px) {
  .grid,
  .targets {
    grid-template-columns: 1fr;
  }
}
</style>
