import assert from "node:assert/strict";
import { before, after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createServer } from "vite";
import vue from "@vitejs/plugin-vue";
import { chromium, expect } from "@playwright/test";

let server, browser, origin, cacheDir;
const root = fileURLToPath(new URL("..", import.meta.url));
before(async () => {
  cacheDir = await mkdtemp(join(tmpdir(), "pezzottify-cache-ui-"));
  server = await createServer({
    root,
    cacheDir,
    configFile: false,
    logLevel: "error",
    optimizeDeps: { noDiscovery: true, include: ["vue", "axios"] },
    plugins: [
      vue(),
      {
        name: "cache-fixture",
        resolveId(id) {
          if (id === "cache-fixture-entry") return "\0" + id;
        },
        load(id) {
          if (id === "\0cache-fixture-entry")
            return `import {createApp} from 'vue';import Cache from '/src/components/admin/MediaCacheStatus.vue';import '/src/assets/base.css';const app=createApp(Cache);app.mount('#app');window.unmount=()=>app.unmount();`;
        },
        configureServer(vite) {
          vite.middlewares.use((req, res, next) => {
            if (req.url !== "/cache-fixture") return next();
            res.setHeader("Content-Type", "text/html");
            res.end(
              '<!doctype html><html><head><meta name="viewport" content="width=device-width, initial-scale=1"></head><body style="margin:0;padding:16px;background:#121212;color:#fff"><div id="app"></div><script type="module" src="/@id/__x00__cache-fixture-entry"></script></body></html>',
            );
          });
        },
      },
    ],
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
const activity = {
  hits: 3,
  misses: 1,
  errors: 0,
  fallback_reads: 1,
  hit_ratio: 0.75,
};
const report = {
  enabled: true,
  caches: [
    {
      name: "ssd",
      path: "/cache/media",
      max_bytes: 50 * 1024 ** 3,
      allocated_bytes: 20 * 1024 ** 3,
      reserved_bytes: 5 * 1024 ** 3,
      lru: true,
      fills: 123,
      fill_errors: 2,
      evictions: 8,
      eviction_errors: 0,
    },
  ],
  audio: activity,
  image: { hits: 0, misses: 0, errors: 0, fallback_reads: 0, hit_ratio: null },
  fills_skipped_capacity: 4,
  fills_skipped_busy: 1,
};
async function fixture(body = report, status = 200) {
  const page = await browser.newPage({
    viewport: { width: 1100, height: 850 },
  });
  page.setDefaultTimeout(5000);
  await page.clock.install();
  const state = { body, status, requests: 0 };
  await page.route("**/v1/admin/media-cache", async (route) => {
    state.requests++;
    assert.equal(route.request().method(), "GET");
    await route.fulfill({ status: state.status, json: state.body });
  });
  await page.goto(origin + "/cache-fixture");
  return { page, state };
}
test("shows capacity, reservations and aggregate read activity on desktop and mobile", async () => {
  const { page } = await fixture();
  try {
    await expect(page.getByText("20 GiB used")).toBeVisible();
    await expect(page.getByText("50 GiB budget")).toBeVisible();
    await expect(page.getByRole("progressbar")).toHaveAttribute(
      "aria-valuenow",
      "50",
    );
    await expect(
      page.getByRole("row", { name: "Audio 75.0% 3 1 1 0" }),
    ).toBeVisible();
    await expect(
      page.getByRole("row", { name: "Artwork — 0 0 0 0" }),
    ).toBeVisible();
    await page.screenshot({
      path: "/tmp/pezzottify-cache-admin-desktop.png",
      fullPage: true,
    });
    await page.setViewportSize({ width: 390, height: 844 });
    assert(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    );
    await page.screenshot({
      path: "/tmp/pezzottify-cache-admin-mobile.png",
      fullPage: true,
    });
  } finally {
    await page.close();
  }
});
test("refresh failures retain a clearly stale snapshot and can recover", async () => {
  const { page, state } = await fixture();
  try {
    await expect(page.getByText("20 GiB used")).toBeVisible();
    state.status = 503;
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText(
      "last successful snapshot",
    );
    await expect(page.getByText("20 GiB used")).toBeVisible();
    state.status = 200;
    state.body = {
      ...report,
      caches: [{ ...report.caches[0], allocated_bytes: 21 * 1024 ** 3 }],
    };
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByText("21 GiB used")).toBeVisible();
    await expect(page.getByRole("alert")).toHaveCount(0);
  } finally {
    await page.close();
  }
});
test("disabled cache and initial failure have distinct states", async () => {
  const { page, state } = await fixture({}, 500);
  try {
    await expect(page.getByRole("alert")).toContainText("Could not load");
    state.status = 200;
    state.body = { ...report, enabled: false, caches: [] };
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(
      page.getByText("Not configured", { exact: true }),
    ).toBeVisible();
    await expect(page.getByRole("table")).toHaveCount(0);
  } finally {
    await page.close();
  }
});
test("automatic refresh stops after unmount and when permission is lost", async () => {
  const { page, state } = await fixture();
  try {
    await expect(page.getByText("20 GiB used")).toBeVisible();
    await page.clock.fastForward(15000);
    await expect.poll(() => state.requests).toBe(2);
    await expect(
      page.getByRole("button", { name: "Refresh", exact: true }),
    ).toBeEnabled();
    await page.evaluate(() => {
      Object.defineProperty(document, "hidden", {
        configurable: true,
        get: () => true,
      });
      document.dispatchEvent(new Event("visibilitychange"));
    });
    await page.clock.fastForward(30000);
    assert.equal(state.requests, 2);
    await page.evaluate(() => {
      Object.defineProperty(document, "hidden", {
        configurable: true,
        get: () => false,
      });
      document.dispatchEvent(new Event("visibilitychange"));
    });
    await expect.poll(() => state.requests).toBe(3);
    state.status = 403;
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText(
      "permission is required",
    );
    await expect(page.getByText("20 GiB used")).toHaveCount(0);
    const previous = state.requests;
    await page.clock.fastForward(30000);
    assert.equal(state.requests, previous);
    state.status = 200;
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByText("20 GiB used")).toBeVisible();
    await page.evaluate(() => window.unmount());
    const atUnmount = state.requests;
    await page.clock.fastForward(30000);
    assert.equal(state.requests, atUnmount);
  } finally {
    await page.close();
  }
});
