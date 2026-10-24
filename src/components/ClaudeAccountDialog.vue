<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import * as api from '../lib/api'
import type { ClaudeAccount, ClaudeAccountList } from '../lib/types'
import { claudeStatus, proxyLabel, canUseClaudeAccount } from '../lib/claudeAccounts'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ close: []; switched: [] }>()
const data = ref<ClaudeAccountList>({ loggedIn: false, currentEmail: '', currentName: '', accounts: [] })
const selectedId = ref('')
const query = ref('')
const busy = ref('')
const error = ref('')
const notice = ref('')
const mode = ref<'detail' | 'create' | 'edit'>('detail')
const nameDraft = ref('')
const proxyMode = ref('inherit')
const proxyDraft = ref('')
const removing = ref(false)
const dialog = ref<HTMLElement>()
let previousFocus: HTMLElement | null = null
const selected = computed(() => data.value.accounts.find(a => a.id === selectedId.value))
const filtered = computed(() => {
  const term = query.value.trim().toLowerCase()
  return data.value.accounts.filter(a => `${a.name} ${a.email}`.toLowerCase().includes(term))
})
const current = computed(() => data.value.accounts.find(a => a.active))
const managedCount = computed(() => data.value.accounts.filter(a => a.source !== 'legacy').length)
const legacyCount = computed(() => data.value.accounts.filter(a => a.source === 'legacy').length)
const formError = computed(() => {
  if (!nameDraft.value.trim()) return '请输入账号名称'
  if (proxyMode.value === 'custom' && !proxyDraft.value.trim()) return '请输入代理端口或 HTTP(S) 地址'
  return ''
})

watch(() => props.open, async open => {
  if (!open) { previousFocus?.focus(); return }
  previousFocus = document.activeElement as HTMLElement | null
  error.value = ''; notice.value = ''; removing.value = false; query.value = ''; mode.value = 'detail'
  await nextTick()
  dialog.value?.focus()
  await reload()
})

function accept(next: ClaudeAccountList) {
  data.value = next
  if (!next.accounts.some(a => a.id === selectedId.value)) {
    selectedId.value = next.accounts.find(a => a.active)?.id || next.accounts[0]?.id || ''
  }
}

async function run(operation: string, task: () => Promise<void>) {
  if (busy.value || !api.isTauri) return
  busy.value = operation; error.value = ''; notice.value = ''
  try { await task() }
  catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
    // A cancelled login may already have created an isolated profile. Keep it
    // visible so the next attempt resumes in the same directory.
    if (operation === 'login' || operation === 'verify') {
      try { accept(await api.listClaudeAccounts()) } catch { /* preserve original error */ }
    }
  } finally { busy.value = '' }
}

async function reload() {
  if (!api.isTauri) { accept(await api.listClaudeAccounts()); return }
  await run('load', async () => accept(await api.listClaudeAccounts()))
}

function select(account: ClaudeAccount) {
  if (busy.value) return
  selectedId.value = account.id; mode.value = 'detail'; removing.value = false; notice.value = ''; error.value = ''
}

function edit(create = false) {
  if (busy.value) return
  mode.value = create ? 'create' : 'edit'; removing.value = false; error.value = ''; notice.value = ''
  nameDraft.value = create ? '' : selected.value?.name || ''
  const proxy = create ? 'inherit' : selected.value?.proxy || 'inherit'
  proxyMode.value = ['inherit', 'off'].includes(proxy) ? proxy : 'custom'
  proxyDraft.value = proxyMode.value === 'custom' ? proxy : ''
  void nextTick(() => dialog.value?.querySelector<HTMLInputElement>('#claude-account-name')?.focus())
}

