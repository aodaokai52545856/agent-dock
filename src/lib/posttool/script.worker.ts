import {
  newQuickJSWASMModuleFromVariant,
  type QuickJSContext,
  type QuickJSRuntime,
} from "quickjs-emscripten-core";
import RELEASE_SYNC from "@jitl/quickjs-wasmfile-release-sync";
import { SCRIPT_SHIM } from "./scriptShim";
let vm: QuickJSContext | undefined,
  runtime: QuickJSRuntime | undefined,
  spent = 0,
  segment = 0,
  closed = false;
const timers = new Set<ReturnType<typeof setTimeout>>();
function dispose() {
  if (closed) return;
  closed = true;
  for (const t of timers) clearTimeout(t);
  timers.clear();
  vm?.dispose();
  runtime?.dispose();
}
function fail(e: unknown) {
  postMessage({ type: "error", error: String(e) });
  dispose();
}
function evaluate(code: string) {
  if (!vm || closed) throw new Error("脚本运行时未初始化");
  segment = performance.now();
  const result = vm.evalCode(code);
  spent += performance.now() - segment;
  if (result.error) {
    const error = vm.dump(result.error);
    result.error.dispose();
    throw new Error(
      typeof error === "object"
        ? (error.message ?? JSON.stringify(error))
        : String(error),
    );
  }
  return result.value;
}
function settle() {
  if (!runtime || !vm || closed) return;
  segment = performance.now();
  const jobs = runtime.executePendingJobs(1000);
  spent += performance.now() - segment;
  if (jobs.error) {
    const error = vm.dump(jobs.error);
    jobs.error.dispose();
    throw new Error(JSON.stringify(error));
  }
  jobs.dispose();
  const pending = evaluate("__pending");
  const n = vm.getNumber(pending);
  pending.dispose();
  if (n === 0 && !runtime.hasPendingJob()) {
    const value = evaluate("__result()");
    const result = JSON.parse(vm.getString(value));
    value.dispose();
    postMessage({ type: "done", result });
    dispose();
  } else if (runtime.hasPendingJob()) {
    const t = setTimeout(() => {
      timers.delete(t);
      try {
        settle();
      } catch (e) {
        fail(e);
      }
    }, 0);
    timers.add(t);
  }
}
self.onmessage = async (event: MessageEvent) => {
  try {
    const input = event.data;
    if (input.type === "run") {
      const qjs = await newQuickJSWASMModuleFromVariant(RELEASE_SYNC);
      runtime = qjs.newRuntime();
      runtime.setMemoryLimit(64 * 1024 * 1024);
      runtime.setMaxStackSize(512 * 1024);
      runtime.setInterruptHandler(
        () => spent + performance.now() - segment > 3000,
      );
      vm = runtime.newContext();
      const value = vm.newString(JSON.stringify(input.input));
      vm.setProp(vm.global, "__inputJSON", value);
      value.dispose();
      const send = vm.newFunction("__send", (id, request) => {
        const text = vm!.getString(request);
        if (text.length > 2 * 1024 * 1024)
          throw new Error("脚本请求超过大小上限");
        postMessage({
          type: "network",
          id: vm!.getNumber(id),
          request: JSON.parse(text),
        });
      });
      vm.setProp(vm.global, "__send", send);
      send.dispose();
      const timer = vm.newFunction("__timer", (id, delay) => {
        const index = vm!.getNumber(id),
          t = setTimeout(() => {
            timers.delete(t);
            try {
              evaluate(`__resolveTimer(${index})`).dispose();
              settle();
            } catch (e) {
              fail(e);
            }
          }, vm!.getNumber(delay));
        timers.add(t);
      });
      vm.setProp(vm.global, "__timer", timer);
      timer.dispose();
      evaluate(SCRIPT_SHIM).dispose();
      for (const code of input.scripts as string[])
        if (code.trim()) evaluate(code).dispose();
      settle();
    } else if (input.type === "network-result" && vm && !closed) {
      evaluate(
        `__resolveNetwork(${Number(input.id)},${JSON.stringify(JSON.stringify(input.response ?? null))},${JSON.stringify(input.error ?? null)})`,
      ).dispose();
      settle();
    }
  } catch (e) {
    fail(e);
  }
};
