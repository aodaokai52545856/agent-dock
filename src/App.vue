<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import CiteTurnDialog from './components/CiteTurnDialog.vue'
import ConfirmDialog from './components/ConfirmDialog.vue'
import DocRail from './components/DocRail.vue'
import LaunchStrip from './components/LaunchStrip.vue'
import MarkdownPreviewDialog from './components/MarkdownPreviewDialog.vue'
import PaneGutter from './components/PaneGutter.vue'
import ProjectDialog from './components/ProjectDialog.vue'
import NewSessionDialog from './components/NewSessionDialog.vue'
import SettingsDrawer from './components/SettingsDrawer.vue'
import BridgePane from './components/BridgePane.vue'
import BridgeSideBar from './components/BridgeSideBar.vue'
import SideBar from './components/SideBar.vue'
import CcswitchDialog from './components/CcswitchDialog.vue'
import VersionDrawer from './components/VersionDrawer.vue'
import DeepseekKeyDialog from './components/DeepseekKeyDialog.vue'
import GrokAccountDialog from './components/GrokAccountDialog.vue'
import StatusBar from './components/StatusBar.vue'
import WindowResizeFrame from './components/WindowResizeFrame.vue'
import TitleBar from './components/TitleBar.vue'
import DshWebPane from './components/DshWebPane.vue'
import TerminalPane from './components/TerminalPane.vue'
import * as api from './lib/api'
import {
  beginPaneDrag,
  docRailPaneWidth,
  endPaneDrag,
  layout,
  paneDragging,
  resizeDocRail,
  resizeSidebar,
  toggleDocRail
} from './lib/layout'
import { isDshWeb, liveOfProject } from './lib/livePty'
import {
  activeLive,
  boot,
  deleteCurrentSession,
  dropProject,
  bindLiveSession,
  findLiveForSession,
  markPtyExit,
  markWindowBlurred,
  noteGrokAccountChange,
  refreshAfterWindowFocus,
  refreshSessions,
  rememberOpened,
  renameCurrentSession,
  saveAppSettings,
  saveProject,
  selectProject,
  selectedProject,
  setSessionToolFilter,
  showToast,
  store,
  watchPendingSession
} from './lib/store'
import { isPendingSessionId, resolveOpenTarget } from './lib/liveBind'
import { refreshCodexThreads } from './lib/pipeline'
import type { ProjectDraft, SessionDoc, ToolId } from './lib/types'

const projectOpen = ref(false)
const editing = ref(false)
const settingsOpen = ref(false)
const versionOpen = ref(false)
const ccswitchOpen = ref(false)
const accountOpen = ref(false)
const deepseekOpen = ref(false)
const ccswitchBusy = ref(false)
const confirmOpen = ref(false)
const confirmMode = ref<'remove' | 'close' | 'delete' | 'close-all'>('remove')
const closingProjectId = ref('')
const removingId = ref('')
const closingPtyId = ref('')
const deletingSession = ref<{ sessionId: string; toolId: ToolId; title: string } | null>(null)
const renamingId = ref('')
const renameDraft = ref('')

watch(renamingId, (id, prev) => {
  store.renamingSessionId = id
  if (prev && !id) void refreshSessions({ silent: true })
})
const projectDialog = ref<{ stopSave: () => void } | null>(null)
const settingsDialog = ref<{ stopSave: () => void } | null>(null)
const termRef = ref<{ dispose: (id: string) => void; fitActive: (force?: boolean) => void } | null>(null)
const consoleWs = ref<HTMLElement | null>(null)
const bridgeWs = ref<HTMLElement | null>(null)
const paneLoading = ref(false)
const paneLoadingText = ref('正在打开会话')
const newSessionOpen = ref(false)
const reopenNewSession = ref(false)
const previewDoc = ref<SessionDoc | null>(null)
const previewOpen = ref(false)
const citeOpen = ref(false)
const citeSession = ref<{ sessionId: string; toolId: ToolId; title: string } | null>(null)

const removingProject = computed(() => store.projects.find((item) => item.id === removingId.value))

