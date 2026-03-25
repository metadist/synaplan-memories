#!/bin/bash
# test-platform-memory-e2e.sh - End-to-end test: Platform -> Qdrant
#
# Tests the complete flow that the Synaplan backend uses to access Qdrant.
# Run from management server or directly on a web node.

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

echo -e "${BLUE}=== Platform -> Qdrant E2E Test ===${NC}\n"

# Helper to run command on a node
remote_exec() {
    local node=$1
    shift
    ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "$node" "$@" 2>/dev/null
}

for node in "${!NODES[@]}"; do
    echo -e "${BLUE}=== Testing $node ===${NC}\n"
    
    # Check if platform is running
    platform_status=$(remote_exec "$node" "docker inspect -f '{{.State.Status}}' synaplan-platform" || echo "not_found")
    
    if [[ "$platform_status" != "running" ]]; then
        echo -e "${YELLOW}warn${NC} synaplan-platform not running on $node - skipping"
        echo ""
        continue
    fi
    
    # Get QDRANT_URL from platform
    qdrant_url=$(remote_exec "$node" "docker exec synaplan-platform printenv QDRANT_URL" || echo "NOT_SET")
    
    echo "Platform config:"
    echo "  QDRANT_URL: $qdrant_url"
    echo ""
    
    # Test 1: Health check
    echo "1. Qdrant health check:"
    health=$(remote_exec "$node" "docker exec synaplan-platform curl -sf --connect-timeout 5 '$qdrant_url/healthz'" || echo "FAILED")
    if [[ "$health" != "FAILED" ]]; then
        echo -e "   ${GREEN}ok${NC} Health endpoint accessible"
    else
        echo -e "   ${RED}FAIL${NC} Health endpoint NOT accessible"
    fi
    
    # Test 2: Collections list
    echo ""
    echo "2. Collections endpoint:"
    collections=$(remote_exec "$node" "docker exec synaplan-platform curl -sf --connect-timeout 5 '$qdrant_url/collections'" || echo "FAILED")
    if [[ "$collections" == *"collections"* ]]; then
        echo -e "   ${GREEN}ok${NC} Collections endpoint accessible"
        coll_count=$(echo "$collections" | jq -r '.result.collections | length' 2>/dev/null || echo "?")
        echo "   Collections: $coll_count"
    else
        echo -e "   ${RED}FAIL${NC} Collections endpoint NOT accessible"
    fi
    
    # Test 3: Check specific collections
    echo ""
    echo "3. Collection details:"
    for coll in user_memories user_documents; do
        coll_info=$(remote_exec "$node" "docker exec synaplan-platform curl -sf --connect-timeout 5 '$qdrant_url/collections/$coll'" || echo "FAILED")
        if [[ "$coll_info" == *"result"* ]]; then
            points=$(echo "$coll_info" | jq -r '.result.points_count // 0' 2>/dev/null || echo "?")
            status=$(echo "$coll_info" | jq -r '.result.status // "?"' 2>/dev/null || echo "?")
            echo -e "   ${GREEN}ok${NC} $coll: $points points (status: $status)"
        else
            echo -e "   ${YELLOW}--${NC} $coll: not found"
        fi
    done
    
    echo ""
done

echo -e "${BLUE}=== Summary ===${NC}\n"
echo "Synaplan connects to Qdrant directly via QDRANT_URL (port 6333)."
echo "The backend uses QdrantClientDirect to manage collections for:"
echo "  - user_memories (AI profiling)"
echo "  - user_documents (file RAG)"
echo ""
echo "To verify from Synaplan UI:"
echo "  - Go to Admin > Settings > Vector Storage"
echo ""
