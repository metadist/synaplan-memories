# Synaplan Qdrant Service

Rust-basierter Microservice für die Qdrant Vector Database Integration im Synaplan Memory System.

## Features

- ✅ **Vector Storage**: Speichern und Verwalten von Memory-Embeddings (1024 Dimensionen)
- ✅ **Semantic Search**: Ähnlichkeitssuche mit Cosine Distance
- ✅ **Multi-Tenant**: User-basierte Filterung und Isolation
- ✅ **Category Filtering**: Optionale Kategorisierung von Memories
- ✅ **REST API**: Einfache HTTP-Schnittstelle
- ✅ **Health Checks**: Monitoring und Status-Endpoints
- ✅ **CORS**: Cross-Origin Resource Sharing aktiviert

## Quick Start

### 1. Environment Setup

```bash
cp .env.example .env
# Anpassen falls nötig
```

### 2. Services starten

```bash
cd /home/ys/synaplan/synaplan-memories
docker compose up -d
```

### 3. Health Check

```bash
curl http://localhost:8090/health
```

## API Endpoints

### Health Check
```http
GET /health
```

**Response:**
```json
{
  "status": "healthy",
  "service": "synaplan-qdrant-service",
  "qdrant": "connected"
}
```

### Collection Info
```http
GET /collection/info
```

**Response:**
```json
{
  "status": "Green",
  "points_count": 42,
  "vectors_count": 42,
  "indexed_vectors_count": 42
}
```

### Upsert Memory
```http
POST /memories
Content-Type: application/json

{
  "point_id": "mem_1_1737115234567",
  "vector": [0.1, 0.2, ..., 0.9],  // 1024 dimensions
  "payload": {
    "user_id": 1,
    "category": "personal",
    "key": "food_preferences",
    "value": "Loves kebab",
    "source": "auto_detected",
    "message_id": 2565,
    "created": 1737115234,
    "updated": 1737115234,
    "active": true
  }
}
```

**Response:**
```json
{
  "success": true,
  "point_id": "mem_1_1737115234567",
  "message": "Memory upserted successfully"
}
```

### Get Memory
```http
GET /memories/{point_id}
```

**Response:**
```json
{
  "id": "mem_1_1737115234567",
  "payload": {
    "user_id": 1,
    "category": "personal",
    "key": "food_preferences",
    "value": "Loves kebab",
    "source": "auto_detected",
    "message_id": 2565,
    "created": 1737115234,
    "updated": 1737115234,
    "active": true
  }
}
```

### Search Memories
```http
POST /memories/search
Content-Type: application/json

{
  "query_vector": [0.1, 0.2, ..., 0.9],  // 1024 dimensions
  "user_id": 1,
  "category": "personal",  // optional
  "limit": 5,              // optional, default: 5
  "min_score": 0.7         // optional, default: 0.7
}
```

**Response:**
```json
{
  "results": [
    {
      "id": "mem_1_1737115234567",
      "score": 0.95,
      "payload": {
        "user_id": 1,
        "category": "personal",
        "key": "food_preferences",
        "value": "Loves kebab",
        "source": "auto_detected",
        "message_id": 2565,
        "created": 1737115234,
        "updated": 1737115234,
        "active": true
      }
    }
  ],
  "count": 1
}
```

### Delete Memory
```http
DELETE /memories/{point_id}
```

**Response:**
```json
{
  "success": true,
  "point_id": "mem_1_1737115234567",
  "message": "Memory deleted successfully"
}
```

## Architecture

```
┌─────────────────┐
│  Synaplan       │
│  Backend (PHP)  │
└────────┬────────┘
         │
         │ HTTP REST
         │
┌────────▼────────────┐
│  Qdrant Service     │
│  (Rust/Axum)        │
└────────┬────────────┘
         │
         │ gRPC (6334)
         │
┌────────▼────────────┐
│  Qdrant Database    │
│  (Vector Store)     │
└─────────────────────┘
```

## Configuration

Environment Variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `QDRANT_URL` | `http://qdrant:6334` | Qdrant gRPC endpoint |
| `QDRANT_API_KEY` | - | Optional API key |
| `QDRANT_COLLECTION_NAME` | `user_memories` | Collection name |
| `QDRANT_VECTOR_DIMENSION` | `1024` | Vector dimension (BGE-M3) |
| `PORT` | `8090` | Service port |
| `RUST_LOG` | `info` | Log level |

## Development

### Build locally

```bash
cd qdrant-service
cargo build --release
```

### Run tests

```bash
cargo test
```

### Format code

```bash
cargo fmt
```

### Lint

```bash
cargo clippy
```

## Integration mit Synaplan Backend

Das Backend muss die `QdrantClientInterface` Implementation aktualisieren:

```yaml
# backend/config/services/vector_search.yaml
App\Service\VectorSearch\QdrantClientInterface:
    class: App\Service\VectorSearch\QdrantClientHttp
    arguments:
        $baseUrl: '%env(QDRANT_SERVICE_URL)%'
```

In `backend/.env`:
```bash
QDRANT_SERVICE_URL=http://qdrant-service:8090
```

## Monitoring

### Logs anschauen

```bash
docker compose logs -f qdrant-service
```

### Collection Status

```bash
curl http://localhost:8090/collection/info | jq
```

## Performance

- **Latency**: < 50ms für Search (typisch)
- **Throughput**: > 1000 req/s (abhängig von Hardware)
- **Vector Dimension**: 1024 (BGE-M3 Model)
- **Distance Metric**: Cosine Similarity

## Troubleshooting

### Service startet nicht

```bash
# Logs prüfen
docker compose logs qdrant-service

# Qdrant Health Check
curl http://localhost:6333/healthz
```

### Connection refused

```bash
# Qdrant neu starten
docker compose restart qdrant

# Warten bis healthy
docker compose ps
```

### Vector dimension mismatch

Stelle sicher, dass das Embedding-Model 1024 Dimensionen produziert (BGE-M3).

## Tech Stack

- **Rust**: 1.83 (stable)
- **Axum**: 0.7 (Web Framework)
- **Qdrant Client**: 1.14.0
- **Tokio**: 1.41 (Async Runtime)
- **Serde**: 1.0 (Serialization)

## License

See main Synaplan project license.

