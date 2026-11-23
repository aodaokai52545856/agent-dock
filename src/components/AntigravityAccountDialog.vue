<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import * as api from '../lib/api'
import { formatPercent, usageTone } from '../lib/grokUsage'
import { modal as vModal } from '../lib/modal'
import { store } from '../lib/store'
import type { AntigravityManagerAccount, AntigravityManagerQuota, AntigravityManagerState } from '../lib/types'
import ToolMark from './ToolMark.vue'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ close: []; install: [] }>()
const data = ref<AntigravityManagerState | null>(null)
const selectedId = ref('')
const query = ref('')
const busy = ref<'' | 'load' | 'switch' | 'launch'>('')
const error = ref('')
const notice = ref('')

const accounts = computed(() => data.value?.accounts ?? [])
const current = computed(() => accounts.value.find(account => account.id === data.value?.currentAccountId))
const selected = computed(() => accounts.value.find(account => account.id === selectedId.value))
const selectedIsCurrent = computed(() => !!selected.value && selected.value.id === current.value?.id)
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

const quotaGroups = computed(() => {
  const groups = new Map<string, { label: string; quotas: AntigravityManagerQuota[] }>()
  for (const quota of selected.value?.quotas ?? []) {
    const key = quota.groupId || quota.groupName || ''
    const existing = groups.get(key)
    if (existing) {
      existing.quotas.push(quota)
    } else {
      groups.set(key, { label: quotaGroupLabel(quota), quotas: [quota] })
    }
  }
  return Array.from(groups, ([key, group]) => ({ key, ...group }))
})

const subscriptions = new Map([
  ['FREE', '免费版'],
  ['PRO', '专业版'],
  ['ULTRA', '旗舰版'],
  ['ENTERPRISE', '企业版'],
  ['TEAM', '团队版'],
  ['STANDARD', '标准版'],
  ['BASIC', '基础版'],
  ['UNKNOWN', '未知套餐'],
])
const quotaGroupNames = new Map([
  ['gemini', 'Gemini 模型'],
  ['gemini models', 'Gemini 模型'],
  ['3p', 'Claude 与 GPT 模型'],
  ['claude and gpt models', 'Claude 与 GPT 模型'],
  ['gemini-argon-wtu', 'Gemini 4 Argon'],
])
const quotaWindows = new Map([
  ['5h', '每 5 小时'],
  ['five hour', '每 5 小时'],
  ['daily', '每日'],
  ['weekly', '每周'],
  ['monthly', '每月'],
])
const quotaTime = new Intl.DateTimeFormat('zh-CN', {
  month: '2-digit',
  day: '2-digit',
  hour: '2-digit',
  minute: '2-digit',
  hourCycle: 'h23',
})

function formatQuotaTime(value?: string | number | null) {
  if (!value) return '—'
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? '—' : quotaTime.format(date)
}

function accountLabel(account: AntigravityManagerAccount) {
  return account.name || account.email
}

function subscriptionLabel(tier: string) {
  return subscriptions.get(tier.trim().toUpperCase()) || tier
}

function quotaGroupLabel(quota: AntigravityManagerQuota) {
  const name = quota.groupName?.trim()
  if (name) return quotaGroupNames.get(name.toLowerCase()) || name
  const id = quota.groupId?.trim() || ''
  return quotaGroupNames.get(id.toLowerCase()) || id
}

function quotaPeriodLabel(value: string) {
  const key = value.trim().toLowerCase().replaceAll('-', ' ')
  const known = quotaWindows.get(key)
  if (known) return known
  const duration = /^(\d+)\s*(h(?:ours?)?|d(?:ays?)?)$/.exec(key)
  return duration ? `每 ${duration[1]} ${duration[2].startsWith('h') ? '小时' : '天'}` : ''
}

