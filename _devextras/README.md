# Synaplan Qdrant - Cluster Scripts

This directory contains scripts for deploying, testing, and diagnosing the Qdrant cluster.

## Quick Start

From the management server (synastorev1):

```bash
ssh -p16803 root@s1
cd /wwwroot/synaplanCluster/synaplan-memories
./_devextras/test-full-cluster.sh
```

From a web node (web1/web2/web3):

```bash
cd /netroot/synaplanCluster/synaplan-memories
./_devextras/diagnose-local.sh
```

## Scripts Overview

| Script | Where to Run | Purpose |
|--------|--------------|---------|
| `start-node1.sh` | web1 (via SSH) | Start bootstrap node |
| `start-node2.sh` | web2 (via SSH) | Start joining node |
| `start-node3.sh` | web3 (via SSH) | Start joining node |
| `restart.sh` | Any web node | Quick restart without rebuild |
| `stop.sh` | Any web node | Stop Qdrant on this node |
| `setup-collection.sh` | Any node (once) | Create collection with replication |
| `test-full-cluster.sh` | Management server | Comprehensive test of all nodes |
| `compare-nodes.sh` | Management server | Compare config across nodes |
| `test-docker-connectivity.sh` | Management server | Test Platform -> Qdrant connectivity |
| `test-replication.sh` | Management server | Insert/verify/delete test point |
| `test-platform-memory-e2e.sh` | Management server | End-to-end Platform -> Qdrant test |
| `diagnose-local.sh` | Any web node | Quick local health check |
| `check-cluster-health.sh` | Management server | Basic cluster health |
| `check-cluster-sync.sh` | Management server | Cluster sync metrics |

## Common Issues

### Cluster won't form / Peers not joining

1. Check P2P port connectivity:
   ```bash
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

### Storage on NFS

Qdrant REQUIRES local storage. Move `/qdrant/storage` to local SSD:
```bash
ssh web2 "sudo mkdir -p /qdrant/storage && sudo chown 1000:1000 /qdrant/storage"
```

### Replication not working

```bash
./_devextras/test-replication.sh
```

Check collection config:
```bash
ssh web1 "curl -s http://localhost:6333/collections/user_memories | jq '.result.config.params'"
```
Should show: `replication_factor: 3`

## Verbose Mode

Most scripts support `--verbose` for detailed output:

```bash
./_devextras/test-full-cluster.sh --verbose
```

## Logs

```bash
# From any web node
docker compose logs -f qdrant

# Or from management server
ssh web1 "cd /netroot/synaplanCluster/synaplan-memories && docker compose logs -f"
```
