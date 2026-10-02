# Paravoid Android integration

The Android app uses the Paravoid source revision pinned in `../paravoid.rev`,
not a published release. The current pin is `33340c6` (DVPK delivery and
APK-only distribution); no public Paravoid version has been released yet.
It adds the `paravoidPackaging` dimension: `normal` and `paravoidAndroid`.
Only **Paravoid phone/debug and phone/paravoidTestRelease** are enabled. Normal
phone/TV build types remain available. Shell TV (LEANBACK) and shrinking are not
supported yet.

## Build

From the repository root, provision the pinned source with
`bash scripts/checkout-paravoid.sh`. Existing checkouts must be clean and match
the pin; the script leaves other revisions unchanged. For an isolated checkout,
set `PARAVOID_CHECKOUT=/path/to/checkout` when running the script and pass the
same path as `-PparavoidCheckout=/path/to/checkout` to Gradle. All six API,
contract and runtime projects come from that checkout, along with the core and
Hilt Gradle plugins.

Use the exact assistant revision in `../simple-android-assistant.rev`; an older
local checkout can fail both normal and shell compilation. Keep Androidoscopy
prepared as documented by the existing build setup. No credentials or release
signing files are included in this branch.

```sh
ANDROID_HOME=/path/to/sdk ./gradlew \
  :app:assembleNormalPhoneDebug :app:assembleParavoidAndroidPhoneDebug \
  -PparavoidCheckout=/path/to/paravoid-android \
  -PassistantCheckout=/path/to/pinned/simple-android-assistant \
  -PassistantAbis=x86_64
```

The ABI override above is for an x86_64 emulator. Omit it to build all supported
assistant ABIs for a phone. The normal app keeps minSdk 24; the experiment uses
minSdk 30. The normal app depends only on Paravoid's API/base Application class;
the shell runtime is a `paravoidAndroidImplementation` dependency.

For live phone testing, `paravoidTestRelease` is non-debuggable, unshrunk, and
signed with the debug key. It uses release dependency variants; it is **not** a
production-signed release. Normal builds do not expose this test build type.

```sh
ANDROID_HOME=/path/to/sdk ./gradlew \
  :app:assembleParavoidAndroidPhoneParavoidTestRelease \
  -PparavoidCheckout=/path/to/paravoid-android \
  -PassistantCheckout=/path/to/pinned/simple-android-assistant \
  -PassistantAbis=arm64-v8a \
  -PconnectionPropertiesFile=/path/to/original/android/local.properties
```

The external file supplies only the OIDC issuer/client and server URL; it is not
copied into this worktree. OIDC settings must be nonempty when this option is used.
Do not commit local configuration or signing material. The `.paravoid` identity
and callback registration remain the same as the debug experiment.

## Identity and authentication

| Mode | Application ID | OIDC redirect URI |
| --- | --- | --- |
| Normal phone | `com.lelloman.pezzottify.android` | `com.lelloman.pezzottify.android://oauth/callback` |
| Paravoid phone | `com.lelloman.pezzottify.android.paravoid` | `com.lelloman.pezzottify.android.paravoid://oauth/callback` |

The apps coexist with separate data. The Paravoid launcher icon has a small V badge
(including round and themed icons); the normal icon is unchanged. The badge is a
flavor resource overlay, not a plugin-wide change to downstream branding.
Provider authorities use `${applicationId}`.
The callback manifest, request URI and callback handler share the flavor's
redirect scheme. **Register the Paravoid URI with the OIDC provider before trying
real login.** Keeping the original scheme would make the two installs compete
for callbacks. Changing the Paravoid application ID later also requires reviewing
this explicitly configured scheme and the provider registration.

OIDC issuer/client settings still come from local.properties. Without those
settings and a test account, startup/callback routing is not proof of login.
Authenticator integration remains disabled by default; its package/signature
registration is a separate validation gate.

## Packaging scope

`PezzottifyApplication` extends `ParavoidAndroidApplication`; the optional Hilt
adapter rewrites payload-side injection lookups. The installed manifest keeps
AppAuth/Androidoscopy Activities and other dependency components. Their code goes
into the payload. Adding/changing component declarations requires a new shell.

The default remains embedded DEX packaging: resources, assets and native libraries
stay in the installed APK. Opt-in complete packaging moves those components into
a signed VPK; see the build and delivery acceptance instructions below.
Build and emulator results must be recorded separately from authenticated server,
playback and background-sync coverage. Do not install the normal APK on a personal
device as part of this experiment: its identity is the everyday app's identity.

## Repeatable checks

After the x86_64 build, from this directory:

```sh
python3 check-paravoid.py
python3 smoke-paravoid.py --serial emulator-5584
```

