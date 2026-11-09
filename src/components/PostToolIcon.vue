<script setup lang="ts">
defineProps<{ name: string }>();
const paths: Record<string, string> = {
  bolt: "M13 2 4 13h7l-1 9 10-12h-7l1-8Z",
  plus: "M12 5v14M5 12h14",
  search: "m21 21-4.4-4.4M19 11a8 8 0 1 1-16 0 8 8 0 0 1 16 0Z",
  folder:
    "M3 7V5a2 2 0 0 1 2-2h5l2 3h7a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z",
  collection: "M4 4h16v16H4zM8 4v16M12 8h4M12 12h4",
  grid: "M3 3h7v7H3zM14 3h7v7h-7zM3 14h7v7H3zM14 14h7v7h-7z",
  history: "M3 11a9 9 0 1 1 2.5 7M3 4v7h7M12 7v5l3 2",
  file: "M14 2H5v20h14V7l-5-5ZM14 2v6h5M8 12h8M8 16h6",
  close: "m6 6 12 12M6 18 18 6",
  settings: "M4 7h16M4 17h16M8 4v6M16 14v6",
  copy: "M8 8h12v13H8zM4 16H2V2h12v2",
  save: "M19 21H5a2 2 0 0 1-2-2V3h14l4 4v12a2 2 0 0 1-2 2ZM7 3v6h10V3M7 21v-8h10v8",
  send: "m3 3 19 9-19 9 4-9-4-9ZM7 12h15",
  stop: "M5 5h14v14H5z",
  download: "M12 3v12m-5-5 5 5 5-5M4 17v4h16v-4",
  upload: "M12 16V4m-5 5 5-5 5 5M4 17v4h16v-4",
  globe:
    "M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0ZM3 12h18M12 3a19 19 0 0 1 0 18 19 19 0 0 1 0-18Z",
  play: "m8 4 13 8-13 8V4Z",
  chevron: "m9 5 7 7-7 7",
  more: "M5 11h.01v2H5zM12 11h.01v2H12zM19 11h.01v2H19z",
  code: "m8 7-5 5 5 5m8-10 5 5-5 5m-3-14-2 18",
  shield: "m12 2 9 4v6c0 5-9 10-9 10S3 17 3 12V6l9-4Zm-4 10 3 3 5-6",
  check: "m5 12 4 4L19 6",
  trash: "M3 6h18M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7M14 10v7",
};
</script>
<template>
  <svg
    class="pt-icon"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.6"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path :d="paths[name] ?? paths.file" />
  </svg>
</template>
