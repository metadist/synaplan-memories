# Scaling Guide

## Rust Microservice (Stateless)

**Horizontal scaling:** Run multiple instances behind a load balancer.

```yaml
# docker-compose.yml (example)
qdrant-service:
  deploy:
    replicas: 5
```

**Performance optimizations:**
- Release build with LTO: `cargo build --release`
- Tokio async runtime (multi-threaded)
- Connection pooling via `qdrant-client`

**Benchmarks (single instance):**
- Upsert: ~200 req/s
- Search: ~200 req/s

**Expected throughput:**
- 3 replicas: ~600 req/s
- 5 replicas: ~1,000 req/s
- 10 replicas: ~2,000 req/s

## Qdrant Database

**Horizontal scaling:** Use Qdrant distributed cluster.

```yaml
# Kubernetes example
qdrant:
  replicas: 3  # Sharding + Replication
  env:
    - QDRANT__CLUSTER__ENABLED: "true"
```

**Features:**
- **Sharding**: Distribute data across nodes
- **Replication**: Replicate shards for high availability
- **Raft consensus**: Cluster coordination

**Expected throughput:**
- Single node: 1,000-5,000 req/s
- 3-node cluster: 10,000+ req/s

## Data Storage

- **Where**: Data stored in Qdrant (not in Rust service)
- **Persistence**: Docker volume or PersistentVolumeClaims (K8s)
- **Backups**: Use Qdrant snapshots API

## API Key Security

**Development:**
```env
SERVICE_API_KEY=dev-key  # or empty
```

**Production:**
```bash
# Generate secure key
openssl rand -hex 32

# Use in Kubernetes Secrets
kubectl create secret generic qdrant-api-key \
  --from-literal=key=$(openssl rand -hex 32)
```

**Headers:**
```bash
curl -H "X-API-Key: your-key" http://qdrant-service:8090/health
```

## Load Balancer Setup

**Nginx:**
```nginx
upstream qdrant_service {
    server qdrant-1:8090;
    server qdrant-2:8090;
    server qdrant-3:8090;
}

server {
    location /qdrant/ {
        proxy_pass http://qdrant_service/;
    }
}
```

**Kubernetes Service:**
```yaml
apiVersion: v1
kind: Service
metadata:
  name: qdrant-service
spec:
  selector:
    app: qdrant-service
  ports:
    - port: 8090
  type: LoadBalancer
```

## Monitoring

**Health Check:**
```bash
curl http://localhost:8090/health
```

**Response:**
```json
{
  "status": "healthy",
  "service": "synaplan-qdrant-service",
  "qdrant": "connected"
}
```

**Metrics to track:**
- Request latency (p50, p95, p99)
- Throughput (req/s)
- Error rate
- Qdrant collection size

## Production Checklist

- [ ] API key configured (min 32 chars)
- [ ] TLS/HTTPS enabled
- [ ] Firewall rules (only backend → service)
- [ ] Health checks configured
- [ ] Backup strategy implemented
- [ ] Monitoring/alerting setup
- [ ] Load balancer configured
- [ ] Log aggregation enabled