function quotaRowLabel(quota: AntigravityManagerQuota) {
  const window = quotaPeriodLabel(quota.window || '')
  if (window) return window
  const match = /^(?:(.*?)\s*(?:·|\s))?(five[- ]hour|\d+[- ]?(?:h(?:ours?)?|d(?:ays?)?)|daily|weekly|monthly)(?:\s+limit(?:\s+remaining)?)?$/i.exec(quota.name.trim())
  if (!match) return quota.name
  const period = quotaPeriodLabel(match[2])
  return match[1] ? `${match[1].trim()} · ${period}` : period
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
  data.value = null
  query.value = ''
  selectedId.value = ''
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
</script>

<template>
  <div v-if="open" class="ad-mask account-mask" @click.self="close">
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
            <p class="secondary">读取本机 Antigravity Tools 账号与配额。切换后启动的 CLI 会话使用所选账号。</p>
          </div>
        </div>
        <div class="actions">
          <button v-if="data?.managerPath" type="button" class="btn btn-ghost btn-small" :disabled="!!busy" @click="openManager">
            {{ busy === 'launch' ? '打开中…' : '打开 Antigravity Tools' }}
          </button>
          <button v-else-if="data" type="button" class="btn btn-ghost btn-small" :disabled="!!busy" @click="emit('install')">
            下载安装
          </button>
          <button type="button" class="btn btn-ghost btn-small" :disabled="!!busy" @click="close">关闭</button>
        </div>
      </header>

      <div class="current-account">
        <span class="current-dot" :class="{ 'is-current': current }" aria-hidden="true"></span>
        <span class="current-label">当前 CLI 账号</span>
        <strong :title="current?.email">{{ current?.email || (busy === 'load' ? '识别中…' : '未识别') }}</strong>
      </div>

      <p v-if="error" class="banner banner-error" role="alert">{{ error }}</p>
      <p v-if="notice" class="banner banner-success" role="status">{{ notice }}</p>

      <div class="workspace">
        <aside class="account-sidebar" aria-label="本机账号">
          <div class="sidebar-head">
            <div class="sidebar-title">
              <h3>本机账号</h3>
              <span class="secondary">{{ accounts.length }} 个</span>
            </div>
            <input v-model="query" type="search" aria-label="搜索 Antigravity 账号" placeholder="搜索名称或邮箱" :disabled="!accounts.length" />
          </div>
          <div class="account-list">
            <p v-if="busy !== 'load' && !error && !filtered.length" class="list-empty secondary">{{ query ? '未找到匹配的账号' : '暂无本机账号' }}</p>
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
              <span v-if="account.email !== accountLabel(account)" class="account-email secondary" :title="account.email">{{ account.email }}</span>
              <span v-if="account.id === current?.id" class="pill pill-current">当前 CLI 账号</span>
              <span v-else-if="account.disabled" class="pill">已停用</span>
            </button>
          </div>
        </aside>

        <section class="account-detail" aria-label="账号详情">
          <template v-if="selected">
            <div class="detail-head">
              <div class="identity">
                <h3>{{ accountLabel(selected) }}</h3>
                <p v-if="selected.email !== accountLabel(selected)" class="secondary">{{ selected.email }}</p>
                <div class="account-meta">
                  <span v-if="selected.subscription" class="pill">{{ subscriptionLabel(selected.subscription) }}</span>
                  <span v-if="selected.disabled" class="pill">已停用</span>
                  <span v-if="selected.lastUsed" class="secondary">上次使用 {{ formatQuotaTime(selected.lastUsed) }}</span>
                </div>
              </div>
              <span v-if="selectedIsCurrent" class="pill pill-current detail-current">当前 CLI 账号</span>
              <button
                v-else
                type="button"
                class="btn btn-primary btn-small"
                :disabled="!!busy || !!switchBlocked"
                :title="switchBlocked || undefined"
                @click="switchAccount(selected)"
              >
                {{ busy === 'switch' ? '切换中…' : '设为 CLI 账号' }}
              </button>
            </div>
            <p v-if="switchBlocked && !selectedIsCurrent" class="switch-hint secondary">{{ switchBlocked }}</p>

            <div class="quota">
              <div class="quota-heading">
                <h4>模型配额</h4>
                <span v-if="selected.quotaUpdatedAt" class="secondary">更新于 {{ formatQuotaTime(selected.quotaUpdatedAt) }}</span>
              </div>
              <div v-if="quotaGroups.length" class="quota-table-wrap">
                <table aria-label="模型配额">
                  <thead><tr><th scope="col">模型 / 周期</th><th scope="col">剩余配额</th><th scope="col">重置时间</th></tr></thead>
                  <tbody v-for="group in quotaGroups" :key="group.key">
                    <tr v-if="group.label" class="quota-group">
                      <th scope="rowgroup" colspan="3">{{ group.label }}</th>
                    </tr>
                    <tr v-for="(quota, index) in group.quotas" :key="index">
                      <th scope="row" class="quota-name">{{ quotaRowLabel(quota) }}</th>
                      <td>
                        <div class="quota-value" :class="usageTone(quota.percentage)">
                          <progress :aria-label="`剩余配额 ${group.label} ${quotaRowLabel(quota)}`" :value="quota.percentage" max="100" />
                          <span>{{ formatPercent(quota.percentage) }}%</span>
                        </div>
                      </td>
                      <td class="reset-time" :title="quota.resetTime || undefined">{{ formatQuotaTime(quota.resetTime) }}</td>
                    </tr>
                  </tbody>
                </table>
              </div>
              <p v-else class="quota-empty secondary">暂无配额记录。在 Antigravity Tools 中同步配额，再刷新本机数据。</p>
            </div>
          </template>
          <div v-else-if="busy === 'load'" class="detail-empty secondary" role="status">正在读取本机数据…</div>
          <div v-else-if="!error && data && !accounts.length" class="detail-empty">
            <h3>{{ api.isTauri ? '暂无已保存的账号' : '请在桌面端查看本机账号' }}</h3>
            <p v-if="api.isTauri" class="secondary">
              {{ data.managerPath ? '在 Antigravity Tools 中登录后，刷新本机数据。' : '安装 Antigravity Tools 并登录后，刷新本机数据。' }}
            </p>
          </div>
        </section>
      </div>

      <footer>
        <span class="data-path secondary" :title="data?.dataDir || undefined">{{ data?.dataDir || 'Antigravity Tools 本机数据' }}</span>
        <button type="button" class="btn btn-ghost btn-small" :disabled="!!busy || !api.isTauri" @click="reload">
          {{ busy === 'load' ? '刷新中…' : '刷新本机数据' }}
        </button>
      </footer>
    </section>
  </div>
</template>

<style scoped>
.account-dialog {
  --account-secondary: color-mix(in srgb, var(--ad-text) 24%, var(--ad-muted));
  display: flex;
  flex-direction: column;
  width: 880px;
  padding: 0;
  overflow: hidden;
  background: var(--ad-float-solid);
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}

.dialog-head, .current-account, footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 16px 24px;
  flex-shrink: 0;
}

