// Run against Vite on port 1423 with playwright-cli run-code (file contents).
// Mock Tauri completely: these checks never touch local credentials/history.
async page => {
  const errors = []
  page.on('pageerror', error => errors.push(error.message))
  await page.setViewportSize({ width: 1280, height: 900 })
  await page.route('**/claude-actions-smoke', route => route.fulfill({ contentType: 'text/html', body: '<!doctype html><html><head><meta charset="utf-8"></head><body><div id="app"></div></body></html>' }))
  await page.goto('http://127.0.0.1:1423/claude-actions-smoke')
  await page.evaluate(async () => {
    const system = { id: 'system', name: '系统 Claude', email: 'old@example.com', accountUuid: 'old', subscriptionType: 'pro', updatedAt: '', active: true, configDir: 'C:\\Example\\.claude', source: 'system', status: 'conflict', message: '系统缓存身份与实际登录凭证不一致。', proxy: 'inherit' }
    const state = window.__actionsSmoke = {
      calls: [], failRepair: true, failCopy: true, copied: null,
      rows: [system,
        { ...system, id: 'work', name: '工作账号', email: 'work@example.com', active: false, source: 'isolated', status: 'ready', configDir: 'C:\\profiles\\work', message: '账号可用。' },
        { ...system, id: 'home', name: '个人账号', email: 'home@example.com', active: false, source: 'isolated', status: 'refreshable', configDir: 'C:\\profiles\\home', message: '启动时自动续期。' },
        { ...system, id: 'empty', name: '待登录账号', email: '', active: false, source: 'isolated', status: 'signedOut', message: '请先登录。' }]
    }
    const list = () => structuredClone({ loggedIn: false, currentEmail: '', currentName: '系统 Claude', accounts: state.rows })
    window.__TAURI_INTERNALS__ = { invoke: async (command, args = {}) => {
      state.calls.push({ command, args })
      if (command === 'list_claude_accounts') return list()
      if (command === 'repair_claude_system_identity') {
        if (state.failRepair) { state.failRepair = false; throw new Error('模拟：身份接口暂时无法连接，请重试。') }
        Object.assign(state.rows[0], { email: 'actual@example.com', accountUuid: 'actual', status: 'ready', message: '账号可用。' })
        return list()
      }
      if (command === 'clear_claude_credentials') {
        Object.assign(state.rows.find(a => a.id === args.accountId), { status: 'signedOut', message: '尚未登录此账号，请点击登录。' })
        return list()
      }
      if (command === 'claude_session_accounts') return { sourceAccountId: 'system', sourceAccountName: '系统 Claude', accounts: list().accounts }
      if (command === 'continue_claude_session') {
        if (state.failCopy) { state.failCopy = false; throw new Error('模拟：原会话记录尚未写完，请稍后重试。') }
        return { sessionId: 'new-session-id', title: args.title + ' · 续接', accountId: args.accountId, accountName: state.rows.find(a => a.id === args.accountId).name }
      }
      throw new Error(`Unexpected command: ${command}`)
    } }
    const source = await (await fetch('/src/components/ClaudeAccountDialog.vue')).text()
    const vueUrl = source.match(/from ["']([^"']*\/vue\.js[^"']*)["']/)?.[1]
    if (!vueUrl) throw new Error('Vue runtime unavailable')
    const { createApp, h, ref } = await import(vueUrl)
    const { default: Accounts } = await import('/src/components/ClaudeAccountDialog.vue')
    const { default: Continue } = await import('/src/components/ClaudeContinueDialog.vue')
    const { modal } = await import('/src/lib/modal.ts')
    await import('/src/styles/global.css')
    const mode = ref('')
    state.openContinuation = () => { mode.value = 'continue' }
    createApp({ render: () => h('div', [
      h(Accounts, { open: mode.value === 'accounts', onClose: () => { mode.value = '' } }),
      h(Continue, { open: mode.value === 'continue', projectId: 'project', sessionId: 'original-session', title: '修复登录问题', onClose: () => { mode.value = '' }, onContinued: result => { state.copied = result; mode.value = '' }, onAccounts: () => { mode.value = 'accounts' } })
    ]) }).directive('modal', modal).mount('#app')
    mode.value = 'accounts'
  })
  const dialog = page.getByRole('dialog', { name: 'Claude 账号', exact: true })
  await dialog.waitFor()
  await page.getByRole('button', { name: '修复系统身份', exact: true }).click()
  await page.getByRole('button', { name: '核验并修复', exact: true }).click()
  await page.getByRole('alert').filter({ hasText: '暂时无法连接' }).waitFor()
  await page.getByRole('button', { name: '核验并修复', exact: true }).click()
  await page.getByText('系统身份已与当前实际登录账号同步，可以重新打开会话。', { exact: true }).waitFor()
  await page.getByText('actual@example.com', { exact: true }).last().waitFor()
  await page.getByRole('button', { name: /工作账号/ }).click()
  await page.getByRole('button', { name: '清除登录凭证', exact: true }).click()
  await page.getByRole('button', { name: '取消', exact: true }).click()
  if (await page.evaluate(() => window.__actionsSmoke.calls.some(c => c.command === 'clear_claude_credentials'))) throw new Error('Cancel cleared credentials')
  await page.getByRole('button', { name: '清除登录凭证', exact: true }).click()
  await page.screenshot({ path: '.playwright-cli/claude-clear-confirm.png' })
  await page.getByRole('button', { name: '确认清除凭证', exact: true }).click()
  await page.getByText('登录凭证已清除，会话历史保留。再次使用此账号前需要重新登录。', { exact: true }).waitFor()
  await page.getByRole('button', { name: '登录账号', exact: true }).waitFor()
  await page.keyboard.press('Escape')
  await dialog.waitFor({ state: 'hidden' })
  await page.evaluate(() => window.__actionsSmoke.openContinuation())
  const continuation = page.getByRole('dialog', { name: '用其他 Claude 账号继续', exact: true })
  await continuation.waitFor()
  await page.getByLabel('目标账号').selectOption('home')
  const options = await page.getByLabel('目标账号').locator('option').evaluateAll(options => options.map(o => ({ value: o.value, disabled: o.disabled })))
  if (options.some(o => o.value === 'system') || options.some(o => ['work', 'empty'].includes(o.value) && !o.disabled)) throw new Error('Unavailable/source account selectable')
  await page.getByRole('button', { name: '复制并继续', exact: true }).click()
  await page.getByRole('alert').filter({ hasText: '尚未写完' }).waitFor()
  await page.screenshot({ path: '.playwright-cli/claude-continue-dark.png' })
  await page.evaluate(() => { document.documentElement.dataset.theme = 'light' })
  await page.setViewportSize({ width: 400, height: 720 })
  if (!await continuation.evaluate(el => el.scrollWidth <= el.clientWidth && el.getBoundingClientRect().right <= innerWidth)) throw new Error('Continuation overflows on narrow screen')
  await page.screenshot({ path: '.playwright-cli/claude-continue-narrow-light.png' })
  await page.getByRole('button', { name: '复制并继续', exact: true }).click()
  await continuation.waitFor({ state: 'hidden' })
  const result = await page.evaluate(() => ({ calls: window.__actionsSmoke.calls, copied: window.__actionsSmoke.copied }))
  const clears = result.calls.filter(c => c.command === 'clear_claude_credentials')
  if (clears.length !== 1 || clears[0].args.accountId !== 'work') throw new Error('Cleared wrong account')
  if (result.copied?.sessionId !== 'new-session-id' || result.copied?.accountId !== 'home' || result.copied?.projectId !== 'project') throw new Error('Continuation result lost target account/project')
  if (errors.length) throw new Error(errors.join('\n'))
  return { ok: true, checks: ['repair retry', 'clear cancel', 'clear selected account', 'disabled targets', 'copy retry', 'correct target', 'narrow layout'], commands: result.calls.map(c => c.command) }
}
