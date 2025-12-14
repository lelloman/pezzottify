# Reverse Proxy Implementation Plan

## Overview

Move SSL termination from catalog-server to a Caddy reverse proxy, enabling:
- Automatic Let's Encrypt certificates (future)
- Standard ports (80/443) for external access
- Easy addition of future services via subdomains
- Internal HTTP communication (no SSL overhead between containers)

## Architecture (Final State)

```
                                    ┌─────────────────────────────────┐
                                    │      Docker Host                │
                                    │                                 │
Internet ──► :443 ──► [Caddy] ──────┼──► HTTP ──► [catalog-server]    │
                         │          │              :3001              │
                         │          │                                 │
                         └──────────┼──► HTTP ──► [future-service]    │
                                    │              :XXXX              │
                                    │                                 │
                                    │   (reverse-proxy-net network)   │
                                    └─────────────────────────────────┘
```

## Current State

- catalog-server handles SSL directly via `[ssl]` config section
- Port 3001 exposed to host (HTTPS)
- Uses `pezzottify-internal` and `monitoring` networks
- SSL certificates mounted from `/home/lelloman/pezzottify-cert/`

## Migration Strategy (Zero-Downtime)

The key insight: **Caddy can proxy to an HTTPS backend**. This allows us to:
1. Keep catalog-server running unchanged (HTTPS on 3001)
2. Start Caddy alongside it (HTTPS→HTTPS proxy)
3. Test that Caddy works
4. Switch traffic at the port level
5. Later: switch catalog-server to HTTP internally

**Rollback at any point**: Stop Caddy, revert port exposure. catalog-server never stopped.

### Migration Phases Overview

| Phase | catalog-server | Caddy | External Port | Downtime |
|-------|---------------|-------|---------------|----------|
| Current | HTTPS :3001 (exposed) | - | 3001 | - |
| Phase 2 | HTTPS :3001 (exposed) | HTTPS→HTTPS | 3001 + 443 | None |
| Phase 3 | HTTPS :3001 (internal) | HTTPS→HTTPS | 443 | ~seconds |
| Phase 4 | HTTP :3001 (internal) | HTTPS→HTTP | 443 | ~seconds |
| Phase 5 | HTTP :3001 (internal) | Let's Encrypt→HTTP | 443 | ~seconds |

---

## Implementation Steps

### Phase 1: Create homelab-infra Repository

#### 1.1 Create repo structure locally

```bash
mkdir -p ~/homelab-infra/caddy
cd ~/homelab-infra
git init
```

#### 1.2 Create .gitignore

Create `~/homelab-infra/.gitignore`:

```
# Secrets
.env
*.env

# SSL/TLS keys (should never be committed)
*.pem
*.key
*.crt

# Editor files
.idea/
.vscode/
*.swp
*~

# OS files
.DS_Store
Thumbs.db
```

#### 1.3 Create Caddyfile (HTTPS→HTTPS mode)

Create `~/homelab-infra/caddy/Caddyfile`:

```
# Pezzottify - catalog server
# Phase 2-3: HTTPS frontend → HTTPS backend (catalog-server keeps SSL)
pezzottify.YOURDOMAIN.com {
    tls /etc/caddy/ssl/cert.pem /etc/caddy/ssl/key.pem
    reverse_proxy https://catalog-server:3001 {
        transport http {
            tls_insecure_skip_verify
        }
    }
}

# Template for future services:
# service2.YOURDOMAIN.com {
#     tls /etc/caddy/ssl/cert.pem /etc/caddy/ssl/key.pem
#     reverse_proxy service2-container:PORT
# }
```

Replace `YOURDOMAIN.com` with your actual domain.

> **Note:** `tls_insecure_skip_verify` is safe here because we're connecting to our own
> internal service. This will be removed in Phase 4 when catalog-server switches to HTTP.

#### 1.4 Create Caddy docker-compose.yml

Create `~/homelab-infra/caddy/docker-compose.yml`:

