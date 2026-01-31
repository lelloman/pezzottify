# Comprehensive E2E Testing Environment Implementation Plan

## Executive Summary

Implement a Docker Compose-based E2E testing environment with a Python test runner (pytest) that coordinates across catalog-server, LelloAuth (OIDC provider), multiple web clients (via Playwright), and multiple Android emulators (via docker-android).

**Key decisions:**
- Docker Compose owns all service lifecycle
- Python pytest is the test runner (no custom orchestration layer)
- Pure pytest fixtures for dependency injection (no base class abstractions)
- LelloAuth container for real OIDC authentication
- Catalog-server serves the web frontend via `--frontend-dir-path` (same as production)
- Test catalog created via server's own schema management (not raw SQL)
- Local-only (requires KVM for Android emulators)
- Configurable N Android emulators and M web clients via Compose profiles/scale

---

## Directory Structure

```
pezzottify/
├── e2e-tests/
│   ├── docker/
│   │   ├── Dockerfile.catalog-server   # Catalog server + web frontend build
│   │   ├── Dockerfile.test-runner      # Python test runner with Playwright
│   │   └── docker-compose.yml          # Full E2E stack
│   ├── tests/
│   │   ├── __init__.py
│   │   ├── conftest.py                 # Pytest fixtures (services, clients, assertions)
│   │   ├── test_auth.py                # OIDC authentication flow tests
│   │   ├── test_sync.py                # Multi-device sync tests
│   │   └── test_flows.py               # Full user flow tests
│   ├── helpers/
│   │   ├── __init__.py
│   │   ├── config.py                   # Environment-based configuration
│   │   ├── catalog_api.py              # Catalog server API client
│   │   ├── web_client.py               # Playwright web client helper
│   │   ├── android_device.py           # ADB-based Android device helper
│   │   ├── websocket_client.py         # WebSocket client for sync monitoring
│   │   └── test_data.py                # Test data constants (reuse from server)
│   ├── scripts/
│   │   ├── build-apk.sh                # Build Android debug APK
│   │   ├── setup-test-data.sh          # Create test catalog via cli-auth + admin API
│   │   └── run-e2e.sh                  # Top-level runner (build, up, test, down)
│   └── requirements.txt                # Python dependencies
```

---

## Docker Compose Configuration

### File: `e2e-tests/docker/docker-compose.yml`

```yaml
services:
  # LelloAuth - OIDC provider
  lelloauth:
    image: ${LELLOAUTH_IMAGE:-lelloauth:latest}
    container_name: pezzottify-e2e-lelloauth
    networks:
      - e2e-net
    ports:
      - "8080:8080"
    environment:
      - LELLOAUTH_ISSUER_URL=http://lelloauth:8080
      # Test client configuration for pezzottify
      - LELLOAUTH_CLIENTS=${LELLOAUTH_CLIENTS:-}
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/.well-known/openid-configuration"]
      interval: 2s
      timeout: 1s
      retries: 15

  # Catalog Server - built with web frontend, configured for OIDC
  catalog-server:
    build:
      context: ../..
      dockerfile: catalog-server/Dockerfile
      args:
        VITE_OIDC_AUTHORITY: http://lelloauth:8080
        VITE_OIDC_CLIENT_ID: pezzottify-e2e
    container_name: pezzottify-e2e-catalog
    networks:
      - e2e-net
    depends_on:
      lelloauth:
        condition: service_healthy
    ports:
      - "3001:3001"
    volumes:
      - catalog-data:/data
    environment:
      - RUST_LOG=info
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3001/"]
      interval: 2s
      timeout: 1s
      retries: 30

  # Test runner - pytest with Playwright
  test-runner:
    build:
      context: ..
      dockerfile: docker/Dockerfile.test-runner
    container_name: pezzottify-e2e-runner
    networks:
      - e2e-net
    depends_on:
      catalog-server:
        condition: service_healthy
    volumes:
      - test-results:/test-results
    environment:
      - CATALOG_SERVER_URL=http://catalog-server:3001
      - LELLOAUTH_URL=http://lelloauth:8080
      - ANDROID_EMULATOR_HOSTS=android-emulator-1:5555,android-emulator-2:5555
      - PYTHONUNBUFFERED=1
    command: ["python", "-m", "pytest", "-v", "--html=/test-results/report.html"]

  # Android emulators - scale with --scale or profiles
  android-emulator-1:
    image: budtmo/docker-android:emulator-33
    container_name: pezzottify-e2e-android-1
    networks:
      - e2e-net
    privileged: true
    devices:
      - /dev/kvm
    ports:
      - "6080:6080"    # VNC (debug)
    environment:
      - EMULATOR_DEVICE=Pixel_5
      - WEB_VNC=true
    profiles:
      - android

  android-emulator-2:
    image: budtmo/docker-android:emulator-33
    container_name: pezzottify-e2e-android-2
    networks:
      - e2e-net
    privileged: true
    devices:
      - /dev/kvm
    ports:
      - "6081:6080"
    environment:
      - EMULATOR_DEVICE=Pixel_5
      - WEB_VNC=true
    profiles:
      - android

networks:
  e2e-net:
    driver: bridge

volumes:
  catalog-data:
  test-results:
```

