FROM minio/minio:latest

# Set build-time arguments for the folders
ARG CRATES_BUCKET=/data/folder1
ARG CRATESIO_BUCKET=/data/folder2

# Create necessary folders
RUN mkdir -p /data/${CRATES_BUCKET} /data/${CRATESIO_BUCKET}

# Start Minio server
CMD ["minio", "server", "/data"]
