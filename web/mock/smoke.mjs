import assert from "node:assert/strict";
import { chromium } from "@playwright/test";
import { createServer } from "vite";
import { screens } from "./fixtures.js";

// Separate test server: tests never reset a developer's running design lab.
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
  const page = await browser.newPage({
    viewport: { width: 1440, height: 1000 },
  });
  const errors = [],
    external = [];
  page.on("pageerror", (e) => errors.push(e.stack));
  await page.route("**/*", (route) => {
    const url = new URL(route.request().url());
    if (["http:", "https:"].includes(url.protocol) && url.origin !== origin) {
      external.push(url.origin);
      return route.abort();
    }
    return route.continue();
  });
  for (const [name, path] of screens.filter(([n]) => n !== "Login")) {
    await page.goto(origin + path);
    await page.waitForLoadState("networkidle");
    assert.equal(new URL(page.url()).pathname, path, `${name} route`);
    assert.ok(
      (await page.locator("main").innerText()).trim(),
      `${name} has content`,
    );
    assert.deepEqual(errors, [], `${name}: browser errors`);
    console.log(`PASS ${name}`);
  }
  await page.goto(origin + "/now-playing");
  await page.getByRole("button", { name: "Play", exact: true }).click();
  await page.getByRole("button", { name: "Pause", exact: true }).waitFor();
  await page.waitForFunction(
    () =>
      document.querySelector(".nowPlaying .times span").textContent !== "0:12",
  );
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  await page.getByRole("button", { name: "Next track", exact: true }).click();
  await page
    .locator(".nowPlaying h1")
    .filter({ hasText: "Open Windows" })
    .waitFor();
  console.log("PASS playback and skip");
  const api = page.request;
  const created = await (
    await api.post(origin + "/v1/user/playlist", {
      data: { name: "Test playlist" },
    })
  ).json();
  await api.put(`${origin}/v1/user/playlist/${created}/add`, {
    data: { tracks_ids: ["track-1", "track-2"] },
  });
  await api.put(`${origin}/v1/user/playlist/${created}/remove`, {
    data: { tracks_positions: [0] },
  });
  assert.deepEqual(
    (await (await api.get(`${origin}/v1/user/playlist/${created}`)).json())
      .tracks,
    ["track-2"],
  );
  await api.post(origin + "/v1/user/liked/album/album-2");
  assert.ok(
    (
      await (await api.get(origin + "/v1/sync/state")).json()
    ).likes.albums.includes("album-2"),
  );
  console.log("PASS playlist edits and likes");
  assert.deepEqual(
    (await (await api.get(origin + "/__mock/status")).json()).unhandled,
    [],
  );
  await page.goto(origin + "/__mock");
  await page.locator("select").selectOption("empty");
  await page.getByRole("button", { name: "Apply and open app" }).click();
  await page.waitForURL(origin + "/");
  assert.deepEqual(
    (await (await api.get(origin + "/v1/content/featured/albums")).json())
      .albums,
    [],
  );
  await page.goto(origin + "/__mock");
  await page.locator("select").selectOption("signed-out");
  await page.getByRole("button", { name: "Apply and open app" }).click();
  await page.waitForURL(origin + "/login");
  // Exercise the actual callback screen through the local OIDC adapter.
  await page.getByRole("button", { name: /sign in with/i }).click();
  await page.waitForURL(origin + "/");
  assert.equal((await api.get(origin + "/v1/auth/session")).status(), 200);
  console.log("PASS empty scenario, login and callback");
  await api.post(origin + "/__mock/scenario", { data: { scenario: "error" } });
  assert.equal((await api.get(origin + "/v1/content/genres")).status(), 503);
  await api.post(origin + "/__mock/scenario", { data: { scenario: "slow" } });
  const started = Date.now();
  await api.get(origin + "/v1/content/genres");
  assert.ok(Date.now() - started >= 1700);
  await api.post(origin + "/__mock/scenario", {
    data: { scenario: "populated" },
  });
  assert.equal((await api.get(origin + "/v1/does-not-exist")).status(), 501);
  assert.deepEqual(
    (await (await api.get(origin + "/__mock/status")).json()).unhandled,
    ["GET /v1/does-not-exist"],
  );
  assert.deepEqual(errors, []);
  assert.deepEqual(external, [], "No external services requested");
  console.log(
    "PASS latency, errors, explicit missing handlers, no external requests",
  );
} finally {
  await browser?.close();
  await server.close();
}
