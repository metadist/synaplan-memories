#!/bin/bash
# stop.sh - Stop Qdrant memory services on this node
#
# This only stops containers defined in this docker-compose.yml
# It does NOT affect other containers (like synaplan-platform)

set -euo pipefail

echo "Stopping Qdrant memory services..."
docker compose down

echo "Done."
