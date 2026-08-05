#!/usr/bin/env bash
# Penerap migrasi + PELACAKnya. Menggantikan kebiasaan menjalankan
# `psql -f migration/NN_*.sql` satu per satu dari ingatan.
#
#   ./scripts/migrate.sh status              # apa yang sudah & belum jalan
#   ./scripts/migrate.sh up                  # jalankan yang belum, berurutan
#   ./scripts/migrate.sh up --dry-run        # tampilkan rencananya saja
#   ./scripts/migrate.sh baseline 56         # tandai 1..56 sudah jalan (sekali)
#
# DATABASE_URL diambil dari lingkungan, atau dari berkas .env bila ada.
#
# ── KENAPA INI ADA ───────────────────────────────────────────────────────────
# Urutan migrasi dulu dijaga lewat komentar "Jalankan setelah migrasi 1–N" dan
# ingatan orang. Yang terjadi di produksi (Agustus 2026): migrasi 42 —
# berlabel "critical security" — berhenti di tengah karena satu langkahnya
# galat, dan TIDAK ADA yang mencatat itu. Berbulan-bulan skema produksi jalan
# dengan index duplikat absensi yang salah kolom, sementara berkasnya di repo
# terlihat seolah sudah diterapkan.
#
# Dua hal yang dicegah skrip ini:
#   1. Migrasi yang belum jalan tak terlihat  → tabel `schema_migrations`.
#   2. Berkas migrasi DIUBAH setelah diterapkan → checksum SHA-256 disimpan
#      saat penerapan lalu dibandingkan tiap `status`. Persis kasus 42: isinya
#      direvisi setelah versi lamanya sempat jalan, dan tak ada tanda apa pun.
#
# Tiap migrasi dijalankan dalam SATU transaksi bersama pencatatannya, jadi
# mustahil ada migrasi yang jalan tapi tak tercatat (atau sebaliknya).
set -euo pipefail

cd "$(dirname "$0")/.."

if [ -z "${DATABASE_URL:-}" ] && [ -f .env ]; then
    DATABASE_URL=$(grep -E '^DATABASE_URL=' .env | head -1 | cut -d= -f2- | tr -d '"'"'"'')
fi
if [ -z "${DATABASE_URL:-}" ]; then
    echo "DATABASE_URL belum diset (dan tak ada di .env)." >&2
    exit 1
fi

PSQL=(psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -q -A -t)

# Nomor urut dari nama berkas: "42_scan_date_unique.sql" → 42, "2.sql" → 2.
versi_dari() { basename "$1" | sed -E 's/^([0-9]+).*/\1/'; }

# Daftar migrasi terurut ANGKA (bukan leksikal — kalau tidak, 10 mendahului 2).
#
# HANYA berkas yang namanya diawali angka yang dianggap migrasi. Berkas lain
# yang kebetulan ada di folder ini (catatan, sisa `._*` bawaan macOS, berkas
# sementara editor) dilewati diam-diam — sebelumnya nama seperti `._1.sql`
# ikut terbaca dan menghasilkan versi tak masuk akal yang menggagalkan query.
daftar_migrasi() {
    find migration -maxdepth 1 -name '*.sql' -print0 \
        | xargs -0 -n1 basename \
        | grep -E '^[0-9]+' \
        | sort -t_ -k1 -n
}

pastikan_tabel() {
    "${PSQL[@]}" -c "
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version    integer     PRIMARY KEY,
            name       text        NOT NULL,
            checksum   text        NOT NULL,
            applied_at timestamptz NOT NULL DEFAULT NOW()
        )" >/dev/null
}

checksum_berkas() { sha256sum "migration/$1" | cut -c1-64; }

sudah_jalan() { "${PSQL[@]}" -c "SELECT 1 FROM schema_migrations WHERE version = $1"; }

