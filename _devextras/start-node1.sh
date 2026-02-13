#!/bin/bash
# start-node1.sh - Start Qdrant cluster BOOTSTRAP node on synweb100 (web1)
#
# This node MUST be started first when initializing the cluster.
# After initial cluster formation, restart order doesn't matter.
#
# Host: synweb100 / 10.0.0.2
# Run from: /netroot/synaplanCluster/synaplan-memories/

set -euo pipefail

NODE_IP="10.0.0.2"

echo "Starting Qdrant Memory Service on synweb100 (Bootstrap Node)..."
echo "  Node IP: ${NODE_IP}"
echo "  Role: Bootstrap (Leader)"

# Verify .env exists
if [ ! -f "qdrant-service/.env" ]; then
    echo "ERROR: qdrant-service/.env not found!"
    echo "Copy qdrant-service/.env.example to qdrant-service/.env and set SERVICE_API_KEY."
    exit 1
fi

# Verify local storage exists (NOT on NFS!)
if [ ! -d "/qdrant/storage" ]; then
    echo "Creating /qdrant/storage..."
    sudo mkdir -p /qdrant/storage
    sudo chown -R 1000:1000 /qdrant
fi

# Check it's not on NFS
if mount | grep -q "/qdrant.*nfs"; then
    echo "ERROR: /qdrant/ is on NFS! Qdrant requires local SSD storage."
    exit 1
fi

# Cluster configuration
export QDRANT_CLUSTER_ENABLED=true
export QDRANT_COMMAND="./qdrant --uri http://${NODE_IP}:6335"
export QDRANT_STORAGE_PATH=/qdrant/storage
export OLLAMA_BASE_URL=http://10.0.1.10:11434

# Bind REST API to internal network IP (not all interfaces)
export QDRANT_REST_PORT=${NODE_IP}:6333

echo "  Qdrant Storage: /qdrant/storage (local)"
echo "  REST API: http://${NODE_IP}:6333"

# Build and start (pulls qdrant image, builds qdrant-service from source)
# Use --force-recreate to ensure containers are refreshed
docker compose up --build --pull always --force-recreate -d

echo ""
echo "Waiting for Qdrant to start..."
sleep 10

# Check status
if curl -sf "http://${NODE_IP}:6333/cluster" > /dev/null 2>&1; then
    echo "Qdrant cluster status:"
    curl -s "http://${NODE_IP}:6333/cluster" | jq -r '.result.status // "unknown"' 2>/dev/null || echo "(install jq for formatted output)"
else
    echo "Warning: Could not check cluster status. Check logs:"
    echo "  docker compose logs qdrant"
fi
