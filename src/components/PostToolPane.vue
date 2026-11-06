<script setup lang="ts">
import {
  computed,
  defineAsyncComponent,
  nextTick,
  onMounted,
  onUnmounted,
  ref,
  watch,
} from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import { modal as vModal } from "../lib/modal";
import { showToast } from "../lib/store";
import PostToolPairs from "./PostToolPairs.vue";
import PostToolIcon from "./PostToolIcon.vue";
import * as api from "../lib/posttool/api";
import {
  compile,
  dirty,
  exportCurl,
  formatBytes,
  freshRequest,
  historyRequest,
  importCurl,
  METHODS,
  newTab,
  normalizeRequest,
  PREVIEW_LIMIT,
  publicUrl,
  syncUrlParams,
  uid,
  variables,
  type Entry,
  type Environment,
  type Pair,
  type RequestDoc,
  type Tab,
} from "../lib/posttool/model";
import { importData, exportCollection } from "../lib/posttool/transfer";
import { runScripts, type ScriptResult } from "../lib/posttool/scripts";
import { iterationData } from "../lib/posttool/runner";
import "../styles/posttool.css";
import "../styles/posttool-workbench.css";

const props = defineProps<{ active: boolean }>();
const environmentLoading = ref(false);
let globalsId = "";
const PostToolStream = defineAsyncComponent(
  () => import("./PostToolStream.vue"),
);
const PostToolOAuth = defineAsyncComponent(() => import("./PostToolOAuth.vue"));
const PostToolCollection = defineAsyncComponent(
  () => import("./PostToolCollection.vue"),
);
const collectionEditing = ref<Entry | null>(null);
const clientMode = ref("http"),
  streamOpened = ref(false);
watch(clientMode, (mode) => {
  if (mode === "stream") streamOpened.value = true;
});
const tabs = ref<Tab[]>([newTab()]),
  selected = ref(tabs.value[0]!.key);
const current = computed(
  () => tabs.value.find((t) => t.key === selected.value) ?? tabs.value[0]!,
);
const doc = computed(() => current.value.request);
const customMethodInput = ref<HTMLInputElement>();
const editorPanel = ref<HTMLElement>();
const customMethod = ref(false);
const methodSelection = computed(() =>
  customMethod.value || !METHODS.includes(doc.value.method)
    ? "CUSTOM"
    : doc.value.method,
);
const editorHeight = ref(208);
let resizeStart: { y: number; height: number; max: number } | undefined;
function chooseMethod(event: Event) {
  const value = (event.target as HTMLSelectElement).value;
  customMethod.value = value === "CUSTOM";
  if (customMethod.value) {
    doc.value.method = "";
    void nextTick(() => customMethodInput.value?.focus());
    return;
  }
  doc.value.method = value;
  if (["POST", "PUT", "PATCH"].includes(value)) {
    section.value = "body";
    editorHeight.value = 260;
    if (doc.value.bodyMode === "none") doc.value.bodyMode = "json";
  }
}
function beginEditorResize(event: PointerEvent) {
  if (event.button !== 0) return;
  const element = event.currentTarget as HTMLElement;
  resizeStart = {
    y: event.clientY,
    height: editorPanel.value?.clientHeight ?? editorHeight.value,
    max: Math.max(
      100,
      (element.closest(".pt-main")?.clientHeight ?? 700) - 444,
    ),
  };
  element.setPointerCapture(event.pointerId);
}
function moveEditorResize(event: PointerEvent) {
  if (resizeStart)
    editorHeight.value = Math.max(
      100,
      Math.min(
        resizeStart.max,
        resizeStart.height + event.clientY - resizeStart.y,
      ),
    );
}
function endEditorResize() {
  resizeStart = undefined;
}
function resizeEditorBy(delta: number) {
  editorHeight.value = Math.max(
    100,
    Math.min(
      Math.max(
        100,
        (editorPanel.value?.closest(".pt-main")?.clientHeight ?? 700) - 444,
      ),
      (editorPanel.value?.clientHeight ?? editorHeight.value) + delta,
    ),
  );
}
watch(
  () => props.active,
  (active) => {
    if (!active) endEditorResize();
  },
);
const section = ref("params"),
  responseView = ref("body"),
  pretty = ref(true);
const nav = ref<"request" | "history" | "draft">("request"),
  search = ref(""),
  page = ref(0),
  entries = ref<Entry[]>([]),
  total = ref(0),
  collections = ref<Entry[]>([]),
  folders = ref<Entry[]>([]),
  folder = ref("");
const environments = ref<Entry[]>([]),
  environmentId = ref(""),
  environment = ref<Environment>({ name: "无环境", values: [] }),
  globals = ref<Pair[]>([]);
const loading = ref(false),
  loadError = ref(""),
  notice = ref(""),
  booted = ref(false);
const modal = ref<
    "none" | "name" | "confirm" | "environment" | "import" | "export" | "runner"
  >("none"),
  modalTitle = ref(""),
  nameValue = ref(""),
  modalError = ref(""),
  importText = ref(""),
  transferBusy = ref(false);
const editEnv = ref<Environment>({ name: "新环境", values: [] }),
  editEnvId = ref(""),
  exportText = ref("");
const editEnvKind = ref("environment");
const controllers = new Map<string, AbortController>(),
  children = new Map<string, Set<string>>();
const runner = ref({
  running: false,
  paused: false,
  iterations: 1,
  delay: 0,
  stopOnError: false,
  completed: 0,
  passed: 0,
  failed: 0,
  total: 0,
  results: [] as {
    name: string;
    status: number;
    elapsed: number;
    error: string;
    passed: boolean;
  }[],
  dataText: "",
  error: "",
});
let runnerStop = false;
let action: (() => Promise<void> | void) | undefined;
let pollTimer: ReturnType<typeof setTimeout> | undefined,
  draftTimer: ReturnType<typeof setTimeout> | undefined,
  searchTimer: ReturnType<typeof setTimeout> | undefined,
  idleTimer: ReturnType<typeof setTimeout> | undefined;
let alive = true,
  listSeq = 0,
  envSeq = 0,
  pollSeq = 0;
const decoders = new Map<string, TextDecoder>();
const busyCount = computed(() => tabs.value.filter((t) => t.running).length);
const bodyText = computed(() => {
  const text = current.value.preview;
  if (current.value.running || !pretty.value || text.length > PREVIEW_LIMIT)
    return text;
  try {
    return JSON.stringify(JSON.parse(text), null, 2);
  } catch {
    return text;
  }
});
const breadcrumbs = ref<Entry[]>([]);
const parentOptions = computed(() => [...collections.value, ...folders.value]);
const countEnabled = (p: Pair[]) => p.filter((p) => p.enabled && p.key).length;
const canSend = computed(
  () =>
    !!doc.value.url.trim() &&
    !!doc.value.method.trim() &&
    !current.value.running &&
    !environmentLoading.value,
);
const bodyBytes = computed(
  () => new TextEncoder().encode(doc.value.body).length,
);
async function copyExport() {
  try {
    await navigator.clipboard.writeText(exportText.value);
    showToast("已复制 cURL");
  } catch (e) {
    modalError.value = String(e);
  }
}

