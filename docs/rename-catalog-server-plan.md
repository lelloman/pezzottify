# Rename "catalog-server" to "pezzottify-server"

## Summary

Rename the `catalog-server` directory and all references to `pezzottify-server` for better clarity in homelab deployments.

## Critical Files to Modify

### 1. Rust/Cargo Core
- `catalog-server/Cargo.toml` - Package name, binary names, default-run
- `catalog-server/src/lib.rs` - Library documentation comment
- `catalog-server/src/main.rs` - `use pezzottify_catalog_server::*` imports
- `catalog-server/src/cli_auth.rs` - `use pezzottify_catalog_server::*` imports
- All other `catalog-server/src/*.rs` files with `use pezzottify_catalog_server::*`

### 2. Docker & Deployment
- `catalog-server/Dockerfile` - Directory paths, binary names
- `docker-compose.yml` - Service name, Dockerfile path, command
- `build-docker.sh` - Working directory references

### 3. CI/CD
- `.github/workflows/catalog-server.yml` - Rename to `pezzottify-server.yml`, path patterns, working directory

### 4. Documentation
- `CLAUDE.md` (root) - Project overview, commands, descriptions
- `README.md` (root) - Component references, links
- `catalog-server/README.md` - All references throughout
- `TODO.md` - Section headers
- `android/CLAUDE.md` - References to catalog-server

### 5. Build Scripts
- `android/run-integration-tests.sh` - CATALOG_SERVER_DIR, Docker image name, paths

### 6. Web/E2E Tests
- `web/e2e-tests/global-setup.ts` - Error messages
- `web/playwright.config.ts` - Comments

### 7. Docs Directory
- `docs/agentic-ingestion.md` - File path references
- `docs/composite-popularity-scoring.md` - References
- `docs/FINGERPRINTS_SPEC.md` - Path references

### 8. Git (optional)
- Consider renaming remote branches: `catalog-server` → `pezzottify-server`

## Implementation Steps

### Step 1: Rename the directory
```bash
mv catalog-server pezzottify-server
git add catalog-server pezzottify-server
```

### Step 2: Update Cargo.toml
**File**: `pezzottify-server/Cargo.toml`
- Change package name: `pezzottify-catalog-server` → `pezzottify-server`
- Change default-run: `catalog-server` → `pezzottify-server`
- Change binary name (line 76): `catalog-server` → `pezzottify-server`

### Step 3: Update Rust crate imports
**Files**: All `pezzottify-server/src/*.rs` files with `use pezzottify_catalog_server::*`
- Replace `use pezzottify_catalog_server::` with `use pezzottify_server::`

### Step 4: Update Dockerfile
**File**: `pezzottify-server/Dockerfile`
- Update binary references: `catalog-server` → `pezzottify-server`
- Update directory paths where applicable

### Step 5: Update docker-compose.yml
**File**: `docker-compose.yml`
- Service name: `catalog-server` → `pezzottify-server`
- Dockerfile path: `catalog-server/Dockerfile` → `pezzottify-server/Dockerfile`
- Volume mount path: `./catalog-server/config.toml` → `./pezzottify-server/config.toml`
- Command: `catalog-server` → `pezzottify-server`

### Step 6: Update CI/CD workflow
**Action**: Rename `.github/workflows/catalog-server.yml` → `.github/workflows/pezzottify-server.yml`
**File**: `.github/workflows/pezzottify-server.yml`
- Workflow name: `Catalog Server CI` → `Pezzottify Server CI`
- Path patterns: `catalog-server/**` → `pezzottify-server/**`
- Workflow trigger: `catalog-server.yml` → `pezzottify-server.yml`
- Working directory: `catalog-server` → `pezzottify-server`
- Cache key path: `catalog-server/Cargo.lock` → `pezzottify-server/Cargo.lock`
- Cache path: `catalog-server/target` → `pezzottify-server/target`

### Step 7: Update build-docker.sh
**File**: `build-docker.sh`
- Currently uses `docker compose` with `catalog-server/Dockerfile` context
- The script builds the service defined in docker-compose.yml, so changes in Step 5 should cover this
- Verify no hardcoded references remain

### Step 8: Update integration test script
**File**: `android/run-integration-tests.sh`
- `CATALOG_SERVER_DIR` → `PEZZOTTIFY_SERVER_DIR` (or keep name if you prefer)
- Docker image name: `pezzottify-catalog-server` → `pezzottify-server`
- Directory path: `catalog-server/` → `pezzottify-server/`
- Docker label: `catalog-server.commit` → `pezzottify-server.commit`

### Step 9: Update root CLAUDE.md
**File**: `CLAUDE.md`
- Project overview: `**catalog-server**` → `**pezzottify-server**`
- All `cd catalog-server` commands → `cd pezzottify-server`
- Docker build example comments
- Architecture section header

### Step 10: Update README.md
**File**: `README.md`
- Component table references
- Links to catalog-server documentation

### Step 11: Update TODO.md
**File**: `TODO.md`
- Section header `[catalog-server]` → `[pezzottify-server]`

### Step 12: Update catalog-server/README.md
**File**: `pezzottify-server/README.md`
- Update all references to "catalog-server" in descriptions and examples

### Step 13: Update docs/*.md files
**Files**: `docs/agentic-ingestion.md`, `docs/composite-popularity-scoring.md`, `docs/FINGERPRINTS_SPEC.md`
- Update file path references from `catalog-server/src/` to `pezzottify-server/src/`

### Step 14: Update android/CLAUDE.md
**File**: `android/CLAUDE.md`
- References to catalog-server in integration test section

### Step 15: Update web/e2e-tests
**File**: `web/e2e-tests/global-setup.ts`
- Error message with cd command
**File**: `web/playwright.config.ts`
- Comment about starting service

## Verification

1. **Build the Rust project**:
   ```bash
   cd pezzottify-server
   cargo build --release
   ```

2. **Run tests**:
   ```bash
   cd pezzottify-server
   cargo test --features fast
   ```

3. **Build Docker image**:
   ```bash
   ./build-docker.sh
   ```

4. **Run docker-compose**:
   ```bash
   docker compose up --build
   ```

5. **Run Android integration tests**:
   ```bash
   cd android
   ./run-integration-tests.sh
   ```

6. **Verify CI/CD workflow** (push to branch or check workflow file syntax)

## Notes

- The Android app and Web frontend connect via configurable URLs (not hardcoded service names), so no changes are needed in the client applications themselves
- The binary name change means the executable will be `pezzottify-server` instead of `catalog-server`
- Consider whether to rename git remote branches (optional, not breaking)
