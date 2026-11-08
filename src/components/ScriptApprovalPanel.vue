<script setup lang="ts">
import { approveScripts, declineScripts } from '../lib/flow/runtime.ts'
import { store } from '../lib/store'
</script>

<template>
  <section v-if="store.scriptApproval" class="panel" aria-label="脚本确认">
    <header>
      <h2>确认要运行的命令</h2>
      <p class="hint">这些命令写在仓库的流程文件里，会在项目目录下直接执行。看过全文再允许，命令改动后会再问一次。</p>
    </header>
    <ul>
      <li v-for="row in store.scriptApproval.scripts" :key="row.nodeId">
        <span class="title">{{ row.title }}</span>
        <pre>{{ row.command }}</pre>
      </li>
    </ul>
    <div class="actions">
      <button type="button" class="btn btn-primary" @click="approveScripts">允许并运行</button>
      <button type="button" class="btn btn-ghost" @click="declineScripts">不运行</button>
    </div>
  </section>
</template>

<style scoped>
.panel {
  padding: 12px 14px;
  border: 1px solid var(--ad-border);
  border-left: 3px solid var(--ad-warning);
  border-radius: 12px;
  background: var(--ad-harbor);
}

h2 {
  margin: 0;
  font-size: 13px;
}

.hint {
  margin: 4px 0 0;
  font-size: 12px;
  color: var(--ad-text-muted);
}

ul {
  margin: 10px 0;
  padding: 0;
  list-style: none;
  display: grid;
  gap: 8px;
}

.title {
  font-size: 12px;
  font-weight: 600;
}

pre {
  margin: 4px 0 0;
  padding: 8px;
  border-radius: 8px;
  background: var(--ad-surface);
  font-size: 12px;
  white-space: pre-wrap;
  word-break: break-all;
}

.actions {
  display: flex;
  gap: 8px;
}
</style>
