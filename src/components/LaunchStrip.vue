<script setup lang="ts">
import { computed } from 'vue'
import { toolLabel } from '../lib/types'
import { activeLive, selectedProject, store } from '../lib/store'

const emit = defineEmits<{
  close: []
  'toggle-docs': []
}>()

const props = defineProps<{
  loading?: boolean
  loadingText?: string
  docsOpen?: boolean
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
  <header v-if="live || loading" class="strip" aria-label="当前启动">
    <span class="ticket">
      <strong>{{ selectedProject?.name ?? '项目' }}</strong>
      <span class="dot">·</span>
      <span>{{ activeLive ? toolLabel(activeLive.toolId) : selectedProject ? toolLabel(store.selectedTool) : '工具' }}</span>
      <span class="dot">·</span>
      <span :class="{ on: selectedProject?.proxyEnabled }">{{ proxyText }}</span>
      <span class="dot">·</span>
      <span class="title">{{ title }}</span>
    </span>
    <span class="status" :class="{ 'status--live': live && !loading, 'status--loading': loading }">
      {{ loading ? (props.loadingText || '正在打开') : live ? '进行中' : '空闲' }}
    </span>
    <button
      type="button"
      class="btn btn-ghost btn-small"
      :class="{ 'is-on': docsOpen }"
      :aria-pressed="docsOpen"
      @click="emit('toggle-docs')"
    >
      文档
    </button>
    <button v-if="live" type="button" class="btn btn-ghost btn-small" @click="emit('close')">关闭终端</button>
  </header>
</template>

<style scoped>
.strip {
  height: 36px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 20px;
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

.status {
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.status--live {
  color: var(--ad-success);
}

.status--loading {
  color: var(--ad-accent);
}

.btn.is-on {
  background: var(--ad-selected);
  border-color: rgba(255, 255, 255, 0.1);
}
</style>
