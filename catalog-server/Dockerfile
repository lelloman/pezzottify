# ------------------------------------------
# 1️⃣ Build stage
# ------------------------------------------
FROM rust:1.82 AS builder

WORKDIR /catalog-server
COPY . .

# Build the release binary
RUN cargo build --release


# ------------------------------------------
# 2️⃣ Runtime stage (slim, ~20MB)
# ------------------------------------------
FROM debian:stable-slim

# Install SQLite if your app loads sqlite dynamically
RUN apt-get update && apt-get install -y libsqlite3-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /catalog-server

# Copy the binaries from the builder stage
COPY --from=builder /catalog-server/target/release/cli-auth /usr/local/bin/cli-auth
COPY --from=builder /catalog-server/target/release/cli-search /usr/local/bin/cli-search
COPY --from=builder /catalog-server/target/release/catalog-server /usr/local/bin/catalog-server

# App will expect these directories
RUN mkdir -p /data/db
RUN mkdir -p /data/catalog

EXPOSE 3001

# Start the server
CMD ["catalog-server", "/data/catalog", "/data/db/user.db", "--content-cache-age-sec=60", "--logging-level", "path"]

# Build with:
# docker build -t catalog-server .

# Run with:
# docker run -d -p 3001:3001 -v /home/lelloman/pezzottify-catalog/:/data/db -v /home/lelloman/pezzottify-catalog:/data/catalog catalog-server
