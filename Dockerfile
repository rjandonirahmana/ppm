# syntax=docker/dockerfile:1.7
# ═══════════════════════════════════════════════════════════════════════════════
# Dockerfile — AFM SMART (Leptos SSR + Axum, satu binary)
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
#           RECORDINGS_DIR (default ./recordings), DB_POOL_MAX_SIZE (default 16),
#           UPLOAD_TMP_DIR (default <temp OS>/ppm-upload — WAJIB di disk, bukan
#           tmpfs; lihat web/multipart.rs)
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
    curl binaryen brotli

# ── Kunci versi toolchain ─────────────────────────────────────────────────────
# `rust-toolchain.toml` DISALIN DULUAN, sebelum apa pun yang memanggil cargo.
# Tanpa ini rustup memakai nightly bawaan image — yang bergerak tiap kali image
# di-rebuild — sehingga BINARI PRODUKSI dibangun compiler yang berbeda dari yang
# dipakai di laptop dan di CI. Perbedaan itu tak terlihat sampai satu regresi
# nightly menghentikan deploy tanpa satu baris kode pun berubah, dan tak ada
# yang bisa mereproduksinya di tempat lain.
#
# Berkas ini juga yang menyatakan `targets = ["wasm32-unknown-unknown"]`, jadi
# rustup memasangnya sendiri — `rustup target add` tak lagi diperlukan.
# Konsekuensi yang diterima: satu toolchain tambahan diunduh di atas bawaan
# image. Itu harga yang jauh lebih murah daripada build yang tak reprodusibel.
COPY rust-toolchain.toml ./
RUN rustup show

# cargo-leptos sebagai layer sendiri (rerun hanya saat base/toolchain berubah).
# Versi DIPIN: `--locked` hanya mengunci dependensi cargo-leptos, bukan versi
# cargo-leptos itu sendiri. Tanpa `--version`, build bulan depan bisa memakai
# rilis baru yang mengubah tata letak keluaran (nama file /pkg, lokasi hash.txt)
# — persis kelas kegagalan yang sudah pernah menimpa proyek ini, lihat catatan
# di bawah. Samakan dengan versi yang dipakai pengembang (`cargo leptos --version`).
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    cargo install cargo-leptos --locked --version 0.3.7

# ── Kunci versi Tailwind ──────────────────────────────────────────────────────
# Versi Tailwind TIDAK ditentukan repo ini, melainkan dipatri di dalam biner
# cargo-leptos (0.3.6 dan 0.3.7 sama-sama membawa v4.2.1) dan diunduh saat build.
# Artinya, tanpa baris ini, menaikkan cargo-leptos suatu hari nanti ikut
# mengganti compiler CSS tanpa ada yang memutuskannya — dan selisihnya muncul
# sebagai tata letak yang bergeser di produksi, bukan sebagai galat build.
#
# Satu-satunya kenop yang tersedia adalah env var ini; tak ada kunci
# `tailwind-version` di `[package.metadata.leptos]`.
#
# Kalau ingin naik versi Tailwind, ubah DI SINI dan jalankan `cargo leptos build`
# sekali di lokal dengan env var yang sama sebelum deploy — kalau tidak, CSS
# pengembang dan CSS produksi dibangun oleh dua compiler berbeda.
ENV LEPTOS_TAILWIND_VERSION=v4.2.1

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
    cargo build --release --features ssr

# Deps WASM/hydrate
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=target,target=/app/target \
    cargo build --release --target wasm32-unknown-unknown --no-default-features --features hydrate

# ── Build final ─────────────────────────────────────────────────────────────────
COPY src/ ./src/
COPY public/ ./public/
COPY tailwind.config.js ./tailwind.config.js
# Sentuh agar Cargo tahu source berubah setelah swap dummy→real.
RUN touch src/main.rs src/lib.rs

