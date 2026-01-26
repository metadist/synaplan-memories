#!/bin/bash
# test-docker-connectivity.sh - Test Docker internal networking for memory service
#
# Specifically tests the connection path:
#   synaplan-platform (backend) -> docker-host:8090 -> synaplan-qdrant-service
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

# Determine if running locally or need SSH
if [[ -f "/netroot/synaplanCluster/synaplan-memories/docker-compose.yml" ]]; then
    # Running directly on a web node
    LOCAL_NODE=$(hostname -s)
    echo "Running locally on: $LOCAL_NODE"
    
    run_cmd() {
        eval "$@"
    }
else
    # Running from management server
    echo "Running from management server (SSH mode)"
    
    run_cmd() {
        local node=$1
        shift
        ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "$node" "$@" 2>/dev/null
    }
fi

for node in "${!NODES[@]}"; do
    echo -e "\n${BLUE}=== $node (${NODES[$node]}) ===${NC}\n"
    
    # 1. Check host's port 8090
    echo "1. Host port 8090 (qdrant-service):"
    if run_cmd "$node" "curl -sf http://localhost:8090/health > /dev/null 2>&1"; then
        echo -e "   ${GREEN}✓${NC} Host localhost:8090 reachable"
    else
        echo -e "   ${RED}✗${NC} Host localhost:8090 NOT reachable"
    fi
    
    # 2. Check qdrant-service container is running and healthy
    echo ""
    echo "2. qdrant-service container:"
    container_status=$(run_cmd "$node" "docker inspect -f '{{.State.Status}}' synaplan-qdrant-service" || echo "not_found")
    container_health=$(run_cmd "$node" "docker inspect -f '{{.State.Health.Status}}' synaplan-qdrant-service" || echo "unknown")
    
    if [[ "$container_status" == "running" ]]; then
        echo -e "   ${GREEN}✓${NC} Status: $container_status"
    else
        echo -e "   ${RED}✗${NC} Status: $container_status"
    fi
    
    if [[ "$container_health" == "healthy" ]]; then
        echo -e "   ${GREEN}✓${NC} Health: $container_health"
    else
        echo -e "   ${YELLOW}⚠${NC} Health: $container_health"
    fi
    
    # 3. Check port mapping
    echo ""
    echo "3. Port mapping:"
    port_binding=$(run_cmd "$node" "docker port synaplan-qdrant-service 8090" || echo "none")
    echo "   synaplan-qdrant-service 8090 -> $port_binding"
    
    # 4. Check synaplan-platform container
    echo ""
    echo "4. synaplan-platform container:"
    platform_status=$(run_cmd "$node" "docker inspect -f '{{.State.Status}}' synaplan-platform" || echo "not_found")
    
    if [[ "$platform_status" == "running" ]]; then
        echo -e "   ${GREEN}✓${NC} Status: $platform_status"
    else
        echo -e "   ${YELLOW}⚠${NC} Status: $platform_status (platform not running on this node)"
        continue
    fi
    
    # 5. Check docker-host resolution from platform
    echo ""
    echo "5. docker-host resolution from platform container:"
    docker_host_ip=$(run_cmd "$node" "docker exec synaplan-platform getent hosts docker-host 2>/dev/null | awk '{print \$1}'" || echo "FAILED")
    
    if [[ -n "$docker_host_ip" && "$docker_host_ip" != "FAILED" ]]; then
        echo -e "   ${GREEN}✓${NC} docker-host resolves to: $docker_host_ip"
    else
        echo -e "   ${RED}✗${NC} docker-host does NOT resolve"
    fi
    
    # 6. Check host.docker.internal (for comparison)
    host_docker_ip=$(run_cmd "$node" "docker exec synaplan-platform getent hosts host.docker.internal 2>/dev/null | awk '{print \$1}'" || echo "FAILED")
    echo "   host.docker.internal resolves to: $host_docker_ip"
    
    # 7. Test connectivity from platform to qdrant-service
    echo ""
    echo "6. Connectivity test from platform -> docker-host:8090:"
    
    # First try with curl
    health_result=$(run_cmd "$node" "docker exec synaplan-platform curl -sf --connect-timeout 5 http://docker-host:8090/health" || echo "CURL_FAILED")
    
    if [[ "$health_result" != "CURL_FAILED" ]]; then
        echo -e "   ${GREEN}✓${NC} Platform can reach qdrant-service"
        echo "   Response: ${health_result:0:100}..."
    else
        echo -e "   ${RED}✗${NC} Platform CANNOT reach qdrant-service"
        
        # Diagnostic: try to understand why
        echo ""
        echo "   Diagnostic info:"
        
        # Check if it's a DNS issue or connection issue
        run_cmd "$node" "docker exec synaplan-platform sh -c 'ping -c 1 docker-host 2>&1 | head -2'" || true
        
        # Check network mode
        network_mode=$(run_cmd "$node" "docker inspect -f '{{.HostConfig.NetworkMode}}' synaplan-platform" || echo "unknown")
        echo "   Platform network mode: $network_mode"
        
        # Check if qdrant-service is on the same network
        qdrant_networks=$(run_cmd "$node" "docker inspect -f '{{range \$k, \$v := .NetworkSettings.Networks}}{{\$k}} {{end}}' synaplan-qdrant-service" || echo "unknown")
        echo "   qdrant-service networks: $qdrant_networks"
    fi
    
    # 8. Test environment variable in platform
    echo ""
    echo "7. QDRANT_SERVICE_URL in platform container:"
    qdrant_url=$(run_cmd "$node" "docker exec synaplan-platform printenv QDRANT_SERVICE_URL" || echo "NOT_SET")
    echo "   QDRANT_SERVICE_URL = $qdrant_url"
    
    # 9. Test direct IP connection as fallback
    echo ""
    echo "8. Alternative connectivity test (direct to ${NODES[$node]}:8090):"
    direct_result=$(run_cmd "$node" "docker exec synaplan-platform curl -sf --connect-timeout 5 http://${NODES[$node]}:8090/health" || echo "FAILED")
    
    if [[ "$direct_result" != "FAILED" ]]; then
        echo -e "   ${GREEN}✓${NC} Direct IP connection works"
    else
        echo -e "   ${RED}✗${NC} Direct IP connection also fails - qdrant-service may not be listening"
    fi
done

echo -e "\n${BLUE}=== Recommendations ===${NC}\n"
echo "If docker-host connection fails but direct IP works:"
echo "  - Check extra_hosts in docker-compose.yml"
echo "  - Verify 'docker-host:host-gateway' is set"
echo ""
echo "If both fail:"
echo "  - Check qdrant-service is running and healthy"
echo "  - Check port 8090 is published to host"
echo "  - Check firewall rules"
