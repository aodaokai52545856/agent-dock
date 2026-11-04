// Actual OMP + a local scripted provider. Never uses a live model or credentials.
// node scripts/omp-rpc-smoke.mjs [omp executable]
import {createServer} from 'node:http'
import {spawn} from 'node:child_process'
import {createInterface} from 'node:readline'
import {mkdtempSync,mkdirSync,writeFileSync,readdirSync,readFileSync} from 'node:fs'
import {tmpdir,homedir} from 'node:os'
import {join,relative,resolve} from 'node:path'
import assert from 'node:assert/strict'

const root=mkdtempSync(join(tmpdir(),'agent-dock-omp-'))
const agent=join(root,'agent'),workspace=join(root,'workspace'),sessions=join(agent,'sessions','fixture')
for(const p of [agent,workspace,sessions])mkdirSync(p,{recursive:true})
writeFileSync(join(workspace,'fixture.md'),'# OMP local test\n')
let calls=0
const server=createServer(async(req,res)=>{
  let body='';for await(const chunk of req)body+=chunk
  if(req.method==='GET'){res.setHeader('content-type','application/json');res.end(JSON.stringify({data:[{id:'mock'}]}));return}
  const request=JSON.parse(body);assert.equal(request.model,'mock');calls++
  const tool=calls===1?{id:'read-fixture',type:'function',function:{name:'read',arguments:JSON.stringify({path:'fixture.md'})}}:
    calls===2?{id:'ask-fixture',type:'function',function:{name:'ask',arguments:JSON.stringify({questions:[{id:'choice',question:'Continue the local smoke test?',options:[{label:'Continue'},{label:'Stop'}],recommended:0}]})}}:null
  res.writeHead(200,{'content-type':'text/event-stream','cache-control':'no-cache'})
  const send=(choices,usage)=>res.write(`data: ${JSON.stringify({id:`response-${calls}`,object:'chat.completion.chunk',created:1,model:'mock',choices,...(usage?{usage}:{})})}\n\n`)
  send([{index:0,delta:{role:'assistant',reasoning_content:'The'},finish_reason:null}])
  await new Promise(r=>setTimeout(r,200))
  send([{index:0,delta:{reasoning_content:' local test is streaming.'},finish_reason:null}])
  send([{index:0,delta:{...(tool?{tool_calls:[{index:0,...tool}]}:{content:'OMP RPC smoke passed'})},finish_reason:null}])
  send([{index:0,delta:{},finish_reason:tool?'tool_calls':'stop'}])
  send([],{prompt_tokens:12,completion_tokens:8,total_tokens:20})
  res.end('data: [DONE]\n\n')
})
await new Promise(r=>server.listen(0,'127.0.0.1',r))
const baseUrl=`http://127.0.0.1:${server.address().port}/v1`
writeFileSync(join(agent,'models.yml'),JSON.stringify({providers:{'dock-smoke':{baseUrl,apiKey:'test-not-a-real-key',api:'openai-completions',models:[{id:'mock',name:'Local Mock',api:'openai-completions',reasoning:false,input:['text'],cost:{input:0,output:0,cacheRead:0,cacheWrite:0},contextWindow:128000,maxTokens:1024}]}}}))
writeFileSync(join(agent,'config.yml'),JSON.stringify({enabledProviders:['dock-smoke'],autoResume:false,'startup.quiet':true}))
const exe=process.argv[2]||'omp'
let frames=[],stderr=''
async function run(resume){
  const args=['--profile','default','--mode','rpc-ui','--model','dock-smoke/mock','--thinking','off','--no-title','--no-extensions','--no-skills','--no-context-files','--no-lsp','--session-dir',sessions,...(resume?['--resume',resume]:[])]
  const child=spawn(exe,args,{cwd:workspace,windowsHide:true,stdio:['pipe','pipe','pipe'],env:{...process.env,PI_CONFIG_DIR:relative(homedir(),root),PI_CODING_AGENT_DIR:agent,OMP_DAEMON_IDLE_GRACE_MS:'0'}})
  child.stderr.on('data',b=>{stderr+=b.toString()})
  const send=frame=>child.stdin.write(JSON.stringify(frame)+'\n')
  const readline=createInterface({input:child.stdout})
  return new Promise((resolveRun,reject)=>{
    const timer=setTimeout(()=>{child.kill();reject(new Error('OMP smoke timeout; frames='+frames.map(f=>f.type).join(',')))},45000)
    let completed=false,promptResult=false,stateId
    readline.on('line',line=>{
      let f;try{f=JSON.parse(line)}catch{return}frames.push(f)
      if(f.type==='ready')send({id:'protocol',type:'negotiate_protocol',protocolVersion:2})
      if(f.type==='response'&&f.id==='protocol')send({id:'ask-dialog',type:'set_ask_dialog',enabled:true})
      if(f.type==='response'&&f.id==='ask-dialog')send({id:'state',type:'get_state'})
      if(f.type==='response'&&f.id==='state'){
        if(!f.success){reject(new Error('get_state failed'));child.kill();return}
        stateId=f.data.sessionId
        if(resume){completed=true;child.stdin.end();return}
        send({id:'prompt',type:'prompt',message:'Run the local fixture test'})
      }
      if(f.type==='extension_ui_request'&&f.method==='ask')send({type:'extension_ui_response',id:f.id,answers:f.questions.map(q=>({id:q.id,selectedOptions:[q.options[0].label]}))})
      if(f.type==='extension_ui_request'&&f.method==='select')send({type:'extension_ui_response',id:f.id,value:f.options[0]})
      if(f.type==='prompt_result'&&f.id==='prompt'){
        assert.equal(f.status,'completed');promptResult=true
        if(f.sessionSettled){completed=true;child.stdin.end()}
      }
      if(f.type==='session_settled'&&promptResult){completed=true;child.stdin.end()}
    })
    child.on('error',e=>{clearTimeout(timer);reject(e)})
    child.on('exit',code=>{clearTimeout(timer);if(code!==0||!completed)reject(new Error(`OMP exited ${code}; stderr=${stderr.replace(/(?:sk-|eyJ)[A-Za-z0-9_.-]+/g,'[redacted]').slice(-1000)}`));else resolveRun(stateId)})
  })
}
try{
  const id=await run()
  assert.equal(calls,3)
  assert(frames.filter(f=>f.type==='message_update'&&f.assistantMessageEvent?.type==='thinking_delta').length>=2)
  assert(frames.some(f=>f.type==='tool_execution_end'&&f.toolName==='read'))
  assert(frames.some(f=>f.type==='extension_ui_request'&&['ask','select'].includes(f.method)))
  const journal=readdirSync(sessions).find(n=>n.endsWith('.jsonl'));assert(journal)
  const path=resolve(sessions,journal),rows=readFileSync(path,'utf8').split('\n').filter(Boolean).map(s=>JSON.parse(s))
  assert(rows.some(v=>v.message?.role==='assistant'&&v.message?.usage?.input>0))
  frames=[];const resumed=await run(path);assert.equal(resumed,id)
  console.log(JSON.stringify({ok:true,providerRequests:calls,cases:['real-rpc-v2','read-tool','ask-round-trip','prompt-result','settled','native-journal-usage','resume'],fixtureRoot:root}))
}finally{server.close()}
