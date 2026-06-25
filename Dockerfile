# Empire 5 — multi-stage Docker build
#
# Stage 1 (builder): compiles all workspace binaries with the official Rust image.
# Stage 2 (runtime): copies only the binaries into a minimal Debian image (~15 MB).
#
# Build:  docker compose build
# Run:    docker compose up -d

# ── Stage 1: build ────────────────────────────────────────────────────────────
FROM rust:1.87-slim AS builder

# System libs needed by sqlx (links against libssl via OpenSSL)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy workspace manifests first so cargo can cache the dependency layer.
# The inner crate Cargo.toml files are needed for workspace resolution.
COPY Cargo.toml Cargo.lock ./
COPY crates/empire-types/Cargo.toml   crates/empire-types/Cargo.toml
COPY crates/empire-config/Cargo.toml  crates/empire-config/Cargo.toml
COPY crates/empire-db/Cargo.toml      crates/empire-db/Cargo.toml
COPY crates/empire-server/Cargo.toml  crates/empire-server/Cargo.toml
COPY crates/empire-client/Cargo.toml  crates/empire-client/Cargo.toml
COPY crates/empire-world/Cargo.toml   crates/empire-world/Cargo.toml
COPY crates/empire-util/Cargo.toml    crates/empire-util/Cargo.toml

# Seed stub src files so cargo can resolve the workspace without full source.
# (The real source is copied below; this layer is only for dependency caching.)
RUN mkdir -p \
    crates/empire-types/src \
    crates/empire-config/src \
    crates/empire-db/src \
    crates/empire-server/src \
    crates/empire-client/src \
    crates/empire-world/src \
    crates/empire-util/src && \
    echo "pub fn _placeholder() {}" > crates/empire-types/src/lib.rs && \
    echo "pub fn _placeholder() {}" > crates/empire-config/src/lib.rs && \
    echo "pub fn _placeholder() {}" > crates/empire-db/src/lib.rs && \
    echo "fn main() {}" > crates/empire-server/src/main.rs && \
    echo "fn main() {}" > crates/empire-client/src/main.rs && \
    echo "fn main() {}" > crates/empire-world/src/main.rs && \
    echo "fn main() {}" > crates/empire-util/src/empdump.rs && \
    echo "fn main() {}" > crates/empire-util/src/empsched.rs && \
    echo "fn main() {}" > crates/empire-util/src/pconfig.rs

RUN cargo build --release 2>/dev/null || true

# Copy the real source.  Touch all .rs files so cargo sees them as newer
# than the stub artifacts and does a proper full recompile.
COPY crates/ crates/
RUN find crates -name "*.rs" -exec touch {} +

RUN cargo build --release

# ── Stage 2: runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /srv/empire

# Binaries from the build stage
COPY --from=builder /build/target/release/empire-server  /usr/local/bin/
COPY --from=builder /build/target/release/empire-world   /usr/local/bin/
COPY --from=builder /build/target/release/empdump        /usr/local/bin/
COPY --from=builder /build/target/release/empsched       /usr/local/bin/
COPY --from=builder /build/target/release/pconfig        /usr/local/bin/

# Config, entrypoint, and bundled info files
COPY docker/empire.toml    ./empire.toml
COPY docker/entrypoint.sh  ./entrypoint.sh
COPY info/                 ./info/
RUN chmod +x entrypoint.sh

# /srv/empire/data is the only path that must persist across container restarts.
# Mount a named volume or bind-mount here.
VOLUME ["/srv/empire/data"]

EXPOSE 6665

ENTRYPOINT ["./entrypoint.sh"]
