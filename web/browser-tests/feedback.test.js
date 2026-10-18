import assert from "node:assert/strict";
import { before, after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import vue from "@vitejs/plugin-vue";
import { chromium } from "@playwright/test";
let server, browser, origin;
const root = fileURLToPath(new URL("..", import.meta.url));
const entry = `import {createApp,h} from 'vue';import {createPinia} from 'pinia';import Settings from '@/components/feedback/FeedbackSettings.vue';import Form from '@/components/feedback/FeedbackForm.vue';import {useFeedbackStore} from '@/store/feedback';const app=createApp({render:()=>[h(Settings),h(Form)]});app.use(createPinia());const feedback=useFeedbackStore();window.feedback=feedback;await feedback.account('A');app.mount('#app');`;
before(async () => {
  server = await createServer({
    root,
    configFile: false,
    cacheDir: "node_modules/.vite-feedback-tests",
    optimizeDeps: { noDiscovery: true, include: ["vue", "pinia"] },
    logLevel: "error",
    plugins: [
      vue(),
      {
        name: "feedback-fixture",
        resolveId(id) {
          if (id === "test-entry" || id === "test-oidc") return "\0" + id;
        },
        load(id) {
          if (id === "\0test-entry") return entry;
          if (id === "\0test-oidc")
            return `export async function getIdToken(){return 'account-A';}`;
        },
        configureServer(vite) {
          vite.middlewares.use((req, res, next) => {
            if (req.url !== "/") return next();
            res.setHeader("Content-Type", "text/html");
            res.end(
              '<!doctype html><div id="app"></div><script type="module" src="/@id/__x00__test-entry"></script>',
            );
          });
        },
      },
    ],
    resolve: {
      alias: [
        { find: /.*\/services\/oidc$/, replacement: "test-oidc" },
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
});
test(
  "consent starts off, preview is immutable through manual retry, and owner status is visible",
  { timeout: 20000 },
  async () => {
    const page = await browser.newPage();
    page.setDefaultTimeout(5000);
    const bodies = [];
    try {
      await page.route("**/v1/auth/session", (r) =>
        r.fulfill({ json: { user_handle: "A" } }),
      );
      await page.route("**/v1/reports", (r) => {
        if (r.request().method() === "GET")
          return r.fulfill({
            json: {
              items: [
                { id: "report-id", kind: "bug", status: "investigating" },
              ],
              next_cursor: null,
            },
          });
        bodies.push(r.request().postData());
        return r.fulfill(
          bodies.length === 1
            ? {
                status: 503,
                headers: { "Retry-After": "60" },
                json: { error: { code: "capacity_exceeded" } },
              }
            : { status: 201, json: { id: "report-id", version: 1 } },
        );
      });
      await page.goto(origin);
      await page
        .getByRole("button", { name: "Report a bug or request a feature" })
        .click();
      assert.equal(
        await page
          .getByRole("checkbox", { name: /Attach selected/ })
          .isChecked(),
        false,
      );
      assert.equal(
        await page
          .getByRole("checkbox", { name: /Attach technical/ })
          .isChecked(),
        false,
      );
      await page
        .getByLabel("Description", { exact: true })
        .fill("A response failed 🌍");
      await page.getByRole("button", { name: "Review before sending" }).click();
      const preview = await page.locator("pre").textContent();
      await page
        .getByRole("button", { name: "Send / retry this exact report" })
        .click();
      await page.getByRole("alert").filter({ hasText: "capacity" }).waitFor();
      await page
        .getByRole("button", { name: "Send / retry this exact report" })
        .click();
      await page.getByRole("status").filter({ hasText: "report-id" }).waitFor();
      assert.deepEqual(bodies, [preview, preview]);
      await page.getByRole("button", { name: "Refresh status" }).click();
      await page.getByText("report-id — bug — investigating").waitFor();
    } finally {
      await page.close();
    }
  },
);
