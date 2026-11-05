// Run with playwright-cli run-code against the local Vite server on port 1423.
// Uses real xterm parsing and synthetic IPC; no provider/model requests.
async page => {
  const errors = []
  page.on('pageerror', error => errors.push(error.message))
  await page.setViewportSize({ width: 1400, height: 900 })
  await page.route('**/omp-native-terminal-smoke', route => route.fulfill({
    contentType: 'text/html',
    body: '<!doctype html><html><head><meta charset="utf-8"></head><body style="margin:0"><div id="app" style="height:100vh;display:flex;flex-direction:column"></div></body></html>'
  }))
  await page.goto('http://localhost:1423/omp-native-terminal-smoke')
  const initial = await page.evaluate(async () => {
    const callbacks = new Map(), listeners = new Map()
    let serial = 0
    const smoke = window.__nativeSmoke = { log: [], hold: false, release: null, write: null, pane: null, host: null }
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} }
    window.__TAURI_INTERNALS__ = {
      transformCallback: fn => { callbacks.set(++serial, fn); return serial },
      unregisterCallback: id => callbacks.delete(id),
      invoke: async (command, args) => {
        if (command === 'plugin:event|listen') { listeners.set(args.event, callbacks.get(args.handler)); return args.handler }
        if (command === 'plugin:event|unlisten') return
        if (command === 'pty_write') { smoke.log.push({ kind: 'input', data: args.data }); return }
        if (command === 'pty_resize') {
          smoke.log.push({ kind: 'resize', cols: args.cols, rows: args.rows })
          if (smoke.hold) await new Promise(resolve => { smoke.release = resolve })
          if (smoke.repaint?.cols === args.cols) {
            const data = smoke.repaint.data
            smoke.repaint = null
            smoke.write(data)
          }
          return
        }
        throw new Error('Unexpected IPC ' + command)
      }
    }
    const source = await (await fetch('/src/components/TerminalPane.vue')).text()
    const vueUrl = source.match(/from ["']([^"']*\/vue\.js[^"']*)["']/)[1]
    const { createApp, h, ref, nextTick } = await import(vueUrl)
    const { default: Pane } = await import('/src/components/TerminalPane.vue')
    const { default: Strip } = await import('/src/components/LaunchStrip.vue')
    const storeUrl = source.match(/from ["']([^"']*\/lib\/store\.ts[^"']*)["']/)[1]
    const { store } = await import(storeUrl)
    await import('/src/styles/global.css')
    const pane = ref(null)
    createApp({ render: () => [h(Strip, { onRedraw: () => pane.value.redrawActive() }), h(Pane, { ref: pane, fontSize: 14 })] }).mount('#app')
    await nextTick()
    store.live = [{ ptyId: 'native', toolId: 'omp', kind: 'pty', projectId: 'fixture', key: 'fixture', alive: true, title: 'OMP native fixture' }]
    store.activePtyId = 'native'
    await nextTick()
    smoke.pane = pane.value
    smoke.host = pane.value.ensureHost('native')
    smoke.write = data => listeners.get('pty-data')({ payload: { ptyId: 'native', data } })
    smoke.host.term.onWriteParsed(() => smoke.log.push({ kind: 'parsed' }))
    return { cols: smoke.host.term.cols, rows: smoke.host.term.rows }
  })
  if (initial.cols !== 120 || initial.rows !== 32) throw new Error('Initial frontend grid differs from the spawned PTY')
  await page.waitForTimeout(150)
  await page.evaluate(() => {
    const smoke = window.__nativeSmoke
    smoke.log.length = 0
    smoke.write('\x1b[2J\x1b[HBEFORE_RESIZE' + '\x1b[0m'.repeat(150000))
    document.querySelector('.workspace').style.width = '1000px'
    smoke.pane.fitActive(true)
  })
  await page.waitForFunction(() => window.__nativeSmoke.log.some(item => item.kind === 'resize'))
  const ordered = await page.evaluate(() => {
    const log = window.__nativeSmoke.log
    return log.findIndex(item => item.kind === 'parsed') < log.findIndex(item => item.kind === 'resize')
  })
  if (!ordered) throw new Error('PTY resized ahead of an unparsed native frame')
  await page.evaluate(() => {
    const smoke = window.__nativeSmoke
    smoke.log.length = 0; smoke.hold = true
    document.querySelector('.workspace').style.width = '900px'
    smoke.pane.fitActive(true)
  })
  await page.waitForFunction(() => Boolean(window.__nativeSmoke.release))
  await page.evaluate(() => window.__nativeSmoke.write('\x1b[2J\x1b[HAFTER_RESIZE'))
  await page.waitForTimeout(80)
  if (await page.evaluate(() => window.__nativeSmoke.host.term.buffer.active.getLine(0)?.translateToString(true) === 'AFTER_RESIZE')) {
    throw new Error('Native output parsed before the backend acknowledged resize')
  }
  await page.evaluate(() => { const smoke = window.__nativeSmoke; smoke.hold = false; smoke.release(); smoke.release = null })
  await page.waitForFunction(() => window.__nativeSmoke.host.term.buffer.active.getLine(0)?.translateToString(true) === 'AFTER_RESIZE')
  const grid = await page.evaluate(() => {
    const smoke = window.__nativeSmoke; smoke.log.length = 0
    return { cols: smoke.host.term.cols, rows: smoke.host.term.rows }
  })
  await page.getByRole('button', { name: '重绘终端', exact: true }).click()
  await page.waitForFunction(() => window.__nativeSmoke.log.filter(item => item.kind === 'resize').length === 2)
  const redraw = await page.evaluate(() => ({ log: window.__nativeSmoke.log, cols: window.__nativeSmoke.host.term.cols, rows: window.__nativeSmoke.host.term.rows }))
  const sizes = redraw.log.filter(item => item.kind === 'resize')
  if (sizes[0].cols !== grid.cols + 1 || sizes[1].cols !== grid.cols || redraw.cols !== grid.cols || redraw.rows !== grid.rows) throw new Error('Redraw did not restore the native viewport')
  if (redraw.log.some(item => item.kind === 'input')) throw new Error('Redraw sent input or resubmitted a task')
  // Model ConPTY's parked viewport: the answer tail is withheld until a size
  // round trip asks the native application to publish its current screen.
  await page.waitForTimeout(900)
  await page.evaluate(() => {
    const smoke = window.__nativeSmoke
    smoke.log.length = 0
    const history = 'Native long answer row 中文内容\r\n'.repeat(900)
    smoke.repaint = { cols: smoke.host.term.cols, data: '\x1b[2J\x1b[H' + history + 'FINAL_ANSWER_TAIL\r\nPROMPT_DRAFT_KEPT' }
    smoke.write(history)
  })
  await page.waitForFunction(() => window.__nativeSmoke.host.term.buffer.active.baseY > 100)
  await page.mouse.move(500, 300)
  await page.mouse.wheel(0, -10000)
  await page.waitForTimeout(500)
  if (await page.evaluate(() => window.__nativeSmoke.log.some(item => item.kind === 'resize'))) throw new Error('Recovery interrupted history reading')
  await page.evaluate(() => {
    const button = document.createElement('button')
    button.id = 'scroll-fixture-focus'
    document.body.append(button)
    button.focus()
  })
  await page.mouse.wheel(0, 10000)
  await page.waitForFunction(() => {
    const term = window.__nativeSmoke.host.term, buffer = term.buffer.active
    const screen = Array.from({ length: term.rows }, (_, i) => buffer.getLine(buffer.viewportY + i)?.translateToString(true) || '').join('\n')
    return screen.includes('FINAL_ANSWER_TAIL') && screen.includes('PROMPT_DRAFT_KEPT')
  })
  await page.waitForTimeout(900)
  const recovered = await page.evaluate(() => ({
    log: window.__nativeSmoke.log,
    focused: document.activeElement?.id,
    cols: window.__nativeSmoke.host.term.cols
  }))
  if (recovered.log.filter(item => item.kind === 'resize').length !== 2) throw new Error('Recovery repeated itself after the native repaint')
  if (recovered.log.some(item => item.kind === 'input')) throw new Error('Automatic recovery sent terminal input')
  if (recovered.focused !== 'scroll-fixture-focus') throw new Error('Automatic recovery stole focus')
  if (recovered.cols !== grid.cols) throw new Error('Automatic recovery changed the final grid')
  await page.evaluate(() => {
    const smoke = window.__nativeSmoke
    smoke.log.length = 0
    smoke.write('Pending output\r\n'.repeat(2000))
    smoke.pane.dispose('native')
  })
  await page.waitForTimeout(500)
  if (await page.evaluate(() => window.__nativeSmoke.log.some(item => item.kind === 'resize'))) throw new Error('Closed terminal retained a pending recovery')
  if (errors.length) throw new Error(errors.join('\n'))
  return { ok: true, cases: ['initial-grid', 'frame-before-resize', 'backend-resize-barrier', 'native-redraw-without-input', 'defer-recovery-in-history', 'recover-answer-tail-on-return', 'no-repaint-loop', 'preserve-focus-and-input', 'cancel-on-close'] }
}
