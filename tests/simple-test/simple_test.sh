#!/bin/bash
set -e

# Configuration
CONTAINER="kellnr-simple-test"
IMAGE="kellnr-test:local"
REGISTRY="kellnr-test"

# Check and clean up existing resources first
echo "Checking for existing resources..."

# Check if the container exists and remove it if it does
if [ "$(docker ps -a -q -f name=$CONTAINER)" ]; then
    echo "Removing existing container: $CONTAINER"
    docker stop $CONTAINER 2>/dev/null || true
    docker rm $CONTAINER 2>/dev/null || true
fi

# Build image
echo "Building image..."
docker build -t "$IMAGE" -f ../Dockerfile --build-arg KELLNR_VERSION="local" ../../

# Start container
echo "Starting container..."
docker run --rm --name "$CONTAINER" \
    -p 8000:8000 \
    -e KELLNR_LOG__LEVEL=debug \
    -e KELLNR_LOG__LEVEL_WEB_SERVER=debug \
    -e KELLNR_PROXY__ENABLED=true \
    -d "$IMAGE"

# Wait for server to start
echo "Waiting for server to start..."
sleep 10

# Publish crates
echo "Publishing crates..."
CRATES=(
    "../test-sparse-registry/crates/test_lib"
    "../test-sparse-registry/crates/UpperCase-Name123"
    "../test-sparse-registry/crates/foo-bar"
)

for CRATE in "${CRATES[@]}"; do
    echo "Publishing $CRATE"
    rm -f "$CRATE/Cargo.lock"
    (cd "$CRATE" && cargo publish --registry "$REGISTRY" --allow-dirty)
done

# Stop container
echo "Stopping container..."
docker stop "$CONTAINER"

echo "Cleanup completed"
echo "Done"