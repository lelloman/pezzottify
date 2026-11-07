#!/usr/bin/env bash
# Installs the Android runtimes the unit tests load into the local Maven
# repository, where Robolectric finds them instead of downloading them itself.
#
# Robolectric downloads a runtime once, with no retry, while a test is starting,
# so one bad transfer on a clean machine fails whichever test got there first.
# Here each file is retried and checked before anything is installed.
#
# A runtime missing from this list still works: Robolectric downloads it, without
# this protection. Update the list with the Robolectric version or a new
# @Config(sdk = ...); robolectric.logging=stdout prints what a test run loads.
set -euo pipefail

versions=(
    5.0.2_r3-robolectric-r0-i7
    12.1-robolectric-8229987-i7
    14-robolectric-10818077-i7
    15-robolectric-12650502-i7
)
# Maven Central rate limits busy addresses, so its Google mirror is the fallback.
repositories=(
    https://repo1.maven.org/maven2
    https://maven-central.storage-download.googleapis.com/maven2
)
artifact_path=org/robolectric/android-all-instrumented
local_root="$HOME/.m2/repository/org/robolectric/android-all-instrumented"
max_attempts=5

staging=""
trap '[[ -z "$staging" ]] || rm -rf "$staging"' EXIT

# Retries stay in this script: curl --retry obeys Retry-After, which can stall a
# rate-limited download for as long as the server asks. timeout bounds each try.
download() {
    timeout 600 curl --fail --silent --show-error --location --connect-timeout 30 \
        --speed-limit 100000 --speed-time 60 \
        --output "$2" "$1"
}

# Fetches a file and its published SHA-512, and succeeds only if they match.
fetch_verified() {
    local url="$1" file="$2" expected actual
    download "$url" "$file" || return 1
    download "$url.sha512" "$file.sha512" || return 1
    expected="$(tr -d '[:space:]' < "$file.sha512")"
    actual="$(sha512sum "$file" | cut -d ' ' -f 1)"
    if [[ "$actual" != "$expected" ]]; then
        echo "SHA-512 mismatch for $url" >&2
        return 1
    fi
}

fetch_from_any() {
    local repository
    for repository in "${repositories[@]}"; do
        fetch_verified "$repository/$artifact_path/$1" "$2" && return 0
    done
    return 1
}

mkdir -p "$local_root"
for version in "${versions[@]}"; do
    destination="$local_root/$version"
    name="android-all-instrumented-$version"
    if [[ -f "$destination/$name.jar" ]]; then
        echo "Have $version"
        continue
    fi

    staging="$(mktemp -d "$local_root/.staging.XXXXXX")"
    for extension in pom jar; do
        attempt=1
        until fetch_from_any "$version/$name.$extension" "$staging/$name.$extension"; do
            if ((attempt == max_attempts)); then
                echo "Could not fetch $name.$extension after $max_attempts attempts" >&2
                exit 1
            fi
            echo "Retrying $name.$extension (attempt $attempt of $max_attempts failed)" >&2
            sleep $((attempt * 5))
            attempt=$((attempt + 1))
        done
    done

    mkdir -p "$destination"
    # The jar goes last: its presence is what marks the runtime as installed.
    mv "$staging/$name.pom" "$staging/$name.pom.sha512" "$staging/$name.jar.sha512" "$destination/"
    mv "$staging/$name.jar" "$destination/"
    rm -rf "$staging"
    staging=""
    echo "Installed $version"
done
