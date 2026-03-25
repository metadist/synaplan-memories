# synaplan-memories

Qdrant vector database deployment for [Synaplan](../synaplan). Provides vector storage for:

- **User memories** (AI personality/profiling)
- **False positive tracking**
- **File RAG** (document vector search)

Synaplan's PHP backend talks directly to Qdrant's REST API on port 6333 via `QdrantClientDirect`.

## Quick Start

### Prerequisites

- Docker + Docker Compose

### 1) Start Qdrant

```bash
cd synaplan-memories
docker compose up -d
```

This starts `synaplan-qdrant` on port `6333` (REST) and `6334` (gRPC).

### 2) Connect Synaplan

In your Synaplan backend env (`synaplan/backend/.env`):

```bash
QDRANT_URL=http://qdrant:6333
```

For platform deployments where Qdrant runs on the Docker host:

```bash
QDRANT_URL=http://docker-host:6333
```

Then restart:

```bash
docker compose restart backend
```

### 3) Verify

```bash
curl http://localhost:6333/healthz
```

## Configuration

See `.env.example`. Key settings:

| Variable | Purpose | Default |
|----------|---------|---------|
| `QDRANT_API_KEY` | Optional native Qdrant authentication | (none) |
| `QDRANT_LOG_LEVEL` | Qdrant log verbosity | `INFO` |
| `QDRANT_STORAGE_PATH` | Override storage path (cluster: local SSD) | Docker volume |

## Cluster Deployment

See [CLUSTER.md](CLUSTER.md) for 3-node cluster setup with replication.

## License

Apache-2.0. See [LICENSE](LICENSE).
