FROM rustfs/rustfs:latest

# Set build-time arguments for the bucket names
ARG CRATES_BUCKET=kellnr-crates
ARG CRATESIO_BUCKET=kellnr-cratesio
ARG TOOLCHAIN_BUCKET=kellnr-toolchains

# RustFS runs as non-root user (UID 10001), so directories need proper ownership
USER root
RUN mkdir -p /data/${CRATES_BUCKET} /data/${CRATESIO_BUCKET} /data/${TOOLCHAIN_BUCKET} && \
    chown -R 10001:10001 /data
USER 10001

# Set environment variables for RustFS
ENV RUSTFS_VOLUMES=/data

# Start RustFS with command line credentials (env vars may be ignored per rustfs/rustfs#1058)
# Credentials are passed via environment variables at runtime
ENTRYPOINT ["sh", "-c", "/usr/bin/rustfs --access-key ${RUSTFS_ACCESS_KEY:-rustfsadmin} --secret-key ${RUSTFS_SECRET_KEY:-rustfsadmin} /data"]
