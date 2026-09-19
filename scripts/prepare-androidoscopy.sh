#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
destination="${ANDROIDOSCOPY_CHECKOUT:-$(dirname "$repo_root")/androidoscopy}"
revision="$(cat "$repo_root/androidoscopy.rev")"

if [[ ! "$revision" =~ ^[0-9a-f]{40}$ ]]; then
    echo "androidoscopy.rev must contain a full Git commit hash" >&2
    exit 1
fi

if [[ -e "$destination" ]]; then
    if [[ ! -e "$destination/.git" ]] ||
        [[ "$(git -C "$destination" rev-parse HEAD)" != "$revision" ]] ||
        [[ -n "$(git -C "$destination" status --porcelain)" ]]; then
        echo "Existing $destination is not a clean checkout of $revision; leaving it unchanged." >&2
        exit 1
    fi
else
    git clone --no-checkout https://github.com/lelloman/androidoscopy.git "$destination"
    git -C "$destination" checkout --detach "$revision"
fi

# Separate Gradle invocation avoids mixing Androidoscopy's AGP 8 with our AGP 9.
if [[ -z "${ANDROID_HOME:-}" && -z "${ANDROID_SDK_ROOT:-}" && -f "$repo_root/android/local.properties" ]]; then
    androidoscopy_sdk_dir="$(sed -n 's/^sdk\.dir=//p' "$repo_root/android/local.properties")"
    if [[ -d "$androidoscopy_sdk_dir" ]]; then
        export ANDROID_HOME="$androidoscopy_sdk_dir"
    fi
fi
cd "$destination/android"
./gradlew :sdk:publishToMavenLocal :sdk-ui:publishToMavenLocal