onMounted(() => {
  void boot()
  void nextTick().then(() => {
    requestAnimationFrame(() => {
      void api.revealMainWindow()
    })
  })
  window.addEventListener('focus', refreshAfterWindowFocus)
  window.addEventListener('blur', markWindowBlurred)
})

onUnmounted(() => {
  window.removeEventListener('focus', refreshAfterWindowFocus)
  window.removeEventListener('blur', markWindowBlurred)
})

async function onSelectProject(id: string) {
  await selectProject(id)
  if (store.appMode === 'console') await finishPaneReady()
}

async function onSelectBridgeProject(id: string) {
  await selectProject(id)
  void refreshCodexThreads()
}

function openAdd() {
  editing.value = false
  projectOpen.value = true
}

function onAddProjectFromSession() {
  newSessionOpen.value = false
  reopenNewSession.value = true
  openAdd()
}

function openEdit(id: string) {
  if (store.selectedProjectId !== id) {
    void onSelectProject(id)
  }
  editing.value = true
  projectOpen.value = true
}

async function onSaveProject(draft: ProjectDraft) {
  const editingNow = editing.value
  try {
    await saveProject(draft, editingNow ? selectedProject.value?.id : undefined)
    projectOpen.value = false
    showToast(editingNow ? '项目已保存' : '项目已添加')
    if (store.appMode === 'bridge') void refreshCodexThreads()
    if (!editingNow && reopenNewSession.value) {
      newSessionOpen.value = true
    }
    reopenNewSession.value = false
    void refreshSessions()
  } catch (err) {
    showToast(err instanceof Error ? err.message : String(err))
  } finally {
    projectDialog.value?.stopSave()
  }
}

function askRemove(id: string) {
  removingId.value = id
  confirmMode.value = 'remove'
  confirmOpen.value = true
}

function onSidebarDrag(clientX: number) {
  const el = store.appMode === 'bridge' ? bridgeWs.value : consoleWs.value
  const left = el?.getBoundingClientRect().left ?? 0
  resizeSidebar(clientX - left)
}

function onDocRailDrag(clientX: number) {
  const right = consoleWs.value?.getBoundingClientRect().right ?? 0
  resizeDocRail(right - clientX)
}

function openDocPreview(doc: SessionDoc) {
  previewDoc.value = doc
  previewOpen.value = true
}

function openCiteTurns(payload: { sessionId: string; toolId: ToolId; title?: string }) {
  const session = store.sessions.find((item) => item.id === payload.sessionId && item.toolId === payload.toolId)
  citeSession.value = {
    sessionId: payload.sessionId,
    toolId: payload.toolId,
    title: payload.title || session?.title || payload.sessionId
  }
  citeOpen.value = true
}

watch(
  () => store.appMode,
  async (mode) => {
    if (mode === 'bridge') {
      void refreshCodexThreads()
      return
    }
    await finishPaneReady()
  }
)

watch(
  () => store.activePtyId,
  async (id) => {
    if (!id || paneLoading.value || store.appMode !== 'console') return
    await finishPaneReady()
  }
)

function liveForSession(sessionId: string, toolId: ToolId) {
  return findLiveForSession(sessionId, toolId)
}

function askClose() {
  closingPtyId.value = store.activePtyId
  confirmMode.value = 'close'
  confirmOpen.value = true
}

function askCloseSession(sessionId: string, toolId: ToolId) {
  const live = liveForSession(sessionId, toolId)
  if (!live) {
    showToast('这个会话没有打开的窗口')
    return
  }
  closingPtyId.value = live.ptyId
  confirmMode.value = 'close'
  confirmOpen.value = true
}

function askCloseAll(projectId?: string) {
  const id = projectId || store.selectedProjectId
  const items = liveOfProject(store.live, id)
  if (!items.length) {
    showToast('这个项目没有打开的会话')
    return
  }
  closingProjectId.value = id
  confirmMode.value = 'close-all'
  confirmOpen.value = true
}

