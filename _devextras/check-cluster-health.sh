#!/bin/bash
# check-cluster-health.sh - Check health status of all Qdrant cluster nodes
#
# Checks health endpoints, cluster status, and basic connectivity
# Run from any node or management server

set -euo pipefail

# Node configuration
declare -A NODES=(
    ["web1"]="10.0.0.2"
    ["web2"]="10.0.0.7"
    ["web3"]="10.0.0.8"
)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== Qdrant Cluster Health Check ===${NC}\n"

# Function to check if running locally or via SSH
check_node() {
    local node_name=$1
    local node_ip=$2
    
    echo -e "${BLUE}Checking ${node_name} (${node_ip})...${NC}"
    
    # Check if we can reach the node
    if ! ping -c 1 -W 2 "${node_ip}" > /dev/null 2>&1; then
        echo -e "  ${RED}✗ Node unreachable${NC}"
        return 1
    fi
    
    # Check Qdrant health endpoints
    echo -n "  Health endpoints: "
    if ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "${node_name}" "curl -sf http://${node_ip}:6333/healthz > /dev/null 2>&1" 2>/dev/null; then
        echo -e "${GREEN}✓${NC}"
    else
        echo -e "${RED}✗${NC}"
    fi
    
    # Check qdrant-service health
    echo -n "  qdrant-service: "
    if ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "${node_name}" "curl -sf http://localhost:8090/health > /dev/null 2>&1" 2>/dev/null; then
        echo -e "${GREEN}✓${NC}"
    else
        echo -e "${RED}✗${NC}"
    fi
    
    # Get cluster status
    echo -n "  Cluster status: "
    local cluster_status=$(ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "${node_name}" \
        "curl -sf http://${node_ip}:6333/cluster 2>/dev/null" 2>/dev/null || echo "")
    
    if [ -z "$cluster_status" ]; then
        echo -e "${RED}✗ Cannot fetch${NC}"
    else
        local peer_count=$(echo "$cluster_status" | jq -r '.result.peers | keys | length' 2>/dev/null || echo "0")
        local status=$(echo "$cluster_status" | jq -r '.result.status // "unknown"' 2>/dev/null || echo "unknown")
        
        if [ "$peer_count" = "3" ] && [ "$status" != "unknown" ]; then
            echo -e "${GREEN}✓${NC} (${peer_count} peers, status: ${status})"
        else
            echo -e "${YELLOW}⚠${NC} (${peer_count} peers, status: ${status})"
        fi
    fi
    
    # Check P2P port connectivity
    echo -n "  P2P port (6335): "
    if timeout 2 bash -c "echo > /dev/tcp/${node_ip}/6335" 2>/dev/null; then
        echo -e "${GREEN}✓${NC}"
    else
        echo -e "${RED}✗${NC}"
    fi
    
    echo ""
}

# Check all nodes
for node_name in "${!NODES[@]}"; do
    check_node "$node_name" "${NODES[$node_name]}"
done

echo -e "${BLUE}=== Summary ===${NC}"
echo "Run 'check-cluster-sync.sh' for detailed synchronization check"
echo "Run 'check-loadbalancer.sh' to test load balancer"