function closeModal() {
  if (transferBusy.value) return;
  modal.value = "none";
  modalError.value = "";
  action = undefined;
}
function askName(
  title: string,
  value: string,
  fn: (name: string) => Promise<void> | void,
) {
  modalTitle.value = title;
  nameValue.value = value;
  modalError.value = "";
  action = () => fn(nameValue.value.trim());
  modal.value = "name";
}
function confirm(title: string, fn: () => Promise<void> | void) {
  modalTitle.value = title;
  modalError.value = "";
  action = fn;
  modal.value = "confirm";
}
async function commitModal() {
  if (!action) return;
  transferBusy.value = true;
  try {
    if (modal.value === "name" && !nameValue.value.trim())
      throw new Error("名称不能为空");
    await action();
    modal.value = "none";
    action = undefined;
  } catch (e) {
    modalError.value = String(e);
  } finally {
    transferBusy.value = false;
  }
}
async function guard(fn: () => Promise<void>) {
  try {
    await fn();
  } catch (e) {
    notice.value = String(e);
  }
}
function addTab(request = freshRequest(), saved = "") {
  if (tabs.value.length >= 12) {
    notice.value = "最多打开 12 个请求标签，请先关闭不需要的标签";
    return;
  }
  const tab = newTab(request, saved);
  tabs.value.push(tab);
  selected.value = tab.key;
}
async function closeTab(tab: Tab) {
  const close = async () => {
    await stop(tab);
    clearTimeout(draftTimer);
    if (tab.responseId) {
      if (!tab.running) await api.release(tab.responseId);
      decoders.delete(tab.responseId);
    }
    tabs.value = tabs.value.filter((t) => t.key !== tab.key);
    if (!tabs.value.length) tabs.value = [newTab()];
    if (selected.value === tab.key) selected.value = tabs.value[0]!.key;
    await api.remove(tab.key);
  };
  if (tab.running || dirty(tab))
    confirm(
      tab.running
        ? "关闭此标签并停止正在运行的请求？"
        : "关闭标签并丢弃未保存修改？",
      close,
    );
  else await guard(close);
}
async function refreshList() {
  const seq = ++listSeq;
  loading.value = true;
  loadError.value = "";
  try {
    const result = await api.list(
      nav.value,
      search.value,
      page.value,
      nav.value === "request" ? folder.value : undefined,
    );
    if (seq !== listSeq || !alive) return;
    entries.value = result.items;
    total.value = result.total;
  } catch (e) {
    if (seq === listSeq) loadError.value = String(e);
  } finally {
    if (seq === listSeq) loading.value = false;
  }
}
async function refreshStructure() {
  const [c, e, f, g] = await Promise.all([
    api.list("collection"),
    api.list("environment"),
    api.list("folder"),
    api.list("globals"),
  ]);
  collections.value = c.items;
  environments.value = e.items;
  folders.value = f.items;
  if (g.items[0]) {
    globalsId = g.items[0].id;
    globals.value = (await api.get<Environment>(g.items[0].id)).values;
  }
}
async function loadEntry(entry: Entry) {
  await guard(async () => {
    const payload = await api.get<unknown>(entry.id);
    const request = normalizeRequest(
      entry.kind === "history"
        ? (payload as { request: unknown }).request
        : payload,
    );
    if (entry.kind === "history" || entry.kind === "draft") request.id = uid();
    const existing = tabs.value.find((t) => t.request.id === request.id);
    if (existing) {
      selected.value = existing.key;
      return;
    }
    addTab(request, entry.kind === "request" ? JSON.stringify(request) : "");
  });
}
function enterFolder(entry?: Entry) {
  folder.value = entry?.id ?? "";
  breadcrumbs.value = entry ? [entry] : [];
  page.value = 0;
  void refreshList();
}
async function saveRequest() {
  await guard(async () => {
    const request = JSON.parse(JSON.stringify(doc.value)) as RequestDoc;
    if (!request.name.trim()) throw new Error("请填写请求名称");
    await api.put({
      ...api.requestEntry(request),
      url: publicUrl(request.url),
    });
    current.value.saved = JSON.stringify(doc.value);
    await api.remove(current.value.key);
    await refreshList();
    showToast("请求已保存到本机");
  });
}
function createCollection() {
  askName("新建集合", "新建集合", async (name) => {
    const id = uid();
    await api.put({
      id,
      kind: "collection",
      parent: "",
      name,
      method: "",
      url: "",
      updated: Date.now(),
      payload: {
        name,
        variables: [],
        auth: freshRequest().auth,
        preScript: "",
        postScript: "",
      },
    });
    await refreshStructure();
    enterFolder({
      id,
      kind: "collection",
      parent: "",
      name,
      method: "",
      url: "",
      updated: Date.now(),
    });
  });
}
function createFolder() {
  if (!folder.value) {
    notice.value = "请先选择一个集合";
    return;
  }
  askName("新建文件夹", "新建文件夹", async (name) => {
    await api.put({
      id: uid(),
      kind: "folder",
      parent: folder.value,
      name,
      method: "",
      url: "",
      updated: Date.now(),
      payload: {
        name,
        auth: { ...freshRequest().auth, type: "inherit" },
        preScript: "",
        postScript: "",
      },
    });
    await refreshStructure();
  });
}
function deleteEntry(entry: Entry) {
  confirm(`删除“${entry.name}”及其内容？此操作不可撤销。`, async () => {
    await api.remove(entry.id);
    await refreshStructure();
    await refreshList();
  });
}
function clearHistory() {
  confirm("清空本地请求历史？已保存的集合不会被删除。", async () => {
    await api.clearHistory();
    await refreshList();
  });
}
async function selectEnvironment() {
  const seq = ++envSeq;
  environmentLoading.value = true;
  try {
    const next = environmentId.value
      ? await api.get<Environment>(environmentId.value)
      : { name: "无环境", values: [] };
    if (seq === envSeq) environment.value = next;
  } catch (e) {
    if (seq === envSeq) {
      environment.value = { name: "加载失败", values: [] };
      notice.value = String(e);
    }
  } finally {
    if (seq === envSeq) environmentLoading.value = false;
  }
}
async function openEnvironment(id = environmentId.value) {
  await guard(async () => {
    editEnvKind.value = "environment";
    editEnvId.value = id;
    editEnv.value = id
      ? await api.get<Environment>(id)
      : { name: "新环境", values: [] };
    modal.value = "environment";
    modalError.value = "";
  });
}
async function openGlobals() {
  await guard(async () => {
    const list = await api.list("globals");
    editEnvKind.value = "globals";
    editEnvId.value = list.items[0]?.id ?? "";
    editEnv.value = {
      name: "全局变量",
      values: JSON.parse(JSON.stringify(globals.value)),
    };
    modal.value = "environment";
    modalError.value = "";
  });
}
async function saveEnvironment() {
  transferBusy.value = true;
  try {
    if (!editEnv.value.name.trim()) throw new Error("请填写环境名称");
    const id = editEnvId.value || uid();
    await api.put({
      id,
      kind: editEnvKind.value,
      parent: "",
      name: editEnv.value.name,
      method: "",
      url: "",
      updated: Date.now(),
      payload: JSON.parse(JSON.stringify(editEnv.value)),
    });
    if (editEnvKind.value === "environment") environmentId.value = id;
    await refreshStructure();
    await selectEnvironment();
    modal.value = "none";
  } catch (e) {
    modalError.value = String(e);
  } finally {
    transferBusy.value = false;
  }
}
async function pickFile(field: "filePath" | "caPath" | "identityPath") {
  await guard(async () => {
    const path = await open({ multiple: false, directory: false });
    if (typeof path === "string") doc.value[field] = path;
  });
}
function prettify() {
  try {
    doc.value.body = JSON.stringify(JSON.parse(doc.value.body), null, 2);
  } catch (e) {
    notice.value = `JSON 格式无效：${e}`;
  }
}

