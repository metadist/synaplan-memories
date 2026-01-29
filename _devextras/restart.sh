#!/bin/bash
# restart.sh - Restart Qdrant memory services on this node
#
# Quick restart without rebuilding. Use start-node*.sh if you need to rebuild.

set -euo pipefail

echo "Restarting Qdrant memory services..."
docker compose restart

echo "Waiting for health check..."
sleep 5

if curl -sf http://localhost:6333/healthz > /dev/null 2>&1; then
    echo "Qdrant is healthy."
else
    echo "Warning: Health check failed. Check logs:"
    echo "  docker compose logs"
fi
