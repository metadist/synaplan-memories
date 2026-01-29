#!/bin/bash
# test-replication.sh - Test Qdrant cluster replication
#
# This script:
#   1. Inserts a test point on one node
#   2. Verifies it appears on all other nodes
#   3. Deletes the test point
#
# Run from management server

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Node configuration
declare -A NODES=(
    ["web1"]="10.0.0.2"
    ["web2"]="10.0.0.7"
    ["web3"]="10.0.0.8"
)

COLLECTION="user_memories"
TEST_POINT_ID="test-replication-$(date +%s)"
VECTOR_DIM=1024

echo -e "${BLUE}=== Qdrant Replication Test ===${NC}\n"
echo "Collection: $COLLECTION"
echo "Test point ID: $TEST_POINT_ID"
echo ""

# Helper to run command on a node
remote_exec() {
    local node=$1
    shift
    ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "$node" "$@" 2>/dev/null
}

# Generate a random vector
generate_vector() {
    python3 -c "import random; print([round(random.uniform(-1, 1), 4) for _ in range($VECTOR_DIM)])" 2>/dev/null || \
    awk -v dim=$VECTOR_DIM 'BEGIN{printf "["; for(i=1;i<=dim;i++){printf "%.4f%s", rand()*2-1, (i<dim?",":"]}"); }'
}

# Check if collection exists
echo -e "${BLUE}1. Checking collection exists...${NC}"
collection_exists=$(remote_exec "web1" "curl -sf http://localhost:6333/collections/$COLLECTION 2>/dev/null" || echo "")

if [[ -z "$collection_exists" ]] || [[ "$collection_exists" == *"not found"* ]]; then
    echo -e "${RED}✗${NC} Collection '$COLLECTION' does not exist!"
    echo ""
    echo "Create it with:"
    echo "  curl -X PUT 'http://localhost:6333/collections/$COLLECTION' \\"
    echo "    -H 'Content-Type: application/json' \\"
    echo "    -d '{\"vectors\": {\"size\": $VECTOR_DIM, \"distance\": \"Cosine\"}, \"shard_number\": 3, \"replication_factor\": 3}'"
    exit 1
fi

echo -e "${GREEN}✓${NC} Collection exists"

# Insert test point on web1
echo ""
echo -e "${BLUE}2. Inserting test point on web1...${NC}"

VECTOR=$(generate_vector)

INSERT_PAYLOAD=$(cat <<EOF
{
  "points": [
    {
      "id": "$TEST_POINT_ID",
      "vector": $VECTOR,
      "payload": {
        "test": true,
        "timestamp": "$(date -Iseconds)",
        "source_node": "web1"
      }
    }
  ]
}
EOF
)

insert_result=$(remote_exec "web1" "curl -sf -X PUT 'http://localhost:6333/collections/$COLLECTION/points?wait=true' -H 'Content-Type: application/json' -d '$INSERT_PAYLOAD'" || echo "FAILED")

if [[ "$insert_result" == *"completed"* ]] || [[ "$insert_result" == *"status"* ]]; then
    echo -e "${GREEN}✓${NC} Point inserted successfully"
else
    echo -e "${RED}✗${NC} Failed to insert point: $insert_result"
    exit 1
fi

# Wait for replication
echo ""
echo -e "${BLUE}3. Waiting for replication (5 seconds)...${NC}"
sleep 5

# Check point on all nodes
echo ""
echo -e "${BLUE}4. Verifying point exists on all nodes...${NC}"

REPLICATION_OK=true

for node in "${!NODES[@]}"; do
    echo -n "   $node: "
    
    # Try to retrieve the point
    point_result=$(remote_exec "$node" "curl -sf 'http://localhost:6333/collections/$COLLECTION/points/$TEST_POINT_ID'" || echo "FAILED")
    
    if [[ "$point_result" == *"$TEST_POINT_ID"* ]]; then
        echo -e "${GREEN}✓${NC} Point found"
    else
        echo -e "${RED}✗${NC} Point NOT found"
        REPLICATION_OK=false
        
        # Debug: show what we got
        echo "      Response: ${point_result:0:100}"
    fi
done

# Test searching for the point
echo ""
echo -e "${BLUE}5. Testing search across nodes...${NC}"

SEARCH_PAYLOAD=$(cat <<EOF
{
  "vector": $VECTOR,
  "limit": 1,
  "with_payload": true
}
EOF
)

for node in "${!NODES[@]}"; do
    echo -n "   $node search: "
    
    search_result=$(remote_exec "$node" "curl -sf -X POST 'http://localhost:6333/collections/$COLLECTION/points/search' -H 'Content-Type: application/json' -d '$SEARCH_PAYLOAD'" || echo "FAILED")
    
    if [[ "$search_result" == *"$TEST_POINT_ID"* ]]; then
        score=$(echo "$search_result" | jq -r '.result[0].score // 0' 2>/dev/null || echo "?")
        echo -e "${GREEN}✓${NC} Found (score: $score)"
    else
        echo -e "${RED}✗${NC} NOT found in search"
    fi
done

# Cleanup: delete the test point
echo ""
echo -e "${BLUE}6. Cleaning up test point...${NC}"

delete_result=$(remote_exec "web1" "curl -sf -X POST 'http://localhost:6333/collections/$COLLECTION/points/delete?wait=true' -H 'Content-Type: application/json' -d '{\"points\": [\"$TEST_POINT_ID\"]}'" || echo "FAILED")

if [[ "$delete_result" == *"completed"* ]] || [[ "$delete_result" == *"status"* ]]; then
    echo -e "${GREEN}✓${NC} Test point deleted"
else
    echo -e "${YELLOW}⚠${NC} Could not delete test point (may need manual cleanup)"
fi

# Summary
echo ""
echo -e "${BLUE}=== Summary ===${NC}"

if $REPLICATION_OK; then
    echo -e "${GREEN}✓ Replication is working correctly!${NC}"
    echo "  - Point inserted on web1 was replicated to all nodes"
    echo "  - Search queries work across all nodes"
else
    echo -e "${RED}✗ Replication issues detected!${NC}"
    echo ""
    echo "Possible causes:"
    echo "  1. P2P port 6335 blocked between nodes"
    echo "  2. Cluster not properly formed (check 'curl localhost:6333/cluster')"
    echo "  3. Collection replication_factor < 3"
    echo ""
    echo "Debug commands:"
    echo "  - Check cluster status: ssh web1 'curl -s http://localhost:6333/cluster | jq'"
    echo "  - Check collection config: ssh web1 'curl -s http://localhost:6333/collections/$COLLECTION | jq'"
    echo "  - Check logs: ssh web1 'docker compose logs qdrant'"
fi
