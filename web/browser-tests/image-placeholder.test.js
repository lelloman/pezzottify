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
import DetailPage from '@/components/common/DetailPage.vue';
import WorkArtwork from '@/components/common/WorkArtwork.vue';
import QueueCollage from '@/components/steering/QueueCollage.vue';
import SteeringArtwork from '@/components/steering/SteeringArtwork.vue';
import {useStaticsStore} from '@/store/statics';
const state=reactive({urls:['/art/first'],lazy:false,offset:0,mounted:true});
const fallbacks=new URLSearchParams(location.search).has('fallbacks');
const app=createApp({setup:()=>()=>fallbacks?h('div',[
  h(DetailPage,{title:'Artist',kind:'Artist',banner:true,imageUrls:state.urls}),
  h(WorkArtwork,{artistIds:['artist'],style:{width:'80px',height:'80px'}}),
  h(QueueCollage,{tracksIds:['album'],class:'singleQueue',style:{width:'80px'}}),
  h(QueueCollage,{tracksIds:['a','b','c','d'],class:'fourQueue',style:{width:'80px'}}),
  h(SteeringArtwork,{reference:{entity_type:'artist',entity_id:'artist',label:'Artist'}}),
]):state.mounted?h(MultiSourceImage,{urls:state.urls,lazy:state.lazy,alt:'Album cover',class:'artwork',style:{width:'120px',height:'120px',borderRadius:'50%',marginTop:state.offset+'px'}}):null});
app.use(createPinia());
if(fallbacks) useStaticsStore().getTrack=id=>({item:{album_id:id}});
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
            if (req.url.split("?")[0] !== "/") return next();
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

test("caller fallbacks remain visible while loading, on failure and when disabled; loaded artwork replaces them", async () => {
  const page = await browser.newPage({
    viewport: { width: 1000, height: 1000 },
  });
  page.setDefaultTimeout(10000);
  let release;
  const pending = new Promise((resolve) => {
    release = resolve;
  });
  try {
    await page.route("**/art/first", async (route) => {
      await pending;
      await route.fulfill({ status: 404 }).catch(() => {});
    });
    await page.route("**/v1/content/image/*", async (route) => {
      await pending;
      await route.fulfill({ status: 404 }).catch(() => {});
    });
    await page.goto(origin + "/?fallbacks", { waitUntil: "domcontentloaded" });
    const images = page.locator("img");
    assert.equal(await images.count(), 8);
    const banner = page.locator(".heroBackdrop");
    const work = page.locator(".portrait img");
    const steering = page.locator(".steeringArtwork img");
    const queue = page.locator(".queueCollage img");
    async function assertFallbacks(state) {
      await page.waitForFunction(
        (value) =>
          [...document.querySelectorAll("img")].every(
            (el) => el.dataset.imageState === value,
          ),
        state,
      );
      assert.equal(
        await banner.evaluate((el) => getComputedStyle(el).visibility),
        "hidden",
      );
      assert.match(
        await page
          .locator(".artistBanner")
          .evaluate((el) => getComputedStyle(el).backgroundImage),
        /linear-gradient/,
      );
      assert.equal(
        await work.evaluate((el) => getComputedStyle(el).visibility),
        "hidden",
      );
      assert.equal(await page.locator(".imageFallback").innerText(), "♪");
      assert.equal(await page.locator(".imageFallback").isVisible(), true);
      assert.equal(
        await steering.evaluate((el) => getComputedStyle(el).opacity),
        "0",
      );
      assert.equal(
        await page.locator(".steeringArtwork .fallback").innerText(),
        "A",
      );
      assert.equal(
        await page.locator(".steeringArtwork .fallback").isVisible(),
        true,
      );
      assert.deepEqual(
        await queue.evaluateAll((els) =>
          els.map((el) => getComputedStyle(el).opacity),
        ),
        ["0", "0", "0", "0", "0"],
      );
    }
    await assertFallbacks("loading");
    const bannerBox = await banner.boundingBox();
    release();
    await assertFallbacks("error");
    await page.evaluate(() => (window.fixture.debug.imagesEnabled = false));
    await assertFallbacks("disabled");
    await page.route("**/art/first", fulfillImage);
    await page.route("**/v1/content/image/*", fulfillImage);
    await page.evaluate(() => (window.fixture.debug.imagesEnabled = true));
    await page.waitForFunction(() =>
      [...document.querySelectorAll("img")].every(
        (el) => el.dataset.imageState === "loaded",
      ),
    );
    assert.equal(
      await banner.evaluate((el) => getComputedStyle(el).visibility),
      "visible",
    );
    assert.deepEqual(await banner.boundingBox(), bannerBox);
    assert.equal(
      await work.evaluate((el) => getComputedStyle(el).visibility),
      "visible",
    );
    assert.equal(
      await steering.evaluate((el) => getComputedStyle(el).opacity),
      "1",
    );
    assert.deepEqual(
      await queue.evaluateAll((els) =>
        els.map((el) => getComputedStyle(el).opacity),
      ),
      ["1", "1", "1", "1", "1"],
    );
  } finally {
    release();
    await page.close();
  }
});
