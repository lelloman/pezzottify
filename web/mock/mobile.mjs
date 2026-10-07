import assert from "node:assert/strict";
import { chromium } from "@playwright/test";
import { createServer } from "vite";
import { screens } from "./fixtures.js";

const server = await createServer({
  mode: "mock",
  cacheDir: "node_modules/.vite-mock-mobile",
  server: { port: 5177, strictPort: false },
  logLevel: "error",
});
let browser;
try {
  await server.listen();
  const origin = `http://127.0.0.1:${server.httpServer.address().port}`;
  browser = await chromium.launch();
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const fits = async (selector) => {
    const box = await page.locator(selector).boundingBox();
    assert.ok(
      box && box.x >= 0 && box.x + box.width <= page.viewportSize().width + 1,
      `${selector} fits viewport`,
    );
  };
  for (const width of [320, 390, 768]) {
    await page.setViewportSize({ width, height: 740 });
    for (const [name, path] of screens.filter(
      ([name]) => !name.startsWith("Admin") && name !== "Login",
    )) {
      console.log(`CHECK ${width} ${name}`);
      await page.goto(origin + path);
      await page.waitForLoadState("networkidle");
      assert.equal(new URL(page.url()).pathname, path);
      assert.ok((await page.locator("main").innerText()).trim());
      assert.ok(
        await page
          .locator("main")
          .evaluate((e) => e.scrollWidth <= e.clientWidth + 1),
        `${name}: content overflow at ${width}`,
      );
      assert.ok(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
        `${name}: document overflow at ${width}`,
      );
    }
    console.log(`PASS user screens at ${width}px`);
  }
  await page.setViewportSize({ width: 320, height: 640 });
  await page.goto(origin + "/album/album-1");
  const nav = page.getByRole("navigation", { name: "Mobile navigation" });
  await nav.getByRole("button", { name: "Your library" }).click();
  await page
    .getByRole("complementary", { name: "Your library" })
    .waitFor({ state: "visible" });
  await nav.getByRole("button", { name: "Queue", exact: true }).click();
  await page.locator(".currentlyPlayingSideBar").waitFor({ state: "visible" });
  await nav.getByRole("button", { name: "Browse", exact: true }).click();
  await page.locator(".mainContentPanel").waitFor({ state: "visible" });
  const footer = await page.locator(".footerPlayer").boundingBox();
  const progress = await page.locator("#TrackProgressBar").boundingBox();
  assert.ok(
    progress.width > footer.width * 0.85,
    "mobile progress spans player",
  );
  await page.locator(".moreActions summary").click();
  await fits(".morePanel");
  await page
    .locator(".morePanel")
    .getByRole("button", { name: "Steer here" })
    .click();
  await page.locator(".destinationPrompt").waitFor();
  await fits(".destinationPrompt");
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.locator(".radioAction summary").click();
  await fits(".radioMenu");
  await page
    .getByRole("button", { name: "Customize radio", exact: true })
    .click();
  await page.locator(".builderBody").waitFor();
  await fits(".radioBuilder");
  await page.locator(".builderBody").evaluate((e) => {
    e.scrollTop = e.scrollHeight;
  });
  const submit = await page
    .getByRole("button", { name: "Start radio", exact: true })
    .boundingBox();
  assert.ok(
    submit.y >= 0 && submit.y + submit.height <= 640,
    "radio footer stays visible",
  );
  await page.getByRole("button", { name: "Close radio customization" }).click();
  await page.goto(origin + "/now-playing");
  await page
    .locator(".nowPlaying")
    .getByRole("button", { name: "Play", exact: true })
    .click();
  await page
    .locator(".nowPlaying")
    .getByRole("button", { name: "Pause", exact: true })
    .click();
  assert.deepEqual(errors, []);
  console.log("PASS mobile navigation, player, menus and dialogs");
} catch (error) {
  console.error(error);
  throw error;
} finally {
  await browser?.close();
  await server.close();
}
