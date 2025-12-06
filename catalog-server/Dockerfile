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

WORKDIR /app

# Copy git info for version and dirty detection
COPY .git/HEAD .git/HEAD
COPY .git/refs .git/refs
COPY .git/index .git/index
COPY .git/objects .git/objects

# Copy source and detect dirty state
COPY catalog-server/ catalog-server/
RUN GIT_DIRTY=$(git -C /app status --porcelain catalog-server/ 2>/dev/null | grep -q . && echo 1 || echo 0) && \
    echo "GIT_DIRTY=$GIT_DIRTY" >> /tmp/build-env && \
    echo "Detected GIT_DIRTY=$GIT_DIRTY"

WORKDIR /app/catalog-server

# Build the release binary with dirty detection
RUN . /tmp/build-env && cargo build --release


# ------------------------------------------
# 3️⃣ Runtime stage (slim)
# ------------------------------------------
FROM debian:stable-slim

# Install SQLite if your app loads sqlite dynamically
RUN apt-get update && apt-get install -y libsqlite3-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binaries from the builder stage
COPY --from=rust-builder /app/catalog-server/target/release/cli-auth /usr/local/bin/cli-auth
COPY --from=rust-builder /app/catalog-server/target/release/catalog-server /usr/local/bin/catalog-server

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
# docker build -t catalog-server -f catalog-server/Dockerfile .

# Run with:
# docker run -d --rm --name pezzottify-catalog-server -p 3001:3001 -v /path/to/catalog:/data/catalog -v /path/to/db:/data/db -v /path/to/media:/data/media catalog-server
