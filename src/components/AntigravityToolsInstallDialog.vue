<script setup lang="ts">
import { listen } from '@tauri-apps/api/event'
import { computed, ref, watch } from 'vue'
import * as api from '../lib/api'
import { formatBytes, formatProgress } from '../lib/ccswitch'
import { modal as vModal } from '../lib/modal'
import { store } from '../lib/store'
import type { AntigravityToolsInstallResult, CcswitchLatest, CcswitchProgress } from '../lib/types'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ close: [] }>()

const releasesUrl = 'https://github.com/lbjlaq/Antigravity-Manager/releases/latest'
const latest = ref<CcswitchLatest | null>(null)
const proxyUrl = ref('')
const loading = ref(false)
const installing = ref(false)
const opening = ref(false)
const progress = ref<CcswitchProgress | null>(null)
const result = ref<AntigravityToolsInstallResult | null>(null)
const error = ref('')

const progressLabel = computed(() => {
  if (!progress.value) return ''
  const { message, received, total } = progress.value
  const amount = formatProgress(received, total)
  return amount ? `${message} · ${amount}` : message
})

watch(() => props.open, (open) => {
  if (!open) return
  proxyUrl.value = store.settings.defaultProxyUrl || ''
  result.value = null
  error.value = ''
  void refreshLatest()
})

async function refreshLatest() {
  if (!api.isTauri || loading.value) return
  loading.value = true
  latest.value = null
  error.value = ''
  try {
    latest.value = await api.antigravityToolsLatest(proxyUrl.value.trim() || undefined)
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    loading.value = false
  }
}

async function install() {
  if (!latest.value || installing.value) return
  installing.value = true
  progress.value = null
  result.value = null
  error.value = ''
  let unlisten: (() => void) | undefined
  try {
    unlisten = await listen<CcswitchProgress>('antigravity-tools-install-progress', ({ payload }) => {
      progress.value = payload
    })
    result.value = await api.installAntigravityTools(proxyUrl.value.trim() || undefined)
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    unlisten?.()
    installing.value = false
  }
}

async function openReleases() {
  try {
    await api.openExternal(releasesUrl)
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  }
}

async function openInstalled() {
  if (opening.value) return
  opening.value = true
  error.value = ''
  try {
    await api.launchAntigravityManager()
    emit('close')
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    opening.value = false
  }
}

function close() {
  if (!installing.value && !opening.value) emit('close')
}
</script>

<template>
  <div v-if="open" class="ad-mask" @click.self="close">
    <div v-modal="close" class="ad-dialog dialog" role="dialog" aria-modal="true" aria-labelledby="agy-tools-install-title">
      <header class="dialog-header">
        <div>
          <h2 id="agy-tools-install-title">下载安装 Antigravity Tools</h2>
          <p>从官方 GitHub 下载适用于当前系统的版本。</p>
        </div>
        <button type="button" class="btn btn-ghost btn-small" :disabled="installing || opening" @click="close">关闭</button>
      </header>

      <dl v-if="api.isTauri" class="release">
        <div>
          <dt>最新版本</dt>
          <dd>{{ latest?.version || (loading ? '查询中…' : '—') }}</dd>
        </div>
        <div>
          <dt>安装包</dt>
          <dd :title="latest?.assetName">{{ latest?.assetName || '—' }}</dd>
        </div>
        <div>
          <dt>大小</dt>
          <dd>{{ latest ? formatBytes(latest.size) : '—' }}</dd>
        </div>
      </dl>

      <label v-if="api.isTauri" class="proxy">
        下载代理
        <input v-model="proxyUrl" :disabled="loading || installing" placeholder="留空直连，例如 http://127.0.0.1:7890" />
      </label>
      <p v-else class="help">桌面端支持下载安装，也可前往官方发布页手动下载。</p>

      <div v-if="installing" class="status" role="status">
        {{ progressLabel || '正在准备下载…' }}
        <progress v-if="progress?.stage === 'download' && progress.total > 0" :value="progress.received" :max="progress.total" aria-label="下载进度" />
      </div>
      <div v-if="result" class="status" role="status">
        <p class="install-message">{{ result.log }}</p>
        <p class="download-path" :title="result.path">{{ result.path }}</p>
      </div>
      <p v-if="error" class="field-error" role="alert">{{ error }}</p>

      <footer class="actions">
        <button type="button" class="btn btn-ghost" @click="openReleases">官方发布页</button>
        <div v-if="api.isTauri" class="install-actions">
          <button type="button" class="btn btn-ghost" :disabled="loading || installing || opening" @click="refreshLatest">刷新版本</button>
          <button v-if="result && !result.launched" type="button" class="btn btn-primary" :disabled="loading || opening" @click="openInstalled">
            {{ opening ? '打开中…' : '打开本机应用' }}
          </button>
          <button v-else type="button" class="btn btn-primary" :disabled="loading || installing || !latest" @click="install">
            {{ installing ? '正在下载安装…' : '下载并安装' }}
          </button>
        </div>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.dialog {
  width: 560px;
  background: var(--ad-float-solid);
  backdrop-filter: none;
  overflow-y: auto;
}

.dialog-header, .actions, .install-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.dialog-header, .actions {
  justify-content: space-between;
  row-gap: 12px;
}

.dialog-header h2 {
  margin: 0;
  font-size: 16px;
}

.dialog-header p, .help {
  margin: 4px 0 0;
  color: var(--ad-muted);
  font-size: 12px;
  line-height: 1.6;
}

.release {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  gap: 16px;
  margin: 16px 0;
  padding: 12px;
  border: 1px solid var(--ad-border);
  border-radius: 8px;
}

.release dt, .download-path {
  font-size: 12px;
  color: var(--ad-muted);
}

.release dd {
  margin: 4px 0 0;
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.proxy {
  display: grid;
  gap: 6px;
  font-size: 13px;
}

.proxy input {
  width: 100%;
  height: 32px;
  padding: 0 10px;
}

.status, .field-error {
  margin: 12px 0 0;
  font-size: 13px;
  line-height: 1.6;
}

.status progress {
  display: block;
  width: 100%;
  height: 8px;
  margin-top: 6px;
}

.download-path {
  margin: 4px 0 0;
  overflow-wrap: anywhere;
}

.install-message {
  margin: 0;
}

.actions {
  margin-top: 16px;
}
</style>
