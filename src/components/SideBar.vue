<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { relativeTime } from '../lib/format'
import { clampOverlayBox, endPaneAnim, layout, sidebarHeadCompact, sidebarPaneWidth, sidebarToolFilterUsesMark, toggleProjects } from '../lib/layout'
import { isFixedDshSession } from '../lib/dsh'
import { isPendingSessionId } from '../lib/liveBind'
import { liveDotForPty, liveDotTitle, projectLiveDot, type LiveDotKind } from '../lib/livePulse'
import { isCurrentSession, liveOfProject } from '../lib/livePty'
import { findLiveForSession, isSessionScanning, setSessionLiveOnly, setSessionToolFilter, store, visibleSessions } from '../lib/store'
import { TOOLS, type SessionToolFilter, type ToolId } from '../lib/types'
import ToolMark from './ToolMark.vue'

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
  rename: [payload: { sessionId: string; toolId: ToolId }]
  close: [payload: { sessionId: string; toolId: ToolId }]
  'close-all': [projectId?: string]
  delete: [payload: { sessionId: string; toolId: ToolId }]
  cite: [payload: { sessionId: string; toolId: ToolId }]
}>()

defineProps<{
  collapsed: boolean
}>()

const renaming = defineModel<string>('renaming', { default: '' })
const draft = defineModel<string>('draft', { default: '' })
const menu = ref<{ x: number; y: number; sessionId: string; toolId: ToolId; focus: number } | null>(null)
const projectMenu = ref<{ x: number; y: number; projectId: string } | null>(null)
const MENU_WIDTH = 228
const MENU_HEIGHT = 226
const filterOpen = ref(false)
const filterFocus = ref(0)

const toolFilterOptions = computed(() => [{ id: 'all' as const, label: '全部' }, ...TOOLS])
const liveFilter = computed(() => store.sessionLiveOnly)
const compactHead = computed(() => sidebarHeadCompact(layout.sidebarWidth))
const filterLabel = computed(
  () => toolFilterOptions.value.find((option) => option.id === store.sessionToolFilter)?.label ?? '全部'
)
const filterToolId = computed(() => {
  const id = store.sessionToolFilter
  return id === 'all' ? undefined : id
})
const filterUsesMark = computed(
  () => Boolean(filterToolId.value) && sidebarToolFilterUsesMark(layout.sidebarWidth, filterLabel.value)
)

const visibleTools = computed(() => {
  if (liveFilter.value) return TOOLS
  if (store.sessionToolFilter !== 'all' && TOOLS.some((tool) => tool.id === store.sessionToolFilter)) {
    return TOOLS.filter((tool) => tool.id === store.sessionToolFilter)
  }
  return TOOLS
})

const groups = computed(() =>
  visibleTools.value
    .map((tool) => ({
      tool,
      sessions: visibleSessions.value.filter(
        (item) => item.toolId === tool.id && (!liveFilter.value || isLive(item.id, item.toolId))
      ),
      error: liveFilter.value ? undefined : store.sessionErrors[tool.id]
    }))
    .filter((group) => !liveFilter.value || group.sessions.length)
)

const liveCount = computed(() => liveOfProject(store.live, store.selectedProjectId).length)

function onPaneTransitionEnd(event: TransitionEvent) {
  if (event.propertyName !== 'width') return
  if (event.target !== event.currentTarget) return
  endPaneAnim()
}

function isLive(sessionId: string, toolId: ToolId) {
  return Boolean(findLiveForSession(sessionId, toolId))
}

function isCurrent(sessionId: string, toolId: ToolId) {
  return isCurrentSession(sessionId, toolId, {
    activePtyId: store.activePtyId,
    focused: store.focusedSession,
    live: findLiveForSession(sessionId, toolId)
  })
}

const now = ref(Date.now())
let pulseTimer = 0

function sessionDot(sessionId: string, toolId: ToolId): LiveDotKind {
  const live = findLiveForSession(sessionId, toolId)
  return liveDotForPty(live, live ? store.ptyDataAt[live.ptyId] : undefined, now.value)
}

