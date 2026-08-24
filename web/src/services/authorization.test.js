import assert from "node:assert/strict";
import test from "node:test";

import { authorizationHeaderValue } from "./authorization.js";

test("formats opaque and OIDC tokens as Bearer credentials", () => {
  assert.equal(authorizationHeaderValue("opaque-token"), "Bearer opaque-token");
  assert.equal(
    authorizationHeaderValue("header.payload.signature"),
    "Bearer header.payload.signature"
  );
});

test("does not silently accept an empty token", () => {
  assert.throws(() => authorizationHeaderValue(""), /token/i);
});
