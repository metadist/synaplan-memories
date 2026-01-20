# Qdrant Memories Integration

Rust microservice providing vector search for Synaplan user memories using Qdrant.

## Quick Start

```bash
# Start services
docker compose up -d

# Check health (with metrics)
curl http://localhost:8090/health

# Get Prometheus metrics
curl http://localhost:8090/metrics

# Get service info (version, stats, etc.) - requires API key
curl -H "X-API-Key: your-api-key" http://localhost:8090/info

# View logs
./logs.sh              # Follow logs in real-time
./logs.sh tail 100     # Show last 100 lines
./logs.sh test         # Test health + show request logs

# Run tests
./test_integration.sh
```

## API Endpoints

### Public Endpoints (No Auth)
- `GET /health` - Health check with metrics (uptime, request stats, Qdrant status)
- `GET /metrics` - Prometheus metrics endpoint

### Protected Endpoints (Require API Key via `X-API-Key` header)
- `GET /info` - Service info (version, stats, collection info)
- `POST /memories` - Upsert memory
- `GET /memories/:point_id` - Get memory
- `DELETE /memories/:point_id` - Delete memory
- `POST /memories/search` - Vector search
- `POST /memories/scroll` - List all memories
- `GET /collection/info` - Collection stats

## Health Check Response

```json
{
  "status": "healthy",
  "service": "synaplan-qdrant-service",
  "version": "0.1.0",
  "uptime_seconds": 3600,
  "qdrant": {
    "status": "connected",
    "collection_status": "Green",
    "points_count": 1234,
    "vectors_count": 1234
  },
  "metrics": {
    "requests_total": 5678,
    "requests_failed": 12,
    "requests_success": 5666,
    "success_rate_percent": "99.79"
  }
}
```

## Service Info Response

```json
{
  "service": "synaplan-qdrant-service",
  "version": "0.1.0",
  "rust_version": "1.75",
  "status": "healthy",
  "collection": {
    "status": "green",
    "points_count": 1234,
    "vectors_count": 1234,
    "indexed_vectors_count": 1234
  }
}
```

## Prometheus Metrics

Available at `/metrics`:

```
# Request metrics
requests_total                   # Total requests received
requests_failed                  # Failed requests (4xx/5xx)
request_duration_seconds         # Request duration histogram

# Service metrics
uptime_seconds                   # Service uptime
qdrant_points_total              # Total points in Qdrant
qdrant_vectors_total             # Total vectors in Qdrant
```

**Integration example (prometheus.yml):**
```yaml
scrape_configs:
  - job_name: 'qdrant-service'
    static_configs:
      - targets: ['qdrant-service:8090']
    metrics_path: '/metrics'
```

## TLS/HTTPS Support

Enable HTTPS with the `tls` feature (optional):

```bash
# Build with TLS support
docker build --build-arg CARGO_FEATURES="tls" -t synaplan-memories:latest .

# Configure TLS (docker-compose.yml or .env)
TLS_ENABLED=true
TLS_CERT_PATH=/path/to/cert.pem
TLS_KEY_PATH=/path/to/key.pem
```

**Production Recommendation:** Use a reverse proxy (Nginx/Caddy) for TLS termination instead of built-in TLS.

## Monitoring & Logs

**View live logs:**
```bash
./logs.sh follow       # or just: ./logs.sh
```

**Test & see request logs:**
```bash
./logs.sh test
```

**What you'll see in logs:**
```
[DEBUG] request{method=POST uri=/memories ...}: started processing request
[DEBUG] request{...}: finished processing request latency=2 ms status=200
[INFO] Memory upserted: mem_1_123
```

**Log levels:**
- `DEBUG`: Request details (method, URI, latency, status)
- `INFO`: Service events (startup, memory operations)
- `WARN`: Non-critical issues
- `ERROR`: Critical failures

## Architecture

- **Qdrant** (v1.12.5): Vector database (port 6333/6334)
- **Rust Service** (port 8090): REST API gateway to Qdrant with metrics
- **PHP Backend**: Uses `QdrantClientHttp` to call Rust service

## Configuration

Copy `.env.example` in `qdrant-service/` and set:
- `QDRANT_URL`: Qdrant gRPC URL (default: `http://qdrant:6334`)
- `QDRANT_VECTOR_DIMENSION`: Embedding size (default: `1024` for BGE-M3)
- `SERVICE_API_KEY`: Auth key for service access (required in production)
- `DISCORD_WEBHOOK_URL`: Discord webhook for alerts (optional)
- `RUST_LOG`: Log level (default: `synaplan_qdrant_service=debug,tower_http=info`)
- `TLS_ENABLED`: Enable HTTPS (default: `false`)
- `TLS_CERT_PATH`: Path to TLS certificate (if TLS enabled)
- `TLS_KEY_PATH`: Path to TLS private key (if TLS enabled)

## Discord Alerts 🔔

The service can send alerts to Discord for important events:

### Setup:
1. Create a Discord webhook: Server Settings → Integrations → Webhooks → New Webhook
2. Copy the webhook URL
3. Set environment variable:
   ```bash
   DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/YOUR_ID/YOUR_TOKEN
   ```

### What gets alerted:

**ℹ️ Info (Blue):**
- Service started/online

**⚠️ Warning (Orange):**
- Service stopping
- High collection usage (>100k points)

**❌ Error (Red):**
- High error rate (>5% of requests failing)

**🚨 Critical (@here ping, Dark Red):**
- Cannot connect to Qdrant database
- Service panic/crash

Example alert:
```
@here **CRITICAL ALERT**

🚨 Qdrant Connection Failed
Cannot connect to Qdrant database: connection refused

Synaplan Qdrant Microservice
2026-01-20T23:45:00Z
```

## Performance

- **Single instance**: ~200 req/s (upsert + search)
- **5 replicas**: ~1,000 req/s
- **Qdrant cluster**: 10,000+ req/s

See `SCALING.md` for production setup.

## Testing

See `TESTING.md` for details.

## Files

- `qdrant-service/`: Rust microservice code
- `logs.sh`: Log monitoring helper
- `test_integration.sh`: E2E tests
- `TESTING.md`: Test guide
- `SCALING.md`: Production scaling guide
