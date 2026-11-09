import assert from "node:assert/strict";
import { sidebarWidth, storedSidebarWidth } from "./sidebar.ts";
import {
  compile,
  freshRequest,
  historyRequest,
  importCurl,
  exportCurl,
  pair,
  resolve,
  syncUrlParams,
  variables,
} from "./model.ts";
import { importData, exportCollection } from "./transfer.ts";
import { iterationData } from "./runner.ts";
import { SCRIPT_SHIM } from "./scriptShim.ts";
import { newQuickJSWASMModuleFromVariant } from "quickjs-emscripten-core";
import RELEASE_SYNC from "@jitl/quickjs-wasmfile-release-sync";

function test(name: string, fn: () => void) {
  fn();
  console.log(`ok ${name}`);
}
test("sidebar preference survives malformed storage and preserves editor space", () => {
  assert.equal(storedSidebarWidth(null), 288);
  assert.equal(storedSidebarWidth("bad"), 288);
  assert.equal(storedSidebarWidth("400"), 400);
  assert.equal(sidebarWidth(560, 1100), 480);
  assert.equal(sidebarWidth(10_000, 1440), 560);
  assert.equal(sidebarWidth(-100, 1440), 224);
  assert.equal(sidebarWidth(Number.NaN, 1440), 288);
});
test("scope order, disabled variables, nested and circular references", () => {
  const vars = variables(
    [pair("base", "http://localhost"), pair("token", "global")],
    [pair("token", "local"), { ...pair("disabled", "value"), enabled: false }],
  );
  assert.equal(resolve("{{base}}/{{token}}", vars), "http://localhost/local");
  assert.throws(() => resolve("{{disabled}}", vars), /未定义变量/);
  assert.throws(() => resolve("{{a}}", { a: "{{b}}", b: "{{a}}" }), /循环/);
  assert.throws(() => resolve("{{constructor}}", {}), /未定义变量/);
});
test("wire request preserves repeated query and headers, auth collisions fail", () => {
  const doc = freshRequest();
  doc.url = "http://localhost:1234/path";
  doc.params = [pair("q", "a&b"), pair("q", "中文")];
  doc.headers = [pair("X-Repeat", "1"), pair("X-Repeat", "2")];
  doc.auth.type = "bearer";
  doc.auth.token = "{{token}}";
  const request = compile(doc, { token: "test" });
  assert.deepEqual(new URL(request.url).searchParams.getAll("q"), [
    "a&b",
    "中文",
  ]);
  assert.equal(request.headers.length, 3);
  doc.headers.push(pair("authorization", "manual"));
  assert.throws(() => compile(doc, { token: "test" }), /冲突|同时存在/);
});
test("query synchronization preserves empty and duplicate values", () => {
  const doc = freshRequest();
  doc.url = "https://example.com/x?a=&a=2#part";
  syncUrlParams(doc);
  assert.equal(doc.url, "https://example.com/x#part");
  assert.deepEqual(
    doc.params.map((p) => p.value),
    ["", "2"],
  );
});
test("curl parsing is inert and export preserves all body modes", () => {
  const doc = importCurl(
    `curl 'https://example.com' -X POST -H 'X-Test: a:b' --data-raw '{"value":"$(whoami)"}'`,
  );
  assert.equal(doc.body, '{"value":"$(whoami)"}');
  assert.equal(doc.headers[0]?.value, "a:b");
  assert.match(exportCurl(doc), /--data-raw/);
  doc.bodyMode = "multipart";
  doc.fields = [{ ...pair("upload", "C:/file.bin"), file: true }];
  assert.match(exportCurl(doc), /--form/);
  assert.doesNotMatch(exportCurl(doc), /--data-raw/);
  assert.throws(
    () => importCurl("curl https://example.com --unknown value"),
    /不支持/,
  );
});
test("history does not retain auth, payload, cookie and secret parameters", () => {
  const doc = freshRequest();
  doc.url = "https://user:pass@example.com/?api_key=secret";
  doc.headers = [pair("Cookie", "secret")];
  doc.body = "secret";
  doc.auth.token = "secret";
  const history = JSON.stringify(historyRequest(doc));
  assert.ok(!history.includes("secret"));
  assert.ok(!history.includes("user:pass"));
});
test("Postman nested collection import and export preserve scripts and form bodies", () => {
  const imported = importData(
    JSON.stringify({
      info: { name: "Sample" },
      variable: [{ key: "base", value: "http://localhost" }],
      item: [
        {
          name: "Folder",
          item: [
            {
              name: "Upload",
              request: {
                method: "POST",
                url: "{{base}}/upload",
                body: {
                  mode: "formdata",
                  formdata: [
                    { key: "f", type: "file", src: ["a.txt", "b.txt"] },
                  ],
                },
              },
              event: [
                { listen: "test", script: { exec: ['pm.test("ok",()=>{})'] } },
              ],
            },
          ],
        },
      ],
    }),
  );
  assert.equal(imported.requests.length, 1);
  assert.equal(imported.documents.length, 2);
  assert.equal(imported.requests[0]!.fields.length, 2);
  const again = importData(JSON.stringify(exportCollection(imported.requests)));
  assert.equal(again.requests[0]!.postScript, 'pm.test("ok",()=>{})');
});
test("OpenAPI references resolve locally and reject missing/recursive references", () => {
  const data = {
    openapi: "3.1.0",
    info: { title: "Test" },
    servers: [{ url: "http://localhost" }],
    paths: {
      "/users": {
        get: { parameters: [{ $ref: "#/components/parameters/q" }] },
      },
    },
    components: {
      parameters: {
        q: { name: "q", in: "query", schema: { default: "hello" } },
      },
    },
  };
  const imported = importData(JSON.stringify(data));
  assert.equal(imported.requests[0]!.params[0]!.value, "hello");
  data.components.parameters.q = { $ref: "#/components/parameters/q" } as any;
  assert.throws(() => importData(JSON.stringify(data)), /循环/);
});
test("CSV handles quoted newlines, commas, empty cells and JSON data", () => {
  assert.deepEqual(iterationData('a,b\r\n"x,y","two\nlines"\r\n1,\r\n'), [
    { a: "x,y", b: "two\nlines" },
    { a: "1", b: "" },
  ]);
  assert.throws(() => iterationData("a,a\n1,2"), /重复/);
  assert.deepEqual(iterationData('[{"id":3}]'), [{ id: "3" }]);
});

