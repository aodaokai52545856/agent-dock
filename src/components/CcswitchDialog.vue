<script setup lang="ts">
import { open as pickPath } from '@tauri-apps/plugin-dialog'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { computed, onUnmounted, reactive, ref, watch } from 'vue'
import * as api from '../lib/api'
import { CCSWITCH_HOMEPAGE, CCSWITCH_RELEASES, formatBytes, formatProgress } from '../lib/ccswitch'
import { applyState, store } from '../lib/store'
import type { CcswitchLatest, CcswitchProgress } from '../lib/types'

const props = defineProps<{
  open: boolean
}>()

const emit = defineEmits<{
  close: []
  launched: []
}>()

const latest = ref<CcswitchLatest | null>(null)
const loadingLatest = ref(false)
const installing = ref(false)
const opening = ref(false)
const found = ref(false)
const installedPath = ref('')
const error = ref('')
const progress = ref<CcswitchProgress | null>(null)
const log = ref('')
let offProgress: UnlistenFn | undefined

const form = reactive({
  downloadDir: '',
  installDir: '',
  proxyUrl: ''
})

const busy = computed(() => loadingLatest.value || installing.value || opening.value)
const lead = computed(() =>
  found.value
    ? '已找到本机安装，可直接打开。也可以重新下载便携版到指定目录。'
    : '未检测到本机安装。可下载 GitHub 最新便携版，并解压到指定目录。'
)
const sizeLabel = computed(() => (latest.value ? formatBytes(latest.value.size) : '—'))
const progressLabel = computed(() => {
  if (!progress.value) return ''
  const extra = formatProgress(progress.value.received, progress.value.total)
  return extra ? `${progress.value.message} · ${extra}` : progress.value.message
})

watch(
  () => props.open,
  (open) => {
    if (!open) {
      installing.value = false
      return
    }
    error.value = ''
    log.value = ''
    progress.value = null
    latest.value = null
    found.value = false
    installedPath.value = ''
    opening.value = false
    form.proxyUrl = store.settings.defaultProxyUrl || ''
    void hydrate()
    void bindProgress()
  }
)

onUnmounted(() => {
  offProgress?.()
})

async function bindProgress() {
  offProgress?.()
  if (!api.isTauri) return
  offProgress = await listen<CcswitchProgress>('ccswitch-progress', (event) => {
    progress.value = event.payload
  })
}

async function hydrate() {
  try {
    const probe = await api.probeCcswitch()
    found.value = !!probe.found && !!probe.path
    installedPath.value = probe.path || ''
    form.downloadDir = probe.downloadDir || form.downloadDir
    form.installDir = probe.installDir || form.installDir
  } catch {
    found.value = false
    installedPath.value = ''
  }
  await refreshLatest()
}

async function openInstalled() {
  if (!api.isTauri) {
    error.value = '请在桌面端打开 CC Switch'
    return
  }
  opening.value = true
  error.value = ''
  try {
    let path = installedPath.value.trim()
    if (!path) {
      const probe = await api.probeCcswitch()
      path = probe.path || ''
      found.value = !!probe.found && !!path
      installedPath.value = path
    }
    if (!path) {
      error.value = '未找到 CC Switch，请先安装或选择已安装程序'
      return
    }
    await api.launchCcswitch(path)
    emit('launched')
    emit('close')
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    opening.value = false
  }
}

async function refreshLatest() {
  loadingLatest.value = true
  error.value = ''
  try {
    latest.value = await api.ccswitchLatest(form.proxyUrl.trim() || undefined)
  } catch (err) {
    latest.value = null
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    loadingLatest.value = false
  }
}

async function pickDir(field: 'downloadDir' | 'installDir') {
  if (!api.isTauri) {
    error.value = '请在桌面窗口里选择文件夹'
    return
  }
  try {
    const selected = await pickPath({
      directory: true,
      multiple: false,
      title: field === 'downloadDir' ? '选择下载位置' : '选择安装位置',
      defaultPath: form[field] || undefined
    })
    if (typeof selected !== 'string' || !selected) return
    form[field] = selected
    error.value = ''
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  }
}

async function pickExisting() {
  if (!api.isTauri) {
    error.value = '请在桌面窗口里选择已安装的程序'
    return
  }
  try {
    const selected = await pickPath({
      directory: false,
      multiple: false,
      title: '选择 CC Switch',
      filters: [{ name: 'CC Switch', extensions: ['exe', 'app', 'AppImage'] }]
    })
    if (typeof selected !== 'string' || !selected) return
    applyState(await api.rememberCcswitchPath(selected))
    await api.launchCcswitch(selected)
    emit('launched')
    emit('close')
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  }
}

async function install() {
  if (!form.downloadDir.trim()) {
    error.value = '请选择下载位置'
    return
  }
  if (!form.installDir.trim()) {
    error.value = '请选择安装位置'
    return
  }
  installing.value = true
  error.value = ''
  log.value = ''
  progress.value = { stage: 'fetch', message: '正在查询最新版', received: 0, total: 0 }
  try {
    const result = await api.installCcswitch({
      downloadDir: form.downloadDir.trim(),
      installDir: form.installDir.trim(),
      proxyUrl: form.proxyUrl.trim() || undefined
    })
    log.value = result.log
    if (!result.ok) {
      error.value = result.log || '安装失败'
      return
    }
    applyState(await api.loadAppState())
    await api.launchCcswitch(result.path)
    emit('launched')
    emit('close')
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    installing.value = false
  }
}