cmd_status() {
    pastikan_tabel
    local drift=0 belum=0
    printf '%-6s %-44s %s\n' "VERSI" "BERKAS" "KEADAAN"
    while read -r f; do
        local v ck tercatat
        v=$(versi_dari "$f")
        tercatat=$("${PSQL[@]}" -c "SELECT checksum FROM schema_migrations WHERE version = $v")
        if [ -z "$tercatat" ]; then
            printf '%-6s %-44s %s\n' "$v" "$f" "BELUM JALAN"
            belum=$((belum + 1))
        else
            ck=$(checksum_berkas "$f")
            if [ "$ck" != "$tercatat" ]; then
                printf '%-6s %-44s %s\n' "$v" "$f" "BERUBAH SETELAH DITERAPKAN"
                drift=$((drift + 1))
            else
                printf '%-6s %-44s %s\n' "$v" "$f" "ok"
            fi
        fi
    done < <(daftar_migrasi)

    echo
    echo "Belum jalan: $belum   Berubah setelah diterapkan: $drift"
    # Berkas yang berubah TIDAK dijalankan ulang otomatis: isinya bisa saja
    # sudah sebagian diterapkan. Itu keputusan manusia, bukan skrip.
    [ "$drift" -eq 0 ] || echo "Periksa manual berkas yang berubah — skrip ini tak menjalankannya ulang."
}

cmd_up() {
    local dry=0
    [ "${1:-}" = "--dry-run" ] && dry=1
    pastikan_tabel
    local n=0
    while read -r f; do
        local v
        v=$(versi_dari "$f")
        [ -n "$(sudah_jalan "$v")" ] && continue
        n=$((n + 1))
        if [ "$dry" -eq 1 ]; then
            echo "akan dijalankan: $v  $f"
            continue
        fi
        echo "--- menjalankan $v  $f"
        local ck
        ck=$(checksum_berkas "$f")
        # Migrasi + pencatatannya dalam SATU transaksi: kalau migrasinya gagal,
        # catatannya ikut batal, jadi tak pernah ada "tercatat padahal gagal".
        # (Migrasi yang memuat CREATE INDEX CONCURRENTLY tak bisa begini —
        # sejauh ini tak ada yang memakainya.)
        {
            echo "BEGIN;"
            cat "migration/$f"
            echo ";"
            echo "INSERT INTO schema_migrations (version, name, checksum)
                  VALUES ($v, '$f', '$ck');"
            echo "COMMIT;"
        } | psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -q
    done < <(daftar_migrasi)
    [ "$n" -eq 0 ] && echo "Tak ada migrasi yang tertunda." || echo "Selesai: $n migrasi."
}

# Untuk database yang SUDAH berjalan sebelum skrip ini ada: tandai 1..N sebagai
# terlaksana tanpa menjalankannya. Pakai SEKALI saja, dan hanya setelah yakin
# skemanya memang sudah setara — kalau ragu, periksa dulu dengan `status`.
cmd_baseline() {
    local sampai="${1:?pakai: migrate.sh baseline <versi_terakhir>}"
    pastikan_tabel
    local n=0
    while read -r f; do
        local v ck
        v=$(versi_dari "$f")
        [ "$v" -le "$sampai" ] || continue
        [ -n "$(sudah_jalan "$v")" ] && continue
        ck=$(checksum_berkas "$f")
        "${PSQL[@]}" -c "INSERT INTO schema_migrations (version, name, checksum)
                         VALUES ($v, '$f', '$ck')" >/dev/null
        n=$((n + 1))
    done < <(daftar_migrasi)
    echo "Ditandai terlaksana: $n migrasi (s/d versi $sampai)."
}

case "${1:-status}" in
    status)   cmd_status ;;
    up)       shift; cmd_up "${1:-}" ;;
    baseline) shift; cmd_baseline "${1:-}" ;;
    *) echo "pakai: migrate.sh [status|up [--dry-run]|baseline <versi>]" >&2; exit 1 ;;
esac
