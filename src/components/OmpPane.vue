<script setup lang="ts">
import { computed,ref,watch,nextTick,onMounted,onUnmounted } from 'vue'
import { ompState,ompSnapshot,ompCommand,ompRespond,ompOpenConfig,ompCliLogin,messageText,type OmpCommand } from '../lib/omp'
import { store,rememberOpened,showToast } from '../lib/store'
import * as api from '../lib/api'
import { renderMarkdown } from '../lib/markdown'

const props=defineProps<{runtimeId:string}>()
const snap=computed(()=>ompState.sessions[props.runtimeId])
const draft=computed({get:()=>ompState.drafts[props.runtimeId]||'',set:v=>{ompState.drafts[props.runtimeId]=v}})
const models=ref<any[]>([]),levels=ref<string[]>([]),providers=ref<any[]>([]),entries=ref<any[]>([])
const images=ref<{type:'image';data:string;mimeType:string}[]>([])
const pending=ref(false),error=ref(''),follow=ref<'steer'|'followUp'>('followUp'),tab=ref<'chat'|'agents'|'tree'|'login'>('chat')
const answers=ref<Record<string,string[]>>({}),custom=ref<Record<string,string>>({}),dialogText=ref(''),scroll=ref<HTMLElement|null>(null),file=ref<HTMLInputElement|null>(null)
const agentView=ref(''),selectedAgent=ref(''),steer=ref('')
const busy=computed(()=>snap.value ? !snap.value.settled : false)
const now=ref(Date.now()),syncing=ref(false),syncError=ref('')
let tick:ReturnType<typeof setInterval>|undefined,lastStateSync=0
const elapsed=computed(()=>Math.max(0,now.value-(snap.value?.activity?.startedAt||now.value)))
const quiet=computed(()=>Math.max(0,now.value-(snap.value?.activity?.lastProgressAt||snap.value?.activity?.startedAt||now.value)))
const duration=(ms:number)=>{const seconds=Math.floor(ms/1000);return `${Math.floor(seconds/60)}:${String(seconds%60).padStart(2,'0')}`}
const phaseLabel=computed(()=>{
  if(!busy.value)return '就绪'
  if(snap.value?.dialogs.length)return '等待回答'
  return ({thinking:'正在思考',responding:'正在回复',tool:'工具执行中',retrying:'请求重试中',background:'等待后台任务',working:'运行中'} as Record<string,string>)[snap.value?.activity?.phase||'working']||'运行中'
})
async function syncState(){
  if(syncing.value)return
  const id=props.runtimeId
  syncing.value=true;lastStateSync=Date.now()
  try{await ompSnapshot(id,true);if(id===props.runtimeId)syncError.value=''}
  catch(e){if(id===props.runtimeId)syncError.value=String(e)}
  finally{syncing.value=false}
}
onMounted(()=>{tick=setInterval(()=>{
  now.value=Date.now()
  if(busy.value&&quiet.value>=30_000&&now.value-lastStateSync>=30_000)void syncState()
},1000)})
onUnmounted(()=>{if(tick)clearInterval(tick)})
const dialog=computed(()=>snap.value?.dialogs[0])
const messages=computed(()=>snap.value?.messages||[])
const modelKey=computed(()=>{const m=snap.value?.state.model;return m&&!snap.value?.setupRequired?`${m.provider}/${m.id}`:''})
const list=(data:any,key:string)=>Array.isArray(data)?data:data?.[key]||[]
async function command(c:OmpCommand){
  error.value=''
  try{return (await ompCommand(props.runtimeId,c)).data}catch(e){error.value=String(e);throw e}
}
async function load(){
  try{
    await ompSnapshot(props.runtimeId,true)
    const [m,l]=await Promise.all([command({type:'get_available_models'}),command({type:'get_available_thinking_levels'})])
    models.value=list(m,'models');levels.value=list(l,'levels')
    if(snap.value?.setupRequired)await showPanel('login')
  }catch(e){error.value=String(e)}
}
watch(()=>props.runtimeId,()=>{images.value=[];selectedAgent.value='';agentView.value='';tab.value='chat';void load()},{immediate:true})
watch(()=>dialog.value?.id,()=>{dialogText.value='';answers.value={};custom.value={}})
watch(()=>messages.value,async()=>{const el=scroll.value;if(!el)return;const atBottom=el.scrollHeight-el.scrollTop-el.clientHeight<180;await nextTick();if(atBottom)el.scrollTop=el.scrollHeight},{deep:true})
async function send(){
  if(pending.value || snap.value?.setupRequired || (!draft.value.trim()&&!images.value.length))return
  const message=draft.value,attachments=[...images.value];pending.value=true
  try{
    if(!messages.value.some(m=>m.role==='user'))await command({type:'set_session_name',name:message.trim().slice(0,48)||'图片会话'})
    await command({type:'prompt',message,images:attachments,...(busy.value?{streamingBehavior:follow.value}:{})})
    draft.value='';images.value=[]
  }catch{}finally{pending.value=false}
}
async function attach(files:FileList|null){
  for(const f of Array.from(files||[])){
    if(!f.type.startsWith('image/')){showToast('请选择图片文件');continue}
    if(f.size>700_000){showToast('图片超过 RPC 输入帧限制，请缩小后添加（单次附件合计小于 700 KB）');continue}
    const data=await new Promise<string>((resolve,reject)=>{const r=new FileReader();r.onload=()=>resolve(String(r.result).split(',')[1]||'');r.onerror=reject;r.readAsDataURL(f)})
    if(images.value.reduce((n,i)=>n+i.data.length,0)+data.length>950_000){showToast('本次图片附件合计过大，请分次发送');break}
    images.value.push({type:'image',data,mimeType:f.type})
  }
}
async function respond(payload:Record<string,unknown>){if(!dialog.value)return;try{await ompRespond(props.runtimeId,{id:dialog.value.id,...payload})}catch(e){error.value=String(e)}}
async function showPanel(which:typeof tab.value){
  tab.value=which
  try{
    if(which==='tree'){const data=await command({type:'get_entries'});entries.value=list(data,'entries')}
    if(which==='login'){const data=await command({type:'get_login_providers'});providers.value=list(data,'providers')}
  }catch{}
}
async function switchModel(event:Event){const key=(event.target as HTMLSelectElement).value;const m=models.value.find(m=>`${m.provider}/${m.id}`===key);if(m){try{await command({type:'set_model',provider:m.provider,modelId:m.id});await load();tab.value='chat'}catch{}}}
async function thinking(event:Event){try{await command({type:'set_thinking_level',level:(event.target as HTMLSelectElement).value});await ompSnapshot(props.runtimeId,true)}catch{}}
async function transition(type:'fork'|'navigate_tree',entryId?:string){try{const data=await command({type,entryId});if(data?.cancelled){showToast('操作已取消，仍保留当前会话');return}if(typeof data?.editorText==='string')draft.value=data.editorText;await ompSnapshot(props.runtimeId,true);tab.value='chat'}catch{}}
async function readAgent(id:string){selectedAgent.value=id;try{agentView.value=JSON.stringify(await command({type:'get_subagent_messages',subagentId:id}),null,2)}catch{}}
async function logout(providerId:string){try{const data=await command({type:'get_logout_accounts',providerId});for(const a of list(data,'accounts'))await command({type:'logout',providerId,credentialId:a.credentialId});await load()}catch{}}
async function cliLogin(){
  try{const opened=await ompCliLogin(store.selectedProjectId,snap.value?.profileId||'default');rememberOpened({...opened,projectId:store.selectedProjectId,toolId:'omp',alive:true})}catch(e){error.value=String(e)}
}
function toggleAnswer(q:any,label:string){const old=answers.value[q.id]||[];answers.value[q.id]=q.multi?(old.includes(label)?old.filter(x=>x!==label):[...old,label]):[label]}
async function submitAsk(){await respond({answers:(dialog.value?.questions||[]).map((q:any)=>({id:q.id,selectedOptions:answers.value[q.id]||[],customInput:custom.value[q.id]||''}))})}
</script>

