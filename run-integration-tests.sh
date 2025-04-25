#!/usr/bin/env bash
# Fail fast on any error, unset var, or failed pipe; show commands for easier debugging
set -euo pipefail

# Ensure DynamoDB container is cleaned up even when the script aborts
cleanup() {
  echo "Shutting down DynamoDB Local..."
  docker-compose -f docker-compose.test.yml down
}
trap cleanup EXIT

echo "Starting DynamoDB Local..."
docker-compose -f docker-compose.test.yml up -d

# Wait for DynamoDB to be ready with timeout
echo "Waiting for DynamoDB Local to be ready..."
for _ in {1..60}; do            # max 60 s
  if curl -s http://localhost:8000 >/dev/null; then
    break
  fi
  sleep 1
done || { echo "DynamoDB did not start in time"; exit 1; }
echo "DynamoDB Local is ready!"

# Run the tests with the USE_DYNAMODB environment variable set to true
echo "Running integration tests with real DynamoDB store..."

# Run invitation service tests
echo "=== Running invitation service tests ==="
pushd invitation-service
USE_DYNAMODB=true cargo test -- --test-threads=1 --nocapture
popd

# Run box service tests
echo "=== Running box service tests ==="
pushd box-service
USE_DYNAMODB=true cargo test -- --test-threads=1 --nocapture
popd

# Run guardian service tests if available
if [ -d "guardian-service" ]; then
  echo "=== Running guardian service tests ==="
  pushd guardian-service
  USE_DYNAMODB=true cargo test -- --test-threads=1 --nocapture
  popd
fi

# Run invitation event service tests
echo "=== Running invitation event service tests ==="
pushd invitation-event-service
USE_DYNAMODB=true cargo test -- --test-threads=1 --nocapture
popd

echo "All integration tests completed successfully!" 