function askDeleteSession(sessionId: string, toolId: ToolId) {
  if (isPendingSessionId(sessionId)) {
    showToast('这个会话还在写入磁盘，关掉终端即可')
    return
  }
  const session = store.sessions.find((item) => item.id === sessionId && item.toolId === toolId)
  deletingSession.value = {
    sessionId,
    toolId,
    title: session?.title ?? sessionId
  }
  confirmMode.value = 'delete'
  confirmOpen.value = true
}

async function onConfirm() {
  confirmOpen.value = false
  if (confirmMode.value === 'remove' && removingProject.value) {
    const name = removingProject.value.name
    try {
      await dropProject(removingProject.value.id)
      showToast(`已移除 ${name}`)
    } catch (err) {
      showToast(err instanceof Error ? err.message : String(err))
    }
    return
  }
  if (confirmMode.value === 'close') {
    const id = closingPtyId.value || store.activePtyId
    if (!id) return
    await api.ptyKill(id)
    termRef.value?.dispose(id)
    markPtyExit(id)
    closingPtyId.value = ''
    return
  }
  if (confirmMode.value === 'close-all') {
    const items = liveOfProject(store.live, closingProjectId.value)
    closingProjectId.value = ''
    for (const item of items) {
      await api.ptyKill(item.ptyId)
      termRef.value?.dispose(item.ptyId)
      markPtyExit(item.ptyId)
    }
    showToast(items.length ? `已关闭 ${items.length} 个会话` : '没有打开的会话')
    return
  }
  if (confirmMode.value === 'delete' && deletingSession.value) {
    const target = deletingSession.value
    deletingSession.value = null
    const live = liveForSession(target.sessionId, target.toolId)
    if (live) {
      await api.ptyKill(live.ptyId)
      termRef.value?.dispose(live.ptyId)
      markPtyExit(live.ptyId)
    }
    const session = store.sessions.find((item) => item.id === target.sessionId && item.toolId === target.toolId)
    if (!session) {
      showToast('这个会话已经不在列表里')
      return
    }
    try {
      await deleteCurrentSession(session)
    } catch (err) {
      showToast(err instanceof Error ? err.message : String(err))
    }
  }
}

function startRename(payload: { sessionId: string; toolId: ToolId }) {
  const session = store.sessions.find((item) => item.id === payload.sessionId && item.toolId === payload.toolId)
  renamingId.value = payload.sessionId
  store.renamingSessionId = payload.sessionId
  renameDraft.value = session?.title ?? ''
}

async function commitRename(payload: { sessionId: string; toolId: ToolId }) {
  const session = store.sessions.find(
    (item) => item.id === payload.sessionId && item.toolId === payload.toolId
  )
  const title = renameDraft.value.trim()
  renamingId.value = ''
  store.renamingSessionId = ''
  if (!session || !title || title === session.title) return
  try {
    await renameCurrentSession(session, title)
  } catch (err) {
    showToast(err instanceof Error ? err.message : String(err))
  }
}

function waitPaint() {
  return new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
  })
}

async function finishPaneReady() {
  await nextTick()
  await waitPaint()
  termRef.value?.fitActive(true)
}

