<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import * as api from '../lib/api'
import {
  formatUsageLine,
  formatUsageTooltip,
  grokUsagePollMs,
  shouldApplyUsageFetch,
  shouldShowGrokUsage,
  usageTone
} from '../lib/grokUsage'
import {
  DSH_BALANCE_INTERVAL_MS,
  DSH_PEAK_TICK_MS,
  balanceTone,
  formatBalanceLine,
  formatBalanceTooltip,
  formatPeakTooltip,
  peakCopy,
  shouldShowDshBalance
} from '../lib/dshBalance'
import SpendPanel from './SpendPanel.vue'
import { SPEND_PANEL_WIDTH, formatSpendLine, resolveSpendRange, sameSpendSnapshot, type SpendPreset } from '../lib/grokSpend'
import { groupLiveByProject, toolTint } from '../lib/livePty'
import { currentPipeline, jumpToLive, selectedProject, store } from '../lib/store'
import { GATE_LABEL, toolLabel } from '../lib/types'
import type { DshBalance, GrokSpend, GrokUsage } from '../lib/types'

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
const dshBalance = ref<DshBalance | null>(null)
const dshLoading = ref(false)
const peakNow = ref(Date.now())
const usageNow = ref(Date.now())
let usageTimer = 0
let dshTimer = 0
let peakTimer = 0
let usageSeq = 0
let spendSeq = 0
let dshSeq = 0

const showGrokUsage = computed(() =>
  shouldShowGrokUsage(store.sessionToolFilter, store.selectedTool, store.appMode)
)
const showDshBalance = computed(() =>
  shouldShowDshBalance(store.sessionToolFilter, store.selectedTool, store.appMode)
)

const usageText = computed(() => {
  if (usage.value) return formatUsageLine(usage.value, usageNow.value)
  return usageLoading.value ? 'Grok 用量…' : 'Grok 用量'
})

const usageTitle = computed(() => {
  if (usage.value) return formatUsageTooltip(usage.value, usageNow.value)
  return usageLoading.value ? '正在读取 Grok 用量' : '点击读取 Grok 用量'
})

const usageClass = computed(() => usageTone(usage.value?.ok ? usage.value.remainingPercent : null))
const dshText = computed(() => {
  if (dshBalance.value) return formatBalanceLine(dshBalance.value)
  return dshLoading.value ? 'DeepSeek 余额…' : 'DeepSeek 余额'
})
const dshTitle = computed(() => {
  if (dshBalance.value) return formatBalanceTooltip(dshBalance.value)
  return dshLoading.value ? '正在读取 DeepSeek 余额' : '点击读取 DeepSeek 余额'
})
const dshClass = computed(() => balanceTone(dshBalance.value))
const peak = computed(() => peakCopy(new Date(peakNow.value)))
const peakTitle = computed(() => formatPeakTooltip(new Date(peakNow.value)))
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
  await jumpToLive(ptyId, { keepMode: store.appMode === 'bridge' })
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

