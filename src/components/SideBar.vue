<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { pathTail, relativeTime } from '../lib/format'
import { endPaneAnim, layout, paneDragging, sidebarPaneWidth, toggleSidebar } from '../lib/layout'
import { isPendingSessionId } from '../lib/liveBind'
import { liveOfProject } from '../lib/livePty'
import { findLiveForSession, isSessionScanning, isToolScanning, setSessionToolFilter, store, visibleSessions } from '../lib/store'
import { TOOLS, type SessionToolFilter, type ToolId } from '../lib/types'

const emit = defineEmits<{
  add: []
  edit: [id: string]
  remove: [id: string]
  select: [id: string]
  create: []
  open: [payload: { sessionId: string; toolId: ToolId }]
  retry: []
  settings: []
  'start-rename': [payload: { sessionId: string; toolId: ToolId }]
  rename: [id: string]
  close: [payload: { sessionId: string; toolId: ToolId }]
  delete: [payload: { sessionId: string; toolId: ToolId }]
  cite: [payload: { sessionId: string; toolId: ToolId }]
}>()

defineProps<{
  collapsed: boolean
}>()

const renaming = defineModel<string>('renaming', { default: '' })
const draft = defineModel<string>('draft', { default: '' })
const menu = ref<{ x: number; y: number; sessionId: string; toolId: ToolId; focus: number } | null>(null)
const MENU_WIDTH = 228
const MENU_HEIGHT = 188
const filterOpen = ref(false)
const filterFocus = ref(0)

const toolFilterOptions = computed(() => [{ id: 'all' as const, label: '全部' }, ...TOOLS])
const filterLabel = computed(
  () => toolFilterOptions.value.find((option) => option.id === store.sessionToolFilter)?.label ?? '全部'
)

const visibleTools = computed(() => {
  if (store.sessionToolFilter !== 'all' && TOOLS.some((tool) => tool.id === store.sessionToolFilter)) {
    return TOOLS.filter((tool) => tool.id === store.sessionToolFilter)
  }
  return TOOLS
})

const groups = computed(() =>
  visibleTools.value.map((tool) => ({
    tool,
    sessions: visibleSessions.value.filter((item) => item.toolId === tool.id),
    error: store.sessionErrors[tool.id]
  }))
)

function onPaneTransitionEnd(event: TransitionEvent) {
  if (event.propertyName !== 'width') return
  if (event.target !== event.currentTarget) return
  endPaneAnim()
}

function isLive(sessionId: string, toolId: ToolId) {
  return Boolean(findLiveForSession(sessionId, toolId))
}

function isPending(sessionId: string) {
  return isPendingSessionId(sessionId)
}

function projectLiveCount(projectId: string) {
  return liveOfProject(store.live, projectId).length
}

function closeMenu() {
  menu.value = null
}

function closeFilter() {
  filterOpen.value = false
}

function closePopovers() {
  closeMenu()
  closeFilter()
}

function toggleFilter(event: MouseEvent) {
  event.stopPropagation()
  closeMenu()
  filterOpen.value = !filterOpen.value
  if (filterOpen.value) {
    filterFocus.value = Math.max(
      0,
      toolFilterOptions.value.findIndex((option) => option.id === store.sessionToolFilter)
    )
  }
}

function pickFilter(id: SessionToolFilter) {
  void setSessionToolFilter(id)
  closeFilter()
}

function openMenu(event: MouseEvent, sessionId: string, toolId: ToolId) {
  event.preventDefault()
  event.stopPropagation()
  closeFilter()
  menu.value = {
    x: Math.min(event.clientX, window.innerWidth - MENU_WIDTH - 8),
    y: Math.min(event.clientY, window.innerHeight - MENU_HEIGHT - 8),
    sessionId,
    toolId,
    focus: 0
  }
}

function menuLive() {
  return menu.value ? isLive(menu.value.sessionId, menu.value.toolId) : false
}

function menuPending() {
  return menu.value ? isPending(menu.value.sessionId) : false
}

function menuCanCite() {
  return Boolean(menu.value && !menuPending() && (menu.value.toolId === 'grokbuild' || menu.value.toolId === 'kimi'))
}

function activateMenu(index: number) {
  if (index === 0 && !menuPending()) renameSession()
  if (index === 1 && menuCanCite()) citeSession()
  if (index === 2 && menuLive()) closeSession()
  if (index === 3 && !menuPending()) deleteSession()
}