**Usage:**
```bash
# Web-only tests (no emulators)
docker compose up --build test-runner

# Full suite including Android
docker compose --profile android up --build test-runner
```

---

## Authentication

Both web and Android clients authenticate via OIDC against the LelloAuth container.

**Setup flow:**
1. LelloAuth starts with a pre-configured client (`pezzottify-e2e`) and test users
2. Catalog-server config points OIDC at `http://lelloauth:8080`
3. Web frontend is built with `VITE_OIDC_AUTHORITY=http://lelloauth:8080`
4. Test users are created in LelloAuth (via its admin API or seed config)
5. Catalog-server users are created via `cli-auth` in `setup-test-data.sh`

**Note:** The catalog-server supports password auth alongside OIDC (unless `disable_password_auth` is set). API-level test helpers can use password auth for convenience, while UI-level tests exercise the full OIDC flow.

---

## Test Data Management

Test catalog and users are created using the server's own tooling, not raw SQL:

1. **`setup-test-data.sh`** runs after catalog-server is healthy:
   - Uses `cli-auth` to create test users with known credentials
   - Uses the admin API (`/v1/admin/*`) to create test catalog entries (artists, albums, tracks)
   - Copies minimal test media files (embedded MP3/JPEG, same pattern as `run-integration-tests.sh`)

2. **Constants** reuse IDs from `catalog-server/tests/common/constants.rs` where possible

This ensures test data stays valid across schema migrations.

---

## Test Framework

### Pytest Fixtures (`conftest.py`)

```python
import pytest
import pytest_asyncio
from helpers.config import E2EConfig
from helpers.catalog_api import CatalogApiClient
from helpers.web_client import PlaywrightWebClient
from helpers.android_device import AndroidDevice
from helpers.websocket_client import SyncWebSocketClient


@pytest.fixture(scope="session")
def config():
    """E2E configuration from environment variables."""
    return E2EConfig.from_env()


@pytest_asyncio.fixture(scope="session")
async def catalog_api(config):
    """Authenticated admin API client for test setup/teardown."""
    client = CatalogApiClient(config.catalog_server_url)
    await client.login_admin()
    yield client
    await client.close()


@pytest_asyncio.fixture
async def web_client(config):
    """Playwright browser pointed at the web app."""
    async with PlaywrightWebClient(config) as client:
        yield client


@pytest_asyncio.fixture
async def second_web_client(config):
    """Second browser instance (different device)."""
    async with PlaywrightWebClient(config, device_suffix="device-2") as client:
        yield client


@pytest_asyncio.fixture
async def android_device(config):
    """First Android emulator, app installed and launched."""
    device = AndroidDevice(config.android_hosts[0])
    await device.wait_for_boot()
    await device.install_apk()
    await device.launch_app()
    yield device
    await device.pull_logs("/test-results/")


@pytest_asyncio.fixture
async def ws_monitor(config):
    """WebSocket client for monitoring sync events."""
    async with SyncWebSocketClient(config.catalog_server_url) as ws:
        yield ws
```

### Multi-Device Assertions (`conftest.py` or `helpers/assertions.py`)

```python
async def assert_synced_across(clients, check_fn, timeout=10):
    """Wait until check_fn returns True for all clients."""
    ...

async def assert_event_received(ws_client, event_type, timeout=5):
    """Wait for a specific sync event on the WebSocket."""
    ...
```

---

## Test Scenarios

### Auth Tests (`test_auth.py`)
- `test_web_oidc_login_flow` — full OIDC redirect, callback, session established
- `test_android_oidc_login_flow` — OIDC login via Android app
- `test_session_persists_across_refresh` — reload page, still logged in

### Sync Tests (`test_sync.py`)
- `test_web_likes_track_android_sees_it`
- `test_android_creates_playlist_web_sees_it`
- `test_concurrent_likes_from_two_web_clients`
- `test_sync_state_consistent_after_reconnect`

### User Flow Tests (`test_flows.py`)
- `test_login_browse_search_play` — full user journey on web
- `test_create_playlist_add_tracks_play` — playlist creation and playback

