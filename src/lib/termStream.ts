// xterm parses write() asynchronously. Resizing it ahead of an older frame
// interprets that frame against the wrong grid (especially full-screen TUIs).
export function createTermStream(write: (data: string, done: () => void) => void) {
  type ResizeJob = { kind: 'resize'; run: () => void | Promise<void>; resolve: () => void; reject: (error: unknown) => void }
  type Job = { kind: 'data'; data: string } | ResizeJob
  const queue: Job[] = []
  let pumping = false
  let disposed = false

  async function pump() {
    if (pumping || disposed) return
    pumping = true
    try {
      while (!disposed && queue.length) {
        const job = queue.shift()!
        if (job.kind === 'data') {
          await new Promise<void>(resolve => write(job.data, resolve))
        } else {
          try { await job.run(); job.resolve() } catch (error) { job.reject(error) }
        }
      }
    } finally { pumping = false }
  }

  return {
    write(data: string) {
      if (disposed || !data) return
      const last = queue.at(-1)
      if (last?.kind === 'data' && last.data.length + data.length <= 256 * 1024) last.data += data
      else queue.push({ kind: 'data', data })
      void pump()
    },
    resize(run: () => void | Promise<void>) {
      if (disposed) return Promise.resolve()
      return new Promise<void>((resolve, reject) => {
        queue.push({ kind: 'resize', run, resolve, reject })
        void pump()
      })
    },
    dispose() {
      disposed = true
      for (const job of queue) if (job.kind === 'resize') job.resolve()
      queue.length = 0
    }
  }
}
