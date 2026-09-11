<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import * as api from '../lib/api'
import {
  GROK_USAGE_INTERVAL_MS,
  formatUsageLine,
  formatUsageTooltip,
  shouldShowGrokUsage,
  usageTone
} from '../lib/grokUsage'
import SpendPanel from './SpendPanel.vue'
import { SPEND_PANEL_WIDTH, formatSpendLine, resolveSpendRange, type SpendPreset } from '../lib/grokSpend'
import { groupLiveByProject, toolTint } from '../lib/livePty'
import { currentPipeline, jumpToLive, selectedProject, store } from '../lib/store'
import { GATE_LABEL, toolLabel } from '../lib/types'
import type { GrokSpend, GrokUsage } from '../lib/types'

defineProps<{
  docsOpen?: boolean
}>()

defineEmits<{
  settings: []
  refresh: []
  docs: []
}>()

const usage = ref<GrokUsage | null>(null)
const usageLoading = ref(false)
const spend = ref<GrokSpend | null>(null)
const spendLoading = ref(false)
let usageTimer = 0
let usageSeq = 0
let spendSeq = 0

const showGrokUsage = computed(() =>
  shouldShowGrokUsage(store.sessionToolFilter, store.selectedTool, store.appMode)
)

const usageText = computed(() => {
  if (usage.value) return formatUsageLine(usage.value)
  return usageLoading.value ? 'Grok 用量…' : 'Grok 用量'
})

const usageTitle = computed(() => {
  if (usage.value) return formatUsageTooltip(usage.value)
  return usageLoading.value ? '正在读取 Grok 用量' : '点击读取 Grok 用量'
})

const usageClass = computed(() => usageTone(usage.value?.ok ? usage.value.remainingPercent : null))
const spendText = computed(() => formatSpendLine(spend.value, spendLoading.value))
const initialRange = resolveSpendRange('today')
const spendPreset = ref<SpendPreset>('today')
const spendStart = ref(initialRange.start)
const spendEnd = ref(initialRange.end)
const spendFollowEnd = ref(true)

const liveOpen = ref(false)
const liveBtn = ref<HTMLElement | null>(null)
const liveMenuStyle = ref<Record<string, string>>({})
const spendOpen = ref(false)
const spendBtn = ref<HTMLElement | null>(null)
const spendMenuStyle = ref<Record<string, string>>({})
const liveCount = computed(() => store.live.filter((item) => item.alive !== false).length)
const liveGroups = computed(() => groupLiveByProject(store.live, store.projects))
const liveLabel = computed(() => {
  const n = liveCount.value
  if (store.appMode === 'bridge') {
    return n ? `后台仍有 ${n} 个终端` : '0 个终端'
  }
  return `${n} 个终端`
})

function placeMenu(el: HTMLElement | null, width: number): Record<string, string> {
  if (!el) return {}
  const rect = el.getBoundingClientRect()
  const left = Math.min(Math.max(8, rect.left), window.innerWidth - width - 8)
  return {
    position: 'fixed',
    left: `${left}px`,
    bottom: `${Math.max(8, window.innerHeight - rect.top + 6)}px`,
    width: `${width}px`,
    zIndex: '50'
  }
}

function placeLiveMenu() {
  liveMenuStyle.value = placeMenu(liveBtn.value, 300)
}

function closeLiveMenu() {
  liveOpen.value = false
}

function closeSpendMenu() {
  spendOpen.value = false
}

function placeSpendMenu() {
  const width = Math.min(SPEND_PANEL_WIDTH, window.innerWidth - 16)
  const left = Math.max(8, Math.round((window.innerWidth - width) / 2))
  spendMenuStyle.value = {
    position: 'fixed',
    left: `${left}px`,
    bottom: '36px',
    width: `${width}px`,
    zIndex: '50'
  }
}

function onResize() {
  closeLiveMenu()
  if (spendOpen.value) placeSpendMenu()
}

function toggleLiveMenu() {
  if (!liveCount.value) return
  closeSpendMenu()
  liveOpen.value = !liveOpen.value
  if (liveOpen.value) placeLiveMenu()
}

function toggleSpendMenu() {
  closeLiveMenu()
  spendOpen.value = !spendOpen.value
  if (spendOpen.value) {
    placeSpendMenu()
    void loadSpend()
  }
}

function onSpendRange(next: { preset: SpendPreset; start: number; end: number; followEnd: boolean }) {
  spendPreset.value = next.preset
  spendStart.value = next.start
  spendEnd.value = next.end
  spendFollowEnd.value = next.followEnd
  void loadSpend()
}

