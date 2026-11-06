<script setup lang="ts">
import { onMounted, ref } from "vue";
import { modal as vModal } from "../lib/modal";
import {
  freshRequest,
  type Auth,
  type Entry,
  type Pair,
} from "../lib/posttool/model";
import * as api from "../lib/posttool/api";
import PostToolPairs from "./PostToolPairs.vue";
const props = defineProps<{ entry: Entry }>();
const emit = defineEmits<{ close: []; saved: [] }>();
const data = ref<{
  name: string;
  variables: Pair[];
  auth: Auth;
  preScript: string;
  postScript: string;
  [key: string]: unknown;
}>({
  name: props.entry.name,
  variables: [],
  auth: freshRequest().auth,
  preScript: "",
  postScript: "",
});
const loading = ref(true),
  saving = ref(false),
  error = ref("");
function close() {
  if (!saving.value) emit("close");
}
onMounted(async () => {
  try {
    data.value = {
      ...data.value,
      ...(await api.get<typeof data.value>(props.entry.id)),
    };
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
});
async function save() {
  saving.value = true;
  try {
    if (!data.value.name.trim()) throw new Error("名称不能为空");
    await api.put({
      ...props.entry,
      name: data.value.name,
      updated: Date.now(),
      payload: JSON.parse(JSON.stringify(data.value)),
    });
    emit("saved");
    emit("close");
  } catch (e) {
    error.value = String(e);
  } finally {
    saving.value = false;
  }
}
</script>
<template>
  <Teleport to="body"
    ><div class="pt-mask" @mousedown.self="close">
      <section
        v-modal="close"
        class="pt-dialog pt-dialog-wide"
        role="dialog"
        aria-modal="true"
        aria-label="集合与文件夹设置"
      >
        <header>
          <strong
            >{{ entry.kind === "collection" ? "集合" : "文件夹" }}设置</strong
          ><button aria-label="关闭设置" @click="close">×</button>
        </header>
        <div class="pt-dialog-body">
          <p v-if="loading">正在加载…</p>
          <template v-else
            ><label class="pt-dialog-label"
              >名称<input v-model="data.name"
            /></label>
            <div class="pt-form">
              <label
                >公共鉴权<select v-model="data.auth.type">
                  <option value="none">No Auth</option>
                  <option v-if="entry.kind === 'folder'" value="inherit">
                    继承父级
                  </option>
                  <option value="bearer">Bearer Token</option>
                  <option value="basic">Basic Auth</option>
                  <option value="apikey">API Key</option>
                </select></label
              ><label v-if="data.auth.type === 'bearer'"
                >Token<input
                  v-model="data.auth.token"
                  type="password"
                  autocomplete="off" /></label
              ><template v-if="data.auth.type === 'basic'"
                ><label>用户名<input v-model="data.auth.username" /></label
                ><label
                  >密码<input
                    v-model="data.auth.password"
                    type="password" /></label></template
              ><template v-if="data.auth.type === 'apikey'"
                ><label>Key<input v-model="data.auth.key" /></label
                ><label
                  >Value<input
                    v-model="data.auth.value"
                    type="password" /></label
                ><label
                  >位置<select v-model="data.auth.location">
                    <option value="header">Header</option>
                    <option value="query">Query</option>
                  </select></label
                ></template
              >
            </div>
            <template v-if="entry.kind === 'collection'"
              ><h4>集合变量</h4>
              <PostToolPairs v-model="data.variables" secrets
            /></template>
            <div class="pt-script-grid pt-collection-scripts">
              <label
                >Pre-request<textarea
                  v-model="data.preScript"
                  spellcheck="false"
                  aria-label="集合前置脚本"
                /></label
              ><label
                >Post-response<textarea
                  v-model="data.postScript"
                  spellcheck="false"
                  aria-label="集合后置脚本"
                />
              </label></div
          ></template>
          <p v-if="error" class="pt-error" role="alert">{{ error }}</p>
        </div>
        <footer>
          <button :disabled="saving" @click="close">关闭</button
          ><button
            :disabled="loading || saving"
            class="pt-primary"
            @click="save"
          >
            {{ saving ? "保存中…" : "保存设置" }}
          </button>
        </footer>
      </section>
    </div></Teleport
  >
</template>
