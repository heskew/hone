# Stage 1: Build Rust backend
FROM rust:1.92-trixie AS backend-builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
RUN cargo build --release --bin hone

# Stage 2: Build frontend
FROM node:24-trixie AS frontend-builder
WORKDIR /app
COPY ui/package*.json ui/
RUN cd ui && npm ci
COPY ui/ ui/
RUN cd ui && npm run build

# Stage 3: Debian 13 security libssl (CVE-2026-14456)
# Distroless cc-debian13 still ships libssl3t64 3.5.6-1~deb13u2.
# Overlay 3.5.7+ and dpkg status.d until distroless rebuilds.
FROM debian:trixie-slim AS libssl
RUN apt-get update \
    && apt-get download libssl3t64 \
    && VER="$(dpkg-deb -f libssl3t64_*.deb Version)" \
    && echo "libssl3t64 ${VER}" \
    && dpkg --compare-versions "$VER" ge "3.5.7-1~deb13u2" \
    && mkdir -p /ssl-overlay/var/lib/dpkg/status.d \
    && dpkg-deb -x libssl3t64_*.deb /ssl-overlay \
    && dpkg-deb -e libssl3t64_*.deb /tmp/libssl-ctrl \
    && cp /tmp/libssl-ctrl/control /ssl-overlay/var/lib/dpkg/status.d/libssl3t64 \
    && cp /tmp/libssl-ctrl/md5sums /ssl-overlay/var/lib/dpkg/status.d/libssl3t64.md5sums

# Stage 4: Runtime stage
# Using Google Distroless (cc-debian13) for minimal attack surface.
# Distroless is the gold standard for security, providing a minimal attack surface 
# with no shell, package manager, or unnecessary utilities. 
FROM gcr.io/distroless/cc-debian13
WORKDIR /app
COPY --from=libssl /ssl-overlay/ /

# Lets the server warn when asked to bind loopback, which is unreachable
# through the container port mapping
ENV HONE_IN_CONTAINER=1

COPY --from=backend-builder /app/target/release/hone /app/hone
COPY --from=frontend-builder /app/ui/dist /app/ui/dist
EXPOSE 3000
ENTRYPOINT ["/app/hone"]
CMD ["serve", "--host", "0.0.0.0", "--static-dir", "/app/ui/dist"]
