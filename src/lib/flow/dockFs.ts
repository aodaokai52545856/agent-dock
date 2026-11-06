import * as api from '../api.ts'
import type { DockFs } from './taskRun.ts'

export type ProjectDockFs = DockFs & {
  list(dir: string): Promise<string[]>
  remove(rel: string): Promise<void>
}

const preview = new Map<string, string>()

function previewFs(projectId: string): ProjectDockFs {
  const key = (rel: string) => `${projectId}/${rel}`
  return {
    async read(rel) {
      const text = preview.get(key(rel))
      return text === undefined ? null : { text, sha256: text }
    },
    async write(rel, text) {
      preview.set(key(rel), text)
      return text
    },
    async list(dir) {
      const prefix = key(`${dir}/`)
      return [...preview.keys()].filter((item) => item.startsWith(prefix)).map((item) => item.slice(prefix.length))
    },
    async remove(rel) {
      preview.delete(key(rel))
    }
  }
}

export function dockFs(projectId: string): ProjectDockFs {
  if (!api.isTauri) return previewFs(projectId)
  return {
    read: (rel) => api.dockFileRead(projectId, rel),
    write: (rel, text) => api.dockFileWrite(projectId, rel, text),
    list: (dir) => api.dockFileList(projectId, dir),
    remove: (rel) => api.dockFileRemove(projectId, rel)
  }
}
