// playwright-cli run-code (file contents), against Vite on port 1423.
// Uses only synthetic snapshots; never accesses credentials or live providers.
async page => {
  const errors=[]
  page.on('pageerror',e=>errors.push(e.message))
  await page.setViewportSize({width:1280,height:840})
  await page.route('**/omp-ui-smoke',r=>r.fulfill({contentType:'text/html',body:'<!doctype html><html><head><meta charset="utf-8"></head><body><div id="app" style="height:100vh"></div></body></html>'}))
  await page.goto('http://127.0.0.1:1423/omp-ui-smoke')
  await page.evaluate(async()=>{
    const snapshot={runtimeId:'omp-test',seq:1,profileId:'work',info:{ptyId:'omp-test',runtimeId:'omp-test',projectId:'project',toolId:'omp',sessionId:'omp@d29yaw:session',title:'OMP smoke',alive:true,kind:'ompRpc'},
      state:{model:{id:'mock',provider:'local',name:'Local Mock'},thinkingLevel:'medium',queues:{steering:[],followUp:[]}},settled:true,promptResults:{},dialogs:[],tools:{'call-1':{type:'tool_execution_end',result:{content:[{type:'text',text:'Found 2 files'}]}}},subagents:{worker:{id:'worker',agent:'reviewer',status:'running'}},
      messages:[{role:'user',content:[{type:'text',text:'检查工作区并给出结果'}]},{role:'assistant',content:[{type:'thinking',thinking:'核对项目文件。'},{type:'toolCall',id:'call-1',name:'read',arguments:{path:'README.md'}},{type:'text',text:'## 已完成\n\n共检查 **2 个文件**。\n\n```ts\nconst result = true\n```'}]}]}
    const smoke=window.__ompSmoke={snapshot,calls:[]}
    window.__TAURI_INTERNALS__={invoke:async(command,args={})=>{
      smoke.calls.push({command,args})
      if(command==='omp_snapshot')return structuredClone(snapshot)
      if(command==='omp_respond_ui'){snapshot.dialogs=[];snapshot.seq++;return}
      if(command==='omp_open_config'||command==='open_external')return
      if(command==='omp_command'){
        const c=args.command
        if(c.type==='set_model'){snapshot.setupRequired=false;snapshot.seq++}
        const data=c.type==='get_available_models'?{models:[{provider:'local',id:'mock',name:'Local Mock'}]}:
          c.type==='get_available_thinking_levels'?{levels:['off','medium','high']}:
          c.type==='get_entries'?{entries:[{id:'entry1',type:'message',message:{role:'user',content:'历史节点'}}]}:
          c.type==='get_login_providers'?{providers:[{id:'local',name:'Local OAuth'}]}:
          c.type==='get_subagent_messages'?{messages:[{role:'assistant',content:'Review complete'}]}:{}
        return{id:'request',success:true,data}
      }
      throw new Error('Unexpected IPC '+command)
    }}
    const source=await(await fetch('/src/components/OmpPane.vue')).text()
    const vueUrl=source.match(/from ["']([^"']*\/vue\.js[^"']*)["']/)?.[1]
    if(!vueUrl)throw new Error('Cannot resolve Vue runtime')
    const{createApp,h}=await import(vueUrl)
    const{default:Pane}=await import('/src/components/OmpPane.vue')
    const{default:Fab}=await import('/src/components/TextConvertFab.vue')
    const ompUrl=source.match(/from ["']([^"']*\/lib\/omp\.ts[^"']*)["']/)?.[1]||'/src/lib/omp.ts'
    const{ompState,ompSnapshot}=await import(ompUrl)
    smoke.refresh=()=>ompSnapshot('omp-test');smoke.draft=text=>{ompState.drafts['omp-test']=text}
    await import('/src/styles/global.css')
    document.documentElement.dataset.theme='dark'
    createApp({render:()=>h(Pane,{runtimeId:'omp-test'})}).mount('#app')
    const host=document.createElement('div');document.body.append(host)
    localStorage.removeItem('agent-dock-text-convert-position')
    let fabApp=createApp(Fab);fabApp.mount(host)
    smoke.remountFab=()=>{fabApp.unmount();fabApp=createApp(Fab);fabApp.mount(host)}
  })
  await page.getByRole('heading',{name:'已完成'}).waitFor()
  await page.getByLabel('发送给 OMP').fill('这是测试消息')
  await page.getByRole('button',{name:'发送',exact:true}).click()
  if(!await page.evaluate(()=>window.__ompSmoke.calls.some(c=>c.command==='omp_command'&&c.args.command.type==='prompt'&&c.args.command.message==='这是测试消息')))throw new Error('Prompt was not submitted')
  await page.evaluate(async()=>{window.__ompSmoke.snapshot.dialogs=[{id:'approval',method:'confirm',title:'允许写入 README.md？',message:'工具请求更新文件'}];window.__ompSmoke.snapshot.seq++;await window.__ompSmoke.refresh()})
  await page.getByRole('button',{name:'拒绝',exact:true}).click()
  if(!await page.evaluate(()=>window.__ompSmoke.calls.some(c=>c.command==='omp_respond_ui'&&c.args.response.confirmed===false)))throw new Error('Approval refusal did not reach RPC')
  await page.getByRole('button',{name:'子代理',exact:true}).click()
  await page.getByRole('button',{name:'查看',exact:true}).click()
  await page.getByText('Review complete',{exact:false}).waitFor()
  await page.getByRole('button',{name:'会话树',exact:true}).click()
  await page.getByRole('button',{name:'从此派生',exact:true}).click()
  await page.getByRole('button',{name:'登录与配置',exact:true}).click()
  await page.getByRole('button',{name:'打开配置文件',exact:true}).click()
  await page.getByRole('button',{name:'对话',exact:true}).click()
  await page.screenshot({path:'.playwright-cli/omp-native-dark.png'})
  await page.evaluate(()=>{document.documentElement.dataset.theme='light'})
  await page.setViewportSize({width:900,height:620})
  await page.waitForTimeout(250)
  await page.screenshot({path:'.playwright-cli/omp-native-light-narrow.png'})
  const overflow=await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth)
  if(overflow)throw new Error('OMP pane overflows the narrow viewport')
  await page.evaluate(async()=>{window.__ompSmoke.snapshot.setupRequired=true;window.__ompSmoke.snapshot.seq++;await window.__ompSmoke.refresh()})
  await page.getByRole('button',{name:'刷新',exact:true}).click()
  await page.getByRole('status').filter({hasText:'尚未配置可用模型'}).waitFor()
  await page.getByText('Local OAuth',{exact:true}).waitFor()
  if(!await page.getByLabel('发送给 OMP').isDisabled())throw new Error('Unconfigured profile can submit a prompt')
  await page.getByLabel('模型',{exact:true}).selectOption('local/mock')
  await page.waitForFunction(()=>!window.__ompSmoke.snapshot.setupRequired)
  if(await page.getByLabel('发送给 OMP').isDisabled())throw new Error('Choosing an available model did not enable chat')
  const promptsBefore=await page.evaluate(()=>window.__ompSmoke.calls.filter(c=>c.command==='omp_command'&&c.args.command.type==='prompt').length)
  await page.evaluate(async()=>{const s=window.__ompSmoke.snapshot;s.settled=false;s.activity={phase:'thinking',startedAt:Date.now()-125000,lastProgressAt:Date.now()-45000};s.seq++;await window.__ompSmoke.refresh()})
  await page.getByRole('status').filter({hasText:'正在思考'}).waitFor()
  await page.getByText('没有新进展',{exact:false}).waitFor()
  await page.screenshot({path:'.playwright-cli/omp-long-thinking.png'})
  await page.evaluate(()=>{const s=window.__ompSmoke.snapshot;s.settled=true;s.activity.phase='idle';s.seq++})
  await page.getByRole('status').filter({hasText:'就绪'}).waitFor()
  if(promptsBefore!==await page.evaluate(()=>window.__ompSmoke.calls.filter(c=>c.command==='omp_command'&&c.args.command.type==='prompt').length))throw new Error('Status recovery resubmitted a prompt')
  const fab=page.getByRole('button',{name:'文本转换',exact:true})
  const before=await fab.boundingBox()
  await page.mouse.move(before.x+22,before.y+22);await page.mouse.down()
  await page.mouse.move(280,110,{steps:8});await page.mouse.up()
  const moved=await fab.boundingBox()
  if(Math.abs(before.x-moved.x)<50||await fab.getAttribute('aria-expanded')!=='false')throw new Error('Dragging FAB failed or opened its panel')
  await fab.click()
  await page.getByRole('dialog',{name:'文本转换',exact:true}).waitFor()
  await page.getByLabel('原文',{exact:true}).fill('软件测试')
  await page.getByRole('button',{name:'全部转换',exact:true}).click()
  if(!await page.getByLabel('结果',{exact:true}).inputValue())throw new Error('Conversion stopped working after drag')
  await fab.click()
  await page.evaluate(()=>window.__ompSmoke.remountFab())
  const restored=await fab.boundingBox()
  if(Math.abs(restored.x-moved.x)>1||Math.abs(restored.y-moved.y)>1)throw new Error('FAB position was not persisted')
  await page.setViewportSize({width:520,height:420})
  const clamped=await fab.boundingBox()
  if(clamped.x<0||clamped.y<0||clamped.x+44>520||clamped.y+44>420)throw new Error('FAB went off screen after resize')
  await fab.click()
  const panel=await page.getByRole('dialog',{name:'文本转换',exact:true}).boundingBox()
  if(panel.x<0||panel.y<0||panel.x+panel.width>521||panel.y+panel.height>421)throw new Error('Dragged FAB panel went off screen')
  if(panel.y<clamped.y+44&&panel.y+panel.height>clamped.y)throw new Error('Conversion panel covers the draggable button')
  await page.screenshot({path:'.playwright-cli/text-convert-dragged.png'})
  await fab.click()
  if(await page.getByRole('dialog',{name:'文本转换',exact:true}).count())throw new Error('Cannot close conversion panel after resize')
  if(errors.length)throw new Error(errors.join('\n'))
  return{ok:true,cases:['prompt','approval-refusal','subagent-transcript','fork','configuration-entry','dark','light-narrow','first-login-setup','model-enables-chat','thinking-wait-feedback','read-only-status-recovery','fab-drag','fab-click-after-drag','fab-conversion','fab-position-persistence','fab-resize-bounds']}
}
