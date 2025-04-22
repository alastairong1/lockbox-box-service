#!/bin/bash
set -e

echo "Starting DynamoDB Local..."
docker-compose -f docker-compose.test.yml up -d

# Wait for DynamoDB to be ready
echo "Waiting for DynamoDB Local to be ready..."
while ! curl -s http://localhost:8000 > /dev/null; do
  sleep 1
done
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

# Clean up
echo "Shutting down DynamoDB Local..."
docker-compose -f docker-compose.test.yml down

echo "All integration tests completed successfully!" 