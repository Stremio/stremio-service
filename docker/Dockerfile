# Copyright (C) 2017-2026 Smart Code OOD 203358507
# Multi-stage Dockerfile for stremio-service
# Automatically downloads required binaries if they don't exist

###############################################################################
# Stage 1: Build FFmpeg (using jellyfin-ffmpeg for broad codec support)
###############################################################################
FROM alpine:3.20 AS ffmpeg-builder

RUN apk add --no-cache \
    build-base \
    coreutils \
    nasm \
    yasm \
    git \
    curl \
    # FFmpeg dependencies
    gnutls-dev \
    freetype-dev \
    lame-dev \
    libass-dev \
    libogg-dev \
    libtheora-dev \
    libvorbis-dev \
    libvpx-dev \
    libwebp-dev \
    opus-dev \
    x264-dev \
    x265-dev \
    dav1d-dev \
    aom-dev \
    xvidcore-dev \
    fdk-aac-dev \
    libva-dev \
    libdrm-dev \
    zimg-dev

WORKDIR /build

# Build FFmpeg 4.4.x (required by Stremio)
RUN git clone --depth 1 --branch n4.4.4 https://github.com/FFmpeg/FFmpeg.git ffmpeg && \
    cd ffmpeg && \
    ./configure \
        --prefix=/usr/local \
        --disable-debug \
        --disable-doc \
        --disable-ffplay \
        --enable-gpl \
        --enable-version3 \
        --enable-nonfree \
        --enable-small \
        --enable-gnutls \
        --enable-libass \
        --enable-libfreetype \
        --enable-libmp3lame \
        --enable-libopus \
        --enable-libtheora \
        --enable-libvorbis \
        --enable-libvpx \
        --enable-libwebp \
        --enable-libx264 \
        --enable-libx265 \
        --enable-libdav1d \
        --enable-libaom \
        --enable-libxvid \
        --enable-libfdk-aac \
        --enable-libzimg \
        --enable-vaapi && \
    make -j$(nproc) && \
    make install

###############################################################################
# Stage 2: Build the Rust application
###############################################################################
FROM rust:1.82-alpine AS rust-builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    pkgconfig \
    git \
    curl \
    glib-dev \
    gtk+3.0-dev \
    libayatana-appindicator-dev \
    zlib-static

# Force dynamic linking for system libraries (GTK, GLib, etc.) which aren't available statically on Alpine
ENV RUSTFLAGS="-C target-feature=-crt-static"

WORKDIR /build

# Copy Cargo files first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./

# Create dummy source for dependency compilation
RUN mkdir -p src && \
    echo "fn main() { println!(\"dummy\"); }" > src/main.rs && \
    mkdir -p src/bin && \
    echo "fn main() { println!(\"dummy\"); }" > src/bin/bundle-macos.rs

# Pre-build dependencies (this layer is cached)
RUN cargo build --release --features=offline-build 2>/dev/null || true

# Copy actual source code
COPY src ./src
COPY icons ./icons
COPY resources ./resources

# Touch to force rebuild
RUN touch src/main.rs

# Build the final binary (offline mode - we'll download server.js separately)
RUN cargo build --release --features=bundled,offline-build

###############################################################################
# Stage 3: Download required binaries
###############################################################################
FROM alpine:3.20 AS downloader

RUN apk add --no-cache curl jq tar xz

WORKDIR /download

# Get server.js version from Cargo.toml and download it
ARG SERVER_VERSION=v4.20.16
RUN echo "Downloading server.js ${SERVER_VERSION}..." && \
    curl -fsSL "https://dl.strem.io/server/${SERVER_VERSION}/desktop/server.js" -o server.js && \
    echo "server.js downloaded successfully"

# Download stremio-runtime (Node.js runtime bundled for Stremio)
# This is typically from the stremio-shell releases
ARG RUNTIME_VERSION=v14.18.1
RUN echo "Downloading stremio-runtime..." && \
    # Try to get from GitHub releases, fall back to node binary
    (curl -fsSL "https://nodejs.org/dist/${RUNTIME_VERSION}/node-${RUNTIME_VERSION}-linux-x64.tar.xz" -o node.tar.xz && \
     tar -xf node.tar.xz && \
     mv node-${RUNTIME_VERSION}-linux-x64/bin/node stremio-runtime && \
     rm -rf node.tar.xz node-${RUNTIME_VERSION}-linux-x64) || \
    echo "Using system node as runtime"

