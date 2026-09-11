import { spawn } from 'node:child_process'
import { createInterface } from 'node:readline'

const cwdFilter = process.argv[2] || process.cwd()
const child = spawn('codex', ['app-server'], {
  stdio: ['pipe', 'pipe', 'pipe'],
  shell: process.platform === 'win32'
})

const pending = new Map()
let nextId = 1

const rl = createInterface({ input: child.stdout })
rl.on('line', (line) => {
  if (!line.trim()) return
  let msg
  try {
    msg = JSON.parse(line)
  } catch {
    return
  }
  if (msg.id != null && pending.has(msg.id)) {
    const { resolve, reject } = pending.get(msg.id)
    pending.delete(msg.id)
    if (msg.error) reject(new Error(JSON.stringify(msg.error)))
    else resolve(msg.result)
  }
})

function send(method, params, timeoutMs = 25000) {
  const id = nextId++
  const payload = { method, id }
  if (params !== undefined) payload.params = params
  child.stdin.write(`${JSON.stringify(payload)}\n`)
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      pending.delete(id)
      reject(new Error(`timeout ${method}`))
    }, timeoutMs)
    pending.set(id, {
      resolve: (value) => {
        clearTimeout(timer)
        resolve(value)
      },
      reject: (err) => {
        clearTimeout(timer)
        reject(err)
      }
    })
  })
}

function notify(method, params = {}) {
  child.stdin.write(`${JSON.stringify({ method, params })}\n`)
}

function norm(value) {
  return String(value || '')
    .replace(/\\/g, '/')
    .replace(/\/+$/, '')
    .toLowerCase()
}

async function listAll(params) {
  const rows = []
  let cursor = null
  for (let i = 0; i < 8; i += 1) {
    const result = await send('thread/list', { limit: 50, sortKey: 'updated_at', ...params, cursor })
    rows.push(...(result?.data ?? []))
    cursor = result?.nextCursor || null
    if (!cursor) break
  }
  return rows
}

const result = {
  binary: 'codex',
  cwd: cwdFilter,
  initialized: false,
  writerSafe: true,
  queries: {}
}

try {
  const init = await send('initialize', {
    clientInfo: { name: 'agent-dock-spike', title: 'Agent Dock spike', version: '0.1.0' }
  })
  result.initialized = true
  result.platformOs = init?.platformOs ?? null
  notify('initialized')

  const kinds = ['cli', 'vscode', 'appServer', 'exec', 'unknown']
  const all = await listAll({ sourceKinds: kinds })
  const byCwd = await listAll({
    sourceKinds: kinds,
    cwd: [cwdFilter, cwdFilter.replace(/\\/g, '/')]
  })
  const appServer = await listAll({ sourceKinds: ['appServer'] })
  const noKinds = await listAll({})
  const archived = await listAll({ sourceKinds: kinds, archived: true })
  const want = norm(cwdFilter)
  const match = (row) => {
    const cwd = row.cwd || row.gitInfo?.cwd || ''
    return cwd && (norm(cwd) === want || norm(cwd).includes(want) || want.includes(norm(cwd)))
  }
  result.queries = {
    all: {
      count: all.length,
      kinds: [...new Set(all.map((row) => row.sourceKind || row.source || 'unknown'))],
      cwdMatched: all.filter(match).length
    },
    serverCwdFilter: {
      count: byCwd.length,
      sample: byCwd.slice(0, 5).map((row) => ({
        id: row.id,
        name: row.name || row.preview,
        cwd: row.cwd || null,
        sourceKind: row.sourceKind || row.source || null
      }))
    },
    appServerOnly: {
      count: appServer.length,
      sample: appServer.slice(0, 5).map((row) => ({
        id: row.id,
        name: row.name || row.preview,
        cwd: row.cwd || null
      }))
    },
    matchedSample: all.filter(match).slice(0, 8).map((row) => ({
      id: row.id,
      name: row.name || row.preview,
      cwd: row.cwd || row.gitInfo?.cwd || null,
      sourceKind: row.sourceKind || row.source || null
    })),
    uniqueCwds: [...new Set(all.map((row) => row.cwd || row.gitInfo?.cwd || ''))].filter(Boolean).slice(0, 20),
    defaultKinds: {
      count: noKinds.length,
      kinds: [...new Set(noKinds.map((row) => row.sourceKind || row.source || 'unknown'))]
    },
    archived: {
      count: archived.length,
      kinds: [...new Set(archived.map((row) => row.sourceKind || row.source || 'unknown'))]
    }
  }
  result.note = 'thread/list only; no thread/resume. Dock reviews use detached delivery.'
  console.log(JSON.stringify(result, null, 2))
} catch (err) {
  result.note = err instanceof Error ? err.message : String(err)
  console.log(JSON.stringify(result, null, 2))
  process.exitCode = 1
} finally {
  child.kill()
}