async function openSession(sessionId?: string, toolId?: ToolId) {
  const project = selectedProject.value
  if (!project) {
    newSessionOpen.value = true
    return
  }
  const tool = toolId ?? store.selectedTool
  if (store.sessionToolFilter !== tool) {
    await setSessionToolFilter(tool)
  }
  const session = sessionId ? store.sessions.find((item) => item.id === sessionId && item.toolId === tool) : undefined
  const target = resolveOpenTarget(store.live, {
    projectId: project.id,
    toolId: tool,
    sessionId: sessionId ?? null,
    sessionUpdatedAt: session?.updatedAt
  })
  if (target.action === 'switch' || target.action === 'bind-and-switch') {
    if (target.action === 'bind-and-switch') {
      await bindLiveSession(target.ptyId, target.sessionId, session?.title)
    }
    const existing = store.live.find((item) => item.ptyId === target.ptyId)
    if (!existing) return
    if (existing.ptyId === store.activePtyId && target.action === 'switch') return
    paneLoadingText.value = '正在切换会话'
    paneLoading.value = true
    try {
      rememberOpened(existing)
      await finishPaneReady()
    } finally {
      paneLoading.value = false
    }
    return
  }
  paneLoadingText.value = tool === 'dsh' ? '正在打开 DeepSeek Web' : '正在打开会话'
  paneLoading.value = true
  try {
    const opened = await api.ptyOpen({
      projectId: project.id,
      toolId: tool,
      sessionId: target.sessionId,
      title: session?.title ?? '新会话',
      cols: 120,
      rows: 32
    })
    store.selectedTool = tool
    rememberOpened({
      ptyId: opened.ptyId,
      key: opened.key,
      projectId: project.id,
      toolId: tool,
      sessionId: opened.sessionId ?? target.sessionId,
      title: opened.title,
      alive: true,
      openedAt: opened.openedAt ?? Date.now(),
      kind: opened.kind ?? (tool === 'dsh' ? 'web' : 'pty'),
      url: opened.url ?? null
    })
    if (!target.sessionId) watchPendingSession(opened.ptyId)
    await finishPaneReady()
  } catch (err) {
    showToast(err instanceof Error ? err.message : String(err))
  } finally {
    paneLoading.value = false
  }
}

function onAppContextMenu(event: MouseEvent) {
  const el = event.target as HTMLElement | null
  if (el?.closest('input, textarea, [contenteditable="true"]')) return
  event.preventDefault()
}

async function onEmptyStart(toolId?: ToolId) {
  if (!selectedProject.value) {
    if (!store.projects.length) {
      openAdd()
      return
    }
    newSessionOpen.value = true
    return
  }
  if (toolId) {
    await openSession(undefined, toolId)
    return
  }
  newSessionOpen.value = true
}

async function onCreateSession(payload: { projectId: string; toolId: ToolId }) {
  newSessionOpen.value = false
  store.selectedTool = payload.toolId
  if (store.selectedProjectId !== payload.projectId) {
    await selectProject(payload.projectId)
  }
  await openSession(undefined, payload.toolId)
}

async function onSaveSettings(settings: typeof store.settings) {
  try {
    await saveAppSettings(settings)
    settingsOpen.value = false
    showToast('设置已保存')
  } catch (err) {
    showToast(err instanceof Error ? err.message : String(err))
  } finally {
    settingsDialog.value?.stopSave()
  }
}

async function onCcswitchOpen() {
  if (ccswitchBusy.value) return
  ccswitchBusy.value = true
  try {
    if (!api.isTauri) {
      showToast('请在桌面端打开 CC Switch')
      ccswitchOpen.value = true
      return
    }
    const probe = await api.probeCcswitch()
    if (probe.found && probe.path) {
      await api.launchCcswitch(probe.path)
      showToast('已打开 CC Switch')
      return
    }
    showToast('未找到 CC Switch，请先安装或选择程序')
    ccswitchOpen.value = true
  } catch (err) {
    showToast(err instanceof Error ? err.message : String(err))
    ccswitchOpen.value = true
  } finally {
    ccswitchBusy.value = false
  }
}

function onCcswitchInstall() {
  ccswitchOpen.value = true
}

async function onVersionsChanged() {
  if (!api.isTauri) return
  store.probes = await api.probeTools()
  void refreshSessions()
}

async function onGrokSwitched() {
  noteGrokAccountChange()
  const grokLive = store.live.filter((item) => item.toolId === 'grokbuild')
  for (const item of grokLive) {
    await api.ptyKill(item.ptyId)
    termRef.value?.dispose(item.ptyId)
    markPtyExit(item.ptyId)
  }
  if (grokLive.length) {
    showToast('已切换 Grok 账号，并关掉旧终端')
  } else {
    showToast('已切换 Grok 账号')
  }
  if (store.selectedTool === 'grokbuild') {
    void refreshSessions()
  }
}

