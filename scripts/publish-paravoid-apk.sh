#!/usr/bin/env bash
# Build the production Pezzottify shell; the shared Store publisher owns uploads.
set -euo pipefail
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
export ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}"
export LELLOSTORE_URL="${LELLOSTORE_URL:-https://store.lelloman.com}"
export LELLOSTORE_OIDC_ISSUER="${LELLOSTORE_OIDC_ISSUER:-https://auth.lelloman.com}"
export LELLOSTORE_CLIENT_ID="${LELLOSTORE_CLIENT_ID:-22cd4a2d-a771-41e3-b76e-3f83ff8e9bbf}"
export PARAVOID_CHECKOUT="${PARAVOID_CHECKOUT:-$repo_root/android/.paravoid-signing/source}"
publisher="${LELLOSTORE_PUBLISHER:-$HOME/lelloprojects/lellostore/scripts/publish-to-lellostore.py}"
trust="${PARAVOID_TRUST_POLICY:-$repo_root/android/.paravoid-signing/trust.json}"
key="${PARAVOID_SIGNING_KEY:-$repo_root/android/.paravoid-signing/release.pk8}"
[[ -x "$publisher" && -s "$trust" && -s "$key" ]] || {
    echo 'Missing authoritative publisher, production trust policy or payload signing key.' >&2
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
