// Standalone browser smoke-test entry; not imported by the application.
import { createApp, h, ref } from "vue";
import Pane from "../src/components/PostToolPane.vue";
import "../src/styles/tokens.css";
import "../src/styles/global.css";

export function mount() {
  const active = ref(true);
  (window as unknown as { __posttoolActive: typeof active }).__posttoolActive =
    active;
  const app = createApp({ setup: () => () => h(Pane, { active: active.value }) });
  app.mount("#app");
  return app;
}
