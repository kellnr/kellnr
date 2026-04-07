#!/bin/bash
# Test script for verifying Kellnr toolchain distribution with rustup.
#
# This script runs inside a Docker container with rustup pre-installed.
# It expects the toolchain to already be uploaded to kellnr (done by
# the Playwright test setup). It waits for component extraction, then
# installs the toolchain via rustup and verifies the binaries work.
#
# Environment variables:
#   KELLNR_DIST_URL  - Kellnr toolchain distribution URL
#                      (e.g. http://host.docker.internal:8000/api/v1/toolchains)
#   CHANNEL          - Channel to test (default: stable)
#   VERBOSE          - Set to 1 for verbose output

set -euo pipefail

KELLNR_DIST_URL="${KELLNR_DIST_URL:-}"
CHANNEL="${CHANNEL:-stable}"
VERBOSE="${VERBOSE:-0}"

if [ -z "$KELLNR_DIST_URL" ]; then
    echo "ERROR: KELLNR_DIST_URL is required"
    exit 1
fi

echo "=== Kellnr Toolchain Distribution Test ==="
echo "URL: $KELLNR_DIST_URL"
echo "Channel: $CHANNEL"
echo ""

# --- Test 1: Manifest SHA256 hash ---
echo "--- Test 1: Manifest SHA256 hash ---"
SHA256_URL="${KELLNR_DIST_URL}/dist/channel-rust-${CHANNEL}.toml.sha256"
SHA256=$(curl -sSf "$SHA256_URL") || { echo "ERROR: Failed to fetch $SHA256_URL"; exit 1; }
HASH=$(echo "$SHA256" | tr -d '[:space:]')
echo "$HASH" | grep -qE '^[0-9a-f]{64}$' || { echo "ERROR: Invalid SHA256: $HASH"; exit 1; }
echo "PASS: valid SHA256 hash"
echo ""

# --- Test 2: Manifest structure ---
echo "--- Test 2: Manifest structure ---"
MANIFEST_URL="${KELLNR_DIST_URL}/dist/channel-rust-${CHANNEL}.toml"
MANIFEST=$(curl -sSf "$MANIFEST_URL") || { echo "ERROR: Failed to fetch manifest"; exit 1; }

[ "$VERBOSE" = "1" ] && echo "$MANIFEST"

echo "$MANIFEST" | grep -q 'manifest-version = "2"' || { echo "ERROR: Missing manifest-version"; exit 1; }
echo "$MANIFEST" | grep -q '\[pkg.rust\]'            || { echo "ERROR: Missing [pkg.rust]"; exit 1; }
echo "$MANIFEST" | grep -q 'available = true'         || { echo "ERROR: Missing available target"; exit 1; }
echo "PASS: manifest structure valid"
echo ""

# --- Test 3: Wait for component extraction ---
echo "--- Test 3: Waiting for component packages in manifest ---"
MAX_WAIT=120
WAITED=0
CURL_FAILURES=0
while true; do
    BODY=$(curl -fsS "$MANIFEST_URL" 2>/dev/null) && {
        if echo "$BODY" | grep -q '\[\[pkg\.rust\.target\..*\.components\]\]'; then
            break
        fi
        CURL_FAILURES=0  # reset on successful fetch
    } || {
        CURL_FAILURES=$((CURL_FAILURES + 1))
        if [ "$CURL_FAILURES" -ge 5 ]; then
            echo "ERROR: Manifest endpoint returned errors $CURL_FAILURES times in a row"
            exit 1
        fi
    }
    if [ "$WAITED" -ge "$MAX_WAIT" ]; then
        echo "ERROR: Components not available after ${MAX_WAIT}s (extraction may have failed)"
        echo "Current manifest:"
        curl -sS "$MANIFEST_URL" || true
        exit 1
    fi
    sleep 2
    WAITED=$((WAITED + 2))
done
echo "PASS: component packages present (after ${WAITED}s)"
echo ""

# --- Test 4: Install via rustup ---
echo "--- Test 4: Installing toolchain via rustup ---"
export RUSTUP_DIST_SERVER="$KELLNR_DIST_URL"

# Remove existing stable toolchain so rustup installs from kellnr
rustup toolchain remove "$CHANNEL" 2>/dev/null || true

if [ "$VERBOSE" = "1" ]; then
    RUSTUP_LOG=debug rustup -v toolchain install "$CHANNEL" --no-self-update 2>&1
else
    rustup toolchain install "$CHANNEL" --no-self-update 2>&1
fi
INSTALL_EXIT=$?

if [ "$INSTALL_EXIT" -ne 0 ]; then
    echo "ERROR: rustup install exited with code $INSTALL_EXIT"
    exit 1
fi
echo "PASS: rustup install succeeded"
echo ""

# --- Test 5: Verify installed binaries ---
echo "--- Test 5: Verifying installed binaries ---"

# Use rustup run to explicitly invoke the kellnr-installed toolchain
RUSTC_OUT=$(rustup run "$CHANNEL" rustc --version 2>&1) || { echo "ERROR: rustc --version failed"; exit 1; }
echo "rustc: $RUSTC_OUT"
echo "$RUSTC_OUT" | grep -q "kellnr-test" || { echo "ERROR: Unexpected rustc output: $RUSTC_OUT"; exit 1; }

CARGO_OUT=$(rustup run "$CHANNEL" cargo --version 2>&1) || { echo "ERROR: cargo --version failed"; exit 1; }
echo "cargo: $CARGO_OUT"
echo "$CARGO_OUT" | grep -q "kellnr-test" || { echo "ERROR: Unexpected cargo output: $CARGO_OUT"; exit 1; }

echo ""
echo "=== All tests passed ==="
exit 0
