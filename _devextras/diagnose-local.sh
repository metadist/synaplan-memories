#!/bin/bash
# diagnose-local.sh - Quick local diagnosis for a single node
#
# Run directly on a web node (web1, web2, or web3) to quickly check:
#   - Container status
#   - Network connectivity
#   - API key configuration
#   - Common issues
#
# Usage: cd /netroot/synaplanCluster/synaplan-memories && ./diagnose-local.sh [QDRANT_HOST]

set -euo pipefail

QDRANT_HOST="${1:-10.0.0.2}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

echo -e "${BOLD}${BLUE}╔══════════════════════════════════════════╗${NC}"
echo -e "${BOLD}${BLUE}║  Synaplan Memory Service Local Diagnosis  ║${NC}"
echo -e "${BOLD}${BLUE}╚══════════════════════════════════════════╝${NC}"
echo ""

HOSTNAME=$(hostname -s)
IP=$(hostname -I | awk '{print $1}')

echo -e "Host: ${BOLD}$HOSTNAME${NC} ($IP)"
echo -e "Time: $(date)"
echo ""

ISSUES=0

#------------------------------------------------------------------------------
echo -e "${BLUE}[1/7] Container Status${NC}"
#------------------------------------------------------------------------------

for container in synaplan-qdrant synaplan-qdrant-service; do
    status=$(docker inspect -f '{{.State.Status}}' "$container" 2>/dev/null || echo "not_found")
    if [[ "$status" == "running" ]]; then
        echo -e "  ${GREEN}✓${NC} $container: $status"
    else
        echo -e "  ${RED}✗${NC} $container: $status"
        ((ISSUES++))
    fi
done

# Check health
svc_health=$(docker inspect -f '{{.State.Health.Status}}' synaplan-qdrant-service 2>/dev/null || echo "unknown")
if [[ "$svc_health" == "healthy" ]]; then
    echo -e "  ${GREEN}✓${NC} qdrant-service health: $svc_health"
elif [[ "$svc_health" == "starting" ]]; then
    echo -e "  ${YELLOW}⚠${NC} qdrant-service health: $svc_health (wait a moment)"
else
    echo -e "  ${RED}✗${NC} qdrant-service health: $svc_health"
    ((ISSUES++))
fi

#------------------------------------------------------------------------------
echo ""
echo -e "${BLUE}[2/7] Qdrant Health${NC}"
#------------------------------------------------------------------------------