test("curl roundtrip preserves fragment, form encoding, proxy and JSON file mode", () => {
  const source = freshRequest();
  source.url = "https://example.com/path#part";
  source.params = [pair("q", "a&b")];
  source.bodyMode = "json";
  source.body = '{"ok":true}';
  source.auth.type = "apikey";
  source.auth.key = "X-Key";
  source.auth.value = "test";
  const curl = exportCurl(source);
  const again = importCurl(curl);
  assert.equal(new URL(again.url).searchParams.get("q"), "a&b");
  assert.equal(new URL(again.url).hash, "#part");
  assert.equal(again.proxy, "direct");
  assert.equal(again.bodyMode, "json");
  assert.ok(again.headers.some((h) => h.key === "X-Key" && h.value === "test"));
  const form = importCurl(
    "curl https://example.com --data-urlencode 'q=a&b' --data-urlencode 'q=中文'",
  );
  assert.equal(form.bodyMode, "urlencoded");
  assert.deepEqual(
    form.fields.map((p) => p.value),
    ["a&b", "中文"],
  );
  const file = importCurl(
    "curl https://example.com -H 'Content-Type: application/json' --data-binary @body.json",
  );
  assert.equal(file.bodyMode, "binary");
  assert.equal(file.filePath, "body.json");
});
test("variable expansion stops before constructing an oversized result", () => {
  assert.throws(
    () => resolve("{{x}}".repeat(100), { x: "a".repeat(1024 * 1024) }),
    /2 MiB/,
  );
});

const qjs = await newQuickJSWASMModuleFromVariant(RELEASE_SYNC);
test("QuickJS sandbox pm tests, variable updates and absence of host access", () => {
  const runtime = qjs.newRuntime();
  runtime.setMemoryLimit(64 * 1024 * 1024);
  const vm = runtime.newContext();
  const value = vm.newString(
    JSON.stringify({
      request: freshRequest(),
      environment: { token: "old" },
      variables: {},
    }),
  );
  vm.setProp(vm.global, "__inputJSON", value);
  value.dispose();
  vm.unwrapResult(vm.evalCode(SCRIPT_SHIM)).dispose();
  vm.unwrapResult(
    vm.evalCode(
      `pm.environment.set('token','new');pm.test('updated',()=>pm.expect(pm.variables.get('token')).to.equal('new'));pm.test('isolated',()=>{pm.expect(typeof process).to.equal('undefined');pm.expect(typeof fetch).to.equal('undefined');pm.expect(typeof window).to.equal('undefined')});`,
    ),
  ).dispose();
  const result = vm.unwrapResult(vm.evalCode("__result()"));
  const data = JSON.parse(vm.getString(result));
  result.dispose();
  assert.equal(data.tests.length, 2);
  assert.ok(data.tests.every((t: any) => t.passed));
  assert.equal(data.environment.token, "new");
  vm.dispose();
  runtime.dispose();
});
test("QuickJS infinite loops interrupt and allocation stays within heap limit", () => {
  const rt = qjs.newRuntime();
  rt.setMemoryLimit(1024 * 1024);
  rt.setMaxStackSize(128 * 1024);
  let cycles = 0;
  rt.setInterruptHandler(() => ++cycles > 10);
  const vm = rt.newContext();
  const result = vm.evalCode("while(true){}");
  assert.ok(result.error);
  result.dispose();
  rt.setInterruptHandler(() => false);
  const allocation = vm.evalCode(
    'let a=[];for(let i=0;i<100000;i++)a.push("x".repeat(1000));',
  );
  assert.ok(allocation.error);
  allocation.dispose();
  vm.dispose();
  rt.dispose();
});
