import assert from "node:assert/strict";
import { before, after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createServer } from "vite";
import vue from "@vitejs/plugin-vue";
import { chromium } from "@playwright/test";
let server, browser, origin, cacheDir;
const root = fileURLToPath(new URL("..", import.meta.url));
const modules = {
  "test-user": `import {reactive} from 'vue';export const user=reactive({permissions:[],hasPermission(p){return this.permissions.includes(p)}});export const useUserStore=()=>user;`,
  "test-auth": `import {reactive} from 'vue';export const auth=reactive({user:{handle:'operator'}});export const useAuthStore=()=>auth;`,
  "test-entry": `import {createApp} from 'vue';import Inbox from '@/components/admin/BugReports.vue';import {user} from 'test-user';import {auth} from 'test-auth';user.permissions=JSON.parse(new URLSearchParams(location.search).get('permissions'));window.fixture={user,auth};createApp(Inbox).mount('#app');`,
};
before(async () => {
  cacheDir = await mkdtemp(join(tmpdir(), "pezzottify-admin-vite-"));
  server = await createServer({
    root,
    cacheDir,
    configFile: false,
    optimizeDeps: { noDiscovery: true, include: ["vue", "axios"] },
    logLevel: "error",
    plugins: [
      vue(),
      {
        name: "admin-fixture",
        resolveId(id) {
          if (Object.hasOwn(modules, id)) return "\0" + id;
        },
        load(id) {
          if (id.startsWith("\0")) return modules[id.slice(1)];
        },
        configureServer(vite) {
          vite.middlewares.use((req, res, next) => {
            if (!req.url.startsWith("/?")) return next();
            res.setHeader("Content-Type", "text/html");
            res.end(
              '<!doctype html><html><body><div id="app"></div><script type="module" src="/@id/__x00__test-entry"></script></body></html>',
            );
          });
        },
      },
    ],
    resolve: {
      alias: [
        { find: "@/store/user", replacement: "test-user" },
        { find: "@/store/auth", replacement: "test-auth" },
        { find: "@", replacement: root + "/src" },
      ],
    },
    server: { host: "127.0.0.1", port: 0 },
  });
  await server.listen();
  origin = `http://127.0.0.1:${server.httpServer.address().port}`;
  browser = await chromium.launch();
});
after(async () => {
  await browser?.close();
  await server?.close();
  if (cacheDir) await rm(cacheDir, { recursive: true, force: true });
});
const report = {
  id: "report-1",
  title: "<img src=x onerror=alert(1)>",
  kind: "feature",
  category: "ui",
  status: "new",
  version: 3,
  description: "<script>window.executed=true</script>",
  user_handle: "alice",
  client_type: "web",
  duplicate_of: null,
  attachments: [
    {
      id: "attachment-1",
      kind: "assistant",
      size_bytes: 90,
      state: "active",
      created_at: 1,
      expires_at: 2,
    },
  ],
};
async function fixture(permissions) {
  const page = await browser.newPage();
  page.setDefaultTimeout(5000);
  const requests = [];
  await page.route("**/v1/admin/reports**", async (route) => {
    const req = route.request(),
      url = new URL(req.url()),
      path = url.pathname.replace("/v1/admin/reports", "");
    requests.push({
      path,
      method: req.method(),
      body: req.postDataJSON(),
      query: Object.fromEntries(url.searchParams),
    });
    let value;
    if (path === "") value = { items: [report], next_cursor: 42 };
    else if (path === "/report-1" && req.method() === "GET") value = report;
    else if (path === "/report-1" && req.method() === "PATCH")
      return route.fulfill({
        status: 409,
        json: { error: { code: "conflict" } },
      });
    else if (path.endsWith("/notes"))
      value = {
        items: [
          {
            id: "note",
            actor_id: 1,
            created_at: 1,
            data: { text: "private note" },
          },
        ],
        next_cursor: null,
      };
    else if (path.includes("/attachments/"))
      value = {
        attachment: report.attachments[0],
        content: JSON.stringify({
          schema_version: 1,
          turns: [
            {
              id: "turn",
              status: "failed",
              events: [
                {
                  kind: "tool_call",
                  content: "<img src=x onerror=alert(1)>",
                  data: { command: "never execute" },
                },
              ],
            },
          ],
        }),
      };
    else if (path.endsWith("/events")) value = { items: [], next_cursor: null };
    else if (path === "/settings")
      value = {
        version: 4,
        hourly_reports: 5,
        daily_reports: 20,
        daily_attachment_bytes: 33554432,
        user_reports: 1000,
        total_reports: 100000,
        metadata_bytes: 268435456,
        diagnostic_bytes: 524288000,
        retention_days: 30,
      };
    else if (path === "/integrations")
      value = [
        {
          id: "integration-1",
          url: "https://example.com/hook",
          enabled: true,
          version: 2,
        },
      ];
    else if (path === "/deliveries")
      value = {
        items: [
          {
            id: "delivery-1",
            state: "failed",
            manual_retries: 0,
            attempts: 5,
            version: 7,
            outcome: "deadline",
          },
        ],
        next_cursor: null,
      };
    else value = null;
    await route.fulfill({ json: value });
  });
  await page.goto(
    origin + "/?permissions=" + encodeURIComponent(JSON.stringify(permissions)),
  );
  await page.getByRole("heading", { name: "Feedback Reports" }).waitFor();
  return { page, requests };
}
test("triage alone can filter, paginate and read notes; conflict preserves draft and sends expected version", async () => {
  const { page, requests } = await fixture(["TriageReports"]);
  await page.getByLabel("Kind", { exact: true }).selectOption("feature");
  await page
    .getByRole("button", { name: "Apply filters / Refresh inbox" })
    .click();
  await page.getByRole("button", { name: "Next inbox page" }).click();
  await page.getByRole("button", { name: /report-1/ }).click();
  await page
    .getByRole("button", { name: "Refresh notes", exact: true })
    .click();
  await page.getByText("private note", { exact: true }).waitFor();
  assert.equal(
    await page.getByRole("button", { name: "View assistant" }).count(),
    0,
  );
  assert.equal(
    await page.getByRole("heading", { name: "Automation controls" }).count(),
    0,
  );
  await page.getByLabel("Internal note", { exact: true }).fill("retain draft");
  await page.getByRole("button", { name: "Save triage" }).click();
  await page.getByRole("alert").waitFor();
  assert.match(await page.getByRole("alert").innerText(), /Refresh/);
  assert.equal(
    await page.getByLabel("Internal note", { exact: true }).inputValue(),
    "retain draft",
  );
  assert.equal(
    requests.find((r) => r.method === "PATCH").body.expected_version,
    3,
  );
  assert(
    requests.some((r) => r.query.before === "42" && r.query.kind === "feature"),
  );
  assert(!requests.some((r) => /attachments|events|integrations/.test(r.path)));
  assert.equal(await page.locator("#app img, #app script").count(), 0);
  assert.equal(await page.evaluate(() => window.executed), undefined);
  await page.close();
});
test("diagnostic-only access renders inert structured tool text and clears it on account change", async () => {
  const { page, requests } = await fixture(["ViewReportDiagnostics"]);
  assert.equal(
    await page.getByRole("heading", { name: "Inbox", exact: true }).count(),
    0,
  );
  await page.getByLabel("Report ID", { exact: true }).fill("report-1");
  await page.getByLabel("Attachment ID", { exact: true }).fill("attachment-1");
  await page.getByRole("button", { name: "View attachment by ID" }).click();
  await page.getByText(/Turn turn/).click();
  await page.getByText(/never execute/).waitFor();
  assert.equal(await page.locator("#app img, #app script").count(), 0);
  await page.evaluate(() => {
    window.fixture.auth.user = { handle: "other" };
  });
  await page.getByText(/never execute/).waitFor({ state: "detached" });
  assert(!requests.some((r) => r.path === ""));
  await page.close();
});
test("integration-only operators confirm versioned disable and bounded manual retry", async () => {
  const { page, requests } = await fixture(["ManageReportIntegrations"]);
  await page
    .getByRole("button", { name: "Disable integration", exact: true })
    .click();
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
  await page.getByRole("status").waitFor();
  assert.deepEqual(requests.find((r) => r.method === "PATCH").body, {
    expected_version: 2,
    enabled: false,
  });
  await page
    .getByRole("button", { name: "Refresh deliveries", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Retry delivery", exact: true })
    .click();
  await page.getByRole("button", { name: "Retry once", exact: true }).click();
  await page.getByRole("status").waitFor();
  assert.deepEqual(requests.find((r) => r.path.endsWith("/retry")).body, {
    expected_version: 7,
  });
  assert(!requests.some((r) => r.path === "" || r.path.endsWith("/events")));
  await page.close();
});
test("attachment deletion needs confirmation and preserves report metadata", async () => {
  const { page, requests } = await fixture(["ViewReportDiagnostics"]);
  await page.getByLabel("Report ID", { exact: true }).fill("report-1");
  await page.getByLabel("Attachment ID", { exact: true }).fill("attachment-1");
  await page.getByRole("button", { name: "Delete attachment by ID" }).click();
  await page.getByRole("dialog").waitFor();
  assert.equal(
    await page.evaluate(() => document.activeElement.textContent),
    "Cancel",
  );
  await page.keyboard.press("Escape");
  await page.getByRole("dialog").waitFor({ state: "hidden" });
  assert(!requests.some((r) => r.method === "DELETE"));
  await page.getByRole("button", { name: "Delete attachment by ID" }).click();
  await page
    .getByRole("button", { name: "Delete attachment", exact: true })
    .click();
  await page.getByRole("status").waitFor();
  assert.equal(requests.filter((r) => r.method === "DELETE").length, 1);
  assert.equal(
    requests.find((r) => r.method === "DELETE").path,
    "/report-1/attachments/attachment-1",
  );
  await page.close();
});
test("late sensitive responses cannot repopulate a changed account", async () => {
  const { page } = await fixture(["ViewReportDiagnostics"]);
  let deliver;
  const received = new Promise((resolve) => {
    deliver = resolve;
  });
  let release;
  const pending = new Promise((resolve) => {
    release = resolve;
  });
  await page.route(
    "**/v1/admin/reports/report-1/attachments/attachment-1",
    async (route) => {
      deliver();
      await pending;
      await route.fulfill({
        json: {
          attachment: { kind: "technical_logs" },
          content: "sensitive late response",
        },
      });
    },
  );
  await page.getByLabel("Report ID", { exact: true }).fill("report-1");
  await page.getByLabel("Attachment ID", { exact: true }).fill("attachment-1");
  await page.getByRole("button", { name: "View attachment by ID" }).click();
  await received;
  await page.evaluate(() => {
    window.fixture.auth.user = { handle: "replacement" };
  });
  const completed = page.waitForResponse((response) =>
    response.url().includes("/attachments/attachment-1"),
  );
  release();
  await completed;
  await page.evaluate(
    () =>
      new Promise((resolve) =>
        window.requestAnimationFrame(() =>
          window.requestAnimationFrame(resolve),
        ),
      ),
  );
  assert.equal(
    await page.getByText("sensitive late response", { exact: true }).count(),
    0,
  );
  await page.close();
});

test("retention changes are versioned and require confirmation", async () => {
  const { page, requests } = await fixture(["ManageReportIntegrations"]);
  await page
    .getByRole("button", { name: "Refresh settings", exact: true })
    .click();
  await page.getByLabel("retention days", { exact: true }).fill("7");
  await page
    .getByRole("button", { name: "Save limits and retention", exact: true })
    .click();
  await page.getByRole("dialog").waitFor();
  assert(!requests.some((r) => r.method === "PUT"));
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await page.getByRole("status").waitFor();
  const body = requests.find((r) => r.method === "PUT").body;
  assert.equal(body.version, 4);
  assert.equal(body.retention_days, 7);
  assert.equal(body.total_reports, 100000);
  await page.close();
});
