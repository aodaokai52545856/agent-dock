import { reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { LiveSessionInfo, PtyOpened } from './types'

export type OmpCommand = { type: 'prompt'; message: string; images?: {type:'image'; data:string; mimeType:string}[]; streamingBehavior?: 'steer'|'followUp'; id?:string }
  | { type: 'set_model'; provider:string; modelId:string }
  | { type: 'set_thinking_level'; level:string }
  | { type: 'steer_subagent'; subagentId:string; message:string }
  | { type: 'cancel_subagent'; subagentId:string }
  | { type: 'navigate_tree'|'fork'|'branch'; entryId?:string }
  | { type: 'new_session' }
  | { type: 'compact'; customInstructions?:string }
  | { type: 'remove_queued_message'; message:string; queue:'steering'|'followUp' }
  | { type: 'promote_queued_message'; message:string }
  | { type: 'login'|'get_logout_accounts'; providerId:string }
  | { type: 'logout'; providerId:string; credentialId:number }
  | { type: 'get_subagent_messages'; subagentId:string }
  | { type: 'set_session_name'; name:string }
  | { type: 'abort'|'get_state'|'get_available_models'|'get_available_thinking_levels'|'get_available_commands'|'get_tree'|'get_entries'|'get_subagents'|'get_session_stats'|'get_login_providers' }
export type OmpSnapshot = {
  runtimeId:string; seq:number; info:LiveSessionInfo; profileId:string; setupRequired?:boolean
  activity?:{phase:string;startedAt?:number|null;lastProgressAt?:number|null}
  state:Record<string,any>; messages:Record<string,any>[]; tools:Record<string,any>
  dialogs:Record<string,any>[]; subagents:Record<string,any>; error?:string|null
  settled:boolean; promptResults:Record<string,any>; notifications?:Record<string,any>[]
}
export const ompState = reactive({ sessions:{} as Record<string,OmpSnapshot>, drafts:{} as Record<string,string> })
let snapshotObserver:((snapshot:OmpSnapshot)=>void)|undefined
export const isOmpRuntime = (id?:string|null) => Boolean(id?.startsWith('omp-'))
const snapshotRequests = new Map<string, { runtimeId:string; cancelled:boolean; promise:Promise<OmpSnapshot> }>()
export async function ompSnapshot(runtimeId:string,refresh=false) {
  const key=`${runtimeId}:${refresh}`
  const pending=snapshotRequests.get(key)
  if(pending)return pending.promise
  const request={runtimeId,cancelled:false,promise:undefined as unknown as Promise<OmpSnapshot>}
  request.promise=invoke<OmpSnapshot>('omp_snapshot',{runtimeId,refresh}).then(value=>{
    if(request.cancelled)throw new Error('OMP 会话已关闭')
    const old=ompState.sessions[runtimeId]
    if(!old || value.seq>=old.seq) {ompState.sessions[runtimeId]=value;snapshotObserver?.(value)}
    return value
  }).finally(()=>{if(snapshotRequests.get(key)===request)snapshotRequests.delete(key)})
  snapshotRequests.set(key,request)
  return request.promise
}
export async function ompCommand(runtimeId:string,command:OmpCommand):Promise<any>{
  const response=await invoke<{id:string;data:any}>('omp_command',{runtimeId,command})
  return response
}
export async function ompRespond(runtimeId:string,response:Record<string,unknown>){
  await invoke('omp_respond_ui',{runtimeId,response:{type:'extension_ui_response',...response}})
  await ompSnapshot(runtimeId)
}
export async function ompProfiles(){return invoke<{id:string;agentDir:string}[]>('omp_profiles')}
export async function ompOpenConfig(profileId:string){return invoke('omp_open_config',{profileId})}
export async function ompCliLogin(projectId:string,profileId:string){return invoke<PtyOpened>('omp_cli_login',{projectId,profileId})}
export function ompDraft(runtimeId:string,text:string){ompState.drafts[runtimeId]=(ompState.drafts[runtimeId]||'')+text}

let stop:UnlistenFn|undefined
const timers=new Map<string,ReturnType<typeof setTimeout>>()
const settledEvents=new Set<string>()
export function forgetOmpSession(runtimeId:string){
  const timer=timers.get(runtimeId)
  if(timer!==undefined)clearTimeout(timer)
  timers.delete(runtimeId);settledEvents.delete(runtimeId)
  for(const [key,request] of snapshotRequests)if(request.runtimeId===runtimeId){request.cancelled=true;snapshotRequests.delete(key)}
  delete ompState.sessions[runtimeId]
  delete ompState.drafts[runtimeId]
}
/** Apply frequent stream frames locally; history snapshots are reserved for boundaries. */
export function applyOmpEvent(runtimeId:string,seq:number,event:Record<string,any>,at=Date.now()):boolean {
  const s=ompState.sessions[runtimeId]
  if(!s)return false
  if(seq<=s.seq)return true
  s.seq=seq
  const activity=s.activity??(s.activity={phase:s.settled?'idle':'working'})
  if(event.type==='agent_start'){
    if(s.settled||!activity.startedAt)activity.startedAt=at
    activity.phase='working';activity.lastProgressAt=at
  }
  if(event.type==='message_update'){
    activity.lastProgressAt=at
    const kind=String(event.assistantMessageEvent?.type||'')
    if(kind.startsWith('thinking_'))activity.phase='thinking'
    else if(kind.startsWith('text_'))activity.phase='responding'
    else if(kind.startsWith('toolcall_'))activity.phase='tool'
  }
  if(['tool_execution_start','tool_execution_update','tool_stream_update'].includes(event.type)){activity.phase='tool';activity.lastProgressAt=at}
  if(['tool_execution_end','auto_retry_end'].includes(event.type)){activity.phase='working';activity.lastProgressAt=at}
  if(event.type==='auto_retry_start'){activity.phase='retrying';activity.lastProgressAt=at}
  if(event.type.startsWith('subagent_'))activity.lastProgressAt=at
  if(event.type==='prompt_result')activity.phase=event.sessionSettled?'idle':'background'
  if(event.type==='session_settled')activity.phase='idle'
  if(['message_start','message_update','message_end'].includes(event.type) && event.message){
    const id=event.messageId||event.message.id||`message-${s.messages.length}`
    const message={...event.message,_dockId:id}
    const index=s.messages.findIndex(m=>m._dockId===id)
    if(index>=0)s.messages[index]=message;else s.messages.push(message)
    return true
  }
  if(event.type==='tool_execution_update'){
    s.tools[event.toolCallId]={...s.tools[event.toolCallId],...event};return true
  }
  if(event.type==='agent_start'){s.settled=false;s.error=null;snapshotObserver?.(s)}
  if(event.type==='session_settled'){s.settled=true;snapshotObserver?.(s)}
  return false
}
let serviceGeneration=0
export async function startOmpService(onSnapshot:(snapshot:OmpSnapshot)=>void,onSettled:(id:string)=>void){
  const generation=++serviceGeneration
  stop?.()
  snapshotObserver=onSnapshot
  const unlisten=await listen<{runtimeId:string;seq?:number;event:Record<string,any>}>('omp-event',({payload:p})=>{
    if(generation!==serviceGeneration)return
    if(p.event.type==='disconnected'){
      const snapshot=p.event.snapshot as OmpSnapshot|undefined
      forgetOmpSession(p.runtimeId)
      // The process is already gone; do not fetch (and retain) its history again.
      if(snapshot)onSnapshot(snapshot)
      return
    }
    if(p.event.type==='session_settled') settledEvents.add(p.runtimeId)
    if(p.event.type==='extension_ui_request' && p.event.method==='set_editor_text') ompState.drafts[p.runtimeId]=p.event.text||''
    if(p.seq!==undefined && applyOmpEvent(p.runtimeId,p.seq,p.event) && p.event.type!=='session_settled')return
    if(timers.has(p.runtimeId)) return
    timers.set(p.runtimeId,setTimeout(()=>{
      timers.delete(p.runtimeId)
      void ompSnapshot(p.runtimeId).then(s=>{if(s.settled && settledEvents.delete(p.runtimeId)) onSettled(p.runtimeId)}).catch(()=>{})
    },100))
  })
  if(generation!==serviceGeneration)unlisten();else stop=unlisten
}
export function stopOmpService(){
  ++serviceGeneration;stop?.();stop=undefined;snapshotObserver=undefined
  for(const request of snapshotRequests.values())request.cancelled=true
  snapshotRequests.clear()
  for(const id of new Set([...Object.keys(ompState.sessions),...Object.keys(ompState.drafts),...timers.keys()]))forgetOmpSession(id)
}

/** Used by automatic orchestration: correlate this prompt, then wait for quiescence. */
// Only silence counts: long turns report progress and a dialog is waiting on a person.
export function ompDeadline(deadline:number,snapshot:Pick<OmpSnapshot,'activity'|'dialogs'>,now:number,patience:number){
  const progress=(snapshot.activity?.lastProgressAt??0)+patience
  return Math.max(deadline,progress,snapshot.dialogs.length?now+patience:0)
}
export async function runOmpPrompt(runtimeId:string,message:string,isCurrent:()=>boolean,timeoutMs=8*60*1000,onInputRequired?:(required:boolean)=>void){
  if(!(await ompSnapshot(runtimeId)).settled) throw new Error('OMP 会话仍在运行，请完成或停止后重试')
  const id=`dock-${crypto.randomUUID()}`
  const ack=await ompCommand(runtimeId,{type:'prompt',id,message})
  if(ack.data?.agentInvoked===false) return ''
  let deadline=Date.now()+timeoutMs
  while(Date.now()<deadline){
    if(!isCurrent()){await ompCommand(runtimeId,{type:'abort'});throw new Error('已取消')}
    const s=await ompSnapshot(runtimeId)
    onInputRequired?.(s.dialogs.length>0)
    deadline=ompDeadline(deadline,s,Date.now(),timeoutMs)
    if(!s.info.alive) throw new Error(s.error||'OMP 进程已退出')
    const result=s.promptResults[id]
    if(result?.status==='error') throw new Error(result.error?.message||'OMP 执行失败')
    if(result?.status==='aborted') throw new Error('OMP 回合已中止')
    if(result && s.settled) {
      const m=[...s.messages].reverse().find(m=>m.role==='assistant')
      return messageText(m?.content)
    }
    await new Promise(r=>setTimeout(r,300))
  }
  await ompCommand(runtimeId,{type:'abort'})
  throw new Error('等待 OMP 完成超时')
}
export function messageText(content:any):string {
  if(typeof content==='string')return content
  return Array.isArray(content)?content.filter(p=>p.type==='text').map(p=>p.text||'').join('\n'):''
}
