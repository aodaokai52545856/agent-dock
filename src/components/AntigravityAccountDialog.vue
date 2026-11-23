<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import * as api from '../lib/api'
import { formatPercent } from '../lib/grokUsage'
import { modal as vModal } from '../lib/modal'
import { store } from '../lib/store'
import type { AntigravityManagerAccount, AntigravityManagerState } from '../lib/types'
import ToolMark from './ToolMark.vue'

const MANAGER_RELEASES_URL = 'https://github.com/lbjlaq/Antigravity-Manager/releases/latest'
const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ close: [] }>()
const data = ref<AntigravityManagerState | null>(null)
const selectedId = ref('')
const query = ref('')
const busy = ref<'' | 'load' | 'switch' | 'launch' | 'install'>('')
const error = ref('')
const notice = ref('')

const accounts = computed(() => data.value?.accounts ?? [])
const current = computed(() => accounts.value.find(account => account.id === data.value?.currentAccountId))
const selected = computed(() => accounts.value.find(account => account.id === selectedId.value))
const filtered = computed(() => {
  const term = query.value.trim().toLowerCase()
  return accounts.value.filter(account => [account.email, account.name].some(value => value?.toLowerCase().includes(term)))
})
const activeTerminal = computed(() => store.live.some(item => item.toolId === 'antigravity' && item.alive !== false))
const switchBlocked = computed(() => {
  if (selected.value?.disabled) return '此账号在 Antigravity Tools 中已停用。'
  if (activeTerminal.value) return '请先关闭 Dock 中运行的 Antigravity 终端。'
  if (!data.value?.canSwitch) return data.value?.switchHint || '请在 Antigravity Tools 中切换账号。'
  return ''
})

const resetTimeFormat = new Intl.DateTimeFormat(undefined, {
  month: 'numeric',
  day: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
})

function formatQuotaTime(value?: string | number | null) {
  if (!value) return '—'
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? '—' : resetTimeFormat.format(date)
}

function accountLabel(account: AntigravityManagerAccount) {
  return account.name || account.email
}

function accept(next: AntigravityManagerState) {
  data.value = next
  if (!next.accounts.some(account => account.id === selectedId.value)) {
    selectedId.value = current.value?.id || next.accounts[0]?.id || ''
  }
}

async function run(operation: typeof busy.value, task: () => Promise<void>) {
  if (busy.value) return
  busy.value = operation
  error.value = ''
  notice.value = ''
  try {
    await task()
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    busy.value = ''
  }
}

function reload() {
  return run('load', async () => accept(await api.listAntigravityManagerAccounts()))
}

watch(() => props.open, open => {
  if (!open) return
  query.value = ''
  void reload()
}, { immediate: true })

function close() {
  if (!busy.value) emit('close')
}

function switchAccount(account: AntigravityManagerAccount) {
  if (switchBlocked.value || account.id === current.value?.id) return
  void run('switch', async () => {
    accept(await api.switchAntigravityManagerAccount(account.id))
    notice.value = account.id === current.value?.id ? '已切换 Antigravity CLI 账号。' : '请刷新以确认当前账号。'
  })
}

function openManager() {
  void run('launch', () => api.launchAntigravityManager())
}

function openInstaller() {
  void run('install', () => api.openExternal(MANAGER_RELEASES_URL))
}
</script>

