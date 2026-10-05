import assert from "node:assert/strict";
import { before, after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import vue from "@vitejs/plugin-vue";
import { chromium } from "@playwright/test";
let server, browser, origin;
const root = fileURLToPath(new URL("..", import.meta.url));
const modules = {
  "test-playback": `import {reactive} from 'vue';
    export const playback = reactive({currentTrackId:'one',currentTrack:{title:'Test song',artistName:'Test artist',albumTitle:'Test album',duration:60000},progressSec:2,progressPercent:.033,isPlaying:false,mode:'local',seeks:[],seekToPercentage(v){this.seeks.push(v)},playPause(){this.isPlaying=!this.isPlaying},skipPreviousTrack(){},skipNextTrack(){}});
    export const usePlaybackStore=()=>playback;`,
  "test-remote": `export const useRemoteStore=()=>({async getTrackLyrics(id,signal){const r=await fetch('/test-lyrics/'+id,{signal});if(!r.ok)throw Error('failed');return r.json()},async downloadLyrics(type,id){const r=await fetch('/test-download/'+id,{method:'POST'});if(!r.ok)throw Error('failed');return r.json()}});`,
  "test-image": `import {h} from 'vue';export default {props:['urls'],setup:props=>()=>h('img',{src:props.urls[0]})};`,
  "test-entry": `import "@/assets/main.css";import {createApp,ref,h} from 'vue';import NowPlaying from '@/components/NowPlaying.vue';import TrackLyrics from '@/components/common/TrackLyrics.vue';import {playback} from 'test-playback';const open=ref(false),track=ref('one');window.testState={playback,track,open};createApp({setup:()=>()=>[h('button',{onClick:()=>open.value=true},'Expand player'),open.value?h(NowPlaying,{onClose:()=>open.value=false}):h(TrackLyrics,{trackId:track.value})]}).mount('#app');`,
};
before(async () => {
  server = await createServer({
    root,
    configFile: false,
    optimizeDeps: { noDiscovery: true, include: ["vue"] },
    logLevel: "error",
    plugins: [
      vue(),
      {
        name: "lyrics-fixture",
        resolveId(id) {
          if (Object.hasOwn(modules, id)) return "\0" + id;
        },
        load(id) {
          if (id.startsWith("\0")) return modules[id.slice(1)];
        },
        configureServer(vite) {
          vite.middlewares.use((req, res, next) => {
            if (req.url !== "/") return next();
            res.setHeader("Content-Type", "text/html");
            res.end(
              '<!doctype html><html><head><meta name="viewport" content="width=device-width, initial-scale=1"></head><body><div id="app"></div><script type="module" src="/@id/__x00__test-entry"></script></body></html>',
            );
          });
        },
      },
    ],
    resolve: {
      alias: [
        { find: "@/store/playback", replacement: "test-playback" },
        { find: "@/store/remote", replacement: "test-remote" },
        {
          find: "@/components/common/MultiSourceImage.vue",
          replacement: "test-image",
        },
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
const found = {
  status: "found",
  plain_lyrics: "First line\nSecond line",
  synced_lyrics: "[00:01.00]First line\n[00:10.00]Second line",
  fetched_at: 1,
};
async function pageWith(data) {
  const page = await browser.newPage();
  page.setDefaultTimeout(10000);
  await page.route("**/test-lyrics/*", (r) => r.fulfill({ json: data }));
  await page.goto(origin);
  return page;
}
test("timed lyrics seek, playback controls, Escape and focus restoration", async () => {
  const page = await pageWith(found);
  try {
    const first = page.getByRole("button", {
      name: "Seek to 0:01: First line",
    });
    await first.waitFor();
    assert.equal(await first.getAttribute("aria-current"), "true");
    await page
      .getByRole("button", { name: "Seek to 0:10: Second line" })
      .click();
    assert.equal(
      await page.evaluate(() => window.testState.playback.seeks[0]),
      1 / 6,
    );
    await page.getByRole("button", { name: "Expand player" }).click();
    await page.getByRole("dialog").waitFor();
    await page.getByRole("button", { name: "Play", exact: true }).click();
    await page.getByRole("button", { name: "Pause", exact: true }).waitFor();
    await page.keyboard.press("Escape");
    await page.getByRole("dialog").waitFor({ state: "detached" });
    assert.equal(
      await page
        .getByRole("button", { name: "Expand player" })
        .evaluate((el) => el === document.activeElement),
      true,
    );
    await page.evaluate(() => {
      window.testState.track.value = "other";
    });
    await page.waitForFunction(() => document.querySelector(".timedLyrics p"));
    assert.equal(await page.locator(".timedLyrics button").count(), 0);
  } finally {
    await page.close();
  }
});
test("download missing lyrics and render plain text without HTML", async () => {
  const page = await browser.newPage();
  page.setDefaultTimeout(10000);
  let requested = false;
  await page.route("**/test-lyrics/*", (r) =>
    r.fulfill({
      json: requested
        ? {
            status: "found",
            plain_lyrics: "<b>Plain text</b>\nNext line",
            fetched_at: 2,
          }
        : null,
    }),
  );
  await page.route("**/test-download/*", (r) => {
    requested = true;
    return r.fulfill({ json: { tracks: 1 } });
  });
  try {
    await page.goto(origin);
    await page.getByRole("button", { name: "Find lyrics" }).click();
    await page.locator(".plainLyrics").waitFor();
    assert.equal(
      await page.locator(".plainLyrics").innerText(),
      "<b>Plain text</b>\nNext line",
    );
    assert.equal(await page.locator(".plainLyrics b").count(), 0);
  } finally {
    await page.close();
  }
});
test("instrumental, missing and provider failure messages", async () => {
  for (const [status, text] of [
    ["instrumental", "This track is instrumental."],
    ["not_found", "No lyrics were found"],
    ["error", "The lyrics provider could not be reached"],
  ]) {
    const page = await pageWith({ status });
    try {
      await page.getByText(text, { exact: false }).waitFor();
    } finally {
      await page.close();
    }
  }
});
test("full-screen player fits mobile and desktop viewports", async () => {
  const page = await pageWith(found);
  try {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.getByRole("button", { name: "Expand player" }).click();
    await page.getByRole("dialog").waitFor();
    const bounds = await page.getByRole("dialog").boundingBox();
    assert.equal(bounds.width, 390);
    assert.equal(bounds.height, 844);
    assert.equal(
      await page
        .locator(".nowPlayingBody")
        .evaluate((el) => el.scrollWidth <= el.clientWidth),
      true,
    );
    await page.screenshot({
      path: "/tmp/pezzottify-lyrics-mobile.png",
      fullPage: true,
    });
    await page.setViewportSize({ width: 1280, height: 900 });
    await page.screenshot({
      path: "/tmp/pezzottify-lyrics-desktop.png",
      fullPage: true,
    });
  } finally {
    await page.close();
  }
});

test("switching tracks during a download cannot replace the new track lyrics", async () => {
  const page = await browser.newPage();
  page.setDefaultTimeout(10000);
  let finishDownload;
  const pending = new Promise((resolve) => {
    finishDownload = resolve;
  });
  await page.route("**/test-lyrics/*", (r) =>
    r.fulfill({
      json: r.request().url().endsWith("/other")
        ? { status: "found", plain_lyrics: "Other track lyrics", fetched_at: 3 }
        : null,
    }),
  );
  await page.route("**/test-download/*", async (r) => {
    await pending;
    await r.fulfill({ json: { tracks: 1 } });
  });
  try {
    await page.goto(origin);
    const started = page.waitForRequest("**/test-download/one");
    await page.getByRole("button", { name: "Find lyrics" }).click();
    await started;
    await page.evaluate(() => {
      window.testState.track.value = "other";
    });
    await page.getByText("Other track lyrics", { exact: true }).waitFor();
    finishDownload();
    await page.waitForLoadState("networkidle");
    assert.equal(
      await page.locator(".plainLyrics").innerText(),
      "Other track lyrics",
    );
  } finally {
    finishDownload();
    await page.close();
  }
});

test("read and download failures expose retry actions", async () => {
  const page = await browser.newPage();
  page.setDefaultTimeout(10000);
  await page.route("**/test-lyrics/*", (r) => r.fulfill({ status: 503 }));
  await page.route("**/test-download/*", (r) => r.fulfill({ status: 503 }));
  try {
    await page.goto(origin);
    await page
      .getByText("Could not load lyrics. Check again to retry.")
      .waitFor();
    await page.getByRole("button", { name: "Find lyrics" }).click();
    await page
      .getByText("Could not start the lyrics download. Try again later.")
      .waitFor();
    assert.equal(
      await page.getByRole("button", { name: "Check again" }).isEnabled(),
      true,
    );
  } finally {
    await page.close();
  }
});