function projectDot(projectId: string): LiveDotKind {
  return projectLiveDot(store.live, projectId, store.ptyDataAt, now.value)
}

function isPending(sessionId: string) {
  return isPendingSessionId(sessionId)
}

function projectLiveCount(projectId: string) {
  return liveOfProject(store.live, projectId).length
}

function closeMenu() {
  menu.value = null
  projectMenu.value = null
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

function pickLiveFilter() {
  closeFilter()
  setSessionLiveOnly(!store.sessionLiveOnly)
}

function menuBox(event: MouseEvent, height: number) {
  return clampOverlayBox(
    { x: event.clientX, y: event.clientY, width: MENU_WIDTH, height },
    {
      windowWidth: window.innerWidth,
      windowHeight: window.innerHeight,
      containRight: sidebarPaneWidth()
    }
  )
}

function openMenu(event: MouseEvent, sessionId: string, toolId: ToolId) {
  event.preventDefault()
  event.stopPropagation()
  closeFilter()
  projectMenu.value = null
  const pos = menuBox(event, MENU_HEIGHT)
  menu.value = {
    x: pos.x,
    y: pos.y,
    sessionId,
    toolId,
    focus: 0
  }
}

function menuLive() {
  return menu.value ? isLive(menu.value.sessionId, menu.value.toolId) : false
}

function menuCanCloseAll() {
  return liveOfProject(store.live, store.selectedProjectId).length > 0
}

function menuPending() {
  return menu.value ? isPending(menu.value.sessionId) : false
}

function menuFixedDsh() {
  return Boolean(menu.value && isFixedDshSession(menu.value.toolId, menu.value.sessionId))
}

function menuCanRename() {
  return Boolean(menu.value && !menuPending() && !menuFixedDsh())
}

function menuCanDelete() {
  return Boolean(menu.value && !menuPending() && !menuFixedDsh())
}

function menuCanCite() {
  return Boolean(menu.value && !menuPending() && (menu.value.toolId === 'grokbuild' || menu.value.toolId === 'kimi'))
}

function activateMenu(index: number) {
  if (index === 0 && menuCanRename()) renameSession()
  if (index === 1 && menuCanCite()) citeSession()
  if (index === 2 && menuLive()) closeSession()
  if (index === 3 && menuCanCloseAll()) closeAllSessions()
  if (index === 4 && menuCanDelete()) deleteSession()
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

function closeAllSessions(projectId?: string) {
  closeMenu()
  emit('close-all', projectId || store.selectedProjectId)
}

function openProjectMenu(event: MouseEvent, projectId: string) {
  event.preventDefault()
  event.stopPropagation()
  closeFilter()
  menu.value = null
  const pos = menuBox(event, 52)
  projectMenu.value = {
    x: pos.x,
    y: pos.y,
    projectId
  }
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

function commitRename(sessionId: string, toolId: ToolId) {
  if (renaming.value !== sessionId) return
  emit('rename', { sessionId, toolId })
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
  if (!menu.value) {
    if (projectMenu.value && event.key === 'Escape') closeMenu()
    return
  }
  if (event.key === 'Escape') {
    closeMenu()
    return
  }
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    const next = event.key === 'ArrowDown' ? (menu.value.focus + 1) % 5 : (menu.value.focus + 4) % 5
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
    if (menuCanRename()) renameSession()
  }
}

watch(
  () => [store.activePtyId, store.focusedSession?.sessionId, store.selectedProjectId] as const,
  async () => {
    await nextTick()
    const rail = document.querySelector<HTMLElement>('.rail[aria-label="工作区"]')
    if (!rail || rail.offsetParent === null) return
    rail.querySelector('.row--current')?.scrollIntoView({ block: 'nearest' })
  }
)

onMounted(() => {
  window.addEventListener('click', closePopovers)
  window.addEventListener('blur', closePopovers)
  window.addEventListener('keydown', onKey)
  pulseTimer = window.setInterval(() => {
    now.value = Date.now()
  }, 500)
})

onUnmounted(() => {
  window.removeEventListener('click', closePopovers)
  window.removeEventListener('blur', closePopovers)
  window.removeEventListener('keydown', onKey)
  window.clearInterval(pulseTimer)
})
</script>

<template>
  <aside
    class="rail"
    :class="{ 'rail--collapsed': collapsed }"
    :style="{ width: sidebarPaneWidth() + 'px' }"
    aria-label="工作区"
    @transitionend="onPaneTransitionEnd"
    @contextmenu.prevent
  >
    <div class="body" :style="{ width: layout.sidebarWidth + 'px' }" :aria-hidden="collapsed">
      <div class="head">
        <p class="kicker">工作区</p>
      </div>

      <button type="button" class="start-btn" @click="$emit('create')">
        <span class="start-plus" aria-hidden="true">+</span>
        新建会话
      </button>

      <div class="block">
        <div class="block-head">
          <button type="button" class="fold-btn" :aria-expanded="!layout.projectsCollapsed" @click="toggleProjects">
            <svg class="fold-caret" :class="{ 'is-closed': layout.projectsCollapsed }" viewBox="0 0 16 16" aria-hidden="true">
              <path d="M6 4.5 10 8l-4 3.5" />
            </svg>
            <span class="kicker">项目</span>
          </button>
          <button type="button" class="text-btn" @click="$emit('add')">添加</button>
        </div>
        <ul v-if="!layout.projectsCollapsed && store.projects.length" class="projects">
          <li v-for="project in store.projects" :key="project.id">
            <div
              class="project"
              :class="{
                'project--active': store.selectedProjectId === project.id,
                'project--live': projectDot(project.id) !== 'off',
                'project--busy': projectDot(project.id) === 'busy'
              }"
              :title="liveDotTitle(projectDot(project.id), 'project') || undefined"
              @contextmenu="openProjectMenu($event, project.id)"
            >
              <button type="button" class="project-main" @click="$emit('select', project.id)">
                <svg class="glyph" viewBox="0 0 16 16" aria-hidden="true">
                  <path d="M2.5 5h4.1l1.1 1.3H13.5V12.5H2.5z" />
                </svg>
                <span class="project-body">
                  <span class="project-name" :title="project.path">{{ project.name }}</span>
                </span>
                <span
                  v-if="projectLiveCount(project.id)"
                  class="project-live"
                  :title="projectLiveCount(project.id) + ' 个已打开会话'"
                >
                  {{ projectLiveCount(project.id) }}
                </span>
              </button>
              <div class="project-ops">
                <button type="button" class="row-btn" @click.stop="$emit('edit', project.id)">编辑</button>
                <button type="button" class="row-btn row-btn-danger" @click.stop="$emit('remove', project.id)">移除</button>
              </div>
            </div>
          </li>
        </ul>
        <p v-else-if="!layout.projectsCollapsed" class="muted pad">还没有项目。先添加一个目录。</p>
      </div>

      <div class="block block--sessions">
        <div class="block-head" :class="{ 'is-compact': compactHead }">
          <div class="block-head-start">
            <p class="kicker">会话</p>
            <div class="tool-picker" @click.stop>
              <button
                type="button"
                class="tool-picker-btn"
                :class="{ 'is-open': filterOpen, 'is-mark': filterUsesMark }"
                aria-haspopup="listbox"
                :aria-expanded="filterOpen"
                :aria-label="'筛选会话工具，当前' + filterLabel"
                :title="filterLabel"
                @click="toggleFilter"
              >
                <span v-if="filterUsesMark && filterToolId" class="tool-picker-mark" aria-hidden="true">
                  <ToolMark :id="filterToolId" />
                </span>
                <span v-else class="tool-picker-label">{{ filterLabel }}</span>
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
            <button
              type="button"
              class="live-filter"
              :class="{ 'is-active': liveFilter }"
              :aria-pressed="liveFilter"
              aria-label="已开对话"
              @click="pickLiveFilter"
            >
              已开对话
              <span v-if="liveCount" class="live-filter-count">{{ liveCount }}</span>
            </button>
          </div>
          <button
            type="button"
            class="text-btn refresh-btn"
            :class="{ 'is-scanning': isSessionScanning() }"
            :disabled="isSessionScanning()"
            aria-label="刷新"
            @click="$emit('retry')"
          >
            <svg class="refresh-icon" viewBox="0 0 16 16" aria-hidden="true">
              <path d="M13.2 8A5.2 5.2 0 1 1 11.7 4.4" />
              <path d="M13.2 2.8v3.1h-3.1" />
            </svg>
            <span class="refresh-label">刷新</span>
          </button>
        </div>
        <div class="toolbar">
          <input v-model="store.sessionQuery" class="search" type="search" placeholder="搜索会话" aria-label="搜索会话" />
        </div>

        <div v-if="store.sessionStatus === 'error' && !liveFilter" class="state">
          <p>{{ store.sessionError }}</p>
          <div class="state-actions">
            <button v-if="store.sessionErrorKind === 'cli_missing'" type="button" class="btn btn-primary btn-small" @click="$emit('settings')">
              去检查安装
            </button>
            <button type="button" class="btn btn-ghost btn-small" @click="$emit('retry')">重试</button>
          </div>
        </div>

        <div v-else-if="!store.selectedProjectId" class="muted pad">点上面的项目，或新建会话时再选。</div>
        <div v-else-if="liveFilter && !groups.length" class="muted session-empty">还没有打开的会话</div>

        <div v-else class="session-pane" :class="{ 'is-busy': store.sessionRefreshBusy }">
        <div class="groups">
          <section v-for="group in groups" :key="group.tool.id" class="group">
            <p v-if="liveFilter || visibleTools.length > 1" class="group-title">
              <span>{{ group.tool.label }}</span>
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
                  :class="{
                    'row--live': isLive(session.id, session.toolId),
                    'row--busy': sessionDot(session.id, session.toolId) === 'busy',
                    'row--current': isCurrent(session.id, session.toolId)
                  }"
                  @contextmenu="openMenu($event, session.id, session.toolId)"
                >
                  <button
                    type="button"
                    class="row-main"
                    :title="[session.title, liveDotTitle(sessionDot(session.id, session.toolId), 'session')].filter(Boolean).join(' · ')"
                    :aria-current="isCurrent(session.id, session.toolId) ? 'true' : undefined"
                    @click="$emit('open', { sessionId: session.id, toolId: session.toolId })"
                  >
                    <span v-if="renaming !== session.id" class="row-title">{{ session.title }}</span>
                    <input
                      v-else
                      v-model="draft"
                      class="rename"
                      maxlength="80"
                      @click.stop
                      @keydown.enter.prevent="commitRename(session.id, session.toolId)"
                      @keydown.esc.prevent="renaming = ''"
                      @blur="commitRename(session.id, session.toolId)"
                    />
                    <span class="row-time">{{ relativeTime(session.updatedAt) }}</span>
                  </button>
                </div>
              </li>
            </ul>
            <p v-else class="muted group-empty">暂无会话</p>
          </section>
        </div>
          <div v-if="store.sessionRefreshBusy" class="session-mask" role="status" aria-live="polite">
            <span class="scan-spin" aria-hidden="true" />
            <span>正在刷新会话</span>
          </div>
        </div>
      </div>
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
          :disabled="!menuCanRename()"
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
        <button
          type="button"
          role="menuitem"
          class="ad-menu-item"
          :class="{ 'is-focus': menu.focus === 3 }"
          :disabled="!menuCanCloseAll()"
          @mouseenter="menu.focus = 3"
          @click="closeAllSessions()"
        >
          <span>关闭所有会话</span>
        </button>
        <div class="ad-menu-sep" role="separator" />
        <button
          type="button"
          role="menuitem"
          class="ad-menu-item ad-menu-item--danger"
          :class="{ 'is-focus': menu.focus === 4 }"
          :disabled="!menuCanDelete()"
          @mouseenter="menu.focus = 4"
          @click="deleteSession"
        >
          <span>删除会话</span>
        </button>
      </div>
    </Teleport>
    <Teleport to="body">
      <div
        v-if="projectMenu"
        class="ad-menu menu"
        role="menu"
        :style="{ left: projectMenu.x + 'px', top: projectMenu.y + 'px', width: MENU_WIDTH + 'px' }"
        @click.stop
        @contextmenu.prevent
      >
        <button
          type="button"
          role="menuitem"
          class="ad-menu-item"
          :disabled="!liveOfProject(store.live, projectMenu.projectId).length"
          @click="closeAllSessions(projectMenu.projectId)"
        >
          <span>关闭所有会话</span>
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
}

