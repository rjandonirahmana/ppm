# syntax=docker/dockerfile:1.7
# ═══════════════════════════════════════════════════════════════════════════════
# Dockerfile — PPM AFM (Leptos SSR + Axum, satu binary)
#
# Build pipeline (pola sama e-ticketing):
#   Stage 1 (builder): Alpine Rust nightly
#     a. Install cargo-leptos (layer sendiri, hanya rerun saat toolchain berubah)
#     b. Pre-compile deps (dummy src) — cache s/d Cargo.toml/lock berubah
#     c. cargo leptos build --release — WASM + SSR sekali jalan
#   Stage 2 (runtime): debian:bookworm-slim
#
# Env WAJIB saat runtime: DATABASE_URL, REDIS_URL, JWT_SECRET
# Opsional: WAHA_BASE_URL/SESSION/API_KEY, TELEGRAM_BOT_TOKEN/ADMIN_CHAT_ID,
#           APP_BASE_URL, RUSTFS_ENDPOINT/ACCESS_KEY/SECRET_KEY/BUCKET/PUBLIC_URL,
#           RECORDINGS_DIR (default ./recordings), DB_POOL_MAX_SIZE (default 8)
#
# Run:  docker build -t ppm .
#       docker run -p 3200:3000 --env-file .env ppm
# ═══════════════════════════════════════════════════════════════════════════════

# ── Builder ───────────────────────────────────────────────────────────────────
FROM rustlang/rust:nightly-alpine AS builder

RUN apk add --no-cache \
    musl-dev g++ make perl pkgconfig \
    openssl-dev openssl-libs-static \
    zlib-dev zlib-static \
    curl binaryen

RUN rustup target add wasm32-unknown-unknown

# cargo-leptos sebagai layer sendiri (rerun hanya saat base/toolchain berubah).
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    cargo install cargo-leptos --locked

ENV OPENSSL_STATIC=1
ENV PKG_CONFIG_ALLOW_CROSS=1
WORKDIR /app

# ── Pre-compile dependency ─────────────────────────────────────────────────────
# Salin manifest + input yang memengaruhi dep saja → layer ini invalid hanya saat
# Cargo.toml/lock/style/config berubah, BUKAN saat edit src/.
COPY Cargo.toml Cargo.lock ./
COPY style/ ./style/
COPY .cargo/ ./.cargo/

# Dummy source agar Cargo compile & cache SEMUA dependency.
RUN mkdir -p src && \
    printf 'fn main() {}' > src/main.rs && \
    printf '' > src/lib.rs

# Deps SSR (native musl)
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=target,target=/app/target \
    cargo build --release --features ssr 2>&1 || true

# Deps WASM/hydrate
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=target,target=/app/target \
    cargo build --release --target wasm32-unknown-unknown --no-default-features --features hydrate 2>&1 || true

# ── Build final ─────────────────────────────────────────────────────────────────
COPY src/ ./src/
COPY public/ ./public/
COPY tailwind.config.js ./tailwind.config.js
# Sentuh agar Cargo tahu source berubah setelah swap dummy→real.
RUN touch src/main.rs src/lib.rs

# cargo leptos build → WASM (hydrate) + binari SSR sekaligus. Artefak dep di
# cache id=target → hanya source berubah yang recompile.
#
# CATATAN: `hash-files = true` (Cargo.toml) → nama file /pkg ber-hash
# (`ppm.<hash>.wasm`, `ppm.<hash>.js`) + `hash.txt` (di root site, BUKAN /pkg).
# Runtime memetakan nama via hash.txt (HydrationScripts/HashedStylesheet). JANGAN
# menormalkan nama & jangan mengasumsikan `*_bg.wasm` atau lokasi hash.txt (dulu
# dua-duanya bikin build gagal padahal cargo sukses). Sanity check: ada file
# .wasm apa pun di /pkg.
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=target,target=/app/target \
    cargo leptos build --release 2>&1 \
    && cp /app/target/release/ppm /app/ppm-bin \
    && cp -r /app/target/site /app/site-out \
    && ls /app/site-out/pkg/*.wasm >/dev/null 2>&1

# ── Runtime ───────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/ppm-bin   ./ppm
COPY --from=builder /app/site-out  ./target/site
# Cargo.toml WAJIB saat runtime: get_configuration(Some("Cargo.toml")) baca
# [package.metadata.leptos] utk site-addr & site-root. Hilang → panic startup.
COPY --from=builder /app/Cargo.toml ./Cargo.toml

# Direktori rekaman siaran (default RECORDINGS_DIR=./recordings). Wajib writable.
RUN mkdir -p /app/recordings

EXPOSE 3000

ENV LEPTOS_SITE_ROOT=target/site
ENV LEPTOS_ENV=PROD

# /healthz murah (tanpa query DB) → tak "unhealthy" saat DB sibuk.
HEALTHCHECK --interval=15s --timeout=3s --start-period=20s --retries=3 \
    CMD curl -fsS http://localhost:3000/healthz || exit 1

CMD ["./ppm"]
