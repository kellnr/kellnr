#!/bin/bash
set -e

# Configuration
CONTAINER="kellnr-postgres-test"
IMAGE="kellnr-test:local"
REGISTRY="kellnr-test"
PG_CONTAINER="postgres-for-kellnr"
PG_USER="kellnr"
PG_PASSWORD="kellnr_password"
PG_DB="kellnr"
NETWORK="kellnr-network"

# Check and clean up existing resources first
echo "Checking for existing resources..."

# Check if the containers exist and remove them if they do
if [ "$(docker ps -a -q -f name=$CONTAINER)" ]; then
    echo "Removing existing container: $CONTAINER"
    docker stop $CONTAINER 2>/dev/null || true
    docker rm $CONTAINER 2>/dev/null || true
fi

if [ "$(docker ps -a -q -f name=$PG_CONTAINER)" ]; then
    echo "Removing existing container: $PG_CONTAINER"
    docker stop $PG_CONTAINER 2>/dev/null || true
    docker rm $PG_CONTAINER 2>/dev/null || true
fi

# Check if the network exists and remove it if it does
if docker network inspect $NETWORK >/dev/null 2>&1; then
    echo "Removing existing network: $NETWORK"
    docker network rm $NETWORK || true
fi

# Create Docker network
echo "Creating Docker network..."
docker network create $NETWORK

# Start PostgreSQL container
echo "Starting PostgreSQL container..."
docker run --rm --name $PG_CONTAINER \
    --network $NETWORK \
    -e POSTGRES_USER=$PG_USER \
    -e POSTGRES_PASSWORD=$PG_PASSWORD \
    -e POSTGRES_DB=$PG_DB \
    -d postgres:14

# Wait for PostgreSQL to start
echo "Waiting for PostgreSQL to start..."
sleep 5

# Build Kellnr image
echo "Building Kellnr image..."
docker build -t "$IMAGE" -f ../Dockerfile --build-arg KELLNR_VERSION="local" ../../

# Start Kellnr container with PostgreSQL config
echo "Starting Kellnr container..."
docker run --rm --name "$CONTAINER" \
    --network $NETWORK \
    -p 8000:8000 \
    -e KELLNR_LOG__LEVEL=debug \
    -e KELLNR_LOG__LEVEL_WEB_SERVER=debug \
    -e KELLNR_PROXY__ENABLED=true \
    -e KELLNR_POSTGRESQL__ENABLED=true \
    -e KELLNR_POSTGRESQL__ADDRESS=$PG_CONTAINER \
    -e KELLNR_POSTGRESQL__PORT=5432 \
    -e KELLNR_POSTGRESQL__DB=$PG_DB \
    -e KELLNR_POSTGRESQL__USER=$PG_USER \
    -e KELLNR_POSTGRESQL__PWD=$PG_PASSWORD \
    -d "$IMAGE"

# Wait for Kellnr to start
echo "Waiting for Kellnr to start..."
sleep 15

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

# Stop containers
echo "Stopping containers..."
docker stop "$CONTAINER"
docker stop "$PG_CONTAINER"

# Clean up resources
echo "Cleaning up resources..."
# First disconnect containers to ensure network can be removed
docker network disconnect $NETWORK $CONTAINER 2>/dev/null || true
docker network disconnect $NETWORK $PG_CONTAINER 2>/dev/null || true
# The containers should be removed automatically due to the --rm flag
# Remove network
docker network rm $NETWORK || true
echo "Cleanup completed"

echo "Done"