async function parentContext(request: RequestDoc) {
  const scope: Pair[][] = [globals.value];
  let inherited: RequestDoc["auth"] | undefined;
  let id = request.parent;
  const parents: {
    _id?: string;
    variables?: Pair[];
    auth?: RequestDoc["auth"];
    preScript?: string;
    postScript?: string;
  }[] = [];
  const visited = new Set<string>();
  while (id) {
    if (visited.has(id) || visited.size >= 16)
      throw new Error("集合层级循环或过深");
    visited.add(id);
    const payload = await api.get<(typeof parents)[number]>(id);
    payload._id = id;
    parents.unshift(payload);
    id = parentOptions.value.find((p) => p.id === id)?.parent ?? "";
  }
  for (const p of parents) {
    if (p.variables) scope.push(p.variables);
    if (p.auth && p.auth.type !== "inherit") inherited = p.auth;
  }
  return {
    vars: variables(...scope, environment.value.values),
    inherited,
    parents,
  };
}
async function scriptNetwork(raw: unknown, owner: Tab, signal: AbortSignal) {
  if (signal.aborted) throw new Error("请求已取消");
  const input =
    typeof raw === "string" ? { url: raw } : (raw as Record<string, any>);
  if (!input || typeof input.url !== "string")
    throw new Error("pm.sendRequest 需要 URL 字符串或包含 url 的对象");
  const request = freshRequest();
  request.url = input.url;
  request.method = input.method ?? "GET";
  request.headers = Array.isArray(input.header)
    ? input.header
    : typeof input.header === "object"
      ? Object.entries(input.header).map(([key, value]) => ({
          key,
          value: String(value),
          enabled: true,
        }))
      : [];
  if (input.body) {
    request.bodyMode = "text";
    request.body =
      typeof input.body === "string"
        ? input.body
        : String(input.body.raw ?? "");
  }
  request.proxy = owner.request.proxy;
  request.insecure = owner.request.insecure;
  const spec = compile(
    request,
    variables(globals.value, environment.value.values),
  );
  children.get(owner.key)?.add(spec.id);
  try {
    const response = await api.send(spec);
    if (response.error) throw new Error(response.error);
    const body = await fullScriptBody(response.id, response.bytes);
    return { ...response, body };
  } finally {
    children.get(owner.key)?.delete(spec.id);
    await api.release(spec.id);
  }
}
async function fullScriptBody(id: string, bytes: number) {
  if (bytes > 2 * 1024 * 1024)
    throw new Error("响应超过脚本正文上限 2 MiB；请缩小响应后执行断言");
  const decoder = new TextDecoder();
  let offset = 0,
    text = "";
  while (offset < bytes) {
    const chunk = await api.slice(id, offset);
    if (chunk.nextOffset === offset) throw new Error("响应文件尚未写入完成");
    text += decoder.decode(Uint8Array.from(chunk.data), { stream: true });
    offset = chunk.nextOffset;
  }
  return text + decoder.decode();
}
function scopePairs(values: Record<string, string>, previous: Pair[]) {
  return [
    ...previous.filter((p) => !p.enabled && !Object.hasOwn(values, p.key)),
    ...Object.entries(values).map(([key, value]) => ({
      ...previous.find((p) => p.key === key),
      key,
      value: String(value),
      enabled: true,
    })),
  ];
}
async function applyScript(
  result: ScriptResult,
  tab: Tab,
  context: Awaited<ReturnType<typeof parentContext>>,
) {
  tab.tests.push(...result.tests);
  const secrets = environment.value.values
    .filter((p) => p.secret && p.value)
    .map((p) => p.value);
  tab.logs.push(
    ...result.logs.map((line) =>
      secrets.reduce((s, secret) => s.split(secret).join("[redacted]"), line),
    ),
  );
  tab.logs = tab.logs.slice(-100);
  environment.value.values = scopePairs(
    result.environment,
    environment.value.values,
  );
  globals.value = scopePairs(result.globals, globals.value);
  const owner = [...context.parents]
    .reverse()
    .find((p) => p.variables !== undefined && p._id);
  if (
    owner &&
    JSON.stringify(variables(owner.variables ?? [])) !==
      JSON.stringify(result.collection)
  ) {
    owner.variables = scopePairs(result.collection, owner.variables ?? []);
    const entry = parentOptions.value.find((p) => p.id === owner._id);
    if (entry) {
      const { _id, ...payload } = owner;
      await api.put({ ...entry, payload });
    }
  }
  globalsId ||= uid();
  await api.put({
    id: globalsId,
    kind: "globals",
    parent: "",
    name: "全局变量",
    method: "",
    url: "",
    updated: Date.now(),
    payload: { name: "全局变量", values: globals.value },
  });
  if (environmentId.value)
    await api.put({
      id: environmentId.value,
      kind: "environment",
      parent: "",
      name: environment.value.name,
      method: "",
      url: "",
      updated: Date.now(),
      payload: environment.value,
    });
}
async function sendRequest(
  tab = current.value,
  data: Record<string, string> = {},
  iteration = 0,
  iterationCount = 1,
): Promise<ScriptResult | undefined> {
  if (tab.running || environmentLoading.value) return;
  tab.running = true;
  const controller = new AbortController();
  controllers.set(tab.key, controller);
  children.set(tab.key, new Set());
  notice.value = "";
  tab.error = "";
  tab.tests = [];
  tab.logs = [];
  try {
    if (tab.responseId) {
      await api.release(tab.responseId);
      decoders.delete(tab.responseId);
    }
    let snapshot = JSON.parse(JSON.stringify(tab.request)) as RequestDoc;
    const context = await parentContext(snapshot);
    let localVars = { ...context.vars, ...data };
    let scriptResult: ScriptResult | undefined;
    const scriptsInput = () => ({
      request: snapshot,
      globals: variables(globals.value),
      collection: variables(...context.parents.map((p) => p.variables ?? [])),
      environment: variables(environment.value.values),
      variables: scriptResult?.variables ?? {},
      data,
      iteration,
      iterationCount,
    });
    const pre = [
      ...context.parents.map((p) => p.preScript ?? ""),
      snapshot.preScript,
    ].filter((s) => s.trim());
    if (pre.length) {
      scriptResult = await runScripts(
        pre,
        scriptsInput(),
        controller.signal,
        (raw) => scriptNetwork(raw, tab, controller.signal),
      );
      await applyScript(scriptResult, tab, context);
      snapshot = normalizeRequest(scriptResult.request);
      localVars = {
        ...variables(
          globals.value,
          ...context.parents.map((p) => p.variables ?? []),
          environment.value.values,
        ),
        ...data,
        ...scriptResult.variables,
      };
      if (scriptResult.skip) {
        tab.logs.push("已跳过此请求");
        return scriptResult;
      }
    }
    if (controller.signal.aborted) throw new Error("请求已取消");
    const spec = compile(snapshot, localVars, context.inherited);
    tab.responseId = spec.id;
    tab.response = undefined;
    tab.preview = "";
    tab.offset = 0;
    tab.running = true;
    decoders.set(spec.id, new TextDecoder());
    startPolling();
    const response = await api.send(spec);
    tab.response = response;
    tab.error = response.error ?? "";
    const post = [
      ...context.parents.map((p) => p.postScript ?? ""),
      snapshot.postScript,
    ].filter((s) => s.trim());
    if (post.length && !response.error) {
      scriptResult = await runScripts(
        post,
        {
          ...scriptsInput(),
          response,
          responseBody: await fullScriptBody(response.id, response.bytes),
        },
        controller.signal,
        (raw) => scriptNetwork(raw, tab, controller.signal),
      );
      await applyScript(scriptResult, tab, context);
    }
    const history = historyRequest(snapshot);
    history.id = uid();
    await api
      .put({
        ...api.requestEntry(history, "history"),
        url: publicUrl(history.url),
        payload: {
          request: history,
          status: response.status,
          elapsedMs: response.elapsedMs,
          error: response.error,
        },
      })
      .catch((e) => {
        notice.value = `请求已完成，但历史保存失败：${e}`;
      });
    if (!tabs.value.some((t) => t.key === tab.key)) await api.release(spec.id);
    else if (props.active && selected.value === tab.key) await readPreview(tab);
    return scriptResult;
  } catch (e) {
    tab.error = String(e);
  } finally {
    tab.running = false;
    controllers.delete(tab.key);
    for (const id of children.get(tab.key) ?? []) void api.cancel(id);
    children.delete(tab.key);
    if (alive && nav.value === "history") void refreshList();
    if (!props.active && !busyCount.value) scheduleIdle();
  }
}
async function stop(tab = current.value) {
  controllers.get(tab.key)?.abort();
  await Promise.allSettled([
    ...(tab.responseId ? [api.cancel(tab.responseId)] : []),
    ...[...(children.get(tab.key) ?? [])].map((id) => api.cancel(id)),
  ]);
}
async function stopAll() {
  runnerStop = true;
  await Promise.allSettled(
    tabs.value.filter((t) => t.running).map((t) => stop(t)),
  );
}
function openRunner() {
  modal.value = "runner";
  modalTitle.value = "Collection Runner · 本机运行";
}
async function startRunner() {
  if (runner.value.running) return;
  runnerStop = false;
  runner.value.error = "";
  runner.value.results = [];
  runner.value.completed = 0;
  runner.value.passed = 0;
  runner.value.failed = 0;
  try {
    const data = iterationData(runner.value.dataText),
      iterations = Math.floor(runner.value.iterations);
    if (iterations < 1 || iterations > 1000)
      throw new Error("迭代次数为 1–1000");
    const parents = new Set([folder.value]);
    for (let n = 0; n < 16; n++)
      for (const f of folders.value)
        if (parents.has(f.parent)) parents.add(f.id);
    const records: Entry[] = [];
    for (let pageNo = 0; pageNo < 100; pageNo++) {
      const p = await api.list("request", "", pageNo);
      records.push(
        ...p.items.filter((e) => !folder.value || parents.has(e.parent)),
      );
      if ((pageNo + 1) * 100 >= p.total) break;
    }
    if (!records.length) throw new Error("请先保存要运行的请求到集合");
    records.reverse();
    const rounds = Math.max(data.length, iterations);
    if (rounds * records.length > 10000)
      throw new Error("单次运行最多 10,000 次请求，请减少迭代或缩小集合");
    runner.value.total = rounds * records.length;
    runner.value.running = true;
    runner.value.paused = false;
    let steps = 0;
    for (let round = 0; round < rounds && !runnerStop; round++) {
      for (let index = 0; index < records.length && !runnerStop; index++) {
        while (runner.value.paused && !runnerStop)
          await new Promise((resolve) => setTimeout(resolve, 100));
        if (runnerStop) break;
        if (++steps > 10000)
          throw new Error("运行步骤超过 10,000，已停止可能的循环");
        const request = normalizeRequest(await api.get(records[index]!.id));
        const tab = newTab(request, JSON.stringify(request));
        tabs.value.push(tab);
        selected.value = tab.key;
        const result = await sendRequest(
          tab,
          data[round % data.length] ?? {},
          round,
          rounds,
        );
        const passed =
          !tab.error &&
          (!tab.response || tab.response.status < 400) &&
          tab.tests.every((t) => t.passed);
        runner.value.completed++;
        if (passed) runner.value.passed++;
        else runner.value.failed++;
        runner.value.results.push({
          name: request.name,
          status: tab.response?.status ?? 0,
          elapsed: tab.response?.elapsedMs ?? 0,
          error: tab.error,
          passed,
        });
        runner.value.results = runner.value.results.slice(-200);
        if (tab.responseId) await api.release(tab.responseId);
        decoders.delete(tab.responseId ?? "");
        tabs.value = tabs.value.filter((t) => t.key !== tab.key);
        selected.value = tabs.value[0]!.key;
        if (!passed && runner.value.stopOnError) {
          runnerStop = true;
          break;
        }
        if (result?.next === null) break;
        if (typeof result?.next === "string") {
          const at = records.findIndex(
            (e) => e.id === result.next || e.name === result.next,
          );
          if (at < 0) throw new Error(`找不到后续请求：${result.next}`);
          index = at - 1;
        }
        if (runner.value.delay > 0)
          await new Promise((resolve) =>
            setTimeout(resolve, Math.min(runner.value.delay, 10000)),
          );
      }
    }
  } catch (e) {
    runner.value.error = String(e);
  } finally {
    runner.value.running = false;
    runner.value.paused = false;
  }
}
async function exportRunner() {
  await guard(async () => {
    const path = await save({ defaultPath: "postTool-run.json" });
    if (path) await api.writeFile(path, JSON.stringify(runner.value, null, 2));
  });
}
async function readPreview(tab: Tab) {
  const id = tab.responseId;
  if (!id) return;
  try {
    let result = await api.slice(id, tab.offset);
    if (tab.responseId !== id || !alive) return;
    // Follow the tail of sustained streams; response files retain the full bounded capture.
    if (
      result.meta.bytes - tab.offset > PREVIEW_LIMIT &&
      result.meta.state === "receiving"
    ) {
      tab.offset = Math.max(0, result.meta.bytes - PREVIEW_LIMIT);
      tab.preview = "";
      decoders.set(id, new TextDecoder());
      result = await api.slice(id, tab.offset);
    }
    tab.response = result.meta;
    const decoder = decoders.get(id) ?? new TextDecoder();
    decoders.set(id, decoder);
    tab.preview = (
      tab.preview +
      decoder.decode(Uint8Array.from(result.data), { stream: true })
    ).slice(-PREVIEW_LIMIT);
    tab.offset = result.nextOffset;
  } catch (e) {
    if (!tab.running) {
      tab.error = String(e);
      tab.responseId = undefined;
    }
  }
}
function startPolling() {
  clearTimeout(pollTimer);
  const seq = ++pollSeq;
  const tick = async () => {
    if (!alive || !props.active || seq !== pollSeq) return;
    const tab = current.value;
    if (
      tab.responseId &&
      (tab.running || (tab.response && tab.offset < tab.response.bytes))
    )
      await readPreview(tab);
    if (
      alive &&
      props.active &&
      seq === pollSeq &&
      tab.responseId &&
      (tab.running ||
        (tab.response &&
          tab.offset < Math.min(tab.response.bytes, PREVIEW_LIMIT)))
    )
      pollTimer = setTimeout(tick, 250);
  };
  void tick();
}
function scheduleIdle() {
  clearTimeout(idleTimer);
  idleTimer = setTimeout(() => {
    if (!props.active && !busyCount.value) {
      for (const tab of tabs.value) {
        tab.preview = "";
        tab.offset = 0;
        tab.responseId = undefined;
      }
      decoders.clear();
      void api.release(undefined, true).catch(() => {});
    }
  }, 30000);
}
async function saveResponse() {
  const id = current.value.responseId;
  if (!id) return;
  await guard(async () => {
    const path = await save({ defaultPath: "response.txt" });
    if (path) {
      await api.responseSave(id, path);
      showToast("响应已保存");
    }
  });
}
async function copyResponse() {
  await guard(async () => {
    await navigator.clipboard.writeText(bodyText.value);
    showToast("已复制当前预览");
  });
}
async function chooseImport() {
  await guard(async () => {
    const path = await open({
      multiple: false,
      filters: [
        { name: "API 数据", extensions: ["json", "yaml", "yml", "txt"] },
      ],
    });
    if (typeof path === "string") importText.value = await api.readFile(path);
  });
}
async function runImport() {
  transferBusy.value = true;
  modalError.value = "";
  try {
    const result = importText.value.trim().startsWith("curl")
      ? {
          requests: [importCurl(importText.value)],
          documents: [],
          warnings: [],
        }
      : importData(
          importText.value.trim().startsWith("{")
            ? importText.value
            : await api.parseDefinition(importText.value),
        );
    await api.importDocuments([
      ...result.documents,
      ...result.requests.map((request) => ({
        ...api.requestEntry(request),
        url: publicUrl(request.url),
      })),
    ]);
    await refreshStructure();
    await refreshList();
    if (result.requests[0])
      addTab(result.requests[0], JSON.stringify(result.requests[0]));
    modal.value = "none";
    notice.value = `导入完成：${result.requests.length} 个请求。${result.warnings.join("；")}`;
  } catch (e) {
    modalError.value = String(e);
  } finally {
    transferBusy.value = false;
  }
}
function openExport() {
  exportText.value = exportCurl(doc.value);
  modalTitle.value = "导出请求（POSIX cURL）";
  modal.value = "export";
}
async function exportJson() {
  await guard(async () => {
    const data = exportCollection([JSON.parse(JSON.stringify(doc.value))]);
    const path = await save({
      defaultPath: "postTool.postman_collection.json",
    });
    if (path) {
      await api.writeFile(path, JSON.stringify(data, null, 2));
      showToast("集合已导出；请检查文件中的敏感信息");
    }
  });
}
function keyboard(event: KeyboardEvent) {
  if (!props.active || modal.value !== "none" || event.isComposing) return;
  if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
    event.preventDefault();
    void sendRequest();
  }
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
    event.preventDefault();
    void saveRequest();
  }
}
watch([nav, folder], () => {
  page.value = 0;
  void refreshList();
});
watch(search, () => {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(() => {
    page.value = 0;
    void refreshList();
  }, 250);
});
watch(page, () => void refreshList());
watch(environmentId, () => void selectEnvironment());
watch(selected, () => {
  customMethod.value = !METHODS.includes(doc.value.method);
  pretty.value = true;
  startPolling();
});
watch(
  () => JSON.stringify(doc.value),
  () => {
    clearTimeout(draftTimer);
    const tab = current.value;
    draftTimer = setTimeout(() => {
      if (!dirty(tab) || !api.desktop) return;
      void api
        .put({
          ...api.requestEntry(tab.request, "draft"),
          id: tab.key,
          payload: JSON.parse(JSON.stringify(tab.request)),
        })
        .catch((e) => {
          notice.value = `草稿保存失败：${e}`;
        });
    }, 1000);
  },
);
watch(
  () => props.active,
  (active) => {
    if (active) {
      clearTimeout(idleTimer);
      startPolling();
    } else {
      pollSeq++;
      clearTimeout(pollTimer);
      scheduleIdle();
    }
  },
);
onMounted(async () => {
  window.addEventListener("keydown", keyboard);
  if (!api.desktop) {
    loadError.value = "网络和本地存储功能需在 Agent Dock 桌面应用中使用";
    return;
  }
  try {
    await refreshStructure();
    await refreshList();
    booted.value = true;
  } catch (e) {
    loadError.value = String(e);
  }
});
onUnmounted(() => {
  endEditorResize();
  alive = false;
  pollSeq++;
  listSeq++;
  clearTimeout(pollTimer);
  clearTimeout(draftTimer);
  clearTimeout(searchTimer);
  clearTimeout(idleTimer);
  window.removeEventListener("keydown", keyboard);
  void stopAll();
  decoders.clear();
  void api.release(undefined, true).catch(() => {});
});
</script>

