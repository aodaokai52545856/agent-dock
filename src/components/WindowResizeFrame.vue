<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'

type ResizeDirection = Parameters<ReturnType<typeof getCurrentWindow>['startResizeDragging']>[0]
import { onMounted, onUnmounted, ref } from 'vue'
import { isTauri } from '../lib/api'

const maximized = ref(false)
let offResized: (() => void) | undefined

async function syncMaximized() {
  if (!isTauri) return
  maximized.value = await getCurrentWindow().isMaximized()
}

async function startResize(direction: ResizeDirection, event: PointerEvent) {
  if (!isTauri || event.button !== 0 || maximized.value) return
  event.preventDefault()
  event.stopPropagation()
  try {
    await getCurrentWindow().startResizeDragging(direction)
  } catch {
    /* permission or unsupported */
  }
}

onMounted(() => {
  void syncMaximized()
  if (!isTauri) return
  void getCurrentWindow()
    .onResized(() => {
      void syncMaximized()
    })
    .then((off) => {
      offResized = off
    })
})

onUnmounted(() => {
  offResized?.()
})
</script>

<template>
  <div v-if="isTauri && !maximized" class="frame" aria-hidden="true">
    <div class="edge n" @pointerdown="startResize('North', $event)" />
    <div class="edge s" @pointerdown="startResize('South', $event)" />
    <div class="edge e" @pointerdown="startResize('East', $event)" />
    <div class="edge w" @pointerdown="startResize('West', $event)" />
    <div class="corner ne" @pointerdown="startResize('NorthEast', $event)" />
    <div class="corner nw" @pointerdown="startResize('NorthWest', $event)" />
    <div class="corner se" @pointerdown="startResize('SouthEast', $event)" />
    <div class="corner sw" @pointerdown="startResize('SouthWest', $event)" />
  </div>
</template>

<style scoped>
.frame {
  position: fixed;
  inset: 0;
  z-index: 50;
  pointer-events: none;
}

.edge,
.corner {
  position: absolute;
  pointer-events: auto;
}

.n,
.s {
  left: 12px;
  right: 12px;
  height: 6px;
  cursor: ns-resize;
}

.n {
  top: 0;
}

.s {
  bottom: 0;
}

.e,
.w {
  top: 12px;
  bottom: 12px;
  width: 6px;
  cursor: ew-resize;
}

.e {
  right: 0;
}

.w {
  left: 0;
}

.corner {
  width: 12px;
  height: 12px;
}

.ne {
  top: 0;
  right: 0;
  cursor: nesw-resize;
}

.nw {
  top: 0;
  left: 0;
  cursor: nwse-resize;
}

.se {
  right: 0;
  bottom: 0;
  cursor: nwse-resize;
}

.sw {
  left: 0;
  bottom: 0;
  cursor: nesw-resize;
}
</style>
