import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { authorizationHeaderValue } from "./authorization.js";

const quiet = { debug() {}, error() {}, warn() {} };
function loadService(file, names, values, exports) {
  const source = readFileSync(new URL(file, import.meta.url), "utf8")
    .replace(/^import .*;\n/gm, "")
    .replaceAll("import.meta.env", "env")
    .replaceAll("export async function", "async function")
    .replaceAll("export function", "function")
    .replace(/export default \{[\s\S]*$/, "");
  return new Function(...names, `${source}\nreturn {${exports}};`)(...values);
}

function oidcService(renew) {
  const savedUser = { id_token: "old", refresh_token: "saved", expired: true };
  let removed = 0;
  const manager = {
    events: new Proxy({}, { get: () => () => {} }),
    getUser: async () => savedUser,
    signinSilent: renew,
    removeUser: async () => { removed++; },
  };
  const service = loadService("./oidc.js",
    ["UserManager", "WebStorageStateStore", "authorizationHeaderValue", "env", "window", "navigator", "localStorage", "console", "setTimeout", "clearTimeout"],
    [function () { return manager; }, function () {}, authorizationHeaderValue,
      { VITE_OIDC_AUTHORITY: "https://id.example", VITE_OIDC_CLIENT_ID: "web" },
      { location: { origin: "https://music.example" }, localStorage: {} }, {},
      { getItem: () => null, setItem() {} }, quiet, () => 1, () => {}],
    "refreshTokens, getIdToken");
  return { ...service, savedUser, removed: () => removed };
}

test("temporary renewal failures retain credentials and allow a later renewal", async () => {
  for (const failure of [new TypeError("Failed to fetch"), { status: 503 }, { error: "server_error" }]) {
    let attempts = 0;
    const service = oidcService(async () => {
      if (++attempts === 1) throw failure;
      return { id_token: "new", refresh_token: "rotated" };
    });
    await assert.rejects(service.refreshTokens());
    assert.equal(service.savedUser.refresh_token, "saved");
    assert.equal(service.removed(), 0);
    assert.equal((await service.refreshTokens()).id_token, "new");
  }
});

test("provider rate limiting is retryable rather than a rejected credential", async () => {
  let attempts = 0;
  const service = oidcService(async () => { attempts++; throw { status: 429 }; });
  await assert.rejects(service.refreshTokens());
  await assert.rejects(service.refreshTokens(), /temporarily rate limited/);
  assert.equal(attempts, 1);
  assert.equal(service.removed(), 0);
});

test("explicit invalid_grant requires authentication", async () => {
  const service = oidcService(async () => { throw { error: "invalid_grant" }; });
  assert.equal(await service.refreshTokens(), null);
});

test("an unavailable provider still lets requests try the server cookie", async () => {
  const service = oidcService(async () => { throw new TypeError("Failed to fetch"); });
  assert.equal(await service.getIdToken(), null);
  assert.equal(service.removed(), 0);
});

function apiService(refreshTokens) {
  let responseInterceptor;
  let logouts = 0;
  const location = { pathname: "/", href: "/" };
  const axios = {
    interceptors: {
      request: { use() {} },
      response: { use(_success, failure) { responseInterceptor = failure; } },
    },
  };
  loadService("./api.js", ["axios", "oidc", "getCsrfToken", "authorizationHeaderValue", "window", "console"],
    [axios, { refreshTokens, logout: async () => { logouts++; } }, () => null, authorizationHeaderValue, { location }, quiet],
    "setupAxiosInterceptors").setupAxiosInterceptors();
  return { fail: responseInterceptor, location, logouts: () => logouts };
}

test("concurrent 401s preserve login when renewal is temporarily unavailable", async () => {
  let rejectRefresh;
  const api = apiService(() => new Promise((_resolve, reject) => { rejectRefresh = reject; }));
  const error = () => ({ response: { status: 401 }, config: { url: "/v1/auth/session", headers: {} } });
  const first = api.fail(error());
  const second = api.fail(error());
  rejectRefresh(new Error("Provider unavailable"));
  const outcomes = await Promise.allSettled([first, second]);
  assert.ok(outcomes.every(result => result.status === "rejected"));
  assert.equal(api.logouts(), 0);
  assert.equal(api.location.href, "/");
});

test("a rejected refresh credential clears login and redirects", async () => {
  const api = apiService(async () => null);
  await assert.rejects(api.fail({ response: { status: 401 }, config: { url: "/v1/auth/session", headers: {} } }));
  assert.equal(api.logouts(), 1);
  assert.equal(api.location.href, "/login");
});

test("startup session failures preserve user and leave the session check retryable", async () => {
  for (const failure of [new TypeError("Failed to fetch"), { response: { status: 503 } }]) {
    const source = readFileSync(new URL("../store/auth.js", import.meta.url), "utf8")
      .replace(/^import .*;\n/gm, "")
      .replace("export const useAuthStore", "const useAuthStore");
    const store = new Function("defineStore", "axios", `${source}\nreturn useAuthStore;`)(
      (_name, options) => options,
      { get: async () => { throw failure; } },
    );
    const state = { ...store.state(), user: { handle: "saved-user" } };
    await assert.rejects(store.actions.checkSession.call(state));
    assert.deepEqual(state.user, { handle: "saved-user" });
    assert.equal(state.sessionChecked, false);
    assert.equal(state.sessionError, true);
  }
});
