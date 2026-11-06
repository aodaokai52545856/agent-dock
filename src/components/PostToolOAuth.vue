<script setup lang="ts">
import { computed, onUnmounted, ref } from "vue";
import { openExternal } from "../lib/api";
import * as api from "../lib/posttool/api";
import {
  compile,
  freshRequest,
  pair,
  uid,
  type Auth,
  type RequestDoc,
} from "../lib/posttool/model";
const auth = defineModel<Auth>({ required: true });
const props = defineProps<{ request: RequestDoc }>();
if (!auth.value.oauth)
  auth.value.oauth = {
    grant: "code",
    authUrl: "",
    tokenUrl: "",
    clientId: "",
    clientSecret: "",
    scope: "",
    redirectUri: "http://127.0.0.1:7842/oauth/callback",
    refreshToken: "",
    expiresAt: 0,
    clientAuth: "body",
  };
const config = computed(() => auth.value.oauth!),
  busy = ref(false),
  error = ref(""),
  status = ref("");
let flow = "",
  requestId = "",
  cancelled = false;
const base64url = (bytes: Uint8Array) =>
  btoa(String.fromCharCode(...bytes))
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
async function stop() {
  cancelled = true;
  busy.value = false;
  status.value = "已取消";
  if (flow) await api.oauthClose(flow).catch(() => {});
  if (requestId) await api.cancel(requestId).catch(() => {});
}
async function acquire(refresh = false) {
  if (busy.value) return;
  busy.value = true;
  cancelled = false;
  error.value = "";
  const target = auth.value,
    cfg = JSON.parse(JSON.stringify(config.value)) as NonNullable<
      Auth["oauth"]
    >;
  try {
    const fields = [pair("client_id", cfg.clientId)];
    if (cfg.clientAuth === "body" && cfg.clientSecret)
      fields.push(pair("client_secret", cfg.clientSecret));
    if (refresh) {
      if (!cfg.refreshToken)
        throw new Error("没有 Refresh Token，请先获取 Token");
      fields.push(
        pair("grant_type", "refresh_token"),
        pair("refresh_token", cfg.refreshToken),
      );
    } else if (cfg.grant === "code") {
      const callback = await api.oauthStart(cfg.redirectUri);
      flow = callback.id;
      const verifier = base64url(crypto.getRandomValues(new Uint8Array(32))),
        challenge = base64url(
          new Uint8Array(
            await crypto.subtle.digest(
              "SHA-256",
              new TextEncoder().encode(verifier),
            ),
          ),
        );
      const url = new URL(cfg.authUrl);
      url.searchParams.set("response_type", "code");
      url.searchParams.set("client_id", cfg.clientId);
      url.searchParams.set("redirect_uri", callback.redirectUri);
      url.searchParams.set("state", callback.state);
      url.searchParams.set("code_challenge", challenge);
      url.searchParams.set("code_challenge_method", "S256");
      if (cfg.scope) url.searchParams.set("scope", cfg.scope);
      status.value = "请在浏览器完成授权；期间保持此面板打开";
      await openExternal(url.toString());
      let code = "";
      while (!cancelled) {
        const result = await api.oauthPoll(flow);
        if (result.done) {
          if (result.error) throw new Error(result.error);
          code = result.code ?? "";
          break;
        }
        await new Promise((resolve) => setTimeout(resolve, 500));
      }
      if (cancelled) throw new Error("授权已取消");
      if (!code) throw new Error("授权没有返回 code");
      fields.push(
        pair("grant_type", "authorization_code"),
        pair("code", code),
        pair("redirect_uri", callback.redirectUri),
        pair("code_verifier", verifier),
      );
    } else if (cfg.grant === "client_credentials")
      fields.push(pair("grant_type", "client_credentials"));
    else
      fields.push(
        pair("grant_type", "password"),
        pair("username", target.username),
        pair("password", target.password),
      );
    if (cfg.scope) fields.push(pair("scope", cfg.scope));
    if (cancelled) throw new Error("授权已取消");
    const request = freshRequest();
    request.url = cfg.tokenUrl;
    request.method = "POST";
    request.bodyMode = "urlencoded";
    request.fields = fields;
    request.proxy = props.request.proxy;
    request.insecure = props.request.insecure;
    request.caPath = props.request.caPath;
    if (cfg.clientAuth === "basic")
      request.auth = {
        ...request.auth,
        type: "basic",
        username: cfg.clientId,
        password: cfg.clientSecret,
      };
    const spec = compile(request, {});
    spec.id = uid();
    requestId = spec.id;
    status.value = "正在获取 Token…";
    const response = await api.send(spec);
    if (response.error) throw new Error(response.error);
    if (response.bytes > 128 * 1024) throw new Error("Token 响应超过 128 KiB");
    let text = "",
      offset = 0;
    const decoder = new TextDecoder();
    while (offset < response.bytes) {
      const chunk = await api.slice(spec.id, offset);
      if (chunk.nextOffset === offset) throw new Error("Token 响应读取不完整");
      text += decoder.decode(Uint8Array.from(chunk.data), { stream: true });
      offset = chunk.nextOffset;
    }
    text += decoder.decode();
    const token = JSON.parse(text);
    if (response.status >= 400)
      throw new Error(
        `Token 端点返回 ${response.status}：${String(token.error_description ?? token.error ?? "授权失败").slice(0, 256)}`,
      );
    if (typeof token.access_token !== "string")
      throw new Error("Token 响应缺少 access_token");
    if (cancelled) throw new Error("授权已取消");
    target.token = token.access_token;
    target.oauth!.refreshToken = token.refresh_token ?? cfg.refreshToken;
    target.oauth!.expiresAt =
      Number(token.expires_in) > 0
        ? Date.now() + Number(token.expires_in) * 1000
        : 0;
    status.value = "Token 已更新";
  } catch (e) {
    error.value = String(e);
    status.value = "";
  } finally {
    if (flow) await api.oauthClose(flow).catch(() => {});
    if (requestId) await api.release(requestId).catch(() => {});
    flow = "";
    requestId = "";
    busy.value = false;
  }
}
onUnmounted(() => {
  void stop();
});
</script>
<template>
  <div class="pt-oauth">
    <div class="pt-form">
      <label
        >Grant Type<select v-model="config.grant" :disabled="busy">
          <option value="code">Authorization Code + PKCE</option>
          <option value="client_credentials">Client Credentials</option>
          <option value="password">Password Credentials</option>
        </select></label
      >
      <label v-if="config.grant === 'code'"
        >Authorization URL<input
          v-model="config.authUrl"
          :disabled="busy"
          placeholder="https://auth.example.com/authorize"
      /></label>
      <label
        >Token URL<input
          v-model="config.tokenUrl"
          :disabled="busy"
          placeholder="https://auth.example.com/token"
      /></label>
      <label
        >Client ID<input v-model="config.clientId" :disabled="busy" /></label
      ><label
        >Client Secret<input
          v-model="config.clientSecret"
          type="password"
          autocomplete="off"
          :disabled="busy"
      /></label>
      <label
        >客户端认证<select v-model="config.clientAuth" :disabled="busy">
          <option value="body">发送到请求正文</option>
          <option value="basic">HTTP Basic</option>
        </select></label
      >
      <label>Scope<input v-model="config.scope" :disabled="busy" /></label>
      <label v-if="config.grant === 'code'"
        >Callback URL<input v-model="config.redirectUri" :disabled="busy"
      /></label>
      <template v-if="config.grant === 'password'"
        ><label>用户名<input v-model="auth.username" :disabled="busy" /></label
        ><label
          >密码<input
            v-model="auth.password"
            type="password"
            :disabled="busy" /></label
      ></template>
      <label
        >Access Token<input
          v-model="auth.token"
          type="password"
          autocomplete="off"
          :disabled="busy" /></label
      ><label
        >Refresh Token<input
          v-model="config.refreshToken"
          type="password"
          autocomplete="off"
          :disabled="busy"
      /></label>
    </div>
    <div class="pt-body-toolbar">
      <button v-if="busy" @click="stop">取消授权</button
      ><template v-else
        ><button class="pt-primary" @click="acquire(false)">获取 Token</button
        ><button :disabled="!config.refreshToken" @click="acquire(true)">
          刷新 Token
        </button></template
      ><span v-if="config.expiresAt" class="pt-hint"
        >过期时间 {{ new Date(config.expiresAt).toLocaleString() }}</span
      >
    </div>
    <p v-if="status" class="pt-hint" role="status">{{ status }}</p>
    <p v-if="error" class="pt-error" role="alert">{{ error }}</p>
  </div>
</template>
