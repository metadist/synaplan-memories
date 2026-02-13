#!/bin/bash
# restart.sh - Restart Qdrant memory services on this node
#
# Quick restart without rebuilding. Use start-node*.sh if you need to rebuild.
#
# Usage: ./restart.sh [QDRANT_HOST]

set -euo pipefail

QDRANT_HOST="${1:-10.0.0.2}"

echo "Restarting Qdrant memory services..."
docker compose restart

echo "Waiting for health check..."
sleep 5

if curl -sf --connect-timeout 5 "http://${QDRANT_HOST}:6333/healthz" > /dev/null 2>&1; then
    echo "Qdrant is healthy."
else
    echo "Warning: Health check failed. Check logs:"
    echo "  docker compose logs"
fi
