#!/bin/bash
# Create a minimal rustup-compatible toolchain archive for testing
#
# This creates a tar.xz archive that rustup can parse (even if installation
# would fail due to minimal content). It's sufficient for testing the
# distribution mechanism: upload, manifest generation, and download.

set -euo pipefail

# Get script directory first
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Configuration
NAME="${1:-rust}"
VERSION="${2:-1.0.0-test}"
TARGET="${3:-x86_64-unknown-linux-gnu}"

ARCHIVE_NAME="${NAME}-${VERSION}-${TARGET}"
WORK_DIR=$(mktemp -d)
ARCHIVE_DIR="${WORK_DIR}/${ARCHIVE_NAME}"

# Create the directory structure rustup expects
mkdir -p "${ARCHIVE_DIR}"

# Create the rust-installer-version marker (rustup checks for this)
echo "3" > "${ARCHIVE_DIR}/rust-installer-version"

# Create a minimal install.sh (required by rustup)
cat > "${ARCHIVE_DIR}/install.sh" << 'INSTALL_EOF'
#!/bin/bash
# Minimal install script for testing
echo "Test toolchain installation script"
echo "Component: rust"
echo "Version: test"
echo "Target: unknown"
exit 0
INSTALL_EOF
chmod +x "${ARCHIVE_DIR}/install.sh"

# Create components file
echo "${NAME}" > "${ARCHIVE_DIR}/components"

# Create a version file
echo "${VERSION}" > "${ARCHIVE_DIR}/version"

# Create the tar.xz archive in work directory
OUTPUT_FILE="${ARCHIVE_NAME}.tar.xz"
cd "${WORK_DIR}"
tar -cJf "${OUTPUT_FILE}" "${ARCHIVE_NAME}"

# Copy to script directory
cp "${OUTPUT_FILE}" "${SCRIPT_DIR}/"

# Cleanup
rm -rf "${WORK_DIR}"

echo "Created ${SCRIPT_DIR}/${OUTPUT_FILE}"
