<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { editHandoff, handoffPreview, sendHandoff } from '../lib/flow/runtime.ts'
import { currentFlow, currentRun } from '../lib/store'
import { roleLabel } from '../lib/flow/roles.ts'

const sending = ref(false)
const draft = ref('')

const envelope = computed(() => currentRun.value?.pendingHandoff ?? null)
const toNode = computed(() =>
  currentFlow.value?.nodes.find((item) => item.id === envelope.value?.toNode) ?? null
)

watch(
  envelope,
  (value) => {
    draft.value = value ? handoffPreview() : ''
  },
  { immediate: true }
)

async function send(doSend: boolean) {
  sending.value = true
  editHandoff(draft.value)
  try {
    await sendHandoff(doSend)
  } finally {
    sending.value = false
  }
}
</script>

<template>
  <section v-if="envelope" class="panel" aria-label="手动桥接">
    <header>
      <h2>手动桥接</h2>
      <p class="hint">
        {{ roleLabel(envelope.fromRole) }} → {{ toNode ? roleLabel(toNode.role) : roleLabel(envelope.toRole) }}。
        默认只写入提示行，不回车。
      </p>
    </header>
    <label class="field">
      <span>即将写入的信封</span>
      <textarea v-model="draft" rows="8" />
    </label>
    <div class="actions">
      <button type="button" class="btn btn-ghost" :disabled="sending" @click="send(false)">
        {{ sending ? '写入中…' : '贴入，不发送' }}
      </button>
      <button type="button" class="btn btn-primary" :disabled="sending" @click="send(true)">
        贴入并发送
      </button>
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

.field {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.field > span {
  font-size: 12px;
  color: var(--ad-muted);
}

textarea {
  min-height: 140px;
  resize: vertical;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 10px;
}
</style>
