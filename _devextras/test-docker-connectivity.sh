#!/bin/bash
# test-docker-connectivity.sh - Test Docker internal networking for Qdrant
#
# Tests the connection path:
#   synaplan-platform (backend) -> docker-host:6333 -> synaplan-qdrant
#
# Run from management server or directly on a web node

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

echo -e "${BLUE}=== Docker Internal Connectivity Test ===${NC}\n"

for node in "${!NODES[@]}"; do
    echo -e "\n${BLUE}=== $node (${NODES[$node]}) ===${NC}\n"
    
    # 1. Check host's port 6333
    echo "1. Host port 6333 (Qdrant REST):"
    if ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "$node" "curl -sf http://${NODES[$node]}:6333/healthz > /dev/null 2>&1" 2>/dev/null; then
        echo -e "   ${GREEN}ok${NC} Host :6333 reachable"
    else
        echo -e "   ${RED}FAIL${NC} Host :6333 NOT reachable"
    fi
    
    # 2. Check qdrant container is running
    echo ""
    echo "2. qdrant container:"
    container_status=$(ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "$node" "docker inspect -f '{{.State.Status}}' synaplan-qdrant" 2>/dev/null || echo "not_found")
    
    if [[ "$container_status" == "running" ]]; then
        echo -e "   ${GREEN}ok${NC} Status: $container_status"
    else
        echo -e "   ${RED}FAIL${NC} Status: $container_status"
    fi
    
    # 3. Check port mapping
    echo ""
    echo "3. Port mapping:"
    port_binding=$(ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "$node" "docker port synaplan-qdrant 6333" 2>/dev/null || echo "none")
    echo "   synaplan-qdrant 6333 -> $port_binding"
    
    # 4. Check synaplan-platform container
    echo ""
    echo "4. synaplan-platform container:"
    platform_status=$(ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "$node" "docker inspect -f '{{.State.Status}}' synaplan-platform" 2>/dev/null || echo "not_found")
    
    if [[ "$platform_status" == "running" ]]; then
        echo -e "   ${GREEN}ok${NC} Status: $platform_status"
    else
        echo -e "   ${YELLOW}warn${NC} Status: $platform_status (platform not running on this node)"
        continue
    fi
    
    # 5. Check docker-host resolution from platform
    echo ""
    echo "5. docker-host resolution from platform container:"
    docker_host_ip=$(ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "$node" "docker exec synaplan-platform getent hosts docker-host 2>/dev/null | awk '{print \$1}'" 2>/dev/null || echo "FAILED")
    
    if [[ -n "$docker_host_ip" && "$docker_host_ip" != "FAILED" ]]; then
        echo -e "   ${GREEN}ok${NC} docker-host resolves to: $docker_host_ip"
    else
        echo -e "   ${RED}FAIL${NC} docker-host does NOT resolve"
    fi
    
    # 6. Test connectivity from platform to Qdrant
    echo ""
    echo "6. Connectivity test from platform -> docker-host:6333:"
    
    health_result=$(ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "$node" "docker exec synaplan-platform curl -sf --connect-timeout 5 http://docker-host:6333/healthz" 2>/dev/null || echo "CURL_FAILED")
    
    if [[ "$health_result" != "CURL_FAILED" ]]; then
        echo -e "   ${GREEN}ok${NC} Platform can reach Qdrant"
    else
        echo -e "   ${RED}FAIL${NC} Platform CANNOT reach Qdrant"
    fi
    
    # 7. Test QDRANT_URL in platform
    echo ""
    echo "7. QDRANT_URL in platform container:"
    qdrant_url=$(ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "$node" "docker exec synaplan-platform printenv QDRANT_URL" 2>/dev/null || echo "NOT_SET")
    echo "   QDRANT_URL = $qdrant_url"
done

echo -e "\n${BLUE}=== Recommendations ===${NC}\n"
echo "If docker-host connection fails but direct IP works:"
echo "  - Check extra_hosts in docker-compose.yml"
echo "  - Verify 'docker-host:host-gateway' is set"
echo ""
echo "If both fail:"
echo "  - Check qdrant is running and healthy"
echo "  - Check port 6333 is published to host"
echo "  - Check firewall rules"
