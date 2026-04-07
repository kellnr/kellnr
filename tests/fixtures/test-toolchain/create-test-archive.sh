#!/bin/bash
# Create minimal rustup-compatible toolchain archives for testing.
#
# Produces archives for both x86_64 and aarch64 targets so the test
# works on both ARM Macs and x86 CI runners.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

NAME="${1:-rust}"
VERSION="${2:-1.0.0-test}"

for TARGET in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
  ARCHIVE_NAME="${NAME}-${VERSION}-${TARGET}"
  WORK_DIR=$(mktemp -d)
  ARCHIVE_DIR="${WORK_DIR}/${ARCHIVE_NAME}"

  mkdir -p "${ARCHIVE_DIR}"

  # Root metadata
  echo "3" > "${ARCHIVE_DIR}/rust-installer-version"
  echo "${VERSION}" > "${ARCHIVE_DIR}/version"
  printf "rustc\ncargo\n" > "${ARCHIVE_DIR}/components"

  cat > "${ARCHIVE_DIR}/install.sh" << 'EOF'
#!/bin/bash
echo "Test toolchain install"
exit 0
EOF
  chmod +x "${ARCHIVE_DIR}/install.sh"

  # --- Component: rustc ---
  RUSTC_DIR="${ARCHIVE_DIR}/rustc"
  mkdir -p "${RUSTC_DIR}/bin"

  cat > "${RUSTC_DIR}/bin/rustc" << BINEOF
#!/bin/bash
echo "rustc ${VERSION} (kellnr-test)"
BINEOF
  chmod +x "${RUSTC_DIR}/bin/rustc"

  cat > "${RUSTC_DIR}/manifest.in" << 'EOF'
file:bin/rustc
EOF

  # --- Component: cargo ---
  CARGO_DIR="${ARCHIVE_DIR}/cargo"
  mkdir -p "${CARGO_DIR}/bin"

  cat > "${CARGO_DIR}/bin/cargo" << BINEOF
#!/bin/bash
echo "cargo ${VERSION} (kellnr-test)"
BINEOF
  chmod +x "${CARGO_DIR}/bin/cargo"

  cat > "${CARGO_DIR}/manifest.in" << 'EOF'
file:bin/cargo
EOF

  # Create archive
  cd "${WORK_DIR}"
  OUTPUT_FILE="${ARCHIVE_NAME}.tar.xz"
  tar -cJf "${OUTPUT_FILE}" "${ARCHIVE_NAME}"
  cp "${OUTPUT_FILE}" "${SCRIPT_DIR}/"
  rm -rf "${WORK_DIR}"

  echo "Created ${SCRIPT_DIR}/${OUTPUT_FILE}"
done
