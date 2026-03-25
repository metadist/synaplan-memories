#!/bin/bash
# diagnose-local.sh - Quick local diagnosis for a single node
#
# Run directly on a web node (web1, web2, or web3) to quickly check:
#   - Container status
#   - Network connectivity
#   - Common issues
#
# Usage: cd /netroot/synaplanCluster/synaplan-memories && ./_devextras/diagnose-local.sh [QDRANT_HOST]

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

echo -e "${BOLD}${BLUE}=== Synaplan Qdrant Local Diagnosis ===${NC}"
echo ""

HOSTNAME=$(hostname -s)
IP=$(hostname -I | awk '{print $1}')

echo -e "Host: ${BOLD}$HOSTNAME${NC} ($IP)"
echo -e "Time: $(date)"
echo ""

ISSUES=0

#------------------------------------------------------------------------------
echo -e "${BLUE}[1/5] Container Status${NC}"
#------------------------------------------------------------------------------

status=$(docker inspect -f '{{.State.Status}}' synaplan-qdrant 2>/dev/null || echo "not_found")
if [[ "$status" == "running" ]]; then
    echo -e "  ${GREEN}ok${NC} synaplan-qdrant: $status"
else
    echo -e "  ${RED}FAIL${NC} synaplan-qdrant: $status"
    ((ISSUES++))
fi

#------------------------------------------------------------------------------
echo ""
echo -e "${BLUE}[2/5] Qdrant Health${NC}"
#------------------------------------------------------------------------------

qdrant_health=$(curl -sf http://${QDRANT_HOST}:6333/healthz 2>/dev/null || echo "FAILED")
if [[ "$qdrant_health" == *"ok"* ]] || [[ "$qdrant_health" != "FAILED" ]]; then
    echo -e "  ${GREEN}ok${NC} Qdrant REST API: OK"
else
    echo -e "  ${RED}FAIL${NC} Qdrant REST API: $qdrant_health"
    ((ISSUES++))
fi

#------------------------------------------------------------------------------
echo ""
echo -e "${BLUE}[3/5] Cluster Status${NC}"
#------------------------------------------------------------------------------

cluster_json=$(curl -sf http://${QDRANT_HOST}:6333/cluster 2>/dev/null || echo "{}")
if [[ "$cluster_json" != "{}" ]]; then
    peer_count=$(echo "$cluster_json" | jq -r '.result.peers | keys | length' 2>/dev/null || echo "0")
    cluster_status=$(echo "$cluster_json" | jq -r '.result.status // "unknown"' 2>/dev/null)
    peer_id=$(echo "$cluster_json" | jq -r '.result.peer_id // "unknown"' 2>/dev/null)
    
    if [[ "$peer_count" == "3" ]]; then
        echo -e "  ${GREEN}ok${NC} Peers: $peer_count"
    else
        echo -e "  ${YELLOW}warning${NC} Peers: $peer_count (expected 3)"
        ((ISSUES++))
    fi
    
    if [[ "$cluster_status" == "enabled" ]]; then
        echo -e "  ${GREEN}ok${NC} Status: $cluster_status"
    else
        echo -e "  ${RED}FAIL${NC} Status: $cluster_status"
        ((ISSUES++))
    fi
    
    echo -e "  ${CYAN}info${NC} Peer ID: ${peer_id:0:12}..."
else
    echo -e "  ${RED}FAIL${NC} Cannot fetch cluster status"
    ((ISSUES++))
fi

#------------------------------------------------------------------------------
echo ""
echo -e "${BLUE}[4/5] Storage Check${NC}"
#------------------------------------------------------------------------------

if [[ -d "/qdrant/storage" ]]; then
    echo -e "  ${GREEN}ok${NC} /qdrant/storage exists"
    
    if mount | grep -q "/qdrant.*nfs"; then
        echo -e "  ${RED}FAIL${NC} WARNING: /qdrant is on NFS! Qdrant requires LOCAL storage."
        ((ISSUES++))
    else
        echo -e "  ${GREEN}ok${NC} Storage is on local disk"
    fi
    
    usage=$(df -h /qdrant/storage | tail -1 | awk '{print $5}')
    echo -e "  ${CYAN}info${NC} Disk usage: $usage"
else
    echo -e "  ${RED}FAIL${NC} /qdrant/storage does NOT exist"
    ((ISSUES++))
fi

#------------------------------------------------------------------------------
echo ""
echo -e "${BLUE}[5/5] Recent Errors in Logs${NC}"
#------------------------------------------------------------------------------

echo "  qdrant container (last 3 errors):"
docker logs synaplan-qdrant 2>&1 | grep -i "error\|panic\|fatal" | tail -3 | while read line; do
    echo -e "    ${RED}$line${NC}"
done || echo -e "    ${GREEN}No recent errors${NC}"

#------------------------------------------------------------------------------
echo ""
echo -e "${BOLD}${BLUE}======================================${NC}"
#------------------------------------------------------------------------------

if [[ $ISSUES -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}All checks passed!${NC}"
else
    echo -e "${RED}${BOLD}$ISSUES issue(s) found${NC}"
    echo ""
    echo "Common fixes:"
    echo "  - Container not running: docker compose up -d"
    echo "  - Cannot reach peers: Check firewall for port 6335"
    echo "  - Storage on NFS: Move /qdrant/storage to local disk"
fi

echo ""
