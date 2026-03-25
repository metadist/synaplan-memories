#!/bin/bash
# test-platform-memory-e2e.sh - Full platform-to-Qdrant integration report
#
# Tests the complete chain: Platform container -> docker-host -> Qdrant
# including env config, network, Qdrant health, collections, data, and
# Synaplan's own memory-service check endpoint.
#
# Run from management server (synastorev1).

set -euo pipefail

# Node configuration
declare -A NODES=(
    ["web1"]="10.0.0.2"
    ["web2"]="10.0.0.7"
    ["web3"]="10.0.0.8"
)

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

ERRORS=0
WARNINGS=0
PASS_COUNT=0

pass()    { echo -e "  ${GREEN}OK${NC}    $1"; ((PASS_COUNT++)) || true; }
fail()    { echo -e "  ${RED}FAIL${NC}  $1"; ((ERRORS++)) || true; }
warn()    { echo -e "  ${YELLOW}WARN${NC}  $1"; ((WARNINGS++)) || true; }
info()    { echo -e "  ${CYAN}INFO${NC}  $1"; }
section() { echo -e "\n${BOLD}${BLUE}━━━ $1 ━━━${NC}\n"; }

remote() {
    local node=$1; shift
    ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no -o BatchMode=yes "$node" "$@" 2>/dev/null
}

platform_exec() {
    local node=$1; shift
    remote "$node" "docker exec synaplan-platform $*"
}

echo -e "${BOLD}${BLUE}"
echo "╔═══════════════════════════════════════════════════════════╗"
echo "║     Synaplan Platform <-> Qdrant Integration Report      ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo -e "${NC}"
echo "  Date: $(date)"
echo "  Run from: $(hostname)"
echo ""

