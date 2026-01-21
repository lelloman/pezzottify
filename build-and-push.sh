#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IMAGE="registry.homelab:5000/catalog-server"
TAG="${1:-latest}"
HOMELAB_DIR="${HOMELAB_DIR:-$HOME/homelab}"

# Get git info for version embedding
GIT_HASH=$(git -C "$SCRIPT_DIR" rev-parse --short HEAD 2>/dev/null || echo "unknown")
GIT_DIRTY=$(git -C "$SCRIPT_DIR" status --porcelain | grep -q . && echo 1 || echo 0)
COMMIT_COUNT=$(git -C "$SCRIPT_DIR" rev-list --count HEAD 2>/dev/null || echo "0")

# Load pezzottify env file for web OIDC build args
if [ -f "$HOMELAB_DIR/pezzottify/.env" ]; then
    source "$HOMELAB_DIR/pezzottify/.env"
fi

echo "Building catalog-server (version info: hash=$GIT_HASH, commits=$COMMIT_COUNT, dirty=$GIT_DIRTY)..."
docker build -t "$IMAGE:$TAG" \
    --build-arg GIT_HASH="$GIT_HASH" \
    --build-arg GIT_DIRTY="$GIT_DIRTY" \
    --build-arg COMMIT_COUNT="$COMMIT_COUNT" \
    --build-arg VITE_OIDC_AUTHORITY="https://auth.lelloman.com" \
    --build-arg VITE_OIDC_CLIENT_ID="${PEZZOTTIFY_WEB_CLIENT_ID:?Set PEZZOTTIFY_WEB_CLIENT_ID in $HOMELAB_DIR/pezzottify/.env}" \
    -f "$SCRIPT_DIR/catalog-server/Dockerfile" "$SCRIPT_DIR"

echo "Pushing $IMAGE:$TAG..."
docker push "$IMAGE:$TAG"

echo "Pushed $IMAGE:$TAG"
