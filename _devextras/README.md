# Synaplan Memory Service - Test & Diagnostic Scripts

This directory contains scripts for testing and diagnosing the Qdrant Memory Service cluster.

## Quick Start

From the management server (synastorev1):

```bash
# SSH to management server
ssh -p16803 root@s1

# Run comprehensive test
cd /wwwroot/synaplanCluster/synaplan-memories
./_devextras/test-full-cluster.sh
```

From a web node (web1/web2/web3):

```bash
# Quick local diagnosis
cd /netroot/synaplanCluster/synaplan-memories
./_devextras/diagnose-local.sh
```

## Scripts Overview

| Script | Where to Run | Purpose |
|--------|--------------|---------|
| `test-full-cluster.sh` | Management server | Comprehensive test of all nodes |
| `compare-nodes.sh` | Management server | Compare config across nodes to find differences |
| `test-docker-connectivity.sh` | Management server | Test Docker network: Platform → qdrant-service |
| `test-replication.sh` | Management server | Insert/verify/delete test point across nodes |
| `test-platform-memory-e2e.sh` | Management server | End-to-end test from Platform to Memory Service |
| `diagnose-local.sh` | Any web node | Quick local health check |
| `check-cluster-health.sh` | Management server | Basic cluster health (existing) |
| `check-cluster-sync.sh` | Management server | Cluster sync metrics (existing) |

## Common Issues & Solutions

### Issue: web1 works but web2/web3 don't

1. **Run comparison:**
   ```bash
   ./_devextras/compare-nodes.sh
   ```

2. **Look for differences in:**
   - API keys (must match across all nodes)
   - Container status (must be running)
   - Cluster peers (should be 3)

### Issue: API Key Mismatch

**Symptom:** Platform can't authenticate to qdrant-service

**Fix:**
1. Check the keys:
   ```bash
   # On management server
   grep SERVICE_API_KEY /wwwroot/synaplanCluster/synaplan-memories/qdrant-service/.env
   grep QDRANT_SERVICE_API_KEY /wwwroot/synaplanCluster/synaplan-platform/.env
   ```

2. Make them match, then restart:
   ```bash
   for host in web1 web2 web3; do
     ssh $host "cd /netroot/synaplanCluster/synaplan-memories && docker compose restart qdrant-service"
     ssh $host "cd /netroot/synaplanCluster/synaplan-platform && docker compose restart"
   done
   ```

### Issue: docker-host not resolving

**Symptom:** Platform container can't reach qdrant-service

**Fix:** Ensure `extra_hosts` is set in `docker-compose.yml`:
```yaml
extra_hosts:
  - "docker-host:host-gateway"
```

### Issue: Cluster won't form / Peers not joining

**Symptom:** Only 1 or 2 peers visible

**Fixes:**
1. Check P2P port connectivity:
   ```bash
   # From web2, check if web1's P2P port is reachable
   ssh web2 "nc -zv 10.0.0.2 6335"
   ```

2. Check firewall:
   ```bash
   ssh web1 "iptables -L -n | grep 6335"
   ```

3. Restart joining nodes:
   ```bash
   ssh web2 "cd /netroot/synaplanCluster/synaplan-memories && docker compose restart qdrant"
   ```

### Issue: Storage on NFS

**Symptom:** Qdrant crashes or corrupts data

**Fix:** Qdrant REQUIRES local storage. Move `/qdrant/storage` to local SSD:
```bash
ssh web2 "sudo mkdir -p /qdrant/storage && sudo chown 1000:1000 /qdrant/storage"
```

Ensure the start script uses `QDRANT_STORAGE_PATH=/qdrant/storage`.

### Issue: Replication not working

**Symptom:** Data inserted on one node doesn't appear on others

**Debug:**
```bash
./_devextras/test-replication.sh
```

**Fixes:**
1. Check collection config:
   ```bash
   ssh web1 "curl -s http://localhost:6333/collections/user_memories | jq '.result.config.params'"
   ```
   Should show: `replication_factor: 3`

2. Recreate collection with proper settings:
   ```bash
   ssh web1 'curl -X PUT "http://localhost:6333/collections/user_memories" \
     -H "Content-Type: application/json" \
     -d "{
       \"vectors\": { \"size\": 1024, \"distance\": \"Cosine\" },
       \"shard_number\": 3,
       \"replication_factor\": 3,
       \"write_consistency_factor\": 2
     }"'
   ```

## Verbose Mode

Most scripts support `--verbose` for detailed output:

```bash
./_devextras/test-full-cluster.sh --verbose
```

## Exit Codes

- `0` - All tests passed
- `>0` - Number of errors found

## Logs

View container logs:
```bash
# From any web node
docker compose logs -f qdrant
docker compose logs -f qdrant-service

# Or from management server
ssh web1 "cd /netroot/synaplanCluster/synaplan-memories && docker compose logs -f"
```