for node_name in web1 web2 web3; do
    node_ip="${NODES[$node_name]}"

    echo -e "${BOLD}${BLUE}═══════════════════════════════════════════════════════════${NC}"
    echo -e "${BOLD}  $node_name ($node_ip)${NC}"
    echo -e "${BOLD}${BLUE}═══════════════════════════════════════════════════════════${NC}"

    # ── 1. Container status ──────────────────────────────────────
    section "1. Container Status"

    qdrant_status=$(remote "$node_name" "docker inspect -f '{{.State.Status}}' synaplan-qdrant" || echo "not_found")
    platform_status=$(remote "$node_name" "docker inspect -f '{{.State.Status}}' synaplan-platform" || echo "not_found")
    platform_health=$(remote "$node_name" "docker inspect -f '{{.State.Health.Status}}' synaplan-platform" || echo "unknown")

    qdrant_image=$(remote "$node_name" "docker inspect -f '{{.Config.Image}}' synaplan-qdrant" || echo "?")
    platform_image=$(remote "$node_name" "docker inspect -f '{{.Config.Image}}' synaplan-platform" || echo "?")

    if [[ "$qdrant_status" == "running" ]]; then
        pass "synaplan-qdrant: running ($qdrant_image)"
    else
        fail "synaplan-qdrant: $qdrant_status"
    fi

    if [[ "$platform_status" == "running" ]]; then
        pass "synaplan-platform: running ($platform_image)"
    else
        fail "synaplan-platform: $platform_status"
        echo ""
        warn "Skipping remaining tests for $node_name (platform not running)"
        echo ""
        continue
    fi

    if [[ "$platform_health" == "healthy" ]]; then
        pass "Platform health: $platform_health"
    else
        warn "Platform health: $platform_health"
    fi

    # Check for orphaned qdrant-service container
    old_svc=$(remote "$node_name" "docker inspect -f '{{.State.Status}}' synaplan-qdrant-service" || echo "not_found")
    if [[ "$old_svc" != "not_found" ]]; then
        warn "Old synaplan-qdrant-service still exists (status: $old_svc) -- should be removed"
    fi

    # ── 2. Platform env config ───────────────────────────────────
    section "2. Platform Environment"

    qdrant_url=$(platform_exec "$node_name" "printenv QDRANT_URL" || echo "NOT_SET")
    vector_provider=$(platform_exec "$node_name" "printenv VECTOR_STORAGE_PROVIDER" || echo "NOT_SET")
    docs_collection=$(platform_exec "$node_name" "printenv QDRANT_DOCUMENTS_COLLECTION" || echo "NOT_SET")

    if [[ "$qdrant_url" == *":6333"* ]]; then
        pass "QDRANT_URL = $qdrant_url"
    elif [[ "$qdrant_url" == *":8090"* ]]; then
        fail "QDRANT_URL = $qdrant_url (still pointing to old qdrant-service port!)"
    elif [[ "$qdrant_url" == "NOT_SET" ]]; then
        fail "QDRANT_URL is not set"
    else
        warn "QDRANT_URL = $qdrant_url (unexpected format)"
    fi

    if [[ "$vector_provider" == "qdrant" ]]; then
        pass "VECTOR_STORAGE_PROVIDER = $vector_provider"
    elif [[ "$vector_provider" == "mariadb" ]]; then
        info "VECTOR_STORAGE_PROVIDER = $vector_provider (using MariaDB, not Qdrant)"
    else
        warn "VECTOR_STORAGE_PROVIDER = $vector_provider"
    fi

    info "QDRANT_DOCUMENTS_COLLECTION = $docs_collection"

    # Check for leftover old env vars
    old_svc_url=$(platform_exec "$node_name" "printenv QDRANT_SERVICE_URL" || echo "")
    old_svc_key=$(platform_exec "$node_name" "printenv QDRANT_SERVICE_API_KEY" || echo "")
    if [[ -n "$old_svc_url" ]]; then
        warn "Old QDRANT_SERVICE_URL still set: $old_svc_url (unused but should be cleaned up)"
    fi
    if [[ -n "$old_svc_key" ]]; then
        warn "Old QDRANT_SERVICE_API_KEY still set (unused but should be cleaned up)"
    fi

    # ── 3. Network connectivity ──────────────────────────────────
    section "3. Network: Platform -> Qdrant"

    # docker-host resolution
    dh_ip=$(platform_exec "$node_name" "getent hosts docker-host 2>/dev/null | awk '{print \$1}'" || echo "")
    if [[ -n "$dh_ip" ]]; then
        pass "docker-host resolves to $dh_ip"
    else
        fail "docker-host does NOT resolve inside platform container"
    fi

    # Direct Qdrant health from platform container
    health_resp=$(platform_exec "$node_name" "curl -sf --connect-timeout 5 '$qdrant_url/healthz'" || echo "FAILED")
    if [[ "$health_resp" == *"passed"* ]] || [[ "$health_resp" == *"ok"* ]] || [[ "$health_resp" != "FAILED" ]]; then
        pass "Platform -> $qdrant_url/healthz: reachable"
    else
        fail "Platform -> $qdrant_url/healthz: NOT reachable"
    fi

    # Qdrant version from telemetry
    telemetry=$(platform_exec "$node_name" "curl -sf --connect-timeout 5 '$qdrant_url/telemetry'" || echo "")
    if [[ -n "$telemetry" ]]; then
        qdrant_ver=$(echo "$telemetry" | jq -r '.result.app.version // "?"' 2>/dev/null || echo "?")
        info "Qdrant version: $qdrant_ver"
    fi

    # ── 4. Qdrant collections ────────────────────────────────────
    section "4. Qdrant Collections"

    collections_resp=$(platform_exec "$node_name" "curl -sf --connect-timeout 5 '$qdrant_url/collections'" || echo "FAILED")

    if [[ "$collections_resp" == "FAILED" ]] || [[ "$collections_resp" == *'"status":"error"'* ]]; then
        http_code=$(platform_exec "$node_name" "curl -s -o /dev/null -w '%{http_code}' --connect-timeout 5 '$qdrant_url/collections'" || echo "000")
        if [[ "$http_code" == "401" ]]; then
            fail "Collections endpoint returned 401 -- Qdrant API key auth is active"
            info "Check: docker exec synaplan-qdrant printenv QDRANT__SERVICE__API_KEY"
            info "The PHP backend does not send an api-key header. Disable Qdrant auth or update backend."
        else
            fail "Cannot fetch collections (HTTP $http_code)"
        fi
    else
        coll_names=$(echo "$collections_resp" | jq -r '.result.collections[].name' 2>/dev/null || echo "")
        coll_count=$(echo "$collections_resp" | jq -r '.result.collections | length' 2>/dev/null || echo "0")
        pass "Collections endpoint accessible ($coll_count collections)"

        for coll in user_memories user_documents; do
            coll_resp=$(platform_exec "$node_name" "curl -sf --connect-timeout 5 '$qdrant_url/collections/$coll'" || echo "FAILED")
            if [[ "$coll_resp" != "FAILED" ]] && [[ "$coll_resp" == *'"result"'* ]]; then
                points=$(echo "$coll_resp" | jq -r '.result.points_count // 0' 2>/dev/null || echo "?")
                vectors=$(echo "$coll_resp" | jq -r '.result.vectors_count // 0' 2>/dev/null || echo "?")
                status=$(echo "$coll_resp" | jq -r '.result.status // "?"' 2>/dev/null || echo "?")
                segments=$(echo "$coll_resp" | jq -r '.result.segments_count // "?"' 2>/dev/null || echo "?")
                vec_size=$(echo "$coll_resp" | jq -r '.result.config.params.vectors.size // "?"' 2>/dev/null || echo "?")
                repl=$(echo "$coll_resp" | jq -r '.result.config.params.replication_factor // "?"' 2>/dev/null || echo "?")
                shards=$(echo "$coll_resp" | jq -r '.result.config.params.shard_number // "?"' 2>/dev/null || echo "?")

                if [[ "$status" == "green" ]]; then
                    pass "$coll: $points points, $vectors vectors (status: $status)"
                elif [[ "$status" == "yellow" ]]; then
                    warn "$coll: $points points, $vectors vectors (status: $status -- syncing?)"
                else
                    fail "$coll: status=$status"
                fi
                info "  dim=$vec_size  shards=$shards  replication=$repl  segments=$segments"
            else
                info "$coll: not found (will be created on first use)"
            fi
        done
    fi

    # ── 5. Cluster status ────────────────────────────────────────
    section "5. Qdrant Cluster"

    cluster_resp=$(platform_exec "$node_name" "curl -sf --connect-timeout 5 '$qdrant_url/cluster'" || echo "FAILED")

    if [[ "$cluster_resp" == "FAILED" ]]; then
        http_code=$(platform_exec "$node_name" "curl -s -o /dev/null -w '%{http_code}' --connect-timeout 5 '$qdrant_url/cluster'" || echo "000")
        if [[ "$http_code" == "401" ]]; then
            warn "Cluster endpoint returned 401 (API key auth active)"
        else
            warn "Cannot fetch cluster status (HTTP $http_code)"
        fi
    else
        cl_status=$(echo "$cluster_resp" | jq -r '.result.status // "disabled"' 2>/dev/null || echo "?")
        cl_peers=$(echo "$cluster_resp" | jq -r '.result.peers | keys | length' 2>/dev/null || echo "0")
        cl_peer_id=$(echo "$cluster_resp" | jq -r '.result.peer_id // "?"' 2>/dev/null || echo "?")
        cl_term=$(echo "$cluster_resp" | jq -r '.result.raft_info.term // "?"' 2>/dev/null || echo "?")
        cl_commit=$(echo "$cluster_resp" | jq -r '.result.raft_info.commit // "?"' 2>/dev/null || echo "?")

        if [[ "$cl_status" == "enabled" ]] && [[ "$cl_peers" == "3" ]]; then
            pass "Cluster: $cl_status, $cl_peers peers"
        elif [[ "$cl_status" == "enabled" ]]; then
            warn "Cluster: $cl_status, but only $cl_peers peer(s) (expected 3)"
        elif [[ "$cl_status" == "disabled" ]]; then
            warn "Cluster: DISABLED (single-node mode -- start-node script not used?)"
        else
            warn "Cluster: $cl_status ($cl_peers peers)"
        fi
        info "  peer_id=${cl_peer_id:0:12}  raft_term=$cl_term  raft_commit=$cl_commit"
    fi

    # ── 6. Synaplan app-level check ──────────────────────────────
    section "6. Synaplan Memory-Service Check API"

    # This is the endpoint the frontend calls to check if Qdrant works end-to-end
    # It requires an authenticated user, so we test via internal localhost
    app_check=$(remote "$node_name" "docker exec synaplan-platform curl -sf --connect-timeout 10 'http://localhost/api/v1/config/memory-service/check'" || echo "FAILED")

    if [[ "$app_check" == "FAILED" ]]; then
        http_code=$(remote "$node_name" "docker exec synaplan-platform curl -s -o /dev/null -w '%{http_code}' --connect-timeout 10 'http://localhost/api/v1/config/memory-service/check'" || echo "000")
        if [[ "$http_code" == "401" ]]; then
            info "memory-service/check returned 401 (auth required -- expected for unauthenticated call)"
            info "Test via browser: https://web.synaplan.com/api/v1/config/memory-service/check"
        else
            warn "memory-service/check returned HTTP $http_code"
        fi
    else
        available=$(echo "$app_check" | jq -r '.available // false' 2>/dev/null || echo "?")
        configured=$(echo "$app_check" | jq -r '.configured // false' 2>/dev/null || echo "?")
        if [[ "$available" == "true" ]]; then
            pass "Synaplan reports: available=$available, configured=$configured"
        elif [[ "$configured" == "true" ]]; then
            fail "Synaplan reports: configured=$configured but available=$available"
        else
            warn "Synaplan reports: configured=$configured, available=$available"
        fi
    fi

    # ── 7. Qdrant logs (recent errors) ───────────────────────────
    section "7. Recent Qdrant Errors"

    error_lines=$(remote "$node_name" "docker logs synaplan-qdrant 2>&1 | grep -iE 'error|panic|fatal|WARN' | tail -5" || echo "")
    if [[ -z "$error_lines" ]]; then
        pass "No recent errors in Qdrant logs"
    else
        warn "Recent log entries:"
        echo "$error_lines" | while IFS= read -r line; do
            echo -e "    ${YELLOW}$line${NC}"
        done
    fi

    echo ""
done

# ── Summary ──────────────────────────────────────────────────────
echo -e "${BOLD}${BLUE}"
echo "╔═══════════════════════════════════════════════════════════╗"
echo "║                        SUMMARY                           ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo -e "${NC}"

echo -e "  ${GREEN}Passed:   $PASS_COUNT${NC}"
echo -e "  ${RED}Failed:   $ERRORS${NC}"
echo -e "  ${YELLOW}Warnings: $WARNINGS${NC}"
echo ""

if [[ $ERRORS -eq 0 ]]; then
    echo -e "  ${GREEN}${BOLD}All critical checks passed.${NC}"
else
    echo -e "  ${RED}${BOLD}$ERRORS critical failure(s) detected.${NC}"
    echo ""
    echo "  Common fixes:"
    echo "    - 401 from Qdrant: unset QDRANT_API_KEY in .env or pass api-key header"
    echo "    - Cluster disabled: restart with start-node*.sh scripts"
    echo "    - Old QDRANT_SERVICE_URL: update platform .env, remove old vars"
    echo "    - Old qdrant-service container: docker stop/rm synaplan-qdrant-service"
fi

echo ""
exit $ERRORS
