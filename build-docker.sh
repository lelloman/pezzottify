#!/bin/bash
# Build Docker image with git version info detected from host

set -e

# Detect git hash (short)
export GIT_HASH=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")

# Detect dirty state: 1 if dirty, 0 if clean
if git status --porcelain 2>/dev/null | grep -q .; then
    export GIT_DIRTY=1
else
    export GIT_DIRTY=0
fi

# Detect commit count for version
export COMMIT_COUNT=$(git rev-list --count HEAD 2>/dev/null || echo "0")

echo "Building with GIT_HASH=$GIT_HASH GIT_DIRTY=$GIT_DIRTY COMMIT_COUNT=$COMMIT_COUNT"

# Pass all arguments to docker compose (e.g., "-d", etc.)
docker compose up --build "$@"