.body {
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.head,
.block-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  flex-wrap: nowrap;
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

.fold-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  height: 28px;
  padding: 0 4px 0 0;
  color: inherit;
}

.fold-btn:hover .kicker {
  color: var(--ad-text);
}

.fold-caret {
  width: 12px;
  height: 12px;
  flex-shrink: 0;
  fill: none;
  stroke: var(--ad-muted);
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
  transform: rotate(90deg);
}

.fold-caret.is-closed {
  transform: rotate(0deg);
}

.block-head-start {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex-wrap: nowrap;
}

.tool-picker {
  position: relative;
  min-width: 0;
}

.tool-picker:has(.is-mark) {
  flex-shrink: 0;
}

.live-filter {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 28px;
  padding: 0 8px;
  border-radius: var(--ad-radius-control);
  color: var(--ad-muted);
  font-size: 12px;
  line-height: 20px;
  flex-shrink: 0;
}

.live-filter:hover {
  color: var(--ad-text);
  background: var(--ad-hover);
}

.live-filter.is-active {
  color: var(--ad-text);
  background: var(--ad-hover);
}

.live-filter-count {
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
}

.tool-picker-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  max-width: 100%;
  height: 28px;
  padding: 0 8px 0 10px;
  border: 1px solid transparent;
  border-radius: var(--ad-radius-control);
  color: var(--ad-text);
  font-size: 12px;
  line-height: 20px;
  white-space: nowrap;
}