async function save() {
  if (formError.value) return
  const creating = mode.value === 'create'
  await run('save', async () => {
    const proxy = proxyMode.value === 'custom' ? proxyDraft.value.trim() : proxyMode.value
    if (creating) {
      const before = new Set(data.value.accounts.map(a => a.id))
      const next = await api.createClaudeAccount(nameDraft.value.trim(), proxy)
      accept(next)
      selectedId.value = next.accounts.find(a => !before.has(a.id))?.id || selectedId.value
    } else if (selected.value) {
      accept(await api.updateClaudeAccount(selected.value.id, nameDraft.value.trim(), proxy))
    }
    mode.value = 'detail'
    notice.value = creating ? '账号已创建，请登录后设为默认账号。' : '设置已保存，下次启动会话时生效。'
  })
}

async function login() {
  const account = selected.value
  if (!account) return
  await run('login', async () => {
    accept(await api.loginClaudeAccount(account.id))
    notice.value = '登录成功，可以将此账号设为默认账号。'
  })
}

async function useAccount() {
  const account = selected.value
  if (!account || !canUseClaudeAccount(account)) return
  await run('switch', async () => {
    accept(await api.switchClaudeAccount(account.id))
    notice.value = `新会话将使用「${account.name}」，已打开的会话继续使用各自账号。`
    emit('switched')
  })
}

async function verify() {
  const account = selected.value
  if (!account) return
  await run('verify', async () => {
    accept(await api.verifyClaudeAccount(account.id))
    notice.value = '账号身份检查通过。'
  })
}

async function cancelLogin() {
  try { await api.cancelClaudeLogin() }
  catch { error.value = '无法取消登录，请关闭登录窗口后重试。' }
}

async function remove() {
  const account = selected.value
  if (!account) return
  await run('remove', async () => {
    const wasDefault = account.active
    accept(await api.deleteClaudeAccount(account.id))
    removing.value = false
    notice.value = '已移出账号列表，登录目录和会话历史保留在本机。'
    if (wasDefault) emit('switched')
  })
}

async function copyPath() {
  if (selected.value && await api.clipboardWrite(selected.value.configDir)) notice.value = '账号目录已复制。'
}