.heading, .actions, .account-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.heading { gap: 12px; }
.actions { flex-shrink: 0; }
.actions .btn, footer .btn { white-space: nowrap; }
.tool-mark { display: block; width: 36px; height: 36px; flex-shrink: 0; }

h2, h3, h4, p { margin: 0; }
h2 { font-size: 20px; line-height: 28px; }
h3 { font-size: 16px; line-height: 24px; }
h4 { font-size: 14px; line-height: 20px; }
.secondary { color: var(--account-secondary); font-size: 12px; line-height: 20px; }

.current-account {
  justify-content: flex-start;
  gap: 8px;
  padding-block: 8px;
  border-block: 1px solid var(--ad-border);
  background: var(--ad-float-inset-solid);
  color: var(--account-secondary);
  font-size: 13px;
}

.current-dot {
  width: 8px;
  height: 8px;
  flex-shrink: 0;
  border-radius: 50%;
  background: var(--ad-muted);
}

.current-dot.is-current { background: var(--ad-antigravity); }
.current-label { flex-shrink: 0; }
.current-account strong { color: var(--ad-text); font-weight: 600; }

.banner {
  padding: 8px 24px;
  border-bottom: 1px solid var(--ad-border);
  font-size: 13px;
  line-height: 20px;
  overflow-wrap: anywhere;
}

.banner-error {
  background: color-mix(in srgb, var(--ad-error) 8%, var(--ad-float-solid));
  box-shadow: inset 3px 0 var(--ad-error);
}

.banner-success {
  background: color-mix(in srgb, var(--ad-success) 8%, var(--ad-float-solid));
  box-shadow: inset 3px 0 var(--ad-success);
}

.workspace {
  display: grid;
  grid-template-columns: 232px minmax(0, 1fr);
  flex: 1 1 auto;
  min-height: 0;
}

.account-sidebar {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  border-inline-end: 1px solid var(--ad-border);
  background: var(--ad-float-inset-solid);
}

.sidebar-head { display: grid; gap: 8px; padding: 16px 12px 8px; }
.sidebar-title { display: flex; justify-content: space-between; align-items: center; gap: 8px; }
.sidebar-title h3 { font-size: 13px; line-height: 20px; }
.sidebar-head input { width: 100%; height: 32px; background: var(--ad-float-solid); }
.sidebar-head input::placeholder { color: var(--account-secondary); }
.account-list { display: grid; align-content: start; gap: 4px; padding: 0 8px 12px; overflow-y: auto; }
.list-empty { padding: 8px; }

.account-row {
  display: grid;
  justify-items: start;
  width: 100%;
  min-width: 0;
  padding: 8px 12px;
  border: 1px solid transparent;
  border-inline-start-width: 2px;
  border-radius: var(--ad-radius-control);
  text-align: left;
}

