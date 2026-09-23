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

# Stage 3: Runtime stage
# Using Google Distroless (cc-debian13:nonroot) for a minimal attack surface
# (no shell, package manager, or extra utilities). The nonroot tag runs as uid 65532.
FROM gcr.io/distroless/cc-debian13:nonroot
WORKDIR /app

# Lets the server warn when asked to bind loopback, which is unreachable
# through the container port mapping
ENV HONE_IN_CONTAINER=1

# Owned by uid 65532, mode 0755, so that user can execute the binary and read the UI.
COPY --chown=65532:65532 --chmod=755 --from=backend-builder /app/target/release/hone /app/hone
COPY --chown=65532:65532 --chmod=755 --from=frontend-builder /app/ui/dist /app/ui/dist
EXPOSE 3000
ENTRYPOINT ["/app/hone"]
CMD ["serve", "--host", "0.0.0.0", "--static-dir", "/app/ui/dist"]
