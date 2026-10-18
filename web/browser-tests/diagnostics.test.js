import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { after, before, test } from "node:test";
import { chromium } from "@playwright/test";
let server, browser, origin;
before(async () => {
  const source = await readFile(
    new URL("../src/services/feedback/diagnostics.js", import.meta.url),
  );
  server = createServer((req, res) => {
    res.setHeader(
      "Content-Type",
      req.url === "/diagnostics.js" ? "application/javascript" : "text/html",
    );
    res.end(
      req.url === "/diagnostics.js"
        ? source
        : "<!doctype html><title>Diagnostic storage tests</title>",
    );
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  origin = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch();
});
after(async () => {
  await browser?.close();
  if (server) await new Promise((resolve) => server.close(resolve));
});
test("real IndexedDB recovers interrupted turns, enforces clear epochs across writers, and isolates accounts", async () => {
  const context = await browser.newContext();
  try {
    const page = await context.newPage();
    await page.goto(origin);
    const result = await page.evaluate(async () => {
      const { DiagnosticRecorder, indexedStorage } = await import(
        "/diagnostics.js"
      );
      const a = new DiagnosticRecorder();
      await a.account("server/A");
      await a.enable(true);
      a.event("t", "c", { kind: "user", content: "private" });
      await a.writes;
      const recovered = new DiagnosticRecorder();
      await recovered.account("server/A");
      const status = recovered.envelope.turns[0].status;
      const stale = indexedStorage();
      await stale.load();
      await a.clear();
      let rejected = false;
      try {
        await stale.save({
          owner: "server/A",
          enabled: true,
          envelope: recovered.envelope,
        });
      } catch {
        rejected = true;
      }
      const afterClear = await indexedStorage().load();
      await recovered.account("server/B");
      const afterSwitch = await indexedStorage().load();
      return {
        status,
        rejected,
        turns: afterClear.envelope.turns.length,
        afterSwitch,
      };
    });
    assert.equal(result.status, "interrupted");
    assert.equal(result.rejected, true);
    assert.equal(result.turns, 0);
    assert.equal(result.afterSwitch, undefined);
  } finally {
    await context.close();
  }
});
