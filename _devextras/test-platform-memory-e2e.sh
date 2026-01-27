#!/bin/bash
# test-platform-memory-e2e.sh - End-to-end test: Platform -> qdrant-service -> Qdrant
#
# Tests the complete flow that the Synaplan backend uses to access the memory service.
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

echo -e "${BLUE}=== Platform → Memory Service E2E Test ===${NC}\n"

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
        echo -e "${YELLOW}⚠${NC} synaplan-platform not running on $node - skipping"
        echo ""
        continue
    fi
    
    # Get the API key from platform
    api_key=$(remote_exec "$node" "docker exec synaplan-platform printenv QDRANT_SERVICE_API_KEY" || echo "")
    qdrant_url=$(remote_exec "$node" "docker exec synaplan-platform printenv QDRANT_SERVICE_URL" || echo "")
    
    echo "Platform config:"
    echo "  QDRANT_SERVICE_URL: $qdrant_url"
    if [[ -n "$api_key" ]]; then
        echo "  QDRANT_SERVICE_API_KEY: ${api_key:0:4}...${api_key: -4}"
    else
        echo -e "  QDRANT_SERVICE_API_KEY: ${RED}NOT SET${NC}"
    fi
    echo ""
    
    # Test 1: Health check without API key
    echo "1. Health check (no auth):"
    health_no_auth=$(remote_exec "$node" "docker exec synaplan-platform curl -sf --connect-timeout 5 '$qdrant_url/health'" || echo "FAILED")
    if [[ "$health_no_auth" == *"status"* ]] || [[ "$health_no_auth" == *"ok"* ]]; then
        echo -e "   ${GREEN}✓${NC} Health endpoint accessible"
    else
        echo -e "   ${RED}✗${NC} Health endpoint NOT accessible"
        echo "   Response: $health_no_auth"
    fi
    
    # Test 2: Health check with API key
    echo ""
    echo "2. Health check (with X-API-Key):"
    health_auth=$(remote_exec "$node" "docker exec synaplan-platform curl -sf --connect-timeout 5 -H 'X-API-Key: $api_key' '$qdrant_url/health'" || echo "FAILED")
    if [[ "$health_auth" == *"status"* ]] || [[ "$health_auth" == *"ok"* ]]; then
        echo -e "   ${GREEN}✓${NC} Authenticated health check passed"
    else
        echo -e "   ${RED}✗${NC} Authenticated health check FAILED"
        echo "   Response: $health_auth"
    fi
    
    # Test 3: Stats endpoint (requires auth)
    echo ""
    echo "3. Stats endpoint (requires auth):"
    stats=$(remote_exec "$node" "docker exec synaplan-platform curl -sf --connect-timeout 5 -H 'X-API-Key: $api_key' '$qdrant_url/stats'" || echo "FAILED")
    if [[ "$stats" == *"points_count"* ]] || [[ "$stats" == *"collection"* ]]; then
        echo -e "   ${GREEN}✓${NC} Stats endpoint accessible"
        points=$(echo "$stats" | jq -r '.points_count // "?"' 2>/dev/null || echo "?")
        echo "   Points in collection: $points"
    else
        echo -e "   ${RED}✗${NC} Stats endpoint NOT accessible"
        echo "   Response: ${stats:0:100}"
    fi
    
    # Test 4: Check API key mismatch
    echo ""
    echo "4. API key validation test:"
    
    # Get the actual SERVICE_API_KEY from qdrant-service
    svc_key=$(remote_exec "$node" "docker exec synaplan-qdrant-service printenv SERVICE_API_KEY" || echo "")
    
    if [[ -z "$svc_key" ]]; then
        echo -e "   ${YELLOW}⚠${NC} SERVICE_API_KEY not set in qdrant-service"
    elif [[ "$api_key" == "$svc_key" ]]; then
        echo -e "   ${GREEN}✓${NC} API keys match"
    else
        echo -e "   ${RED}✗${NC} API KEY MISMATCH!"
        echo "   Platform key: ${api_key:0:4}...${api_key: -4}"
        echo "   Service key:  ${svc_key:0:4}...${svc_key: -4}"
    fi
    
    # Test 5: Simulate what Synaplan backend does - memory check endpoint
    echo ""
    echo "5. Simulating Synaplan memory-service/check API call:"
    
    # The Synaplan backend checks /health and /stats
    # We simulate this from inside the platform container
    
    check_cmd="curl -sf -w '%{http_code}' -H 'X-API-Key: $api_key' '$qdrant_url/health' -o /dev/null"
    http_code=$(remote_exec "$node" "docker exec synaplan-platform sh -c \"$check_cmd\"" || echo "000")
    
    if [[ "$http_code" == "200" ]]; then
        echo -e "   ${GREEN}✓${NC} Memory service is available (HTTP $http_code)"
    else
        echo -e "   ${RED}✗${NC} Memory service unavailable (HTTP $http_code)"
    fi
    
    echo ""
done

echo -e "${BLUE}=== Summary ===${NC}\n"
echo "The memory service check in Synaplan works like this:"
echo "  1. Backend reads QDRANT_SERVICE_URL and QDRANT_SERVICE_API_KEY from env"
echo "  2. Makes HTTP request to {QDRANT_SERVICE_URL}/health"
echo "  3. Makes HTTP request to {QDRANT_SERVICE_URL}/stats"
echo "  4. If both succeed, memory service is 'available'"
echo ""
echo "Common issues:"
echo "  - API key mismatch: Platform's QDRANT_SERVICE_API_KEY != Service's SERVICE_API_KEY"
echo "  - Network issue: docker-host:8090 not reachable from platform container"
echo "  - Service down: qdrant-service container not running or unhealthy"
echo ""
echo "To verify from Synaplan UI:"
echo "  - Go to Admin > Settings > Memory Service"
echo "  - Should show 'configured: true' and 'available: true'"
