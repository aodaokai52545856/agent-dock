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
const maximized = ref(false)
const view = ref<'rendered' | 'source'>('rendered')
const outline = ref<{ id: string; text: string; level: number }[]>([])
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
const characterCount = computed(() => body.value?.text.replace(/\s/g, '').length.toLocaleString() || '0')

watch(
  () => [props.open, props.doc?.path, props.projectId],
  () => {
    if (!props.open || !props.doc) {
      loadSeq += 1
      paintSeq += 1
      body.value = null
      error.value = ''
      loading.value = false
      maximized.value = false
      outline.value = []
      return
    }
    view.value = 'rendered'
    body.value = null
    size.value = loadDocPreviewSize(window.innerWidth, window.innerHeight)
    void load()
  },
  { immediate: true }
)

watch(
  () => [props.open, html.value, view.value, store.settings.uiTheme] as const,
  async () => {
    const seq = ++paintSeq
    if (!props.open || !html.value || view.value !== 'rendered') return
    await nextTick()
    if (seq !== paintSeq || !mdRef.value) return
    outline.value = Array.from(mdRef.value.querySelectorAll<HTMLElement>('h1, h2, h3')).map((heading, index) => {
      heading.id = `ad-doc-heading-${index}`
      return { id: heading.id, text: heading.textContent || '', level: Number(heading.tagName[1]) }
    })
    await paintMermaidDiagrams(mdRef.value, pageMermaidTheme())
  }
)