---

## Android Device Control

Android emulators run as Docker containers. The test runner interacts with them via ADB over the network.

**Open questions:**
- **UI automation:** Appium (heavy but capable), UIAutomator2 via `uiautomator2` Python package (lighter), or pure ADB commands + screenshots?
- **Boot detection:** Poll `adb shell getprop sys.boot_completed` (returns "1" when booted) instead of relying on `adb connect` healthcheck
- **APK delivery:** Pre-built APK mounted as volume, or built as part of compose?

**Recommended approach:** Start with ADB + `uiautomator2` Python package for basic interactions (launch, tap, verify text). Upgrade to Appium only if more complex UI automation is needed.

**Boot detection in test fixtures:**
```python
async def wait_for_boot(self, timeout=120):
    """Poll sys.boot_completed via ADB until emulator is ready."""
    while True:
        result = await self.adb("shell", "getprop", "sys.boot_completed")
        if result.strip() == "1":
            return
        await asyncio.sleep(2)
```

---

## Implementation Phases

### Phase 1: Infrastructure
1. Create directory structure
2. Write `docker-compose.yml` with catalog-server + LelloAuth + test-runner
3. Write `Dockerfile.test-runner` (Python + Playwright + ADB tools)
4. Write `setup-test-data.sh` (create users and catalog via `cli-auth` + admin API)
5. Write `conftest.py` skeleton with `config` and `catalog_api` fixtures
6. **Verify:** `docker compose up` starts all services, test data is seeded

### Phase 2: Web Client Tests
1. Implement `helpers/web_client.py` (Playwright wrapper)
2. Write `test_auth.py` — OIDC login flow
3. Write first sync test: web likes track, verify via API
4. Implement `helpers/websocket_client.py` for real-time event monitoring
5. **Verify:** Web tests pass against compose stack

### Phase 3: Android Integration
1. Add Android emulator services to compose (behind `android` profile)
2. Implement `helpers/android_device.py` (ADB + uiautomator2)
3. Write `scripts/build-apk.sh`
4. Write first Android test: launch app, verify login
5. **Verify:** Android tests pass with emulator

### Phase 4: Multi-Device Tests
1. Implement multi-device assertion helpers
2. Write cross-platform sync tests (web <-> Android)
3. Add screenshot/logcat capture on failure
4. Add HTML test report generation
5. **Verify:** Full suite passes reliably

---

## Dependencies

### Python (`requirements.txt`)
```
pytest>=7.4.0
pytest-asyncio>=0.21.0
pytest-html>=3.2.0
playwright>=1.40.0
websockets>=11.0
aiohttp>=3.9.0
uiautomator2>=3.0.0
pydantic>=2.4.0
rich>=13.6.0
```

### Docker Images
- LelloAuth image (from external registry)
- `budtmo/docker-android:emulator-33` — Android emulators
- `python:3.11-slim` — Test runner base
- Catalog-server built from existing `catalog-server/Dockerfile` (includes web frontend)

---

## Runner Script

### `scripts/run-e2e.sh`
```bash
#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
E2E_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$E2E_DIR")"

PROFILE_ARGS=""
if [[ "${INCLUDE_ANDROID:-false}" == "true" ]]; then
  PROFILE_ARGS="--profile android"
fi

cd "$E2E_DIR/docker"

# Build and start services
docker compose $PROFILE_ARGS up --build -d

# Wait for catalog-server, then seed test data
docker compose exec catalog-server /app/setup-test-data.sh

# Run tests
docker compose $PROFILE_ARGS run --rm test-runner

# Collect results
docker compose cp test-runner:/test-results ./results/

# Cleanup
docker compose $PROFILE_ARGS down -v
```

**Usage:**
```bash
# Web-only
./e2e-tests/scripts/run-e2e.sh

# With Android emulators
INCLUDE_ANDROID=true ./e2e-tests/scripts/run-e2e.sh
```

---

## Notes

- **Network:** All services share `e2e-net` bridge network, inter-service communication uses container names as hostnames
- **OIDC redirect URIs:** LelloAuth must be configured with `http://catalog-server:3001/auth/callback` as an allowed redirect URI
- **Android emulator resources:** Each emulator needs ~2GB RAM and 2 CPU cores; requires KVM on the host
- **No Docker-in-Docker:** Compose owns all container lifecycle; the test runner is a pure pytest process with no Docker SDK dependency
- **Test data:** Created via server tooling (`cli-auth`, admin API), not raw SQL — survives schema migrations
- **Existing fixtures:** `web/e2e-tests/fixtures.ts` has reusable patterns for API helpers (sync state, likes, playlists) — port to Python
