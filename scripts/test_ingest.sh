#!/bin/bash
# Test script for CRISPR ingest endpoint
# Usage: ./scripts/test_ingest.sh

BASE_URL="${CRISPR_URL:-http://localhost:8080}"

echo "Testing CRISPR at $BASE_URL"
echo "================================"

# Test 1: Health check
echo -e "\n1. Health check..."
curl -s "$BASE_URL/health"
echo ""

# Test 2: Readiness check
echo -e "\n2. Readiness check..."
curl -s "$BASE_URL/ready"
echo ""

# Test 3: API status
echo -e "\n3. API status..."
curl -s "$BASE_URL/api/v1/status" | jq .

# Test 4: Ingest a sample error (NullPointerException)
echo -e "\n4. Ingesting NullPointerException error..."
curl -s -X POST "$BASE_URL/ingest" \
  -H "Content-Type: application/json" \
  -d '{
    "project": "test/sample-app",
    "level": "ERROR",
    "message": "NullPointerException: Cannot invoke method on null object",
    "stack_trace": "java.lang.NullPointerException: Cannot invoke method on null object\n    at com.example.UserService.getUser(UserService.java:42)\n    at com.example.Controller.handleRequest(Controller.java:18)\n    at org.springframework.web.servlet.FrameworkServlet.service(FrameworkServlet.java:897)",
    "timestamp": "2024-01-15T10:30:00Z",
    "attributes": {
      "user_id": "12345",
      "request_id": "abc-123-def"
    }
  }' | jq .

# Test 5: Ingest same error again (should increment count)
echo -e "\n5. Ingesting same error again (should show occurrence #2)..."
curl -s -X POST "$BASE_URL/ingest" \
  -H "Content-Type: application/json" \
  -d '{
    "project": "test/sample-app",
    "level": "ERROR",
    "message": "NullPointerException: Cannot invoke method on null object",
    "stack_trace": "java.lang.NullPointerException: Cannot invoke method on null object\n    at com.example.UserService.getUser(UserService.java:42)\n    at com.example.Controller.handleRequest(Controller.java:18)\n    at org.springframework.web.servlet.FrameworkServlet.service(FrameworkServlet.java:897)"
  }' | jq .

# Test 6: Ingest a different error
echo -e "\n6. Ingesting ArrayIndexOutOfBoundsException (new incident)..."
curl -s -X POST "$BASE_URL/ingest" \
  -H "Content-Type: application/json" \
  -d '{
    "project": "test/sample-app",
    "level": "FATAL",
    "message": "ArrayIndexOutOfBoundsException: Index 10 out of bounds for length 5",
    "stack_trace": "java.lang.ArrayIndexOutOfBoundsException: Index 10 out of bounds for length 5\n    at com.example.DataProcessor.process(DataProcessor.java:88)\n    at com.example.BatchJob.run(BatchJob.java:25)"
  }' | jq .

# Test 7: Ingest a non-error (should be rejected)
echo -e "\n7. Ingesting INFO level (should be rejected)..."
curl -s -X POST "$BASE_URL/ingest" \
  -H "Content-Type: application/json" \
  -d '{
    "project": "test/sample-app",
    "level": "INFO",
    "message": "Application started successfully"
  }' | jq .

# Test 8: Ingest without project (should fail)
echo -e "\n8. Ingesting without project (should fail)..."
curl -s -X POST "$BASE_URL/ingest" \
  -H "Content-Type: application/json" \
  -d '{
    "level": "ERROR",
    "message": "Unknown error"
  }' | jq .

# Test 9: Ingest with service_name (should infer project)
echo -e "\n9. Ingesting with service_name inference..."
curl -s -X POST "$BASE_URL/ingest" \
  -H "Content-Type: application/json" \
  -d '{
    "service_name": "acme/payment-service",
    "level": "ERROR",
    "message": "Payment processing failed: Invalid card number"
  }' | jq .

echo -e "\n================================"
echo "Tests complete!"