<template>
  <div v-if="open" class="ad-mask" @click.self="close">
    <section
      v-modal="close"
      class="ad-dialog account-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="antigravity-accounts-title"
      tabindex="-1"
      :aria-busy="!!busy"
    >
      <header class="dialog-head">
        <div class="heading">
          <span class="tool-mark" aria-hidden="true"><ToolMark id="antigravity" /></span>
          <div>
            <h2 id="antigravity-accounts-title">Antigravity 账号</h2>
            <p class="muted">复用本机 Antigravity Tools 的账号与配额。切换用于后续启动的 agy 会话。</p>
          </div>
        </div>
        <div class="actions">
          <button v-if="data?.managerPath" type="button" class="btn btn-ghost btn-small" :disabled="!!busy" @click="openManager">
            {{ busy === 'launch' ? '打开中…' : '打开 Antigravity Tools' }}
          </button>
          <button v-else-if="data" type="button" class="btn btn-ghost btn-small" :disabled="!!busy" @click="openInstaller">
            下载安装 ↗
          </button>
          <button type="button" class="btn btn-ghost btn-small" :disabled="!!busy" @click="close">关闭</button>
        </div>
      </header>

      <div class="summary">
        <span>当前 CLI 账号 <strong :title="current?.email">{{ current?.email || '未识别' }}</strong></span>
        <span class="total">{{ accounts.length }} 个账号</span>
      </div>

      <p v-if="error" class="banner banner-error" role="alert">{{ error }}</p>
      <p v-if="notice" class="banner banner-success" role="status">{{ notice }}</p>

      <div class="workspace">
        <aside class="account-sidebar" aria-label="本机账号">
          <div class="search">
            <input v-model="query" type="search" aria-label="搜索 Antigravity 账号" placeholder="搜索名称或邮箱" :disabled="!accounts.length" />
          </div>
          <div class="account-list">
            <p v-if="!data && busy === 'load'" class="list-empty muted" role="status">正在读取本机账号…</p>
            <p v-else-if="!error && !filtered.length" class="list-empty muted">{{ query ? '未找到匹配的账号' : '暂无本机账号' }}</p>
            <button
              v-for="account in filtered"
              :key="account.id"
              class="account-row"
              type="button"
              :aria-pressed="selectedId === account.id"
              :disabled="!!busy"
              @click="selectedId = account.id"
            >
              <span class="account-name" :title="accountLabel(account)">{{ accountLabel(account) }}</span>
              <span v-if="account.email !== accountLabel(account)" class="account-email muted" :title="account.email">{{ account.email }}</span>
              <span v-if="account.id === current?.id || account.disabled" class="pill" :class="{ current: account.id === current?.id }">
                {{ account.id === current?.id ? '当前 CLI 账号' : '已停用' }}
              </span>
            </button>
          </div>
        </aside>

        <section class="account-detail" aria-label="账号详情">
          <template v-if="selected">
            <div class="detail-head">
              <div class="identity">
                <h3>{{ accountLabel(selected) }}</h3>
                <p v-if="selected.email !== accountLabel(selected)" class="muted">{{ selected.email }}</p>
                <div class="badges">
                  <span v-if="selected.subscription" class="pill">{{ selected.subscription }}</span>
                  <span v-if="selected.disabled" class="pill">已停用</span>
                </div>
                <p v-if="selected.lastUsed" class="muted">上次使用 {{ formatQuotaTime(selected.lastUsed) }}</p>
              </div>
              <button
                type="button"
                class="btn btn-primary"
                :disabled="!!busy || !!switchBlocked || selected.id === current?.id"
                :title="switchBlocked || undefined"
                @click="switchAccount(selected)"
              >
                {{ busy === 'switch' ? '切换中…' : selected.id === current?.id ? '当前 CLI 账号' : '切换 CLI 账号' }}
              </button>
            </div>
            <p v-if="switchBlocked && selected.id !== current?.id" class="muted">{{ switchBlocked }}</p>

            <div class="quota">
              <div class="quota-heading">
                <h4>模型配额</h4>
                <span v-if="selected.quotaUpdatedAt" class="muted">更新于 {{ formatQuotaTime(selected.quotaUpdatedAt) }}</span>
              </div>
              <div v-if="selected.quotas.length" class="quota-table-wrap">
                <table>
                  <thead><tr><th scope="col">模型</th><th scope="col">剩余</th><th scope="col">重置时间</th></tr></thead>
                  <tbody>
                    <tr v-for="(quota, index) in selected.quotas" :key="index">
                      <td class="quota-name">{{ quota.name }}</td>
                      <td>
                        <div class="quota-value">
                          <progress :aria-label="`${quota.name} 剩余配额`" :value="quota.percentage" max="100" />
                          <span>{{ formatPercent(quota.percentage) }}%</span>
                        </div>
                      </td>
                      <td class="reset-time" :title="quota.resetTime || undefined">{{ formatQuotaTime(quota.resetTime) }}</td>
                    </tr>
                  </tbody>
                </table>
              </div>
              <p v-else class="quota-empty muted">暂无配额记录。可在 Antigravity Tools 中同步后再刷新。</p>
            </div>
          </template>
          <div v-else-if="data && !accounts.length" class="detail-empty">
            <h3>{{ api.isTauri ? '暂无已保存的账号' : '请在桌面端读取本机账号' }}</h3>
            <p class="muted">
              {{ data.managerPath ? '在 Antigravity Tools 登录后，刷新此列表即可查看账号。' : '安装 Antigravity Tools 并登录后，即可在此查看账号。' }}
            </p>
          </div>
        </section>
      </div>

      <footer>
        <span class="data-path muted" :title="data?.dataDir || undefined">{{ data?.dataDir || 'Antigravity Tools 本机数据' }}</span>
        <button type="button" class="btn btn-ghost btn-small" :disabled="!!busy || !api.isTauri" @click="reload">
          {{ busy === 'load' ? '刷新中…' : '刷新本地数据' }}
        </button>
      </footer>
    </section>
  </div>
