// Executed ONLY inside QuickJS. This string has no access to the browser or Tauri.
export const SCRIPT_SHIM = String.raw`
const __input = JSON.parse(__inputJSON);
let __pending = 0, __next = undefined, __skip = false, __serial = 0;
const __callbacks = new Map(), __tests = [], __logs = [];
function __scope(values) {
  const data = Object.assign(Object.create(null), values || {});
  return {get:k=>data[k], has:k=>Object.prototype.hasOwnProperty.call(data,k), set:(k,v)=>{data[k]=String(v)}, unset:k=>{delete data[k]}, clear:()=>{for(const k of Object.keys(data))delete data[k]}, toObject:()=>({...data}), replaceIn:s=>String(s).replace(/\{\{\s*([^{}]+?)\s*\}\}/g,(_,k)=>String(data[k]??''))};
}
function __expect(actual, negate=false, deep=false) {
  const check=(ok,message)=>{if(negate?ok:!ok)throw new Error(message);return chain};
  const chain={equal:v=>check(deep?JSON.stringify(actual)===JSON.stringify(v):actual===v,'expected '+JSON.stringify(actual)+(negate?' not':'')+' to equal '+JSON.stringify(v)), eql:v=>check(JSON.stringify(actual)===JSON.stringify(v),'values differ'), include:v=>check(typeof actual==='string'?actual.includes(v):Array.isArray(actual)?actual.includes(v):Object.entries(v).every(([k,x])=>actual[k]===x),'value not included'), property:function(k,v){check(actual!=null&&k in Object(actual),'missing property '+k);if(arguments.length>1)check(actual[k]===v,'property differs');return __expect(actual[k],negate)}, lengthOf:n=>check(actual?.length===n,'length differs'), oneOf:vs=>check(vs.includes(actual),'value not in expected set'), match:r=>check(r.test(actual),'pattern does not match'), above:n=>check(actual>n,'value not above '+n), below:n=>check(actual<n,'value not below '+n), within:(a,b)=>check(actual>=a&&actual<=b,'value outside range'), a:t=>check(typeof actual===t||(t==='array'&&Array.isArray(actual)),'type differs')};
  chain.equals=chain.equal;chain.an=chain.a;chain.length=chain.lengthOf;
  Object.defineProperty(chain,'deep',{get:()=>__expect(actual,negate,true)});
  for(const key of ['to','be','been','is','that','which','and','has','have','with','at','of','same'])Object.defineProperty(chain,key,{get:()=>chain});
  Object.defineProperty(chain,'not',{get:()=>__expect(actual,!negate,deep)});
  for(const [key,fn] of Object.entries({true:()=>actual===true,false:()=>actual===false,ok:()=>!!actual,null:()=>actual===null,undefined:()=>actual===undefined,empty:()=>actual!=null&&(actual.length===0||Object.keys(actual).length===0),exist:()=>actual!=null}))Object.defineProperty(chain,key,{get:()=>check(fn(),'assertion '+key+' failed')});
  return chain;
}
const __headers=(rows)=>({get:k=>rows.find(p=>p.key.toLowerCase()===k.toLowerCase())?.value, has:k=>rows.some(p=>p.key.toLowerCase()===k.toLowerCase()), add:p=>rows.push({...p,enabled:true}), upsert:p=>{const at=rows.findIndex(h=>h.key.toLowerCase()===p.key.toLowerCase());if(at<0)rows.push({...p,enabled:true});else rows[at]={...p,enabled:true}}, remove:k=>{for(let i=rows.length-1;i>=0;i--)if(rows[i].key.toLowerCase()===k.toLowerCase())rows.splice(i,1)}, all:()=>rows.slice(), toObject:()=>Object.fromEntries(rows.map(p=>[p.key,p.value]))});
const __request=__input.request||{}, __requestHeaders=__request.headers||[];
__request.headers=__headers(__requestHeaders);
const __response=__input.response, __responseBody=__input.responseBody||'';
const pm={
  globals:__scope(__input.globals), collectionVariables:__scope(__input.collection), environment:__scope(__input.environment), variables:__scope(__input.variables), iterationData:__scope(__input.data),
  info:{iteration:__input.iteration||0,iterationCount:__input.iterationCount||1,requestName:__request.name||'',requestId:__request.id||'',eventName:__response?'test':'prerequest'},
  request:__request,
  response:__response?{code:__response.status,status:__response.statusText,responseTime:__response.elapsedMs,responseSize:__response.bytes,headers:__headers(__response.headers||[]),text:()=>__responseBody,json:()=>JSON.parse(__responseBody),to:{have:{status:n=>__expect(__response.status).equal(n),header:k=>__expect(__response.headers.some(p=>p.key.toLowerCase()===k.toLowerCase())).true,jsonBody:()=>JSON.parse(__responseBody)}}}:undefined,
  expect:__expect,
  test:(name,fn)=>{const result={name:String(name),passed:false};if(__tests.length>=1000)throw new Error('最多 1000 条断言');__tests.push(result);try{const out=fn();if(out&&typeof out.then==='function'){__pending++;out.then(()=>{result.passed=true},e=>{result.error=String(e)}).finally(()=>__pending--)}else result.passed=true}catch(e){result.error=String(e)}},
  sendRequest:(request,callback)=>{const id=++__serial;if(id>32)throw new Error('脚本派生请求超过 32 次');__pending++;const promise=new Promise((resolve,reject)=>{__callbacks.set(id,{resolve,reject,callback});__send(id,JSON.stringify(request))});return promise},
  execution:{setNextRequest:id=>{__next=id},skipRequest:()=>{__skip=true}}
};
pm.test.skip=(name)=>__tests.push({name:String(name)+' (skipped)',passed:true});
const __localGet=pm.variables.get;
pm.variables.get=k=>pm.variables.has(k)?__localGet(k):pm.iterationData.has(k)?pm.iterationData.get(k):pm.environment.has(k)?pm.environment.get(k):pm.collectionVariables.has(k)?pm.collectionVariables.get(k):pm.globals.get(k);
pm.variables.replaceIn=s=>String(s).replace(/\{\{\s*([^{}]+?)\s*\}\}/g,(_,k)=>String(pm.variables.get(k)??''));
const console={log:(...args)=>{if(__logs.length<100)__logs.push(args.map(x=>typeof x==='string'?x:JSON.stringify(x)).join(' ').slice(0,2048))},warn:(...args)=>console.log(...args),error:(...args)=>console.log(...args),info:(...args)=>console.log(...args)};
function __resolveNetwork(id,json,error){const item=__callbacks.get(id);if(!item)return;__callbacks.delete(id);__pending--;try{if(error){item.callback?.(error);item.reject(new Error(error))}else{const r=JSON.parse(json),response={...r,code:r.status,json:()=>JSON.parse(r.body),text:()=>r.body,headers:__headers(r.headers||[])};item.callback?.(null,response);item.resolve(response)}}catch(e){item.reject(e)}}
function setTimeout(fn,delay=0){const id=++__serial;if(id>32)throw new Error('脚本定时器超过上限');__pending++;__callbacks.set(id,{timer:fn});__timer(id,Math.min(Math.max(Number(delay)||0,0),1000));return id}
function clearTimeout(id){if(__callbacks.delete(id))__pending--}
function __resolveTimer(id){const item=__callbacks.get(id);if(!item)return;__callbacks.delete(id);__pending--;item.timer()}
function __result(){return JSON.stringify({tests:__tests,logs:__logs,globals:pm.globals.toObject(),collection:pm.collectionVariables.toObject(),environment:pm.environment.toObject(),variables:pm.variables.toObject(),next:__next,skip:__skip,request:{...__request,headers:__requestHeaders}})}
`;
