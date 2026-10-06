import test from "node:test";
import assert from "node:assert/strict";
import { createAuthenticatedFetch } from "./authenticatedFetch.js";

test("search sends the OIDC token and preserves the streaming response", async () => {
  const response = new Response('data: {"section":"results","items":[]}\n\n');
  const request = createAuthenticatedFetch(
    { getIdToken: async () => "current" },
    async (url, options) => {
      assert.match(url, /tchaikowski/);
      assert.equal(options.headers.get("Authorization"), "Bearer current");
      assert.equal(options.headers.get("Accept"), "text/event-stream");
      return response;
    },
  );
  assert.equal(
    await request("/v1/content/search/stream?q=tchaikowski", {
      headers: { Accept: "text/event-stream" },
    }),
    response,
  );
});

test("401 renews credentials once and replays the search with CSRF and body intact", async () => {
  globalThis.document = { cookie: "csrf_token=csrf" };
  try {
    let calls = 0;
    let renewals = 0;
    const request = createAuthenticatedFetch(
      {
        getIdToken: async () => "old",
        refreshTokens: async () => {
          renewals++;
          return { id_token: "new" };
        },
      },
      async (_url, options) => {
        calls++;
        assert.equal(
          options.headers.get("Authorization"),
          calls === 1 ? "Bearer old" : "Bearer new",
        );
        assert.equal(options.headers.get("X-CSRF-Token"), "csrf");
        assert.equal(options.body, '{"query":"tchaikowski"}');
        return new Response(null, { status: 401 });
      },
    );
    assert.equal(
      (
        await request("/v1/content/search", {
          method: "POST",
          body: '{"query":"tchaikowski"}',
        })
      ).status,
      401,
    );
    assert.equal(calls, 2);
    assert.equal(renewals, 1);
  } finally {
    delete globalThis.document;
  }
});

test("cookie sessions still work without an OIDC token", async () => {
  const request = createAuthenticatedFetch(
    { getIdToken: async () => null },
    async (_url, options) => {
      assert.equal(options.headers.has("Authorization"), false);
      return new Response("[]");
    },
  );
  assert.equal((await request("/v1/content/search/stream")).status, 200);
});

test("temporary renewal failure propagates instead of appearing as empty results", async () => {
  const request = createAuthenticatedFetch(
    {
      getIdToken: async () => "old",
      refreshTokens: async () => {
        throw new Error("Provider unavailable");
      },
    },
    async () => new Response(null, { status: 401 }),
  );
  await assert.rejects(
    request("/v1/content/search/stream"),
    /Provider unavailable/,
  );
});

test("cancelling a search during renewal prevents a stale retry", async () => {
  const controller = new AbortController();
  let calls = 0;
  const request = createAuthenticatedFetch(
    {
      getIdToken: async () => "old",
      refreshTokens: async () => {
        controller.abort();
        return { id_token: "new" };
      },
    },
    async () => {
      calls++;
      return new Response(null, { status: 401 });
    },
  );
  await assert.rejects(
    request("/v1/content/search/stream", { signal: controller.signal }),
    { name: "AbortError" },
  );
  assert.equal(calls, 1);
});