.tool-picker-btn.is-mark {
  flex-shrink: 0;
  padding-left: 8px;
}

.tool-picker-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tool-picker-mark {
  width: 16px;
  height: 16px;
  flex-shrink: 0;
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
  max-width: 3em;
  overflow: hidden;
  white-space: nowrap;
  transition:
    max-width var(--ad-transition),
    opacity var(--ad-transition),
    margin var(--ad-transition);
}

.block-head.is-compact .kicker {
  max-width: 0;
  opacity: 0;
  margin: 0;
  pointer-events: none;
}

.text-btn {
  padding: 0 4px;
  height: 28px;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.refresh-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 28px;
  overflow: hidden;
}

.refresh-icon {
  width: 0;
  height: 14px;
  opacity: 0;
  flex-shrink: 0;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.5;
  stroke-linecap: round;
  stroke-linejoin: round;
  overflow: hidden;
  transition:
    width var(--ad-transition),
    opacity var(--ad-transition);
}

.refresh-label {
  overflow: hidden;
  max-width: 3em;
  white-space: nowrap;
  transition:
    max-width var(--ad-transition),
    opacity var(--ad-transition);
}

.block-head.is-compact .refresh-icon {
  width: 14px;
  opacity: 1;
}

.block-head.is-compact .refresh-label {
  max-width: 0;
  opacity: 0;
}

.refresh-btn.is-scanning .refresh-icon {
  animation: ad-scan-spin 700ms linear infinite;
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

.session-empty {
  flex: 1;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 12px;
  text-align: center;
}

.session-pane {
  position: relative;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.groups {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 8px 12px 16px;
}

.session-pane.is-busy .groups {
  visibility: hidden;
}

.session-mask {
  position: absolute;
  inset: 0;
  z-index: 2;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  /* Extra fill stacked on --ad-sidebar reads darker than the rest of the rail. */
  background: transparent;
  color: var(--ad-muted);
  font-size: 12px;
  line-height: 18px;
}

.session-mask .scan-spin {
  width: 18px;
  height: 18px;
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
  border: 1.5px solid var(--ad-border-strong);
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

.row--current {
  background: var(--ad-selected);
  box-shadow: inset 0 0 0 1px rgba(236, 236, 236, 0.28);
}

.row--current .row-title {
  color: var(--ad-text);
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
  z-index: 80;
}
</style>
