#!/bin/bash
# setup-collection.sh - Create Qdrant collection with cluster replication
#
# Run this ONCE after the cluster is formed.
# Run from any node (web1, web2, or web3).

set -euo pipefail

COLLECTION_NAME="${1:-user_memories}"
VECTOR_SIZE="${2:-1024}"

echo "Creating collection: ${COLLECTION_NAME}"
echo "  Vector size: ${VECTOR_SIZE}"
echo "  Shards: 3"
echo "  Replication factor: 3"
echo "  Write consistency: 2"

# Check if collection exists
if curl -sf "http://localhost:6333/collections/${COLLECTION_NAME}" > /dev/null 2>&1; then
    echo ""
    echo "Collection '${COLLECTION_NAME}' already exists."
    echo "Current info:"
    curl -s "http://localhost:6333/collections/${COLLECTION_NAME}" | jq '.result.config'
    exit 0
fi

# Create collection
curl -X PUT "http://localhost:6333/collections/${COLLECTION_NAME}" \
  -H "Content-Type: application/json" \
  -d "{
    \"vectors\": {
      \"size\": ${VECTOR_SIZE},
      \"distance\": \"Cosine\"
    },
    \"shard_number\": 3,
    \"replication_factor\": 3,
    \"write_consistency_factor\": 2
  }"

echo ""
echo "Collection created. Verifying..."

# Verify
curl -s "http://localhost:6333/collections/${COLLECTION_NAME}/cluster" | jq '.result.local_shards | length' 2>/dev/null && echo "shards on this node"

echo ""
echo "Done. Check cluster distribution:"
echo "  curl http://localhost:6333/collections/${COLLECTION_NAME}/cluster | jq"