<template>
  <section class="omp-pane" aria-label="OMP 会话">
    <header class="omp-toolbar">
      <strong>OMP <small>{{ snap?.profileId }}</small></strong>
      <select aria-label="模型" :value="modelKey" @change="switchModel"><option value="" disabled>选择模型</option><option v-for="m in models" :key="m.provider+'/'+m.id" :value="m.provider+'/'+m.id">{{ m.name||m.id }} · {{ m.provider }}</option></select>
      <select aria-label="思考强度" :value="snap?.state.thinkingLevel" @change="thinking"><option v-for="l in levels" :key="l" :value="l">{{ l }}</option></select>
      <span class="run-state" role="status">{{ phaseLabel }}<template v-if="busy"> · 已运行 {{ duration(elapsed) }}</template></span>
      <button class="btn btn-ghost btn-small" @click="load">刷新</button>
    </header>
    <nav class="omp-tabs" aria-label="OMP 面板"><button v-for="item in (['chat','agents','tree','login'] as const)" :key="item" :class="{selected:tab===item}" @click="showPanel(item)">{{ {chat:'对话',agents:'子代理',tree:'会话树',login:'登录与配置'}[item] }}</button><button :disabled="busy" @click="command({type:'compact'}).catch(()=>{})">压缩</button><button :disabled="busy" @click="transition('fork')">派生会话</button></nav>
    <div v-if="error||snap?.error" class="omp-error" role="alert">{{ error||snap?.error }} <button class="text-btn" @click="load">重新加载</button></div>
    <div v-if="snap?.setupRequired" class="omp-setup" role="status">此 profile 尚未配置可用模型。请先在“登录与配置”中登录，再从上方选择模型。</div>
    <div v-if="busy&&quiet>=30_000&&!dialog" class="omp-wait" role="status"><span>已 {{ duration(quiet) }} 没有新进展，正在等待模型或后台任务。{{ syncError ? '状态同步失败：'+syncError : '可继续等待、同步状态或停止本轮。' }}</span><button class="text-btn" :disabled="syncing" @click="syncState">{{ syncing?'同步中…':'同步状态' }}</button></div>
    <div v-if="!snap" class="omp-empty">正在连接 OMP…</div>
    <div v-else-if="tab==='chat'" ref="scroll" class="omp-transcript">
      <div v-if="!messages.length" class="omp-empty">发送消息开始 OMP 会话。模型、技能和工具沿用当前 profile 配置。</div>
      <article v-for="(m,index) in messages" :key="m._dockId||index" class="omp-message" :data-role="m.role">
        <header>{{ m.role==='user'?'你':m.role==='assistant'?'OMP':m.role }} <small v-if="m.model">{{ m.model }}</small></header>
        <div v-if="typeof m.content==='string'" class="ad-markdown" v-html="renderMarkdown(m.content)" />
        <template v-else v-for="(part,i) in m.content||[]" :key="i">
          <div v-if="part.type==='text'" class="ad-markdown" v-html="renderMarkdown(part.text||'')" />
          <details v-else-if="part.type==='thinking'" class="omp-thinking"><summary>思考过程</summary><pre>{{ part.thinking||part.text }}</pre></details>
          <img v-else-if="part.type==='image' && part.data" class="omp-image" :src="`data:${part.mimeType};base64,${part.data}`" alt="会话图片" />
          <details v-else-if="part.type==='toolCall'" class="omp-tool" :open="snap.tools[part.id]?.type!=='tool_execution_end'"><summary>{{ part.name }} · {{ snap.tools[part.id]?.type==='tool_execution_end' ? (snap.tools[part.id]?.isError?'失败':'完成'):'执行中' }}</summary><pre>{{ JSON.stringify(part.arguments,null,2) }}</pre><pre v-if="snap.tools[part.id]">{{ messageText((snap.tools[part.id].result||snap.tools[part.id].partialResult)?.content) }}</pre></details>
          <details v-else><summary>{{ part.type||'内容' }}</summary><pre>{{ JSON.stringify(part,null,2) }}</pre></details>
        </template>
      </article>
      <div v-if="snap.state.queues?.steering?.length || snap.state.queues?.followUp?.length" class="omp-queues"><strong>待处理消息</strong><template v-for="q in (['steering','followUp'] as const)" :key="q"><div v-for="message in snap.state.queues[q]||[]" :key="String(message)"><span>{{ message }}</span><button @click="command({type:'remove_queued_message',message:String(message),queue:q}).catch(()=>{})">移除</button><button @click="command({type:'promote_queued_message',message:String(message)}).catch(()=>{})">提升</button></div></template></div>
    </div>
    <div v-else-if="tab==='agents'" class="omp-panel"><p v-if="!Object.keys(snap.subagents).length">暂无子代理</p><article v-for="(a,id) in snap.subagents" :key="id" class="omp-tool"><strong>{{ a.name||a.agent||id }}</strong><span>{{ a.status||a.type }}</span><button @click="readAgent(String(id))">查看</button><button @click="command({type:'cancel_subagent',subagentId:String(id)}).catch(()=>{})">停止</button></article><template v-if="selectedAgent"><pre>{{ agentView }}</pre><input v-model="steer" placeholder="向此子代理发送指令" /><button :disabled="!steer.trim()" @click="command({type:'steer_subagent',subagentId:selectedAgent,message:steer}).then(()=>{steer=''})">发送指令</button></template></div>
    <div v-else-if="tab==='tree'" class="omp-panel"><p>选择历史节点恢复当前分支，或派生为新会话。</p><article v-for="e in entries" :key="e.id" class="omp-tree-entry"><span>{{ e.type }} · {{ messageText(e.message?.content).slice(0,100)||e.id }}</span><button :disabled="busy" @click="transition('navigate_tree',e.id)">恢复此处</button><button v-if="e.type==='message'" :disabled="busy" @click="transition('fork',e.id)">从此派生</button></article></div>
    <div v-else class="omp-panel"><p>配置与凭证由 OMP profile 管理。</p><button class="btn btn-ghost" @click="api.openExternal('https://omp.sh')">OMP 配置说明</button><button class="btn btn-ghost" @click="ompOpenConfig(snap.profileId).catch(e=>{error=String(e)})">打开配置文件</button><button class="btn btn-ghost" @click="cliLogin">使用终端登录</button><article v-for="p in providers" :key="p.id" class="omp-tree-entry"><span>{{ p.name||p.id }}</span><button @click="command({type:'login',providerId:p.id}).then(load).catch(()=>{})">登录</button><button @click="logout(p.id)">退出登录</button></article><p>当前 profile：{{ snap.profileId }}。API Key 与自定义模型可在 OMP 配置文件中设置。</p></div>
    <div v-if="snap?.notifications?.length" class="omp-notices"><div v-for="(n,i) in snap.notifications.slice(-3)" :key="i"><span>{{ n.message||n.text||n.instructions||n.title||(Array.isArray(n.content)?n.content.join('\n'):n.content) }}</span><button v-if="n.method==='open_url'" @click="api.openExternal(n.launchUrl||n.url)">打开登录链接</button></div></div>
    <section v-if="dialog" class="omp-dialog" role="region" aria-label="OMP 需要输入">
      <strong>{{ dialog.title||'OMP 需要你的输入' }}</strong><p>{{ dialog.message }}</p>
      <template v-if="dialog.method==='select'"><button v-for="(option,i) in dialog.options" :key="option" @click="respond({value:option})">{{ option }} <small>{{ dialog.optionDetails?.[i]?.description }}</small></button></template>
      <template v-else-if="dialog.method==='confirm'"><button class="btn btn-primary" @click="respond({confirmed:true})">允许</button><button class="btn btn-ghost" @click="respond({confirmed:false})">拒绝</button></template>
      <template v-else-if="dialog.method==='ask'"><fieldset v-for="q in dialog.questions" :key="q.id"><legend>{{ q.question }}</legend><button v-for="o in q.options" :key="o.label" :class="{selected:answers[q.id]?.includes(o.label)}" @click="toggleAnswer(q,o.label)">{{ o.label }} <small>{{ o.description }}</small></button><input v-model="custom[q.id]" placeholder="补充回答" /></fieldset><button class="btn btn-primary" @click="submitAsk">提交回答</button></template>
      <p v-else-if="dialog.unsupported" role="alert">当前 OMP 交互类型暂不支持，可取消此请求后继续。</p>
      <template v-else><input v-if="dialog.method==='input'" v-model="dialogText" :type="dialog.secret?'password':'text'" :placeholder="dialog.placeholder" /><textarea v-else v-model="dialogText" rows="4" /><button class="btn btn-primary" @click="respond({value:dialogText})">提交</button></template>
      <button class="btn btn-ghost" @click="respond({cancelled:true})">取消</button>
    </section>
    <footer class="omp-composer">
      <div v-if="images.length" class="omp-attachments"><button v-for="(im,i) in images" :key="i" @click="images.splice(i,1)"><img :src="`data:${im.mimeType};base64,${im.data}`" alt="待发送图片" />移除</button></div>
      <textarea v-model="draft" aria-label="发送给 OMP" placeholder="发送消息，Shift+Enter 换行" :disabled="!snap?.info.alive||snap?.setupRequired" @keydown.enter.exact.prevent="send" @paste="event=>{if(event.clipboardData?.files.length){event.preventDefault();void attach(event.clipboardData.files)}}" />
      <div class="composer-actions"><input ref="file" hidden type="file" accept="image/*" multiple @change="attach(($event.target as HTMLInputElement).files)" /><button class="btn btn-ghost btn-small" @click="file?.click()">添加图片</button><select v-if="busy" v-model="follow" aria-label="运行中消息处理"><option value="followUp">排队跟进</option><option value="steer">引导当前任务</option></select><button v-if="busy" class="btn btn-ghost btn-small" @click="command({type:'abort'}).catch(()=>{})">停止</button><button class="btn btn-primary btn-small" :disabled="pending||!snap?.info.alive||snap?.setupRequired||(!draft.trim()&&!images.length)" @click="send">{{ pending?'提交中…':busy?'发送跟进':'发送' }}</button></div>
    </footer>
  </section>
