# Qdrant Memories Integration

Rust microservice providing vector search for Synaplan user memories using Qdrant.

## Quick Start

```bash
# Start services
docker compose up -d

# Check health
curl http://localhost:8090/health

# View logs
./logs.sh              # Follow logs in real-time
./logs.sh tail 100     # Show last 100 lines
./logs.sh test         # Test health + show request logs

# Run tests
./test_integration.sh
```

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
- **Rust Service** (port 8090): REST API gateway to Qdrant
- **PHP Backend**: Uses `QdrantClientHttp` to call Rust service

## Configuration

Copy `.env.example` in `qdrant-service/` and set:
- `QDRANT_URL`: Qdrant gRPC URL (default: `http://qdrant:6334`)
- `QDRANT_VECTOR_DIMENSION`: Embedding size (default: `1024` for BGE-M3)
- `SERVICE_API_KEY`: Auth key for service access (required in production)
- `RUST_LOG`: Log level (default: `synaplan_qdrant_service=debug,tower_http=info`)

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
