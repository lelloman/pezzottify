# Paravoid integration experiment

This worktree uses the source checkout of Paravoid, not a published release.
It adds the `paravoidPackaging` dimension: `normal` and `paravoidAndroid`.
Only **Paravoid phone/debug** is enabled. Normal phone/TV build types remain
available. Shell TV (LEANBACK) and shrinking are not supported yet.

## Build

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

## Identity and authentication

| Mode | Application ID | OIDC redirect URI |
| --- | --- | --- |
| Normal phone | `com.lelloman.pezzottify.android` | `com.lelloman.pezzottify.android://oauth/callback` |
| Paravoid phone | `com.lelloman.pezzottify.android.paravoid` | `com.lelloman.pezzottify.android.paravoid://oauth/callback` |

The apps coexist with separate data. Provider authorities use `${applicationId}`.
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

This remains embedded DEX packaging: resources, assets and native libraries stay
in the installed APK. It is not an external updater or complete resource packaging.
Build and emulator results must be recorded separately from authenticated server,
playback and background-sync coverage. Do not install the normal APK on a personal
device as part of this experiment: its identity is the everyday app's identity.
