import assert from "node:assert/strict";
import { chromium } from "@playwright/test";
import { createServer } from "vite";
import { albums, tracks, resolvedAlbum, resolvedTrack } from "./fixtures.js";

// Regression: duration recomputation on a 90-track album used to repeatedly
// fetch every pending track, flooding the browser with thousands of requests.
const server = await createServer({
  mode: "mock",
  server: { port: 5175, strictPort: false },
  logLevel: "error",
});
let browser;
try {
  await server.listen();
  const origin = `http://127.0.0.1:${server.httpServer.address().port}`;
  browser = await chromium.launch();
  const page = await browser.newPage();
  const counts = new Map(),
    errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const albumId = "large-album";
  const album = resolvedAlbum(albums[0]);
  album.album = { ...album.album, id: albumId, name: "90-track regression" };
  album.discs = [
    {
      number: 1,
      tracks: Array.from({ length: 90 }, (_, i) => ({
        ...tracks[0],
        id: `large-${i}`,
        album_id: albumId,
      })),
    },
  ];
  await page.route(`**/v1/content/album/${albumId}/resolved`, (route) =>
    route.fulfill({ json: album }),
  );
  await page.route("**/v1/content/track/large-*/resolved", async (route) => {
    const id = route.request().url().split("/").at(-2);
    counts.set(id, (counts.get(id) || 0) + 1);
    await new Promise((resolve) =>
      setTimeout(resolve, 100 + Number(id.split("-")[1]) * 25),
    );
    await route
      .fulfill({ json: resolvedTrack({ ...tracks[0], id, album_id: albumId }) })
      .catch(() => {});
  });
  await page.goto(`${origin}/album/${albumId}`, {
    waitUntil: "domcontentloaded",
  });
  await page.waitForFunction(
    () => document.querySelectorAll(".tracksContainer .trackRow").length === 90,
  );
  await page.waitForLoadState("networkidle");
  assert.equal(counts.size, 90);
  assert.equal(
    [...counts.values()].reduce((sum, count) => sum + count, 0),
    90,
  );
  assert.deepEqual(errors, []);
  console.log(
    "PASS 90-track album: exactly one request per track, all rows loaded, no browser errors",
  );
} finally {
  await browser?.close();
  await server.close();
}
