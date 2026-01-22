# Synaplan Qdrant Microservice

High-performance vector storage and search microservice for AI memories, RAG documents, and false positives detection.

## Features

- **Vector Storage & Search**: Fast semantic search using Qdrant vector database
- **Namespace Support**: Separate collections for memories, RAG docs, false positives
- **Metrics & Monitoring**: Prometheus metrics, health checks, webhook alerts
- **Security**: API key authentication, optional TLS support
- **Performance**: Production-ready Rust implementation with connection pooling

## Architecture

```
Backend (PHP) → Qdrant Microservice (Rust) → Qdrant Database
                        ↓
                   Webhook Alerts
                   (Discord/Slack/Telegram)
```

**Responsibilities:**
- ✅ Store pre-computed vectors (1024-dim BGE-M3)
- ✅ Semantic search with filters (user_id, category, min_score)
- ✅ Collection management & health monitoring
- ❌ **NOT** responsible for embedding generation (handled by backend)

## Quick Start

### Prerequisites

```bash
docker compose up -d
```

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `QDRANT_URL` | No | `http://localhost:6334` | Qdrant gRPC endpoint |
| `QDRANT_API_KEY` | No | - | Qdrant API key (if enabled) |
| `QDRANT_COLLECTION_NAME` | No | `user_memories` | Collection name |
| `QDRANT_VECTOR_DIMENSION` | No | `1024` | Vector dimension (BGE-M3) |
| `PORT` | No | `8090` | Service HTTP port |
| `SERVICE_API_KEY` | No | - | API key for authentication |
| `WEBHOOK_URL` | No | - | Webhook URL for alerts (Discord/Slack/Telegram) |
| `ENABLE_DAILY_STATS` | No | `false` | Enable daily statistics reports to webhook |
| `STATS_INTERVAL_HOURS` | No | `24` | Interval in hours between stats reports |
| `RUST_LOG` | No | `info` | Log level |

### Example Configuration

```bash
# docker-compose.yml or .env
QDRANT_URL=http://qdrant:6334
QDRANT_COLLECTION_NAME=user_memories
QDRANT_VECTOR_DIMENSION=1024
PORT=8090
SERVICE_API_KEY=your-secret-key
WEBHOOK_URL=https://discord.com/api/webhooks/...
ENABLE_DAILY_STATS=true
STATS_INTERVAL_HOURS=24
RUST_LOG=synaplan_qdrant_service=info,tower_http=info
```

## API Endpoints

### Health & Status

```bash
# Health check (unauthenticated)
GET /health

# Service info (requires auth)
GET /info

# Collection info (requires auth)
GET /collection/info

# Capabilities (unauthenticated, cacheable)
GET /capabilities
```

### Vector Operations

All endpoints require API key authentication via `X-API-Key` header.

#### Upsert Memory

```bash
POST /memories
Content-Type: application/json
X-API-Key: your-secret-key

{
  "point_id": "mem_1730_123456",
  "vector": [0.1, 0.2, ...], // 1024-dim array
  "payload": {
    "user_id": 1730,
    "category": "personal",
    "key": "name",
    "value": "Yusuf Senel"
  }
}
```

#### Search Memories

```bash
POST /memories/search
Content-Type: application/json
X-API-Key: your-secret-key

{
  "vector": [0.1, 0.2, ...], // 1024-dim query vector
  "user_id": 1730,
  "category": "personal", // optional
  "limit": 15,
  "min_score": 0.35
}
```

#### Scroll (List All)

```bash
POST /memories/scroll
Content-Type: application/json
X-API-Key: your-secret-key

{
  "user_id": 1730,
  "category": "personal", // optional
  "limit": 100
}
```

#### Get Memory

```bash
GET /memories/:point_id
X-API-Key: your-secret-key
```

#### Delete Memory

```bash
DELETE /memories/:point_id
X-API-Key: your-secret-key
```

## Development

### Build

```bash
cd qdrant-service
cargo build --release
```

### Run Tests

```bash
cargo test
```

### Run Locally

```bash
# Start Qdrant first
docker compose up -d qdrant

# Run service
RUST_LOG=debug cargo run
```

### Benchmark

```bash
./qdrant-service/benchmark.sh
```

## Production Deployment

### Docker Compose

```bash
docker compose up -d
```

### Monitoring

- **Metrics**: `http://localhost:8090/metrics` (Prometheus format)
- **Health**: `http://localhost:8090/health`
- **Logs**: `docker compose logs -f qdrant-service`

### Webhook Alerts

Configure `WEBHOOK_URL` to receive alerts for:
- ✅ Service startup
- ❌ Qdrant connection failures
- ⚠️ High error rates (>5%)
- 📊 Daily statistics reports (if `ENABLE_DAILY_STATS=true`)

**Supported platforms:** Discord, Slack, Telegram (webhook-compatible)

**Daily Statistics:**
- Enable with `ENABLE_DAILY_STATS=true`
- Configure interval with `STATS_INTERVAL_HOURS` (default: 24 hours)
- Sends formatted report with:
  - Total vectors upserted
  - Total searches performed
  - Total vectors deleted
  - Service uptime
- Discord-optimized format with rich embeds and number formatting

## Performance

- **Latency**: ~2-5ms for vector search (100k points)
- **Throughput**: ~1000 req/s (single instance)
- **Memory**: ~50MB baseline + 1-2MB per 10k points
- **CPU**: ~1-2% idle, ~50% under load

## Best Practices

1. **Use appropriate vector dimensions** (1024 for BGE-M3)
2. **Set reasonable limits** (max 100 results per search)
3. **Use min_score filtering** (0.3-0.5 for semantic search)
4. **Monitor collection size** (consider archiving old memories)
5. **Enable API key authentication** in production
6. **Configure webhook alerts** for critical errors

## Troubleshooting

### Connection Refused

```bash
# Check if Qdrant is running
docker compose ps qdrant

# Check Qdrant logs
docker compose logs qdrant
```

### High Memory Usage

```bash
# Check collection size
curl -H "X-API-Key: your-key" http://localhost:8090/collection/info

# Consider archiving old data or increasing resources
```

### Slow Searches

- Reduce search limit (15-30 recommended)
- Increase min_score threshold
- Check Qdrant indexing status

## License

Proprietary - Synaplan GmbH

## Support

For issues or questions, contact the Synaplan development team.
