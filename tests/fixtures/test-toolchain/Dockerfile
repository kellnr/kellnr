# Dockerfile for testing rustup integration with Kellnr toolchain server
#
# NOTE: This Dockerfile is provided for manual testing/debugging only.
# The automated tests use the official rust:slim-trixie image directly
# with bind-mounted scripts instead of building this custom image.
#
# Usage (manual testing):
#   docker build -t kellnr-rustup-test .
#   docker run -e KELLNR_DIST_URL=http://host.docker.internal:8000/api/v1/toolchain \
#              -e CHANNEL=stable \
#              kellnr-rustup-test

FROM debian:bookworm-slim

# Install dependencies
RUN apt-get update && apt-get install -y \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Install rustup (minimal installation without default toolchain)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain none

# Add cargo/rustup to PATH
ENV PATH="/root/.cargo/bin:${PATH}"

# Create a script to test toolchain installation
COPY test-rustup.sh /test-rustup.sh
RUN chmod +x /test-rustup.sh

ENTRYPOINT ["/test-rustup.sh"]