const confirmCopy = () => {
  if (confirmMode.value === 'remove') {
    const name = removingProject.value?.name ?? '该项目'
    return {
      title: '移除项目',
      body: `移除项目 ${name} 后，只是从列表拿掉，不会删除磁盘上的代码或 CLI 会话。`,
      action: '移除项目'
    }
  }
  if (confirmMode.value === 'delete') {
    const name = deletingSession.value?.title ?? '该会话'
    return {
      title: '删除会话',
      body: `删除「${name}」后，磁盘上的对话也会删掉，不能再打开。若终端正开着，会先关掉。`,
      action: '删除会话'
    }
  }
  if (confirmMode.value === 'close-all') {
    const count = liveOfProject(store.live, closingProjectId.value).length
    return {
      title: '关闭所有会话',
      body: `将关闭本项目下 ${count} 个已打开的终端。对话还在磁盘上，可以再点会话打开。`,
      action: '全部关闭'
    }
  }
  const closing = store.live.find((item) => item.ptyId === closingPtyId.value)
  if (isDshWeb(closing)) {
    return {
      title: '关闭会话',
      body: '关闭后 DeepSeek Web 会结束。对话还在磁盘上，可以再点会话打开。',
      action: '关闭'
    }
  }
  return {
    title: '关闭会话',
    body: '关闭后这个终端会结束。对话还在磁盘上，可以再点会话打开。',
    action: '关闭会话'
  }
}
</script>