```yaml
services:
  caddy:
    image: caddy:2
    container_name: caddy
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
      - /home/lelloman/pezzottify-cert:/etc/caddy/ssl:ro
      - caddy_data:/data
      - caddy_config:/config
    networks:
      - reverse-proxy-net

volumes:
  caddy_data:
  caddy_config:

networks:
  reverse-proxy-net:
    external: true
```

#### 1.5 Create README

Create `~/homelab-infra/README.md` with the following content:

```markdown
# Homelab Infrastructure

Private infrastructure configuration for homelab services.

## Structure

    homelab-infra/
    ├── caddy/              # Reverse proxy (SSL termination)
    │   ├── docker-compose.yml
    │   └── Caddyfile
    └── README.md

## Setup

### Prerequisites

Create the shared Docker network (once):

    docker network create reverse-proxy-net

### Starting Caddy

    cd caddy
    docker compose up -d

### Adding a new service

1. Ensure your service's docker-compose.yml includes the reverse-proxy-net network
2. Add entry to caddy/Caddyfile
3. Reload Caddy: docker compose exec caddy caddy reload --config /etc/caddy/Caddyfile
```

#### 1.6 Create GitHub repo and push

```bash
cd ~/homelab-infra
git add .
git commit -m "Initial setup: Caddy reverse proxy"

# Create private repo on GitHub (using gh CLI or web UI)
gh repo create homelab-infra --private --source=. --push

# Or manually:
# 1. Create private repo on github.com
# 2. git remote add origin git@github.com:YOURUSERNAME/homelab-infra.git
# 3. git push -u origin main
```

---

### Phase 2: Deploy Caddy (parallel with catalog-server)

In this phase, both catalog-server and Caddy run simultaneously. No changes to catalog-server yet.

#### 2.0 Pre-flight checks

```bash
# Verify DNS points to your server
dig pezzottify.YOURDOMAIN.com

# Ensure port 443 is available
sudo lsof -i :443
# Should return nothing (port free) or only your existing service

# Ensure port 443 is open in firewall
# Check your firewall rules - e.g., ufw status, iptables -L, etc.

# Verify certificate file names match what's in the Caddyfile
ls -la /home/lelloman/pezzottify-cert/
# The Caddyfile expects: cert.pem and key.pem
# If your files are named differently (e.g., fullchain.pem, privkey.pem),
# update the Caddyfile tls directive to match
```

#### 2.1 On the server: Clone homelab-infra

```bash
cd ~
git clone git@github.com:YOURUSERNAME/homelab-infra.git
```

#### 2.2 Create the shared Docker network

```bash
docker network create reverse-proxy-net
```

#### 2.3 Find catalog-server container name

The container name depends on your docker-compose project name. Find it:

```bash
docker ps --format "{{.Names}}" | grep catalog
# Example output: pezzottify_mirror-catalog-server-1
```

#### 2.4 Connect catalog-server to the network (without restart)

```bash
# Use the actual container name from step 2.3
# The --alias flag makes it reachable as "catalog-server" (matching the Caddyfile)
docker network connect --alias catalog-server reverse-proxy-net <CONTAINER_NAME>

# Example:
# docker network connect --alias catalog-server reverse-proxy-net pezzottify_mirror-catalog-server-1
```

> **Note:** The `--alias catalog-server` is important! The Caddyfile uses `catalog-server` as the
> hostname. Without the alias, Caddy couldn't resolve the container name.

> **⚠️ Warning:** This manual network connection is temporary. If you restart catalog-server
> (via `docker compose restart` or similar) before completing Phase 3, you'll need to re-run
> this command. Phase 3 makes the network connection permanent via docker-compose.yml.

#### 2.5 Start Caddy

```bash
cd ~/homelab-infra/caddy
docker compose up -d
docker compose logs -f  # Watch for startup
```

#### 2.6 Test Caddy (without switching traffic)

```bash
# Test via port 443 (Caddy) - should work now
curl -k -I https://pezzottify.YOURDOMAIN.com:443/v1/auth/session

# Original port 3001 still works
curl -k -I https://pezzottify.YOURDOMAIN.com:3001/v1/auth/session
```

