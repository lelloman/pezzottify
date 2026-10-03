#!/usr/bin/env bash
# Build a Paravoid payload (VPK) update for an already published production shell and
# publish it to LelloStore. Installed shells only accept a VPK built for their exact
# shell contract, so the build pins the shell version to the published one and checks
# the contract against that shell's saved baseline.
#
# Usage: scripts/publish-paravoid-vpk.sh <shell-version-code>
#   e.g. scripts/publish-paravoid-vpk.sh 1745
# The shell's baseline must be saved in
#   android/.paravoid-signing/releases/<base-version>.<shell-version-code>/baseline-candidate
# (kept after each shell publication). The payload version is the current commit count.
set -euo pipefail
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
shell_version="${1:?usage: publish-paravoid-vpk.sh <shell-version-code>}"
[[ "$shell_version" =~ ^[0-9]+$ ]] || { echo "Shell version code must be a number." >&2; exit 1; }

export ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}"
export LELLOSTORE_URL="${LELLOSTORE_URL:-https://store.lelloman.com}"
export LELLOSTORE_OIDC_ISSUER="${LELLOSTORE_OIDC_ISSUER:-https://auth.lelloman.com}"
export LELLOSTORE_CLIENT_ID="${LELLOSTORE_CLIENT_ID:-22cd4a2d-a771-41e3-b76e-3f83ff8e9bbf}"
signing="$repo_root/android/.paravoid-signing"
export PARAVOID_CHECKOUT="${PARAVOID_CHECKOUT:-$signing/source}"
publisher="${LELLOSTORE_PUBLISHER:-$HOME/lelloprojects/lellostore/scripts/publish-to-lellostore.py}"
trust="${PARAVOID_TRUST_POLICY:-$signing/trust.json}"
key="${PARAVOID_SIGNING_KEY:-$signing/release.pk8}"
package=com.lelloman.pezzottify.android
variant=paravoidAndroidPhoneParavoidRelease
base_version="$(tr -d '[:space:]' < "$repo_root/VERSION")"
shell_dir="$signing/releases/$base_version.$shell_version"
[[ -x "$publisher" && -s "$trust" && -s "$key" ]] || {
    echo 'Missing authoritative publisher, production trust policy or payload signing key.' >&2
    exit 1
}
[[ -s "$shell_dir/baseline-candidate/shell-contract.json" ]] || {
    echo "No saved baseline for shell $base_version.$shell_version in $shell_dir." >&2
    exit 1
}
[[ -z "$(git -C "$repo_root" status --porcelain)" ]] || {
    echo 'Working tree is not clean; commit before publishing a payload.' >&2
    exit 1
}

payload_version="$(git -C "$repo_root" rev-list --count HEAD)"
(( payload_version > shell_version )) || {
    echo "Payload version $payload_version must be greater than the shell's $shell_version." >&2
    exit 1
}

baseline="$(mktemp -d)"
trap 'rm -rf "$baseline"' EXIT
mkdir -p "$baseline/$variant"
cp "$shell_dir/baseline-candidate/"* "$baseline/$variant/"

bash "$repo_root/scripts/checkout-paravoid.sh" >&2
(
    cd "$repo_root/android"
    ./gradlew ":app:assemble${variant^}" \
        "-PparavoidCheckout=$PARAVOID_CHECKOUT" -PparavoidComplete=true -PparavoidProduction=true \
        "-PparavoidTrustPolicy=$trust" "-PparavoidReleaseKey=$key" \
        "-PparavoidPayloadVersion=$payload_version" "-PparavoidShellVersionCode=$shell_version" \
        "-PparavoidBaseline=$baseline" --max-workers=4 >&2
)
output="$repo_root/android/app/build/outputs/paravoid/$variant"
grep -q '^Compatible with accepted shell contract' "$output/vpk-contract-check.txt" || {
    echo 'The payload is not compatible with the published shell:' >&2
    cat "$output/vpk-contract-check.txt" >&2
    exit 1
}
python3 "$repo_root/android/check-paravoid-complete.py" "$output" --application-id "$package" >&2
contract="$(sed -n 's/^Contract ID: //p' "$output/vpk-contract-check.txt")"

json_field() { python3 -c "import json,sys; print(json.loads(sys.stdin.read().strip().splitlines()[-1])$1)"; }
echo "Uploading payload $payload_version for shell $shell_version ($contract)..." >&2
upload_id="$("$publisher" upload-vpk "$package" "$contract" "$output/payload.vpk" --yes --json < /dev/null | json_field "['id']")"
for _ in $(seq 1 90); do
    status_json="$("$publisher" upload-status "$upload_id" --json < /dev/null | tail -n 1)"
    status="$(json_field "['status']" <<< "$status_json")"
    [[ "$status" == queued || "$status" == running ]] || break
    sleep 10
done
[[ "$status" == ready ]] || { echo "Upload validation ended as '$status': $status_json" >&2; exit 1; }
vpk_id="$(json_field "['result_json']" <<< "$status_json" | python3 -c "import json,sys; print(json.load(sys.stdin)['vpk_id'])")"
revision="$("$publisher" distribution "$package" --json < /dev/null | json_field "['publication_revision']")"
"$publisher" publish-vpk "$package" "$vpk_id" --expected-revision "$revision" --yes --json < /dev/null

archive="$shell_dir-p$payload_version"
mkdir -p "$archive"
cp "$output"/{payload.vpk,payload.vpk.sha256,payload.sha256,release.json,vpk-contract-check.txt,vpk-report.json,packaging-report.txt} "$archive/"
echo "Published payload $payload_version ($vpk_id) for shell $shell_version; artifacts kept in $archive." >&2
