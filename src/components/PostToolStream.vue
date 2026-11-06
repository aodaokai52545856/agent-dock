<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { uid, formatBytes, type Pair } from "../lib/posttool/model";
import * as api from "../lib/posttool/api";
import PostToolPairs from "./PostToolPairs.vue";
import { open } from "@tauri-apps/plugin-dialog";
const props = defineProps<{ active: boolean }>();
const kind = ref("websocket"),
  url = ref(""),
  headers = ref<Pair[]>([]),
  username = ref(""),
  password = ref(""),
  clientId = ref(""),
  namespace = ref("/"),
  auth = ref("{}"),
  cleanSession = ref(true);
const id = ref(""),
  state = ref("closed"),
  error = ref(""),
  messages = ref<api.StreamRow[]>([]),
  payload = ref(""),
  topic = ref(""),
  qos = ref(0),
  retain = ref(false),
  binary = ref(false),
  query = ref(""),
  dropped = ref(0),
  bufferBytes = ref(0),
  paused = ref(false),
  configOpen = ref(false);
const proto = ref<api.ProtoInfo>({ id: "", methods: [] }),
  method = ref(""),
  protoPath = ref(""),
  includePaths = ref(""),
  services = ref<string[]>([]),
  reflectService = ref(""),
  loadingProto = ref(false);
