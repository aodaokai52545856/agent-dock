// Run against Vite on port 1423 with playwright-cli run-code (file contents).
// All Tauri commands are mocked; no local accounts or credentials are accessed.
async page => {
  const errors = []
  page.on('pageerror', error => errors.push(error.message))
  await page.setViewportSize({ width: 1280, height: 840 })
  await page.route('**/claude-accounts-smoke', route => route.fulfill({ contentType: 'text/html', body: '<!doctype html><html><head><meta charset="utf-8"></head><body><div id="app"></div></body></html>' }))
  await page.goto('http://127.0.0.1:1423/claude-accounts-smoke')
  await page.evaluate(async () => {
    const system = { id: 'system', name: '系统 Claude', email: 'main@example.com', accountUuid: 'main', subscriptionType: 'max', updatedAt: '', active: true, configDir: 'C:\\Users\\Example\\.claude', source: 'system', status: 'ready', message: '已检测到本地登录，实际可用性由 Claude 确认。', proxy: 'inherit' }
    const state = window.__claudeSmoke = {
      calls: [], failedOnce: false, cancelNextLogin: false, pendingLogin: null, mismatchedSystem: true,
      rows: [system,
        { ...system, id: 'work', name: '工作账号', email: 'work@example.com', active: false, source: 'isolated', configDir: 'C:\\Users\\Example\\AppData\\Roaming\\com.aitools.agentdock\\claude-accounts\\profiles\\work', status: 'refreshable', message: '访问令牌已到期；启动时由 Claude 自动续期。', proxy: 'http://127.0.0.1:7901' },
        { ...system, id: 'legacy', name: '原有个人账号', email: 'personal@example.com', active: false, source: 'legacy', configDir: '', status: 'legacy', message: '旧版凭证快照。登录后转为独立账号，原快照保留。' }]
    }
    const list = () => structuredClone({ loggedIn: true, currentName: state.rows.find(a => a.active)?.name || '', currentEmail: state.rows.find(a => a.active)?.email || '', accounts: state.rows })
    window.__TAURI_INTERNALS__ = { invoke: async (command, args = {}) => {
      state.calls.push({ command, args })
      const row = state.rows.find(a => a.id === args.accountId)
      if (command === 'create_claude_account') state.rows.push({ ...system, id: 'new-account', name: args.name, proxy: args.proxy === '7902' ? 'http://127.0.0.1:7902' : args.proxy, email: '', active: false, source: 'isolated', status: 'signedOut', message: '尚未登录此账号，请点击登录。', configDir: 'C:\\profiles\\new-account' })
      else if (command === 'update_claude_account') Object.assign(row, { name: args.name, proxy: args.proxy })
      else if (command === 'switch_claude_account') state.rows.forEach(a => { a.active = a.id === args.accountId })
      else if (command === 'delete_claude_account') state.rows = state.rows.filter(a => a.id !== args.accountId)
      else if (command === 'login_claude_account') {
        if (state.cancelNextLogin) {
          state.cancelNextLogin = false
          return new Promise((_, reject) => { state.pendingLogin = reject })
        }
        row.source = 'isolated'; row.configDir ||= 'C:\\profiles\\legacy'
        if (!state.failedOnce) {
          state.failedOnce = true; row.status = 'unverified'; row.message = '登录尚未通过身份检查，请点击检查登录。'
          throw new Error('模拟：身份检查暂时无法连接，请重试。')
        }
        row.status = 'ready'; row.message = '已检测到本地登录。'
      } else if (command === 'verify_claude_account') {
        if (row.source === 'system' && state.mismatchedSystem) {
          row.status = 'conflict'; row.message = '系统缓存身份与实际登录凭证不一致，凭证实际属于 ac***@gmx.es。'
          throw new Error('账号信息与登录凭据不属于同一账号，已停止操作。')
        }
        row.status = 'ready'; row.email ||= 'new@example.com'; row.message = '已检测到本地登录。'
      }
      else if (command === 'cancel_claude_login') { state.pendingLogin?.(new Error('已取消 Claude 登录')); state.pendingLogin = null; return }
      else if (command === 'clipboard_write') return
      else if (command !== 'list_claude_accounts') throw new Error(`Unexpected command: ${command}`)
      return list()
    } }
    // Use the exact Vite dependency URL to share Vue's renderer/reactivity
    // instance with the compiled SFC, including its dependency version query.
    const source = await (await fetch('/src/components/ClaudeAccountDialog.vue')).text()
    const vueUrl = source.match(/from ["']([^"']*\/vue\.js[^"']*)["']/)?.[1]
    if (!vueUrl) throw new Error('Could not resolve the component Vue runtime')
    const { createApp, h, ref } = await import(vueUrl)
    const { default: Dialog } = await import('/src/components/ClaudeAccountDialog.vue')
    const { modal } = await import('/src/lib/modal.ts')
    await import('/src/styles/global.css')
    const open = ref(false)
    createApp({ render: () => h(Dialog, { open: open.value, onClose: () => { open.value = false }, onSwitched: () => state.calls.push({ command: 'switched-event' }) }) }).directive('modal', modal).mount('#app')
    open.value = true
  })
  const dialog = page.getByRole('dialog', { name: 'Claude 账号' })
  await dialog.waitFor()
  await page.getByRole('button', { name: /工作账号/ }).click()
  await page.screenshot({ path: '.playwright-cli/claude-accounts-dark.png' })
  await page.getByRole('button', { name: /添加账号/ }).click()
  await page.getByLabel(/账号名称/).fill('研发账号')
  await page.getByLabel('网络代理', { exact: true }).selectOption('custom')
  await page.getByLabel(/代理地址/).fill('7902')
  await page.getByRole('button', { name: '创建账号', exact: true }).click()
  await page.getByRole('heading', { name: '研发账号' }).waitFor()
  await page.getByRole('button', { name: '登录账号', exact: true }).click()
  await page.getByRole('button', { name: '检查登录', exact: true }).waitFor()
  await page.getByRole('button', { name: '检查登录', exact: true }).click()
  await page.getByRole('button', { name: '设为默认账号', exact: true }).click()
  await page.getByRole('button', { name: '已设为默认', exact: true }).waitFor()
  await page.locator('[role="dialog"][aria-busy="false"]').waitFor()
  await page.evaluate(() => { document.documentElement.dataset.theme = 'light' })
  await page.screenshot({ path: '.playwright-cli/claude-accounts-light.png' })
  await page.evaluate(() => { window.__claudeSmoke.cancelNextLogin = true })
  await page.getByRole('button', { name: '重新登录', exact: true }).click()
  await page.getByRole('button', { name: '取消登录', exact: true }).click()
  await page.getByRole('alert').filter({ hasText: '已取消' }).waitFor()
  await page.getByRole('button', { name: /原有个人账号/ }).click()
  await page.getByRole('button', { name: '迁移并登录', exact: true }).click()
  await page.getByRole('button', { name: '设为默认账号', exact: true }).waitFor()
  await page.getByRole('button', { name: '移出账号列表', exact: true }).click()
  await page.getByRole('button', { name: '移出列表', exact: true }).click()
  await page.getByRole('button', { name: /系统 Claude/ }).click()
  await page.getByRole('button', { name: '检查登录', exact: true }).click()
  await page.getByText('系统缓存身份与实际登录凭证不一致，凭证实际属于 ac***@gmx.es。', { exact: true }).waitFor()
  if (await page.getByRole('button', { name: '设为默认账号', exact: true }).count()) throw new Error('A mismatched system identity can still be selected')
  await page.getByLabel('搜索账号', { exact: true }).fill('no-such-account')
  await page.getByText('未找到匹配的账号', { exact: true }).waitFor()
  await page.getByLabel('搜索账号', { exact: true }).fill('')
  await page.setViewportSize({ width: 680, height: 760 })
  const fits = await dialog.evaluate(el => el.scrollWidth <= el.clientWidth && el.getBoundingClientRect().right <= innerWidth)
  if (!fits) throw new Error('Dialog overflows at narrow viewport')
  await page.screenshot({ path: '.playwright-cli/claude-accounts-narrow.png' })
  const calls = await page.evaluate(() => window.__claudeSmoke.calls)
  if (calls.some(c => c.command === 'pty_kill')) throw new Error('Changing default killed an existing terminal')
  if (!calls.some(c => c.command === 'create_claude_account' && c.args.proxy === '7902')) throw new Error('Custom proxy was lost')
  if (errors.length) throw new Error(errors.join('\n'))
  await page.keyboard.press('Escape')
  await dialog.waitFor({ state: 'hidden' })
  return { ok: true, commands: calls.map(c => c.command), screenshots: ['dark', 'light', 'narrow'] }
}
