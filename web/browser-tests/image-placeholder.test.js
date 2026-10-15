import assert from "node:assert/strict";
import { before, after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import vue from "@vitejs/plugin-vue";
import { chromium } from "@playwright/test";

let server, browser, origin;
const root = fileURLToPath(new URL("..", import.meta.url));
const entry = `import {createApp,h,reactive} from 'vue';
import {createPinia} from 'pinia';
import MultiSourceImage from '@/components/common/MultiSourceImage.vue';
import {useDebugStore} from '@/store/debug';
const state=reactive({urls:['/art/first'],lazy:false,offset:0,mounted:true});
const app=createApp({setup:()=>()=>state.mounted?h(MultiSourceImage,{urls:state.urls,lazy:state.lazy,alt:'Album cover',class:'artwork',style:{width:'120px',height:'120px',borderRadius:'50%',marginTop:state.offset+'px'}}):null});
app.use(createPinia());
window.fixture={state,debug:useDebugStore(),rerender:()=>state.urls=[...state.urls]};
app.mount('#app');`;
before(async () => {
  server = await createServer({
    root,
    configFile: false,
    cacheDir: "node_modules/.vite-image-placeholder-tests",
    optimizeDeps: { noDiscovery: true, include: ["vue", "pinia"] },
    logLevel: "error",
    plugins: [
      vue(),
      {
        name: "image-placeholder-fixture",
        resolveId(id) {
          if (id === "test-entry") return "\0test-entry";
        },
        load(id) {
          if (id === "\0test-entry") return entry;
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
    resolve: { alias: { "@": root + "/src" } },
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
const svg =
  '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect width="100" height="100" fill="red"/></svg>';
const fulfillImage = (route) =>
  route.fulfill({ contentType: "image/svg+xml", body: svg });
async function stateIs(page, state) {
  await page.waitForFunction(
    (value) => document.querySelector("img")?.dataset.imageState === value,
    state,
  );
}

test("shimmer survives retries, respects reduced motion, then displays artwork without layout changes", async () => {
  const page = await browser.newPage();
  page.setDefaultTimeout(10000);
  try {
    let release;
    let attempts = 0;
    const pending = new Promise((resolve) => {
      release = resolve;
    });
    await page.route("**/art/first", async (route) => {
      if (++attempts === 1) return route.fulfill({ status: 503 });
      await pending;
      await fulfillImage(route);
    });
    await page.goto(origin, { waitUntil: "domcontentloaded" });
    await stateIs(page, "loading");
    const image = page.locator("img.artwork");
    const box = await image.boundingBox();
    assert.equal(await image.getAttribute("aria-busy"), "true");
    assert.notEqual(
      await image.evaluate((el) => getComputedStyle(el).animationName),
      "none",
    );
    await page.emulateMedia({ reducedMotion: "reduce" });
    assert.equal(
      await image.evaluate((el) => getComputedStyle(el).animationName),
      "none",
    );
    await page.evaluate(() => window.fixture.rerender());
    release();
    await stateIs(page, "loaded");
    assert.equal(attempts, 2);
    assert.deepEqual(await image.boundingBox(), box);
    assert.equal(await image.getAttribute("alt"), "Album cover");
    assert.equal(await image.getAttribute("aria-busy"), "false");
    assert.equal(
      await image.evaluate((el) => getComputedStyle(el).animationName),
      "none",
    );
    assert.equal(
      await image.evaluate((el) => getComputedStyle(el).borderRadius),
      "50%",
    );
  } finally {
    await page.close();
  }
});

test("HTTP and decode failures advance sources, exhausted sources stay in error, new URLs recover", async () => {
  const page = await browser.newPage();
  page.setDefaultTimeout(10000);
  const requests = [];
  try {
    await page.route("**/art/*", (route) => {
      requests.push(route.request().url().split("/").at(-1));
      if (route.request().url().endsWith("/good")) return fulfillImage(route);
      return route.fulfill({
        status: route.request().url().endsWith("/corrupt") ? 200 : 404,
        body: "invalid image",
      });
    });
    await page.goto(origin);
    await stateIs(page, "error");
    await page.evaluate(
      () =>
        (window.fixture.state.urls = [
          "/art/missing",
          "/art/corrupt",
          "/art/good",
        ]),
    );
    await stateIs(page, "loaded");
    assert.deepEqual(requests, ["first", "missing", "corrupt", "good"]);
    await page.evaluate(() => (window.fixture.state.urls = ["/art/corrupt"]));
    await stateIs(page, "error");
    const image = page.locator("img");
    assert.equal(await image.getAttribute("alt"), "Album cover unavailable");
    await image.evaluate((el) => el.decode());
    assert.equal(
      await image.evaluate((el) => el.complete && el.naturalWidth > 0),
      true,
    );
    assert.equal(
      await image.evaluate((el) => getComputedStyle(el).animationName),
      "none",
    );
    const requestCount = requests.length;
    await page.evaluate(() => window.fixture.rerender());
    assert.equal(requests.length, requestCount);
    await page.evaluate(() => (window.fixture.state.urls = []));
    await stateIs(page, "error");
    await page.evaluate(() => (window.fixture.state.urls = ["/art/good"]));
    await stateIs(page, "loaded");
  } finally {
    await page.close();
  }
});

test("offscreen and disabled images do not shimmer or fetch; cancelled work cannot overwrite a new image", async () => {
  const page = await browser.newPage();
  page.setDefaultTimeout(10000);
  try {
    await page.addInitScript(() =>
      localStorage.setItem("imagesEnabled", "false"),
    );
    let requests = 0;
    await page.route("**/art/*", async (route) => {
      requests++;
      await fulfillImage(route);
    });
    await page.goto(origin);
    await stateIs(page, "disabled");
    assert.equal(requests, 0);
    await page.evaluate(() => {
      window.fixture.state.lazy = true;
      window.fixture.state.offset = 2000;
    });
    // Remount so IntersectionObserver sees the image in its offscreen position.
    await page.evaluate(() => (window.fixture.state.mounted = false));
    await page.evaluate(() => (window.fixture.state.mounted = true));
    await page.evaluate(() => (window.fixture.debug.imagesEnabled = true));
    await stateIs(page, "idle");
    assert.equal(requests, 0);
    await page.locator("img").scrollIntoViewIfNeeded();
    await stateIs(page, "loaded");
    assert.equal(requests, 1);
    let release;
    const pending = new Promise((resolve) => {
      release = resolve;
    });
    await page.route("**/art/slow", async (route) => {
      await pending;
      await fulfillImage(route).catch(() => {});
    });
    const started = page.waitForRequest("**/art/slow");
    await page.evaluate(() => (window.fixture.state.urls = ["/art/slow"]));
    await started;
    await stateIs(page, "loading");
    await page.evaluate(() => (window.fixture.state.urls = ["/art/new"]));
    await stateIs(page, "loaded");
    const src = await page.locator("img").getAttribute("src");
    release();
    await page.waitForLoadState("networkidle");
    assert.equal(await page.locator("img").getAttribute("src"), src);
    await page.evaluate(() => (window.fixture.debug.imagesEnabled = false));
    await stateIs(page, "disabled");
    assert.equal(
      await page
        .locator("img")
        .evaluate((el) => getComputedStyle(el).animationName),
      "none",
    );
  } finally {
    await page.close();
  }
});

test("exhausted transient retries stop shimmering and remain a stable error", async () => {
  const page = await browser.newPage();
  page.setDefaultTimeout(10000);
  try {
    let attempts = 0;
    await page.route("**/art/first", (route) => {
      attempts++;
      return route.fulfill({ status: 503 });
    });
    await page.goto(origin, { waitUntil: "domcontentloaded" });
    await stateIs(page, "loading");
    await stateIs(page, "error");
    assert.equal(attempts, 4);
    await page.evaluate(() => window.fixture.rerender());
    await page.waitForLoadState("networkidle");
    assert.equal(attempts, 4);
    assert.equal(await page.locator("img").getAttribute("aria-busy"), "false");
  } finally {
    await page.close();
  }
});
