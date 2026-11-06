import assert from 'node:assert/strict'
import { presentOmpMessages, presentOmpTool } from './ompPresentation.ts'
import { highlightOmpCode, renderOmpMarkdown, renderOmpTerminal } from './ompMarkdown.ts'

const call = {type:'toolCall', id:'call-1', name:'bash', arguments:{command:'rg concept src', i:'查找概念引用', timeout:30}}
const result = {role:'toolResult', toolCallId:'call-1', isError:false, content:[{type:'text', text:'src/concept.py:20: build_contract()'}]}
const messages = [{role:'assistant', content:[call]}, result]
const transcript = presentOmpMessages(messages, {}, true)
assert.equal(transcript.length, 1)
assert.equal(transcript[0].parts[0].tool.status, 'complete')
assert.equal(transcript[0].parts[0].tool.output, 'src/concept.py:20: build_contract()')
assert.equal(transcript[0].parts[0].tool.command, 'rg concept src')
assert.equal(transcript[0].parts[0].tool.description, '查找概念引用')
assert.equal(messages.length, 2)
assert.equal('tool' in call, false)
console.log('ok historical tool results join their calls without duplicates or mutating RPC history')

assert.equal(presentOmpTool(call, undefined, undefined, true).status, 'ended')
assert.equal(presentOmpTool(call, undefined, undefined, false).status, 'running')
assert.equal(presentOmpTool(call, {type:'tool_execution_end', isError:true}, undefined, false).status, 'error')
assert.equal(presentOmpTool(call, {partialResult:{content:[{type:'text',text:'streaming'}]}}, undefined, false).output, 'streaming')
assert.equal(presentOmpMessages([result], {}, true).length, 1)
assert.equal(presentOmpTool({...call, name:'read', arguments:{path:'src/lib.ts'}}, undefined, result, true).outputLanguage, 'typescript')
const image = {type:'image', data:'aGVsbG8=', mimeType:'image/png'}
assert.equal(presentOmpTool(call, undefined, {...result, content:[image]}, true).images.length, 1)
assert.equal(presentOmpMessages([{role:'compactionSummary',summary:'## 上下文\n\n保留结果'}], {}, true)[0].parts[0].text, '## 上下文\n\n保留结果')
console.log('ok live, failed, incomplete, orphaned and image results keep their correct presentation')

const html = renderOmpMarkdown('## 已完成\n\n**检查通过**，修改 `src/app.ts`。\n\n```ts\nconst result = true\n```\n\n| 项目 | 数量 |\n| --- | ---: |\n| 文件 | 2 |')
assert.match(html, /<h2>已完成<\/h2>/)
assert.match(html, /hljs-keyword/)
assert.match(html, /omp-code-copy/)
assert.match(html, /ad-table-scroll/)
assert.match(html, /align="right"/)
assert.match(highlightOmpCode('<script>alert(1)</script>', 'unsupported-language'), /&lt;script&gt;/)
assert.equal(highlightOmpCode('true', 'plaintext'), 'true')
assert.match(renderOmpMarkdown('```ts\nconst partial = "unfinished\n```'), /hljs-keyword/)
console.log('ok Markdown, tables, syntax highlighting, unknown languages and partial code blocks render')

const unsafe = renderOmpMarkdown('<img src=x onerror=alert(1)>\n\n[run](javascript:alert%281%29)\n\n![x](data:image/svg+xml;base64,abc)\n\n```html\n<script>alert(1)</script>\n```')
assert.equal(/<img\b|<script\b|href="javascript:/i.test(unsafe), false)
assert.match(renderOmpTerminal('\x1b[31mFAIL <script>\x1b[0m ok'), /class="omp-ansi-red">FAIL &lt;script&gt;<\/span> ok/)
assert.equal(renderOmpTerminal('\x1b]8;;https://example.com\x07link\x1b]8;;\x07'), 'link')
assert.match(renderOmpTerminal('src/app.ts:42: const x = "<script>"'), /omp-output-path">src\/app.ts<\/span>:<span class="omp-output-line">42<\/span>: const x = &quot;&lt;script&gt;&quot;/)
console.log('ok generated markup, code and ANSI output cannot inject executable HTML')