# cargo leptos build → WASM (hydrate) + binari SSR sekaligus. Artefak dep di
# cache id=target → hanya source berubah yang recompile.
#
# CATATAN: `hash-files = FALSE` (Cargo.toml) → nama file /pkg TETAP
# (`ppm.wasm`, `ppm.js`, `ppm.css`) dan TIDAK ada `hash.txt`.
#
# Komentar di sini dulu menyatakan sebaliknya (`hash-files = true`, nama
# ber-hash, runtime memetakan lewat hash.txt) — bertentangan dengan Cargo.toml
# di paket yang sama. Siapa pun yang menyiapkan deploy dari komentar ini akan
# menunggu berkas ber-hash yang tak pernah ada. Dokumentasi yang berbohong lebih
# berbahaya daripada tak ada dokumentasi (pelajaran yang sudah ditulis sendiri
# di migrasi 49).
#
# KONSEKUENSI nama tetap, dan ini yang penting saat deploy: nama berkas TIDAK
# berubah antar rilis, jadi browser bisa menyajikan /pkg lama dari cache.
# Karena itu rute /pkg disajikan `no-cache, must-revalidate` (lihat main.rs) —
# JANGAN diubah jadi `immutable` selama hash-files masih false.
#
# Sanity check di bawah: ada berkas .wasm di /pkg.
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=target,target=/app/target \
    cargo leptos build --release \
    && cp /app/target/release/ppm /app/ppm-bin \
    && cp -r /app/target/site /app/site-out \
    && test -f /app/site-out/pkg/*.wasm || (echo "ERROR: WASM file not found" && exit 1)

# ── Pra-kompresi aset /pkg ────────────────────────────────────────────────────
# Dulu `CompressionLayer` mengompresi ppm.wasm (1–3 MB) ULANG untuk setiap klien
# yang belum punya salinannya — brotli kualitas tinggi atas berkas sebesar itu
# adalah pekerjaan CPU yang terasa di VPS kecil, dan terjadi tepat pada momen
# paling genting: kunjungan pertama seseorang, sebelum satu piksel pun tampil.
#
# Hasilnya selalu sama untuk berkas yang sama, jadi dikerjakan SEKALI di sini
# dan disajikan `ServeDir::precompressed_*` (main.rs). Kualitas 11 sengaja
# dipakai — di sini waktunya tak dibayar pengguna mana pun.
#
# Aman untuk pengembangan: tanpa berkas .br/.gz, ServeDir menyajikan yang asli
# dan CompressionLayer mengambil alih seperti sebelumnya.
RUN for f in /app/site-out/pkg/*.wasm /app/site-out/pkg/*.js /app/site-out/pkg/*.css; do \
        [ -f "$f" ] || continue; \
        brotli -q 11 -f -o "$f.br" "$f"; \
        gzip -9 -c "$f" > "$f.gz"; \
    done && ls -la /app/site-out/pkg/

# ── Runtime ───────────────────────────────────────────────────────────────────
# KENAPA Debian, padahal builder-nya Alpine: binari dari target musl bersifat
# STATIS (ditambah OPENSSL_STATIC=1 di atas), jadi ia berjalan di distro mana
# pun — pilihan runtime jadi bebas. Debian dipilih karena `curl` untuk
# HEALTHCHECK dan ca-certificates-nya sudah teruji di sini.
#
# Ini keputusan sadar, bukan kelalaian: alternatifnya `alpine:3.x` (image lebih
# kecil, selaras builder). Yang TIDAK boleh dilakukan adalah mengganti target
# builder ke glibc sambil membiarkan runtime Alpine — binari glibc tak jalan di
# musl, dan gagalnya baru terlihat saat container start.
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

# ── Jalan sebagai NON-ROOT ────────────────────────────────────────────────────
# Proses ini memegang kredensial DB, S3/RustFS, WAHA, dan token Telegram, serta
# menulis berkas ke ./recordings dari data yang dikirim pengguna. Berjalan
# sebagai root berarti setiap celah — RCE, path traversal saat menulis rekaman,
# atau container escape — langsung mendapat root. UID tetap (10001) supaya
# kepemilikan berkas pada volume ter-mount tetap sama antar rebuild.
RUN useradd --system --uid 10001 --create-home --shell /usr/sbin/nologin ppm

WORKDIR /app

COPY --from=builder /app/ppm-bin   ./ppm
COPY --from=builder /app/site-out  ./target/site
# Cargo.toml WAJIB saat runtime: get_configuration(Some("Cargo.toml")) baca
# [package.metadata.leptos] utk site-addr & site-root. Hilang → panic startup.
COPY --from=builder /app/Cargo.toml ./Cargo.toml

# Direktori rekaman siaran (default RECORDINGS_DIR=./recordings). Wajib writable
# OLEH USER `ppm` — bukan root, lihat catatan user di atas.
RUN mkdir -p /app/recordings && chown -R ppm:ppm /app

USER ppm

EXPOSE 3000

ENV LEPTOS_SITE_ROOT=target/site
ENV LEPTOS_ENV=PROD

# /healthz murah (tanpa query DB) → tak "unhealthy" saat DB sibuk.
#
# start-period 45s (dulu 20s): selama jendela ini kegagalan probe TIDAK dihitung
# sebagai unhealthy. Startup menyambung ke Postgres & Redis, dan pada instalasi
# baru `ensure_seed_admin` ikut menulis. Di VPS yang sibuk atau saat DB baru
# bangun, 20 detik cukup ketat untuk membuat container di-restart tepat sebelum
# ia sempat siap — lalu mengulanginya terus. Melebihkan jendela ini tak berbiaya
# apa pun: begitu /healthz menjawab, probe langsung berlaku normal.
HEALTHCHECK --interval=15s --timeout=3s --start-period=45s --retries=3 \
    CMD curl -fsS http://localhost:3000/healthz || exit 1

CMD ["./ppm"]
