#!/bin/bash
# test-full-cluster.sh - Comprehensive test suite for Qdrant Memory Service cluster
#
# Tests all aspects: connectivity, docker, qdrant cluster, replication, API keys
# Run from management server (synastorev1) or any machine with SSH access to web1-3
#
# Usage: ./test-full-cluster.sh [--verbose]

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
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

VERBOSE=false
ERRORS=0
WARNINGS=0

if [[ "${1:-}" == "--verbose" ]]; then
    VERBOSE=true
fi

# Helper functions
pass() { echo -e "  ${GREEN}✓${NC} $1"; }
fail() { echo -e "  ${RED}✗${NC} $1"; ((ERRORS++)) || true; }
warn() { echo -e "  ${YELLOW}⚠${NC} $1"; ((WARNINGS++)) || true; }
info() { echo -e "  ${CYAN}ℹ${NC} $1"; }
section() { echo -e "\n${BOLD}${BLUE}=== $1 ===${NC}\n"; }
subsection() { echo -e "${CYAN}--- $1 ---${NC}"; }

# Execute command on remote node via SSH
remote_exec() {
    local node=$1
    shift
    ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no -o BatchMode=yes "$node" "$@" 2>/dev/null
}

# Check if remote command succeeds
remote_check() {
    local node=$1
    shift
    remote_exec "$node" "$@" > /dev/null 2>&1
}

##############################################################################
section "1. NETWORK CONNECTIVITY"
##############################################################################

for node in "${!NODES[@]}"; do
    subsection "Testing $node (${NODES[$node]})"
    
    # Ping test
    if ping -c 1 -W 2 "${NODES[$node]}" > /dev/null 2>&1; then
        pass "Ping: reachable"
    else
        fail "Ping: unreachable"
        continue
    fi
    
    # SSH test
    if remote_check "$node" "echo ok"; then
        pass "SSH: connected"
    else
        fail "SSH: connection failed"
        continue
    fi
    
    # Port checks from management server
    for port in 6333 6335 8090; do
        if timeout 2 bash -c "echo > /dev/tcp/${NODES[$node]}/$port" 2>/dev/null; then
            pass "Port $port: open"
        else
            if [[ "$port" == "6333" ]]; then
                # 6333 might be bound to localhost only in production
                info "Port $port: not exposed (might be localhost-only)"
            else
                fail "Port $port: closed"
            fi
        fi
    done
done

# Inter-node P2P connectivity
echo ""
subsection "Inter-node P2P connectivity (port 6335)"
for src_node in "${!NODES[@]}"; do
    for dst_node in "${!NODES[@]}"; do
        if [[ "$src_node" != "$dst_node" ]]; then
            if remote_check "$src_node" "timeout 2 bash -c 'echo > /dev/tcp/${NODES[$dst_node]}/6335' 2>/dev/null"; then
                pass "$src_node → $dst_node: P2P OK"
            else
                fail "$src_node → $dst_node: P2P BLOCKED"
            fi
        fi
    done
done

##############################################################################
section "2. DOCKER CONTAINER STATUS"
##############################################################################

for node in "${!NODES[@]}"; do
    subsection "Docker on $node"
    
    # Check Docker daemon
    if remote_check "$node" "docker info > /dev/null 2>&1"; then
        pass "Docker daemon: running"
    else
        fail "Docker daemon: not running"
        continue
    fi
    
    # Check qdrant container
    qdrant_status=$(remote_exec "$node" "docker inspect -f '{{.State.Status}}' synaplan-qdrant 2>/dev/null" || echo "not_found")
    if [[ "$qdrant_status" == "running" ]]; then
        pass "synaplan-qdrant: running"
    else
        fail "synaplan-qdrant: $qdrant_status"
    fi
    
    # Check qdrant-service container
    service_status=$(remote_exec "$node" "docker inspect -f '{{.State.Status}}' synaplan-qdrant-service 2>/dev/null" || echo "not_found")
    if [[ "$service_status" == "running" ]]; then
        pass "synaplan-qdrant-service: running"
    else
        fail "synaplan-qdrant-service: $service_status"
    fi
    
    # Check container health
    service_health=$(remote_exec "$node" "docker inspect -f '{{.State.Health.Status}}' synaplan-qdrant-service 2>/dev/null" || echo "unknown")
    if [[ "$service_health" == "healthy" ]]; then
        pass "qdrant-service health: $service_health"
    elif [[ "$service_health" == "starting" ]]; then
        warn "qdrant-service health: $service_health (still initializing)"
    else
        fail "qdrant-service health: $service_health"
    fi
    
    # Show recent logs if verbose
    if $VERBOSE; then
        echo ""
        info "Recent qdrant container logs:"
        remote_exec "$node" "cd /netroot/synaplanCluster/synaplan-memories && docker compose logs --tail=5 qdrant 2>/dev/null" | head -10 || true
        echo ""
        info "Recent qdrant-service container logs:"
        remote_exec "$node" "cd /netroot/synaplanCluster/synaplan-memories && docker compose logs --tail=5 qdrant-service 2>/dev/null" | head -10 || true
    fi
