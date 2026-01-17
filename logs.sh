#!/bin/bash
# Helper script to monitor Qdrant service logs
# Usage: ./logs.sh [follow|tail|clear]

cd "$(dirname "$0")"

case "${1:-follow}" in
    follow|f)
        echo "📋 Following Qdrant Service logs (Ctrl+C to stop)..."
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        docker compose logs qdrant-service --follow --tail=20
        ;;
    
    tail|t)
        LINES="${2:-50}"
        echo "📋 Last $LINES lines from Qdrant Service:"
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        docker compose logs qdrant-service --tail="$LINES"
        ;;
    
    both|b)
        echo "📋 Last 30 lines from BOTH services:"
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        docker compose logs qdrant qdrant-service --tail=30
        ;;
    
    test|health)
        echo "🔍 Testing health endpoint..."
        echo ""
        curl -s http://localhost:8090/health | jq || curl -s http://localhost:8090/health
        echo ""
        echo ""
        echo "📋 Latest request logs:"
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        sleep 1
        docker compose logs qdrant-service --tail=5
        ;;
    
    clear|c)
        echo "🧹 Restarting service to clear logs..."
        docker compose restart qdrant-service
        sleep 2
        echo "✅ Service restarted"
        ;;
    
    *)
        echo "Qdrant Service Log Helper"
        echo ""
        echo "Usage: ./logs.sh [command]"
        echo ""
        echo "Commands:"
        echo "  follow, f      - Follow logs in real-time (default)"
        echo "  tail [N], t    - Show last N lines (default: 50)"
        echo "  both, b        - Show logs from Qdrant + Service"
        echo "  test, health   - Test health endpoint and show request logs"
        echo "  clear, c       - Restart service (clears logs)"
        echo ""
        echo "Examples:"
        echo "  ./logs.sh              # Follow logs"
        echo "  ./logs.sh tail 100     # Show last 100 lines"
        echo "  ./logs.sh test         # Test health + show logs"
        ;;
esac

