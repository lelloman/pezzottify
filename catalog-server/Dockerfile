# ------------------------------------------
# 1️⃣ Web build stage
# ------------------------------------------
FROM node:20-alpine AS web-builder

WORKDIR /web
COPY web/package*.json ./
RUN npm ci
COPY web/ .
RUN npm run build


# ------------------------------------------
# 2️⃣ Rust build stage
# ------------------------------------------
FROM rust:1.91.1 AS rust-builder

# Build args for version info (detected on host, passed by build script)
ARG GIT_HASH=unknown
ARG GIT_DIRTY=0

WORKDIR /app

COPY catalog-server/ catalog-server/

WORKDIR /app/catalog-server

# Build with version info from build args
# Use BuildKit cache mounts for cargo registry and target directory
# Binaries are copied out because cache mounts aren't available in later stages
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/catalog-server/target \
    GIT_HASH="$GIT_HASH" GIT_DIRTY="$GIT_DIRTY" cargo build --release && \
    cp target/release/cli-auth /tmp/ && \
    cp target/release/catalog-server /tmp/


# ------------------------------------------
# 3️⃣ Runtime stage (slim)
# ------------------------------------------
FROM debian:stable-slim

# Install SQLite if your app loads sqlite dynamically
RUN apt-get update && apt-get install -y libsqlite3-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binaries from the builder stage
COPY --from=rust-builder /tmp/cli-auth /usr/local/bin/cli-auth
COPY --from=rust-builder /tmp/catalog-server /usr/local/bin/catalog-server

# Copy the web frontend from the web builder stage
COPY --from=web-builder /web/dist /app/web

# App will expect these directories
RUN mkdir -p /data/db
RUN mkdir -p /data/media

EXPOSE 3001
EXPOSE 9091

# Start the server
# Args: <catalog-db-path> <user-db-path> [--media-path <path>] [options]
# Optional: --downloader-url <URL> --downloader-timeout-sec <SECONDS>
#   Enables on-demand content fetching from an external downloader service
CMD ["catalog-server", "/data/db/catalog.db", "/data/db/user.db", "--media-path", "/data/media", "--content-cache-age-sec=60", "--logging-level", "path", "--frontend-dir-path", "/app/web"]

# NOTE: Build context must be the repository root (not catalog-server/)
# Build with (from repo root):
#   ./build-docker.sh catalog-server
# Or manually:
#   GIT_HASH=$(git rev-parse --short HEAD) GIT_DIRTY=$(git status --porcelain | grep -q . && echo 1 || echo 0) \
#   docker compose up --build catalog-server