done

##############################################################################
section "3. QDRANT LOCAL HEALTH (per node)"
##############################################################################

for node in "${!NODES[@]}"; do
    subsection "Qdrant on $node"
    
    # Check /healthz endpoint
    healthz=$(remote_exec "$node" "curl -sf http://localhost:6333/healthz 2>/dev/null" || echo "FAILED")
    if [[ "$healthz" == *"ok"* ]] || [[ "$healthz" == *"title"* ]]; then
        pass "Qdrant /healthz: OK"
    else
        fail "Qdrant /healthz: $healthz"
    fi
    
    # Check qdrant-service /health endpoint
    svc_health=$(remote_exec "$node" "curl -sf http://localhost:8090/health 2>/dev/null" || echo "FAILED")
    if [[ "$svc_health" == *"status"*"ok"* ]] || [[ "$svc_health" == *"healthy"* ]]; then
        pass "qdrant-service /health: OK"
    else
        fail "qdrant-service /health: $svc_health"
    fi
done

##############################################################################
section "4. QDRANT CLUSTER STATUS"
##############################################################################

# Get cluster info from each node
declare -A PEER_COUNTS
declare -A CLUSTER_STATUS
declare -A RAFT_TERMS
declare -A RAFT_COMMITS

for node in "${!NODES[@]}"; do
    subsection "Cluster view from $node"
    
    cluster_json=$(remote_exec "$node" "curl -sf http://localhost:6333/cluster 2>/dev/null" || echo "{}")
    
    if [[ "$cluster_json" == "{}" ]] || [[ -z "$cluster_json" ]]; then
        fail "Cannot fetch cluster status"
        continue
    fi
    
    # Parse cluster info
    peer_count=$(echo "$cluster_json" | jq -r '.result.peers | keys | length' 2>/dev/null || echo "0")
    status=$(echo "$cluster_json" | jq -r '.result.status // "unknown"' 2>/dev/null)
    peer_id=$(echo "$cluster_json" | jq -r '.result.peer_id // "unknown"' 2>/dev/null)
    raft_term=$(echo "$cluster_json" | jq -r '.result.raft_info.term // 0' 2>/dev/null)
    raft_commit=$(echo "$cluster_json" | jq -r '.result.raft_info.commit // 0' 2>/dev/null)
    
    PEER_COUNTS[$node]=$peer_count
    CLUSTER_STATUS[$node]=$status
    RAFT_TERMS[$node]=$raft_term
    RAFT_COMMITS[$node]=$raft_commit
    
    if [[ "$peer_count" == "3" ]]; then
        pass "Peers: $peer_count (peer_id: ${peer_id:0:8}...)"
    else
        fail "Peers: $peer_count (expected 3)"
    fi
    
    if [[ "$status" == "enabled" ]]; then
        pass "Status: $status"
    else
        fail "Status: $status (expected: enabled)"
    fi
    
    info "Raft term: $raft_term, commit: $raft_commit"
done

# Cross-check: all nodes should see same peer count and term
echo ""
subsection "Cluster Consensus Check"

first_peer_count=""
first_term=""
consensus_ok=true

for node in "${!NODES[@]}"; do
    if [[ -z "$first_peer_count" ]]; then
        first_peer_count="${PEER_COUNTS[$node]:-}"
        first_term="${RAFT_TERMS[$node]:-}"
    else
        if [[ "${PEER_COUNTS[$node]:-}" != "$first_peer_count" ]]; then
            consensus_ok=false
        fi
        if [[ "${RAFT_TERMS[$node]:-}" != "$first_term" ]]; then
            consensus_ok=false
        fi
    fi
done

