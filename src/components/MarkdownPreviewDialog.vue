<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from 'vue'
import * as api from '../lib/api'
import {
  DOC_PREVIEW_EDGES,
  loadDocPreviewSize,
  nextDocPreviewSize,
  saveDocPreviewSize,
  type DocPreviewEdge,
  type DocPreviewSize
} from '../lib/docPreview'
import { renderMarkdown } from '../lib/markdown'
import { pageMermaidTheme, paintMermaidDiagrams } from '../lib/mermaidRender'
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
const mdRef = ref<HTMLElement | null>(null)
const size = ref<DocPreviewSize>(loadDocPreviewSize())
let loadSeq = 0
let paintSeq = 0
let resizing: {
  edge: DocPreviewEdge
  startX: number
  startY: number
  start: DocPreviewSize
  pointerId: number
  target: HTMLElement
} | null = null

const html = computed(() => (body.value ? renderMarkdown(body.value.text) : ''))

watch(
  () => [props.open, props.doc?.path, props.projectId],
  () => {
    if (!props.open || !props.doc) {
      body.value = null
      error.value = ''
      return
    }
    size.value = loadDocPreviewSize(window.innerWidth, window.innerHeight)
    void load()
  }
)

watch(
  () => [props.open, html.value, store.settings.uiTheme] as const,
  async () => {
    if (!props.open || !html.value) return
    const seq = ++paintSeq
    await nextTick()
    if (seq !== paintSeq || !mdRef.value) return
    await paintMermaidDiagrams(mdRef.value, pageMermaidTheme())
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

function onResizeMove(event: PointerEvent) {
  if (!resizing || event.pointerId !== resizing.pointerId) return
  size.value = nextDocPreviewSize(
    resizing.start,
    resizing.edge,
    event.clientX - resizing.startX,
    event.clientY - resizing.startY,
    window.innerWidth,
    window.innerHeight
  )
}

function onResizeUp(event: PointerEvent) {
  if (!resizing || event.pointerId !== resizing.pointerId) return
  const { target, pointerId } = resizing
  resizing = null
  document.documentElement.classList.remove('ad-resizing')
  target.removeEventListener('pointermove', onResizeMove)
  target.removeEventListener('pointerup', onResizeUp)
  target.removeEventListener('pointercancel', onResizeUp)
  if (target.hasPointerCapture(pointerId)) target.releasePointerCapture(pointerId)
  saveDocPreviewSize(size.value)
}

function beginResize(edge: DocPreviewEdge, event: PointerEvent) {
  if (event.button !== 0) return
  event.preventDefault()
  event.stopPropagation()
  const target = event.currentTarget as HTMLElement
  resizing = {
    edge,
    startX: event.clientX,
    startY: event.clientY,
    start: { ...size.value },
    pointerId: event.pointerId,
    target
  }
  document.documentElement.classList.add('ad-resizing')
  target.setPointerCapture(event.pointerId)
  target.addEventListener('pointermove', onResizeMove)
  target.addEventListener('pointerup', onResizeUp)
  target.addEventListener('pointercancel', onResizeUp)
}

onUnmounted(() => {
  paintSeq += 1
  if (!resizing) return
  document.documentElement.classList.remove('ad-resizing')
  resizing.target.removeEventListener('pointermove', onResizeMove)
  resizing.target.removeEventListener('pointerup', onResizeUp)
  resizing.target.removeEventListener('pointercancel', onResizeUp)
  resizing = null
})
</script>

<template>
  <div v-if="open" class="ad-mask" @click.self="emit('close')">
    <div
      class="ad-dialog dialog"
      v-modal="() => emit('close')"
      role="dialog"
      aria-modal="true"
      aria-label="文档预览"
      :style="{ width: size.w + 'px', height: size.h + 'px' }"
    >
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
      <div v-else ref="mdRef" class="md" v-html="html" />

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

      <span
        v-for="edge in DOC_PREVIEW_EDGES"
        :key="edge"
        class="grip"
        :class="edge"
        :title="edge === 'se' ? '拖动调整大小' : undefined"
        @pointerdown="beginResize(edge, $event)"
      />
    </div>
  </div>
</template>

<style scoped>
.dialog {
  position: relative;
  width: 1120px;
  max-width: calc(100vw - 48px);
  height: 720px;
  max-height: calc(100vh - 48px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--ad-float-read);
}

header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 16px;
  flex-shrink: 0;
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
  flex: 1;
  min-height: 0;
  padding: 24px 0;
}

.state p {
  margin: 0 0 12px;
}

.md {
  flex: 1;
  min-height: 0;
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

.md :deep(.ad-mermaid) {
  margin: 0 0 12px;
  padding: 16px;
  overflow: auto;
  background: var(--ad-harbor);
  border: 1px solid var(--ad-border);
  border-radius: 8px;
}

.md :deep(.ad-mermaid svg) {
  display: block;
  max-width: 100%;
  height: auto;
  margin: 0 auto;
}

.md :deep(.ad-mermaid.is-error) {
  white-space: pre-wrap;
  font-family: var(--ad-mono);
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-error);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 16px;
  flex-shrink: 0;
}

.grip {
  position: absolute;
  z-index: 2;
  touch-action: none;
}

.grip.n,
.grip.s {
  left: 12px;
  right: 12px;
  height: 6px;
  cursor: ns-resize;
}

.grip.n {
  top: 0;
}

.grip.s {
  bottom: 0;
}

.grip.e,
.grip.w {
  top: 12px;
  bottom: 12px;
  width: 6px;
  cursor: ew-resize;
}

.grip.e {
  right: 0;
}

.grip.w {
  left: 0;
}

.grip.ne,
.grip.nw,
.grip.se,
.grip.sw {
  width: 12px;
  height: 12px;
}

.grip.ne {
  top: 0;
  right: 0;
  cursor: nesw-resize;
}

.grip.nw {
  top: 0;
  left: 0;
  cursor: nwse-resize;
}

.grip.se {
  right: 0;
  bottom: 0;
  cursor: nwse-resize;
}

.grip.sw {
  left: 0;
  bottom: 0;
  cursor: nesw-resize;
}

.grip.se::after {
  content: '';
  position: absolute;
  right: 4px;
  bottom: 4px;
  width: 8px;
  height: 8px;
  border-right: 2px solid var(--ad-muted);
  border-bottom: 2px solid var(--ad-muted);
  pointer-events: none;
}
</style>
