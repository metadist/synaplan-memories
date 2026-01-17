# Testing Guide

## Test Levels

### 1. Rust Unit Tests
Tests config, hashing, models. No Qdrant needed.

```bash
cd qdrant-service
cargo test
```

### 2. PHP Unit Tests
Tests `QdrantClientHttp` with MockHttpClient. Runs in CI.

```bash
cd ../synaplan
docker compose exec backend php bin/phpunit tests/Service/VectorSearch/
```

**Result:**
```
OK (10 tests, 20 assertions)
```

### 3. Integration Tests
End-to-end API tests with real Qdrant.

```bash
cd synaplan-memories
docker compose up -d
./test_integration.sh
```

## Vector Dimension

```
QDRANT_VECTOR_DIMENSION=1024

"Hello world" → Embedding Model → [0.23, -0.45, ..., 0.12]
                                   ↑______________________↑
                                        1024 numbers
```

**Common models:**
- BGE-M3: 1024 (recommended)
- OpenAI text-embedding-3-small: 1536
- Sentence-Transformers: 384/768

## Checklist Before Commit

- [ ] Rust: `cargo test && cargo fmt && cargo clippy`
- [ ] PHP: `docker compose exec backend php bin/phpunit tests/Service/VectorSearch/`
- [ ] Integration: `./test_integration.sh`