if $consensus_ok && [[ "$first_peer_count" == "3" ]]; then
    pass "All nodes agree: $first_peer_count peers, term $first_term"
else
    fail "Nodes disagree on cluster state!"
    for node in "${!NODES[@]}"; do
        info "$node: peers=${PEER_COUNTS[$node]:-?}, term=${RAFT_TERMS[$node]:-?}, commit=${RAFT_COMMITS[$node]:-?}"
    done
fi

##############################################################################
section "5. COLLECTION STATUS & REPLICATION"
##############################################################################

# Get collection info from bootstrap node (web1)
subsection "Collection: user_memories"

collection_json=$(remote_exec "web1" "curl -sf http://localhost:6333/collections/user_memories 2>/dev/null" || echo "{}")

if [[ "$collection_json" == "{}" ]]; then
    warn "Collection 'user_memories' not found - may need to create it"
else
    # Parse collection info
    shard_count=$(echo "$collection_json" | jq -r '.result.config.params.shard_number // 0' 2>/dev/null)
    repl_factor=$(echo "$collection_json" | jq -r '.result.config.params.replication_factor // 0' 2>/dev/null)
    write_cf=$(echo "$collection_json" | jq -r '.result.config.params.write_consistency_factor // 0' 2>/dev/null)
    points_count=$(echo "$collection_json" | jq -r '.result.points_count // 0' 2>/dev/null)
    status=$(echo "$collection_json" | jq -r '.result.status // "unknown"' 2>/dev/null)
    
    if [[ "$shard_count" == "3" ]]; then
        pass "Shard count: $shard_count"
    else
        warn "Shard count: $shard_count (recommended: 3)"
    fi
    
    if [[ "$repl_factor" == "3" ]]; then
        pass "Replication factor: $repl_factor"
    else
        warn "Replication factor: $repl_factor (recommended: 3)"
    fi
    
    if [[ "$write_cf" == "2" ]]; then
        pass "Write consistency factor: $write_cf"
    else
        info "Write consistency factor: $write_cf"
    fi
    
    info "Points count: $points_count"
    
    if [[ "$status" == "green" ]]; then
        pass "Collection status: $status"
    elif [[ "$status" == "yellow" ]]; then
        warn "Collection status: $status (some replicas may be syncing)"
    else
        fail "Collection status: $status"
    fi
fi

# Check shard distribution across nodes
echo ""
subsection "Shard Distribution"

cluster_info=$(remote_exec "web1" "curl -sf 'http://localhost:6333/collections/user_memories/cluster' 2>/dev/null" || echo "{}")

if [[ "$cluster_info" != "{}" ]]; then
    # Show shard locations
    echo "$cluster_info" | jq -r '.result.local_shards[]? | "  Shard \(.shard_id): \(.state)"' 2>/dev/null || info "Could not parse shard info"
    
    # Check for inactive shards
    inactive=$(echo "$cluster_info" | jq -r '[.result.local_shards[]? | select(.state != "Active")] | length' 2>/dev/null || echo "0")
    if [[ "$inactive" == "0" ]]; then
        pass "All local shards active"
    else
        warn "$inactive shard(s) not active"
    fi
fi

##############################################################################
section "6. API KEY VERIFICATION"
##############################################################################

# Check if API keys match between platform and memory service
subsection "API Key Configuration"

for node in "${!NODES[@]}"; do
    # Get SERVICE_API_KEY from qdrant-service container
    svc_key=$(remote_exec "$node" "docker exec synaplan-qdrant-service printenv SERVICE_API_KEY 2>/dev/null" || echo "")
    
    if [[ -z "$svc_key" ]]; then
        warn "$node: SERVICE_API_KEY not set in qdrant-service container"
    else
        # Mask the key for display
        masked_key="${svc_key:0:4}...${svc_key: -4}"
        info "$node: SERVICE_API_KEY = $masked_key"
    fi
done

# Check platform's configured key
echo ""
subsection "Platform API Key (from .env)"

platform_key=$(grep -E "^QDRANT_SERVICE_API_KEY=" /wwwroot/synaplan-platform/.env 2>/dev/null | cut -d= -f2 || echo "")
if [[ -n "$platform_key" ]]; then
    masked="${platform_key:0:4}...${platform_key: -4}"
    info "Platform QDRANT_SERVICE_API_KEY = $masked"
    
    # Compare with first node's key
    node1_key=$(remote_exec "web1" "docker exec synaplan-qdrant-service printenv SERVICE_API_KEY 2>/dev/null" || echo "")
    if [[ "$platform_key" == "$node1_key" ]]; then
        pass "API keys match between platform and memory service"
    else
        fail "API keys DO NOT match! Platform and qdrant-service have different keys"
    fi
