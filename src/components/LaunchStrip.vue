<script setup lang="ts">
import { computed } from 'vue'
import { activeLive, selectedProject, store } from '../lib/store'
import { toolLabel } from '../lib/types'

const emit = defineEmits<{
  close: []
}>()

const props = defineProps<{
  loading?: boolean
  loadingText?: string
}>()

const proxyText = computed(() => {
  const project = selectedProject.value
  if (!project) return '未选项目'
  return project.proxyEnabled ? project.proxyUrl : '代理关'
})

const title = computed(() => activeLive.value?.title ?? '未打开会话')
const live = computed(() => Boolean(activeLive.value))
</script>

<template>
  <header
    v-if="live || loading"
    class="strip"
    aria-label="当前启动"
  >
    <span class="ticket">
      <strong>{{ selectedProject?.name ?? '项目' }}</strong>
      <span class="dot">·</span>
      <span>{{ activeLive ? toolLabel(activeLive.toolId) : selectedProject ? toolLabel(store.selectedTool) : '工具' }}</span>
      <span class="dot">·</span>
      <span :class="{ on: selectedProject?.proxyEnabled }">{{ proxyText }}</span>
      <span class="dot">·</span>
      <span class="title">{{ title }}</span>
    </span>
    <div class="tools">
      <span
        class="pulse"
        :class="{ 'is-live': live && !loading, 'is-loading': loading }"
      >
        {{ loading ? (props.loadingText || '正在打开') : live ? '进行中' : '空闲' }}
      </span>
      <button
        v-if="live"
        type="button"
        class="tool tool-close"
        aria-label="关闭会话"
        @click="emit('close')"
      >
        关闭会话
      </button>
    </div>
  </header>
</template>

<style scoped>
.strip {
  height: 32px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 var(--ad-stage-pad);
  background: var(--ad-editor);
  border-bottom: 1px solid var(--ad-border);
}

.ticket {
  min-width: 0;
  flex: 1;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
  overflow: hidden;
}

.ticket strong {
  color: var(--ad-text);
  font-weight: 600;
}

.dot {
  opacity: 0.5;
}

.on {
  color: var(--ad-success);
}

.title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--ad-text);
}

.tools {
  display: flex;
  align-items: center;
  flex-shrink: 0;
  margin-left: auto;
  gap: 2px;
}

.pulse {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 22px;
  padding: 0 8px 0 2px;
  margin-right: 4px;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.pulse::before {
  content: '';
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ad-faint);
}

.pulse.is-live {
  color: var(--ad-success);
}

.pulse.is-live::before {
  background: var(--ad-success);
}

.pulse.is-loading {
  color: var(--ad-accent);
}

.pulse.is-loading::before {
  background: var(--ad-accent);
}

.tool {
  height: 22px;
  padding: 0 8px;
  border-radius: var(--ad-radius-control);
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.tool:hover {
  color: var(--ad-text);
  background: var(--ad-hover);
}

.tool-close:hover {
  color: var(--ad-error);
}
</style>