The first command reads APKs and generated manifests. The second refuses physical
device serials and requires an emulator; use a disposable emulator without logged-in
accounts. It installs **both** APKs, force-stops/restarts them, temporarily rotates
the display (restoring its settings), sends a callback without an auth code, and
checks copies of their Room databases. It does not clear app data or use a server.

The initial integration's API-30 and API-36.1/x86_64 runs passed for both modes: login Compose UI, cold start,
restart, Androidoscopy initialization, rotation, isolated callback resolution and
SQLite integrity/Room identity for `StaticsDb` and `user_content`. APK inspection
found five payload DEX files totaling 70,789,516 bytes, requiring Paravoid's updated
32 MiB/file and 128 MiB total bounds (older shells reject this payload).

Not yet proved: real OIDC login/token exchange; Androidoscopy Dashboard/Session
screens; AppAuth's browser Activity lifecycle; authenticated background sync,
playback or assistant JNI execution. ADB cannot directly launch these non-exported
library Activities as the shell UID; do not weaken their exported flags for a test.
Startup and packaged native libraries are not evidence that all those flows work.

## Latest dependency upgrade validation

On 2026-10-02, revision `33340c6` passed both phone/debug APK builds and
`check-paravoid.py` (five payload DEX files totaling 70,950,800 bytes). Both
packaging modes passed lint and all 50 app unit tests each. Normal phone release
and TV debug Kotlin compilation and the standalone player debug APK build passed.
A disposable API-36.1/x86_64 emulator passed `smoke-paravoid.py` for both modes: cold
start, restart, login UI, Androidoscopy initialization, rotation, isolated OAuth
callback routing and Room database integrity. This upgrade did not rerun API 30
or authenticated login, playback and background-sync scenarios.

## Complete packaging

The former `codex/paravoid-v1-packaging` and `codex/paravoid-release-realapp`
experiments are integrated into `dev`. Complete packaging is opt-in; ordinary
normal and embedded DEX builds retain their existing commands.

Generate disposable test keys using the pinned Paravoid checkout (requires Python
`cryptography`), then build from this directory:

```sh
python3 /path/to/paravoid-android/packaging-tests/prepare-keys.py \
  --application-id com.lelloman.pezzottify.android.paravoid \
  --output /tmp/pezzottify-paravoid-test-keys
./gradlew :app:assembleParavoidAndroidPhoneDebug \
  -PparavoidCheckout=/path/to/paravoid-android \
  -PparavoidComplete=true \
  -PparavoidTrustPolicy=/tmp/pezzottify-paravoid-test-keys/trust.json \
  -PparavoidReleaseKey=/tmp/pezzottify-paravoid-test-keys/release.der
python3 check-paravoid-complete.py
```

Outputs under `app/build/outputs/paravoid/paravoidAndroidPhoneDebug/` include
`shell.apk`, `payload.vpk`, its signed `release.json`, `payload.sha256`, packaging
reports and a `baseline-candidate/`. The final ordinary APK output is also the
complete shell. This is embedded bootstrap with updates disabled by default.
Keys stay outside Git; disposable test keys are unsuitable for published artifacts.

To produce a compatible next payload, pass a monotonically increasing
`-PparavoidPayloadVersion=N` and `-PparavoidBaseline=/path/to/accepted`, where
`accepted/paravoidAndroidPhoneDebug/` contains the first build's baseline candidate.
The plugin checks resource IDs and the shell contract against that baseline.
A contract change requires a new shell APK. Complete shells require Android 11
(API 30); the Paravoid-only Room overlay disables pre-unlock execution of Room's
multi-instance invalidation service. Normal APK declarations are unchanged.

## Disposable update-delivery acceptance

`-PparavoidAcceptance=true` requires `-PparavoidComplete=true` and enables debug
HTTP updates at `http://127.0.0.1:19165/`. Paravoid acceptance is limited to
debug variants; normal variants keep acceptance disabled. `-PparavoidAcceptanceGeneration=A|B|broken|repair` selects the
Application marker; `broken` deliberately fails startup so the shell's quarantine
and forward-repair paths can be tested. This property is rejected outside acceptance
mode. Normal builds have acceptance disabled, and release-derived builds disable
the marker and fault regardless of build properties.

The pinned Paravoid checkout's `release-tests/pezzottify-build.sh` builds immutable
A1/B2/broken4/repair5 artifacts from this app, and `pezzottify-device.py` exercises
signed delivery, activation, incompatible-release refusal, quarantine, repair,
retained app preferences and Room database integrity. See that checkout's
`release-tests/PEZZOTTIFY.md` for the environment variables and invocation; the
historical branch/commit references there describe the original fixture, whose
properties are now available in `dev`. Use only a fresh disposable emulator;
the harness refuses a physical device or an existing app installation.

CI checks ordinary APK packaging first, then builds complete packaging with
throwaway keys and inspects embedded/standalone payload integrity and code separation.
Delivery acceptance remains an explicit emulator run.
