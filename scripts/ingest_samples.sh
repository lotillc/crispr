#!/bin/bash
# Ingest sample errors from sample_errors.json
# Usage: ./scripts/ingest_samples.sh

BASE_URL="${CRISPR_URL:-http://localhost:8080}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Ingesting sample errors to $BASE_URL"
echo "================================"

# Read and ingest each error from the JSON file
jq -c '.[]' "$SCRIPT_DIR/sample_errors.json" | while read -r error; do
    echo ""
    echo "Ingesting: $(echo "$error" | jq -r '.message' | head -c 60)..."

    response=$(curl -s -X POST "$BASE_URL/ingest" \
        -H "Content-Type: application/json" \
        -d "$error")

    echo "$response" | jq -r '"  -> \(.message) (incident: \(.incident_id))"'
done

echo ""
echo "================================"
echo "Done! Check the database for created incidents."