function citeSession() {
  const current = menu.value
  closeMenu()
  if (current) emit('cite', { sessionId: current.sessionId, toolId: current.toolId })
}

function closeSession() {
  const current = menu.value
  closeMenu()
  if (current) emit('close', { sessionId: current.sessionId, toolId: current.toolId })
}

function deleteSession() {
  const current = menu.value
  closeMenu()
  if (current) emit('delete', { sessionId: current.sessionId, toolId: current.toolId })
}

function renameSession() {
  const current = menu.value
  closeMenu()
  if (!current) return
  emit('start-rename', { sessionId: current.sessionId, toolId: current.toolId })
  void nextTick(() => {
    const input = document.querySelector<HTMLInputElement>('.rename')
    input?.focus()
    input?.select()
  })
}

function onKey(event: KeyboardEvent) {
  if (filterOpen.value) {
    if (event.key === 'Escape') {
      closeFilter()
      return
    }
    const options = toolFilterOptions.value
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      const step = event.key === 'ArrowDown' ? 1 : options.length - 1
      filterFocus.value = (filterFocus.value + step) % options.length
      return
    }
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      const option = options[filterFocus.value]
      if (option) pickFilter(option.id)
      return
    }
  }
  if (!menu.value) return
  if (event.key === 'Escape') {
    closeMenu()
    return
  }
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    const next = event.key === 'ArrowDown' ? (menu.value.focus + 1) % 4 : (menu.value.focus + 3) % 4
    menu.value = { ...menu.value, focus: next }
    return
  }
  if (event.key === 'Enter') {
    event.preventDefault()
    activateMenu(menu.value.focus)
    return
  }
  if (event.key === 'F2') {
    event.preventDefault()
    renameSession()
  }
}

onMounted(() => {
  window.addEventListener('click', closePopovers)
  window.addEventListener('blur', closePopovers)
  window.addEventListener('keydown', onKey)
})

onUnmounted(() => {
  window.removeEventListener('click', closePopovers)
  window.removeEventListener('blur', closePopovers)
  window.removeEventListener('keydown', onKey)
})
</script>

