<script setup lang="ts">
defineProps<{
  open: boolean
  title: string
  body: string
  action: string
  danger?: boolean
}>()

const emit = defineEmits<{
  close: []
  confirm: []
}>()
</script>

<template>
  <div v-if="open" class="ad-mask" @click.self="emit('close')">
    <div class="ad-dialog dialog" role="dialog" aria-modal="true">
      <h2>{{ title }}</h2>
      <p>{{ body }}</p>
      <div class="actions">
        <button type="button" class="btn btn-ghost" @click="emit('close')">取消</button>
        <button type="button" class="btn" :class="danger ? 'btn-danger' : 'btn-primary'" @click="emit('confirm')">
          {{ action }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dialog {
  width: 480px;
  max-width: calc(100vw - 48px);
}

h2 {
  margin: 0 0 8px;
  font-size: 16px;
  line-height: 24px;
}

p {
  margin: 0 0 24px;
  color: var(--ad-muted);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
