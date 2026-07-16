# PPM AFM — Absensi Santri

Portal absensi santri **PPM AFM** — **Leptos SSR + Axum + PostgreSQL**. Render di
server (cepat, hemat memori), interaktif via hydration WASM. Pola mengikuti proyek
Leptos SSR lain di mesin ini (e-ticketing, wedding-web): satu binary, satu port.

## Menjalankan

```bash
# 1. Postgres + migrasi (urut)
createdb ppm
psql "$DATABASE_URL" -f migration/1.sql
psql "$DATABASE_URL" -f migration/2.sql

# 2. Env
cp .env.example .env      # isi DATABASE_URL

# 3. Dev
cargo leptos watch        # SSR + hydration WASM → http://localhost:3000
# atau SSR saja (tanpa hydration, /pkg belum dibuild):
cargo run
```

Butuh `cargo-leptos` (`cargo install cargo-leptos`) untuk build WASM/hydration.

## Struktur (lapisan: handler → service → repository → Postgres)

```
migration/1.sql   master data (users, classes, schedules, sessions, complaints)
migration/2.sql   INTI ABSENSI (attendances, point_logs, permit_requests,
                  parent_students, sessions) + kolom login/identitas users
src/
  main.rs         bootstrap Axum + Leptos SSR + seed admin + CLI `hash`
  lib.rs          entry SSR + hydrate (fn hydrate)
  config.rs       AppConfig + pool Postgres
  state.rs        AppState { pool, jwt_secret }
  auth.rs         JWT sesi (sign/verify, cookie ppm_token)
  device_api.rs   handler HTTP perangkat RFID (POST /api/rfid/scan)
  models/         DTO bersama per-domain (auth, attendance, schedule, dashboard)
  repository/     query Postgres per-domain (users, attendance, schedule, device)
  service/        logika bisnis (auth login+seed, dashboard, attendance scan+pamong)
  web/
    api.rs        server functions /api-fn (login, session, santri_home, pamong)
    app.rs        shell HTML (Tailwind CDN + token desain) + router
    components.rs DeviceFrame (kartu lebar 2 kolom di desktop, ala login)
    pages/
      login.rs             → login BERFUNGSI (bcrypt → JWT cookie → redirect per peran)
      dashboard_santri.rs  → data ASLI DB (poin, jadwal, riwayat, progress)
      verifikasi_pamong.rs → antrean verifikasi ASLI + Setujui/Tolak
      menu.rs              → /menu: pratinjau semua halaman + tombol Keluar
      design_pages.rs + html/*.html → halaman desain lain (masih statis)
style/main.css    stub cargo-leptos (utility via Tailwind CDN)
```

## Auth & akun

- Login `/login` menerima **username / email / NIS** + password (bcrypt).
  Sukses → cookie HttpOnly `ppm_token` (JWT 7 hari) → redirect per peran:
  admin→/staf, teacher→/guru, supervisor→/verifikasi-pamong, santri→/santri, parent→/orang-tua.
- **Bootstrap**: bila tabel `users` KOSONG saat start, dibuat akun `admin`
  (password dari env `ADMIN_PASSWORD`, default `admin123`).
- Mengisi user manual lewat SQL? Buat hash-nya dengan:
  ```bash
  cargo run -- hash rahasia123   # → cetak bcrypt hash utk kolom password_hash
  ```

## Endpoint perangkat RFID

```
POST /api/rfid/scan
{ "api_key": "<rfid_devices.api_key>", "card": <users.rfid_cards> }
```
Alur: validasi device → kartu→santri → cari jadwal aktif (jendela 45 mnt sebelum
mulai s/d selesai, waktu WIB) → `present`/`late` dari `limit_entery_time` →
simpan `attendances` (dedup per jadwal/hari; scan di luar jadwal tetap dicatat
sebagai log gerbang). Respons JSON `{ok, message, student, status}`.

**Semua halaman sudah bisa dibuka.** Buka **`/menu`** untuk daftar link semua halaman
(login, santri, portal, riwayat, profil, staf, guru, dewan guru, poin, verifikasi
pamong & tahap-2, halaqah daftar/mulai/live, rekaman, orang tua, koneksi ortu).

> Halaman selain login & dashboard santri masih **statis** (HTML desain di-embed
> via `include_str!` + `inner_html`) — visual dulu, data disambung bertahap.

## Catatan migrasi (yang ditambah / diperbaiki)

- **FIX** `1.sql`: kolom `complaints.category` sebelumnya tergantung DI LUAR tabel
  (SQL invalid). Dipindah ke dalam `complaints`.
- **`attendances`** (inti absensi, sebelumnya belum ada): scan RFID/QR/manual per
  sesi + verifikasi **dua tahap** (`pamong_status`, `verify_status`).
- **`point_logs`** (riwayat poin, sumber kebenaran `users.points`).
- **`permit_requests`** (izin/sakit) & **`parent_students`** (koneksi orang tua↔santri).
- **`sessions`** (login server-side, opsional).
- Identitas login pada `users`: `username`, `email`, `nis`, `points`.
- **Keanggotaan kelas TIDAK di `users.class_id`** — santri bisa ikut banyak kelas &
  jadwal → dipakai junction `class_participants` (m-to-m). Kelas UTAMA santri ditandai
  `class_participants.is_primary` + index parsial (query cepat, tanpa denormalisasi).
- **Orang tua = `users` role `parent`** (bukan tabel terpisah); relasinya lewat
  junction `parent_students`.

## Desain / Tailwind

Design system "Islamic Institutional" (emerald + Work Sans, Material 3). Sumber:
`Downloads/ppm-afm-design-absensi`. Token disuntik via **Tailwind Play CDN** di
`web/app.rs` (cepat & akurat untuk MVP). **Produksi**: kompilasi Tailwind (CLI) →
satu CSS file, hindari CDN.

## Roadmap (belum dikerjakan)

Backend: auth (bcrypt login → JWT/session), guard peran, server functions
(absensi, verifikasi pamong/dewan, poin, izin), endpoint device RFID (scan → insert
attendance). Halaman lain dari desain: dashboard staf/dewan guru, verifikasi
kehadiran, pantauan poin, halaqah live, koneksi orang tua, profil, dsb.