</template>

<style scoped>
.account-dialog {
  display: flex;
  flex-direction: column;
  width: 880px;
  height: 604px;
  padding: 0;
  overflow: hidden;
}

.dialog-head, .summary, footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 16px 24px;
  flex-shrink: 0;
}

.heading, .actions, .badges {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.tool-mark {
  display: block;
  width: 40px;
  height: 40px;
  flex-shrink: 0;
}

h2, h3, h4, p { margin: 0; }
h2 { font-size: 20px; line-height: 28px; }
h3 { font-size: 16px; line-height: 24px; }
h4 { font-size: 13px; line-height: 20px; }
.muted { font-size: 12px; line-height: 20px; }

.summary {
  padding-block: 8px;
  border-block: 1px solid var(--ad-border);
  background: var(--ad-float-inset);
  color: var(--ad-muted);
  font-size: 12px;
}

.summary > span:first-child {
  display: flex;
  gap: 8px;
  min-width: 0;
}

.summary strong { color: var(--ad-text); }
.total { white-space: nowrap; }

.banner {
  padding: 8px 24px;
  border-bottom: 1px solid var(--ad-border);
  font-size: 13px;
  line-height: 20px;
  overflow-wrap: anywhere;
}

.banner-error { color: var(--ad-error); }
.banner-success { color: var(--ad-success); }

.workspace {
  display: grid;
  grid-template-columns: 256px minmax(0, 1fr);
  flex: 1;
  min-height: 0;
}

.account-sidebar {
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-right: 1px solid var(--ad-border);
  background: var(--ad-float-inset);
}

.search { padding: 16px 16px 8px; }
.search input { width: 100%; height: 32px; }
.account-list { padding: 0 8px 8px; overflow-y: auto; }
.list-empty { padding: 8px; }

.account-row {
  display: grid;
  justify-items: start;
  gap: 4px;
  width: 100%;
  padding: 8px 12px;
  border-left: 3px solid transparent;
  border-radius: 4px;
  text-align: left;
}

.account-row:hover { background: var(--ad-hover); }
.account-row[aria-pressed='true'] {
  background: var(--ad-selected);
  border-color: var(--ad-antigravity);
}

.account-name, .account-email, .data-path, .summary strong {
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.account-name { font-size: 13px; font-weight: 600; }

.pill {
  width: fit-content;
  color: var(--ad-muted);
  background: var(--ad-hover);
  border-radius: 4px;
  padding: 0 4px;
  font-size: 12px;
  line-height: 20px;
}

.pill.current { color: var(--ad-antigravity); }

.account-detail {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px 24px;
  min-width: 0;
  min-height: 0;
  overflow-y: auto;
}

.detail-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; }
.detail-head > .btn { flex-shrink: 0; }
.identity { min-width: 0; overflow-wrap: anywhere; }
.badges { margin-top: 4px; }

.quota { min-width: 0; }
.quota-heading { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 8px; margin-bottom: 8px; }
.quota-table-wrap { overflow-x: auto; border: 1px solid var(--ad-border); border-radius: 6px; }
.quota table { width: 100%; border-collapse: collapse; font-size: 12px; }
.quota th, .quota td { padding: 8px 12px; }
.quota th { background: var(--ad-float-inset); color: var(--ad-muted); text-align: left; }
.quota th:not(:first-child), .quota td:not(:first-child) { text-align: right; }
.quota td { border-top: 1px solid var(--ad-border); }
.quota tbody tr:hover { background: var(--ad-hover); }
.quota-name { overflow-wrap: anywhere; }
.quota-value { display: flex; justify-content: flex-end; align-items: center; gap: 8px; }
.quota-value progress { width: 52px; height: 6px; accent-color: var(--ad-antigravity); }
.quota-value span { min-width: 36px; font-variant-numeric: tabular-nums; }
.reset-time { white-space: nowrap; font-variant-numeric: tabular-nums; }
.quota-empty, .detail-empty { padding: 16px; background: var(--ad-float-inset); border-radius: 6px; }

footer { padding-block: 8px; border-top: 1px solid var(--ad-border); }
footer .btn { flex-shrink: 0; }

@media (max-width: 760px) {
  .workspace { grid-template-columns: minmax(132px, 35%) minmax(0, 1fr); }
  .dialog-head, .summary, .account-detail, footer { padding-inline: 12px; }
  .actions { flex-wrap: wrap; justify-content: flex-end; }
  .detail-head { flex-wrap: wrap; }
  .quota-value progress { display: none; }
}
</style>