###############################################################################
# Stage 4: Final runtime image
###############################################################################
FROM alpine:3.20 AS final

LABEL org.opencontainers.image.source="https://github.com/Stremio/stremio-service"
LABEL org.opencontainers.image.description="Stremio Service - Freedom to Stream"
LABEL org.opencontainers.image.licenses="GPL-2.0"
LABEL maintainer="Stremio"

# Install runtime dependencies
RUN apk add --no-cache \
    nginx \
    bash \
    curl \
    ca-certificates \
    libgcc \
    libstdc++ \
    # FFmpeg runtime dependencies
    gnutls \
    freetype \
    lame-libs \
    libass \
    libogg \
    libtheora \
    libvorbis \
    libvpx \
    libwebp \
    opus \
    x264-libs \
    x265-libs \
    dav1d-libs \
    aom-libs \
    xvidcore \
    fdk-aac \
    libva \
    libdrm \
    zimg \
    # For IP banning
    iptables \
    ip6tables \
    # For log processing
    grep \
    gawk \
    coreutils \
    # For htpasswd \
    apache2-utils \
    # For Rust UI dependencies (linked via tao/tray-icon) \
    glib \
    gtk+3.0 \
    libayatana-appindicator \
    zlib

# Create directories
RUN mkdir -p /app /data /var/log/nginx /var/log/stremio \
    /etc/nginx/http.d /run/nginx /var/lib/stremio-bans

WORKDIR /app

# Copy FFmpeg from builder
COPY --from=ffmpeg-builder /usr/local/bin/ffmpeg /app/ffmpeg
COPY --from=ffmpeg-builder /usr/local/bin/ffprobe /app/ffprobe

# Copy Rust binary
COPY --from=rust-builder /build/target/release/stremio-service /app/stremio-service

# Copy downloaded binaries
COPY --from=downloader /download/server.js /app/server.js
COPY --from=downloader /download/stremio-runtime /app/stremio-runtime

# Make binaries executable
RUN chmod +x /app/stremio-service /app/stremio-runtime /app/ffmpeg /app/ffprobe

# Copy existing binaries if they exist (to allow pre-built binaries)
# These COPY commands will fail silently if files don't exist due to the -f flag behavior
COPY resources/bin/linux/server.js* /app/ 2>/dev/null || true
COPY resources/bin/linux/stremio-runtime* /app/ 2>/dev/null || true
COPY resources/bin/linux/ffmpeg* /app/ 2>/dev/null || true
COPY resources/bin/linux/ffprobe* /app/ 2>/dev/null || true

# Copy nginx configuration
COPY docker/nginx/nginx.conf /etc/nginx/nginx.conf
COPY docker/nginx/http.d/default.conf /etc/nginx/http.d/default.conf
COPY docker/nginx/http.d/https.conf /etc/nginx/http.d/https.conf.template
COPY docker/nginx/blocked_ips.conf /etc/nginx/blocked_ips.conf
COPY docker/nginx/scanner_block.conf /etc/nginx/scanner_block.conf
COPY docker/nginx/auth.conf /etc/nginx/auth.conf

# Copy entrypoint and helper scripts
COPY docker/scripts/docker-entrypoint.sh /entrypoint.sh
COPY docker/scripts/ip-banner.sh /app/ip-banner.sh
COPY docker/scripts/healthcheck.sh /app/healthcheck.sh

RUN chmod +x /entrypoint.sh /app/ip-banner.sh /app/healthcheck.sh

# Create log files
RUN touch /var/log/nginx/access.log /var/log/nginx/scanner.log /var/log/nginx/unmatched.log

# Environment variables
ENV STREMIO_DATA_DIR=/data
ENV FFMPEG_BIN=/app/ffmpeg
ENV FFPROBE_BIN=/app/ffprobe
ENV NO_CORS=0
ENV AUTO_SERVER_URL=1
ENV SERVER_URL=""
ENV IPADDRESS=""
ENV DOMAIN=""
ENV USERNAME=""
ENV PASSWORD=""
ENV ENABLE_IP_BAN=1
ENV BAN_THRESHOLD=10
ENV BAN_WINDOW=60
ENV BAN_DURATION=3600
ENV LOG_LEVEL=info

# Volume for persistent data
VOLUME ["/data"]

# Expose ports
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=15s --retries=3 \
    CMD /app/healthcheck.sh

ENTRYPOINT ["/entrypoint.sh"]
