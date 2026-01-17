#!/bin/bash
# Test script for Qdrant Service
# Tests all API endpoints with example requests

set -e

BASE_URL="${BASE_URL:-http://localhost:8090}"
USER_ID=1
POINT_ID="mem_${USER_ID}_$(date +%s)"

echo "================================"
echo "Qdrant Service API Tests"
echo "================================"
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Helper function
test_endpoint() {
    local name=$1
    local method=$2
    local endpoint=$3
    local data=$4
    
    echo -e "${YELLOW}Testing: $name${NC}"
    
    if [ "$method" = "GET" ]; then
        response=$(curl -s -w "\n%{http_code}" "$BASE_URL$endpoint")
    elif [ "$method" = "DELETE" ]; then
        response=$(curl -s -w "\n%{http_code}" -X DELETE "$BASE_URL$endpoint")
    else
        response=$(curl -s -w "\n%{http_code}" -X POST \
            -H "Content-Type: application/json" \
            -d "$data" \
            "$BASE_URL$endpoint")
    fi
    
    http_code=$(echo "$response" | tail -n1)
    body=$(echo "$response" | head -n-1)
    
    if [ "$http_code" -ge 200 ] && [ "$http_code" -lt 300 ]; then
        echo -e "${GREEN}✓ Success (HTTP $http_code)${NC}"
        echo "$body" | jq . 2>/dev/null || echo "$body"
    else
        echo -e "${RED}✗ Failed (HTTP $http_code)${NC}"
        echo "$body"
    fi
    echo ""
}

# 1. Health Check
test_endpoint "Health Check" "GET" "/health"

# 2. Collection Info
test_endpoint "Collection Info" "GET" "/collection/info"

# 3. Generate dummy vector (1024 dimensions)
VECTOR=$(python3 -c "import json; import random; print(json.dumps([round(random.uniform(-1, 1), 4) for _ in range(1024)]))")

# 4. Upsert Memory
UPSERT_DATA=$(cat <<EOF
{
  "point_id": "$POINT_ID",
  "vector": $VECTOR,
  "payload": {
    "user_id": $USER_ID,
    "category": "test",
    "key": "test_memory",
    "value": "This is a test memory created by test script",
    "source": "test_script",
    "message_id": 9999,
    "created": $(date +%s),
    "updated": $(date +%s),
    "active": true
  }
}
EOF
)

test_endpoint "Upsert Memory" "POST" "/memories" "$UPSERT_DATA"

# 5. Get Memory
test_endpoint "Get Memory" "GET" "/memories/$POINT_ID"

# 6. Search Memories
SEARCH_DATA=$(cat <<EOF
{
  "query_vector": $VECTOR,
  "user_id": $USER_ID,
  "limit": 5,
  "min_score": 0.5
}
EOF
)

test_endpoint "Search Memories" "POST" "/memories/search" "$SEARCH_DATA"

# 7. Search with Category Filter
SEARCH_CATEGORY_DATA=$(cat <<EOF
{
  "query_vector": $VECTOR,
  "user_id": $USER_ID,
  "category": "test",
  "limit": 3,
  "min_score": 0.7
}
EOF
)

test_endpoint "Search with Category" "POST" "/memories/search" "$SEARCH_CATEGORY_DATA"

# 8. Delete Memory
test_endpoint "Delete Memory" "DELETE" "/memories/$POINT_ID"

# 9. Verify Deletion
echo -e "${YELLOW}Verifying deletion...${NC}"
response=$(curl -s -w "\n%{http_code}" "$BASE_URL/memories/$POINT_ID")
http_code=$(echo "$response" | tail -n1)

if [ "$http_code" = "404" ]; then
    echo -e "${GREEN}✓ Memory successfully deleted${NC}"
else
    echo -e "${RED}✗ Memory still exists${NC}"
fi

echo ""
echo "================================"
echo "All tests completed!"
echo "================================"

