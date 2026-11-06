import type { RequestDoc, ResponseMeta, TestResult } from "./model";
export interface ScriptInput {
  request: RequestDoc;
  response?: ResponseMeta;
  responseBody?: string;
  globals: Record<string, string>;
  collection: Record<string, string>;
  environment: Record<string, string>;
  variables: Record<string, string>;
  data?: Record<string, string>;
  iteration?: number;
  iterationCount?: number;
}
export interface ScriptResult {
  request: RequestDoc;
  tests: TestResult[];
  logs: string[];
  globals: Record<string, string>;
  collection: Record<string, string>;
  environment: Record<string, string>;
  variables: Record<string, string>;
  next?: string | null;
  skip: boolean;
}
let running = false;
export async function runScripts(
  scripts: string[],
  input: ScriptInput,
  signal: AbortSignal,
  network: (request: unknown) => Promise<unknown>,
): Promise<ScriptResult> {
  if (running) throw new Error("另一个脚本正在执行，请等待或停止该请求");
  if (signal.aborted) throw new Error("脚本已取消");
  running = true;
  return new Promise((resolve, reject) => {
    let worker: Worker | undefined;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let ended = false;
    const finish = (error?: unknown, result?: ScriptResult) => {
      if (ended) return;
      ended = true;
      clearTimeout(timer);
      signal.removeEventListener("abort", abort);
      worker?.terminate();
      running = false;
      if (error) reject(error);
      else resolve(result!);
    };
    const abort = () => finish(new Error("脚本已取消"));
    try {
      worker = new Worker(new URL("./script.worker.ts", import.meta.url), {
        type: "module",
      });
      signal.addEventListener("abort", abort, { once: true });
      timer = setTimeout(
        () => finish(new Error("脚本超过 30 秒总时限，已终止")),
        30000,
      );
      worker.onerror = (e) => finish(new Error(e.message));
      worker.onmessage = async (e) => {
        if (ended) return;
        const message = e.data;
        if (message.type === "done") finish(undefined, message.result);
        else if (message.type === "error") finish(new Error(message.error));
        else if (message.type === "network") {
          try {
            const response = await network(message.request);
            if (!ended)
              worker!.postMessage({
                type: "network-result",
                id: message.id,
                response,
              });
          } catch (error) {
            if (!ended)
              worker!.postMessage({
                type: "network-result",
                id: message.id,
                error: String(error),
              });
          }
        }
      };
      worker.postMessage({ type: "run", scripts, input });
    } catch (e) {
      finish(e);
    }
  });
}
