import { listen } from '@tauri-apps/api/event'
import * as api from './api'
import { createVersionCenter } from './versionCenter'
import { refreshSessions, setToolProbes, showToast } from './store'
import type { VersionTaskSnapshot } from './types'

export const versionCenter = createVersionCenter({
  ...api,
  settled(tasks) {
    const failed = tasks.filter(task => task.status === 'failed').length
    const ready = tasks.some(task => task.status === 'ready')
    const installing = tasks.some(task => task.restart)
    const text = failed ? `${failed} 项任务失败，其余任务已处理，可在版本面板查看日志并重试`
      : ready ? 'Agent Dock 更新已下载，可在版本面板安装' : installing ? '正在安装 Agent Dock，客户端即将退出' : `${tasks.length} 项版本任务已完成`
    showToast(text)
    if (api.isTauri && !document.hasFocus()) void api.notifyDesktop('版本管理', text)
    if (api.isTauri && tasks.some(task => task.target !== 'app')) {
      void api.probeTools().then(setToolProbes).catch(() => {})
      void refreshSessions({ silent: true })
    }
  }
})

let stop: (() => void) | undefined
let starting: Promise<void> | undefined
export function startVersionService() {
  if (starting) return starting
  starting = (async () => {
    let unlisten: (() => void) | undefined
    if (api.isTauri) {
      try { unlisten = await listen<VersionTaskSnapshot>('version-tasks', event => versionCenter.accept(event.payload)) }
      catch { /* Polling remains available if event registration fails. */ }
    }
    await Promise.all([versionCenter.init(), versionCenter.sync()])
    const timer = window.setInterval(() => { if (versionCenter.running.value) void versionCenter.sync() }, 2000)
    stop = () => { unlisten?.(); window.clearInterval(timer); stop = undefined; starting = undefined }
  })()
  return starting
}
export function stopVersionService() { stop?.() }
