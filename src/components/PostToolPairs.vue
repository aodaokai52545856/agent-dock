<script setup lang="ts">
import type { Pair } from "../lib/posttool/model";
import { pair } from "../lib/posttool/model";
import PostToolIcon from "./PostToolIcon.vue";
const rows = defineModel<Pair[]>({ required: true });
withDefaults(
  defineProps<{
    files?: boolean;
    secrets?: boolean;
    keyLabel?: string;
    valueLabel?: string;
  }>(),
  { keyLabel: "参数名", valueLabel: "值" },
);
</script>
<template>
  <div class="pt-pairs">
    <div class="pt-pair-head">
      <span></span><span>{{ keyLabel }}</span
      ><span>{{ valueLabel }}</span
      ><span>操作</span>
    </div>
    <div v-if="!rows.length" class="pt-pair-empty">
      <PostToolIcon name="code" /><span>暂无{{ keyLabel }}，点击下方添加</span>
    </div>
    <div
      v-for="(row, index) in rows"
      :key="index"
      :class="['pt-pair-row', { disabled: !row.enabled }]"
    >
      <input
        v-model="row.enabled"
        type="checkbox"
        :aria-label="`启用第 ${index + 1} 项`"
      />
      <input
        v-model="row.key"
        :aria-label="`${keyLabel} ${index + 1}`"
        :placeholder="keyLabel"
        spellcheck="false"
      />
      <div class="pt-pair-value">
        <input
          v-model="row.value"
          :type="secrets && row.secret ? 'password' : 'text'"
          :aria-label="`${valueLabel} ${index + 1}`"
          :placeholder="row.file ? '本地文件完整路径' : valueLabel"
          spellcheck="false"
        /><label v-if="files"
          ><input v-model="row.file" type="checkbox" />文件</label
        ><label v-if="secrets"
          ><input v-model="row.secret" type="checkbox" />敏感</label
        >
      </div>
      <button
        type="button"
        :aria-label="`移除第 ${index + 1} 项`"
        @click="rows.splice(index, 1)"
      >
        <PostToolIcon name="close" />
      </button>
    </div>
    <div class="pt-pairs-footer">
      <button
        type="button"
        class="pt-add-row"
        :disabled="rows.length >= 512"
        @click="rows.push(pair())"
      >
        <PostToolIcon name="plus" />添加一项</button
      ><span v-pre>支持 {{ 变量 }}</span>
    </div>
  </div>
</template>