<template>
  <aside
    class="rail"
    :class="{ 'rail--collapsed': collapsed, 'rail--static': paneDragging }"
    :style="{ width: sidebarPaneWidth() + 'px' }"
    aria-label="工作区"
    @transitionend="onPaneTransitionEnd"
  >
    <div class="body" :style="{ width: layout.sidebarWidth + 'px' }" :aria-hidden="collapsed">
      <div class="head">
        <p class="kicker">工作区</p>
        <button type="button" class="icon-btn" title="收起工作区" @click="toggleSidebar">‹</button>
      </div>

      <button type="button" class="start-btn" @click="$emit('create')">
        <span class="start-plus" aria-hidden="true">+</span>
        新建会话
      </button>

      <div class="block">
        <div class="block-head">
          <p class="kicker">项目</p>
          <button type="button" class="text-btn" @click="$emit('add')">添加</button>
        </div>
        <ul v-if="store.projects.length" class="projects">
          <li v-for="project in store.projects" :key="project.id">
            <div
              class="project"
              :class="{
                'project--active': store.selectedProjectId === project.id,
                'project--proxy': project.proxyEnabled
              }"
            >
              <button type="button" class="project-main" @click="$emit('select', project.id)">
                <svg class="glyph" viewBox="0 0 16 16" aria-hidden="true">
                  <path d="M2.5 5h4.1l1.1 1.3H13.5V12.5H2.5z" />
                </svg>
                <span class="project-body">
                  <span class="project-name">{{ project.name }}</span>
                  <span class="project-path" :title="project.path">{{ pathTail(project.path) }}</span>
                </span>
                <span
                  v-if="projectLiveCount(project.id)"
                  class="project-live"
                  :title="projectLiveCount(project.id) + ' 个终端在运行'"
                >
                  {{ projectLiveCount(project.id) }}
                </span>
                <span v-if="project.proxyEnabled" class="proxy-dot" title="代理开" />
              </button>
              <div class="project-ops">
                <button type="button" class="row-btn" @click.stop="$emit('edit', project.id)">编辑</button>
                <button type="button" class="row-btn row-btn-danger" @click.stop="$emit('remove', project.id)">移除</button>
              </div>
            </div>
          </li>
        </ul>
        <p v-else class="muted pad">还没有项目。先添加一个目录。</p>
      </div>

      <div class="block block--sessions">
        <div class="block-head">
          <div class="block-head-start">
            <p class="kicker">会话</p>
            <div class="tool-picker" @click.stop>
              <button
                type="button"
                class="tool-picker-btn"
                :class="{ 'is-open': filterOpen }"
                aria-haspopup="listbox"
                :aria-expanded="filterOpen"
                aria-label="筛选会话工具"
                @click="toggleFilter"
              >
                <span>{{ filterLabel }}</span>
                <svg class="tool-picker-chevron" viewBox="0 0 12 12" aria-hidden="true">
                  <path d="M2.4 4.2L6 7.8l3.6-3.6" />
                </svg>
              </button>
              <div v-if="filterOpen" class="ad-menu tool-picker-menu" role="listbox" aria-label="会话工具">
                <button
                  v-for="(option, index) in toolFilterOptions"
                  :key="option.id"
                  type="button"
                  class="ad-menu-item"
                  :class="{
                    'is-active': store.sessionToolFilter === option.id,
                    'is-focus': filterFocus === index
                  }"
                  role="option"
                  :aria-selected="store.sessionToolFilter === option.id"
                  @mouseenter="filterFocus = index"
                  @click="pickFilter(option.id)"
                >
                  <span>{{ option.label }}</span>
                  <span v-if="store.sessionToolFilter === option.id" class="tool-picker-check" aria-hidden="true">
                    ✓
                  </span>
                </button>
              </div>
            </div>
          </div>
          <button type="button" class="text-btn" :disabled="isSessionScanning()" @click="$emit('retry')">刷新</button>
        </div>
        <div class="toolbar">
          <input v-model="store.sessionQuery" class="search" type="search" placeholder="搜索会话" aria-label="搜索会话" />
        </div>

        <div v-if="store.sessionStatus === 'error'" class="state">
          <p>{{ store.sessionError }}</p>
          <div class="state-actions">
            <button v-if="store.sessionErrorKind === 'cli_missing'" type="button" class="btn btn-primary btn-small" @click="$emit('settings')">
              去检查安装
            </button>
            <button type="button" class="btn btn-ghost btn-small" @click="$emit('retry')">重试</button>
          </div>
        </div>

        <div v-else-if="!store.selectedProjectId" class="muted pad">点上面的项目，或新建会话时再选。</div>

        <div v-else class="groups">
          <section v-for="group in groups" :key="group.tool.id" class="group">
            <p
              v-if="visibleTools.length > 1 || isToolScanning(group.tool.id)"
              class="group-title"
            >
              <span>{{ group.tool.label }}</span>
              <span
                v-if="isToolScanning(group.tool.id)"
                class="scan-spin"
                role="status"
                :aria-label="`正在扫描 ${group.tool.label} 会话`"
              />
            </p>
            <div v-if="group.error" class="group-error">
              <p>{{ group.error.message }}</p>
              <button
                v-if="group.error.kind === 'cli_missing'"
                type="button"
                class="text-btn"
                @click="$emit('settings')"
              >
                去检查安装
              </button>
            </div>
            <ul v-else-if="group.sessions.length" class="sessions">
              <li v-for="session in group.sessions" :key="session.id">
                <div
                  class="row"
                  :class="{ 'row--live': isLive(session.id, session.toolId) }"
                  @contextmenu="openMenu($event, session.id, session.toolId)"
                >
                  <button
                    type="button"
                    class="row-main"
                    :title="session.title"
                    @click="$emit('open', { sessionId: session.id, toolId: session.toolId })"
                  >
                    <span v-if="renaming !== session.id" class="row-title">{{ session.title }}</span>
                    <input
                      v-else
                      v-model="draft"
                      class="rename"
                      maxlength="80"
                      @click.stop
                      @keydown.enter="$emit('rename', session.id)"
                      @keydown.esc="renaming = ''"
                    />
                    <span v-if="isLive(session.id, session.toolId)" class="live-dot" title="进行中" />
                    <span class="row-time">{{ relativeTime(session.updatedAt) }}</span>
                  </button>
                </div>
              </li>
            </ul>
            <p v-else-if="isToolScanning(group.tool.id)" class="muted group-empty">正在扫描</p>
            <p v-else class="muted group-empty">暂无会话</p>
          </section>
        </div>
      </div>
    </div>

    <div class="strip" :aria-hidden="!collapsed">
      <button type="button" class="icon-btn" title="展开工作区" @click="toggleSidebar">›</button>
      <span class="collapsed-label">工作区</span>
      <button type="button" class="icon-btn" title="新建会话" @click="$emit('create')">+</button>
    </div>

    <Teleport to="body">
      <div
        v-if="menu"
        class="ad-menu menu"
        role="menu"
        :style="{ left: menu.x + 'px', top: menu.y + 'px', width: MENU_WIDTH + 'px' }"
        @click.stop
        @contextmenu.prevent
      >
        <button
          type="button"
          role="menuitem"
          class="ad-menu-item"
          :class="{ 'is-focus': menu.focus === 0 }"
          :disabled="menuPending()"
          @mouseenter="menu.focus = 0"
          @click="renameSession"
        >
          <span>重命名</span>
          <span class="ad-menu-hint">F2</span>
        </button>
        <div class="ad-menu-sep" role="separator" />
        <button
          type="button"
          role="menuitem"
          class="ad-menu-item"
          :class="{ 'is-focus': menu.focus === 1 }"
          :disabled="!menuCanCite()"
          @mouseenter="menu.focus = 1"
          @click="citeSession"
        >
          <span>引用片段</span>
        </button>
        <div class="ad-menu-sep" role="separator" />
        <button
          type="button"
          role="menuitem"
          class="ad-menu-item"
          :class="{ 'is-focus': menu.focus === 2 }"
          :disabled="!menuLive()"
          @mouseenter="menu.focus = 2"
          @click="closeSession"
        >
          <span>关闭会话</span>
        </button>
        <div class="ad-menu-sep" role="separator" />
        <button
          type="button"
          role="menuitem"
          class="ad-menu-item ad-menu-item--danger"
          :class="{ 'is-focus': menu.focus === 3 }"
          :disabled="menuPending()"
          @mouseenter="menu.focus = 3"
          @click="deleteSession"
        >
          <span>删除会话</span>
        </button>
      </div>
    </Teleport>
  </aside>
