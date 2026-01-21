# syntax=docker/dockerfile:1.7

# ==============================================================================
# Subcog Container Image
# ==============================================================================
# Multi-arch (amd64/arm64) minimal container using scratch base.
# Built from pre-compiled musl binaries for fully static linking.
# Includes CA certificates for HTTPS support (~200KB overhead).
#
# Build args:
#   VERSION     - Semver version (e.g., 0.6.1)
#   TARGETARCH  - Target architecture (set automatically by buildx)
#
# Usage:
#   docker run --rm ghcr.io/zircote/subcog:latest --help
#   docker run --rm -v subcog-data:/data ghcr.io/zircote/subcog:latest serve
# ==============================================================================

# ------------------------------------------------------------------------------
# Stage 1: Download and verify binary
# ------------------------------------------------------------------------------
FROM --platform=$BUILDPLATFORM alpine:3.21 AS downloader

ARG VERSION
ARG TARGETARCH

# Validate VERSION is provided
RUN if [ -z "${VERSION}" ]; then \
      echo "ERROR: VERSION build arg is required (e.g., --build-arg VERSION=0.14.0)" && exit 1; \
    fi

# Install download and verification tools
RUN apk add --no-cache curl ca-certificates

# Map Docker arch to Rust target triple
# amd64 -> x86_64-unknown-linux-musl
# arm64 -> aarch64-unknown-linux-musl
RUN case "${TARGETARCH}" in \
      amd64) echo "x86_64-unknown-linux-musl" > /tmp/target ;; \
      arm64) echo "aarch64-unknown-linux-musl" > /tmp/target ;; \
      *) echo "Unsupported architecture: ${TARGETARCH}" && exit 1 ;; \
    esac

# Download binary archive and checksums
WORKDIR /download
RUN --mount=type=cache,target=/var/cache/apk \
    TARGET=$(cat /tmp/target) && \
    ARCHIVE="subcog-${VERSION}-${TARGET}.tar.gz" && \
    BASE_URL="https://github.com/zircote/subcog/releases/download/v${VERSION}" && \
    echo "Downloading ${ARCHIVE}..." && \
    curl -fsSL "${BASE_URL}/${ARCHIVE}" -o "${ARCHIVE}" && \
    curl -fsSL "${BASE_URL}/checksums.txt" -o checksums.txt && \
    # Verify checksum
    grep "${ARCHIVE}" checksums.txt | sha256sum -c - && \
    # Extract binary
    tar -xzf "${ARCHIVE}" && \
    chmod +x subcog && \
    # Verify binary runs (basic sanity check on same-arch builds)
    if [ "$(uname -m)" = "x86_64" ] && [ "${TARGETARCH}" = "amd64" ]; then \
      ./subcog --version; \
    elif [ "$(uname -m)" = "aarch64" ] && [ "${TARGETARCH}" = "arm64" ]; then \
      ./subcog --version; \
    fi

# Create minimal passwd/group files for non-root user support in scratch
# Using uid/gid 65532 (same as distroless nonroot user)
RUN mkdir -p /scratch-etc && \
    echo "root:x:0:0:root:/root:/sbin/nologin" > /scratch-etc/passwd && \
    echo "nonroot:x:65532:65532:nonroot:/home/nonroot:/sbin/nologin" >> /scratch-etc/passwd && \
    echo "root:x:0:" > /scratch-etc/group && \
    echo "nonroot:x:65532:" >> /scratch-etc/group

# Create data directory with correct ownership
RUN mkdir -p /scratch-data && chown 65532:65532 /scratch-data

# ------------------------------------------------------------------------------
# Stage 2: Production image (scratch - truly minimal)
# ------------------------------------------------------------------------------
FROM scratch AS runtime

# Metadata labels (OCI Image Spec)
LABEL org.opencontainers.image.title="Subcog" \
      org.opencontainers.image.description="A persistent memory system for AI coding assistants" \
      org.opencontainers.image.url="https://github.com/zircote/subcog" \
      org.opencontainers.image.source="https://github.com/zircote/subcog" \
      org.opencontainers.image.vendor="zircote" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.base.name="scratch"

# Copy CA certificates for HTTPS support
COPY --from=downloader /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

# Copy minimal passwd/group for non-root user support
COPY --from=downloader /scratch-etc/passwd /etc/passwd
COPY --from=downloader /scratch-etc/group /etc/group

# Copy data directory with correct ownership
COPY --from=downloader --chown=65532:65532 /scratch-data /data

# Copy binary from downloader stage
COPY --from=downloader --chown=65532:65532 /download/subcog /usr/local/bin/subcog

# Data directory for SQLite database and embeddings cache
VOLUME ["/data"]

# Environment configuration
ENV SUBCOG_DATA_DIR=/data \
    SUBCOG_LOG_FORMAT=json \
    RUST_BACKTRACE=1 \
    SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt

# Run as non-root user (uid 65532, gid 65532)
USER 65532:65532

# Default entrypoint
ENTRYPOINT ["/usr/local/bin/subcog"]

# Default command (show help)
CMD ["--help"]
