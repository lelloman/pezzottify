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

WORKDIR /catalog-server
COPY catalog-server/ .

# Build the release binary
RUN cargo build --release


# ------------------------------------------
# 3️⃣ Runtime stage (slim)
# ------------------------------------------
FROM debian:stable-slim

# Install SQLite if your app loads sqlite dynamically
RUN apt-get update && apt-get install -y libsqlite3-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binaries from the builder stage
COPY --from=rust-builder /catalog-server/target/release/cli-auth /usr/local/bin/cli-auth
COPY --from=rust-builder /catalog-server/target/release/catalog-server /usr/local/bin/catalog-server

# Copy the web frontend from the web builder stage
COPY --from=web-builder /web/dist /app/web

# App will expect these directories
RUN mkdir -p /data/db
RUN mkdir -p /data/catalog

EXPOSE 3001
EXPOSE 9091

# Start the server with frontend serving enabled
CMD ["catalog-server", "/data/catalog", "/data/db/user.db", "--content-cache-age-sec=60", "--logging-level", "path", "--frontend-dir-path", "/app/web"]

# NOTE: Build context must be the repository root (not catalog-server/)
# Build with (from repo root):
# docker build -t catalog-server -f catalog-server/Dockerfile .

# Run with:
# docker run -d --rm --name pezzottify-catalog-server -p 3001:3001 -v /path/to/catalog:/data/catalog -v /path/to/db:/data/db catalog-server