Both should return 200/401 (depending on auth state). If Caddy fails, catalog-server is unaffected.

#### 2.7 Verify certificate chain

```bash
# Check Caddy is serving the certificate on 443
openssl s_client -connect pezzottify.YOURDOMAIN.com:443 </dev/null 2>/dev/null | openssl x509 -noout -subject
```

**At this point:**
- ✅ catalog-server: Running unchanged on HTTPS :3001 (exposed)
- ✅ Caddy: Running on :443, proxying to catalog-server via HTTPS
- ✅ Both ports work externally
- ✅ Rollback: Just stop Caddy, nothing else needed

---

### Phase 3: Switch Traffic to Caddy

Now we switch external traffic from port 3001 to port 443.

#### 3.1 Update pezzottify docker-compose.yml

In `/home/lelloman/pezzottify_mirror/docker-compose.yml`, modify the `catalog-server` service:

**Remove port exposure** (Caddy handles external traffic now):
```yaml
    # ports:
    #   - "3001:3001"  # REMOVE - Caddy handles this now
```

**Add container_name** (required for Caddy DNS resolution):
```yaml
    container_name: catalog-server
```

> **Why this matters:** With `container_name: catalog-server`, Docker DNS will resolve
> `catalog-server` to this container on all connected networks. This replaces the manual
> `--alias` we used in Phase 2.

**Add reverse-proxy-net to networks**:
```yaml
    networks:
      - monitoring
      - pezzottify-internal
      - reverse-proxy-net
```

**Update networks section** at the bottom:
```yaml
networks:
  monitoring:
    driver: bridge
  pezzottify-internal:
    name: pezzottify-internal
    driver: bridge
  reverse-proxy-net:
    external: true
```

#### 3.2 Restart catalog-server with new config

```bash
cd /path/to/pezzottify
docker compose up -d catalog-server
```

> **Downtime:** Only during this restart (~5-10 seconds)

#### 3.3 Verify traffic flows through Caddy

```bash
# Port 443 should work
curl -k -I https://pezzottify.YOURDOMAIN.com/v1/auth/session

# Port 3001 should NOT work externally anymore
curl -k -I https://pezzottify.YOURDOMAIN.com:3001/v1/auth/session  # Should fail
```

#### 3.4 Update firewall (if applicable)