</template>

<style scoped>
.rail {
  position: relative;
  flex-shrink: 0;
  background: var(--ad-sidebar);
  min-width: 0;
  overflow: hidden;
  transition: width var(--ad-pane-move);
}

.rail--static {
  transition: none;
}

.body {
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
  opacity: 1;
  transition: opacity var(--ad-pane-fade);
}

.rail--collapsed .body {
  opacity: 0;
  pointer-events: none;
}

.strip {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 8px 0;
  opacity: 0;
  pointer-events: none;
  transition: opacity var(--ad-pane-fade);
}

.rail--collapsed .strip {
  opacity: 1;
  pointer-events: auto;
  transition-delay: 60ms;
}

.head,
.block-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.head {
  height: 36px;
  padding: 0 8px 0 12px;
}

.start-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  width: calc(100% - 16px);
  height: 32px;
  margin: 0 8px 8px;
  padding: 0 10px;
  border-radius: 8px;
  color: var(--ad-text);
  text-align: left;
}

.start-btn:hover {
  background: var(--ad-hover);
}

.start-plus {
  width: 16px;
  color: var(--ad-muted);
}

.block {
  padding: 0 0 8px;
  border-bottom: 1px solid var(--ad-border);
}

.block--sessions {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  border-bottom: none;
}

.block-head {
  padding: 0 12px;
  height: 32px;
}

.block-head-start {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.tool-picker {
  position: relative;
}

.tool-picker-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 86px;
  height: 28px;
  padding: 0 8px 0 10px;
  border: 1px solid transparent;
  border-radius: var(--ad-radius-control);
  color: var(--ad-text);
  font-size: 12px;
  line-height: 20px;
}

.tool-picker-btn:hover,
.tool-picker-btn.is-open {
  background: var(--ad-hover);
}

.tool-picker-chevron {
  width: 10px;
  height: 10px;
  flex-shrink: 0;
  fill: none;
  stroke: var(--ad-muted);
  stroke-width: 1.4;
  stroke-linecap: square;
  stroke-linejoin: miter;
  transition: transform var(--ad-transition);
}

.tool-picker-btn.is-open .tool-picker-chevron {
  transform: rotate(180deg);
}

.tool-picker-menu {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  z-index: 50;
  min-width: 148px;
  animation: ad-rise 140ms ease;
}