function goToHeading(id: string) {
  mdRef.value?.querySelector<HTMLElement>(`#${id}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

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
  <div v-if="open" class="ad-mask reader-mask" :class="{ maximized }" @click.self="emit('close')">
    <div
      class="ad-dialog dialog"
      v-modal="() => emit('close')"
      role="dialog"
      aria-modal="true"
      aria-label="文档阅读"
      :style="maximized ? { width: 'calc(100vw - 24px)', height: 'calc(100vh - 24px)' } : { width: size.w + 'px', height: size.h + 'px' }"
    >
      <header>
        <div class="heading">
          <span class="eyebrow">{{ doc?.kind === 'summary' ? `第 ${doc.round || ''} 轮总结` : 'MARKDOWN 文档' }}</span>
          <h2>{{ doc?.title || body?.title || '文档' }}</h2>
          <p class="hint" :title="body?.relPath || doc?.path">{{ body?.relPath || doc?.relPath || (doc?.kind === 'summary' ? '会话总结 · 已保存到本机' : doc?.path) }}</p>
        </div>
        <div class="window-actions"><button type="button" class="btn btn-ghost btn-small" :aria-pressed="maximized" @click="maximized = !maximized">{{ maximized ? '还原' : '全屏阅读' }}</button><button type="button" class="btn btn-ghost btn-small" @click="emit('close')">关闭</button></div>
      </header>

      <div class="reader-toolbar"><div role="tablist" aria-label="阅读模式"><button id="doc-rendered-tab" type="button" role="tab" :aria-selected="view === 'rendered'" aria-controls="doc-reading-content" @click="view = 'rendered'">阅读</button><button id="doc-source-tab" type="button" role="tab" :aria-selected="view === 'source'" aria-controls="doc-reading-content" @click="view = 'source'">Markdown 原文</button></div><span>{{ characterCount }} 字符</span></div>
      <div v-if="loading" class="state muted" role="status">正在打开文档…</div>
      <div v-else-if="error" class="state">
        <p>{{ error }}</p>
        <button type="button" class="btn btn-ghost btn-small" @click="load">重试</button>
      </div>
      <div v-else id="doc-reading-content" class="reading" role="tabpanel" :aria-labelledby="view === 'rendered' ? 'doc-rendered-tab' : 'doc-source-tab'">
        <nav v-if="view === 'rendered' && outline.length > 1" class="outline" aria-label="文档目录"><h3>目录</h3><button v-for="item in outline" :key="item.id" type="button" :class="`level-${item.level}`" :title="item.text" @click="goToHeading(item.id)">{{ item.text }}</button></nav>
        <div class="reading-scroll"><article v-if="view === 'rendered'" ref="mdRef" class="md" v-html="html" /><pre v-else class="source">{{ body?.text }}</pre></div>
      </div>

      <div class="actions">
        <span class="reading-meta">{{ doc?.updatedAt ? new Date(doc.updatedAt).toLocaleString() : '' }}</span>
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
        v-for="edge in maximized ? [] : DOC_PREVIEW_EDGES"
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
  container-type: inline-size;
  padding: 0;
  width: 1280px;
  max-width: calc(100vw - 48px);
  height: 800px;
  max-height: calc(100vh - 48px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--ad-float-read);
}
.reader-mask.maximized { padding: 12px; }
.reader-mask.maximized .dialog { max-width: calc(100vw - 24px); max-height: calc(100vh - 24px); }

header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding: 20px 24px 16px;
  flex-shrink: 0;
}
.heading { min-width: 0; }
.eyebrow { color: var(--ad-muted); font-size: 10px; letter-spacing: .08em; line-height: 20px; }
.window-actions { display: flex; gap: 8px; flex-shrink: 0; }

h2 {
  margin: 4px 0;
  font-size: 18px;
  line-height: 28px;
  overflow: hidden;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow-wrap: anywhere;
}
.hint { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.reader-toolbar { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 0 24px; border-block: 1px solid var(--ad-border); flex-shrink: 0; }
.reader-toolbar [role='tablist'] { display: flex; gap: 24px; }
.reader-toolbar button { height: 40px; border-bottom: 2px solid transparent; font-size: 12px; color: var(--ad-muted); }
.reader-toolbar button[aria-selected='true'] { color: var(--ad-text); border-bottom-color: var(--ad-text); }
.reader-toolbar > span { color: var(--ad-muted); font-size: 11px; font-variant-numeric: tabular-nums; }
.reading { display: flex; flex: 1; min-height: 0; min-width: 0; }
.reading-scroll { flex: 1; min-width: 0; overflow: auto; scrollbar-gutter: stable; overscroll-behavior: contain; }
.outline { width: 208px; flex-shrink: 0; overflow: auto; padding: 24px 12px; border-right: 1px solid var(--ad-border); }
.outline h3 { margin: 0 12px 12px; font-size: 11px; font-weight: 500; color: var(--ad-muted); }
.outline button { display: block; width: 100%; text-align: left; padding: 6px 12px; border-radius: 4px; font-size: 12px; line-height: 20px; color: var(--ad-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.outline button:hover { color: var(--ad-text); background: var(--ad-hover); }
.outline .level-2 { padding-left: 22px; }.outline .level-3 { padding-left: 32px; }
.source { margin: 0; padding: 32px; white-space: pre-wrap; overflow-wrap: anywhere; font-family: var(--ad-mono); font-size: 13px; line-height: 22px; tab-size: 2; }

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
  padding: 32px 24px;
}

.state p {
  margin: 0 0 12px;
}

.md {
  width: 100%;
  max-width: 1120px;
  margin: 0 auto;
  padding: 32px 40px 64px;
  overflow-wrap: anywhere;
  color: var(--ad-text);
  font-size: 15px;
  line-height: 1.85;
}

.md :deep(h1),
.md :deep(h2),
.md :deep(h3) {
  margin: 28px 0 12px;
  font-weight: 600;
  line-height: 1.5;
  scroll-margin-top: 24px;
}
.md :deep(> :first-child) { margin-top: 0; }

.md :deep(h1) {
  font-size: 26px;
}

.md :deep(h2) {
  font-size: 20px;
}
.md :deep(h3) { font-size: 16px; }
.md :deep(a) { color: var(--ad-accent); text-decoration: underline; text-underline-offset: 3px; }
.md :deep(img) { max-width: 100%; height: auto; border-radius: 6px; }
.md :deep(hr) { margin: 28px 0; border: 0; border-top: 1px solid var(--ad-border); }

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
  display: block;
  overflow: auto;
  width: 100%;
  border-collapse: collapse;
  margin: 0 0 12px;
}

.md :deep(th),
.md :deep(td) {
  border: 1px solid var(--ad-border);
  padding: 10px 14px;
  text-align: left;
}
.md :deep(th) { background: var(--ad-hover); font-weight: 600; }

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
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 24px;
  border-top: 1px solid var(--ad-border);
  flex-shrink: 0;
}
.reading-meta { margin-right: auto; font-size: 11px; color: var(--ad-muted); }
@container (max-width: 920px) { .outline { display: none; }.md { padding: 24px 28px 48px; } }
@container (max-width: 620px) { header { flex-wrap: wrap; padding: 16px; }.window-actions { margin-left: auto; }.reader-toolbar { padding-inline: 16px; }.md { padding: 20px 20px 40px; }.actions { flex-wrap: wrap; padding: 12px 16px; }.reading-meta { width: 100%; }.source { padding: 20px; } }

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
