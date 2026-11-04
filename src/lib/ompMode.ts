import { parseOmpLaunchMode, type OmpLaunchMode, type LiveSessionInfo } from './types.ts'
type ModeStorage={getItem(key:string):string|null;setItem(key:string,value:string):void}
const key=(projectId:string,sessionId:string)=>`ad-omp-launch:${JSON.stringify([projectId,sessionId])}`
function storage():ModeStorage|undefined { try{return typeof localStorage==='undefined'?undefined:localStorage}catch{return undefined} }
export function preferredOmpMode(projectId:string,sessionId?:string,fallback:unknown='wrapped',source=storage()):OmpLaunchMode {
  try {const saved=sessionId?source?.getItem(key(projectId,sessionId)):null;if(saved==='native'||saved==='wrapped')return saved}catch{}
  return parseOmpLaunchMode(fallback)
}
export function rememberOmpMode(info:LiveSessionInfo,target=storage()) {
  if(info.toolId!=='omp'||!info.sessionId||!['pty','ompRpc'].includes(info.kind||''))return
  try{target?.setItem(key(info.projectId,info.sessionId),info.kind==='ompRpc'?'wrapped':'native')}catch{}
}
