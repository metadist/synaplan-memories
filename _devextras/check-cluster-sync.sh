#!/bin/bash
# check-cluster-sync.sh - Check cluster synchronization between all nodes
#
# Compares cluster metrics (peers, term, commit) across all nodes
# to verify they are in sync. Run from any node or management server.

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

echo -e "${BLUE}=== Qdrant Cluster Synchronization Check ===${NC}\n"

# Function to fetch metrics from a node
fetch_metrics() {
    local node_name=$1
    local metrics=$(ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "${node_name}" \
        "curl -sf http://localhost:6333/metrics 2>/dev/null" 2>/dev/null || echo "")
    echo "$metrics"
}

# Function to extract metric value
get_metric() {
    local metrics=$1
    local metric_name=$2
    echo "$metrics" | grep "^${metric_name} " | awk '{print $2}' | head -1
}

# Collect metrics from all nodes
declare -A NODE_METRICS
for node_name in "${!NODES[@]}"; do
    echo -e "${BLUE}Fetching metrics from ${node_name}...${NC}"
    NODE_METRICS["${node_name}"]=$(fetch_metrics "$node_name")
    if [ -z "${NODE_METRICS[$node_name]}" ]; then
        echo -e "  ${RED}✗ Failed to fetch metrics${NC}\n"
    else
        echo -e "  ${GREEN}✓ Metrics fetched${NC}\n"
    fi
done

# Check cluster metrics
echo -e "${BLUE}=== Cluster Metrics Comparison ===${NC}\n"

# Check cluster_enabled
echo "Cluster Enabled:"
for node_name in "${!NODES[@]}"; do
    local enabled=$(get_metric "${NODE_METRICS[$node_name]}" "cluster_enabled")
    if [ -n "$enabled" ]; then
        if [ "$enabled" = "1" ]; then
            echo -e "  ${node_name}: ${GREEN}✓ Enabled${NC}"
        else
            echo -e "  ${node_name}: ${RED}✗ Disabled${NC}"
        fi
    else
        echo -e "  ${node_name}: ${RED}✗ Not available${NC}"
    fi
done
echo ""

# Check cluster_peers_total
echo "Cluster Peers Total:"
declare -A PEER_COUNTS
for node_name in "${!NODES[@]}"; do
    local peers=$(get_metric "${NODE_METRICS[$node_name]}" "cluster_peers_total")
    PEER_COUNTS["${node_name}"]=$peers
    if [ -n "$peers" ]; then
        if [ "$peers" = "3" ]; then
            echo -e "  ${node_name}: ${GREEN}${peers}${NC}"
        else
            echo -e "  ${node_name}: ${YELLOW}${peers}${NC} (expected 3)"
        fi
    else
        echo -e "  ${node_name}: ${RED}Not available${NC}"
    fi
done

# Check if all nodes see the same number of peers
local first_peer_count=""
local peers_match=true
for node_name in "${!NODES[@]}"; do
    if [ -n "${PEER_COUNTS[$node_name]}" ]; then
        if [ -z "$first_peer_count" ]; then
            first_peer_count="${PEER_COUNTS[$node_name]}"
        elif [ "${PEER_COUNTS[$node_name]}" != "$first_peer_count" ]; then
            peers_match=false
        fi
    fi
done

if [ "$peers_match" = true ] && [ "$first_peer_count" = "3" ]; then
    echo -e "  ${GREEN}✓ All nodes see 3 peers${NC}\n"
else
    echo -e "  ${YELLOW}⚠ Peer counts differ or incomplete${NC}\n"
fi

# Check cluster_term (Raft consensus term)
echo "Cluster Term (Raft Consensus):"
declare -A TERMS
for node_name in "${!NODES[@]}"; do
    local term=$(get_metric "${NODE_METRICS[$node_name]}" "cluster_term")
    TERMS["${node_name}"]=$term
    if [ -n "$term" ]; then
        echo -e "  ${node_name}: ${term}"
    else
        echo -e "  ${node_name}: ${RED}Not available${NC}"
    fi
done

# Check if all terms match
local first_term=""
local terms_match=true
for node_name in "${!NODES[@]}"; do
    if [ -n "${TERMS[$node_name]}" ]; then
        if [ -z "$first_term" ]; then
            first_term="${TERMS[$node_name]}"
        elif [ "${TERMS[$node_name]}" != "$first_term" ]; then
            terms_match=false
        fi
    fi
