#!/usr/bin/env bash
set -euo pipefail

check_absent() {
    local description="$1"
    local pattern="$2"
    shift 2

    if rg -n --glob '*.rs' "$pattern" "$@"; then
        echo "database boundary violation: ${description}" >&2
        exit 1
    fi
}

check_absent \
    "async handlers must use DatabaseHandles for mutable stores" \
    'State\([^)]*\): State<(GuardedUserManager|GuardedServerStore)>' \
    src/server

check_absent \
    "migrated handler groups must use the shared database executor" \
    'tokio::task::spawn_blocking' \
    src/server/handlers_account.rs \
    src/server/handlers_admin_users.rs \
    src/server/handlers_catalog.rs \
    src/server/handlers_library.rs

check_absent \
    "catalog event reads must use the bounded atomic page API" \
    'get_catalog_events_(since|current_seq)' \
    src/server_store

echo "database boundary checks passed"