async function onJump(ptyId: string) {
  closeLiveMenu()
  await jumpToLive(ptyId)
}

function onDocClick() {
  closeLiveMenu()
  closeSpendMenu()
}

function onKey(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    closeLiveMenu()
    closeSpendMenu()
  }
}

async function loadUsage() {
  if (!showGrokUsage.value) return
  const seq = ++usageSeq
  const had = Boolean(usage.value)
  if (!had) usageLoading.value = true
  try {
    const next = await api.grokUsage(store.selectedProjectId || null)
    if (seq !== usageSeq) return
    if (next.ok || !usage.value) usage.value = next
  } catch (err) {
    if (seq !== usageSeq) return
    if (!usage.value) {
      usage.value = {
        ok: false,
        usedPercent: null,
        remainingPercent: null,
        resetsAt: null,
        periodLabel: null,
        prepaidBalance: null,
        onDemandUsed: null,
        onDemandCap: null,
        grokBuildUsedPercent: null,
        usedCredits: null,
        creditLimit: null,
        fetchedAt: new Date().toISOString(),
        message: err instanceof Error ? err.message : '读取 Grok 用量失败，点击重试'
      }
    }
  } finally {
    if (seq === usageSeq) usageLoading.value = false
  }
}

async function loadSpend(force = false) {
  if (!showGrokUsage.value) return
  const seq = ++spendSeq
  if (spendFollowEnd.value) spendEnd.value = Math.floor(Date.now() / 1000)
  if (!spend.value || force) spendLoading.value = true
  try {
    const next = await api.grokSpend(spendStart.value, spendEnd.value)
    if (seq !== spendSeq) return
    if (next.ok || !spend.value) spend.value = next
  } catch (err) {
    if (seq !== spendSeq) return
    if (!spend.value) {
      spend.value = {
        ok: false,
        totalTokens: 0,
        inputTokens: 0,
        outputTokens: 0,
        cacheReadTokens: 0,
        cacheCreationTokens: 0,
        cacheHitPercent: 0,
        turnCount: 0,
        costUsd: 0,
        granularity: 'hour',
        rangeStart: spendStart.value,
        rangeEnd: spendEnd.value,
        points: [],
        scannedFiles: 0,
        fetchedAt: new Date().toISOString(),
        message: err instanceof Error ? err.message : '读取 token 消耗失败'
      }
    }
  } finally {
    if (seq === spendSeq) spendLoading.value = false
  }
}

function stopUsageTimer() {
  if (usageTimer) {
    window.clearInterval(usageTimer)
    usageTimer = 0
  }
}

watch(
  [showGrokUsage, () => store.selectedProjectId, () => store.grokAuthRev],
  ([show]) => {
    stopUsageTimer()
    usageSeq += 1
    if (!show) {
      usage.value = null
      usageLoading.value = false
      spend.value = null
      spendLoading.value = false
      spendSeq += 1
      closeSpendMenu()
      return
    }
    void loadUsage()
    void loadSpend()
    usageTimer = window.setInterval(() => {
      void loadUsage()
      void loadSpend()
    }, GROK_USAGE_INTERVAL_MS)
  },
  { immediate: true }
)

onMounted(() => {
  window.addEventListener('click', onDocClick)
  window.addEventListener('keydown', onKey)
  window.addEventListener('resize', onResize)
})

onUnmounted(() => {
  usageSeq += 1
  spendSeq += 1
  stopUsageTimer()
  window.removeEventListener('click', onDocClick)
  window.removeEventListener('keydown', onKey)
  window.removeEventListener('resize', onResize)
})

watch(liveCount, (n) => {
  if (!n) closeLiveMenu()
})
</script>

