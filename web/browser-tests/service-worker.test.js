import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { after, before, test } from "node:test";
import { URL } from "node:url";
import { chromium } from "@playwright/test";

let browser;
let server;
let origin;
const requests = [];
// 100 ms of mono PCM silence: enough for Chromium to decode real audio.
const audio = Buffer.alloc(1644);
audio.write("RIFF", 0);
audio.writeUInt32LE(audio.length - 8, 4);
audio.write("WAVEfmt ", 8);
audio.writeUInt32LE(16, 16);
audio.writeUInt16LE(1, 20);
audio.writeUInt16LE(1, 22);
audio.writeUInt32LE(8000, 24);
audio.writeUInt32LE(16000, 28);
audio.writeUInt16LE(2, 32);
audio.writeUInt16LE(16, 34);
audio.write("data", 36);
audio.writeUInt32LE(audio.length - 44, 40);

before(async () => {
  const worker = await readFile(new URL("../public/sw.js", import.meta.url));
  server = createServer((req, res) => {
    if (req.url === "/sw.js") {
      res.writeHead(200, { "Content-Type": "application/javascript" });
      res.end(worker);
    } else if (req.url.startsWith("/v1/content/")) {
      requests.push({ authorization: req.headers.authorization, range: req.headers.range });
      if (req.headers.authorization !== "Bearer fresh") {
        res.writeHead(401);
        res.end();
      } else {
        res.writeHead(206, {
          "Content-Type": "audio/wav",
          "Content-Range": `bytes 0-${audio.length - 1}/${audio.length}`,
          "Content-Length": audio.length,
        });
        res.end(audio);
      }
    } else {
      res.writeHead(200, { "Content-Type": "text/html" });
      res.end("<!doctype html><title>Service worker regression</title>");
    }
  });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  origin = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch();
});

after(async () => {
  await browser?.close();
  if (server) await new Promise(resolve => server.close(resolve));
});

for (const cachedToken of ["Bearer fresh", null, "Bearer stale"]) {
  test(`authenticates no-cors media with cached token ${cachedToken}`, { timeout: 15000 }, async () => {
    const context = await browser.newContext();
    try {
      const page = await context.newPage();
      await page.goto(origin);
      await page.evaluate(async () => {
        await navigator.serviceWorker.register("/sw.js");
        await navigator.serviceWorker.ready;
      });
      await page.reload();
      requests.length = 0;
      const result = await page.evaluate(async cached => {
        navigator.serviceWorker.addEventListener("message", event => {
          if (event.data?.type === "GET_AUTH_TOKEN") {
            event.ports[0].postMessage({ token: "Bearer fresh" });
          }
        });
        navigator.serviceWorker.controller.postMessage({
          type: "SET_AUTH_TOKEN", token: cached,
        });
        // Native media loads use no-cors and a browser-generated Range header.
        // An ordinary fetch() uses cors and would hide this regression.
        const audio = new Audio();
        audio.preload = "auto";
        return new Promise((resolve, reject) => {
          audio.onloadedmetadata = () => resolve({ duration: audio.duration });
          audio.onerror = () => reject(new Error(`Audio load failed: ${audio.error?.code}`));
          audio.src = "/v1/content/stream/test-track";
        });
      }, cachedToken);
      assert.equal(result.duration, 0.1);
      assert.deepEqual(requests, (cachedToken === "Bearer stale"
        ? ["Bearer stale", "Bearer fresh"] : ["Bearer fresh"]
      ).map(authorization => ({ authorization, range: "bytes=0-" })));
    } finally {
      await context.close();
    }
  });
}
