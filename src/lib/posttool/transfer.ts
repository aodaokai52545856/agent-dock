import {
  freshRequest,
  pair,
  uid,
  type Entry,
  type Pair,
  type RequestDoc,
} from "./model.ts";
type Json = Record<string, any>;
export interface ImportResult {
  requests: RequestDoc[];
  documents: Entry[];
  warnings: string[];
}
const pairs = (values: Json[] = []): Pair[] =>
  values.map((p) => ({
    ...pair(String(p.key ?? ""), String(p.value ?? "")),
    enabled: p.disabled !== true,
    secret: p.type === "secret",
  }));
const record = (
  kind: string,
  name: string,
  parent: string,
  payload: unknown,
): Entry => ({
  id: uid(),
  kind,
  parent,
  name,
  method: "",
  url: "",
  updated: Date.now(),
  payload,
});
function auth(raw: Json | undefined, result: ImportResult): RequestDoc["auth"] {
  const base = freshRequest().auth;
  if (!raw) return { ...base, type: "inherit" };
  const values = Object.fromEntries(
    (raw[raw.type] ?? []).map((p: Json) => [p.key, String(p.value ?? "")]),
  );
  if (raw.type === "noauth") return base;
  if (raw.type === "bearer")
    return { ...base, type: "bearer", token: values.token ?? "" };
  if (raw.type === "basic")
    return {
      ...base,
      type: "basic",
      username: values.username ?? "",
      password: values.password ?? "",
    };
  if (raw.type === "apikey")
    return {
      ...base,
      type: "apikey",
      key: values.key ?? "",
      value: values.value ?? "",
      location: values.in === "query" ? "query" : "header",
    };
  if (raw.type === "oauth2")
    return {
      ...base,
      type: "oauth2",
      token: values.accessToken ?? "",
      oauth: {
        grant:
          values.grant_type === "client_credentials"
            ? "client_credentials"
            : "code",
        authUrl: values.authUrl ?? "",
        tokenUrl: values.accessTokenUrl ?? "",
        clientId: values.clientId ?? "",
        clientSecret: values.clientSecret ?? "",
        scope: values.scope ?? "",
        redirectUri: values.redirect_uri ?? "",
        refreshToken: values.refreshToken ?? "",
        expiresAt: 0,
        clientAuth: "body",
      },
    };
  result.warnings.push(`鉴权 ${raw.type} 尚未转换，导入后请配置`);
  return base;
}
function scripts(events: Json[] = []) {
  const s = { preScript: "", postScript: "" };
  for (const e of events) {
    const code = Array.isArray(e.script?.exec)
      ? e.script.exec.join("\n")
      : String(e.script?.exec ?? "");
    if (e.listen === "prerequest") s.preScript = code;
    if (e.listen === "test") s.postScript = code;
  }
  return s;
}
export function importData(text: string): ImportResult {
  if (text.length > 16 * 1024 * 1024) throw new Error("导入文件超过 16 MiB");
  let data: Json;
  try {
    data = JSON.parse(text);
  } catch {
    throw new Error("请输入有效 JSON；YAML 定义请先转换为 JSON");
  }
  const result: ImportResult = { requests: [], documents: [], warnings: [] };
  if (data.openapi || data.swagger) return importOpenApi(data);
  if (Array.isArray(data.values)) {
    const kind =
      data._postman_variable_scope === "globals" ? "globals" : "environment";
    result.documents.push(
      record(kind, String(data.name ?? "导入环境"), "", {
        name: String(data.name ?? "导入环境"),
        values: pairs(data.values),
      }),
    );
    return result;
  }
  if (!Array.isArray(data.item))
    throw new Error(
      "无法识别：需要 Postman Collection / Environment 或 OpenAPI 文件",
    );
  const collection = record(
    "collection",
    String(data.info?.name ?? "导入集合"),
    "",
    {
      name: data.info?.name ?? "导入集合",
      variables: pairs(data.variable),
      auth: auth(data.auth, result),
      ...scripts(data.event),
    },
  );
  // A collection without a parent cannot inherit authorization.
  if ((collection.payload as Json).auth.type === "inherit")
    (collection.payload as Json).auth.type = "none";
  result.documents.push(collection);
  const visit = (items: Json[], parent: string, depth: number) => {
    if (depth > 16) throw new Error("集合嵌套超过 16 层");
    for (const item of items) {
      if (result.requests.length + result.documents.length >= 10000)
        throw new Error("单次导入最多 10,000 项");
      if (Array.isArray(item.item)) {
        const folder = record("folder", String(item.name ?? "文件夹"), parent, {
          name: item.name,
          auth: auth(item.auth, result),
          ...scripts(item.event),
        });
        result.documents.push(folder);
        visit(item.item, folder.id, depth + 1);
        continue;
      }
      if (!item.request) {
        result.warnings.push(`跳过非请求项 ${item.name ?? ""}`);
        continue;
      }
      const raw: Json =
          typeof item.request === "string"
            ? { url: item.request }
            : item.request,
        doc = freshRequest();
      doc.parent = parent;
      doc.name = String(item.name ?? "导入请求");
      doc.method = String(raw.method ?? "GET");
      doc.url =
        typeof raw.url === "string" ? raw.url : String(raw.url?.raw ?? "");
      doc.headers = pairs(raw.header);
      doc.auth = auth(raw.auth, result);
      doc.description =
        typeof raw.description === "string"
          ? raw.description
          : String(raw.description?.content ?? "");
      Object.assign(doc, scripts(item.event));
      if (raw.body?.mode === "raw") {
        doc.body = String(raw.body.raw ?? "");
        const language = raw.body.options?.raw?.language;
        doc.bodyMode = ["json", "xml", "html"].includes(language)
          ? language
          : "text";
      } else if (raw.body?.mode === "urlencoded") {
        doc.bodyMode = "urlencoded";
        doc.fields = pairs(raw.body.urlencoded);
      } else if (raw.body?.mode === "formdata") {
        doc.bodyMode = "multipart";
        doc.fields = (raw.body.formdata ?? []).flatMap((p: Json) =>
          p.type === "file"
            ? (Array.isArray(p.src) ? p.src : [p.src ?? ""]).map(
                (path: string) => ({
                  ...pair(p.key, String(path)),
                  file: true,
                  enabled: !p.disabled,
                }),
              )
            : [{ ...pair(p.key, String(p.value ?? "")), enabled: !p.disabled }],
        );
        if (doc.fields.some((p) => p.file))
          result.warnings.push(`“${doc.name}”含本地文件，请检查路径`);
      } else if (raw.body?.mode === "file") {
        doc.bodyMode = "binary";
        doc.filePath = String(raw.body.file?.src ?? "");
      } else if (raw.body?.mode === "graphql") {
        doc.bodyMode = "graphql";
        doc.body = String(raw.body.graphql?.query ?? "");
        doc.graphqlVariables = String(raw.body.graphql?.variables ?? "{}");
      } else if (raw.body?.mode)
        result.warnings.push(`“${doc.name}”的正文模式 ${raw.body.mode} 未转换`);
      if (raw.url?.variable?.length) {
        for (const p of raw.url.variable)
          doc.url = doc.url.replace(`:${p.key}`, `{{${p.key}}}`);
        const vars = raw.url.variable.map((p: Json) =>
          pair(p.key, String(p.value ?? "")),
        );
        (collection.payload as Json).variables.push(...vars);
      }
      if (item.response?.length)
        result.warnings.push(
          `“${doc.name}”的 ${item.response.length} 个示例响应尚未导入`,
        );
      result.requests.push(doc);
    }
  };
  visit(data.item, collection.id, 0);
  result.warnings = [...new Set(result.warnings)].slice(0, 50);
  return result;
}
function importOpenApi(data: Json): ImportResult {
  const result: ImportResult = { requests: [], documents: [], warnings: [] },
    collection = record(
      "collection",
      String(data.info?.title ?? "OpenAPI"),
      "",
      { variables: [], auth: freshRequest().auth },
    );
  result.documents.push(collection);
  const resolve = (value: Json, seen: string[] = []): Json => {
    if (!value?.$ref) return value ?? {};
    const ref = value.$ref as string;
    if (!ref.startsWith("#/")) throw new Error(`仅支持本地 $ref：${ref}`);
    if (seen.includes(ref) || seen.length > 16)
      throw new Error(`循环/过深 $ref：${ref}`);
    let target: Json = data;
    for (const p of ref.slice(2).split("/")) {
      const key = p.replace(/~1/g, "/").replace(/~0/g, "~");
      if (!Object.hasOwn(target, key)) throw new Error(`找不到 $ref：${ref}`);
      target = target[key];
    }
    return resolve(target, [...seen, ref]);
  };
  const server = String(
    data.servers?.[0]?.url ??
      (data.host
        ? `${data.schemes?.[0] ?? "https"}://${data.host}${data.basePath ?? ""}`
        : "{{base_url}}"),
  );
  for (const [path, itemValue] of Object.entries(data.paths ?? {})) {
    const item = resolve(itemValue as Json);
    for (const method of [
      "get",
      "post",
      "put",
      "patch",
      "delete",
      "head",
      "options",
      "trace",
    ]) {
      if (!item[method]) continue;
      if (result.requests.length >= 10000)
        throw new Error("单次导入最多 10,000 个请求");
      const op = resolve(item[method]),
        doc = freshRequest();
      doc.parent = collection.id;
      doc.name = String(
        op.summary ?? op.operationId ?? `${method.toUpperCase()} ${path}`,
      );
      doc.method = method.toUpperCase();
      doc.url =
        server.replace(/\/$/, "") + path.replace(/\{([^{}]+)\}/g, "{{$1}}");
      doc.description = String(op.description ?? "");
      for (const raw of [
        ...(item.parameters ?? []),
        ...(op.parameters ?? []),
      ]) {
        const p = resolve(raw),
          schema = resolve(p.schema),
          value = String(p.example ?? schema.example ?? schema.default ?? "");
        if (p.in === "query") doc.params.push(pair(p.name, value));
        if (p.in === "header") doc.headers.push(pair(p.name, value));
        if (p.in === "path")
          (collection.payload as Json).variables.push(pair(p.name, value));
        if (p.in === "body") {
          doc.bodyMode = "json";
          doc.body = JSON.stringify(schema.example ?? {}, null, 2);
        }
      }
      const body = resolve(op.requestBody),
        content = body.content ?? {},
        type = Object.keys(content)[0];
      if (type) {
        const media = content[type],
          schema = resolve(media.schema);
        doc.headers.push(pair("Content-Type", type));
        doc.bodyMode = type.includes("json") ? "json" : "text";
        doc.body =
          typeof media.example === "string"
            ? media.example
            : JSON.stringify(media.example ?? schema.example ?? {}, null, 2);
      }
      if (op.security?.length || data.security?.length)
        result.warnings.push("OpenAPI security 需导入后检查并配置鉴权");
      result.requests.push(doc);
    }
  }
  if (!result.requests.length)
    throw new Error("OpenAPI 文件没有可导入的 HTTP 操作");
  result.warnings = [...new Set(result.warnings)];
  return result;
}
export function exportCollection(requests: RequestDoc[]) {
  return {
    info: {
      name: "postTool 导出",
      schema:
        "https://schema.getpostman.com/json/collection/v2.1.0/collection.json",
    },
    item: requests.map((doc) => ({
      name: doc.name,
      request: {
        method: doc.method,
        url:
          doc.url +
          (doc.params.some((p) => p.enabled && p.key)
            ? (doc.url.includes("?") ? "&" : "?") +
              new URLSearchParams(
                doc.params
                  .filter((p) => p.enabled && p.key)
                  .map((p) => [p.key, p.value]),
              ).toString()
            : ""),
        description: doc.description,
        header: doc.headers.map((p) => ({
          key: p.key,
          value: p.value,
          disabled: !p.enabled,
        })),
        auth: exportAuth(doc.auth),
        body:
          doc.bodyMode === "none"
            ? undefined
            : doc.bodyMode === "multipart"
              ? {
                  mode: "formdata",
                  formdata: doc.fields.map((p) => ({
                    key: p.key,
                    type: p.file ? "file" : "text",
                    ...(p.file ? { src: p.value } : { value: p.value }),
                    disabled: !p.enabled,
                  })),
                }
              : doc.bodyMode === "urlencoded"
                ? {
                    mode: "urlencoded",
                    urlencoded: doc.fields.map((p) => ({
                      key: p.key,
                      value: p.value,
                      disabled: !p.enabled,
                    })),
                  }
                : doc.bodyMode === "binary"
                  ? { mode: "file", file: { src: doc.filePath } }
                  : doc.bodyMode === "graphql"
                    ? {
                        mode: "graphql",
                        graphql: {
                          query: doc.body,
                          variables: doc.graphqlVariables,
                        },
                      }
                    : {
                        mode: "raw",
                        raw: doc.body,
                        options: { raw: { language: doc.bodyMode } },
                      },
      },
      event: [
        {
          listen: "prerequest",
          script: { type: "text/javascript", exec: doc.preScript.split("\n") },
        },
        {
          listen: "test",
          script: { type: "text/javascript", exec: doc.postScript.split("\n") },
        },
      ],
    })),
  };
}
function exportAuth(auth: RequestDoc["auth"]) {
  if (auth.type === "inherit") return undefined;
  if (auth.type === "none") return { type: "noauth" };
  const type = auth.type;
  const data =
    type === "bearer"
      ? { token: auth.token }
      : type === "basic"
        ? { username: auth.username, password: auth.password }
        : type === "oauth2"
          ? {
              accessToken: auth.token,
              grant_type: auth.oauth?.grant ?? "code",
              authUrl: auth.oauth?.authUrl ?? "",
              accessTokenUrl: auth.oauth?.tokenUrl ?? "",
              clientId: auth.oauth?.clientId ?? "",
              clientSecret: auth.oauth?.clientSecret ?? "",
              scope: auth.oauth?.scope ?? "",
              redirect_uri: auth.oauth?.redirectUri ?? "",
            }
          : { key: auth.key, value: auth.value, in: auth.location };
  return {
    type,
    [type]: Object.entries(data).map(([key, value]) => ({
      key,
      value,
      type: "string",
    })),
  };
}