</template>

<style scoped>
.omp-setup{padding:12px 16px;color:var(--ad-muted);background:var(--ad-harbor);border-bottom:1px solid var(--ad-border)}
.omp-wait{display:flex;align-items:center;gap:12px;padding:8px 16px;color:var(--ad-warning);font-size:12px;border-bottom:1px solid var(--ad-border)}.omp-wait span{flex:1}.omp-wait button{flex-shrink:0}
.omp-error{white-space:pre-wrap;overflow-wrap:anywhere;max-height:180px;overflow:auto}
.omp-pane{height:100%;min-height:0;display:flex;flex-direction:column;background:var(--ad-ink);color:var(--ad-text);font-size:14px}.omp-toolbar,.omp-tabs,.composer-actions{display:flex;align-items:center;gap:8px;padding:8px 16px;border-bottom:1px solid var(--ad-border)}.omp-toolbar{flex-wrap:wrap}.omp-toolbar select{max-width:260px;min-width:80px;flex:1}.omp-toolbar small,.run-state{color:var(--ad-muted);font-size:12px}.run-state{margin-left:auto}.omp-tabs{flex-wrap:wrap}.omp-tabs button,.omp-tree-entry button,.omp-panel button,.omp-dialog>button{padding:4px 8px;min-height:28px;border-radius:4px}.selected{background:var(--ad-hover);color:var(--ad-accent)}.omp-transcript,.omp-panel{flex:1;min-height:0;overflow:auto;padding:16px}.omp-message{padding:12px 0;border-bottom:1px solid var(--ad-border)}.omp-message>header{color:var(--ad-muted);font-size:12px;margin-bottom:8px}.omp-message[data-role=user]{padding:12px;background:var(--ad-harbor);border-radius:4px}.omp-message header small{float:right}.omp-tool,.omp-thinking,.omp-tree-entry{border:1px solid var(--ad-border);border-radius:4px;padding:8px;margin:8px 0}.omp-tool summary,.omp-thinking summary{cursor:pointer}.omp-tree-entry{display:flex;gap:8px;align-items:center}.omp-tree-entry>span{flex:1}.omp-image{max-width:100%;max-height:360px}.omp-empty{padding:24px;color:var(--ad-muted)}.omp-error{padding:8px 16px;color:var(--ad-error);border-bottom:1px solid var(--ad-border)}.omp-composer{padding:8px 16px 12px;border-top:1px solid var(--ad-border)}.omp-composer textarea{width:100%;min-height:80px;resize:vertical;max-height:240px;background:var(--ad-harbor);color:var(--ad-text);border:1px solid var(--ad-border);border-radius:4px;padding:8px}.composer-actions{padding:8px 0 0;border:0;justify-content:flex-end}.omp-dialog{padding:16px;max-height:40%;overflow:auto;background:var(--ad-harbor);border-top:2px solid var(--ad-accent)}.omp-dialog input,.omp-dialog textarea{display:block;width:100%;margin:8px 0}.omp-dialog fieldset{border:1px solid var(--ad-border);margin:8px 0;padding:8px}.omp-dialog button small{display:block;color:var(--ad-muted)}.omp-attachments{display:flex;gap:8px}.omp-attachments img{height:48px;display:block}.omp-notices{padding:8px 16px;font-size:12px}.omp-notices>div{display:flex;justify-content:space-between;gap:8px}pre{white-space:pre-wrap;overflow-wrap:anywhere;font-size:12px;max-height:400px;overflow:auto}select,input{color:var(--ad-text);background:var(--ad-harbor);border:1px solid var(--ad-border);min-height:32px;border-radius:4px}textarea{font-family:inherit}
</style>
