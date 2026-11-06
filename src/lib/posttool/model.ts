export interface Pair {
  key: string;
  value: string;
  enabled: boolean;
  file?: boolean;
  secret?: boolean;
}
export type BodyMode =
  | "none"
  | "json"
  | "text"
  | "xml"
  | "html"
  | "urlencoded"
  | "multipart"
  | "binary"
  | "graphql";
export interface Auth {
  type: "none" | "inherit" | "bearer" | "basic" | "apikey" | "oauth2";
  token: string;
  username: string;
  password: string;
  key: string;
  value: string;
  location: "header" | "query";
  oauth?: {
    grant: "code" | "client_credentials" | "password";
    authUrl: string;
    tokenUrl: string;
    clientId: string;
    clientSecret: string;
    scope: string;
    redirectUri: string;
    refreshToken: string;
    expiresAt: number;
    clientAuth: "body" | "basic";
  };
}
export interface RequestDoc {
  id: string;
  name: string;
  parent: string;
  method: string;
  url: string;
  headers: Pair[];
  params: Pair[];
  bodyMode: BodyMode;
  body: string;
  fields: Pair[];
  filePath: string;
  auth: Auth;
  description: string;
  proxy: string;
  insecure: boolean;
  followRedirects: boolean;
  cookies: boolean;
  timeoutMs: number;
  caPath: string;
  identityPath: string;
  preScript: string;
  postScript: string;
  graphqlVariables: string;
  operationName: string;
}
export interface Environment {
  name: string;
  values: Pair[];
}
export interface Entry {
  id: string;
  kind: string;
  parent: string;
  name: string;
  method: string;
  url: string;
  updated: number;
  payload?: unknown;
}
export interface Page {
  items: Entry[];
  total: number;
}
export interface ResponseMeta {
  id: string;
  state: string;
  status: number;
  statusText: string;
  headers: Pair[];
  url: string;
  bytes: number;
  elapsedMs: number;
  firstByteMs: number;
  error: string | null;
  contentType: string;
}
export interface ResponseSlice {
  meta: ResponseMeta;
  offset: number;
  nextOffset: number;
  data: number[];
}
export interface ResourceInfo {
  active: number;
  responses: number;
  clients: number;
  cacheBytes: number;
}
export interface Tab {
  key: string;
  request: RequestDoc;
  saved: string;
  responseId?: string;
  response?: ResponseMeta;
  preview: string;
  offset: number;
  running: boolean;
  error: string;
  tests: TestResult[];
  logs: string[];
}
export interface TestResult {
  name: string;
  passed: boolean;
  error?: string;
}
export const METHODS = [
  "GET",
  "POST",
  "PUT",
  "PATCH",
  "DELETE",
  "HEAD",
  "OPTIONS",
];
export const PREVIEW_LIMIT = 256 * 1024;
export function uid(): string {
  return crypto.randomUUID();
}
export function pair(key = "", value = ""): Pair {
  return { key, value, enabled: true };
}
export function freshRequest(): RequestDoc {
  return {
    id: uid(),
    name: "未命名请求",
    parent: "",
    method: "GET",
    url: "",
    headers: [],
    params: [],
    bodyMode: "none",
    body: "",
    fields: [],
    filePath: "",
    auth: {
      type: "none",
      token: "",
      username: "",
      password: "",
      key: "",
      value: "",
      location: "header",
    },
    description: "",
    proxy: "direct",
    insecure: false,
    followRedirects: true,
    cookies: true,
    timeoutMs: 30000,
    caPath: "",
    identityPath: "",
    preScript: "",
    postScript: "",
    graphqlVariables: "{}",
    operationName: "",
  };
}
export function normalizeRequest(value: unknown): RequestDoc {
  if (!value || typeof value !== "object")
    throw new Error("请求数据不是有效对象");
  const base = freshRequest(),
    raw = value as Partial<RequestDoc>;
  const doc = { ...base, ...raw, auth: { ...base.auth, ...raw.auth } };
  for (const field of ["headers", "params", "fields"] as const) {
    if (!Array.isArray(doc[field]) || doc[field].length > 512)
      throw new Error(`${field} 超过 512 项或格式无效`);
    doc[field] = doc[field].map((p) => ({
      key: String(p.key ?? ""),
      value: String(p.value ?? ""),
      enabled: p.enabled !== false,
      file: p.file === true,
    }));
  }
  if (typeof doc.url !== "string" || typeof doc.body !== "string")
    throw new Error("请求 URL/正文格式无效");
  if (doc.body.length > 2 * 1024 * 1024)
    throw new Error("正文超过 2 MiB，请改用文件上传");
  return doc;
}
export function newTab(doc = freshRequest(), saved = ""): Tab {
  return {
    key: uid(),
    request: doc,
    saved,
    preview: "",
    offset: 0,
    running: false,
    error: "",
    tests: [],
    logs: [],
  };
}
export function dirty(tab: Tab): boolean {
  return JSON.stringify(tab.request) !== tab.saved;
}
export function variables(...scopes: Pair[][]): Record<string, string> {
  const result: Record<string, string> = Object.create(null);
  for (const scope of scopes)
    for (const p of scope) if (p.enabled && p.key) result[p.key] = p.value;
  return result;
}
export function resolve(text: string, vars: Record<string, string>): string {
  const dynamic: Record<string, string> = Object.create(null);
  let expansions = 0;
  const visit = (value: string, stack: string[]): string => {
    if (value.length > 2 * 1024 * 1024) throw new Error("变量展开超过 2 MiB");
    let length = value.length;
    return value.replace(/\{\{\s*([^{}]+?)\s*\}\}/g, (match, key: string) => {
      if (++expansions > 4096) throw new Error("变量展开次数超过 4096");
      if (stack.includes(key) || stack.length >= 16)
        throw new Error(
          `变量循环引用或层级过深：${[...stack, key].join(" → ")}`,
        );
      if (key.startsWith("$")) {
        if (!(key in dynamic)) {
          if (key === "$guid" || key === "$randomUUID") dynamic[key] = uid();
          else if (key === "$timestamp")
            dynamic[key] = String(Math.floor(Date.now() / 1000));
          else if (key === "$isoTimestamp")
            dynamic[key] = new Date().toISOString();
          else if (key === "$randomInt")
            dynamic[key] = String(Math.floor(Math.random() * 1001));
          else throw new Error(`不支持的动态变量：${key}`);
        }
        const result = dynamic[key]!;
        length += result.length - match.length;
        if (length > 2 * 1024 * 1024) throw new Error("变量展开超过 2 MiB");
        return result;
      }
      if (!Object.hasOwn(vars, key)) throw new Error(`未定义变量：${key}`);
      const result = visit(vars[key]!, [...stack, key]);
      length += result.length - match.length;
      if (length > 2 * 1024 * 1024) throw new Error("变量展开超过 2 MiB");
      return result;
    });
  };
  const result = visit(text, []);
  if (result.length > 2 * 1024 * 1024) throw new Error("变量展开超过 2 MiB");
  return result;
}
export function compile(
  doc: RequestDoc,
  vars: Record<string, string>,
  inherited?: Auth,
) {
  const r = (s: string) => resolve(s, vars);
  const url = new URL(r(doc.url));
  if (!["http:", "https:"].includes(url.protocol))
    throw new Error("请输入 http:// 或 https:// URL");
  for (const p of doc.params.filter((p) => p.enabled && p.key))
    url.searchParams.append(r(p.key), r(p.value));
  const headers = doc.headers
    .filter((p) => p.enabled && p.key)
    .map((p) => ({ ...p, key: r(p.key), value: r(p.value) }));
  const auth = doc.auth.type === "inherit" ? inherited : doc.auth;
  if (doc.auth.type === "inherit" && !auth)
    throw new Error("当前请求没有可继承的父级鉴权");
  const add = (key: string, value: string) => {
    if (headers.some((p) => p.key.toLowerCase() === key.toLowerCase()))
      throw new Error(`${key} 同时存在手填值和自动生成值，请移除其中一处`);
    headers.push(pair(key, value));
  };
  if (auth?.type === "bearer" || auth?.type === "oauth2")
    add("Authorization", `Bearer ${r(auth.token)}`);
  if (auth?.type === "basic")
    add(
      "Authorization",
      `Basic ${btoa(String.fromCharCode(...new TextEncoder().encode(`${r(auth.username)}:${r(auth.password)}`)))}`,
    );
  if (auth?.type === "apikey") {
    if (auth.location === "query")
      url.searchParams.append(r(auth.key), r(auth.value));
    else add(r(auth.key), r(auth.value));
  }
  const contentTypes: Partial<Record<BodyMode, string>> = {
    json: "application/json",
    xml: "application/xml",
    text: "text/plain",
    html: "text/html",
    graphql: "application/json",
  };
  if (
    contentTypes[doc.bodyMode] &&
    !headers.some((p) => p.key.toLowerCase() === "content-type")
  )
    headers.push(pair("Content-Type", contentTypes[doc.bodyMode]!));
  if (
    doc.bodyMode === "multipart" &&
    headers.some((p) => p.key.toLowerCase() === "content-type")
  )
    throw new Error(
      "multipart 的 Content-Type 由客户端生成，请移除手填值以生成正确 boundary",
    );
  let body = r(doc.body);
  if (doc.bodyMode === "graphql")
    body = JSON.stringify({
      query: body,
      variables: JSON.parse(r(doc.graphqlVariables || "{}")),
      ...(doc.operationName ? { operationName: r(doc.operationName) } : {}),
    });
  return {
    id: uid(),
    method: doc.method.toUpperCase(),
    url: url.toString(),
    headers,
    body,
    bodyMode: doc.bodyMode,
    fields: doc.fields.map((p) => ({ ...p, key: r(p.key), value: r(p.value) })),
    filePath: doc.filePath,
    proxy: r(doc.proxy),
    insecure: doc.insecure,
    followRedirects: doc.followRedirects,
    cookies: doc.cookies,
    timeoutMs: doc.timeoutMs,
    caPath: doc.caPath,
    identityPath: doc.identityPath,
  };
}
export function publicUrl(raw: string): string {
  try {
    const url = new URL(raw);
    url.username = "";
    url.password = "";
    for (const key of [...url.searchParams.keys()])
      if (/token|secret|key|password|auth/i.test(key))
        url.searchParams.set(key, "[redacted]");
    return url.toString();
  } catch {
    return raw.replace(
      /([?&](?:[^=&]*(?:token|secret|key|password|auth)[^=&]*)=)[^&]*/gi,
      "$1[redacted]",
    );
  }
}
export function historyRequest(doc: RequestDoc): RequestDoc {
  const copy = JSON.parse(JSON.stringify(doc)) as RequestDoc;
  copy.url = publicUrl(copy.url);
  copy.body = "";
  copy.preScript = "";
  copy.postScript = "";
  copy.filePath = "";
  copy.identityPath = "";
  copy.auth = { ...freshRequest().auth, type: "none" };
  copy.headers = copy.headers.map((p) =>
    /authorization|cookie|token|secret|key/i.test(p.key)
      ? { ...p, value: "[redacted]" }
      : p,
  );
  copy.params = copy.params.map((p) =>
    /token|secret|key|password|auth/i.test(p.key)
      ? { ...p, value: "[redacted]" }
      : p,
  );
  copy.fields = [];
  return copy;
}
export function formatBytes(n: number): string {
  return n < 1024
    ? `${n} B`
    : n < 1024 * 1024
      ? `${(n / 1024).toFixed(1)} KiB`
      : `${(n / 1024 / 1024).toFixed(1)} MiB`;
}
export function syncUrlParams(doc: RequestDoc): void {
  const index = doc.url.indexOf("?");
  if (index < 0) return;
  const fragment = doc.url.indexOf("#", index),
    query = doc.url.slice(index + 1, fragment < 0 ? undefined : fragment);
  doc.params = [...new URLSearchParams(query)].map(([k, v]) => pair(k, v));
  doc.url =
    doc.url.slice(0, index) + (fragment < 0 ? "" : doc.url.slice(fragment));
}

