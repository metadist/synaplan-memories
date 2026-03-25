#!/bin/bash
# compare-nodes.sh - Compare configuration across all nodes to find differences
#
# Useful for debugging why one node works but others don't.
# Run from management server.

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Nodes
NODES=("web1" "web2" "web3")

echo -e "${BLUE}=== Node Configuration Comparison ===${NC}"
echo ""

# Helper
remote_exec() {
    ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "$1" "$2" 2>/dev/null || echo "SSH_FAILED"
}

# Print header row
print_row() {
    local label=$1
    shift
    printf "%-30s" "$label"
    for val in "$@"; do
        printf "%-20s" "$val"
    done
    echo ""
}

# Separator
sep() {
    printf "%-30s" ""
    for node in "${NODES[@]}"; do
        printf "%-20s" "--------------------"
    done
    echo ""
}

# Header
printf "%-30s" "Check"
for node in "${NODES[@]}"; do
    printf "%-20s" "$node"
done
echo ""
sep

#------------------------------------------------------------------------------
# Docker container status
#------------------------------------------------------------------------------

vals=()
for node in "${NODES[@]}"; do
    status=$(remote_exec "$node" "docker inspect -f '{{.State.Status}}' synaplan-qdrant")
    vals+=("$status")
done
print_row "qdrant container" "${vals[@]}"

sep

#------------------------------------------------------------------------------
# Qdrant cluster metrics
#------------------------------------------------------------------------------

vals=()
for node in "${NODES[@]}"; do
    peers=$(remote_exec "$node" "curl -sf http://${NODES[$node]}:6333/cluster 2>/dev/null | jq -r '.result.peers | keys | length' 2>/dev/null")
    [[ -z "$peers" ]] && peers="ERR"
    vals+=("$peers")
done
print_row "Cluster peers seen" "${vals[@]}"

vals=()
for node in "${NODES[@]}"; do
    status=$(remote_exec "$node" "curl -sf http://${NODES[$node]}:6333/cluster 2>/dev/null | jq -r '.result.status // \"?\"' 2>/dev/null")
    vals+=("$status")
done
print_row "Cluster status" "${vals[@]}"

vals=()
for node in "${NODES[@]}"; do
    term=$(remote_exec "$node" "curl -sf http://${NODES[$node]}:6333/cluster 2>/dev/null | jq -r '.result.raft_info.term // 0' 2>/dev/null")
    vals+=("$term")
done
print_row "Raft term" "${vals[@]}"

vals=()
for node in "${NODES[@]}"; do
    commit=$(remote_exec "$node" "curl -sf http://${NODES[$node]}:6333/cluster 2>/dev/null | jq -r '.result.raft_info.commit // 0' 2>/dev/null")
    vals+=("$commit")
done
print_row "Raft commit" "${vals[@]}"

sep

#------------------------------------------------------------------------------
# Network connectivity
#------------------------------------------------------------------------------

vals=()
for node in "${NODES[@]}"; do
    result=$(remote_exec "$node" "curl -sf http://${NODES[$node]}:6333/healthz > /dev/null && echo OK || echo FAIL")
    vals+=("$result")
done
print_row "Qdrant :6333 health" "${vals[@]}"

sep

#------------------------------------------------------------------------------
# Storage
#------------------------------------------------------------------------------

vals=()
for node in "${NODES[@]}"; do
    result=$(remote_exec "$node" "[[ -d /qdrant/storage ]] && echo EXISTS || echo MISSING")
    vals+=("$result")
done
print_row "/qdrant/storage dir" "${vals[@]}"

vals=()
for node in "${NODES[@]}"; do
    result=$(remote_exec "$node" "mount | grep -q '/qdrant.*nfs' && echo NFS_BAD || echo LOCAL_OK")
    vals+=("$result")
done
print_row "Storage type" "${vals[@]}"

vals=()
for node in "${NODES[@]}"; do
    result=$(remote_exec "$node" "df -h /qdrant/storage 2>/dev/null | tail -1 | awk '{print \$5}'" || echo "?")
    vals+=("$result")
done
print_row "Storage usage" "${vals[@]}"

sep

#------------------------------------------------------------------------------
# Docker Image Versions
#------------------------------------------------------------------------------

echo ""
echo -e "${BLUE}Docker Image Versions:${NC}"
echo ""

vals=()
for node in "${NODES[@]}"; do
    img=$(remote_exec "$node" "docker inspect -f '{{.Config.Image}}' synaplan-qdrant 2>/dev/null | sed 's/.*://'")
    vals+=("$img")
done
print_row "qdrant image tag" "${vals[@]}"

#------------------------------------------------------------------------------
echo ""
echo -e "${BLUE}======================================${NC}"
echo ""
echo "Legend:"
echo "  - All values in a row should match (or be expected to differ)"
echo "  - Red flags: storage on NFS, missing containers"
echo "  - Check differences to find why web1 works but others don't"
echo ""
