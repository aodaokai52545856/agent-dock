import { readFileSync } from 'node:fs'

const inputPath = process.argv[2]
if (!inputPath) {
  fail('缺少任务文件')
}

const input = JSON.parse(readFileSync(inputPath, 'utf8'))
const apiKey = String(input.apiKey || process.env.CURSOR_API_KEY || '').trim()
const cwd = String(input.cwd || '').trim()
const prompt = String(input.prompt || '').trim()
const agentId = input.agentId ? String(input.agentId) : ''

if (!apiKey) fail('缺少 CURSOR_API_KEY')
if (!cwd) fail('缺少项目目录')
if (!prompt) fail('本轮任务不能为空')

const sdk = await loadSdk()
const Agent = sdk.Agent
if (!Agent) fail('当前 @cursor/sdk 没有 Agent 导出')

let agent
try {
  if (agentId) {
    agent = await Agent.resume(agentId, { apiKey, local: { cwd } })
  } else {
    agent = await Agent.create({
      apiKey,
      model: { id: 'composer-2.5' },
      local: { cwd }
    })
  }
  const run = await agent.send(prompt)
  const result = await run.wait()
  const id = agent.agentId || agent.id || agentId || ''
  const text =
    typeof result?.result === 'string'
      ? result.result
      : typeof result?.text === 'string'
        ? result.text
        : JSON.stringify(result ?? {})
  console.log(
    JSON.stringify({
      ok: result?.status !== 'error',
      agentId: id,
      status: result?.status || 'finished',
      text
    })
  )
} catch (err) {
  fail(err instanceof Error ? err.message : String(err))
} finally {
  try {
    if (agent?.[Symbol.asyncDispose]) await agent[Symbol.asyncDispose]()
    else if (typeof agent?.close === 'function') await agent.close()
  } catch {
    /* ignore dispose */
  }
}

async function loadSdk() {
  try {
    return await import('@cursor/sdk')
  } catch (first) {
    try {
      return await import(new URL('../node_modules/@cursor/sdk/dist/esm/index.js', import.meta.url).href)
    } catch {
      fail(
        `无法加载 @cursor/sdk。请在 agent-dock 目录执行 npm install。${
          first instanceof Error ? first.message : String(first)
        }`
      )
    }
  }
}

function fail(message) {
  console.error(message)
  process.exit(1)
}