qdrant_health=$(curl -sf http://${QDRANT_HOST}:6333/healthz 2>/dev/null || echo "FAILED")
if [[ "$qdrant_health" == *"ok"* ]] || [[ "$qdrant_health" != "FAILED" ]]; then
    echo -e "  ${GREEN}✓${NC} Qdrant REST API: OK"
else
    echo -e "  ${RED}✗${NC} Qdrant REST API: $qdrant_health"
    ((ISSUES++))
fi

svc_health_resp=$(curl -sf http://localhost:8090/health 2>/dev/null || echo "FAILED")
if [[ "$svc_health_resp" == *"ok"* ]] || [[ "$svc_health_resp" == *"status"* ]]; then
    echo -e "  ${GREEN}✓${NC} qdrant-service API: OK"
else
    echo -e "  ${RED}✗${NC} qdrant-service API: $svc_health_resp"
    ((ISSUES++))
fi

#------------------------------------------------------------------------------
echo ""
echo -e "${BLUE}[3/7] Cluster Status${NC}"
#------------------------------------------------------------------------------

cluster_json=$(curl -sf http://${QDRANT_HOST}:6333/cluster 2>/dev/null || echo "{}")
if [[ "$cluster_json" != "{}" ]]; then
    peer_count=$(echo "$cluster_json" | jq -r '.result.peers | keys | length' 2>/dev/null || echo "0")
    cluster_status=$(echo "$cluster_json" | jq -r '.result.status // "unknown"' 2>/dev/null)
    peer_id=$(echo "$cluster_json" | jq -r '.result.peer_id // "unknown"' 2>/dev/null)
    
    if [[ "$peer_count" == "3" ]]; then
        echo -e "  ${GREEN}✓${NC} Peers: $peer_count"
    else
        echo -e "  ${YELLOW}⚠${NC} Peers: $peer_count (expected 3)"
        ((ISSUES++))
    fi
    
    if [[ "$cluster_status" == "enabled" ]]; then
        echo -e "  ${GREEN}✓${NC} Status: $cluster_status"
    else
        echo -e "  ${RED}✗${NC} Status: $cluster_status"
        ((ISSUES++))
    fi
    
    echo -e "  ${CYAN}ℹ${NC} Peer ID: ${peer_id:0:12}..."
else
    echo -e "  ${RED}✗${NC} Cannot fetch cluster status"
    ((ISSUES++))
fi

#------------------------------------------------------------------------------
echo ""
echo -e "${BLUE}[4/7] Storage Check${NC}"
#------------------------------------------------------------------------------

# Check /qdrant/storage exists and is not on NFS
if [[ -d "/qdrant/storage" ]]; then
    echo -e "  ${GREEN}✓${NC} /qdrant/storage exists"
    
    # Check if it's on NFS
    if mount | grep -q "/qdrant.*nfs"; then
        echo -e "  ${RED}✗${NC} WARNING: /qdrant is on NFS! Qdrant requires LOCAL storage."
        ((ISSUES++))
    else
        echo -e "  ${GREEN}✓${NC} Storage is on local disk"
    fi
    
    # Check disk usage
    usage=$(df -h /qdrant/storage | tail -1 | awk '{print $5}')
    echo -e "  ${CYAN}ℹ${NC} Disk usage: $usage"
else
    echo -e "  ${RED}✗${NC} /qdrant/storage does NOT exist"
    ((ISSUES++))
fi

#------------------------------------------------------------------------------
echo ""
echo -e "${BLUE}[5/7] API Key Configuration${NC}"
#------------------------------------------------------------------------------

# Get SERVICE_API_KEY from qdrant-service
svc_key=$(docker exec synaplan-qdrant-service printenv SERVICE_API_KEY 2>/dev/null || echo "")
if [[ -n "$svc_key" ]]; then
    echo -e "  ${GREEN}✓${NC} SERVICE_API_KEY is set: ${svc_key:0:4}...${svc_key: -4}"
else
    echo -e "  ${YELLOW}⚠${NC} SERVICE_API_KEY not set (anyone can access the service)"
fi

# Check if platform is running and compare keys
platform_status=$(docker inspect -f '{{.State.Status}}' synaplan-platform 2>/dev/null || echo "not_found")
if [[ "$platform_status" == "running" ]]; then
    platform_key=$(docker exec synaplan-platform printenv QDRANT_SERVICE_API_KEY 2>/dev/null || echo "")
    
    if [[ -z "$platform_key" ]]; then
        echo -e "  ${YELLOW}⚠${NC} Platform QDRANT_SERVICE_API_KEY not set"
    elif [[ "$platform_key" == "$svc_key" ]]; then
        echo -e "  ${GREEN}✓${NC} Platform and service API keys match"
    else
        echo -e "  ${RED}✗${NC} API KEY MISMATCH between platform and service!"
        echo -e "    Platform: ${platform_key:0:4}...${platform_key: -4}"
        echo -e "    Service:  ${svc_key:0:4}...${svc_key: -4}"
        ((ISSUES++))
    fi
fi

#------------------------------------------------------------------------------
echo ""
echo -e "${BLUE}[6/7] Platform Connectivity${NC}"
#------------------------------------------------------------------------------

if [[ "$platform_status" == "running" ]]; then
    # Test docker-host resolution
    docker_host_ip=$(docker exec synaplan-platform getent hosts docker-host 2>/dev/null | awk '{print $1}' || echo "")
    if [[ -n "$docker_host_ip" ]]; then
        echo -e "  ${GREEN}✓${NC} docker-host resolves to: $docker_host_ip"
    else
        echo -e "  ${RED}✗${NC} docker-host does NOT resolve in platform container"
        ((ISSUES++))
    fi
    
    # Test connectivity
    qdrant_url=$(docker exec synaplan-platform printenv QDRANT_SERVICE_URL 2>/dev/null || echo "")
    if [[ -n "$qdrant_url" ]]; then
        health_test=$(docker exec synaplan-platform curl -sf --connect-timeout 3 "$qdrant_url/health" 2>/dev/null || echo "FAILED")
        if [[ "$health_test" != "FAILED" ]]; then
            echo -e "  ${GREEN}✓${NC} Platform can reach $qdrant_url"
        else
            echo -e "  ${RED}✗${NC} Platform CANNOT reach $qdrant_url"
            ((ISSUES++))
        fi
    fi
else
    echo -e "  ${CYAN}ℹ${NC} synaplan-platform not running on this node"
fi

#------------------------------------------------------------------------------
echo ""
echo -e "${BLUE}[7/7] Recent Errors in Logs${NC}"
#------------------------------------------------------------------------------

echo "  qdrant container (last 3 errors):"
docker logs synaplan-qdrant 2>&1 | grep -i "error\|panic\|fatal" | tail -3 | while read line; do
    echo -e "    ${RED}$line${NC}"
done || echo -e "    ${GREEN}No recent errors${NC}"

echo ""
echo "  qdrant-service container (last 3 errors):"
docker logs synaplan-qdrant-service 2>&1 | grep -i "error\|panic\|fatal" | tail -3 | while read line; do
    echo -e "    ${RED}$line${NC}"
done || echo -e "    ${GREEN}No recent errors${NC}"

#------------------------------------------------------------------------------
echo ""
echo -e "${BOLD}${BLUE}══════════════════════════════════════════${NC}"
#------------------------------------------------------------------------------

if [[ $ISSUES -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}✓ All checks passed!${NC}"
else
    echo -e "${RED}${BOLD}✗ $ISSUES issue(s) found${NC}"
    echo ""
    echo "Common fixes:"
    echo "  - Container not running: docker compose up -d"
    echo "  - API key mismatch: Edit .env files and restart"
    echo "  - Cannot reach peers: Check firewall for port 6335"
    echo "  - Storage on NFS: Move /qdrant/storage to local disk"
fi

echo ""
