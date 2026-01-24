# Stage 1: Build Rust backend
FROM rust:1.92-bookworm AS backend-builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
RUN cargo build --release --bin hone

# Stage 2: Build frontend
FROM node:24-bookworm AS frontend-builder
WORKDIR /app
COPY ui/package*.json ui/
RUN cd ui && npm ci
COPY ui/ ui/
RUN cd ui && npm run build

# Stage 3: Docker Hardened Image runtime
FROM dhi.io/debian-base:bookworm
WORKDIR /app
COPY --from=backend-builder /app/target/release/hone /app/hone
COPY --from=frontend-builder /app/ui/dist /app/ui/dist
EXPOSE 3000
ENTRYPOINT ["/app/hone"]
CMD ["serve", "--host", "0.0.0.0", "--static-dir", "/app/ui/dist"]