else
    fail "QDRANT_SERVICE_API_KEY not found in platform .env"
fi

##############################################################################
section "7. DOCKER INTERNAL CONNECTIVITY TEST"
##############################################################################

# Test that the synaplan-platform container can reach qdrant-service
subsection "Platform → qdrant-service connectivity"

for node in "${!NODES[@]}"; do
    echo ""
    info "Testing from $node..."
    
    # Check if platform container exists
    platform_status=$(remote_exec "$node" "docker inspect -f '{{.State.Status}}' synaplan-platform 2>/dev/null" || echo "not_found")
    
    if [[ "$platform_status" != "running" ]]; then
        warn "$node: synaplan-platform container not running ($platform_status)"
        continue
    fi
    
    # Test docker-host resolution from platform container
    docker_host_ip=$(remote_exec "$node" "docker exec synaplan-platform getent hosts docker-host 2>/dev/null | awk '{print \$1}'" || echo "")
    if [[ -n "$docker_host_ip" ]]; then
        pass "$node: docker-host resolves to $docker_host_ip"
    else
        fail "$node: docker-host does not resolve"
    fi
    
    # Test connectivity from platform to qdrant-service
    health_check=$(remote_exec "$node" "docker exec synaplan-platform curl -sf http://docker-host:8090/health 2>/dev/null" || echo "FAILED")
    if [[ "$health_check" == *"status"* ]] || [[ "$health_check" == *"ok"* ]]; then
        pass "$node: Platform can reach qdrant-service"
    else
        fail "$node: Platform CANNOT reach qdrant-service"
        # Try to diagnose
        curl_verbose=$(remote_exec "$node" "docker exec synaplan-platform curl -v http://docker-host:8090/health 2>&1 | head -20" || echo "")
        if $VERBOSE; then
            info "Verbose curl output:"
            echo "$curl_verbose"
        fi
    fi
done

##############################################################################
section "8. END-TO-END MEMORY SERVICE TEST"
##############################################################################

subsection "Test memory operations via qdrant-service API"

# Pick a working node (prefer web1)
test_node="web1"

# Get API key
api_key=$(remote_exec "$test_node" "docker exec synaplan-qdrant-service printenv SERVICE_API_KEY 2>/dev/null" || echo "")

if [[ -z "$api_key" ]]; then
    warn "Cannot perform E2E test: no API key available"
else
    # Test health with API key
    health_auth=$(remote_exec "$test_node" "curl -sf -H 'X-API-Key: $api_key' http://localhost:8090/health 2>/dev/null" || echo "FAILED")
    if [[ "$health_auth" == *"ok"* ]] || [[ "$health_auth" == *"status"* ]]; then
        pass "Authenticated health check: OK"
    else
        fail "Authenticated health check failed"
    fi
    
    # Test stats endpoint
    stats=$(remote_exec "$test_node" "curl -sf -H 'X-API-Key: $api_key' http://localhost:8090/stats 2>/dev/null" || echo "FAILED")
    if [[ "$stats" == *"collection"* ]] || [[ "$stats" == *"points"* ]]; then
        pass "Stats endpoint: accessible"
        point_count=$(echo "$stats" | jq -r '.points_count // 0' 2>/dev/null || echo "?")
        info "Points in collection: $point_count"
    else
        warn "Stats endpoint: $stats"
    fi
fi

##############################################################################
section "SUMMARY"
##############################################################################

echo ""
if [[ $ERRORS -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}All tests passed!${NC}"
else
    echo -e "${RED}${BOLD}$ERRORS error(s) detected${NC}"
fi

if [[ $WARNINGS -gt 0 ]]; then
    echo -e "${YELLOW}$WARNINGS warning(s)${NC}"
fi

echo ""
echo "Next steps if issues found:"
echo "  1. Check container logs: docker compose logs -f"
echo "  2. Verify .env files have matching SERVICE_API_KEY values"
echo "  3. Ensure /qdrant/storage is on LOCAL disk (not NFS)"
echo "  4. Check firewall rules for port 6335 between nodes"
echo ""

exit $ERRORS
