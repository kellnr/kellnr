#!/bin/bash
# Test script for verifying Kellnr toolchain distribution with rustup
#
# Environment variables:
#   KELLNR_DIST_URL - The Kellnr toolchain distribution URL (e.g., http://host.docker.internal:8000/api/v1/toolchains)
#   CHANNEL - The channel to test (default: stable)
#   VERBOSE - Set to 1 for verbose output

set -euo pipefail

KELLNR_DIST_URL="${KELLNR_DIST_URL:-}"
CHANNEL="${CHANNEL:-stable}"
VERBOSE="${VERBOSE:-0}"

if [ -z "$KELLNR_DIST_URL" ]; then
    echo "ERROR: KELLNR_DIST_URL environment variable is required"
    exit 1
fi

echo "=== Kellnr Toolchain Distribution Test ==="
echo "Distribution URL: $KELLNR_DIST_URL"
echo "Channel: $CHANNEL"
echo ""

# Test 1: Fetch and verify the manifest
echo "--- Test 1: Fetching channel manifest ---"
MANIFEST_URL="${KELLNR_DIST_URL}/dist/channel-rust-${CHANNEL}.toml"
echo "Fetching: $MANIFEST_URL"

MANIFEST=$(curl -sSf "$MANIFEST_URL" 2>&1) || {
    echo "ERROR: Failed to fetch manifest"
    echo "Response: $MANIFEST"
    exit 1
}

echo "Manifest fetched successfully!"
if [ "$VERBOSE" = "1" ]; then
    echo "Content:"
    echo "$MANIFEST"
fi

# Verify manifest contains expected fields
if ! echo "$MANIFEST" | grep -q 'manifest-version = "2"'; then
    echo "ERROR: Manifest missing 'manifest-version = \"2\"'"
    exit 1
fi

if ! echo "$MANIFEST" | grep -q '\[pkg.rust\]'; then
    echo "ERROR: Manifest missing '[pkg.rust]' section"
    exit 1
fi

if ! echo "$MANIFEST" | grep -q 'available = true'; then
    echo "ERROR: Manifest missing target with 'available = true'"
    exit 1
fi

echo "Manifest validation passed!"
echo ""

# Test 2: Verify manifest SHA256 hash endpoint
# rustup fetches the .sha256 of the manifest before the manifest itself.
# If this returns 404, rustup gives up with "no release found".
echo "--- Test 2: Fetching manifest SHA256 hash ---"
SHA256_URL="${KELLNR_DIST_URL}/dist/channel-rust-${CHANNEL}.toml.sha256"
echo "Fetching: $SHA256_URL"

SHA256_RESPONSE=$(curl -sSf "$SHA256_URL" 2>&1) || {
    echo "ERROR: Failed to fetch manifest SHA256 hash (this is what rustup requests first)"
    echo "Response: $SHA256_RESPONSE"
    exit 1
}

echo "SHA256 hash fetched successfully: $SHA256_RESPONSE"

# Verify the hash is a valid 64-character hex string
HASH=$(echo "$SHA256_RESPONSE" | tr -d '[:space:]')
if ! echo "$HASH" | grep -qE '^[0-9a-f]{64}$'; then
    echo "ERROR: SHA256 hash is not a valid 64-character hex string: '$HASH'"
    exit 1
fi

echo "SHA256 hash validation passed!"
echo ""

# Test 3: Test rustup with custom dist server
echo "--- Test 3: Testing rustup with custom dist server ---"
export RUSTUP_DIST_SERVER="$KELLNR_DIST_URL"
export RUSTUP_UPDATE_ROOT="$KELLNR_DIST_URL"

echo "RUSTUP_DIST_SERVER=$RUSTUP_DIST_SERVER"

# Try to install the toolchain via rustup.
# rustup must be able to:
#   1. Fetch the manifest SHA256 hash
#   2. Fetch and parse the manifest
#   3. Download the archive
# The actual installation may fail because our test archive is minimal,
# but rustup must not fail with "no release found" — that indicates
# it couldn't fetch the manifest hash or parse the manifest.
echo ""
echo "Running: rustup install $CHANNEL --no-self-update"

OUTPUT=$(rustup install "$CHANNEL" --no-self-update 2>&1) || true
echo "$OUTPUT"

if echo "$OUTPUT" | grep -q "no release found"; then
    echo ""
    echo "ERROR: rustup reported 'no release found' — manifest hash or manifest endpoint is broken"
    exit 1
fi

if echo "$OUTPUT" | grep -q "could not download nonexistent rust version"; then
    echo ""
    echo "ERROR: rustup could not find the rust version — distribution endpoint is broken"
    exit 1
fi

echo ""
echo "=== Test Complete ==="
echo "The Kellnr toolchain distribution mechanism is working correctly!"
exit 0
