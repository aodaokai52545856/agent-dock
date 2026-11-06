<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { approveLessons } from '../lib/flow/runtime.ts'
import { currentRun } from '../lib/store'

const saving = ref(false)
const chosen = ref<string[]>([])
const lessons = computed(() => currentRun.value?.pendingLessons ?? [])

watch(lessons, (rows) => {
  chosen.value = rows.map((row) => row.id)
}, { immediate: true })

async function finish(ids: string[]) {
  saving.value = true
  try {
    await approveLessons(ids)
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <section v-if="lessons.length" class="panel" aria-label="经验审批">
    <header>
      <h2>经验审批</h2>
      <p class="hint">勾选的条目原样写入 .agentDock/memory，下次开发时会读到。</p>
    </header>
    <ul>
      <li v-for="lesson in lessons" :key="lesson.id">
        <label class="row">
          <input v-model="chosen" type="checkbox" :value="lesson.id" :disabled="saving" />
          <span class="title">{{ lesson.title }}</span>
          <span class="scope">{{ lesson.scope }}</span>
        </label>
        <p v-if="lesson.missingSections.length" class="warn">缺段落：{{ lesson.missingSections.join('、') }}</p>
        <p v-if="lesson.missingPaths.length" class="warn">路径不存在：{{ lesson.missingPaths.join('、') }}</p>
        <details>
          <summary>查看全文</summary>
          <pre>{{ lesson.body }}</pre>
        </details>
      </li>
    </ul>
    <div class="actions">
      <button type="button" class="btn btn-primary" :disabled="saving || !chosen.length" @click="finish(chosen)">
        {{ saving ? '写入中…' : `写入 ${chosen.length} 条经验` }}
      </button>
      <button type="button" class="btn btn-ghost" :disabled="saving" @click="finish([])">全部跳过</button>
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

header {
  margin-bottom: 8px;
}

h2 {
  margin: 0;
  font-size: 13px;
  line-height: 20px;
  font-weight: 600;
}

.hint {
  margin: 4px 0 0;
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

ul {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.row {
  display: flex;
  align-items: baseline;
  gap: 8px;
  font-size: 13px;
  line-height: 20px;
}

.title {
  min-width: 0;
  overflow-wrap: anywhere;
}

.scope {
  margin-left: auto;
  flex: none;
  max-width: 40%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  color: var(--ad-muted);
}

.warn {
  margin: 2px 0 0 22px;
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-warning);
}

details {
  margin: 2px 0 0 22px;
  font-size: 12px;
  color: var(--ad-muted);
}

summary {
  cursor: pointer;
}

pre {
  max-height: 240px;
  margin: 6px 0 0;
  overflow: auto;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  font-size: 12px;
  line-height: 18px;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 10px;
}
</style>
