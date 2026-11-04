// Synthetic IPC only. Run with playwright-cli against the local Vite server.
async page=>{
  const errors=[];page.on('pageerror',e=>errors.push(e.message))
  await page.setViewportSize({width:1100,height:640})
  await page.route('**/omp-mode-smoke',r=>r.fulfill({contentType:'text/html',body:'<!doctype html><html><head><meta charset="utf-8"></head><body><div id="app"></div></body></html>'}))
  await page.goto('http://127.0.0.1:1423/omp-mode-smoke')
  await page.evaluate(async()=>{
    window.__TAURI_INTERNALS__={invoke:async(command,args)=>{if(command==='omp_profiles')return[{id:'default',agentDir:'fixture'},{id:'work',agentDir:'fixture-work'}];if(command==='save_settings')return;throw new Error('Unexpected IPC '+command)}}
    const source=await(await fetch('/src/components/NewSessionDialog.vue')).text()
    const vueUrl=source.match(/from ["']([^"']*\/vue\.js[^"']*)["']/)[1]
    const{createApp,h,ref,nextTick}=await import(vueUrl)
    const{default:Dialog}=await import('/src/components/NewSessionDialog.vue')
    const{default:Settings}=await import('/src/components/SettingsDrawer.vue')
    const storeUrl=source.match(/from ["']([^"']*\/lib\/store\.ts[^"']*)["']/)[1]
    const{store}=await import(storeUrl)
    const modeUrl=source.match(/from ["']([^"']*\/lib\/ompMode\.ts[^"']*)["']/)[1]
    const{rememberOmpMode}=await import(modeUrl)
    const{modal}=await import('/src/lib/modal.ts')
    await import('/src/styles/global.css')
    store.projects=[{id:'project',name:'Test project',path:'D:/fixture',proxyEnabled:false,proxyUrl:'',createdAt:''}]
    store.selectedProjectId='project';store.selectedTool='omp';store.settings.ompLaunchMode='wrapped'
    const open=ref(false),settingsOpen=ref(false),resume=ref(null)
    const smoke=window.__ompModes={payloads:[],saved:[],newDialog:async()=>{open.value=false;resume.value=null;await nextTick();open.value=true},history:async()=>{
      open.value=false;await nextTick();rememberOmpMode({toolId:'omp',projectId:'project',sessionId:'omp@d29yaw:history',kind:'pty'})
      resume.value={projectId:'project',sessionId:'omp@d29yaw:history',profileId:'work',title:'Saved OMP session'};open.value=true
    },settings:()=>{open.value=false;settingsOpen.value=true}}
    const app=createApp({render:()=>h('div',{},[
      h(Dialog,{open:open.value,resume:resume.value,onClose:()=>open.value=false,onCreate:p=>{smoke.payloads.push(p);open.value=false}}),
      h(Settings,{open:settingsOpen.value,settings:store.settings,onClose:()=>settingsOpen.value=false,onSave:s=>{smoke.saved.push(s);Object.assign(store.settings,s);settingsOpen.value=false}})
    ])});app.directive('modal',modal);app.mount('#app');await nextTick();open.value=true
  })
  const mode=page.getByLabel('OMP 启动方式',{exact:true})
  if(await mode.inputValue()!=='wrapped')throw new Error('Old default was not preserved')
  await mode.selectOption('native')
  await page.screenshot({path:'.playwright-cli/omp-native-mode-picker.png'})
  await page.getByRole('button',{name:'新建会话',exact:true}).click()
  if(!await page.evaluate(()=>window.__ompModes.payloads[0].ompMode==='native'&&window.__ompModes.payloads[0].toolId==='omp'))throw new Error('Native mode was not emitted')
  await page.evaluate(()=>window.__ompModes.newDialog())
  await page.getByRole('button',{name:'新建会话',exact:true}).click()
  if(!await page.evaluate(()=>window.__ompModes.payloads[1].ompMode==='wrapped'))throw new Error('Wrapped mode was not emitted')
  await page.evaluate(()=>window.__ompModes.history())
  if(await mode.inputValue()!=='native')throw new Error('History did not restore its last mode')
  if(!await page.getByRole('button',{name:'选择项目',exact:true}).isDisabled())throw new Error('History project can be changed')
  if(!await page.getByLabel('OMP profile',{exact:true}).isDisabled())throw new Error('History profile can be changed')
  await mode.selectOption('wrapped')
  await page.getByRole('button',{name:'打开会话',exact:true}).click()
  if(!await page.evaluate(()=>{const p=window.__ompModes.payloads[2];return p.sessionId==='omp@d29yaw:history'&&p.profileId==='work'&&p.ompMode==='wrapped'}))throw new Error('History reference was changed')
  await page.evaluate(()=>window.__ompModes.settings())
  await page.getByRole('button',{name:'终端与工具',exact:true}).click()
  await page.getByLabel('OMP 默认启动方式',{exact:true}).selectOption('native')
  await page.getByRole('button',{name:'保存设置',exact:true}).click()
  if(!await page.evaluate(()=>window.__ompModes.saved[0].ompLaunchMode==='native'))throw new Error('Default mode was not saved')
  await page.evaluate(()=>window.__ompModes.newDialog())
  if(await mode.inputValue()!=='native')throw new Error('New default was not applied')
  const dialog=await page.getByRole('dialog',{name:'新建会话',exact:true}).boundingBox()
  if(dialog.y<0||dialog.y+dialog.height>641)throw new Error('Mode picker does not fit the minimum window height')
  if(errors.length)throw new Error(errors.join('\n'))
  return{ok:true,cases:['native-choice','wrapped-choice','history-mode-memory','history-profile-integrity','saved-default','minimum-window-layout']}
}
