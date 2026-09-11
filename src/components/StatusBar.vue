<script setup lang="ts">
import { currentPipeline, selectedProject, store } from '../lib/store'
import { GATE_LABEL } from '../lib/types'

defineEmits<{
  settings: []
  refresh: []
}>()
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

.link {
  height: 22px;
  padding: 0 8px;
  border-radius: var(--ad-radius-control);
  color: var(--ad-muted);
}

.link:hover {
  color: var(--ad-text);
  background: var(--ad-hover);
}
</style>
