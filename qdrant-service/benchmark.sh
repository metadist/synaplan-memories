#!/bin/bash
# Performance Benchmark for Qdrant Service
# Tests throughput and latency

set -e

BASE_URL="http://localhost:8090"
USER_ID=1
NUM_REQUESTS=100
CONCURRENT=10

echo "================================"
echo "Qdrant Service Performance Test"
echo "================================"
echo "Requests: $NUM_REQUESTS"
echo "Concurrency: $CONCURRENT"
echo ""

# Generate test vector
VECTOR=$(python3 -c "import json; import random; print(json.dumps([round(random.uniform(-1, 1), 4) for _ in range(1024)]))")

# 1. Benchmark Upsert
echo "Benchmarking UPSERT operations..."
time {
    for i in $(seq 1 $NUM_REQUESTS); do
        POINT_ID="perf_test_${USER_ID}_${i}"
        curl -s -X POST "$BASE_URL/memories" \
            -H "Content-Type: application/json" \
            -d "{
                \"point_id\": \"$POINT_ID\",
                \"vector\": $VECTOR,
                \"payload\": {
                    \"user_id\": $USER_ID,
                    \"category\": \"performance_test\",
                    \"key\": \"test_$i\",
                    \"value\": \"Performance test memory $i\",
                    \"source\": \"benchmark\",
                    \"created\": $(date +%s),
                    \"updated\": $(date +%s),
                    \"active\": true
                }
            }" > /dev/null
    done
}

echo ""
echo "Benchmarking SEARCH operations..."
time {
    for i in $(seq 1 $NUM_REQUESTS); do
        curl -s -X POST "$BASE_URL/memories/search" \
            -H "Content-Type: application/json" \
            -d "{
                \"query_vector\": $VECTOR,
                \"user_id\": $USER_ID,
                \"limit\": 5,
                \"min_score\": 0.7
            }" > /dev/null
    done
}

echo ""
echo "================================"
echo "Performance test completed!"
echo "================================"
echo ""
echo "For more detailed benchmarks, use tools like:"
echo "- wrk: wrk -t4 -c100 -d30s --latency $BASE_URL/health"
echo "- ab: ab -n 1000 -c 10 $BASE_URL/health"
echo "- hey: hey -n 1000 -c 10 $BASE_URL/health"