<template>
  <footer class="bar">
    <span>{{ store.projects.length }} 个项目</span>
    <span v-if="selectedProject" class="path" :title="selectedProject.path">{{ selectedProject.path }}</span>
    <span>{{ selectedProject?.proxyEnabled ? '代理开' : '代理关' }}</span>
    <span v-if="store.appMode === 'bridge'">闸门 {{ GATE_LABEL[currentPipeline?.gate ?? 'idle'] }}</span>
    <div class="live-wrap">
      <button
        v-if="liveCount"
        ref="liveBtn"
        type="button"
        class="link live-btn"
        :class="{ 'is-open': liveOpen }"
        :aria-expanded="liveOpen"
        aria-haspopup="menu"
        :title="liveOpen ? '收起终端列表' : '查看已打开的终端'"
        @click.stop="toggleLiveMenu"
      >
        {{ liveLabel }}
        <span class="live-caret" aria-hidden="true">{{ liveOpen ? '▴' : '▾' }}</span>
      </button>
      <span v-else>{{ liveLabel }}</span>
      <Teleport to="body">
        <div
          v-if="liveOpen"
          class="ad-menu live-menu"
          role="menu"
          :style="liveMenuStyle"
          @click.stop
        >
          <p class="live-kicker">已打开的终端</p>
          <template v-for="group in liveGroups" :key="group.projectId">
            <p class="live-project">{{ group.projectName }}</p>
            <button
              v-for="item in group.items"
              :key="item.ptyId"
              type="button"
              role="menuitem"
              class="ad-menu-item live-item"
              :class="{ 'is-active': item.ptyId === store.activePtyId }"
              @click="onJump(item.ptyId)"
            >
              <span class="live-main">
                <span class="live-dot" :style="{ background: toolTint(item.toolId) }" aria-hidden="true" />
                <span class="live-title">{{ item.title }}</span>
              </span>
              <span class="ad-menu-hint">{{ toolLabel(item.toolId) }}</span>
            </button>
          </template>
        </div>
      </Teleport>
    </div>
    <button
      v-if="showGrokUsage"
      type="button"
      class="usage"
      :class="usageClass"
      :title="usageTitle"
      :disabled="usageLoading && !usage"
      @click="loadUsage"
    >
      {{ usageText }}
    </button>
    <div v-if="showGrokUsage" class="live-wrap">
      <button
        ref="spendBtn"
        type="button"
        class="link live-btn"
        :class="{ 'is-open': spendOpen }"
        :aria-expanded="spendOpen"
        aria-haspopup="dialog"
        title="查看本机 Grok token 消耗"
        @click.stop="toggleSpendMenu"
      >
        {{ spendText }}
        <span class="live-caret" aria-hidden="true">{{ spendOpen ? '▴' : '▾' }}</span>
      </button>
      <Teleport to="body">
        <SpendPanel
          v-if="spendOpen"
          class="ad-menu spend-panel"
          :style="spendMenuStyle"
          :spend="spend"
          :loading="spendLoading"
          :preset="spendPreset"
          :start="spendStart"
          :end="spendEnd"
          :follow-end="spendFollowEnd"
          @click.stop
          @apply-range="onSpendRange"
          @refresh="loadSpend(true)"
        />
      </Teleport>
    </div>
    <span class="spacer" />
    <button
      v-if="store.appMode === 'console'"
      type="button"
      class="link"
      :class="{ on: docsOpen }"
      :aria-pressed="docsOpen"
      @click="$emit('docs')"
    >
      文档
    </button>
    <button type="button" class="link" @click="$emit('refresh')">
      {{ store.appMode === 'bridge' ? '刷新线程' : '刷新会话' }}
    </button>
    <button type="button" class="link" @click="$emit('settings')">设置</button>
  </footer>
</template>

<style scoped>
.bar {
  height: 28px;
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 0 16px;
  background: var(--ad-sidebar);
  border-top: 1px solid var(--ad-border);
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 42vw;
}

.spacer {
  flex: 1;
}

.usage,
.link {
  height: 22px;
  padding: 0 8px;
  border-radius: var(--ad-radius-control);
  color: var(--ad-muted);
}

.usage {
  max-width: 42vw;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.usage.warn {
  color: var(--ad-warning);
}

.usage.err {
  color: var(--ad-error);
}

.usage:hover:not(:disabled),
.link:hover,
.link.on {
  color: var(--ad-text);
  background: var(--ad-hover);
}

.usage:disabled {
  cursor: default;
  opacity: 0.72;
}

.live-wrap {
  position: relative;
}

.live-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.live-btn.is-open {
  color: var(--ad-text);
  background: var(--ad-hover);
}

.live-caret {
  font-size: 10px;
  line-height: 1;
  opacity: 0.7;
}

.live-menu {
  max-height: min(360px, calc(100vh - 48px));
  overflow: auto;
}

.live-kicker,
.live-project {
  margin: 0;
  padding: 6px 10px 4px;
  font-size: 11px;
  line-height: 16px;
  color: var(--ad-faint);
}

.live-project {
  padding-top: 8px;
}

.live-item {
  gap: 12px;
}

.live-main {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
}

.live-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

.live-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.spend-panel {
  max-height: min(92vh, 760px);
  overflow: auto;
}
</style>
