#!/usr/bin/env bash
# Build the production Pezzottify shell; the update server's publisher owns uploads.
set -euo pipefail
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
# Deployment-specific publisher settings stay out of Git; see android/PARAVOID.md.
local_env="$repo_root/scripts/local/paravoid-publish.env"
# shellcheck source=/dev/null
[[ -f "$local_env" ]] && { set -a; source "$local_env"; set +a; }
export ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}"
export PARAVOID_CHECKOUT="${PARAVOID_CHECKOUT:-$repo_root/android/.paravoid-signing/source}"
publisher="${PARAVOID_PUBLISHER:-}"
trust="${PARAVOID_TRUST_POLICY:-$repo_root/android/.paravoid-signing/trust.json}"
key="${PARAVOID_SIGNING_KEY:-$repo_root/android/.paravoid-signing/release.pk8}"
[[ -x "$publisher" && -s "$trust" && -s "$key" ]] || {
    echo 'Missing PARAVOID_PUBLISHER, production trust policy or payload signing key.' >&2
    exit 1
}
bash "$repo_root/scripts/checkout-paravoid.sh" >&2
payload_version="$(git -C "$repo_root" rev-list --count HEAD)"
(
    cd "$repo_root/android"
    ./gradlew :app:assembleParavoidAndroidPhoneParavoidRelease \
        "-PparavoidCheckout=$PARAVOID_CHECKOUT" -PparavoidComplete=true -PparavoidProduction=true \
        "-PparavoidTrustPolicy=$trust" "-PparavoidReleaseKey=$key" \
        "-PparavoidPayloadVersion=$payload_version" --max-workers=4 >&2
)
output="$repo_root/android/app/build/outputs/paravoid/paravoidAndroidPhoneParavoidRelease"
python3 "$repo_root/android/check-paravoid-complete.py" "$output" \
    --application-id com.lelloman.pezzottify.android >&2
python3 "$repo_root/android/check-paravoid-services.py" "$output" >&2
"$publisher" upload "$output/shell.apk" --distribution-mode paravoid --dry-run --json >&2
exec "$publisher" upload "$output/shell.apk" --distribution-mode paravoid "$@"
