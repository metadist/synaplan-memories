#!/bin/bash
# Helper script to monitor Qdrant logs
# Usage: ./logs.sh [follow|tail]

cd "$(dirname "$0")"

case "${1:-follow}" in
    follow|f)
        echo "Following Qdrant logs (Ctrl+C to stop)..."
        echo "-------------------------------------------"
        docker compose logs qdrant --follow --tail=20
        ;;
    
    tail|t)
        LINES="${2:-50}"
        echo "Last $LINES lines from Qdrant:"
        echo "-------------------------------------------"
        docker compose logs qdrant --tail="$LINES"
        ;;
    
    test|health)
        echo "Testing health endpoint..."
        echo ""
        curl -s http://localhost:6333/healthz || echo "FAILED"
        echo ""
        echo ""
        echo "Cluster status:"
        curl -s http://localhost:6333/cluster | jq '.result.status // "not clustered"' 2>/dev/null || echo "(not available)"
        ;;
    
    *)
        echo "Qdrant Log Helper"
        echo ""
        echo "Usage: ./logs.sh [command]"
        echo ""
        echo "Commands:"
        echo "  follow, f      - Follow logs in real-time (default)"
        echo "  tail [N], t    - Show last N lines (default: 50)"
        echo "  test, health   - Test health endpoint and show cluster status"
        echo ""
        echo "Examples:"
        echo "  ./logs.sh              # Follow logs"
        echo "  ./logs.sh tail 100     # Show last 100 lines"
        echo "  ./logs.sh test         # Test health"
        ;;
esac
