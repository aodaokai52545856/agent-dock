// Local Vite + real xterm. All PTY traffic is synthetic; no model requests.
async page => {
  const errors = []
  page.on('pageerror', e => errors.push(e.message))
  await page.setViewportSize({ width: 1200, height: 760 })
  await page.route('**/omp-scroll-regression', route => route.fulfill({ contentType: 'text/html', body: '<!doctype html><html><body style="margin:0"><div id="app" style="height:100vh;display:flex"></div></body></html>' }))
  await page.goto('http://localhost:1423/omp-scroll-regression')
  await page.evaluate(async () => {
    const callbacks = new Map(), listeners = new Map()
    let serial = 0
    const smoke = window.__scrollRegression = { sizes: [], input: [], repaint: null }
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
    await import('/src/styles/global.css')
    const pane = ref(null)
    createApp({ render: () => h('div', { style: { display: store.appMode === 'console' ? 'flex' : 'none', flex: '1', minWidth: '0', minHeight: '0' } }, [h(Pane, { ref: pane, fontSize: 14 })]) }).mount('#app')
    await nextTick()
    smoke.write = data => listeners.get('pty-data')({ payload: { ptyId: 'early-omp', data } })
    // The backend starts emitting before the pty_open response registers live metadata.
    smoke.write('EARLY_NATIVE_OUTPUT\r\n')
    smoke.host = pane.value.ensureHost('early-omp')
    store.live = [{ ptyId: 'early-omp', toolId: 'omp', kind: 'pty', projectId: 'fixture', key: 'fixture', alive: true }]
    store.activePtyId = 'early-omp'
    smoke.pane = pane.value; smoke.store = store
    smoke.screen = () => {
      const term = smoke.host.term, buffer = term.buffer.active
      return Array.from({ length: term.rows }, (_, i) => buffer.getLine(buffer.viewportY + i)?.translateToString(true) || '').join('\n')
    }
    await nextTick()
  })
  await page.waitForTimeout(700)
  if (!await page.evaluate(() => Boolean(window.__scrollRegression.host.recovery))) throw new Error('Early PTY output permanently skipped OMP recovery initialization')
  await page.evaluate(() => { window.__scrollRegression.write('idle row\r\n'.repeat(120) + 'IDLE_LAST_CONTENT\r\n') })
  await page.waitForTimeout(150)
  await page.evaluate(() => {
    const s = window.__scrollRegression
    Object.defineProperty(document, 'hidden', { configurable: true, value: true })
    window.dispatchEvent(new Event('blur'))
    s.host.term.scrollToTop()
    s.write('\x1b[?1049hTEMPORARY_NATIVE_REPAINT\x1b[?1049l')
  })
  await page.waitForTimeout(100)
  await page.evaluate(() => {
    delete document.hidden
    document.dispatchEvent(new Event('visibilitychange'))
    window.dispatchEvent(new Event('focus'))
  })
  await page.waitForTimeout(500)
  if (!await page.evaluate(() => window.__scrollRegression.host.follow.isFollowing() && window.__scrollRegression.screen().includes('IDLE_LAST_CONTENT'))) throw new Error('Idle buffer restore was mistaken for a user scroll and returned to the session beginning')
  // A TUI can displace the xterm viewport while writing, without a user scroll.
  await page.evaluate(() => {
    const s = window.__scrollRegression
    s.write('history\r\n'.repeat(150))
  })
  await page.waitForTimeout(150)
  await page.evaluate(() => { const s = window.__scrollRegression; s.host.term.scrollToTop(); s.write('LIVE_STREAM_TAIL\r\n') })
  await page.waitForTimeout(150)
  if (!await page.evaluate(() => window.__scrollRegression.screen().includes('LIVE_STREAM_TAIL'))) throw new Error('OMP output stopped following the live viewport')
  // Hidden output must resume following after returning to the session.
  await page.evaluate(() => { window.__scrollRegression.store.activePtyId = '' })
  await page.waitForTimeout(60)
  await page.evaluate(() => { const s = window.__scrollRegression; s.host.term.scrollToTop(); s.write('BACKGROUND_OUTPUT_TAIL\r\n') })
  await page.waitForTimeout(60)
  await page.evaluate(() => { window.__scrollRegression.store.activePtyId = 'early-omp' })
  await page.waitForTimeout(700)
  if (!await page.evaluate(() => window.__scrollRegression.screen().includes('BACKGROUND_OUTPUT_TAIL'))) throw new Error('Returning to OMP kept a stale viewport')
  // Switching Dock tabs keeps activePtyId unchanged and hides the whole pane.
  await page.evaluate(() => { window.__scrollRegression.store.appMode = 'usage' })
  await page.waitForTimeout(60)
  await page.evaluate(() => { const s = window.__scrollRegression; s.host.term.scrollToTop(); s.write('DOCK_TAB_BACKGROUND_TAIL\r\n') })
  await page.waitForTimeout(60)
  await page.evaluate(() => { window.__scrollRegression.store.appMode = 'console' })
  await page.waitForTimeout(700)
  if (!await page.evaluate(() => window.__scrollRegression.screen().includes('DOCK_TAB_BACKGROUND_TAIL'))) throw new Error('Returning from another Dock tab kept a stale viewport')
  // Deliberately reading history must remain stable during subsequent output.
  await page.mouse.move(500, 300)
  await page.mouse.wheel(0, -10000)
  await page.waitForTimeout(100)
  await page.evaluate(() => { const s = window.__scrollRegression; s.sizes.length = 0; s.write('WHILE_READING_HISTORY\r\n') })
  await page.waitForTimeout(500)
  if (!await page.evaluate(() => { const b = window.__scrollRegression.host.term.buffer.active; return b.viewportY < b.baseY })) throw new Error('Following output interrupted manual history reading')
  if (await page.evaluate(() => window.__scrollRegression.sizes.length)) throw new Error('Recovery interrupted manual history reading')
  // A short answer and periodic cursor controls used to prevent recovery forever.
  await page.evaluate(() => {
    const s = window.__scrollRegression
    s.repaint = { cols: s.host.term.cols, data: '\r\nSHORT_ANSWER_RECOVERED\r\nDRAFT_PRESERVED' }
    s.noise = setInterval(() => s.write('\x1b[?25h'), 50)
  })
  await page.mouse.wheel(0, 10000)
  await page.waitForFunction(() => window.__scrollRegression.screen().includes('SHORT_ANSWER_RECOVERED'), undefined, { timeout: 4000 })
  await page.evaluate(() => clearInterval(window.__scrollRegression.noise))
  await page.waitForTimeout(900)
  if (await page.evaluate(() => window.__scrollRegression.sizes.length) !== 2) throw new Error('Bottom recovery looped or changed more than one size round trip')
  if (await page.evaluate(() => window.__scrollRegression.input.length)) throw new Error('Scroll recovery sent terminal input')
  // A free-spinning wheel keeps sending events well beyond the cooldown.
  // Neither repeated bottom events nor terminal housekeeping may repaint again.
  const repairedSizes = await page.evaluate(() => {
    const s = window.__scrollRegression
    s.wheelNoise = setInterval(() => s.write('\x1b[?25h\x1b[0m\x1b]0;OMP\x07'), 40)
    return s.sizes.length
  })
  for (let i = 0; i < 100; i++) {
    await page.mouse.wheel(0, 600)
    await page.waitForTimeout(20)
  }
  await page.evaluate(() => clearInterval(window.__scrollRegression.wheelNoise))
  await page.waitForTimeout(400)
  if (await page.evaluate(() => window.__scrollRegression.sizes.length) !== repairedSizes) throw new Error('Free-spinning at the bottom repeatedly repainted an unchanged answer')
  await page.mouse.wheel(0, -10000)
  await page.waitForTimeout(100)
  await page.mouse.wheel(0, 10000)
  await page.waitForTimeout(1700)
  if (await page.evaluate(() => window.__scrollRegression.sizes.length) !== repairedSizes) throw new Error('Returning to an already recovered answer repainted it again')
  // A genuinely new answer is still eligible for one recovery on return.
  await page.mouse.wheel(0, -10000)
  await page.waitForTimeout(100)
  await page.evaluate(() => {
    const s = window.__scrollRegression
    s.write('NEW_SHORT_ANSWER_PREFIX\r\n')
    s.repaint = { cols: s.host.term.cols, data: '\r\nNEW_SHORT_ANSWER_RECOVERED\r\n' }
  })
  await page.mouse.wheel(0, 10000)
  await page.waitForFunction(() => window.__scrollRegression.screen().includes('NEW_SHORT_ANSWER_RECOVERED'))
  await page.waitForTimeout(400)
  if (await page.evaluate(() => window.__scrollRegression.sizes.length) !== repairedSizes + 2) throw new Error('New answer did not recover exactly once')
  // Inactive WebViews can deliver a delayed native buffer restore on return.
  // Buffer activation clears selection even when the user selected nothing.
  await page.evaluate(() => {
    const s = window.__scrollRegression
    s.write('IDLE_HISTORY_ROW\r\n'.repeat(120) + 'IDLE_LAST_CONTENT\r\n')
  })
  await page.waitForTimeout(150)
  await page.evaluate(() => {
    const s = window.__scrollRegression
    Object.defineProperty(document, 'hidden', { configurable: true, value: true })
    window.dispatchEvent(new Event('blur'))
    s.host.term.scrollToTop()
    s.write('\x1b[?1049hTEMPORARY_NATIVE_REPAINT\x1b[?1049l')
  })
  await page.waitForTimeout(100)
  await page.evaluate(() => {
    delete document.hidden
    document.dispatchEvent(new Event('visibilitychange'))
    window.dispatchEvent(new Event('focus'))
  })
  await page.waitForTimeout(500)
  const idle = await page.evaluate(() => {
    const s = window.__scrollRegression
    return { following: s.host.follow.isFollowing(), tailVisible: s.screen().includes('IDLE_LAST_CONTENT') }
  })
  if (!idle.following || !idle.tailVisible) throw new Error('Idle buffer restore was mistaken for a user scroll and returned to the session beginning')
  const bar = await page.evaluate(() => {
    const s = window.__scrollRegression, viewport = s.host.el.querySelector('.xterm-viewport')
    const rect = viewport.getBoundingClientRect()
    return { width: viewport.offsetWidth - viewport.clientWidth, scrollable: viewport.scrollHeight > viewport.clientHeight,
      style: getComputedStyle(viewport).scrollbarWidth, x: rect.right - 3, y: rect.top + 20 }
  })
  if (bar.style === 'none' || bar.width <= 0 || !bar.scrollable) throw new Error('The native terminal scrollbar is still hidden')
  // Use the real browser scrollbar, rather than directly mutating follow state.
  await page.mouse.move(bar.x, bar.y)
  await page.mouse.down()
  await page.waitForTimeout(120)
  await page.mouse.up()
  await page.waitForTimeout(150)
  const savedLine = await page.evaluate(() => {
    const s = window.__scrollRegression, buffer = s.host.term.buffer.active
    if (s.host.follow.isFollowing() || buffer.viewportY >= buffer.baseY) throw new Error('Scrollbar interaction did not pause live following')
    return buffer.viewportY
  })
  await page.evaluate(() => {
    const s = window.__scrollRegression
    window.dispatchEvent(new Event('blur'))
    s.host.term.scrollToTop()
    s.host.term.clearSelection()
    window.dispatchEvent(new Event('focus'))
  })
  await page.waitForTimeout(300)
  if (await page.evaluate(() => window.__scrollRegression.host.term.buffer.active.viewportY) !== savedLine) throw new Error('Returning to the window lost the saved history position')
  await page.mouse.move(500, 300)
  await page.mouse.wheel(0, 10000)
  await page.waitForTimeout(150)
  if (!await page.evaluate(() => window.__scrollRegression.host.follow.isFollowing())) throw new Error('Returning from scrollbar history to the bottom did not resume following')
  if (errors.length) throw new Error(errors.join('\n'))
  return { ok: true, cases: ['output-before-metadata', 'follow-streaming-output', 'resume-hidden-output', 'resume-dock-tab-output', 'preserve-manual-history', 'short-answer-bottom-recovery', 'periodic-control-output', 'no-repaint-loop', 'free-spinning-wheel-no-repaint', 'deduplicate-answer-recovery', 'recover-new-short-answer-once', 'idle-buffer-restore', 'visible-scrollbar', 'scrollbar-pauses-follow', 'restore-idle-history-marker'] }
}