// Tokenizer only: imported text is never passed to a shell.
export function shellWords(text: string): string[] {
  const words: string[] = [];
  let word = "",
    quote = "",
    started = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i]!;
    if (c === "\\" && quote !== "'") {
      const next = text[++i];
      if (next === "\n") continue;
      if (next === "\r" && text[i + 1] === "\n") {
        i++;
        continue;
      }
      if (next === undefined) throw new Error("cURL 末尾转义不完整");
      word += next;
      started = true;
      continue;
    }
    if (quote) {
      if (c === quote) quote = "";
      else word += c;
      started = true;
      continue;
    }
    if (c === '"' || c === "'") {
      quote = c;
      started = true;
      continue;
    }
    if (/\s/.test(c)) {
      if (started) {
        words.push(word);
        word = "";
        started = false;
      }
    } else {
      word += c;
      started = true;
    }
  }
  if (quote) throw new Error("cURL 引号未闭合");
  if (started) words.push(word);
  return words;
}
export function importCurl(text: string): RequestDoc {
  const args = shellWords(text.replace(/`\r?\n/g, "").replace(/\^\r?\n/g, "")),
    doc = freshRequest();
  let explicit = false;
  if (!/^curl(?:\.exe)?$/i.test(args.shift() || ""))
    throw new Error("请输入以 curl 开头的请求命令");
  doc.followRedirects = false;
  doc.proxy = "";
  const val = (i: number) => {
    if (args[i + 1] === undefined) throw new Error(`${args[i]} 缺少参数`);
    return args[i + 1]!;
  };
  for (let i = 0; i < args.length; i++) {
    const a = args[i]!;
    if (a === "-X" || a === "--request") {
      doc.method = val(i).toUpperCase();
      i++;
      explicit = true;
    } else if (a === "-H" || a === "--header") {
      const s = val(i),
        p = s.indexOf(":");
      if (p < 1) throw new Error("Header 格式为 Key: Value");
      doc.headers.push(pair(s.slice(0, p), s.slice(p + 1).trimStart()));
      i++;
    } else if (a === "--data-urlencode") {
      const value = val(i),
        index = value.indexOf("=");
      if (index < 1 || value.slice(index + 1).startsWith("@"))
        throw new Error("--data-urlencode 目前支持 name=value 格式");
      if (!["none", "urlencoded"].includes(doc.bodyMode))
        throw new Error("不能混合导入 raw 与 --data-urlencode 正文");
      doc.bodyMode = "urlencoded";
      doc.fields.push(pair(value.slice(0, index), value.slice(index + 1)));
      if (!explicit) doc.method = "POST";
      i++;
    } else if (["-d", "--data", "--data-raw", "--data-binary"].includes(a)) {
      const s = val(i);
      if (s.startsWith("@") && a !== "--data-raw") {
        if (a !== "--data-binary")
          throw new Error(
            "文件正文请使用 --data-binary @file；--data 的文件换行转换暂不支持",
          );
        doc.bodyMode = "binary";
        doc.filePath = s.slice(1);
      } else {
        doc.bodyMode = "text";
        doc.body += (doc.body ? "&" : "") + s;
      }
      if (!explicit) doc.method = "POST";
      i++;
    } else if (a === "-F" || a === "--form") {
      const s = val(i),
        p = s.indexOf("=");
      if (p < 1) throw new Error("表单格式为 key=value");
      doc.bodyMode = "multipart";
      doc.fields.push({
        ...pair(s.slice(0, p), s.slice(p + 1).replace(/^@/, "")),
        file: s[p + 1] === "@",
      });
      if (!explicit) doc.method = "POST";
      i++;
    } else if (a === "-u" || a === "--user") {
      const s = val(i),
        p = s.indexOf(":");
      doc.auth = {
        ...doc.auth,
        type: "basic",
        username: p < 0 ? s : s.slice(0, p),
        password: p < 0 ? "" : s.slice(p + 1),
      };
      i++;
    } else if (a === "--noproxy") {
      if (val(i) !== "*")
        throw new Error("--noproxy 目前支持 *；域名绕过规则请在网络设置中配置");
      doc.proxy = "direct";
      i++;
    } else if (a === "--cacert") {
      doc.caPath = val(i);
      i++;
    } else if (a === "--cert") {
      doc.identityPath = val(i);
      i++;
    } else if (a === "--url") {
      doc.url = val(i);
      i++;
    } else if (a === "-k" || a === "--insecure") doc.insecure = true;
    else if (a === "-L" || a === "--location") doc.followRedirects = true;
    else if (a === "-I" || a === "--head") {
      doc.method = "HEAD";
      explicit = true;
    } else if (
      a === "--compressed" ||
      a === "--silent" ||
      a === "-s" ||
      a === "--show-error" ||
      a === "-S"
    )
      continue;
    else if (a === "-x" || a === "--proxy") {
      doc.proxy = val(i);
      i++;
    } else if (a === "--max-time") {
      doc.timeoutMs = Number(val(i)) * 1000;
      i++;
    } else if (a.startsWith("-"))
      throw new Error(
        `暂不支持 cURL 选项 ${a}，请移除后导入，避免改变请求含义`,
      );
    else if (!doc.url) doc.url = a;
    else throw new Error(`不支持多个 URL 或额外参数：${a}`);
  }
  if (!doc.url) throw new Error("cURL 缺少 URL");
  doc.name = publicUrl(doc.url);
  if (
    ["text", "none"].includes(doc.bodyMode) &&
    doc.headers.some(
      (p) => p.key.toLowerCase() === "content-type" && p.value.includes("json"),
    )
  )
    doc.bodyMode = "json";
  return doc;
}
export function requestUrl(doc: RequestDoc): string {
  const index = doc.url.indexOf("#"),
    base = index < 0 ? doc.url : doc.url.slice(0, index),
    fragment = index < 0 ? "" : doc.url.slice(index);
  const params = doc.params
    .filter((p) => p.enabled && p.key)
    .map((p) => [p.key, p.value]);
  if (doc.auth.type === "apikey" && doc.auth.location === "query")
    params.push([doc.auth.key, doc.auth.value]);
  return (
    base +
    (params.length
      ? (base.includes("?") ? "&" : "?") +
        new URLSearchParams(params).toString()
      : "") +
    fragment
  );
}
export function exportCurl(doc: RequestDoc): string {
  const quote = (s: string) => "'" + s.replace(/'/g, "'\\''") + "'";
  const parts = [
    "curl",
    "--request",
    quote(doc.method),
    quote(requestUrl(doc)),
  ];
  for (const p of doc.headers.filter((p) => p.enabled && p.key))
    parts.push("--header", quote(`${p.key}: ${p.value}`));
  if (doc.auth.type === "bearer" || doc.auth.type === "oauth2")
    parts.push("--header", quote(`Authorization: Bearer ${doc.auth.token}`));
  if (doc.auth.type === "basic")
    parts.push("--user", quote(`${doc.auth.username}:${doc.auth.password}`));
  if (doc.auth.type === "apikey" && doc.auth.location === "header")
    parts.push("--header", quote(`${doc.auth.key}: ${doc.auth.value}`));
  const types: Partial<Record<BodyMode, string>> = {
    json: "application/json",
    xml: "application/xml",
    text: "text/plain",
    html: "text/html",
    graphql: "application/json",
  };
  if (
    types[doc.bodyMode] &&
    !doc.headers.some(
      (p) => p.enabled && p.key.toLowerCase() === "content-type",
    )
  )
    parts.push("--header", quote(`Content-Type: ${types[doc.bodyMode]}`));
  if (doc.bodyMode === "multipart") {
    for (const p of doc.fields.filter((p) => p.enabled && p.key))
      parts.push("--form", quote(`${p.key}=${p.file ? "@" : ""}${p.value}`));
  } else if (doc.bodyMode === "urlencoded")
    parts.push(
      "--data-raw",
      quote(
        new URLSearchParams(
          doc.fields
            .filter((p) => p.enabled && p.key)
            .map((p) => [p.key, p.value]),
        ).toString(),
      ),
    );
  else if (doc.bodyMode === "binary")
    parts.push("--data-binary", quote("@" + doc.filePath));
  else if (doc.bodyMode === "graphql")
    parts.push(
      "--data-raw",
      quote(
        JSON.stringify({
          query: doc.body,
          variables: JSON.parse(doc.graphqlVariables || "{}"),
          ...(doc.operationName ? { operationName: doc.operationName } : {}),
        }),
      ),
    );
  else if (doc.bodyMode !== "none") parts.push("--data-raw", quote(doc.body));
  if (doc.insecure) parts.push("--insecure");
  if (doc.followRedirects) parts.push("--location");
  if (doc.proxy === "direct") parts.push("--noproxy", quote("*"));
  else if (doc.proxy) parts.push("--proxy", quote(doc.proxy));
  if (doc.caPath) parts.push("--cacert", quote(doc.caPath));
  if (doc.identityPath) parts.push("--cert", quote(doc.identityPath));
  parts.push("--max-time", String(Math.max(0.1, doc.timeoutMs / 1000)));
  return parts.join(" ");
}