If you have firewall rules exposing port 3001, you can now:
- Remove the 3001 rule (optional, since it's no longer exposed by Docker)
- Ensure 443 is open

#### 3.5 Commit pezzottify changes

```bash
cd /path/to/pezzottify
git add docker-compose.yml
git commit -m "Route traffic through Caddy reverse proxy"
git push
```

**At this point:**
- ✅ catalog-server: Running HTTPS :3001 (internal only)
- ✅ Caddy: Running on :443, proxying to catalog-server
- ✅ External traffic goes through Caddy
- ✅ Rollback: Re-add port exposure to docker-compose.yml, restart

---

### Phase 4: Switch catalog-server to HTTP (Optional Optimization)

This removes the unnecessary HTTPS overhead on the internal connection.

> **⚠️ Brief outage expected:** There's no way to do this atomically. Either Caddy or
> catalog-server will be misconfigured for a few seconds. The steps below minimize this window.

#### 4.1 Prepare all changes (don't apply yet)

**Update Caddyfile** - change from HTTPS to HTTP backend:

```
# Before (HTTPS→HTTPS)
pezzottify.YOURDOMAIN.com {
    tls /etc/caddy/ssl/cert.pem /etc/caddy/ssl/key.pem
    reverse_proxy https://catalog-server:3001 {
        transport http {
            tls_insecure_skip_verify
        }
    }
}

# After (HTTPS→HTTP)
pezzottify.YOURDOMAIN.com {
    tls /etc/caddy/ssl/cert.pem /etc/caddy/ssl/key.pem
    reverse_proxy catalog-server:3001
}
```

**Update catalog-server config.toml** (on server, not in repo):

The config.toml is created from config.example.toml on the server and is NOT version controlled.
Edit it directly on the server:

```bash
# On the server, edit the config file
nano /path/to/pezzottify/catalog-server/config.toml
# Or wherever your config.toml is located
```

Comment out the SSL section:

```toml
# [ssl]
# cert_path = "/etc/pezzottify/ssl/cert.pem"
# key_path = "/etc/pezzottify/ssl/key.pem"
```

**Update pezzottify docker-compose.yml** - remove SSL volume:

```yaml
    volumes:
      - /home/lelloman/pezzottify-catalog/:/data/db
      - /home/lelloman/pezzottify-catalog/:/data/media
      - ./catalog-server/config.toml:/etc/pezzottify/config.toml:ro
      # REMOVE: - /home/lelloman/pezzottify-cert/:/etc/pezzottify/ssl:ro
```

#### 4.2 Apply changes quickly (brief outage starts here)

Run these commands in quick succession:

```bash
# 1. Reload Caddy first (now expects HTTP, but catalog-server still HTTPS)
cd ~/homelab-infra/caddy
docker compose exec caddy caddy reload --config /etc/caddy/Caddyfile
# Requests will fail for a moment (502 errors)

# 2. Immediately restart catalog-server (now HTTP)
cd /path/to/pezzottify
docker compose up -d catalog-server
# Wait for it to come up (~5-10 seconds)
```

> **Downtime:** ~5-15 seconds while config mismatch exists + catalog-server restarts.
> During this window, requests will get 502 errors.

#### 4.3 Commit changes

```bash
# Commit homelab-infra changes
cd ~/homelab-infra
git add caddy/Caddyfile
git commit -m "Switch to HTTP backend for catalog-server"
git push

# Commit pezzottify changes (config.toml is not in repo, so only docker-compose.yml)
cd /path/to/pezzottify
git add docker-compose.yml
git commit -m "Remove SSL volume mount (Caddy handles TLS)"
git push
```

#### 4.4 Verify

```bash
curl -k -I https://pezzottify.YOURDOMAIN.com/v1/auth/session
```

**At this point:**
- ✅ catalog-server: Running HTTP :3001 (internal only)
- ✅ Caddy: HTTPS :443 → HTTP :3001
- ✅ No more double SSL overhead

---

## Rollback Procedures

### From Phase 2 (Caddy running parallel)

Nothing to rollback - catalog-server was never changed.

```bash
# Just stop Caddy if you want
cd ~/homelab-infra/caddy && docker compose down
```

### From Phase 3 (Traffic through Caddy, catalog-server still HTTPS)

```bash
# 1. Restore port exposure in pezzottify docker-compose.yml
# Add back:
#   ports:
#     - "3001:3001"

# 2. Restart catalog-server
cd /path/to/pezzottify
docker compose up -d catalog-server

# 3. Stop Caddy
cd ~/homelab-infra/caddy && docker compose down

# External traffic now goes to :3001 again
```

### From Phase 4 (catalog-server on HTTP)

```bash
# 1. Restore Caddyfile to HTTPS→HTTPS mode:
#    reverse_proxy https://catalog-server:3001 {
#        transport http {
#            tls_insecure_skip_verify
#        }
#    }

# 2. Reload Caddy
cd ~/homelab-infra/caddy
docker compose exec caddy caddy reload --config /etc/caddy/Caddyfile

# 3. Restore SSL config in config.toml on server (uncomment [ssl] section)
#    Edit: /path/to/pezzottify/catalog-server/config.toml

# 4. Restore SSL volume mount in pezzottify docker-compose.yml:
#    - /home/lelloman/pezzottify-cert/:/etc/pezzottify/ssl:ro

# 5. Restart catalog-server
cd /path/to/pezzottify
docker compose up -d catalog-server

# Now you're back to Phase 3 state (Caddy → HTTPS catalog-server)
# To fully rollback to pre-Caddy, follow "From Phase 3" steps
```

---

## Phase 5: Switch to Let's Encrypt (Future)

Once everything is stable, switching to Let's Encrypt is simple.

### 5.1 Update Caddyfile

Remove the `tls` directive:

```
# Before (self-signed)
pezzottify.YOURDOMAIN.com {
    tls /etc/caddy/ssl/cert.pem /etc/caddy/ssl/key.pem
    reverse_proxy catalog-server:3001
}

# After (Let's Encrypt - automatic)
pezzottify.YOURDOMAIN.com {
    reverse_proxy catalog-server:3001
}
```

### 5.2 Ensure port 80 is open

Let's Encrypt ACME challenge requires port 80.

### 5.3 Restart Caddy

```bash
cd ~/homelab-infra/caddy
docker compose down
docker compose up -d
docker compose logs -f  # Watch for certificate acquisition
```

### 5.4 Update Android app

Remove certificate pinning since Let's Encrypt certs are CA-signed:
1. Delete `android/ssl_pin.txt` if it exists
2. Rebuild the app

### 5.5 Optional cleanup

Remove the certificate volume mount from Caddy's docker-compose.yml.

---

## Files Changed Summary

| Phase | File | Location | Action | Version Controlled |
|-------|------|----------|--------|-------------------|
| 1 | `.gitignore` | homelab-infra/ | Create | Yes |
| 1 | `README.md` | homelab-infra/ | Create | Yes |
| 1 | `docker-compose.yml` | homelab-infra/caddy/ | Create | Yes |
| 1 | `Caddyfile` | homelab-infra/caddy/ | Create | Yes |
| 3 | `docker-compose.yml` | pezzottify/ | Modify (remove port, add network) | Yes |
| 4 | `Caddyfile` | homelab-infra/caddy/ | Modify (HTTP backend) | Yes |
| 4 | `config.toml` | server filesystem | Modify (remove SSL) | No |
| 4 | `docker-compose.yml` | pezzottify/ | Modify (remove SSL volume) | Yes |

---

## Checklists

### Phase 1 Checklist (Create repo)

- [ ] Created `homelab-infra` directory with git init
- [ ] Created `.gitignore`
- [ ] Created `caddy/Caddyfile` (HTTPS→HTTPS mode)
- [ ] Created `caddy/docker-compose.yml`
- [ ] Created `README.md`
- [ ] Created private GitHub repo and pushed

### Phase 2 Checklist (Deploy Caddy parallel)

- [ ] Verified DNS points to server
- [ ] Verified port 443 is open in firewall
- [ ] Verified certificate file names match Caddyfile
- [ ] Cloned homelab-infra on server
- [ ] Created `reverse-proxy-net` Docker network
- [ ] Found actual catalog-server container name (`docker ps`)
- [ ] Connected container to reverse-proxy-net with alias
- [ ] Started Caddy
- [ ] Verified both :443 and :3001 work

### Phase 3 Checklist (Switch traffic)

- [ ] Updated pezzottify docker-compose.yml (remove port, add network, add container_name)
- [ ] Restarted catalog-server
- [ ] Verified :443 works, :3001 no longer exposed
- [ ] Updated firewall rules if needed
- [ ] Committed and pushed pezzottify changes

### Phase 4 Checklist (HTTP internal - optional)

- [ ] Updated Caddyfile (HTTP backend)
- [ ] Updated config.toml on server (remove SSL section)
- [ ] Updated pezzottify docker-compose.yml (remove SSL volume)
- [ ] Reloaded Caddy
- [ ] Restarted catalog-server
- [ ] Verified everything works
- [ ] Committed and pushed homelab-infra Caddyfile changes
- [ ] Committed and pushed pezzottify docker-compose.yml changes

### Phase 5 Checklist (Let's Encrypt - future)

- [ ] Port 80 open
- [ ] Removed `tls` directive from Caddyfile
- [ ] Restarted Caddy
- [ ] Verified Let's Encrypt certificate
- [ ] Rebuilt Android app without cert pinning
