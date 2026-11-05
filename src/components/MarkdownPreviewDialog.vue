<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import '../styles/document-reader.css'
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
import { disposeMermaidDiagrams, pageMermaidTheme, paintMermaidDiagrams } from '../lib/mermaidRender'
import { formatCiteBlock } from '../lib/sessionCite'
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
const scrollRef = ref<HTMLElement | null>(null)
const size = ref<DocPreviewSize>(loadDocPreviewSize())
const maximized = ref(false)
const view = ref<'rendered' | 'source'>('rendered')
const outline = ref<{ id: string; text: string; level: number }[]>([])
const outlineOpen = ref(window.innerWidth >= 1000)
const activeHeading = ref('')
const compact = ref(true)
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
      activeHeading.value = ''
      return
    }
    view.value = 'rendered'
    body.value = null
    outline.value = []
    activeHeading.value = ''
    size.value = loadDocPreviewSize(window.innerWidth, window.innerHeight)
    void load()
  },
  { immediate: true }
)

watch(
  () => [props.open, html.value, view.value, store.settings.uiTheme] as const,
  async () => {
    const seq = ++paintSeq
    if (mdRef.value) disposeMermaidDiagrams(mdRef.value)
    if (!props.open || !html.value || view.value !== 'rendered') return
    await nextTick()
    if (seq !== paintSeq || !mdRef.value) return
    outline.value = Array.from(mdRef.value.querySelectorAll<HTMLElement>('h1, h2, h3')).map((heading, index) => {
      heading.id = `ad-doc-heading-${index}`
      return { id: heading.id, text: heading.textContent || '', level: Number(heading.tagName[1]) }
    })
    updateActiveHeading()
    try {
      await paintMermaidDiagrams(mdRef.value, pageMermaidTheme())
      if (seq === paintSeq) updateActiveHeading()
    } catch {
      if (seq === paintSeq) showToast('图表加载失败，可切换 Markdown 原文查看')
    }
  }
)

function updateActiveHeading() {
  if (!scrollRef.value || !mdRef.value) return
  const top = scrollRef.value.getBoundingClientRect().top + 48
  let current = outline.value[0]?.id || ''
  for (const item of outline.value) {
    const heading = mdRef.value.querySelector<HTMLElement>(`#${item.id}`)
    if (heading && heading.getBoundingClientRect().top <= top) current = item.id
  }
  activeHeading.value = current
}

function goToHeading(id: string) {
  activeHeading.value = id
  const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches
  mdRef.value?.querySelector<HTMLElement>(`#${id}`)?.scrollIntoView({ behavior: reducedMotion ? 'instant' : 'smooth', block: 'start' })
  if ((scrollRef.value?.parentElement?.clientWidth ?? Infinity) <= 760) outlineOpen.value = false
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
    await api.writeSessionText(store.activePtyId, citePayload(includeBody))
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

onBeforeUnmount(() => {
  paintSeq += 1
  if (mdRef.value) disposeMermaidDiagrams(mdRef.value)
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
      :data-density="compact ? 'compact' : 'comfortable'"
      :style="maximized ? { width: 'calc(100vw - 24px)', height: 'calc(100vh - 24px)' } : { width: size.w + 'px', height: size.h + 'px' }"
    >
      <header>
        <div class="heading">
          <span class="eyebrow">{{ doc?.kind === 'summary' ? `第 ${doc.round || ''} 轮总结` : 'MARKDOWN 文档' }}</span>
          <h2 :title="doc?.title || body?.title || '文档'">{{ doc?.title || body?.title || '文档' }}</h2>
          <p class="hint" :title="body?.relPath || doc?.path">{{ body?.relPath || doc?.relPath || (doc?.kind === 'summary' ? '会话总结 · 已保存到本机' : doc?.path) }}</p>
        </div>
        <div class="window-actions"><button type="button" class="btn btn-ghost btn-small" :aria-pressed="maximized" @click="maximized = !maximized">{{ maximized ? '还原' : '全屏阅读' }}</button><button type="button" class="btn btn-ghost btn-small" @click="emit('close')">关闭</button></div>
      </header>

      <div class="reader-toolbar">
        <div role="tablist" aria-label="阅读模式">
          <button id="doc-rendered-tab" type="button" role="tab" :aria-selected="view === 'rendered'" aria-controls="doc-reading-content" @click="view = 'rendered'">阅读</button>
          <button id="doc-source-tab" type="button" role="tab" :aria-selected="view === 'source'" aria-controls="doc-reading-content" @click="view = 'source'">Markdown 原文</button>
        </div>
        <div class="reader-options">
          <button v-if="view === 'rendered' && outline.length > 1" type="button" :aria-pressed="outlineOpen" aria-controls="doc-outline" @click="outlineOpen = !outlineOpen">目录</button>
          <button v-if="view === 'rendered'" type="button" :aria-pressed="compact" title="切换紧凑排版" @click="compact = !compact">紧凑</button>
          <span>{{ characterCount }} 字符</span>
        </div>
      </div>
      <div v-if="loading" class="state muted" role="status">正在打开文档…</div>
      <div v-else-if="error" class="state">
        <p>{{ error }}</p>
        <button type="button" class="btn btn-ghost btn-small" @click="load">重试</button>
      </div>
      <div v-else-if="body && !body.text.trim()" class="state muted">这份文档暂无内容。</div>
      <div v-else id="doc-reading-content" class="reading" role="tabpanel" :aria-labelledby="view === 'rendered' ? 'doc-rendered-tab' : 'doc-source-tab'">
        <nav v-if="view === 'rendered' && outline.length > 1 && outlineOpen" id="doc-outline" class="outline" aria-label="文档目录">
          <h3>目录 <span>{{ outline.length }}</span></h3>
          <button v-for="item in outline" :key="item.id" type="button" :class="[`level-${item.level}`, { active: activeHeading === item.id }]" :aria-current="activeHeading === item.id ? 'location' : undefined" :title="item.text" @click="goToHeading(item.id)">{{ item.text }}</button>
        </nav>
        <div ref="scrollRef" class="reading-scroll" @scroll.passive="updateActiveHeading"><article v-if="view === 'rendered'" ref="mdRef" class="md" v-html="html" /><pre v-else class="source">{{ body?.text }}</pre></div>
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
