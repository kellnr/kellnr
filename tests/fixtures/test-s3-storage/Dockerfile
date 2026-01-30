FROM minio/minio:latest

# Set build-time arguments for the folders
ARG CRATES_BUCKET=/data/folder1
ARG CRATESIO_BUCKET=/data/folder2
ARG TOOLCHAIN_BUCKET=/data/folder3

# Create necessary folders
RUN mkdir -p /data/${CRATES_BUCKET} /data/${CRATESIO_BUCKET} /data/${TOOLCHAIN_BUCKET}

# Start Minio server
CMD ["minio", "server", "/data"]