function close() { if (!busy.value) emit('close') }
function onKey(event: KeyboardEvent) {
  if (event.key === 'Escape') { event.stopPropagation(); close() }
  if (event.key !== 'Tab') return
  const items = [...(dialog.value?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), [tabindex="0"]') || [])]
  const first = items[0], last = items[items.length - 1]
  if (!first || !last) return
  if (event.shiftKey && (document.activeElement === first || document.activeElement === dialog.value)) { event.preventDefault(); last.focus() }
  else if (!event.shiftKey && (document.activeElement === last || document.activeElement === dialog.value)) { event.preventDefault(); first.focus() }
}
</script>

<template>
  <div v-if="open" class="ad-mask" @click.self="close">
    <div ref="dialog" class="ad-dialog claude-dialog" role="dialog" aria-modal="true" aria-labelledby="claude-accounts-title" tabindex="-1" :aria-busy="!!busy" @keydown="onKey">
      <header class="dialog-head">
        <div class="heading">
          <span class="claude-mark" aria-hidden="true">✳</span>
          <div><h2 id="claude-accounts-title">Claude 账号</h2><p>每个账号独立登录，可同时运行多个账号。</p></div>
        </div>
        <button type="button" class="btn btn-ghost btn-small" :disabled="!!busy" @click="close">关闭</button>
      </header>

      <div class="summary">
        <span>新会话默认 <strong>{{ current?.name || '系统 Claude' }}</strong></span>
        <span>{{ managedCount }} 个账号<span v-if="legacyCount"> · {{ legacyCount }} 个待迁移</span></span>
      </div>
      <p v-if="!api.isTauri" class="banner">浏览器仅供预览。请在桌面端创建、登录和管理本机账号。</p>
      <p v-if="error" class="banner banner-error" role="alert">{{ error }} <button type="button" :disabled="!!busy" @click="reload">刷新列表</button></p>
      <p v-if="notice" class="banner banner-success" role="status">{{ notice }}</p>
      <div v-if="busy === 'login'" class="banner login-progress" role="status">
        <span class="spinner" aria-hidden="true" /><div><strong>等待浏览器授权…</strong><p>请在打开的浏览器中登录{{ selected?.email ? ` ${selected.email}` : '此账号' }}。完成后自动更新，最长等待 5 分钟。</p></div><button type="button" class="text-button" @click="cancelLogin">取消登录</button>
      </div>

      <div class="workspace">
        <aside class="account-sidebar" aria-label="账号列表">
          <div class="list-tools">
            <input v-model="query" aria-label="搜索账号" placeholder="搜索名称或邮箱" :disabled="!!busy" />
            <button type="button" class="btn btn-primary add-account" :disabled="!!busy || !api.isTauri" @click="edit(true)">＋ 添加账号</button>
          </div>
          <div class="account-list">
            <p v-if="busy === 'load'" class="empty" role="status">正在读取本机账号…</p>
            <p v-else-if="!filtered.length" class="empty">{{ query ? '未找到匹配的账号' : '暂无账号，请在桌面端添加。' }}</p>
            <button v-for="account in filtered" :key="account.id" type="button" class="account-row" :class="{ selected: selectedId === account.id }" :aria-pressed="selectedId === account.id" :disabled="!!busy" @click="select(account)">
              <span class="avatar" :class="{ legacy: account.source === 'legacy' }" aria-hidden="true">{{ account.source === 'legacy' ? '↗' : account.name.slice(0, 1).toUpperCase() }}</span>
              <span class="row-body"><span class="row-name">{{ account.name }} <span v-if="account.active" class="default-dot" title="新会话默认账号" /></span><span class="row-email">{{ account.email || (account.source === 'system' ? '使用原有 Claude 目录' : '等待首次登录') }}</span></span>
              <span class="row-status" :class="account.status">{{ claudeStatus(account.status) }}</span>
            </button>
          </div>
          <div class="sidebar-foot"><span class="isolation-dot" /> 登录、设置和历史分别存储</div>
        </aside>

        <section class="account-detail" aria-label="账号详情">
          <form v-if="mode !== 'detail'" class="account-form" @submit.prevent="save">
            <h3>{{ mode === 'create' ? '添加独立账号' : '编辑账号' }}</h3>
            <p class="muted">{{ mode === 'create' ? '创建后登录一次，即可与其他账号同时使用。' : '名称和代理设置会在下次启动时生效。' }}</p>
            <label for="claude-account-name">账号名称 <span class="required">*</span></label>
            <input id="claude-account-name" v-model="nameDraft" required maxlength="40" placeholder="例如：工作账号" :disabled="!!busy" />
            <label for="claude-proxy-mode">网络代理</label>
            <select id="claude-proxy-mode" v-model="proxyMode" :disabled="!!busy">
              <option value="inherit">跟随项目 / 应用代理</option><option value="off">直连</option><option value="custom">为此账号指定代理</option>
            </select>
            <template v-if="proxyMode === 'custom'">
              <label for="claude-proxy-url">代理地址 <span class="required">*</span></label>
              <input id="claude-proxy-url" v-model="proxyDraft" required placeholder="7890 或 http://127.0.0.1:7890" :disabled="!!busy" />
              <p class="muted">支持 HTTP(S)；地址中不包含用户名或密码。</p>
            </template>
            <div class="form-note">账号目录由应用管理。各账号分别保存登录状态，切换默认账号无需退出其他会话。</div>
            <div class="actions">
              <button type="button" class="btn btn-ghost" :disabled="!!busy" @click="mode = 'detail'">取消</button>
              <button type="submit" class="btn btn-primary" :disabled="!!busy || !!formError || !api.isTauri">{{ busy === 'save' ? '保存中…' : mode === 'create' ? '创建账号' : '保存设置' }}</button>
            </div>
          </form>

          <template v-else-if="selected">
            <div class="detail-head"><div><p class="eyebrow">{{ selected.source === 'system' ? '系统目录' : selected.source === 'legacy' ? '旧版快照' : '独立账号' }}</p><h3>{{ selected.name }}</h3></div><span v-if="selected.active" class="default-badge">新会话默认</span></div>
            <p class="detail-email">{{ selected.email || '尚未记录登录邮箱' }}</p>
            <div class="status-line" :class="selected.status"><span class="status-dot" /><span>{{ claudeStatus(selected.status) }}</span><span v-if="selected.subscriptionType" class="plan">{{ selected.subscriptionType.toUpperCase() }}</span></div>
            <p class="status-message">{{ selected.message }}</p>
            <template v-if="selected.source === 'legacy'">
              <div class="migration-note"><strong>迁移为独立账号</strong><p>用此邮箱重新登录一次，建立独立目录。旧版快照保留在本机，之后不再轮换全局凭证。</p></div>
            </template>
            <dl v-else class="facts">
              <div><dt>网络代理</dt><dd :title="selected.proxy">{{ proxyLabel(selected.proxy) }}</dd></div>
              <div class="path-fact"><dt>账号目录</dt><dd><code>{{ selected.configDir }}</code><button type="button" class="text-button" @click="copyPath">复制</button></dd></div>
            </dl>
            <div v-if="selected.source === 'system'" class="form-note">此入口使用本机原有 Claude 目录。添加其他账号时选择「添加账号」，在新目录中完成登录。</div>
            <div class="detail-actions">
              <button v-if="selected.status === 'unverified' || selected.source === 'system'" type="button" class="btn" :class="canUseClaudeAccount(selected) ? 'btn-ghost' : 'btn-primary'" :disabled="!!busy || !api.isTauri" @click="verify">{{ busy === 'verify' ? '检查中…' : '检查登录' }}</button>
              <button v-if="canUseClaudeAccount(selected)" type="button" class="btn btn-primary" :disabled="!!busy || selected.active || !api.isTauri" @click="useAccount">{{ busy === 'switch' ? '设置中…' : selected.active ? '已设为默认' : '设为默认账号' }}</button>
              <button v-if="selected.source !== 'system'" type="button" class="btn" :class="canUseClaudeAccount(selected) || selected.status === 'unverified' ? 'btn-ghost' : 'btn-primary'" :disabled="!!busy || !api.isTauri" @click="login">{{ busy === 'login' ? '等待授权…' : selected.source === 'legacy' ? '迁移并登录' : canUseClaudeAccount(selected) ? '重新登录' : '登录账号' }}</button>
              <button v-if="selected.source !== 'legacy'" type="button" class="btn btn-ghost" :disabled="!!busy || !api.isTauri" @click="edit()">编辑</button>
            </div>
            <div v-if="selected.source !== 'system'" class="remove-zone">
              <button v-if="!removing" type="button" class="text-button danger" :disabled="!!busy || !api.isTauri" @click="removing = true">移出账号列表</button>
              <div v-else class="remove-confirm" role="alert">
                <p>将「{{ selected.name }}」移出列表？登录目录和历史会保留{{ selected.active ? '，新会话将回到系统 Claude' : '' }}。</p>
                <div class="actions"><button type="button" class="btn btn-ghost btn-small" :disabled="!!busy" @click="removing = false">取消</button><button type="button" class="btn btn-danger btn-small" :disabled="!!busy" @click="remove">{{ busy === 'remove' ? '移除中…' : '移出列表' }}</button></div>
              </div>
            </div>
          </template>
          <div v-else class="detail-empty"><span class="claude-mark" aria-hidden="true">✳</span><h3>为不同工作保留各自的账号</h3><p>添加账号后，在独立目录完成登录。已有会话会继续使用原来的账号。</p></div>
        </section>
      </div>
      <footer><span>恢复历史会话时，自动使用该会话所属的账号。</span><button type="button" class="text-button" :disabled="!!busy" @click="reload">{{ busy === 'load' ? '刷新中…' : '刷新状态' }}</button></footer>
    </div>
  </div>
</template>

<style scoped>
.claude-dialog { width: 880px; max-width: calc(100vw - 48px); max-height: calc(100vh - 48px); padding: 0; overflow: auto; outline: none; }
.dialog-head { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 24px; }
.heading { display: flex; align-items: center; gap: 16px; }
.claude-mark { color: var(--ad-claude); font-size: 32px; line-height: 40px; }
h2, h3, p { margin: 0; }
h2 { font-size: 20px; line-height: 28px; font-weight: 600; }
h3 { font-size: 16px; line-height: 24px; font-weight: 600; overflow-wrap: anywhere; }
.heading p, .muted, .status-message, .form-note, .detail-empty p { color: var(--ad-muted); font-size: 12px; line-height: 20px; }
.heading p { margin-top: 4px; }
.summary { display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 8px; padding: 12px 24px; border-block: 1px solid var(--ad-border); color: var(--ad-muted); font-size: 12px; }
.summary strong { margin-left: 8px; color: var(--ad-text); font-weight: 600; }
.workspace { display: grid; grid-template-columns: 320px minmax(0, 1fr); min-height: 440px; }
.account-sidebar { display: flex; flex-direction: column; border-right: 1px solid var(--ad-border); min-width: 0; }
.list-tools { display: flex; flex-direction: column; gap: 8px; padding: 16px; }
.list-tools input, .account-form input, .account-form select { width: 100%; height: 32px; font-size: 13px; }
.btn { height: 32px; font-size: 13px; }
.btn-small { height: 28px; font-size: 12px; }
.account-list { flex: 1; max-height: 360px; overflow-y: auto; padding: 0 8px 8px; }
.account-row { display: flex; align-items: center; gap: 8px; text-align: left; width: 100%; padding: 12px 8px; border: 1px solid transparent; border-left: 3px solid transparent; border-radius: 4px; margin-bottom: 4px; }
.account-row:hover { background: var(--ad-hover); }
.account-row.selected { background: var(--ad-selected); border-left-color: var(--ad-claude); }
.account-row:disabled { opacity: 0.7; }
.avatar { display: grid; place-items: center; height: 32px; width: 32px; flex-shrink: 0; color: var(--ad-claude); background: color-mix(in srgb, var(--ad-claude) 12%, transparent); border-radius: 4px; font-weight: 600; }
.avatar.legacy { color: var(--ad-warning); background: color-mix(in srgb, var(--ad-warning) 12%, transparent); }
.row-body { display: flex; flex-direction: column; min-width: 0; flex: 1; }
.row-name { display: flex; align-items: center; gap: 8px; font-size: 13px; font-weight: 600; }
.row-name, .row-email { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.row-email { font-size: 11px; color: var(--ad-muted); margin-top: 4px; }
.default-dot, .isolation-dot, .status-dot { display: inline-block; width: 6px; height: 6px; border-radius: 50%; background: var(--ad-claude); flex-shrink: 0; }
.row-status { font-size: 11px; flex-shrink: 0; color: var(--ad-muted); }
.row-status.ready { color: var(--ad-success); }
.row-status.legacy, .row-status.refreshable { color: var(--ad-warning); }
.row-status.broken, .row-status.conflict { color: var(--ad-error); }
.sidebar-foot { padding: 12px 16px; display: flex; align-items: center; gap: 8px; font-size: 11px; color: var(--ad-muted); border-top: 1px solid var(--ad-border); }
.isolation-dot { background: var(--ad-success); }
.account-detail { padding: 24px; min-width: 0; display: flex; flex-direction: column; }
.detail-head { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
.eyebrow { color: var(--ad-muted); font-size: 12px; margin-bottom: 8px; }
.default-badge { white-space: nowrap; font-size: 11px; color: var(--ad-claude); border: 1px solid color-mix(in srgb, var(--ad-claude) 32%, transparent); padding: 4px 8px; border-radius: 4px; }
.detail-email { margin-top: 8px; color: var(--ad-muted); overflow-wrap: anywhere; }
.status-line { display: flex; align-items: center; gap: 8px; margin-top: 24px; font-size: 13px; }
.status-line .status-dot { background: var(--ad-muted); }
.status-line.ready .status-dot { background: var(--ad-success); }
.status-line.refreshable .status-dot, .status-line.legacy .status-dot { background: var(--ad-warning); }
.status-line.broken .status-dot, .status-line.conflict .status-dot { background: var(--ad-error); }
.plan { color: var(--ad-muted); font-family: var(--ad-mono); font-size: 11px; margin-left: auto; }
.status-message { margin-top: 8px; }
.facts { margin: 24px 0 16px; padding: 0; }
.facts > div { display: flex; gap: 16px; padding: 12px 0; border-top: 1px solid var(--ad-border); font-size: 12px; }
.facts dt { width: 64px; flex-shrink: 0; color: var(--ad-muted); }
.facts dd { margin: 0; min-width: 0; overflow-wrap: anywhere; }
.path-fact dd { display: flex; gap: 8px; align-items: flex-start; flex: 1; }
.path-fact code { font-family: var(--ad-mono); font-size: 11px; flex: 1; overflow-wrap: anywhere; }
.text-button { padding: 0; color: var(--ad-muted); font-size: 12px; flex-shrink: 0; }
.text-button:hover:not(:disabled) { color: var(--ad-text); text-decoration: underline; }
.text-button.danger { color: var(--ad-error); }
.detail-actions { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 24px; }
.remove-zone { margin-top: auto; padding-top: 24px; }
.remove-confirm { padding: 12px; border: 1px solid color-mix(in srgb, var(--ad-error) 32%, transparent); border-radius: 4px; }
.remove-confirm p { font-size: 12px; margin-bottom: 12px; }
.actions { display: flex; justify-content: flex-end; gap: 8px; }
.account-form { display: flex; flex-direction: column; }
.account-form > .muted { margin-top: 4px; }
.account-form label { margin: 24px 0 8px; font-size: 12px; }
.account-form .muted { margin-top: 8px; }
.required { color: var(--ad-error); }
.form-note, .migration-note { padding: 12px; border: 1px solid var(--ad-border); border-radius: 4px; background: var(--ad-hover); }
.account-form .form-note { margin: 24px 0; }
.migration-note { margin-top: 24px; font-size: 12px; line-height: 20px; border-color: color-mix(in srgb, var(--ad-warning) 32%, transparent); }
.migration-note p { color: var(--ad-muted); margin-top: 8px; }
.banner { margin: 0; padding: 12px 24px; background: var(--ad-hover); color: var(--ad-muted); font-size: 12px; line-height: 20px; border-bottom: 1px solid var(--ad-border); overflow-wrap: anywhere; }
.banner-error { color: var(--ad-error); }
.banner-error button { text-decoration: underline; }
.banner-success { color: var(--ad-success); }
.login-progress { display: flex; gap: 12px; align-items: center; }
.login-progress strong { color: var(--ad-text); }
.spinner { width: 16px; height: 16px; flex-shrink: 0; border: 2px solid var(--ad-border-strong); border-top-color: var(--ad-claude); border-radius: 50%; animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
footer { display: flex; align-items: center; justify-content: space-between; gap: 16px; border-top: 1px solid var(--ad-border); padding: 12px 24px; color: var(--ad-muted); font-size: 11px; }
.empty { padding: 24px 8px; color: var(--ad-muted); font-size: 12px; text-align: center; }
.detail-empty { margin: auto; max-width: 280px; text-align: center; }
.detail-empty h3 { margin: 16px 0 8px; }
@media (max-width: 720px) { .workspace { grid-template-columns: 1fr; } .account-sidebar { border-right: none; border-bottom: 1px solid var(--ad-border); } .account-list { max-height: 180px; } .sidebar-foot { display: none; } .dialog-head, .account-detail { padding: 16px; } footer { align-items: flex-start; } }
@media (prefers-reduced-motion: reduce) { .spinner { animation: none; } }
</style>
