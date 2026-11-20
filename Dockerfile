# syntax=docker/dockerfile:1

# ==============================================================================
# Stage 1: Build Svelte 5 Light/Dark SaaS Console
# ==============================================================================
FROM node:22-slim AS ui-builder
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

# ==============================================================================
# Stage 2: Compile Rust 6-Layer Security Gateway + Embedded UI
# ==============================================================================
FROM rust:1-bookworm AS rust-builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY --from=ui-builder /app/frontend/dist ./frontend/dist
RUN cargo build --release --locked -p aegismesh-server --bin aegismesh

# ==============================================================================
# Stage 3: Minimal Hardened Production Runtime (Google Cloud Run Ready)
# ==============================================================================
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home aegis

WORKDIR /app
RUN mkdir -p /app/database && chown -R aegis:aegis /app

COPY --from=rust-builder /app/target/release/aegismesh /app/aegismesh
COPY --chown=aegis:aegis database/moneymax.db /app/database/moneymax.db
COPY --chown=aegis:aegis aegismesh.toml.example /app/aegismesh.toml

USER aegis
ENV PORT=8080 \
    AEGIS_HOST=0.0.0.0 \
    RUST_LOG=info

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -fsS "http://127.0.0.1:${PORT}/health" || exit 1

ENTRYPOINT ["/app/aegismesh", "--no-browser"]
