# Qdrant Cluster Deployment Guide

This document describes deploying the Synaplan Memory Service (Qdrant + qdrant-service) as a 3-node cluster.

## Quick Reference

```bash
# Connect to management server
ssh -p16803 root@synastorev1.synaplan.com

# Start cluster (initial or after full shutdown)
ssh web1 "cd /netroot/synaplanCluster/synaplan-memories && ./start-node1.sh"
ssh web2 "cd /netroot/synaplanCluster/synaplan-memories && ./start-node2.sh"
ssh web3 "cd /netroot/synaplanCluster/synaplan-memories && ./start-node3.sh"

# Check cluster status
ssh web1 "curl -s http://localhost:6333/cluster | jq '.result.peers | keys | length'"
```

## Infrastructure

| Host | Alias | IP | Role |
|------|-------|-----|------|
| synweb100 | web1 | 10.0.0.2 | Bootstrap node |
| synweb101 | web2 | 10.0.0.7 | Joining node |
| synweb102 | web3 | 10.0.0.8 | Joining node |

**Management Server**: synastorev1.synaplan.com (SSH port 16803)

## Directory Structure

| Server | Path | Type |
|--------|------|------|
| synastorev1 | `/wwwroot/synaplanCluster/synaplan-memories/` | Source (NFS export) |
| web1/web2/web3 | `/netroot/synaplanCluster/synaplan-memories/` | NFS mount |
| web1/web2/web3 | `/qdrant/storage/` | **Local SSD** (per-node) |

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Memory Service Cluster                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│   web1 (10.0.0.2)      web2 (10.0.0.7)      web3 (10.0.0.8)    │
│                                                                  │
│   ┌─────────────┐      ┌─────────────┐      ┌─────────────┐     │
│   │qdrant-svc   │      │qdrant-svc   │      │qdrant-svc   │     │
│   │ :8090       │      │ :8090       │      │ :8090       │     │
│   └──────┬──────┘      └──────┬──────┘      └──────┬──────┘     │
│          │                    │                    │             │
│   ┌──────▼──────┐      ┌──────▼──────┐      ┌──────▼──────┐     │
│   │ Qdrant      │◄────►│ Qdrant      │◄────►│ Qdrant      │     │
│   │ :6333/:6334 │ P2P  │ :6333/:6334 │ P2P  │ :6333/:6334 │     │
│   │ :6335       │      │ :6335       │      │ :6335       │     │
│   └──────┬──────┘      └──────┬──────┘      └──────┬──────┘     │
│          │                    │                    │             │
│   /qdrant/storage      /qdrant/storage      /qdrant/storage     │
│   (Local SSD)          (Local SSD)          (Local SSD)         │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Why Local SSD?

**Qdrant explicitly warns against NFS** for storage:

- **mmap**: Qdrant uses memory-mapped files; NFS doesn't handle mmap well
- **Locking**: File-based locking doesn't work reliably over NFS
- **Latency**: Vector operations need low-latency storage

**Rule**: `/qdrant/storage` must be on LOCAL disk, never on `/netroot/` (NFS).

## Initial Deployment

From synastorev1:

### 1. Prepare local storage on all nodes

```bash
for host in web1 web2 web3; do
  ssh $host "sudo mkdir -p /qdrant/storage && sudo chown -R 1000:1000 /qdrant"
done
```

### 2. Configure environment

```bash
cd /wwwroot/synaplanCluster/synaplan-memories

# Create .env from template
cp .env.example .env

# Generate and set API key
APIKEY=$(openssl rand -hex 32)
sed -i "s/changeme-in-production/$APIKEY/" .env

# Verify
cat .env
```

**Important**: Copy the same `SERVICE_API_KEY` to the platform's `.env` as `QDRANT_SERVICE_API_KEY`.

### 3. Start bootstrap node first

```bash
ssh web1 "cd /netroot/synaplanCluster/synaplan-memories && ./start-node1.sh"
```

Wait for health check to pass:

```bash
ssh web1 "curl -s http://localhost:6333/healthz"
```

### 4. Start joining nodes

```bash
ssh web2 "cd /netroot/synaplanCluster/synaplan-memories && ./start-node2.sh"
ssh web3 "cd /netroot/synaplanCluster/synaplan-memories && ./start-node3.sh"
```

### 5. Verify cluster

```bash
ssh web1 "curl -s http://localhost:6333/cluster | jq '.result.peers | keys | length'"
# Should return: 3
```

### 6. Create collection with replication

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

## Ports

| Port | Protocol | Purpose | Access |
|------|----------|---------|--------|
| 6333 | HTTP | Qdrant REST API | localhost only |
| 6334 | gRPC | Qdrant gRPC API | Docker network |
| 6335 | TCP | P2P cluster sync | 10.0.0.x network |
| 8090 | HTTP | qdrant-service REST | Host (for backend) |

## Operations

### Restart all nodes

After cluster is formed, order doesn't matter:

```bash
for host in web1 web2 web3; do
  ssh $host "cd /netroot/synaplanCluster/synaplan-memories && docker compose restart" &
done
wait
```

### Check cluster health

```bash
ssh web1 "curl -s http://localhost:6333/cluster | jq"
```

### View logs

```bash
ssh web1 "cd /netroot/synaplanCluster/synaplan-memories && docker compose logs -f"
```

## Recovery

### After node restart

Qdrant automatically:
1. Reconnects via Raft consensus
2. Syncs missed operations via WAL
3. Re-enables replica

### After node replacement

If a node is rebuilt:

```bash
# 1. Create local storage on new node
ssh web2 "sudo mkdir -p /qdrant/storage && sudo chown -R 1000:1000 /qdrant"

# 2. Remove old peer (run on healthy node)
OLD_PEER_ID=$(ssh web1 "curl -s http://localhost:6333/cluster | jq -r '.result.peers | keys[1]'")
ssh web1 "curl -X DELETE 'http://localhost:6333/cluster/peer/${OLD_PEER_ID}?force=true'"

# 3. Start new node
ssh web2 "cd /netroot/synaplanCluster/synaplan-memories && ./start-node2.sh"
```

## Backup

Create snapshots and sync to NFS:

```bash
# Create snapshot
ssh web1 "curl -X POST 'http://localhost:6333/collections/user_memories/snapshots'"

# List snapshots
ssh web1 "curl -s 'http://localhost:6333/collections/user_memories/snapshots' | jq"

# Sync to NFS backup
ssh web1 "rsync -av /qdrant/storage/snapshots/ /netroot/backups/qdrant/web1/"
```

## Security Notes

1. **P2P port (6335)**: Only accessible on internal 10.0.0.x network
2. **REST API (6333)**: Bound to localhost only in production
3. **qdrant-service (8090)**: Protected by SERVICE_API_KEY
4. **Consider enabling**: Qdrant native authentication (`QDRANT_API_KEY`) and TLS

## Troubleshooting

### Cluster won't form

```bash
# Check if bootstrap is reachable
curl -v http://10.0.0.2:6335

# Check Qdrant logs
docker compose logs qdrant
```

### Node won't join

```bash
# Verify storage is local (not NFS)
mount | grep /qdrant  # Should be empty

# Check bootstrap connectivity
curl -sf http://10.0.0.2:6335
```

### High memory usage

Qdrant uses mmap; RSS may look high but actual memory usage is lower.
Check with: `docker stats synaplan-qdrant`
