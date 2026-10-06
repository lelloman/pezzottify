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
  await page
    .locator(".nowPlaying")
    .getByRole("button", { name: "Play", exact: true })
    .click();
  await page
    .locator(".nowPlaying")
    .getByRole("button", { name: "Pause", exact: true })
    .waitFor();
  await page.waitForFunction(
    () =>
      document.querySelector(".nowPlaying .times span").textContent !== "0:12",
  );
  await page
    .locator(".nowPlaying")
    .getByRole("button", { name: "Pause", exact: true })
    .click();
  await page
    .locator(".nowPlaying")
    .getByRole("button", { name: "Next track", exact: true })
    .click();
  await page
    .locator(".nowPlaying h1")
    .filter({ hasText: "Open Windows" })
    .waitFor();
  console.log("PASS playback and skip");
  // Library navigation, filters, sorting, playback states, and playlist creation.
  await page.goto(origin + "/album/album-1");
  await page.waitForLoadState("networkidle");
  const library = page.getByRole("complementary", { name: "Your library" });
  const rows = library.locator(".libraryRow");
  assert.equal(await rows.count(), 7);
  assert.equal(
    await library.locator('.libraryLink[aria-current="page"]').count(),
    1,
  );
  await library.getByRole("button", { name: "Albums", exact: true }).click();
  assert.equal(await rows.count(), 3);
  await library
    .getByRole("button", { name: "Search your library", exact: true })
    .click();
  const librarySearch = library.getByRole("searchbox", {
    name: "Search your library",
  });
  await librarySearch.fill("mira");
  assert.equal(await rows.count(), 1);
  assert.match(await rows.first().innerText(), /Golden Hour/);
  await librarySearch.fill("nothing matches this");
  await library
    .getByText("No matches in your library.", { exact: true })
    .waitFor();
  await librarySearch.press("Escape");
  await library
    .getByRole("button", { name: "Show all library items", exact: true })
    .click();
  await library.getByLabel("Sort your library", { exact: true }).click();
  await library.getByRole("button", { name: "Creator", exact: true }).click();
  await library.getByLabel("Sort your library", { exact: true }).click();
  await library
    .getByRole("button", { name: "Alphabetical", exact: true })
    .click();
  const links = library.locator(".libraryLink");
  await links.first().focus();
  await page.keyboard.press("ArrowDown");
  assert.equal(
    await links.nth(1).evaluate((e) => e === document.activeElement),
    true,
  );
  await page.keyboard.press("End");
  assert.equal(
    await links.last().evaluate((e) => e === document.activeElement),
    true,
  );
  await library
    .getByRole("button", { name: "Collapse your library", exact: true })
    .click();
  assert.equal(await library.evaluate((e) => e.clientWidth), 72);
  await library
    .getByRole("button", { name: "Expand your library", exact: true })
    .click();
  await library
    .getByRole("button", { name: "Expand your library", exact: true })
    .click();
  assert.ok(await library.evaluate((e) => e.clientWidth > 600));
  assert.equal(await page.locator(".mainContentPanel").isVisible(), false);
  assert.equal(
    await library
      .locator(".libraryList")
      .evaluate((e) => getComputedStyle(e).display),
    "grid",
  );
  await library
    .getByRole("button", { name: "Minimize your library", exact: true })
    .click();
  await library
    .getByRole("button", { name: "Play Golden Hour", exact: true })
    .focus();
  await page.keyboard.press("Enter");
  await library
    .locator(".libraryRow.playing")
    .filter({ hasText: "Golden Hour" })
    .waitFor();
  await library
    .getByRole("button", { name: "Create playlist", exact: true })
    .click();
  await page.waitForURL(/\/playlist\/[^/]+\?edit=true$/);
  assert.ok(!page.url().includes("object"));
  await page.locator("#editPlaylistNameInput").waitFor();
  await page.locator("#editPlaylistNameInput").fill("Library smoke playlist");
  await page.getByText("Save", { exact: true }).click();
  await library.getByRole("link", { name: /Library smoke playlist/ }).waitFor();
  await library
    .getByRole("button", { name: "Show all library items", exact: true })
    .click();
  console.log(
    "PASS library filtering, keyboard navigation, width, playback and creation",
  );
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