done

if [ "$terms_match" = true ] && [ -n "$first_term" ]; then
    echo -e "  ${GREEN}✓ All nodes on same term: ${first_term}${NC}\n"
else
    echo -e "  ${YELLOW}⚠ Terms differ - cluster may be split or recovering${NC}\n"
fi

# Check cluster_commit (Raft commit index)
echo "Cluster Commit (Last Committed Operation):"
declare -A COMMITS
for node_name in "${!NODES[@]}"; do
    local commit=$(get_metric "${NODE_METRICS[$node_name]}" "cluster_commit")
    COMMITS["${node_name}"]=$commit
    if [ -n "$commit" ]; then
        echo -e "  ${node_name}: ${commit}"
    else
        echo -e "  ${node_name}: ${RED}Not available${NC}"
    fi
done

# Check if commits are close (within reasonable range)
local commits_match=true
local max_diff=10  # Allow small differences due to async replication
local first_commit=""
for node_name in "${!NODES[@]}"; do
    if [ -n "${COMMITS[$node_name]}" ]; then
        if [ -z "$first_commit" ]; then
            first_commit="${COMMITS[$node_name]}"
        else
            local diff=$(( ${COMMITS[$node_name]} - first_commit ))
            if [ ${diff#-} -gt $max_diff ]; then
                commits_match=false
            fi
        fi
    fi
done

if [ "$commits_match" = true ] && [ -n "$first_commit" ]; then
    echo -e "  ${GREEN}✓ Commits synchronized${NC}\n"
else
    echo -e "  ${YELLOW}⚠ Commits differ significantly - check replication${NC}\n"
fi

# Check pending operations
echo "Pending Operations:"
for node_name in "${!NODES[@]}"; do
    local pending=$(get_metric "${NODE_METRICS[$node_name]}" "cluster_pending_operations_total")
    if [ -n "$pending" ]; then
        if [ "$pending" = "0" ]; then
            echo -e "  ${node_name}: ${GREEN}${pending}${NC}"
        else
            echo -e "  ${node_name}: ${YELLOW}${pending}${NC} (operations pending)"
        fi
    else
        echo -e "  ${node_name}: ${RED}Not available${NC}"
    fi
done
echo ""

# Check collection metrics
echo -e "${BLUE}=== Collection Metrics ===${NC}\n"

# Check collections_total
echo "Collections Total:"
declare -A COLLECTION_COUNTS
for node_name in "${!NODES[@]}"; do
    local colls=$(get_metric "${NODE_METRICS[$node_name]}" "collections_total")
    COLLECTION_COUNTS["${node_name}"]=$colls
    if [ -n "$colls" ]; then
        echo -e "  ${node_name}: ${colls}"
    else
        echo -e "  ${node_name}: ${RED}Not available${NC}"
    fi
done

# Check if collection counts match
local first_coll_count=""
local colls_match=true
for node_name in "${!NODES[@]}"; do
    if [ -n "${COLLECTION_COUNTS[$node_name]}" ]; then
        if [ -z "$first_coll_count" ]; then
            first_coll_count="${COLLECTION_COUNTS[$node_name]}"
        elif [ "${COLLECTION_COUNTS[$node_name]}" != "$first_coll_count" ]; then
            colls_match=false
        fi
    fi
done

if [ "$colls_match" = true ] && [ -n "$first_coll_count" ]; then
    echo -e "  ${GREEN}✓ All nodes see ${first_coll_count} collection(s)${NC}\n"
else
    echo -e "  ${YELLOW}⚠ Collection counts differ${NC}\n"
fi

# Summary
echo -e "${BLUE}=== Summary ===${NC}"
if [ "$peers_match" = true ] && [ "$terms_match" = true ] && [ "$commits_match" = true ] && [ "$colls_match" = true ]; then
    echo -e "${GREEN}✓ Cluster is synchronized${NC}"
else
    echo -e "${YELLOW}⚠ Cluster synchronization issues detected${NC}"
    echo "  - Check network connectivity between nodes"
    echo "  - Check Qdrant logs: docker compose logs qdrant"
    echo "  - Verify P2P port 6335 is accessible"
fi
