// Run with playwright-cli run-code against Vite on port 1423; synthetic RPC only.
async page => {
  const errors = []
  const onError = error => errors.push(error.message)
  page.on('pageerror', onError)
  await page.setViewportSize({width:1280,height:1000})
  await page.context().grantPermissions(['clipboard-read','clipboard-write'])
  await page.route('**/omp-presentation-smoke', route => route.fulfill({contentType:'text/html',body:'<!doctype html><html><head><meta charset="utf-8"></head><body><div id="app" style="height:100vh"></div></body></html>'}))
  await page.goto('http://127.0.0.1:1423/omp-presentation-smoke')
  await page.evaluate(async () => {
    const command = 'rg -n "concept_wiki|contract_persisted" src/ai_query/orchestrator src/ai_query/understanding'
    const snapshot = {
      runtimeId:'omp-presentation',seq:1,profileId:'default',
      info:{ptyId:'omp-presentation',runtimeId:'omp-presentation',projectId:'project',toolId:'omp',sessionId:'sample',title:'概念业务分层',alive:true,kind:'ompRpc'},
      state:{model:{id:'deepseek',provider:'local',name:'DeepSeek V4.1 Flash'},thinkingLevel:'high',queues:{steering:[],followUp:[]}},
      settled:true,promptResults:{},dialogs:[],tools:{},subagents:{},
      messages:[
        {role:'compactionSummary',summary:'## 已保留的上下文\n\n正在核对概念层、指标层和契约恢复流程。'},
        {role:'user',content:'梳理 concept_wiki 的业务分层，检查路由与契约恢复流程。'},
        {role:'assistant',model:'deepseek-flash',content:[
          {type:'thinking',thinking:'先定位 **路由入口**，再核对 `contract_persisted` 的调用关系。'},
          {type:'text',text:'先核对入口、指标及持久化调用，再整理分层关系。'},
          {type:'toolCall',id:'call-search',name:'bash',arguments:{command,cwd:'D:/projects/aiquery',i:'定位业务入口与契约引用',async:false,timeout:30}}
        ]},
        {role:'toolResult',toolCallId:'call-search',content:[{type:'text',text:'\x1b[36msrc/ai_query/orchestrator/concept_route.py\x1b[0m:20: from ai_query.concept_wiki.attainment import build_attainment\n\x1b[36msrc/ai_query/orchestrator/pipeline.py\x1b[0m:4563: "concept_wiki.contract_restored"\n\x1b[36msrc/ai_query/orchestrator/pipeline.py\x1b[0m:6835: "concept_wiki.contract_persisted"'}],isError:false},
        {role:'assistant',model:'deepseek-flash',content:[{type:'text',text:'## 概念业务分层\n\n已定位 **3 处关键引用**。入口、指标计算与契约持久化分工清晰。\n\n| 业务层 | 入口文件 | 引用数 |\n| :--- | :--- | ---: |\n| 路由分发 | `concept_route.py` | 1 |\n| 契约恢复与持久化 | `pipeline.py` | 2 |\n\n### 调用示例\n\n```python\nfrom ai_query.concept_wiki.attainment import build_attainment\n\n# 恢复契约后计算当前指标\ndef resolve_concept(contract: dict) -> dict:\n    result = build_attainment(contract)\n    return {"status": "ready", "data": result}\n```\n\n> 契约恢复与持久化共用同一条流水线，后续调整时应一起核对。\n\n- [x] 确认概念路由入口\n- [x] 定位契约恢复与持久化调用\n- [ ] 检查异常回退分支'}]}
      ]
    }
    const smoke = window.__ompPresentation = {snapshot,command,calls:[]}
    window.__TAURI_INTERNALS__ = {invoke:async (command,args={}) => {
      smoke.calls.push({command,args})
      if(command==='omp_snapshot') return structuredClone(snapshot)
      if(command==='omp_command') return {id:'test',data:args.command.type==='get_available_models'?{models:[{provider:'local',id:'deepseek',name:'DeepSeek V4.1 Flash'}]}:args.command.type==='get_available_thinking_levels'?{levels:['off','medium','high']}: {}}
      if(command==='open_external') return
      throw new Error('Unexpected IPC '+command)
    }}
    const source = await (await fetch('/src/components/OmpPane.vue')).text()
    const vueUrl = source.match(/from ["']([^"']*\/vue\.js[^"']*)["']/)?.[1]
    if(!vueUrl) throw new Error('Cannot resolve Vue runtime')
    const {createApp,h} = await import(vueUrl)
    const {default:Pane} = await import('/src/components/OmpPane.vue')
    const ompUrl = source.match(/from ["']([^"']*\/lib\/omp\.ts[^"']*)["']/)?.[1] || '/src/lib/omp.ts'
    const {ompSnapshot} = await import(ompUrl)
    smoke.refresh = () => {snapshot.seq++;return ompSnapshot('omp-presentation')}
    await import('/src/styles/global.css')
    document.documentElement.dataset.theme = 'dark'
    createApp({render:() => h(Pane,{runtimeId:'omp-presentation'})}).mount('#app')
  })
  await page.getByRole('heading',{name:'概念业务分层'}).waitFor()
  if(await page.locator('.omp-tool-card').count()!==1 || await page.locator('[data-role="toolResult"]').count()) throw new Error('Historical output is duplicated')
  await page.locator('.omp-tool-status').filter({hasText:'已完成'}).waitFor()
  if(!await page.locator('.omp-ansi-cyan').count()) throw new Error('ANSI output has no color')
  const keyword = page.locator('.omp-markdown .hljs-keyword').first()
  const colors = await keyword.evaluate(el => ({keyword:getComputedStyle(el).color,body:getComputedStyle(el.closest('code')).color}))
  if(colors.keyword===colors.body) throw new Error('Code tokens have no syntax color')
  const rawArguments = page.locator('.omp-tool-arguments')
  if(await rawArguments.getAttribute('open')!==null) throw new Error('Raw JSON is expanded by default')
  await rawArguments.locator('summary').click()
  await rawArguments.locator('.hljs-attr').first().waitFor()
  await rawArguments.locator('summary').click()
  await page.locator('.omp-thinking summary').click()
  await page.locator('.omp-thinking strong').waitFor()
  await page.locator('.omp-thinking summary').click()
  await page.locator('.omp-transcript').evaluate(el => {el.scrollTop=0})
  await page.screenshot({path:'.playwright-cli/omp-presentation-dark.png'})
  await page.locator('.omp-markdown .omp-code-copy').click()
  const copied = await page.evaluate(() => navigator.clipboard.readText())
  if(!copied.includes('def resolve_concept')) throw new Error('Copied code lost its original content')
  await page.locator('.omp-transcript').evaluate(el => {el.scrollTop=el.scrollHeight})
  await page.screenshot({path:'.playwright-cli/omp-presentation-answer.png'})

  await page.evaluate(async () => {
    const s=window.__ompPresentation.snapshot
    s.messages[3].content[0].text=Array.from({length:45},(_,i)=>`src/concept_${i}.py:${i+1}: contract_persisted`).join('\n')
    await window.__ompPresentation.refresh()
  })
  const output = page.locator('.omp-tool-body > .omp-code-block').last()
  const limitedHeight = await output.locator('pre').evaluate(el=>el.clientHeight)
  await output.getByRole('button',{name:'展开',exact:true}).click()
  const expandedHeight = await output.locator('pre').evaluate(el=>el.clientHeight)
  if(expandedHeight<=limitedHeight) throw new Error('Long output did not expand')
  await output.getByRole('button',{name:'收起',exact:true}).click()
  await output.getByRole('button',{name:'换行',exact:true}).click()
  if(await output.locator('pre').evaluate(el=>getComputedStyle(el).whiteSpace)!=='pre-wrap') throw new Error('Output wrapping failed')

  await page.evaluate(async () => {
    const s=window.__ompPresentation.snapshot
    s.settled=false
    s.messages.push({role:'assistant',content:[{type:'toolCall',id:'call-live',name:'read',arguments:{path:'src/concept.ts'}}]})
    s.tools['call-live']={type:'tool_execution_update',partialResult:{content:[{type:'text',text:'export const contract = true'}]}}
    await window.__ompPresentation.refresh()
  })
  await page.locator('.omp-tool-card[data-status="running"]').waitFor()
  await page.locator('.omp-tool-card').last().locator('.hljs-keyword').first().waitFor()
  await page.evaluate(async () => {
    const s=window.__ompPresentation.snapshot
    s.settled=true
    s.tools['call-live']={type:'tool_execution_end',isError:true,result:{content:[{type:'text',text:'Permission denied'}]}}
    await window.__ompPresentation.refresh()
  })
  await page.locator('.omp-tool-card[data-status="error"]').waitFor()
  await page.evaluate(async () => {
    const s=window.__ompPresentation.snapshot
    s.messages.pop(); delete s.tools['call-live']
    s.messages[3].content[0].text='src/ai_query/orchestrator/concept_route.py:20: build_attainment\nsrc/ai_query/orchestrator/pipeline.py:4563: contract_restored\nsrc/ai_query/orchestrator/pipeline.py:6835: contract_persisted'
    await window.__ompPresentation.refresh()
  })
  for(const theme of ['dark','light']) {
    await page.evaluate(theme=>{document.documentElement.dataset.theme=theme},theme)
    for(const width of [900,520,360]) {
      await page.setViewportSize({width,height:800})
      if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth)) throw new Error(`${theme} overflows at ${width}px`)
    }
  }
  await page.setViewportSize({width:900,height:900})
  await page.locator('.omp-transcript').evaluate(el=>{el.scrollTop=0})
  await page.screenshot({path:'.playwright-cli/omp-presentation-light.png'})
  page.off('pageerror',onError)
  if(errors.length) throw new Error(errors.join('\n'))
  return {ok:true,cases:['historical-result-pairing','completed-status','ANSI-color','syntax-color','collapsed-JSON','thinking-Markdown','copy-original-code','long-output-expand','line-wrap','streamed-file-highlight','failed-tool','dark-light-360-to-900px']}
}
