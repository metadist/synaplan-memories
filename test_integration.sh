#!/bin/bash
# Integration Tests für Qdrant Service
# Startet Qdrant + Service und führt API Tests aus

set -e

echo "================================================"
echo "Qdrant Service Integration Tests"
echo "================================================"
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Prüfe ob Services laufen
echo -e "${YELLOW}1. Prüfe Services...${NC}"
cd "$(dirname "$0")"

if ! docker compose ps | grep -q "synaplan-qdrant"; then
    echo -e "${RED}❌ Qdrant läuft nicht! Starte mit: docker compose up -d${NC}"
    exit 1
fi

if ! docker compose ps | grep -q "synaplan-qdrant-service"; then
    echo -e "${RED}❌ Qdrant Service läuft nicht! Starte mit: docker compose up -d${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Services laufen${NC}"
echo ""

# Warte auf Health Check
echo -e "${YELLOW}2. Warte auf Service...${NC}"
for i in {1..30}; do
    if curl -s -f http://localhost:8090/health > /dev/null 2>&1; then
        echo -e "${GREEN}✅ Service ist bereit!${NC}"
        break
    fi
    if [ $i -eq 30 ]; then
        echo -e "${RED}❌ Service antwortet nicht nach 30 Sekunden${NC}"
        exit 1
    fi
    sleep 1
done
echo ""

# Führe API Tests aus
echo -e "${YELLOW}3. Führe API Tests aus...${NC}"
export QDRANT_API_KEY="${SERVICE_API_KEY:-changeme-in-production}"
./qdrant-service/test_api.sh

echo ""
echo -e "${GREEN}================================================${NC}"
echo -e "${GREEN}✅ Alle Integration Tests bestanden!${NC}"
echo -e "${GREEN}================================================${NC}"

