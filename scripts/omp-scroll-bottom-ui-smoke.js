// playwright-cli run-code against local Vite on port 1423.
// Uses real xterm and synthetic IPC only; no model requests or credentials.
async page => {
  const errors = []
  page.on('pageerror', error => errors.push(error.message))
  const jump = page.getByRole('button', { name: '滚动到底部', exact: true })
  await page.setViewportSize({ width: 1200, height: 760 })
  await page.route('**/omp-jump-native', route => route.fulfill({ contentType: 'text/html', body: '<!doctype html><html><body style="margin:0"><div id="app" style="height:100vh;display:flex;flex-direction:column"></div></body></html>' }))
  await page.goto('http://127.0.0.1:1423/omp-jump-native')
  await page.evaluate(async () => {
    const callbacks = new Map(), listeners = new Map()
    let serial = 0
    const smoke = window.__jumpNative = { sizes: [], input: [], repaint: null }
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} }
    window.__TAURI_INTERNALS__ = {
      transformCallback: fn => { callbacks.set(++serial, fn); return serial },
      unregisterCallback: id => callbacks.delete(id),
      invoke: async (command, args) => {
        if (command === 'plugin:event|listen') { listeners.set(args.event, callbacks.get(args.handler)); return args.handler }
        if (command === 'plugin:event|unlisten') return
        if (command === 'pty_write') { smoke.input.push(args.data); return }
        if (command === 'pty_resize') {
          smoke.sizes.push([args.cols, args.rows])
          if (smoke.repaint?.cols === args.cols) {
            const data = smoke.repaint.data; smoke.repaint = null; smoke.write(data)
          }
          return
        }
        throw new Error('Unexpected IPC ' + command)
      }
    }
    const source = await (await fetch('/src/components/TerminalPane.vue')).text()
    const { createApp, h, ref, nextTick } = await import(source.match(/from ["']([^"']*\/vue\.js[^"']*)["']/)[1])
    const { store } = await import(source.match(/from ["']([^"']*\/lib\/store\.ts[^"']*)["']/)[1])
    const { default: Pane } = await import('/src/components/TerminalPane.vue')
    const { default: Strip } = await import('/src/components/LaunchStrip.vue')
    await import('/src/styles/global.css')
    document.documentElement.dataset.theme = 'dark'
    const pane = ref(null)
    createApp({ render: () => [h(Strip, { onScrollBottom: () => pane.value.scrollToBottom() }), h(Pane, { ref: pane, fontSize: 14 })] }).mount('#app')
    await nextTick()
    store.live = [{ ptyId: 'native', toolId: 'omp', kind: 'pty', projectId: 'fixture', key: 'fixture', alive: true, title: 'OMP scroll fixture' }]
    store.activePtyId = 'native'
    await nextTick()
    smoke.host = pane.value.ensureHost('native'); smoke.store = store
    smoke.write = data => listeners.get('pty-data')({ payload: { ptyId: 'native', data } })
    smoke.screen = () => {
      const term = smoke.host.term, buffer = term.buffer.active
      return Array.from({ length: term.rows }, (_, i) => buffer.getLine(buffer.viewportY + i)?.translateToString(true) || '').join('\n')
    }
  })
  await jump.waitFor()
  await page.waitForTimeout(300)
  await page.evaluate(() => window.__jumpNative.write('Historical answer row\r\n'.repeat(180)))
  await page.waitForFunction(() => window.__jumpNative.host.term.buffer.active.baseY > 100)
  await page.mouse.move(500, 300)
  await page.mouse.wheel(0, -10000)
  await page.waitForFunction(() => !window.__jumpNative.host.follow.isFollowing())
  await page.evaluate(() => {
    const s = window.__jumpNative
    s.sizes.length = 0
    s.host.term.select(0, 0, 10)
    s.write('SHORT_ANSWER_PREFIX\r\n')
    s.repaint = { cols: s.host.term.cols, data: '\r\nSHORT_ANSWER_TAIL\r\nDRAFT_PRESERVED' }
  })
  await jump.click()
  await page.waitForFunction(() => {
    const s = window.__jumpNative, b = s.host.term.buffer.active
    return s.host.follow.isFollowing() && b.viewportY === b.baseY && !s.host.term.hasSelection()
      && s.screen().includes('SHORT_ANSWER_TAIL') && s.screen().includes('DRAFT_PRESERVED')
  })
  await page.waitForTimeout(400)
  const sizes = await page.evaluate(() => window.__jumpNative.sizes.length)
  if (sizes !== 2) throw new Error('Native jump did not recover the parked viewport exactly once')
  await jump.click()
  await jump.click()
  await page.waitForTimeout(1700)
  if (await page.evaluate(() => window.__jumpNative.sizes.length) !== sizes) throw new Error('Repeated jump repainted unchanged output')
  await page.evaluate(() => window.__jumpNative.write('\r\nAFTER_JUMP_LIVE_TAIL\r\n'))
  await page.waitForFunction(() => window.__jumpNative.screen().includes('AFTER_JUMP_LIVE_TAIL'))
  if (await page.evaluate(() => window.__jumpNative.input.length)) throw new Error('Native jump sent terminal input')
  await page.screenshot({ path: '.playwright-cli/omp-native-scroll-bottom.png' })
  await page.evaluate(() => { window.__jumpNative.store.live[0].toolId = 'codex' })
  if (await jump.count()) throw new Error('OMP jump is shown for a different tool')

  await page.route('**/omp-jump-wrapped', route => route.fulfill({ contentType: 'text/html', body: '<!doctype html><html><body style="margin:0"><div id="app" style="height:100vh;display:flex;flex-direction:column"></div></body></html>' }))
  await page.goto('http://127.0.0.1:1423/omp-jump-wrapped')
  await page.evaluate(async () => {
    const snapshot = { runtimeId: 'wrapped', seq: 1, profileId: 'fixture',
      info: { ptyId: 'wrapped', runtimeId: 'wrapped', projectId: 'fixture', toolId: 'omp', sessionId: 'fixture', title: 'OMP scroll fixture', alive: true, kind: 'ompRpc' },
      state: { model: { id: 'mock', provider: 'local' }, thinkingLevel: 'medium', queues: {} },
      settled: true, promptResults: {}, dialogs: [], tools: {}, subagents: {}, notifications: [],
      messages: Array.from({ length: 80 }, (_, i) => ({ role: i % 2 ? 'assistant' : 'user', content: '历史消息 ' + i })) }
    const smoke = window.__jumpWrapped = { snapshot, calls: [] }
    window.__TAURI_INTERNALS__ = { invoke: async (command, args) => {
      smoke.calls.push({ command, args })
      if (command === 'omp_snapshot') return structuredClone(snapshot)
      if (command === 'omp_command') {
        const type = args.command.type
        return { id: 'fixture', success: true, data: type === 'get_available_models' ? { models: [{ id: 'mock', provider: 'local' }] } : { levels: ['medium'] } }
      }
      throw new Error('Unexpected IPC ' + command)
    } }
    const source = await (await fetch('/src/components/OmpPane.vue')).text()
    const { createApp, h, ref } = await import(source.match(/from ["']([^"']*\/vue\.js[^"']*)["']/)[1])
    const { store } = await import('/src/lib/store.ts')
    const { ompState, ompSnapshot } = await import('/src/lib/omp.ts')
    const { default: Pane } = await import('/src/components/OmpPane.vue')
    const { default: Strip } = await import('/src/components/LaunchStrip.vue')
    await import('/src/styles/global.css')
    document.documentElement.dataset.theme = 'light'
    store.live = [snapshot.info]; store.activePtyId = 'wrapped'
    ompState.drafts.wrapped = '保留这段未发送的草稿'
    smoke.refresh = () => ompSnapshot('wrapped')
    const pane = ref(null)
    createApp({ render: () => [h(Strip, { onScrollBottom: () => pane.value.scrollToBottom() }),
      h('div', { style: { flex: '1', minHeight: '0' } }, [h(Pane, { ref: pane, runtimeId: 'wrapped' })])] }).mount('#app')
  })
  await page.setViewportSize({ width: 900, height: 620 })
  await page.locator('.omp-message').last().waitFor()
  await page.locator('.omp-transcript').evaluate(el => { el.scrollTop = 0 })
  await page.evaluate(async () => {
    const s = window.__jumpWrapped
    s.snapshot.messages.push({ role: 'assistant', content: '跳转前的新消息' }); s.snapshot.seq++; await s.refresh()
  })
  if (await page.locator('.omp-transcript').evaluate(el => el.scrollTop > 1)) throw new Error('New output interrupted history reading')
  await jump.click()
  await page.waitForFunction(() => { const el = document.querySelector('.omp-transcript'); return el.scrollHeight - el.clientHeight - el.scrollTop < 2 })
  await page.evaluate(async () => {
    const s = window.__jumpWrapped
    s.snapshot.messages.push({ role: 'assistant', content: '跳转后的最新消息' }); s.snapshot.seq++; await s.refresh()
  })
  await page.getByText('跳转后的最新消息', { exact: true }).waitFor()
  await page.waitForFunction(() => { const el = document.querySelector('.omp-transcript'); return el.scrollHeight - el.clientHeight - el.scrollTop < 2 })
  await page.getByRole('button', { name: '子代理', exact: true }).click()
  await jump.click()
  await page.waitForFunction(() => { const el = document.querySelector('.omp-transcript'); return el && el.scrollHeight - el.clientHeight - el.scrollTop < 2 })
  if (await page.getByLabel('发送给 OMP').inputValue() !== '保留这段未发送的草稿') throw new Error('Wrapped jump changed the draft')
  if (await page.evaluate(() => window.__jumpWrapped.calls.some(c => c.command === 'omp_command' && c.args.command.type === 'prompt'))) throw new Error('Wrapped jump submitted a prompt')
  if (await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)) throw new Error('Toolbar overflows the narrow viewport')
  await page.screenshot({ path: '.playwright-cli/omp-wrapped-scroll-bottom.png' })
  if (errors.length) throw new Error(errors.join('\n'))
  return { ok: true, cases: ['native-jump', 'clear-selection', 'recover-parked-viewport-once', 'repeated-jump-no-repaint', 'native-follow-after-jump', 'no-terminal-input', 'only-omp', 'wrapped-jump', 'preserve-history', 'wrapped-follow-after-jump', 'return-to-chat', 'preserve-draft', 'no-prompt', 'dark', 'light-narrow'] }
}
