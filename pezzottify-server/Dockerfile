# ------------------------------------------
# 1️⃣ Web build stage
# ------------------------------------------
FROM node:20-alpine AS web-builder

# OIDC configuration (required for SSO login)
ARG VITE_OIDC_AUTHORITY
ARG VITE_OIDC_CLIENT_ID

WORKDIR /web
COPY web/package.json web/package-lock.json ./
COPY web/vendor/ ./vendor/
RUN npm ci
COPY web/ .

# Pass build args as environment variables for Vite build
ENV VITE_OIDC_AUTHORITY=${VITE_OIDC_AUTHORITY}
ENV VITE_OIDC_CLIENT_ID=${VITE_OIDC_CLIENT_ID}

RUN npm run build


# ------------------------------------------
# 2️⃣ Rust build stage
# ------------------------------------------
FROM rust:1.96-trixie AS rust-builder

# Give rustc worker threads more stack for optimized release compilation.
ENV RUST_MIN_STACK=33554432

# Build args for version info (detected on host, passed by build script)
ARG GIT_HASH=unknown
ARG GIT_DIRTY=0
ARG COMMIT_COUNT=0

WORKDIR /app

# Prepared by scripts/prepare-docker; the consumer never builds the engine here.
COPY .engine-build/source/ /simple-server/
COPY .engine-build/artifact/ /engine/
ENV SIMPLE_SERVER_ENGINE_DIR=/engine
COPY pezzottify-server/ pezzottify-server/
COPY VERSION VERSION

WORKDIR /app/pezzottify-server

RUN test "$(cat /engine/SOURCE_REVISION)" = "$(cat simple-server.rev)" \
    && cd /engine && sha256sum -c SHA256SUMS

# Build with version info from build args
RUN GIT_HASH="$GIT_HASH" GIT_DIRTY="$GIT_DIRTY" COMMIT_COUNT="$COMMIT_COUNT" cargo build --release --locked


# ------------------------------------------
# 3️⃣ Runtime stage (slim)
# ------------------------------------------
FROM debian:trixie-slim

# Install runtime dependencies:
# - libsqlite3-dev: SQLite dynamic loading
# - ffmpeg: ffprobe for content validation
# - ca-certificates: TLS certificate validation for HTTPS (OIDC, etc.)
RUN apt-get update && apt-get install -y libsqlite3-dev ffmpeg ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binaries from the builder stage
COPY --from=rust-builder /app/pezzottify-server/target/release/cli-auth /usr/local/bin/cli-auth
COPY --from=rust-builder /app/pezzottify-server/target/release/pezzottify-server /usr/local/bin/pezzottify-server

COPY --from=rust-builder /engine/libsimple_server_engine.so /usr/local/bin/lib/libsimple_server_engine.so.1

# Copy the web frontend from the web builder stage
COPY --from=web-builder /web/dist /app/web

# App will expect these directories
RUN mkdir -p /data/db
RUN mkdir -p /data/media

EXPOSE 3001
EXPOSE 9091

# Start the server
# Args: --db-dir <path> [--media-path <path>] [options]
# Optional: --downloader-url <URL> --downloader-timeout-sec <SECONDS>
#   Enables on-demand content fetching from an external downloader service
CMD ["pezzottify-server", "--db-dir", "/data/db", "--media-path", "/data/media", "--content-cache-age-sec=60", "--logging-level", "path", "--frontend-dir-path", "/app/web"]

# NOTE: Build context must be the repository root (not pezzottify-server/)
# Build with (from repo root):
#   ./build-docker.sh pezzottify-server
# Direct build: bash pezzottify-server/scripts/prepare-docker
#               docker build -f pezzottify-server/Dockerfile .
# Or manually:
#   GIT_HASH=$(git rev-parse --short HEAD) GIT_DIRTY=$(git status --porcelain | grep -q . && echo 1 || echo 0) \
#   docker compose up --build pezzottify-server
