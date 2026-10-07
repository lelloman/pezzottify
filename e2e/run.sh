#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Defaults
ANDROID=false
NO_TEARDOWN=false
PYTEST_ARGS=()
COMPOSE_PROFILES=()

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --android)
            ANDROID=true
            COMPOSE_PROFILES+=("android")
            shift
            ;;
        --no-teardown)
            NO_TEARDOWN=true
            shift
            ;;
        *)
            PYTEST_ARGS+=("$1")
            shift
            ;;
    esac
done

cd "$SCRIPT_DIR"

# Build compose profile args
PROFILE_ARGS=""
for profile in "${COMPOSE_PROFILES[@]+"${COMPOSE_PROFILES[@]}"}"; do
    PROFILE_ARGS="$PROFILE_ARGS --profile $profile"
done

# Build Android APK if requested
if [ "$ANDROID" = true ]; then
    echo "=== Building Android debug APK ==="
    cd "$REPO_ROOT/android"
    ./gradlew assembleDebug
    cd "$SCRIPT_DIR"
fi

cleanup() {
    if [ "$NO_TEARDOWN" = false ]; then
        echo "=== Tearing down ==="
        docker compose $PROFILE_ARGS down -v --remove-orphans 2>/dev/null || true
    else
        echo "=== Keeping stack running (--no-teardown) ==="
        echo "To tear down manually: cd $SCRIPT_DIR && docker compose down -v"
    fi
}

# Set up cleanup trap
trap cleanup EXIT

echo "=== Building pezzottify-server image (needed by seed container) ==="
docker compose build pezzottify-server

echo "=== Building and starting services ==="
docker compose $PROFILE_ARGS up --build -d --scale test-runner=0

echo "=== Waiting for pezzottify-server to be healthy ==="
timeout=120
elapsed=0
while ! docker compose ps pezzottify-server --format '{{.Health}}' 2>/dev/null | grep -q "healthy"; do
    if [ $elapsed -ge $timeout ]; then
        echo "ERROR: pezzottify-server did not become healthy within ${timeout}s"
        docker compose logs pezzottify-server
        exit 1
    fi
    sleep 2
    elapsed=$((elapsed + 2))
    echo "  waiting... (${elapsed}s)"
done

echo "=== pezzottify-server is healthy ==="

echo "=== Running tests ==="
# Default to all tests if no pytest args provided
if [ ${#PYTEST_ARGS[@]} -eq 0 ]; then
    PYTEST_ARGS=("tests/" "-v" "--tb=short")
fi

set +e
docker compose run --rm test-runner "${PYTEST_ARGS[@]}"
TEST_EXIT_CODE=$?
set -e

# Copy test results if available
RESULTS_CONTAINER=$(docker compose ps -q test-runner 2>/dev/null || true)
if [ -n "$RESULTS_CONTAINER" ]; then
    mkdir -p "$SCRIPT_DIR/test-results"
    docker cp "$RESULTS_CONTAINER:/test-results/." "$SCRIPT_DIR/test-results/" 2>/dev/null || true
fi

exit $TEST_EXIT_CODE
