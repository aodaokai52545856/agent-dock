<script setup lang="ts">
import type { ToolId } from '../lib/types'

defineProps<{
  id: ToolId
}>()
</script>

<template>
  <span class="frame" :data-tool="id">
    <!-- Crop guide: same circle for every mark. Grok fills it; others scale down to match. -->
    <svg class="guide" viewBox="0 0 32 32" aria-hidden="true">
      <circle cx="16" cy="16" r="15" />
    </svg>
    <svg
      v-if="id === 'opencode'"
      class="mark"
      viewBox="0 0 24 30"
      preserveAspectRatio="xMidYMid meet"
      aria-hidden="true"
    >
      <path d="M18 24H6V12H18V24Z" fill="#5c5858" />
      <path d="M18 6H6V24H18V6ZM24 30H0V0H24V30Z" fill="#cfc9c9" />
    </svg>
    <svg
      v-else-if="id === 'grokbuild'"
      class="mark"
      viewBox="55 60 400 390"
      preserveAspectRatio="xMidYMid meet"
      aria-hidden="true"
    >
      <path
        fill="currentColor"
        fill-rule="evenodd"
        d="M213.235 306.019l178.976-180.002v.169l51.695-51.763c-.924 1.32-1.86 2.605-2.785 3.89-39.281 54.164-58.46 80.649-43.07 146.922l-.09-.101c10.61 45.11-.744 95.137-37.398 131.836-46.216 46.306-120.167 56.611-181.063 14.928l42.462-19.675c38.863 15.278 81.392 8.57 111.947-22.03 30.566-30.6 37.432-75.159 22.065-112.252-2.92-7.025-11.67-8.795-17.792-4.263l-124.947 92.341zm-25.786 22.437l-.033.034L68.094 435.217c7.565-10.429 16.957-20.294 26.327-30.149 26.428-27.803 52.653-55.359 36.654-94.302-21.422-52.112-8.952-113.177 30.724-152.898 41.243-41.254 101.98-51.661 152.706-30.758 11.23 4.172 21.016 10.114 28.638 15.639l-42.359 19.584c-39.44-16.563-84.629-5.299-112.207 22.313-37.298 37.308-44.84 102.003-1.128 143.81z"
      />
    </svg>
    <svg
      v-else
      class="mark"
      viewBox="0 0 24 25"
      preserveAspectRatio="xMidYMid meet"
      aria-hidden="true"
    >
      <path
        d="M21.7202 0.94c1.23 0 2.23 1 2.23 2.23s-1 2.23-2.23 2.23h-1.97c-.15 0-.26-.12-.26-.26V3.17c0-1.23 1-2.23 2.23-2.23Z"
        fill="#1783FF"
      />
      <path
        d="M9.39 13.95 17.82 5.59c.16-.16.07-.47-.14-.47h-4.54l-.14.06-9.08 9.01c-.14.14-.35.02-.35-.21V5.39c0-.15-.1-.27-.22-.27H.22C.1 5.12 0 5.24 0 5.39v18.53c0 .15.1.27.22.27H3.35c.12 0 .22-.12.22-.27v-3.78c0-.08.03-.16.08-.21l2.82-2.79c.07-.07.16-.08.24-.03l7.53 5.54c1.23.83 2.61 1.34 4.01 1.49.12.01.23-.11.23-.27v-3.56c0-.14-.08-.25-.19-.26-.82-.13-1.63-.45-2.35-.94l-6.52-4.72c-.14-.09-.15-.32-.03-.44Z"
        fill="currentColor"
      />
    </svg>
  </span>
</template>

<style scoped>
.frame {
  position: relative;
  display: grid;
  place-items: center;
  width: 100%;
  height: 100%;
}

.guide {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
}

.guide circle {
  fill: none;
  stroke: none;
}

.mark {
  display: block;
  color: #f4f4f4;
}

.frame[data-tool='grokbuild'] .mark {
  width: 100%;
  height: 100%;
}

.frame[data-tool='kimi'] .mark {
  width: 78%;
  height: 78%;
}

.frame[data-tool='opencode'] .mark {
  width: 68%;
  height: 68%;
}
</style>
