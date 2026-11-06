import { invoke } from "@tauri-apps/api/core";
import type {
  Entry,
  Page,
  RequestDoc,
  ResponseMeta,
  ResponseSlice,
  ResourceInfo,
} from "./model";
export const desktop =
  typeof window !== "undefined" &&
  !!(window as unknown as { __TAURI_INTERNALS__?: unknown })
    .__TAURI_INTERNALS__;
// No browser fetch fallback: CORS, credentials and cancellation semantics differ from the desktop client.
function call<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  if (!desktop)
    return Promise.reject(
      new Error("请在 Agent Dock 桌面应用中使用网络和本地文件功能"),
    );
  return invoke<T>(`posttool_${name}`, args);
}
export const send = (spec: unknown) => call<ResponseMeta>("request", { spec });
export const cancel = (id: string) => call<void>("cancel", { id });
export const release = (id?: string, clearClients = false) =>
  call<void>("release", { id: id ?? null, clearClients });
export const slice = (id: string, offset = 0) =>
  call<ResponseSlice>("response_slice", { id, offset });
export const resources = () => call<ResourceInfo>("resources");
export const responseSave = (id: string, path: string) =>
  call<number>("response_save", { id, path });
export const list = (kind: string, search = "", page = 0, parent?: string) =>
  call<Page>("document_list", { kind, search, page, parent: parent ?? null });
export const get = <T>(id: string) => call<T>("document_get", { id });
export const put = (document: Entry) =>
  call<void>("document_put", { document });
export const importDocuments = (documents: Entry[]) =>
  call<void>("document_import", { documents });
export const remove = (id: string) => call<void>("document_delete", { id });
export const clearHistory = () => call<void>("history_clear");
export const readFile = (path: string) => call<string>("file_read", { path });
export const parseDefinition = (text: string) =>
  call<string>("parse_definition", { text });
export const writeFile = (path: string, text: string) =>
  call<void>("file_write", { path, text });
export interface StreamRow {
  seq: number;
  time: number;
  direction: string;
  topic: string;
  payload: string;
  binary: boolean;
  truncated: boolean;
}
export interface StreamBatch {
  status: string;
  error: string | null;
  items: StreamRow[];
  cursor: number;
  dropped: number;
  bufferBytes: number;
}
export const streamConnect = (spec: unknown) =>
  call<void>("stream_connect", { spec });
export const streamSend = (id: string, command: unknown) =>
  call<void>("stream_send", { id, command });
export const streamClose = (id: string) => call<void>("stream_close", { id });
export const streamRelease = (id: string) =>
  call<void>("stream_release", { id });
export const streamPoll = (id: string, after: number) =>
  call<StreamBatch>("stream_poll", { id, after });
export interface GrpcMethod {
  name: string;
  service: string;
  clientStreaming: boolean;
  serverStreaming: boolean;
  input: string;
  output: string;
}
export interface ProtoInfo {
  id: string;
  methods: GrpcMethod[];
}
export const protoLoad = (path: string, includes: string[]) =>
  call<ProtoInfo>("proto_load", { path, includes });
export const grpcReflect = (url: string, headers: unknown, symbol?: string) =>
  call<ProtoInfo & { services?: string[] }>("grpc_reflect", {
    url,
    headers,
    symbol: symbol ?? null,
  });
export interface OAuthCallback {
  id: string;
  state: string;
  redirectUri: string;
  code: string | null;
  error: string | null;
  done: boolean;
}
export const oauthStart = (redirectUri: string) =>
  call<OAuthCallback>("oauth_start", { redirectUri });
export const oauthPoll = (id: string) =>
  call<OAuthCallback>("oauth_poll", { id });
export const oauthClose = (id: string) => call<void>("oauth_close", { id });
export function requestEntry(doc: RequestDoc, kind = "request"): Entry {
  return {
    id: doc.id,
    kind,
    parent: doc.parent,
    name: doc.name,
    method: doc.method,
    url: "",
    updated: Date.now(),
    payload: doc,
  };
}
