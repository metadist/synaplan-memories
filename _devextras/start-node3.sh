#!/bin/bash
# start-node3.sh - Start Qdrant cluster JOINING node on synweb102 (web3)
#
# Ensure synweb100 (bootstrap node) is running before starting this node
# for the first time. After cluster formation, restart order doesn't matter.
#
# Host: synweb102 / 10.0.0.8
# Run from: /netroot/synaplanCluster/synaplan-memories/

set -euo pipefail

NODE_IP="10.0.0.8"
BOOTSTRAP_IP="10.0.0.2"

echo "Starting Qdrant Memory Service on synweb102 (Joining Node)..."
echo "  Node IP: ${NODE_IP}"
echo "  Bootstrap: ${BOOTSTRAP_IP}"

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

# Check bootstrap node (only matters for initial cluster formation)
if ! curl -sf --connect-timeout 5 http://${BOOTSTRAP_IP}:6335 > /dev/null 2>&1; then
    echo "Note: Bootstrap node (${BOOTSTRAP_IP}:6335) not responding."
    echo "This is OK if the cluster was already formed."
fi

# Cluster configuration
export QDRANT_CLUSTER_ENABLED=true
export QDRANT_COMMAND="--bootstrap http://${BOOTSTRAP_IP}:6335"
export QDRANT_STORAGE_PATH=/qdrant/storage
export OLLAMA_BASE_URL=http://10.0.1.10:11434

# Don't expose REST API to all interfaces in production
export QDRANT_REST_PORT=127.0.0.1:6333

echo "  Qdrant Storage: /qdrant/storage (local)"

# Build and start (pulls qdrant image, builds qdrant-service from source)
# Use --force-recreate to ensure containers are refreshed
docker compose up --build --pull always --force-recreate -d

echo ""
echo "Waiting for Qdrant to join cluster..."
sleep 15

# Check status
if curl -sf http://localhost:6333/cluster > /dev/null 2>&1; then
    echo "Qdrant cluster status:"
    curl -s http://localhost:6333/cluster | jq -r '"Peers: \(.result.peers | keys | length)", "Status: \(.result.status // "unknown")"' 2>/dev/null || echo "(install jq)"
else
    echo "Warning: Could not check cluster status. Check logs:"
    echo "  docker compose logs qdrant"
fi