@media (prefers-reduced-motion: reduce) {
  .tool-picker-menu {
    animation: none;
  }
}

.tool-picker-check {
  color: var(--ad-muted);
  font-size: 12px;
}

.kicker {
  margin: 0;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-faint);
}

.text-btn {
  padding: 0 4px;
  height: 28px;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.text-btn:hover:not(:disabled) {
  color: var(--ad-text);
}

.projects {
  list-style: none;
  margin: 0;
  padding: 0 8px;
  max-height: 168px;
  overflow: auto;
}

.project {
  display: flex;
  align-items: center;
  gap: 2px;
  margin-bottom: 2px;
  border-radius: 8px;
}

.project:hover,
.project:focus-within {
  background: var(--ad-hover);
}

.project--active {
  background: var(--ad-selected);
}

.project-main {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  text-align: left;
}

.glyph {
  width: 15px;
  height: 15px;
  flex-shrink: 0;
  fill: none;
  stroke: var(--ad-muted);
  stroke-width: 1.2;
  stroke-linejoin: round;
}

.project--active .glyph {
  stroke: var(--ad-text);
}

.proxy-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ad-success);
  flex-shrink: 0;
}

.project-live {
  min-width: 16px;
  height: 16px;
  padding: 0 5px;
  border-radius: 999px;
  background: rgba(61, 154, 106, 0.16);
  color: var(--ad-success);
  font-size: 11px;
  line-height: 16px;
  text-align: center;
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}

.project-body {
  min-width: 0;
  display: flex;
  flex-direction: column;
  flex: 1;
}

.project-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--ad-text);
}

.project-path,
.muted,
.group-empty,
.group-error p {
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.project-ops {
  display: flex;
  gap: 2px;
  padding-right: 6px;
  flex-shrink: 0;
}

.row-btn {
  height: 24px;
  padding: 0 6px;
  font-size: 12px;
  color: var(--ad-muted);
  border-radius: 6px;
}

.row-btn:hover {
  color: var(--ad-text);
  background: rgba(255, 255, 255, 0.06);
}

.row-btn-danger:hover {
  color: var(--ad-error);
}

.toolbar {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 4px 12px 0;
}

.search,
.toolbar .btn {
  width: 100%;
}

.search {
  border-radius: var(--ad-radius-control);
}

.groups {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 8px 12px 16px;
}

.group {
  margin-bottom: 12px;
}

.group-title {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0 12px 6px;
  font-size: 11px;
  line-height: 20px;
  color: var(--ad-faint);
}

.scan-spin {
  width: 12px;
  height: 12px;
  border: 1.5px solid rgba(255, 255, 255, 0.18);
  border-top-color: var(--ad-text);
  border-radius: 50%;
  animation: ad-scan-spin 700ms linear infinite;
}

@keyframes ad-scan-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .scan-spin {
    animation: none;
    border-color: var(--ad-muted);
  }
}

.group-error,
.group-empty,
.pad {
  padding: 0 12px;
}

.sessions {
  list-style: none;
  margin: 0;
  padding: 0;
}

.row {
  display: flex;
  align-items: center;
  margin-bottom: 3px;
  border-radius: 10px;
}

.row:hover {
  background: var(--ad-hover);
}

.row--live {
  background: transparent;
}

.live-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ad-success);
  flex-shrink: 0;
}

.row-main {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 10px;
  height: 38px;
  padding: 0 12px;
  text-align: left;
}

.row-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.row-time {
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
  font-variant-numeric: tabular-nums;
}

.rename {
  flex: 1;
  min-width: 0;
  height: 30px;
  padding: 0 8px;
}

.state,
.skel {
  padding: 16px;
}

.state p {
  margin: 0 0 16px;
  color: var(--ad-muted);
}

.state-actions {
  display: flex;
  gap: 8px;
}

.skel-row {
  height: 28px;
  margin-bottom: 8px;
  border-radius: 8px;
  background: var(--ad-hover);
}

.icon-btn {
  width: 28px;
  height: 28px;
  color: var(--ad-muted);
  border-radius: 8px;
}

.icon-btn:hover {
  color: var(--ad-text);
  background: var(--ad-hover);
}

.collapsed-label {
  writing-mode: vertical-rl;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
  letter-spacing: 2px;
}

.menu {
  position: fixed;
  z-index: 40;
}
</style>