async function loadDshBalance() {
  if (!showDshBalance.value) return
  const seq = ++dshSeq
  const had = Boolean(dshBalance.value)
  if (!had) dshLoading.value = true
  try {
    const next = await api.dshBalance(store.selectedProjectId || null)
    if (seq !== dshSeq) return
    if (next.ok || !dshBalance.value) dshBalance.value = next
  } catch (err) {
    if (seq !== dshSeq) return
    if (!dshBalance.value) {
      dshBalance.value = {
        ok: false,
        available: null,
        currency: null,
        totalBalance: null,
        grantedBalance: null,
        toppedUpBalance: null,
        fetchedAt: new Date().toISOString(),
        message: err instanceof Error ? err.message : '读取 DeepSeek 余额失败，点击重试'
      }
    }
  } finally {
    if (seq === dshSeq) dshLoading.value = false
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
    usageNow.value = Date.now()
    if (shouldApplyUsageFetch(usage.value, next, usageNow.value)) {
      const prevMs = grokUsagePollMs(usage.value, usageNow.value)
      usage.value = next
      if (showGrokUsage.value && grokUsagePollMs(usage.value, usageNow.value) !== prevMs) {
        startUsageTimer()
      }
    }
  } catch (err) {
    if (seq !== usageSeq) return
    const failed: GrokUsage = {
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
    usageNow.value = Date.now()
    if (shouldApplyUsageFetch(usage.value, failed, usageNow.value)) usage.value = failed
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
    if (next.ok && !force && sameSpendSnapshot(spend.value, next)) return
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

function startUsageTimer() {
  stopUsageTimer()
  usageNow.value = Date.now()
  usageTimer = window.setInterval(() => {
    usageNow.value = Date.now()
    void loadUsage()
    void loadSpend()
  }, grokUsagePollMs(usage.value, usageNow.value))
}

function onWindowFocus() {
  if (!showGrokUsage.value) return
  usageNow.value = Date.now()
  void loadUsage()
  void loadSpend()
}

function stopDshTimer() {
  if (dshTimer) {
    window.clearInterval(dshTimer)
    dshTimer = 0
  }
}

function stopPeakTimer() {
  if (peakTimer) {
    window.clearInterval(peakTimer)
    peakTimer = 0
  }
}

function startPeakTimer() {
  stopPeakTimer()
  peakNow.value = Date.now()
  peakTimer = window.setInterval(() => {
    peakNow.value = Date.now()
  }, DSH_PEAK_TICK_MS)
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
    startUsageTimer()
  },
  { immediate: true }
)

watch(
  [showDshBalance, () => store.selectedProjectId, () => store.dshKeyRev],
  ([show]) => {
    stopDshTimer()
    stopPeakTimer()
    dshSeq += 1
    dshBalance.value = null
    if (!show) {
      dshLoading.value = false
      return
    }
    startPeakTimer()
    void loadDshBalance()
    dshTimer = window.setInterval(() => {
      void loadDshBalance()
    }, DSH_BALANCE_INTERVAL_MS)
  },
  { immediate: true }
)

onMounted(() => {
  window.addEventListener('click', onDocClick)
  window.addEventListener('keydown', onKey)
  window.addEventListener('resize', onResize)
  window.addEventListener('focus', onWindowFocus)
})

onUnmounted(() => {
  usageSeq += 1
  spendSeq += 1
  dshSeq += 1
  stopUsageTimer()
  stopDshTimer()
  stopPeakTimer()
  window.removeEventListener('click', onDocClick)
  window.removeEventListener('keydown', onKey)
  window.removeEventListener('resize', onResize)
  window.removeEventListener('focus', onWindowFocus)
})

watch(liveCount, (n) => {
  if (!n) closeLiveMenu()
})
</script>

<template>
  <footer class="bar">
    <span class="project-total">{{ store.projects.length }} 个项目</span>
    <span v-if="selectedProject" class="path" :title="selectedProject.path">{{ selectedProject.path }}</span>
    <span class="proxy-state">{{ selectedProject?.proxyEnabled ? '代理开' : '代理关' }}</span>
    <span v-if="store.appMode === 'bridge'" class="gate-state">闸门 {{ GATE_LABEL[currentPipeline?.gate ?? 'idle'] }}</span>
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
    <div v-if="showDshBalance" class="dsh-meter">
      <button
        type="button"
        class="usage"
        :class="dshClass"
        :title="dshTitle"
        :disabled="dshLoading && !dshBalance"
        @click="loadDshBalance"
      >
        {{ dshText }}
      </button>
      <span
        class="peak"
        :class="peak.peak ? 'hot' : 'go'"
        :title="peakTitle"
      >
        {{ peak.label }}
        <strong>{{ peak.punch }}</strong>
      </span>
    </div>
    <div v-if="showGrokUsage" class="live-wrap spend-wrap">
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
      class="link docs-btn"
      :class="{ on: docsOpen }"
      :aria-pressed="docsOpen"
      :title="docsOpen ? '收起文档栏' : '打开会话文档'"
      :aria-label="docsOpen ? '收起文档栏' : '打开会话文档'"
      @click="$emit('docs')"
    >
      <svg class="docs-icon" viewBox="0 0 16 16" aria-hidden="true">
        <path d="M4.5 2.5h4.2L12.5 6.3V13.5H4.5z" />
        <path d="M8.6 2.5V6.3H12.5" />
        <path d="M6.2 9h4.6M6.2 11.2h3.2" />
      </svg>
      文档
    </button>
    <button
      v-if="store.appMode !== 'usage'"
      type="button"
      class="link"
      :disabled="store.appMode === 'console' ? store.sessionRefreshBusy : store.codexStatus === 'loading'"
      @click="$emit('refresh')"
    >
      {{ store.appMode === 'bridge' ? '刷新线程' : '刷新会话' }}
    </button>
    <button type="button" class="link" @click="$emit('settings')">设置</button>
  </footer>
</template>

<style scoped>
.bar {
  position: relative;
  z-index: 2;
  flex-shrink: 0;
  height: 28px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 16px;
  overflow: hidden;
  isolation: isolate;
  background: var(--ad-sidebar);
  border-top: 1px solid var(--ad-border);
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
  white-space: nowrap;
}

.bar > * { flex-shrink: 0; }

.path {
  min-width: 0;
  flex: 0 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 28vw;
}

.spacer {
  flex: 1;
  min-width: 0;
}

.usage,
.link {
  height: 22px;
  padding: 0 8px;
  border-radius: var(--ad-radius-control);
  color: var(--ad-muted);
  white-space: nowrap;
}

.usage {
  max-width: 24vw;
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
.link:hover:not(:disabled),
.link.on {
  color: var(--ad-text);
  background: var(--ad-hover);
}

.docs-btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 22px;
  padding: 0 9px 0 7px;
  font-weight: 600;
  letter-spacing: 0.02em;
  color: var(--ad-text);
  background: color-mix(in srgb, var(--ad-text) 14%, transparent);
  border: 1px solid var(--ad-border-strong);
}

.docs-icon {
  width: 12px;
  height: 12px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.35;
  stroke-linejoin: round;
  stroke-linecap: round;
}

.link.docs-btn:hover,
.link.docs-btn.on {
  color: var(--ad-primary-text);
  background: var(--ad-primary);
  border-color: var(--ad-primary);
}

.usage:disabled {
  cursor: default;
  opacity: 0.72;
}

.dsh-meter {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.peak {
  height: 22px;
  padding: 0 8px;
  border-radius: var(--ad-radius-control);
  display: inline-flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
  white-space: nowrap;
  font-weight: 500;
}

.peak strong {
  font-weight: 800;
  letter-spacing: 0.02em;
}

.peak.hot {
  color: var(--ad-error);
  background: color-mix(in srgb, var(--ad-error) 14%, transparent);
}

.peak.go {
  color: var(--ad-success);
  background: color-mix(in srgb, var(--ad-success) 14%, transparent);
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
  max-height: min(calc(100vh - 56px), 760px);
  overflow: auto;
  background: var(--ad-float-solid);
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}

@media (max-width: 1100px) {
  .path { display: none; }
  .bar { padding-inline: 12px; gap: 4px; }
}

@media (max-width: 900px) {
  .proxy-state, .peak { display: none; }
}

@media (max-width: 680px) {
  .project-total, .spend-wrap { display: none; }
  .bar { padding-inline: 8px; gap: 2px; }
  .usage { max-width: 30vw; }
}
</style>
