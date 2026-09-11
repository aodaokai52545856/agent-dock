<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import * as api from '../lib/api'
import {
  GROK_USAGE_INTERVAL_MS,
  formatUsageLine,
  formatUsageTooltip,
  shouldShowGrokUsage,
  usageTone
} from '../lib/grokUsage'
import { currentPipeline, selectedProject, store } from '../lib/store'
import { GATE_LABEL } from '../lib/types'
import type { GrokUsage } from '../lib/types'

defineEmits<{
  settings: []
  refresh: []
}>()

const usage = ref<GrokUsage | null>(null)
const usageLoading = ref(false)
let usageTimer = 0
let usageSeq = 0

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
        fetchedAt: new Date().toISOString(),
        message: err instanceof Error ? err.message : '读取 Grok 用量失败，点击重试'
      }
    }
  } finally {
    if (seq === usageSeq) usageLoading.value = false
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
      return
    }
    void loadUsage()
    usageTimer = window.setInterval(() => {
      void loadUsage()
    }, GROK_USAGE_INTERVAL_MS)
  },
  { immediate: true }
)

onUnmounted(() => {
  usageSeq += 1
  stopUsageTimer()
})
</script>

<template>
  <footer class="bar">
    <span>{{ store.projects.length }} 个项目</span>
    <span v-if="selectedProject" class="path" :title="selectedProject.path">{{ selectedProject.path }}</span>
    <span>{{ selectedProject?.proxyEnabled ? '代理开' : '代理关' }}</span>
    <template v-if="store.appMode === 'bridge'">
      <span>闸门 {{ GATE_LABEL[currentPipeline?.gate ?? 'idle'] }}</span>
      <span v-if="store.live.length">后台仍有 {{ store.live.length }} 个终端</span>
    </template>
    <span v-else>{{ store.live.length }} 个终端</span>
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
    <span class="spacer" />
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
.link:hover {
  color: var(--ad-text);
  background: var(--ad-hover);
}

.usage:disabled {
  cursor: default;
  opacity: 0.72;
}
</style>