.account-row:hover { background: var(--ad-hover); }
.account-row[aria-pressed='true'] {
  background: color-mix(in srgb, var(--ad-antigravity) 12%, var(--ad-float-inset-solid));
  border-color: var(--ad-border);
  border-inline-start-color: var(--ad-antigravity);
}

.account-name, .account-email, .data-path, .current-account strong {
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.account-name { font-size: 14px; font-weight: 600; line-height: 20px; }
.account-row .pill { margin-top: 4px; }

.pill {
  width: fit-content;
  color: var(--ad-text);
  background: var(--ad-hover);
  border-radius: 4px;
  padding: 0 8px;
  font-size: 12px;
  line-height: 22px;
}

.pill-current {
  color: var(--ad-antigravity);
  background: color-mix(in srgb, var(--ad-antigravity) 12%, transparent);
}

.account-detail {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 16px 24px;
  min-width: 0;
  min-height: 0;
  overflow-y: auto;
}

.detail-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; }
.detail-head > :is(.btn, .pill) { flex-shrink: 0; white-space: nowrap; }
.detail-current { line-height: 28px; }
.identity { min-width: 0; overflow-wrap: anywhere; }
.account-meta { flex-wrap: wrap; margin-top: 8px; }
.account-meta:empty { display: none; }
.switch-hint { padding: 8px 12px; background: var(--ad-float-inset-solid); border-radius: var(--ad-radius-control); }

.quota { min-width: 0; }
.quota-heading { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 8px; margin-bottom: 8px; }
.quota-table-wrap { overflow: auto; border: 1px solid var(--ad-border-strong); border-radius: var(--ad-radius-control); }
.quota table { width: 100%; border-collapse: collapse; font-size: 13px; line-height: 20px; }
.quota th, .quota td { padding: 8px 12px; }
.quota th { text-align: left; }
.quota thead th {
  color: var(--account-secondary);
  background: var(--ad-float-inset-solid);
  font-size: 12px;
  font-weight: 600;
  white-space: nowrap;
}
.quota thead th:not(:first-child), .quota td { text-align: right; }
.quota tbody :is(th, td) { border-top: 1px solid var(--ad-border); }
.quota tbody tr:not(.quota-group):hover { background: var(--ad-hover); }
.quota-group th { padding-block: 4px; background: var(--ad-float-inset-solid); font-weight: 600; }
.quota-name { width: 100%; font-weight: 400; overflow-wrap: anywhere; }
.quota-group ~ tr .quota-name { padding-inline-start: 20px; }
.quota-value { display: flex; justify-content: flex-end; align-items: center; gap: 8px; }
.quota-value progress {
  appearance: none;
  width: 72px;
  height: 6px;
  border: 0;
  border-radius: 999px;
  overflow: hidden;
  background: var(--ad-border-strong);
  color: var(--ad-antigravity);
}
.quota-value progress::-webkit-progress-bar { background: var(--ad-border-strong); }
.quota-value progress::-webkit-progress-value { background: currentColor; border-radius: inherit; }
.quota-value progress::-moz-progress-bar { background: currentColor; border-radius: inherit; }
.quota-value.warn progress { color: var(--ad-warning); }
.quota-value.err progress { color: var(--ad-error); }
.quota-value span { min-width: 40px; font-variant-numeric: tabular-nums; }
.reset-time { white-space: nowrap; font-variant-numeric: tabular-nums; }
.quota-empty, .detail-empty { padding: 16px; background: var(--ad-float-inset-solid); border-radius: var(--ad-radius-control); }

footer { padding-block: 8px; border-top: 1px solid var(--ad-border); }
footer .btn { flex-shrink: 0; }

@media (max-width: 760px) {
  .workspace { grid-template-columns: minmax(152px, 31%) minmax(0, 1fr); }
  .dialog-head, .current-account, .account-detail, footer { padding-inline: 16px; }
  .detail-head { flex-wrap: wrap; }
  .quota-value progress { width: 40px; }
}

@media (max-width: 600px) {
  .account-mask { padding: 12px; }
  .dialog-head { flex-wrap: wrap; gap: 12px; }
  .actions { margin-inline-start: auto; }
  .workspace { display: block; overflow-y: auto; }
  .account-sidebar { border-inline-end: 0; border-bottom: 1px solid var(--ad-border); }
  .account-list { max-height: 144px; }
  .account-detail { overflow: visible; }
}

@media (max-width: 400px) {
  .quota th, .quota td { padding-inline: 8px; }
  .quota-group ~ tr .quota-name { padding-inline-start: 12px; }
  .quota-value progress { display: none; }
}
</style>
