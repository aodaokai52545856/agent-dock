// Synthetic version center: no software is downloaded, upgraded or removed.
async page=>{
  const errors=[];page.on('pageerror',e=>errors.push(e.message))
  await page.route('**/omp-version-smoke',r=>r.fulfill({contentType:'text/html',body:'<!doctype html><html><head><meta charset="utf-8"></head><body><div id="app"></div></body></html>'}))
  await page.goto('http://127.0.0.1:1423/omp-version-smoke')
  await page.setViewportSize({width:1280,height:840})
  await page.evaluate(async()=>{
    const calls=window.__ompVersionCalls=[]
    const snapshot={revision:1,batchId:'',readyVersion:null,readyTools:{},tasks:[]}
    window.__TAURI_INTERNALS__={invoke:async(command,args={})=>{
      calls.push({command,args})
      if(command==='app_version')return'0.1.22'
      if(command==='version_task_state')return structuredClone(snapshot)
      if(command==='list_tool_versions')return[{toolId:args.toolId,name:args.toolId,found:true,path:args.toolId==='omp'?'C:/Users/Example/.bun/bin/omp.exe':'C:/tools/tool.exe',localVersion:'18.15.0',latestVersion:'18.16.0',compare:'可更新',checks:[{name:'安装来源',ok:true,detail:'bun'}]}]
      if(command==='start_version_tasks'){
        snapshot.revision++;snapshot.batchId='batch-'+snapshot.revision
        const ready=args.action==='upgrade';snapshot.readyTools=ready?{omp:'18.16.0'}:{}
        snapshot.tasks.push({id:'task-'+snapshot.revision,batchId:snapshot.batchId,target:'omp',action:args.action,status:ready?'ready':'succeeded',stage:ready?'等待应用':'已完成',log:'Synthetic OMP update log',localVersion:'18.16.0',downloaded:100,total:100,startedAt:1,finishedAt:2,restart:false})
        return structuredClone(snapshot)
      }
      if(command==='probe_tools')return Object.fromEntries(['opencode','grokbuild','kimi','claude','pi','omp','dsh'].map(id=>[id,{found:true}]))
      if(command==='list_sessions')return{ok:true,sessions:[],toolFound:true}
      if(command==='list_live_ptys')return[]
      return undefined
    }}
    const source=await(await fetch('/src/components/VersionDrawer.vue')).text()
    const vueUrl=source.match(/from ["']([^"']*\/vue\.js[^"']*)["']/)?.[1]
    const{createApp,h,ref}=await import(vueUrl)
    const{default:Drawer}=await import('/src/components/VersionDrawer.vue')
    const{modal}=await import('/src/lib/modal.ts')
    await import('/src/styles/global.css')
    const open=ref(true);window.__ompVersionOpen=value=>{open.value=value}
    createApp({render:()=>h(Drawer,{open:open.value,onClose:()=>{open.value=false}})}).directive('modal',modal).mount('#app')
  })
  const row=page.locator('tr.software-row').filter({hasText:'OMP'})
  await row.getByRole('button',{name:'检测',exact:true}).click()
  await row.getByText('18.16.0',{exact:true}).waitFor()
  await row.getByRole('button',{name:'详情',exact:false}).click()
  await page.getByText('C:/Users/Example/.bun/bin/omp.exe',{exact:true}).waitFor()
  await row.getByRole('button',{name:'升级',exact:true}).click()
  await row.getByRole('button',{name:'应用更新',exact:true}).waitFor()
  if(!(await row.locator('td').nth(1).innerText()).includes('18.15.0'))throw new Error('Pending target was reported as already installed')
  await page.getByRole('button',{name:'关闭',exact:true}).click()
  await page.evaluate(()=>window.__ompVersionOpen(true))
  await row.getByRole('button',{name:'应用更新',exact:true}).waitFor()
  await row.getByRole('button',{name:'应用更新',exact:true}).click()
  const dialog=page.getByRole('dialog',{name:'应用 OMP 更新'})
  await dialog.waitFor()
  if(await page.evaluate(()=>window.__ompVersionCalls.some(c=>c.command==='start_version_tasks'&&c.args.action==='apply')))throw new Error('Update was applied before confirmation')
  await dialog.getByRole('button',{name:'应用更新',exact:true}).click()
  await row.getByText('18.16.0',{exact:true}).first().waitFor()
  if(!await page.evaluate(()=>window.__ompVersionCalls.some(c=>c.command==='start_version_tasks'&&c.args.targets[0]==='omp'&&c.args.action==='apply')))throw new Error('OMP application was not dispatched')
  await page.screenshot({path:'.playwright-cli/omp-version-center.png'})
  if(errors.length)throw new Error(errors.join('\n'))
  return{ok:true,cases:['omp-detection','actual-path','installation-source','upgrade','pending-update','reopen','explicit-apply']}
}
