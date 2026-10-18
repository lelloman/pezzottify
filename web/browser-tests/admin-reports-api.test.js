import assert from "node:assert/strict";
import { test } from "node:test";
import {
  createAdminReportsApi,
  filters,
  reportError,
  transcript,
} from "../src/services/adminReports.js";
test("API uses versioned resources, encoded IDs and explicit cursor/version bodies", async () => {
  const calls = [];
  const api = createAdminReportsApi({
    async request(config) {
      calls.push(config);
      return { data: "ok" };
    },
  });
  assert.equal(await api.notes("report/one", 15), "ok");
  await api.deleteAttachment("report/one", "attachment/two");
  await api.toggleIntegration({ id: "integration", enabled: true, version: 5 });
  await api.retry({ id: "delivery", version: 9 });
  assert.equal(calls[0].url, "/v1/admin/reports/report%2Fone/notes");
  assert.deepEqual(calls[0].params, { before: 15, limit: 25 });
  assert.equal(calls[1].method, "delete");
  assert.equal(
    calls[1].url,
    "/v1/admin/reports/report%2Fone/attachments/attachment%2Ftwo",
  );
  assert.deepEqual(calls[2].data, { expected_version: 5, enabled: false });
  assert.deepEqual(calls[3].data, { expected_version: 9 });
  assert.deepEqual(
    filters({
      kind: "feature",
      has_diagnostics: false,
      user_id: 0,
      status: "",
      before: undefined,
    }),
    { kind: "feature", has_diagnostics: false, user_id: 0 },
  );
});
test("errors are actionable without reflecting raw server data and transcripts fail safely", () => {
  for (const [status, hint] of [
    [409, "Refresh"],
    [413, "Shorten"],
    [429, "Wait"],
    [503, "Free diagnostic"],
  ]) {
    const message = reportError({
      response: {
        status,
        data: { error: { message: "secret raw SQL" } },
        headers: { "retry-after": "60" },
      },
    });
    assert.match(message, new RegExp(hint));
    assert.match(message, /60/);
    assert(!message.includes("secret"));
  }
  assert.equal(transcript("broken"), null);
  assert.equal(transcript('{"schema_version":2,"turns":[]}'), null);
  assert.deepEqual(transcript('{"schema_version":1,"turns":[]}').turns, []);
});