const selectedMethod = computed(() =>
  proto.value.methods.find((m) => m.name === method.value),
);
async function loadProto() {
  loadingProto.value = true;
  try {
    error.value = "";
    if (!protoPath.value) {
      const path = await open({
        multiple: false,
        filters: [{ name: "Protocol Buffers", extensions: ["proto"] }],
      });
      if (typeof path !== "string") return;
      protoPath.value = path;
    }
    proto.value = await api.protoLoad(
      protoPath.value,
      includePaths.value
        .split(";")
        .map((s) => s.trim())
        .filter(Boolean),
    );
    method.value = proto.value.methods[0]?.name ?? "";
  } catch (e) {
    error.value = String(e);
  } finally {
    loadingProto.value = false;
  }
}
async function reflect() {
  loadingProto.value = true;
  try {
    error.value = "";
    const result = await api.grpcReflect(
      url.value,
      JSON.parse(JSON.stringify(headers.value)),
      reflectService.value || undefined,
    );
    if (result.services) {
      services.value = result.services;
      reflectService.value = services.value[0] ?? "";
    } else {
      proto.value = result;
      method.value = result.methods[0]?.name ?? "";
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    loadingProto.value = false;
  }
}
const connected = computed(() => state.value === "connected"),
  running = computed(() => ["connecting", "connected"].includes(state.value));
const filtered = computed(() =>
  messages.value.filter(
    (m) =>
      !query.value ||
      m.payload.includes(query.value) ||
      m.topic.includes(query.value),
  ),
);
let timer: ReturnType<typeof setTimeout> | undefined,
  alive = true,
  cursor = 0,
  seq = 0;
async function connect() {
  if (running.value) return;
  try {
    error.value = "";
    if (id.value) await api.streamRelease(id.value);
    id.value = uid();
    cursor = 0;
    messages.value = [];
    dropped.value = 0;
    state.value = "connecting";
    await api.streamConnect({
      id: id.value,
      kind: kind.value,
      url: url.value,
      headers: JSON.parse(JSON.stringify(headers.value)),
      username: username.value,
      password: password.value,
      clientId: clientId.value,
      namespace: namespace.value,
      auth: auth.value,
      cleanSession: cleanSession.value,
      protoId: proto.value.id,
      method: method.value,
      payload: payload.value,
    });
    poll();
  } catch (e) {
    error.value = String(e);
    state.value = "error";
  }
}
async function disconnect() {
  try {
    await api.streamClose(id.value);
    poll();
  } catch (e) {
    error.value = String(e);
  }
}
async function send(action = "send") {
  try {
    await api.streamSend(id.value, {
      action,
      payload: payload.value,
      topic: topic.value,
      qos: qos.value,
      retain: retain.value,
      binary: binary.value,
    });
    if (props.active) poll();
  } catch (e) {
    error.value = String(e);
  }
}
function poll() {
  clearTimeout(timer);
  const request = ++seq;
  const tick = async () => {
    if (!alive || !props.active || !id.value || request !== seq) return;
    try {
      const result = await api.streamPoll(id.value, cursor);
      if (!alive || request !== seq) return;
      state.value = result.status;
      error.value = result.error ?? "";
      bufferBytes.value = result.bufferBytes;
      dropped.value = result.dropped;
      if (!paused.value) {
        messages.value.push(...result.items);
        messages.value = messages.value.slice(-500);
        let size = messages.value.reduce(
          (n, m) => n + m.payload.length + m.topic.length,
          0,
        );
        while (size > 512 * 1024 && messages.value.length) {
          const first = messages.value.shift()!;
          size -= first.payload.length + first.topic.length;
        }
        cursor = result.cursor;
      }
    } catch (e) {
      error.value = String(e);
      state.value = "error";
    }
    if (
      alive &&
      props.active &&
      !paused.value &&
      request === seq &&
      running.value
    )
      timer = setTimeout(tick, 250);
  };
  void tick();
}
function clear() {
  messages.value = [];
}
async function copy() {
  try {
    await navigator.clipboard.writeText(
      filtered.value
        .map(
          (m) =>
            `${new Date(m.time).toISOString()} ${m.direction} ${m.topic} ${m.payload}`,
        )
        .join("\n"),
    );
  } catch (e) {
    error.value = String(e);
  }
}
watch(
  () => props.active,
  (active) => {
    if (active) poll();
    else {
      seq++;
      clearTimeout(timer);
    }
  },
);
watch(paused, (value) => {
  if (!value) poll();
  else {
    seq++;
    clearTimeout(timer);
  }
});
onUnmounted(() => {
  alive = false;
  seq++;
  clearTimeout(timer);
  if (id.value) void api.streamRelease(id.value).catch(() => {});
});
</script>
<template>
  <section v-if="active" class="pt-stream" aria-label="长连接调试">
    <form class="pt-stream-connect" @submit.prevent="connect">
      <select v-model="kind" aria-label="协议" :disabled="running">
        <option value="websocket">WebSocket</option>
        <option value="socketio">Socket.IO · Engine.IO 4</option>
        <option value="mqtt">MQTT 3.1.1</option>
        <option value="grpc">gRPC</option></select
      ><input
        v-model="url"
        required
        aria-label="连接 URL"
        :placeholder="
          kind === 'mqtt' ? 'mqtt://localhost:1883' : 'ws://localhost:8080'
        "
        :disabled="running"
      /><button v-if="running" type="button" @click="disconnect">断开</button
      ><button v-else type="submit" class="pt-primary">连接</button
      ><button type="button" @click="configOpen = !configOpen">连接设置</button>
    </form>
    <div v-if="configOpen" class="pt-stream-config">
      <PostToolPairs
        v-if="kind !== 'mqtt'"
        v-model="headers"
        key-label="握手 Header"
      />
      <div v-if="kind === 'mqtt'" class="pt-runner-options">
        <label
          >Client ID<input
            v-model="clientId"
            placeholder="留空自动生成" /></label
        ><label>用户名<input v-model="username" /></label
        ><label
          >密码<input
            v-model="password"
            type="password"
            autocomplete="off" /></label
        ><label
          ><input v-model="cleanSession" type="checkbox" />Clean session</label
        >
      </div>
      <div v-if="kind === 'socketio'" class="pt-runner-options">
        <label>Namespace<input v-model="namespace" /></label
        ><label>Auth JSON<input v-model="auth" /></label>
      </div>
      <p class="pt-hint">
        使用本机直连与系统信任证书。单消息上限 1 MiB；缓冲上限 5,000 条 / 8
        MiB。断开后不会自动重连。
      </p>
    </div>
    <div v-if="kind === 'grpc'" class="pt-grpc-config">
      <div class="pt-file-select">
        <input
          v-model="protoPath"
          aria-label="proto 文件"
          placeholder="本地 .proto 文件路径"
          :disabled="running"
        /><input
          v-model="includePaths"
          aria-label="proto 依赖目录"
          placeholder="依赖目录，以分号分隔"
          :disabled="running"
        /><button :disabled="running || loadingProto" @click="loadProto">
          {{ loadingProto ? "加载中…" : "加载 proto" }}</button
        ><button :disabled="running || loadingProto || !url" @click="reflect">
          Reflection v1alpha
        </button>
      </div>
      <div class="pt-file-select">
        <select
          v-if="services.length"
          v-model="reflectService"
          aria-label="反射服务"
        >
          <option v-for="service in services" :key="service">
            {{ service }}
          </option></select
        ><button
          v-if="services.length"
          :disabled="running || loadingProto"
          @click="reflect"
        >
          加载服务</button
        ><select v-model="method" aria-label="gRPC 方法" :disabled="running">
          <option value="">选择服务 / 方法</option>
          <option
            v-for="item in proto.methods"
            :key="item.name"
            :value="item.name"
          >
            {{ item.name }} {{ item.clientStreaming ? "[client stream]" : "" }}
            {{ item.serverStreaming ? "[server stream]" : "" }}
          </option></select
        ><span v-if="selectedMethod" class="pt-hint"
          >{{ selectedMethod.input }} → {{ selectedMethod.output }}</span
        >
      </div>
      <button
        v-if="connected && selectedMethod?.clientStreaming"
        @click="send('finish')"
      >
        结束客户端发送流
      </button>
      <p class="pt-hint">
        输入 http:// 或 https:// 服务地址。下方 JSON
        在开始调用时作为首条消息发送；客户端流可继续发送，结束后等待服务端响应。
      </p>
    </div>
    <div class="pt-stream-state">
      <span :class="connected ? 'pt-success' : 'pt-live'">{{
        {
          closed: "已断开",
          connecting: "连接中",
          connected: "已连接",
          error: "连接错误",
        }[state] ?? state
      }}</span
      ><span>缓冲 {{ formatBytes(bufferBytes) }}</span
      ><span v-if="dropped">已淘汰 {{ dropped }} 条旧消息</span>
      <div class="pt-spacer" />
      <input
        v-model="query"
        aria-label="搜索消息"
        placeholder="搜索消息或主题"
      /><button @click="paused = !paused">
        {{ paused ? "继续画面" : "暂停画面" }}</button
      ><button @click="clear">清空画面</button
      ><button @click="copy">复制可见消息</button>
    </div>
    <p v-if="error" class="pt-response-error" role="alert">{{ error }}</p>
    <div class="pt-stream-messages">
      <div v-if="!filtered.length" class="pt-response-empty">
        <strong>等待消息</strong><span>连接服务后，收发消息会显示在这里</span>
      </div>
      <div v-for="message in filtered" :key="message.seq" class="pt-stream-row">
        <span :class="message.direction === 'in' ? 'pt-success' : 'pt-live'">{{
          message.direction === "in"
            ? "↓"
            : message.direction === "out"
              ? "↑"
              : "•"
        }}</span
        ><time>{{ new Date(message.time).toLocaleTimeString() }}</time
        ><strong>{{ message.topic }}</strong>
        <pre
          >{{ message.payload
          }}{{ message.truncated ? "\n[消息预览已截断]" : "" }}</pre>
      </div>
    </div>
    <div class="pt-stream-send">
      <div class="pt-stream-send-options">
        <input
          v-if="kind === 'mqtt' || kind === 'socketio'"
          v-model="topic"
          :aria-label="kind === 'mqtt' ? 'MQTT Topic' : 'Socket.IO Event'"
          :placeholder="kind === 'mqtt' ? 'Topic，例如 demo/#' : 'Event 名称'"
        /><template v-if="kind === 'mqtt'"
          ><select v-model.number="qos" aria-label="QoS">
            <option :value="0">QoS 0</option>
            <option :value="1">QoS 1</option>
            <option :value="2">QoS 2</option></select
          ><label><input v-model="retain" type="checkbox" />Retain</label
          ><button :disabled="!connected || !topic" @click="send('subscribe')">
            订阅</button
          ><button
            :disabled="!connected || !topic"
            @click="send('unsubscribe')"
          >
            取消订阅
          </button></template
        ><label v-else-if="kind === 'websocket'"
          ><input v-model="binary" type="checkbox" />Base64 二进制</label
        >
        <div class="pt-spacer" />
        <button
          v-if="kind === 'websocket'"
          :disabled="!connected"
          @click="send('ping')"
        >
          Ping</button
        ><button
          class="pt-primary"
          :disabled="!connected"
          @click="send(kind === 'mqtt' ? 'publish' : 'send')"
        >
          {{ kind === "mqtt" ? "发布消息" : "发送消息" }}
        </button>
      </div>
      <textarea
        v-model="payload"
        aria-label="消息正文"
        maxlength="1048576"
        placeholder="输入消息；Socket.IO 可使用 JSON，二进制消息请输入 Base64"
        spellcheck="false"
      />
    </div>
  </section>
</template>
