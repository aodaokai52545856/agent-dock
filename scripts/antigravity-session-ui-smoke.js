// Run with playwright-cli run-code against Vite on localhost:1423.
async page => {
  const first = '11111111-1111-4111-8111-111111111111'
  const second = '22222222-2222-4222-8222-222222222222'
  await page.addInitScript(({ first, second }) => {
    const callbacks = new Map(), listeners = new Map()
    let serial = 0
    const projectId = 'fixture'
    const toolId = 'antigravity'
    const account = { id: 'work@example.test', name: 'work@example.test' }
    const rows = [first, second].map((id, i) => ({
      toolId, id, title: i ? '第二个会话' : '第一个会话', updatedAt: Date.now(), cwd: '/fixture', renameKind: 'overlay'
    }))
    const live = ['terminal-a', 'terminal-b'].map(ptyId => ({
      projectId, toolId, ptyId, sessionId: null, key: ptyId, alive: true, title: '新会话', account, openedAt: Date.now()
    }))
    const state = window.__sessionSmoke = { copied: '', opened: [], live, write: (ptyId, data) => {
      listeners.get('pty-data')({ payload: { ptyId, data } })
    }, bind: (ptyId, id) => {
      const info = live.find(item => item.ptyId === ptyId)
      Object.assign(info, { sessionId: id, key: projectId + '|' + toolId + '|' + id })
      listeners.get('antigravity-session-changed')({ payload: structuredClone(info) })
    } }
    localStorage.setItem('ad-session-tool-filter', toolId)
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} }
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: fn => { callbacks.set(++serial, fn); return serial },
      unregisterCallback: id => callbacks.delete(id),
      invoke: async (name, args = {}) => {
        if (name === 'plugin:event|listen') { listeners.set(args.event, callbacks.get(args.handler)); return args.handler }
        if (name === 'load_app_state') return { projects: [{ id: projectId, name: '测试项目', path: '/fixture' }], settings: { sessionToolFilter: toolId }, titleOverlays: {} }
        if (name === 'probe_tools') return { antigravity: { found: true } }
        if (name === 'list_live_ptys') return structuredClone(live)
        if (name === 'list_sessions') return { ok: true, sessions: args.toolId === toolId ? structuredClone(rows) : [] }
        if (name === 'clipboard_write') { state.copied = args.text; return }
        if (name === 'pty_open') {
          state.opened.push(args.sessionId)
          const open = live.find(item => item.sessionId === args.sessionId)
          if (open) return { ...open, reused: true }
          const next = { ...args, ptyId: 'reopened', key: 'reopened', openedAt: Date.now(), alive: true }
          live.push(next)
          return next
        }
        if (name === 'pty_kill') { live.splice(live.findIndex(p => p.ptyId === args.ptyId), 1); return }
        if (name === 'pty_bind_session') throw new Error('Antigravity cannot bind by timestamp')
        if (name === 'list_session_docs' || name === 'list_tool_versions') return []
        return null
      }
    }
  }, { first, second })
  await page.goto('http://localhost:1423/')
  const row = title => page.locator('.row-main').filter({ hasText: title })
  const menu = name => page.getByRole('menuitem', { name, exact: true })
  await row('新会话').first().click({ button: 'right' })
  if (!await menu('复制会话 ID').isDisabled()) throw new Error('Pending terminal exposes a placeholder')
  await page.keyboard.press('Escape')
  await page.evaluate(({ first, second }) => {
    window.__sessionSmoke.bind('terminal-b', second)
    window.__sessionSmoke.bind('terminal-a', first)
    window.__sessionSmoke.write('terminal-a', 'FIRST_TRANSCRIPT\r\n')
    window.__sessionSmoke.write('terminal-b', 'SECOND_TRANSCRIPT\r\n')
  }, { first, second })
  for (const [title, id] of [['第一个会话', first], ['第二个会话', second]]) {
    await row(title).click()
    await page.waitForFunction(id => window.__sessionSmoke.opened.at(-1) === id, id)
    const screen = page.locator('.term-host:not(.is-hidden) .xterm-rows')
    await screen.getByText(id === first ? 'FIRST_TRANSCRIPT' : 'SECOND_TRANSCRIPT', { exact: false }).first().waitFor()
    await row(title).click({ button: 'right' })
    await menu('复制会话 ID').click()
    if (await page.evaluate(() => window.__sessionSmoke.copied) !== id) throw new Error('Copied a different session ID')
  }
  await row('第一个会话').click({ button: 'right' })
  await menu('关闭会话').click()
  await page.getByRole('dialog').getByRole('button', { name: '关闭会话' }).click()
  await page.waitForFunction(() => window.__sessionSmoke.live.length === 1)
  const group = page.locator('.group').filter({ hasText: 'work@example.test' })
  await group.getByText('第一个会话', { exact: true }).waitFor()
  await row('第一个会话').click()
  await page.waitForFunction(id => window.__sessionSmoke.live.some(p => p.ptyId === 'reopened' && p.sessionId === id), first)
  if (await row('第一个会话').count() !== 1 || await row('第二个会话').count() !== 1) throw new Error('History duplicated after reopen')
  return 'Passed pending ID, simultaneous bind, right-click copy, close and exact reopen'
}