<template>
  <section class="posttool" aria-label="postTool API 工作台">
    <aside v-if="active" class="pt-sidebar">
      <div class="pt-brand">
        <span class="pt-brand-mark"><PostToolIcon name="bolt" /></span>
        <div><strong>postTool</strong><span>API 工作台</span></div>
        <span class="pt-local-tag" title="数据保存在本机">LOCAL</span>
      </div>
      <div class="pt-side-actions">
        <button class="pt-new-request" @click="addTab()">
          <PostToolIcon name="plus" />新建请求
        </button>
        <button
          class="pt-icon-button"
          aria-label="新建集合"
          title="新建集合"
          @click="createCollection"
        >
          <PostToolIcon name="folder" />
        </button>
      </div>
      <div class="pt-side-tabs" role="tablist" aria-label="请求导航">
        <button
          v-for="item in ['request', 'history', 'draft'] as const"
          :key="item"
          role="tab"
          :aria-selected="nav === item"
          :class="{ selected: nav === item }"
          @click="nav = item"
        >
          <PostToolIcon
            :name="
              { request: 'collection', history: 'history', draft: 'file' }[item]
            "
          />
          {{ { request: "集合", history: "历史", draft: "草稿" }[item] }}
        </button>
      </div>
      <label class="pt-search-wrap"
        ><PostToolIcon name="search" /><input
          v-model="search"
          class="pt-search"
          aria-label="搜索请求"
          placeholder="搜索请求或 URL"
      /></label>
      <div v-if="nav === 'request'" class="pt-collections">
        <div class="pt-section-caption">
          <span>我的集合</span><span>{{ collections.length }}</span>
        </div>
        <button :class="{ selected: !folder }" @click="enterFolder()">
          未分组请求
        </button>
        <div
          v-for="collection in collections"
          :key="collection.id"
          class="pt-folder-row"
        >
          <button
            :class="{ selected: folder === collection.id }"
            @click="enterFolder(collection)"
          >
            <PostToolIcon name="folder" />{{ collection.name }}
          </button>
          <button
            :aria-label="`设置集合 ${collection.name}`"
            @click="collectionEditing = collection"
          >
            <PostToolIcon name="settings" /></button
          ><button
            :aria-label="`删除集合 ${collection.name}`"
            @click="deleteEntry(collection)"
          >
            ×
          </button>
        </div>
        <div
          v-for="item in folders.filter((f) => f.parent === folder)"
          :key="item.id"
          class="pt-folder-row"
        >
          <button @click="enterFolder(item)">
            <PostToolIcon name="folder" />{{ item.name }}
          </button>
          <button
            :aria-label="`设置文件夹 ${item.name}`"
            @click="collectionEditing = item"
          >
            <PostToolIcon name="settings" /></button
          ><button
            :aria-label="`删除文件夹 ${item.name}`"
            @click="deleteEntry(item)"
          >
            ×
          </button>
        </div>
        <button v-if="folder" class="pt-add-row" @click="createFolder">
          ＋ 文件夹
        </button>
      </div>
      <div v-if="breadcrumbs.length" class="pt-breadcrumb">
        <button @click="enterFolder()">集合</button
        ><span>/ {{ breadcrumbs[0]?.name }}</span>
      </div>
      <div class="pt-entry-list">
        <p v-if="loading" class="pt-empty">正在读取…</p>
        <div v-else-if="loadError" class="pt-error">
          {{ loadError }}<button @click="refreshList">重试</button>
        </div>
        <div v-else-if="!entries.length" class="pt-empty">
          <PostToolIcon :name="search ? 'search' : 'collection'" />
          <span>{{ search ? "未找到符合条件的请求" : "这里还没有请求" }}</span
          ><button @click="search ? (search = '') : addTab()">
            {{ search ? "清除筛选" : "新建请求" }}
          </button>
        </div>
        <div
          v-for="entry in entries"
          :key="entry.id"
          :class="['pt-entry-row', { selected: doc.id === entry.id }]"
        >
          <button :title="entry.url || entry.name" @click="loadEntry(entry)">
            <span class="pt-method" :data-method="entry.method">{{
              entry.method
            }}</span
            ><span class="pt-entry-info"
              ><span>{{ entry.name }}</span
              ><small v-if="entry.url">{{ entry.url }}</small></span
            ></button
          ><button
            :aria-label="`删除 ${entry.name}`"
            @click="deleteEntry(entry)"
          >
            ×
          </button>
        </div>
      </div>
      <div class="pt-pagination">
        <button :disabled="page === 0" @click="page--">‹</button
        ><span
          >{{ total }} 条 · {{ page + 1 }}/{{
            Math.max(1, Math.ceil(total / 100))
          }}</span
        ><button :disabled="(page + 1) * 100 >= total" @click="page++">
          ›
        </button>
      </div>
      <button v-if="nav === 'history'" class="pt-clear" @click="clearHistory">
        清空历史
      </button>
      <div class="pt-side-footer">
        <button
          @click="
            modal = 'import';
            modalError = '';
          "
        >
          导入</button
        ><button @click="openExport">导出</button
        ><span><PostToolIcon name="shield" />本地存储</span>
      </div>
    </aside>
    <main class="pt-main">
      <header v-if="active" class="pt-workbar">
        <div class="pt-client-switch" role="group" aria-label="客户端类型">
          <button
            :class="{ selected: clientMode === 'http' }"
            :aria-pressed="clientMode === 'http'"
            title="HTTP、GraphQL 和 SSE"
            @click="clientMode = 'http'"
          >
            <PostToolIcon name="globe" />HTTP
          </button>
          <button
            :class="{ selected: clientMode === 'stream' }"
            :aria-pressed="clientMode === 'stream'"
            title="WebSocket、Socket.IO、MQTT 和 gRPC"
            @click="clientMode = 'stream'"
          >
            <PostToolIcon name="bolt" />实时 / RPC
          </button>
        </div>
        <span class="pt-toolbar-divider" />
        <button class="pt-runner-button" @click="openRunner">
          <PostToolIcon name="play" />Runner
        </button>
        <button class="pt-globals-button" @click="openGlobals">
          <PostToolIcon name="code" />全局变量
        </button>
        <div class="pt-spacer" />
        <span v-if="busyCount" class="pt-live"
          >{{ busyCount }} 个请求运行中</span
        >
        <button
          v-if="busyCount"
          class="pt-icon-button"
          aria-label="全部停止"
          title="全部停止"
          @click="stopAll"
        >
          <PostToolIcon name="stop" />
        </button>
        <div class="pt-environment">
          <PostToolIcon name="globe" /><select
            v-model="environmentId"
            :disabled="busyCount > 0 || runner.running"
            aria-label="当前环境"
          >
            <option value="">无环境</option>
            <option v-for="env in environments" :key="env.id" :value="env.id">
              {{ env.name }}
            </option>
          </select>
          <button
            class="pt-icon-button"
            :aria-label="environmentId ? '编辑环境' : '新建环境'"
            :title="environmentId ? '编辑环境' : '新建环境'"
            @click="openEnvironment()"
          >
            <PostToolIcon :name="environmentId ? 'settings' : 'plus'" />
          </button>
          <button
            v-if="environmentId"
            class="pt-icon-button"
            aria-label="新建环境"
            title="新建环境"
            @click="openEnvironment('')"
          >
            <PostToolIcon name="plus" />
          </button>
        </div>
      </header>
      <PostToolStream
        v-if="streamOpened"
        :active="active && clientMode === 'stream'"
      /><template v-if="active && clientMode === 'http'"
        ><div class="pt-tabs" role="tablist" aria-label="打开的请求">
          <div
            v-for="tab in tabs"
            :key="tab.key"
            :class="['pt-tab', { selected: selected === tab.key }]"
          >
            <button
              role="tab"
              :aria-selected="selected === tab.key"
              @click="selected = tab.key"
            >
              <span class="pt-method" :data-method="tab.request.method">{{
                tab.request.method
              }}</span
              ><span class="pt-tab-name">{{ tab.request.name }}</span
              ><span v-if="tab.running" class="pt-dot">●</span
              ><span v-else-if="dirty(tab)" class="pt-dirty" title="未保存"
                >•</span
              ></button
            ><button
              :aria-label="`关闭 ${tab.request.name}`"
              @click="closeTab(tab)"
            >
              ×
            </button>
          </div>
          <button
            class="pt-new-tab"
            aria-label="新建请求标签"
            @click="addTab()"
          >
            ＋
          </button>
        </div>
        <div v-if="notice" class="pt-notice" role="status">
          <span>{{ notice }}</span
          ><button aria-label="关闭提示" @click="notice = ''">×</button>
        </div>
        <div class="pt-request-heading">
          <PostToolIcon name="file" />
          <input
            v-model="doc.name"
            aria-label="请求名称"
            placeholder="请求名称"
          />
          <span v-if="dirty(current)" class="pt-unsaved-label">未保存</span>
          <select v-model="doc.parent" aria-label="所属集合">
            <option value="">未分组</option>
            <option
              v-for="parent in parentOptions"
              :key="parent.id"
              :value="parent.id"
            >
              {{ parent.kind === "folder" ? "↳ " : "" }}{{ parent.name }}
            </option>
          </select>
          <button
            class="pt-save-button"
            :disabled="!api.desktop"
            @click="saveRequest"
          >
            <PostToolIcon name="save" />保存 <kbd>Ctrl S</kbd>
          </button>
          <button
            class="pt-icon-button"
            aria-label="复制请求"
            title="复制请求"
            @click="
              addTab({
                ...JSON.parse(JSON.stringify(doc)),
                id: uid(),
                name: doc.name + ' 副本',
              })
            "
          >
            <PostToolIcon name="copy" />
          </button>
        </div>
        <form class="pt-urlbar" @submit.prevent="sendRequest()">
          <div class="pt-url-field" :data-method="doc.method">
            <div class="pt-method-control">
              <select
                :value="methodSelection"
                class="pt-method-select"
                :data-method="doc.method"
                aria-label="HTTP 方法"
                title="选择 HTTP 请求方法"
                @change="chooseMethod"
              >
                <option v-for="method in METHODS" :key="method" :value="method">
                  {{ method }}
                </option>
                <option value="CUSTOM">自定义…</option></select
              ><PostToolIcon name="chevron" class="pt-method-chevron" />
            </div>
            <input
              v-if="methodSelection === 'CUSTOM'"
              ref="customMethodInput"
              v-model="doc.method"
              class="pt-custom-method"
              aria-label="自定义 HTTP 方法"
              placeholder="METHOD"
              required
              @blur="doc.method = doc.method.trim().toUpperCase()"
            />
            <input
              v-model="doc.url"
              required
              aria-label="请求 URL"
              class="pt-url-input"
              placeholder="输入请求 URL，例如 https://api.example.com/users"
              spellcheck="false"
              @blur="syncUrlParams(doc)"
            />
          </div>
          <button
            v-if="current.running"
            type="button"
            class="pt-stop"
            @click="stop()"
          >
            <PostToolIcon name="stop" />停止
          </button>
          <button
            v-else
            type="submit"
            class="pt-send"
            :disabled="!canSend"
            title="发送请求（Ctrl / ⌘ + Enter）"
          >
            发送<PostToolIcon name="send" />
          </button>
        </form>
        <nav class="pt-sections" aria-label="请求配置">
          <button
            v-for="s in [
              { id: 'params', name: 'Params', count: countEnabled(doc.params) },
              { id: 'auth', name: 'Authorization' },
              {
                id: 'headers',
                name: 'Headers',
                count: countEnabled(doc.headers),
              },
              { id: 'body', name: 'Body' },
              { id: 'scripts', name: 'Scripts' },
              { id: 'settings', name: 'Settings' },
              { id: 'description', name: '说明' },
            ]"
            :key="s.id"
            :class="{ selected: section === s.id }"
            @click="section = s.id"
          >
            {{ s.name }}<span v-if="s.count">{{ s.count }}</span>
          </button>
        </nav>
        <div
          ref="editorPanel"
          class="pt-editor-pane"
          :style="{ height: editorHeight + 'px' }"
        >
          <PostToolPairs
            v-if="section === 'params'"
            v-model="doc.params"
            key-label="Query 参数"
          />
          <PostToolPairs
            v-else-if="section === 'headers'"
            v-model="doc.headers"
            key-label="Header"
          />
          <div v-else-if="section === 'auth'" class="pt-form">
            <label
              >鉴权类型<select v-model="doc.auth.type">
                <option value="none">No Auth</option>
                <option value="inherit">继承父级</option>
                <option value="bearer">Bearer Token</option>
                <option value="basic">Basic Auth</option>
                <option value="apikey">API Key</option>
                <option value="oauth2">OAuth 2.0</option>
              </select></label
            ><template v-if="doc.auth.type === 'bearer'"
              ><label
                >Token<input
                  v-model="doc.auth.token"
                  type="password"
                  autocomplete="off"
                  placeholder="{{token}}" /></label></template
            ><template v-if="doc.auth.type === 'basic'"
              ><label
                >用户名<input
                  v-model="doc.auth.username"
                  autocomplete="off" /></label
              ><label
                >密码<input
                  v-model="doc.auth.password"
                  type="password"
                  autocomplete="off" /></label></template
            ><template v-if="doc.auth.type === 'apikey'"
              ><label>Key<input v-model="doc.auth.key" /></label
              ><label
                >Value<input
                  v-model="doc.auth.value"
                  type="password"
                  autocomplete="off" /></label
              ><label
                >位置<select v-model="doc.auth.location">
                  <option value="header">Header</option>
                  <option value="query">Query</option>
                </select></label
              ></template
            ><PostToolOAuth
              v-if="doc.auth.type === 'oauth2'"
              v-model="doc.auth"
              :request="doc"
            />
            <p class="pt-hint">
              鉴权值可引用环境变量。自动生成的鉴权头与手填 Header 冲突时会提示。
            </p>
          </div>
          <div v-else-if="section === 'body'" class="pt-body-panel">
            <div class="pt-body-toolbar">
              <select v-model="doc.bodyMode" aria-label="正文类型">
                <option value="none">none</option>
                <option value="json">JSON</option>
                <option value="text">Text</option>
                <option value="xml">XML / SOAP</option>
                <option value="html">HTML</option>
                <option value="urlencoded">x-www-form-urlencoded</option>
                <option value="multipart">form-data</option>
                <option value="binary">binary 文件</option>
                <option value="graphql">GraphQL</option></select
              ><button v-if="doc.bodyMode === 'json'" @click="prettify">
                格式化 JSON</button
              ><span class="pt-hint">{{ formatBytes(bodyBytes) }}</span>
            </div>
            <p v-if="doc.bodyMode === 'none'" class="pt-empty">
              此请求没有正文
            </p>
            <PostToolPairs
              v-else-if="
                doc.bodyMode === 'urlencoded' || doc.bodyMode === 'multipart'
              "
              v-model="doc.fields"
              :files="doc.bodyMode === 'multipart'"
            />
            <div v-else-if="doc.bodyMode === 'binary'" class="pt-file-select">
              <input
                v-model="doc.filePath"
                aria-label="上传文件路径"
                placeholder="选择文件，直接流式上传"
              /><button @click="pickFile('filePath')">选择文件</button>
            </div>
            <div v-else class="pt-code-body">
              <textarea
                v-model="doc.body"
                aria-label="请求正文"
                spellcheck="false"
                :maxlength="2 * 1024 * 1024"
                placeholder="输入请求正文，支持 {{variables}}"
              />
              <div v-if="doc.bodyMode === 'graphql'" class="pt-graphql">
                <label
                  >Variables<textarea
                    v-model="doc.graphqlVariables"
                    aria-label="GraphQL Variables"
                    spellcheck="false"
                  /></label
                ><label
                  >Operation name<input v-model="doc.operationName"
                /></label>
              </div>
            </div>
          </div>
          <div v-else-if="section === 'scripts'" class="pt-script-grid">
            <label
              >Pre-request<textarea
                v-model="doc.preScript"
                aria-label="前置脚本"
                spellcheck="false"
                placeholder="pm.environment.set('token', '...')"
              /></label
            ><label
              >Post-response<textarea
                v-model="doc.postScript"
                aria-label="后置脚本"
                spellcheck="false"
                placeholder="pm.test('status', () => pm.response.to.have.status(200))"
              />
            </label>
          </div>
          <div v-else-if="section === 'settings'" class="pt-form pt-settings">
            <label
              >总超时（毫秒）<input
                v-model.number="doc.timeoutMs"
                type="number"
                min="100"
                max="3600000" /></label
            ><label
              >代理<input
                v-model="doc.proxy"
                placeholder="direct / 系统代理留空 / http:// / socks5://" /></label
            ><label class="pt-check"
              ><input
                v-model="doc.followRedirects"
                type="checkbox"
              />跟随重定向（最多 10 次）</label
            ><label class="pt-check"
              ><input v-model="doc.cookies" type="checkbox" />启用 Cookie
              Jar</label
            ><label class="pt-check"
              ><input v-model="doc.insecure" type="checkbox" />跳过 TLS
              证书校验</label
            ><label
              >CA 证书（PEM）
              <div class="pt-file-select">
                <input v-model="doc.caPath" /><button
                  @click="pickFile('caPath')"
                >
                  选择
                </button>
              </div></label
            ><label
              >客户端证书与私钥（PEM）
              <div class="pt-file-select">
                <input v-model="doc.identityPath" /><button
                  @click="pickFile('identityPath')"
                >
                  选择
                </button>
              </div></label
            >
          </div>
          <textarea
            v-else-if="section === 'description'"
            v-model="doc.description"
            class="pt-description"
            aria-label="请求说明"
            placeholder="请求说明与使用示例"
          />
        </div>
        <div
          class="pt-editor-divider"
          role="separator"
          aria-label="调整请求与响应区高度"
          aria-orientation="horizontal"
          :aria-valuenow="editorHeight"
          tabindex="0"
          title="拖动调整高度，或使用上下方向键"
          @pointerdown="beginEditorResize"
          @pointermove="moveEditorResize"
          @pointerup="endEditorResize"
          @pointercancel="endEditorResize"
          @lostpointercapture="endEditorResize"
          @keydown.up.prevent="resizeEditorBy(-24)"
          @keydown.down.prevent="resizeEditorBy(24)"
        >
          <span />
        </div>
        <section class="pt-response" aria-label="响应">
          <header class="pt-response-bar">
            <strong>响应<span>RESPONSE</span></strong>
            <span
              v-if="current.response?.status"
              :class="[
                'pt-status-badge',
                current.response.status < 400 ? 'pt-success' : 'pt-error-text',
              ]"
              ><span class="pt-status-dot" />{{ current.response.status }}
              {{ current.response.statusText }}</span
            >
            <span v-if="current.response" class="pt-response-metric"
              ><span>耗时</span
              ><b>{{ current.response.elapsedMs }}<small>ms</small></b></span
            >
            <span v-if="current.response" class="pt-response-metric"
              ><span>大小</span
              ><b>{{ formatBytes(current.response.bytes) }}</b></span
            >
            <span v-if="current.running" class="pt-live">{{
              current.response?.state === "receiving" ? "接收中…" : "连接中…"
            }}</span>
            <div class="pt-spacer" />
            <button
              v-if="current.responseId"
              :disabled="current.running"
              @click="saveResponse"
            >
              <PostToolIcon name="download" /><span class="pt-action-label"
                >保存完整响应</span
              >
            </button>
            <button v-if="current.preview" @click="copyResponse">
              <PostToolIcon name="copy" /><span class="pt-action-label"
                >复制预览</span
              >
            </button>
          </header>
          <div v-if="current.error" class="pt-response-error" role="alert">
            {{ current.error }}
          </div>
          <nav v-if="current.response" class="pt-sections">
            <button
              v-for="s in [
                { id: 'body', name: 'Body' },
                { id: 'headers', name: 'Headers' },
                { id: 'tests', name: 'Test Results' },
                { id: 'console', name: 'Console' },
              ]"
              :key="s.id"
              :class="{ selected: responseView === s.id }"
              @click="responseView = s.id"
            >
              {{ s.name }}
            </button>
            <div class="pt-spacer" />
            <label v-if="responseView === 'body'" class="pt-check"
              ><input v-model="pretty" type="checkbox" />Pretty JSON</label
            >
          </nav>
          <div class="pt-response-content">
            <div
              v-if="!current.response && !current.error"
              class="pt-response-empty"
            >
              <span class="pt-send-symbol"><PostToolIcon name="send" /></span>
              <strong>{{
                current.running ? "正在等待响应" : "发送请求，查看响应"
              }}</strong>
              <span>选择请求方法，输入 URL，即可开始调试</span>
              <kbd><span>Ctrl</span> + <span>Enter</span></kbd>
            </div>
            <template v-else-if="responseView === 'body'"
              ><div
                v-if="
                  current.response && current.response.bytes > PREVIEW_LIMIT
                "
                class="pt-preview-note"
              >
                预览最多
                {{
                  formatBytes(PREVIEW_LIMIT)
                }}；完整响应在本机，可保存到文件。{{
                  current.running ? "正在跟随最新数据。" : ""
                }}
              </div>
              <pre class="pt-response-body">{{
                bodyText || (current.response?.bytes === 0 ? "（空响应）" : "")
              }}</pre>
            </template>
            <dl
              v-else-if="responseView === 'headers'"
              class="pt-response-headers"
            >
              <template
                v-for="(header, index) in current.response?.headers"
                :key="index"
                ><dt>{{ header.key }}</dt>
                <dd>{{ header.value }}</dd></template
              >
            </dl>
            <div v-else-if="responseView === 'tests'" class="pt-tests">
              <p v-if="!current.tests.length" class="pt-empty">
                此请求暂无断言结果
              </p>
              <p
                v-for="(test, index) in current.tests"
                :key="index"
                :class="test.passed ? 'pt-success' : 'pt-error-text'"
              >
                {{ test.passed ? "✓" : "×" }} {{ test.name }} {{ test.error }}
              </p>
            </div>
            <pre v-else class="pt-response-body">{{
              current.logs.join("\n") || "暂无脚本日志"
            }}</pre>
          </div>
        </section>
      </template>
      <footer v-if="active" class="pt-status">
        <span>{{ booted ? "本地工作区" : "postTool" }}</span
        ><span>{{ tabs.length }}/12 个标签</span
        ><span>响应上限 128 MiB · 预览 {{ formatBytes(PREVIEW_LIMIT) }}</span>
        <div class="pt-spacer" />
        <span>{{ busyCount ? "网络任务运行中" : "就绪" }}</span>
      </footer>
    </main>
    <PostToolCollection
      v-if="collectionEditing && active"
      :entry="collectionEditing"
      @close="collectionEditing = null"
      @saved="refreshStructure"
    />
    <Teleport to="body">
      <div
        v-if="modal !== 'none' && active"
        class="pt-mask"
        @mousedown.self="closeModal"
      >
        <section
          v-modal="closeModal"
          class="pt-dialog"
          :class="{
            'pt-dialog-wide': [
              'environment',
              'import',
              'export',
              'runner',
            ].includes(modal),
          }"
          role="dialog"
          aria-modal="true"
          :aria-label="
            modal === 'environment'
              ? '环境变量'
              : modal === 'import'
                ? '导入 API 数据'
                : modalTitle
          "
        >
          <header>
            <strong>{{
              modal === "environment"
                ? "环境变量"
                : modal === "import"
                  ? "导入 API 数据"
                  : modalTitle
            }}</strong
            ><button aria-label="关闭弹窗" @click="closeModal">×</button>
          </header>
          <div class="pt-dialog-body">
            <input
              v-if="modal === 'name'"
              v-model="nameValue"
              aria-label="名称"
              @keydown.enter="commitModal"
            />
            <p v-else-if="modal === 'confirm'">{{ modalTitle }}</p>
            <template v-else-if="modal === 'environment'">
              <label class="pt-dialog-label"
                >{{ editEnvKind === "globals" ? "名称" : "环境名称"
                }}<input v-model="editEnv.name"
              /></label>
              <PostToolPairs v-model="editEnv.values" secrets />
              <p class="pt-hint">数据在本机加密保存；勾选敏感值后隐藏显示。</p>
            </template>
            <template v-else-if="modal === 'import'">
              <p class="pt-hint">
                粘贴 cURL、Postman Collection / Environment 或 OpenAPI
                JSON。导入内容不会作为命令执行。
              </p>
              <button @click="chooseImport">选择本地文件</button>
              <textarea
                v-model="importText"
                aria-label="导入内容"
                spellcheck="false"
                placeholder="curl https://… 或粘贴 JSON"
              />
            </template>
            <template v-else-if="modal === 'export'">
              <p class="pt-hint">
                导出包含请求配置，请检查明文 Token 和文件路径。
              </p>
              <textarea
                v-model="exportText"
                aria-label="导出内容"
                readonly
                spellcheck="false"
              />
              <button @click="copyExport">复制 cURL</button
              ><button @click="exportJson">导出 Postman JSON</button>
            </template>
            <template v-else-if="modal === 'runner'">
              <p class="pt-hint">
                {{
                  folder
                    ? "运行当前集合/文件夹及其子目录"
                    : "运行全部已保存请求"
                }}，默认按保存顺序串行。页面保留最近 200 条摘要。
              </p>
              <div class="pt-runner-options">
                <label
                  >迭代次数<input
                    v-model.number="runner.iterations"
                    type="number"
                    min="1"
                    max="1000"
                    :disabled="runner.running" /></label
                ><label
                  >间隔（ms）<input
                    v-model.number="runner.delay"
                    type="number"
                    min="0"
                    max="10000"
                    :disabled="runner.running" /></label
                ><label
                  ><input
                    v-model="runner.stopOnError"
                    type="checkbox"
                  />失败后停止</label
                >
              </div>
              <textarea
                v-if="!runner.running"
                v-model="runner.dataText"
                aria-label="Runner 数据"
                placeholder="可选：粘贴 CSV 或 JSON 数组作为运行数据"
                spellcheck="false"
              />
              <p aria-live="polite">
                已执行 {{ runner.completed }}/{{ runner.total }} · 通过
                {{ runner.passed }} · 失败 {{ runner.failed }}
                {{ runner.paused ? "（已暂停）" : "" }}
              </p>
              <progress
                v-if="runner.running"
                :value="runner.completed"
                :max="runner.total"
                style="width: 100%"
              />
              <p v-if="runner.error" class="pt-error" role="alert">
                {{ runner.error }}
              </p>
              <div class="pt-runner-results">
                <div
                  v-for="(result, index) in runner.results"
                  :key="index"
                  :class="result.passed ? 'pt-success' : 'pt-error-text'"
                >
                  <span>{{ result.passed ? "✓" : "×" }} {{ result.name }}</span
                  ><span
                    >{{ result.status || "—" }} · {{ result.elapsed }} ms</span
                  ><span>{{ result.error }}</span>
                </div>
              </div>
            </template>
            <p v-if="modalError" class="pt-error" role="alert">
              {{ modalError }}
            </p>
          </div>
          <footer>
            <button :disabled="transferBusy" @click="closeModal">关闭</button>
            <button
              v-if="modal === 'name' || modal === 'confirm'"
              class="pt-primary"
              :disabled="transferBusy"
              @click="commitModal"
            >
              {{
                transferBusy
                  ? "处理中…"
                  : modal === "confirm"
                    ? "确认操作"
                    : "保存"
              }}
            </button>
            <button
              v-if="modal === 'environment'"
              class="pt-primary"
              :disabled="transferBusy || busyCount > 0"
              @click="saveEnvironment"
            >
              {{ transferBusy ? "保存中…" : "保存变量" }}
            </button>
            <button
              v-if="modal === 'import'"
              class="pt-primary"
              :disabled="transferBusy || !importText.trim()"
              @click="runImport"
            >
              {{ transferBusy ? "导入中…" : "导入到本机" }}
            </button>
            <template v-if="modal === 'runner'"
              ><button v-if="runner.results.length" @click="exportRunner">
                导出摘要</button
              ><button
                v-if="runner.running"
                @click="runner.paused = !runner.paused"
              >
                {{ runner.paused ? "继续" : "暂停" }}</button
              ><button v-if="runner.running" @click="stopAll">停止运行</button
              ><button
                v-else
                class="pt-primary"
                :disabled="busyCount > 0"
                @click="startRunner"
              >
                开始运行
              </button></template
            >
          </footer>
        </section>
      </div>
    </Teleport>
  </section>
</template>