<template>
  <div class="app" @contextmenu="onAppContextMenu">
    <WindowResizeFrame />
    <TitleBar
      @settings="settingsOpen = true"
      @versions="versionOpen = true"
      @accounts="accountOpen = true"
      @deepseek="deepseekOpen = true"
      @ccswitch-open="onCcswitchOpen"
      @ccswitch-install="onCcswitchInstall"
    />
    <div class="workspaces">
      <div v-show="store.appMode === 'console'" ref="consoleWs" class="workspace">
        <SideBar
          :collapsed="layout.sidebarCollapsed"
          v-model:renaming="renamingId"
          v-model:draft="renameDraft"
          @add="openAdd"
          @edit="openEdit"
          @remove="askRemove"
          @select="onSelectProject"
          @create="newSessionOpen = true"
          @open="(payload) => openSession(payload.sessionId, payload.toolId)"
          @retry="refreshSessions"
          @settings="settingsOpen = true"
          @start-rename="startRename"
          @rename="commitRename"
          @close="(payload) => askCloseSession(payload.sessionId, payload.toolId)"
          @close-all="askCloseAll"
          @delete="(payload) => askDeleteSession(payload.sessionId, payload.toolId)"
          @cite="openCiteTurns"
        />
        <PaneGutter
          label="调整工作区宽度"
          :can-toggle="false"
          @start="beginPaneDrag"
          @drag="onSidebarDrag"
          @end="endPaneDrag"
        />
        <div class="main">
          <LaunchStrip
            :loading="paneLoading"
            :loading-text="paneLoadingText"
            @close="askClose"
          />
          <div class="term-stack">
            <DshWebPane v-if="isDshWeb(activeLive)" :url="activeLive?.url || ''" />
            <TerminalPane
              v-else
              ref="termRef"
              :loading="paneLoading"
              :loading-text="paneLoadingText"
              :font-size="store.settings.terminalFontSize"
              @start="onEmptyStart"
            />
            <div
              class="doc-layer"
              :class="{ 'is-collapsed': layout.docRailCollapsed, 'is-static': paneDragging }"
              :style="{ width: docRailPaneWidth() + 'px' }"
              :aria-hidden="layout.docRailCollapsed"
            >
              <PaneGutter
                v-show="!layout.docRailCollapsed"
                label="调整文档栏宽度"
                @start="beginPaneDrag"
                @drag="onDocRailDrag"
                @end="endPaneDrag"
                @toggle="toggleDocRail"
              />
              <DocRail
                :collapsed="layout.docRailCollapsed"
                @open="openDocPreview"
                @cite="openCiteTurns"
              />
            </div>
          </div>
        </div>
      </div>
      <div v-show="store.appMode === 'bridge'" ref="bridgeWs" class="workspace">
        <BridgeSideBar
          :collapsed="layout.sidebarCollapsed"
          @add="openAdd"
          @edit="openEdit"
          @remove="askRemove"
          @select="onSelectBridgeProject"
        />
        <PaneGutter
          label="调整工作区宽度"
          :can-toggle="false"
          @start="beginPaneDrag"
          @drag="onSidebarDrag"
          @end="endPaneDrag"
        />
        <BridgePane />
      </div>
    </div>
    <StatusBar
      :docs-open="!layout.docRailCollapsed"
      @settings="settingsOpen = true"
      @refresh="store.appMode === 'bridge' ? refreshCodexThreads() : refreshSessions()"
      @docs="toggleDocRail"
    />

    <NewSessionDialog
      :open="newSessionOpen"
      @close="newSessionOpen = false"
      @create="onCreateSession"
      @add-project="onAddProjectFromSession"
    />
    <ProjectDialog
      ref="projectDialog"
      :visible="projectOpen"
      :project="editing ? selectedProject : null"
      :default-proxy-url="store.settings.defaultProxyUrl"
      @close="projectOpen = false; reopenNewSession = false"
      @save="onSaveProject"
    />
    <VersionDrawer
      :open="versionOpen"
      @close="versionOpen = false"
      @changed="onVersionsChanged"
    />
    <CcswitchDialog
      :open="ccswitchOpen"
      @close="ccswitchOpen = false"
      @launched="showToast('已打开 CC Switch')"
    />
    <GrokAccountDialog
      :open="accountOpen"
      @close="accountOpen = false"
      @switched="onGrokSwitched"
    />
    <DeepseekKeyDialog :open="deepseekOpen" @close="deepseekOpen = false" />
    <SettingsDrawer
      ref="settingsDialog"
      :open="settingsOpen"
      :settings="store.settings"
      @close="settingsOpen = false"
      @save="onSaveSettings"
    />
    <MarkdownPreviewDialog
      :open="previewOpen"
      :project-id="store.selectedProjectId"
      :doc="previewDoc"
      :tool-id="store.focusedSession?.toolId ?? store.selectedTool"
      :session-title="store.focusedSession?.title ?? ''"
      @close="previewOpen = false"
    />
    <CiteTurnDialog
      :open="citeOpen"
      :project-id="store.selectedProjectId"
      :tool-id="citeSession?.toolId ?? 'grokbuild'"
      :session-id="citeSession?.sessionId ?? ''"
      :session-title="citeSession?.title ?? ''"
      @close="citeOpen = false"
    />
    <ConfirmDialog
      :open="confirmOpen"
      :title="confirmCopy().title"
      :body="confirmCopy().body"
      :action="confirmCopy().action"
      danger
      @close="confirmOpen = false"
      @confirm="onConfirm"
    />
    <div v-if="store.toast" class="toast" role="status">{{ store.toast }}</div>
  </div>
</template>

<style scoped>
.app {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: transparent;
}

.workspaces {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.workspace {
  flex: 1;
  min-height: 0;
  display: flex;
  overflow: hidden;
}

.main {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--ad-editor);
}

.term-stack {
  position: relative;
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.doc-layer {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  z-index: 4;
  display: flex;
  overflow: hidden;
  background: var(--ad-sidebar);
  border-left: 1px solid var(--ad-border);
  box-shadow: -8px 0 24px rgba(0, 0, 0, 0.4);
  transition: width var(--ad-pane-move);
}

.doc-layer.is-collapsed {
  border-left: none;
  box-shadow: none;
  pointer-events: none;
}

.doc-layer.is-static {
  transition: none;
}

.toast {
  position: fixed;
  top: 52px;
  left: 50%;
  transform: translateX(-50%);
  background: var(--ad-raised);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: var(--ad-radius-pill);
  box-shadow: var(--ad-shadow-menu);
  padding: 8px 16px;
  font-size: 13px;
  z-index: 30;
}
</style>
