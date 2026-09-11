<script setup lang="ts">
import { paneDragging } from '../lib/layout'

defineProps<{
  label: string
}>()

const emit = defineEmits<{
  start: []
  drag: [clientX: number]
  end: []
  toggle: []
}>()

function onPointerDown(event: PointerEvent) {
  const target = event.currentTarget as HTMLElement
  target.setPointerCapture(event.pointerId)
  document.body.style.userSelect = 'none'
  document.body.style.cursor = 'col-resize'
  emit('start')
  const move = (ev: PointerEvent) => emit('drag', ev.clientX)
  const up = () => {
    document.body.style.userSelect = ''
    document.body.style.cursor = ''
    target.releasePointerCapture(event.pointerId)
    target.removeEventListener('pointermove', move)
    target.removeEventListener('pointerup', up)
    emit('end')
  }
  target.addEventListener('pointermove', move)
  target.addEventListener('pointerup', up)
}
</script>

<template>
  <div
    class="gutter"
    :class="{ 'is-dragging': paneDragging }"
    role="separator"
    :aria-label="label"
    title="拖动调整宽度，双击收起或展开"
    @pointerdown="onPointerDown"
    @dblclick="emit('toggle')"
  />
</template>

<style scoped>
.gutter {
  width: 0;
  flex-shrink: 0;
  position: relative;
  z-index: 3;
  cursor: col-resize;
}

.gutter::before {
  content: '';
  position: absolute;
  top: 0;
  bottom: 0;
  left: -0.5px;
  width: 1px;
  background: rgba(255, 255, 255, 0.28);
  opacity: 0;
  pointer-events: none;
  transition: opacity var(--ad-transition);
}

.gutter::after {
  content: '';
  position: absolute;
  top: 0;
  bottom: 0;
  left: -5px;
  width: 10px;
}

.gutter:hover::before,
.gutter.is-dragging::before {
  opacity: 1;
}

@media (prefers-reduced-motion: reduce) {
  .gutter::before {
    transition: none;
  }
}
</style>
