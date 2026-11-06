// Run with playwright-cli -s=posttool run-code against Vite port 1426.
// UI uses an in-memory Tauri command adapter. Rust transport has separate socket tests.
async (page) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.setViewportSize({ width: 1440, height: 960 });
  await page.route("**/posttool-smoke", (route) =>
    route.fulfill({
      contentType: "text/html",
      body: '<!doctype html><html><head><meta charset="utf-8"></head><body><div id="app" style="height:100vh"></div></body></html>',
    }),
  );
  await page.goto("http://127.0.0.1:1426/posttool-smoke");
  await page.evaluate(async () => {
    const documents = new Map(),
      responses = new Map(),
      cancelled = new Set(),
      calls = [],
      requests = [];
    window.__posttoolSmoke = { documents, responses, calls, requests };
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, args = {}) => {
        calls.push(command);
        if (command === "posttool_document_list") {
          const items = [...documents.values()].filter(
            (d) =>
              d.kind === args.kind &&
              (args.parent == null || d.parent === args.parent) &&
              (!args.search || d.name.includes(args.search)),
          );
          return {
            items: items
              .slice(args.page * 100, args.page * 100 + 100)
              .map((d) => ({ ...d, payload: null })),
            total: items.length,
          };
        }
        if (command === "posttool_document_get") {
          if (!documents.has(args.id)) throw Error("文档不存在");
          return JSON.parse(JSON.stringify(documents.get(args.id).payload));
        }
        if (command === "posttool_document_put") {
          documents.set(
            args.document.id,
            JSON.parse(JSON.stringify(args.document)),
          );
          return;
        }
        if (command === "posttool_document_delete") {
          documents.delete(args.id);
          return;
        }
        if (command === "posttool_request") {
          const spec = args.spec;
          requests.push(JSON.parse(JSON.stringify(spec)));
          const creatingUser =
            spec.method === "POST" && spec.url.includes("/v1/users");
          const body = JSON.stringify(
            creatingUser
              ? {
                  id: "usr_2481",
                  ...JSON.parse(spec.body),
                  created_at: "2026-11-05T10:30:00Z",
                }
              : {
                  ok: true,
                  method: spec.method,
                  url: spec.url,
                  headers: spec.headers,
                  body: spec.body,
                },
          );
          const bytes = new TextEncoder().encode(body);
          const meta = {
            id: spec.id,
            state: "receiving",
            status: creatingUser ? 201 : 200,
            statusText: creatingUser ? "Created" : "OK",
            headers: [
              { key: "content-type", value: "application/json", enabled: true },
            ],
            url: spec.url,
            bytes: bytes.length,
            elapsedMs: 12,
            firstByteMs: 3,
            error: null,
            contentType: "application/json",
          };
          responses.set(spec.id, { meta, bytes });
          await new Promise((resolve) =>
            setTimeout(resolve, spec.url.includes("/slow") ? 1000 : 100),
          );
          if (cancelled.has(spec.id)) {
            meta.state = "cancelled";
            meta.error = "请求已取消";
          } else meta.state = "done";
          return { ...meta };
        }
        if (command === "posttool_cancel") {
          cancelled.add(args.id);
          return;
        }
        if (command === "posttool_response_slice") {
          const r = responses.get(args.id);
          if (!r) throw Error("响应不存在");
          const data = [...r.bytes.slice(args.offset, args.offset + 65536)];
          return {
            meta: { ...r.meta },
            offset: args.offset,
            nextOffset: args.offset + data.length,
            data,
          };
        }
        if (command === "posttool_release") {
          if (args.id) responses.delete(args.id);
          else responses.clear();
          return;
        }
        if (command === "posttool_history_clear") {
          for (const [id, d] of documents)
            if (d.kind === "history") documents.delete(id);
          return;
        }
        throw Error("unhandled mock command " + command);
      },
    };
    const { mount } = await import("/scripts/posttool-ui-entry.ts");
    mount();
  });
  await page
    .getByLabel("请求 URL", { exact: true })
    .fill("http://localhost:9876/echo?q=hello&q=world");
  await page.getByLabel("请求名称", { exact: true }).fill("Echo test");
  await page.getByRole("button", { name: "发送", exact: false }).click();
  await page.getByText("200 OK", { exact: true }).waitFor();
  const response = page.locator(".pt-response-body");
  await response.filter({ hasText: "hello" }).waitFor();
  await page.getByRole("button", { name: "保存 Ctrl S" }).click();
  await page
    .locator(".pt-entry-list")
    .getByRole("button", { name: "GET Echo test" })
    .waitFor();
  const methodPicker = page.getByRole("combobox", {
    name: "HTTP 方法",
    exact: true,
  });
  if ((await methodPicker.locator("option").count()) !== 8)
    throw Error("HTTP method choices are missing");
  for (const method of [
    "POST",
    "PUT",
    "PATCH",
    "DELETE",
    "HEAD",
    "OPTIONS",
    "GET",
  ]) {
    await methodPicker.selectOption(method);
    if (["POST", "PUT", "PATCH"].includes(method)) {
      if (
        (await page.getByLabel("正文类型", { exact: true }).inputValue()) !==
        "json"
      )
        throw Error("Write method did not expose JSON body");
      await page
        .getByLabel("请求正文", { exact: true })
        .fill('{"name":"method test"}');
    }
    const count = await page.evaluate(
      () => window.__posttoolSmoke.requests.length,
    );
    await page.getByRole("button", { name: "发送", exact: true }).click();
    await page.waitForFunction(
      ({ count, method }) => {
        const requests = window.__posttoolSmoke.requests;
        const last = requests.at(-1);
        return (
          requests.length > count &&
          last.method === method &&
          window.__posttoolSmoke.responses.get(last.id)?.meta.state === "done"
        );
      },
      { count, method },
    );
  }
  await methodPicker.selectOption("POST");
  await page.getByLabel("正文类型", { exact: true }).selectOption("multipart");
  await page.getByRole("button", { name: "添加一项", exact: true }).click();
  await page.getByLabel("参数名 1", { exact: true }).fill("name");
  await page.getByLabel("值 1", { exact: true }).fill("retained");
  await methodPicker.selectOption("PUT");
  if (
    (await page.getByLabel("正文类型", { exact: true }).inputValue()) !==
      "multipart" ||
    (await page.getByLabel("值 1", { exact: true }).inputValue()) !== "retained"
  )
    throw Error("Changing method discarded an existing body");
  await page.getByLabel("正文类型", { exact: true }).selectOption("json");
  await methodPicker.selectOption("CUSTOM");
  if (
    !(await page
      .getByRole("button", { name: "发送", exact: true })
      .isDisabled())
  )
    throw Error("Empty custom method can be sent");
  await page.getByLabel("自定义 HTTP 方法", { exact: true }).fill("propfind");
  await page.getByLabel("请求 URL", { exact: true }).focus();
  await page.getByRole("button", { name: "发送", exact: true }).click();
  await page.waitForFunction(
    () =>
      window.__posttoolSmoke.requests.at(-1).method === "PROPFIND" &&
      window.__posttoolSmoke.responses.get(
        window.__posttoolSmoke.requests.at(-1).id,
      )?.meta.state === "done",
  );
  await methodPicker.selectOption("GET");
  await page.getByRole("button", { name: "Scripts", exact: true }).click();
  await page
    .getByLabel("前置脚本", { exact: true })
    .fill(
      "pm.request.headers.upsert({key:'X-Script',value:'worked'}); pm.variables.set('check','yes');",
    );
  await page
    .getByLabel("后置脚本", { exact: true })
    .fill(
      "pm.test('status is 200',()=>pm.response.to.have.status(200)); pm.test('script header',()=>pm.expect(pm.response.json().headers.find(h=>h.key==='X-Script').value).to.equal('worked')); console.log('sandbox finished');",
    );
  await page.getByRole("button", { name: "发送", exact: false }).click();
  await page.locator(".pt-response-error").waitFor({ state: "hidden" });
  await page.getByRole("button", { name: "Test Results", exact: true }).click();
  await page.getByText("✓ status is 200").waitFor();
  await page.getByText("✓ script header").waitFor();
  await page.getByRole("button", { name: "Console", exact: true }).click();
  await page.getByText("sandbox finished", { exact: true }).waitFor();
  const divider = page.getByRole("separator", { name: "调整请求与响应区高度" });
  const oldHeight = await page
    .locator(".pt-editor-pane")
    .evaluate((el) => el.clientHeight);
  await divider.focus();
  await divider.press("ArrowDown");
  const newHeight = await page
    .locator(".pt-editor-pane")
    .evaluate((el) => el.clientHeight);
  if (newHeight <= oldHeight) throw Error("Keyboard panel resizing failed");
  await page.getByRole("button", { name: "Runner", exact: true }).click();
  await page.getByLabel("迭代次数", { exact: true }).fill("3");
  await page.getByRole("button", { name: "开始运行", exact: true }).click();
  await page.getByText(/已执行 3\/3/).waitFor();
  await page.getByRole("button", { name: "开始运行", exact: true }).waitFor();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "关闭", exact: true })
    .click();
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("HeapProfiler.collectGarbage");
  await cdp.send("Performance.enable");
  const beforeDOM = await cdp.send("Memory.getDOMCounters");
  const beforeMetrics = await cdp.send("Performance.getMetrics");
  const beforeHeap = beforeMetrics.metrics.find(
    (m) => m.name === "JSHeapUsedSize",
  ).value;
  await page.evaluate(async () => {
    for (let i = 0; i < 200; i++) {
      window.__posttoolActive.value = false;
      await new Promise((r) => setTimeout(r, 0));
      window.__posttoolActive.value = true;
      await new Promise((r) => setTimeout(r, 0));
    }
  });
  await cdp.send("HeapProfiler.collectGarbage");
  const afterDOM = await cdp.send("Memory.getDOMCounters");
  const afterMetrics = await cdp.send("Performance.getMetrics");
  const heapDelta =
    afterMetrics.metrics.find((m) => m.name === "JSHeapUsedSize").value -
    beforeHeap;
  if (afterDOM.jsEventListeners - beforeDOM.jsEventListeners > 10)
    throw Error("Event listener growth after 200 page transitions");
  if (afterDOM.nodes - beforeDOM.nodes > 100)
    throw Error("DOM node growth after 200 page transitions");
  if (heapDelta > 8 * 1024 * 1024)
    throw Error("Heap growth exceeded 8 MiB after 200 page transitions");
  await cdp.detach();
  await page.evaluate(() => {
    window.__posttoolActive.value = false;
  });
  if ((await page.locator(".pt-urlbar").count()) !== 0)
    throw Error("Hidden postTool retained its editor DOM");
  await page.evaluate(() => {
    window.__posttoolActive.value = true;
  });
  await page.getByLabel("请求 URL", { exact: true }).waitFor();
  await page.getByRole("button", { name: "Body", exact: true }).last().click();
  await page.getByRole("button", { name: "Scripts", exact: true }).click();
  await page.getByLabel("前置脚本", { exact: true }).fill("");
  await page.getByLabel("后置脚本", { exact: true }).fill("");
  await methodPicker.selectOption("POST");
  await page.getByLabel("请求名称", { exact: true }).fill("创建用户");
  await page
    .getByLabel("请求 URL", { exact: true })
    .fill("https://api.example.com/v1/users");
  await page
    .getByLabel("请求正文", { exact: true })
    .fill(
      '{\n  "name": "Lin",\n  "email": "lin@example.com",\n  "role": "developer"\n}',
    );
  await page.getByRole("button", { name: "发送", exact: true }).click();
  await page.getByText("201 Created", { exact: true }).waitFor();
  await page.getByRole("button", { name: "保存 Ctrl S" }).click();
  await page.screenshot({ path: ".pwtmp/posttool-ui.png", fullPage: true });
  await page.screenshot({
    path: ".pwtmp/posttool-redesign-dark.png",
    fullPage: true,
  });
  for (const [width, height] of [
    [1440, 960],
    [1280, 800],
    [1100, 640],
  ]) {
    await page.setViewportSize({ width, height });
    for (const theme of ["dark", "light"]) {
      await page.evaluate(
        (theme) => (document.documentElement.dataset.theme = theme),
        theme,
      );
      const layout = await page.evaluate(() => {
        const root = document.querySelector(".posttool"),
          url = document.querySelector(".pt-urlbar"),
          send = document.querySelector(".pt-send"),
          input = document.querySelector(".pt-url-input");
        return {
          overflow:
            root.scrollWidth > innerWidth || root.scrollHeight > innerHeight,
          urlHeight: url.getBoundingClientRect().height,
          sendBottom: send.getBoundingClientRect().bottom,
          inputWidth: input.getBoundingClientRect().width,
        };
      });
      if (
        layout.overflow ||
        layout.inputWidth < 240 ||
        layout.urlHeight > 64 ||
        layout.sendBottom > height
      )
        throw Error(
          `Invalid layout at ${width}x${height} ${theme}: ${JSON.stringify(layout)}`,
        );
      if (width === 1280)
        await page.screenshot({
          path: `.pwtmp/posttool-redesign-${theme}-1280.png`,
          fullPage: true,
        });
      if (width === 1100)
        await page.screenshot({
          path: `.pwtmp/posttool-redesign-${theme}-compact.png`,
          fullPage: true,
        });
    }
  }
  await page.setViewportSize({ width: 1440, height: 960 });
  await page.evaluate(() => (document.documentElement.dataset.theme = "dark"));
  const result = await page.evaluate(() => ({
    commands: window.__posttoolSmoke.calls.length,
    documents: window.__posttoolSmoke.documents.size,
    workers: performance
      .getEntriesByType("resource")
      .filter((r) => r.name.includes("worker")).length,
    overflow: document.documentElement.scrollWidth > innerWidth,
  }));
  if (result.overflow) throw Error("Viewport horizontal overflow");
  if (errors.length) throw Error(errors.join("\n"));
  return {
    ok: true,
    ...result,
    transitions: 200,
    heapDelta,
    listenerDelta: afterDOM.jsEventListeners - beforeDOM.jsEventListeners,
    nodeDelta: afterDOM.nodes - beforeDOM.nodes,
  };
};
