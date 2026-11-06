import assert from 'node:assert/strict'
import { applyOmpEvent, forgetOmpSession, ompDeadline, ompSnapshot, ompState, messageText, type OmpSnapshot } from './omp.ts'
import { preferredOmpMode,rememberOmpMode } from './ompMode.ts'
import { parseOmpLaunchMode } from './types.ts'

ompState.sessions['omp-test']={runtimeId:'omp-test',seq:0,messages:[],tools:{},state:{},settled:true,error:null} as unknown as OmpSnapshot
applyOmpEvent('omp-test',1,{type:'agent_start'})
assert.equal(ompState.sessions['omp-test'].settled,false)
for(const [seq,text] of [[2,'a'],[3,'ab'],[4,'abc']] as const){
  applyOmpEvent('omp-test',seq,{type:seq===4?'message_end':'message_update',messageId:'m',message:{role:'assistant',content:[{type:'text',text}]}})
}
assert.equal(ompState.sessions['omp-test'].messages.length,1)
assert.equal(messageText(ompState.sessions['omp-test'].messages[0].content),'abc')
applyOmpEvent('omp-test',2,{type:'message_update',messageId:'m',message:{role:'assistant',content:'stale'}})
assert.equal(messageText(ompState.sessions['omp-test'].messages[0].content),'abc')
applyOmpEvent('omp-test',5,{type:'session_settled'})
assert.equal(ompState.sessions['omp-test'].settled,true)
console.log('ok OMP streaming keeps one message, ignores stale events and finishes only when settled')

applyOmpEvent('omp-test',6,{type:'agent_start'},1_000)
applyOmpEvent('omp-test',7,{type:'message_update',messageId:'reasoning',message:{role:'assistant',content:[{type:'thinking',thinking:'The'}]},assistantMessageEvent:{type:'thinking_delta',delta:'The'}},2_000)
assert.equal(ompState.sessions['omp-test'].activity?.phase,'thinking')
assert.equal(ompState.sessions['omp-test'].activity?.startedAt,1_000)
assert.equal(ompState.sessions['omp-test'].activity?.lastProgressAt,2_000)
applyOmpEvent('omp-test',8,{type:'auto_retry_start'},3_000)
assert.equal(ompState.sessions['omp-test'].activity?.phase,'retrying')
applyOmpEvent('omp-test',9,{type:'prompt_result',sessionSettled:false},4_000)
assert.equal(ompState.sessions['omp-test'].activity?.phase,'background')
assert.equal(ompState.sessions['omp-test'].settled,false)
applyOmpEvent('omp-test',10,{type:'session_settled'},5_000)
assert.equal(ompState.sessions['omp-test'].activity?.phase,'idle')
console.log('ok OMP thinking, retry and background progress remain distinct from completion')

const memory=new Map<string,string>()
const storage={getItem:(key:string)=>memory.get(key)||null,setItem:(key:string,value:string)=>{memory.set(key,value)}}
assert.equal(parseOmpLaunchMode(undefined),'wrapped')
assert.equal(preferredOmpMode('project','session','native',storage),'native')
rememberOmpMode({toolId:'omp',projectId:'project',sessionId:'session',kind:'pty'} as any,storage)
assert.equal(preferredOmpMode('project','session','wrapped',storage),'native')
assert.equal(preferredOmpMode('other','session','wrapped',storage),'wrapped')
rememberOmpMode({toolId:'omp',projectId:'project',sessionId:'session',kind:'ompRpc'} as any,storage)
assert.equal(preferredOmpMode('project','session','native',storage),'wrapped')
console.log('ok OMP launch mode preserves old defaults and remembers each project/session choice')

let requests = 0
let finish: (value: OmpSnapshot) => void = () => {}
;(globalThis as any).window = { __TAURI_INTERNALS__: { invoke: () => {
  requests++
  return new Promise<OmpSnapshot>(resolve => { finish = resolve })
} } }
const original = { ...ompState.sessions['omp-test'], runtimeId:'omp-retired' } as OmpSnapshot
const first = ompSnapshot('omp-retired')
const second = ompSnapshot('omp-retired')
assert.equal(requests, 1)
ompState.drafts['omp-retired'] = 'large unsent draft'
forgetOmpSession('omp-retired')
finish(original)
await assert.rejects(first, /已关闭/)
await assert.rejects(second, /已关闭/)
assert.equal(ompState.sessions['omp-retired'], undefined)
assert.equal(ompState.drafts['omp-retired'], undefined)
const reopened = ompSnapshot('omp-retired')
assert.equal(requests, 2)
finish(original)
await reopened
forgetOmpSession('omp-retired')
assert.equal(ompState.sessions['omp-retired'], undefined)
console.log('ok OMP coalesces concurrent snapshots and cannot resurrect a closed session from an in-flight response')

assert.equal(ompDeadline(100,{activity:{phase:'tool',lastProgressAt:90},dialogs:[]},95,50),140)
assert.equal(ompDeadline(100,{activity:{phase:'tool',lastProgressAt:10},dialogs:[]},95,50),100)
assert.equal(ompDeadline(100,{dialogs:[]},95,50),100)
assert.equal(ompDeadline(100,{dialogs:[{}]},95,50),145)
console.log('ok OMP keeps waiting while there is progress or a dialog, and stops on silence')
