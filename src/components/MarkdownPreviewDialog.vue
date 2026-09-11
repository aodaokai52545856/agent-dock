<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import * as api from '../lib/api'
import { renderMarkdown } from '../lib/markdown'
import { formatCiteBlock, wrapBracketedPaste } from '../lib/sessionCite'
import { showToast, store } from '../lib/store'
import type { SessionDoc, SessionDocBody, ToolId } from '../lib/types'

const props = defineProps<{
  open: boolean
  projectId: string
  doc: SessionDoc | null
  toolId: ToolId
  sessionTitle: string
}>()

const emit = defineEmits<{
  close: []
}>()

const body = ref<SessionDocBody | null>(null)
const loading = ref(false)
const error = ref('')
const citing = ref(false)
let loadSeq = 0

const html = computed(() => (body.value ? renderMarkdown(body.value.text) : ''))

watch(
  () => [props.open, props.doc?.path, props.projectId],
  () => {
    if (!props.open || !props.doc) {
      body.value = null
      error.value = ''
      return
    }
    void load()
  }
)

async function load() {
  if (!props.doc) return
  const seq = ++loadSeq
  loading.value = true
  error.value = ''
  try {
    const next = await api.readSessionDoc(props.projectId, props.doc.path)
    if (seq !== loadSeq) return
    body.value = next
  } catch (err) {
    if (seq !== loadSeq) return
    error.value = err instanceof Error ? err.message : String(err)
    body.value = null
  } finally {
    if (seq === loadSeq) loading.value = false
  }
}

async function copyText() {
  const text = body.value?.text
  if (!text) return
  try {
    await navigator.clipboard.writeText(text)
    showToast('已复制文档')
  } catch {
    showToast('复制失败，请手动选择文本')
  }
}

function citePayload(includeBody: boolean) {
  return formatCiteBlock({
    toolId: props.toolId,
    sessionTitle: props.sessionTitle,
    relPath: body.value?.relPath ?? props.doc?.relPath,
    body: includeBody ? body.value?.text : null
  })
}

async function cite(includeBody: boolean) {
  if (!store.activePtyId) {
    showToast('先打开目标会话')
    return
  }
  citing.value = true
  try {
    await api.ptyWrite(store.activePtyId, wrapBracketedPaste(citePayload(includeBody)))
    showToast('已写入当前终端，未发送')
    emit('close')
  } catch (err) {
    showToast(err instanceof Error ? err.message : String(err))
  } finally {
    citing.value = false
  }
}
</script>

<template>
  <div v-if="open" class="ad-mask" @click.self="emit('close')">
    <div class="ad-dialog dialog" role="dialog" aria-modal="true" aria-label="文档预览">
      <header>
        <div>
          <h2>{{ body?.title || doc?.title || '文档' }}</h2>
          <p class="hint">{{ body?.relPath || doc?.relPath || '会话本地文件' }}</p>
        </div>
        <button type="button" class="btn btn-ghost btn-small" @click="emit('close')">关闭</button>
      </header>

      <div v-if="loading" class="state muted">正在打开文档</div>
      <div v-else-if="error" class="state">
        <p>{{ error }}</p>
        <button type="button" class="btn btn-ghost btn-small" @click="load">重试</button>
      </div>
      <div v-else class="md" v-html="html" />

      <div class="actions">
        <button type="button" class="btn btn-ghost" :disabled="!body" @click="copyText">复制</button>
        <button
          v-if="body?.relPath || doc?.relPath"
          type="button"
          class="btn btn-ghost"
          :disabled="citing || !body"
          @click="cite(false)"
        >
          引用路径
        </button>
        <button type="button" class="btn btn-primary" :disabled="citing || !body" @click="cite(true)">
          {{ citing ? '写入中…' : '引用到当前终端' }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dialog {
  width: 880px;
  max-width: calc(100vw - 48px);
  max-height: calc(100vh - 48px);
  display: flex;
  flex-direction: column;
}

header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 16px;
}

h2 {
  margin: 0;
  font-size: 16px;
  line-height: 24px;
}

.hint,
.muted {
  margin: 4px 0 0;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.state {
  padding: 24px 0;
}

.state p {
  margin: 0 0 12px;
}

.md {
  flex: 1;
  min-height: 240px;
  max-height: calc(100vh - 240px);
  overflow: auto;
  padding-right: 8px;
  color: var(--ad-text);
  font-size: 14px;
  line-height: 22px;
}

.md :deep(h1),
.md :deep(h2),
.md :deep(h3) {
  margin: 16px 0 8px;
  font-weight: 600;
}

.md :deep(h1) {
  font-size: 20px;
}

.md :deep(h2) {
  font-size: 16px;
}

.md :deep(p),
.md :deep(ul),
.md :deep(ol) {
  margin: 0 0 12px;
}

.md :deep(code) {
  font-family: var(--ad-mono);
  font-size: 13px;
  background: var(--ad-hover);
  padding: 1px 4px;
  border-radius: 4px;
}

.md :deep(pre) {
  overflow: auto;
  padding: 12px;
  margin: 0 0 12px;
  background: var(--ad-hover);
  border-radius: 8px;
}

.md :deep(pre code) {
  padding: 0;
  background: transparent;
}

.md :deep(blockquote) {
  margin: 0 0 12px;
  padding-left: 12px;
  border-left: 2px solid var(--ad-border);
  color: var(--ad-muted);
}

.md :deep(table) {
  width: 100%;
  border-collapse: collapse;
  margin: 0 0 12px;
}

.md :deep(th),
.md :deep(td) {
  border: 1px solid var(--ad-border);
  padding: 6px 8px;
  text-align: left;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 16px;
}
</style>
