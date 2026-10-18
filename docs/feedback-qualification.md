# Feedback v1 qualification and rollout gates

This is the qualification record for the feedback/assistant diagnostics feature.
The normative API and privacy contract is [feedback-reports-v1.md](feedback-reports-v1.md).
This record authorizes no production deployment, publication, or live destination.

## Qualification blocker

The assistant source pinned at `87a499561a4db146d75051c34d46f89f4dac9e85`
declares `DiagnosticEnvelope.schema_version` with default `1` and calls the default
`Json.encodeToString` in `DiagnosticRecorder.snapshot()` (lines 20 and 115 of
`android/assistant-core/src/main/java/com/lelloman/simpleaiassistant/diagnostics/DiagnosticRecorder.kt`).
Default Kotlin serialization omits default-valued fields; see
[encodeDefaults](https://kotlinlang.org/api/kotlinx.serialization/kotlinx-serialization-json/kotlinx.serialization.json/-json-builder/encode-defaults.html).
Thus source inspection predicts every Android snapshot lacks `schema_version`.
The host `AndroidFeedback.prepare()` uploads `snapshot.content` unchanged, while
server `validate_diagnostics` requires an explicit numeric version. The HTTP
qualification test verifies that omission is rejected with 400 and a corrected
request can still use the same key. The new Android shared-fixture test asserts
that the outgoing snapshot contains version 1 and should expose this mismatch.
It has not been executed here because Java/Android tooling is unavailable.

Decision needed: route a scoped assistant-library fix to emit the mandatory version
(and other required default-valued fields, such as running turn status), test actual
serialized snapshots against the shared contract, and update this repository's
source pin. Explicit encoding must preserve the persisted/snapshot byte ceilings.
Do not weaken server validation or silently reinterpret unversioned uploads.
This qualification ticket must remain blocked until that dependency is resolved;
no library source was edited or published by this ticket.

## Reproducible coverage

| Task | Artifact and evidence |
| --- | --- |
| 16: shared contract | `docs/fixtures/assistant-diagnostics-v1.json` is consumed directly by server HTTP tests, web unit tests, and Android `FeedbackContractTest`. The Android test resource directory points at the same file, without a copied fixture. |
| 17: persistence and compatibility | `report_repository::tests`: `migrates_legacy_reports_without_losing_diagnostics`, `legacy_client_labels_remain_supported_without_relaxing_modern_validation`, `create_replay_conflict_and_owner_isolation`, `optimistic_updates_and_duplicate_cycles`, `cursor_is_stable_across_new_submissions`. `reports_owner_api_and_legacy_submission` exercises the HTTP routes. |
| 18: abuse/privacy/retention | `concurrent_quota_and_replays_are_atomic`, `retention_preserves_metadata_and_accounting_uses_bytes`, `diagnostic_capacity_rejects_without_partial_report_and_settings_conflict`, `audit_capacity_reserves_room_for_owner_deletion`; `report_limits_legacy_parity_and_explicit_permissions`; admission slot/window/body-size/deadline tests in `server/report_admission.rs`. |
| 19: controlled forwarding | `server_store/report_automation.rs` and `server/report_automation.rs` tests cover atomic fanout, stable IDs, bounded retries/responses, crash leases, DNS/address pinning, redirect rejection and HMAC. `audit_saturation_never_blocks_disable_and_retention_recovers` and `failed_automation_audit_rolls_back_control_and_pruning` cover the kill switch under saturation and audit failure. |
| 20: Android library | Recorder implementation and its boundary/redaction/eviction/recovery/cancellation tests live in the checkout pinned by `simple-android-assistant.rev`. `scripts/checkout-simple-android-assistant.sh` provisions this source. It uses a composite build; the older JitPack pin is substituted by the checkout. |
| 21: Android host | `FeedbackAccountIsolationTest`, `FeedbackLogExportTest`, remote API `FeedbackApiTest`, existing bug-report ViewModel tests, and new `FeedbackContractTest` cover immutable snapshots, byte accounting, explicit credentials, stale-account refusal and shared-schema recovery. Device acceptance remains a separate gate below. |
| 22: web | `src/services/diagnostics.test.js`, `feedback.test.js` and browser `diagnostics.test.js`/`feedback.test.js` cover Unicode/redaction/bounds, storage failure/recovery, consent races, IndexedDB clear epochs, account binding, fixed previews and explicit retries. |
| 23: administration | `triage_notes_are_paginated_and_separate_from_sensitive_audit`, automation permission HTTP tests, browser `admin-reports.test.js` and `admin-reports-api.test.js` cover separated permissions, safe text rendering, stale response suppression, conflict drafts, attachment deletion/retention and delivery controls. |
| 24: integrated qualification | `shared_client_snapshot_contract_exact_bytes_and_manual_replay` sends the shared envelope under both Android and web client identities, retrieves identical attachment bytes, checks concurrent replay receipts, edited-key conflict, absent consent and absence of duplicate reports. `diagnostic_utf8_boundary_and_invalid_schema_leave_submission_recoverable` verifies the exact 2,097,152-byte Unicode limit, one-byte overflow, unsupported schema, no-store errors, same-key recovery after rejection and byte-identical download. Web and Android shared-fixture recovery tests validate the client side of this contract. |

These are deterministic/local tests, not evidence of a successful production rollout.
The shared fixture verifies schema compatibility; it does not replace running the
Android recorder's own tests or exercising the real account lifecycle on a device.

## Commands

From `pezzottify-server/`, with `CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0` exported:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test --features fast
```

From the root: `bash scripts/check-db-boundaries.sh`. This script is absent in the
qualification baseline and cannot be reported as passing.

From `web/`, after Rust checks finish:

```sh
npm ci
npm run lint -- --max-warnings 0
npm run build
npm run test:unit
npm run test:worker
```

Android requires the pinned assistant/Paravoid/Androidoscopy source/artifacts,
JDK 17, Android SDK/NDK and native build tooling described in
`.github/workflows/android.yml`. Use that workflow's exact build and lint commands:

```sh
./gradlew --no-parallel --max-workers=2 \
  '-Dorg.gradle.jvmargs=-Xmx4g -Dfile.encoding=UTF-8' \
  testDebugUnitTest \
  :app:testNormalPhoneDebugUnitTest :app:testParavoidAndroidPhoneDebugUnitTest \
  :app:compileNormalPhoneReleaseKotlin :app:assembleNormalPhoneDebug \
  :app:assembleParavoidAndroidPhoneDebug :player-app:assembleDebug :player-app:assembleRelease
./gradlew --no-parallel --max-workers=2 \
  '-Dorg.gradle.jvmargs=-Xmx4g -Dfile.encoding=UTF-8' \
  :domain:lintDebug :ui:lintDebug :remoteapi:lintDebug :localdata:lintDebug \
  :debuginterface:lintDebug :player:lintDebug :logger:lintDebug \
  :equalizer:lintDebug :theme:lintDebug :player-app:lintDebug \
  :app:lintNormalPhoneDebug :app:lintParavoidAndroidPhoneDebug
python3 check-paravoid.py
```

Complete Paravoid packaging also uses the workflow's disposable key preparation,
`:app:assembleParavoidAndroidPhoneDebug -PparavoidComplete=true` with temporary
trust/key properties, `python3 check-paravoid-complete.py` and
`python3 check-paravoid-services.py app/build/outputs/paravoid/paravoidAndroidPhoneDebug`.
Those packaging checks and the new Android unit test are not verified in a session
without Java/Android tooling. Prior task 21's build evidence predates the current
pinned library and is historical evidence, not a build of this head.

## Rollout sequence (requires separate authorization)

1. Before any upgrade, back up `server.db` and validate restore in a disposable
   environment. Treat backups as sensitive: they contain attachments and integration
   signing material. Attachment expiry does not erase earlier backups.
2. Run the required checks on the intended revision. In a disposable environment,
   exercise migration from schema 7 (legacy reports) and schema 8 (reports before
   automation). Schema 9 must preserve reports and create no integration destinations;
   historical reports must not be replayed into newly enabled integrations.
3. Upgrade the server before clients. Verify legacy submission, owner reads,
   permission grants, quota/byte accounting and retention; inspect rejection counters.
   Explicitly grant triage, diagnostic access and integration administration separately.
   Do not assume an old administrator role grants new sensitive permissions.
4. Validate browser and Android clients with recording disabled by default. Preview
   technical logs and conversations separately; verify bytes against the stored copy,
   then manually retry the same request and verify one receipt. Confirm no automatic
   upload after a chat failure, restart, network recovery or account switch.
5. Confirm attachment expiry/deletion preserves report metadata and feature requests.
   Exercise quota exhaustion and recovery without deleting reports to create space.
6. Leave integrations disabled and unconfigured. A later approved receiver trial must
   use a separately approved destination, metadata-only payloads, HMAC verification
   and event-ID deduplication. Inspect retry/terminal failure visibility and verify
   disable stops new claims. An already claimed request may finish after disable.
7. On an acceptance failure, stop rollout and disable forwarding via its audited
   versioned control. Preserve evidence without copying private attachments into logs
   or tickets. Do not run an older server against a migrated database; restore the
   matching pre-upgrade database and binary only under an approved recovery plan.

## Remaining device/manual acceptance

- Android phone and supported alternative shell: English/Italian form layout,
  accessibility/TalkBack, keyboard focus, response selection and long Unicode previews.
- Cancel, background, force-stop and restart during streaming/checkpointing; verify
  interrupted recovery, unchanged model history and nonfatal recorder storage failure.
- Logout/server/account switch during recording, preview and upload; clear/disable;
  verify no new-account recording without consent or stale-account submission.
- Verify diagnostic files are excluded from real backup/restore; reinstall/OS storage
  eviction and browser IndexedDB unavailable/evicted behavior must fail gracefully.
- Use an isolated server to compare preview/upload bytes, owner status, separate
  attachment deletion/expiry and legacy-client submission under real authentication.
- Browser manual keyboard/screen-reader/visual acceptance and an approved receiver
  interoperability exercise remain unperformed. No live external forwarding is
  needed for automated tests, and no such destination was configured here.