function openHome() {
  void api.openExternal(latest.value?.homepage || CCSWITCH_HOMEPAGE)
}

function openReleases() {
  void api.openExternal(latest.value?.releasesUrl || CCSWITCH_RELEASES)
}

function close() {
  if (installing.value || opening.value) return
  emit('close')
}
</script>

<template>
  <div v-if="open" class="ad-mask" @click.self="close">
    <div class="ad-dialog dialog" role="dialog" aria-modal="true" aria-label="CC Switch">
      <header>
        <div>
          <h2>CC Switch</h2>
          <p class="lead">{{ lead }}</p>
        </div>
        <button type="button" class="btn btn-ghost btn-small" :disabled="installing" @click="close">关闭</button>
      </header>

      <p v-if="installedPath" class="path" :title="installedPath">{{ installedPath }}</p>

      <div class="meta">
        <div>
          <span>最新版本</span>
          <strong>{{ latest?.version || (loadingLatest ? '查询中…' : '—') }}</strong>
        </div>
        <div>
          <span>安装包</span>
          <strong :title="latest?.assetName">{{ latest?.assetName || '—' }}</strong>
        </div>
        <div>
          <span>大小</span>
          <strong>{{ latest ? sizeLabel : '—' }}</strong>
        </div>
      </div>

      <label class="field">
        <span>下载位置</span>
        <div class="path-row">
          <input v-model="form.downloadDir" :disabled="busy" placeholder="安装包保存到这个文件夹" />
          <button type="button" class="btn btn-ghost" :disabled="busy" @click="pickDir('downloadDir')">浏览</button>
        </div>
      </label>
      <label class="field">
        <span>安装位置</span>
        <div class="path-row">
          <input v-model="form.installDir" :disabled="busy" placeholder="解压或复制到这个文件夹" />
          <button type="button" class="btn btn-ghost" :disabled="busy" @click="pickDir('installDir')">浏览</button>
        </div>
      </label>
      <label class="field">
        <span>下载代理</span>
        <input
          v-model="form.proxyUrl"
          :disabled="busy"
          placeholder="留空则直连，例如 http://127.0.0.1:7890"
        />
        <p class="help">走 GitHub Releases。网络不通时填写代理，或打开官方页面手动下载。</p>
      </label>

      <p v-if="!api.isTauri" class="help">浏览器预览无法下载安装，请在桌面端使用。</p>
      <p v-if="progressLabel && installing" class="status">{{ progressLabel }}</p>
      <p v-if="error" class="field-error">{{ error }}</p>
      <pre v-if="log && error" class="log">{{ log }}</pre>

      <div class="actions">
        <button type="button" class="btn btn-ghost" :disabled="busy" @click="pickExisting">选择已安装程序</button>
        <span class="spacer" />
        <button type="button" class="btn btn-ghost" :disabled="busy" @click="openHome">官网</button>
        <button type="button" class="btn btn-ghost" :disabled="busy" @click="openReleases">Releases</button>
        <button type="button" class="btn btn-ghost" :disabled="busy" @click="refreshLatest">
          {{ loadingLatest ? '查询中…' : '刷新版本' }}
        </button>
        <button type="button" class="btn btn-ghost" :disabled="busy || !latest" @click="install">
          {{ installing ? '安装中…' : '下载并安装' }}
        </button>
        <button
          type="button"
          class="btn btn-primary"
          :disabled="busy"
          @click="openInstalled"
        >
          {{ opening ? '打开中…' : '打开' }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dialog {
  width: 640px;
  max-width: calc(100vw - 48px);
  max-height: calc(100vh - 48px);
  overflow: auto;
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

.lead {
  margin: 4px 0 0;
  color: var(--ad-muted);
  font-size: 12px;
  line-height: 20px;
}

.path {
  margin: 0 0 12px;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.meta {
  display: grid;
  grid-template-columns: 120px minmax(0, 1fr) 88px;
  gap: 12px;
  margin-bottom: 16px;
  padding: 12px;
  border: 1px solid var(--ad-border);
  border-radius: 8px;
  background: var(--ad-harbor);
}

.meta span {
  display: block;
  font-size: 12px;
  line-height: 18px;
  color: var(--ad-muted);
}

.meta strong {
  display: block;
  margin-top: 2px;
  font-size: 13px;
  line-height: 20px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 16px;
  font-size: 13px;
  color: var(--ad-muted);
}

.field input {
  height: 32px;
  padding: 0 10px;
}

.path-row {
  display: flex;
  gap: 8px;
}

.path-row input {
  flex: 1;
}

.help {
  margin: 0;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-muted);
}

.status {
  margin: 0 0 8px;
  font-size: 12px;
  line-height: 20px;
  color: var(--ad-text);
}

.log {
  margin: 0 0 12px;
  max-height: 120px;
  overflow: auto;
  padding: 8px 10px;
  font-size: 12px;
  line-height: 18px;
  white-space: pre-wrap;
  background: var(--ad-harbor);
  border: 1px solid var(--ad-border);
  border-radius: 6px;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
}

.spacer {
  flex: 1;
}
</style>
