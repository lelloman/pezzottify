# Assistant session and execution safety

## Behavior

- Web logout, failed session checks, and account changes clear chat state and
  provider credentials, abort model requests, and close the separate MCP socket.
  Existing unowned browser configuration is cleared once on migration.
- MCP validates its credential and resolves current permissions before every RPC,
  using the same policy as HTTP. Opaque-token revocation and permission changes
  apply to already-open connections. OIDC credentials follow existing HTTP JWT
  expiry/validation policy; this does not add identity-provider token revocation.
- Clearing web chat cancels streaming, language detection, and compaction. Old
  continuations cannot write into a new conversation or start subsequent tools.
- Web tools have a ten-round per-turn budget, independent of transcript length.
  Budget-refused calls receive explicit results so the next request is well formed.
- Web playlist deletion and server administrative mutation tools require explicit
  confirmation of the tool and arguments. Android playlist deletion and track
  removal require a dialog; dismissal, cancellation, or a 60-second timeout denies
  the operation. Approval is bound to the pending request, not a model-supplied flag.
- Android clears unknown legacy chat ownership and clears history on account
  changes/logout, but preserves history when reopening the same account. History
  includes server identity so identical handles on different servers do not share it.
  Its shared library cancels active/queued operations before deleting history and
  clears the in-memory compaction summary too.

Cancellation cannot undo a side effect already dispatched or completed. Existing
server permissions continue to be enforced independently of assistant confirmation.

## Dependency and verification

The Android changes require shared library commit
`fbdc72fcf475f61d96a00d459be829f915a13f95`, on branch
`fix/assistant-session-safety`. Publish that commit to the library's Git remote
before relying on ordinary JitPack resolution. No external publication is part of
this change.

The app supports an explicit local composite build for testing before publication:

```sh
cd android
./gradlew -PassistantCheckout=/tmp/simple-android-assistant-safety \
  :app:compilePhoneDebugKotlin :ui:testDebugUnitTest :domain:test
```

Other checks:

- Library: `./gradlew :assistant-core:testDebugUnitTest`
- Web: `npm run test:unit`, targeted ESLint, `npm run build`
- Server: `cargo test --test e2e_mcp_tests`, `cargo test --lib`

Tests cover revocation on an open MCP socket, permission downgrade, shared
connection failures, pending-RPC cleanup, stale socket callbacks, conversation
cancellation, compaction invalidation, tool budgets, account history isolation,
and rejected/stale/timed-out confirmations. No real model calls are needed.

## SimpleAI streaming

The pinned library now consumes incremental responses from an updated SimpleAI
Android service. Existing chat UI state already renders these text deltas.
Tool arguments are assembled and validated before tools can execute. Clearing
chat or logging out cancels the callback session and its remote HTTP request.
Streams have a 180-second deadline and service-disconnection handling.

This requires the SimpleAI app changes on `fix/android-cloud-streaming` in
`/tmp/simple-ai-cloud-streaming`. That app forwards the backend's existing SSE
support, so no backend change is required. Older SimpleAI apps fall back to the
existing full-response call, without remote cancellation.

Before rollout, publish the library commit, build/install the updated SimpleAI
and Pezzottify apps, and check streaming, tool turns, clear/logout during a
response, and fallback with an older SimpleAI app on-device. Nothing was
published or installed during implementation.
