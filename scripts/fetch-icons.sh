#!/usr/bin/env bash
# Regenerasi subset ikon Material Symbols → public/fonts/material-symbols.woff2
# dari ikon yang DIPAKAI di src/web/. Jalankan tiap menambah ikon baru
# (kalau tidak, ikon baru tampil sbg TEKS karena tak ada di subset).
#
#   ./scripts/fetch-icons.sh
#
# Ekstraksi menangkap SEMUA pola pemakaian:
#   1. <span class="material-symbols-outlined">home</span>   (satu baris)
#   2. <span class="material-symbols-outlined ...">          (MULTI-BARIS —
#          "task_alt"                                          formatter Rust
#      </span>                                                 sering begini!)
#   3. icon: "home" / icon="home"      (NavDef, prop komponen)
#   4. data-icon="home"
#   5. ("home", "Label", ...)          (grid ikon berbasis tuple array)
# Pola 5 bisa ikut menangkap string non-ikon — TIDAK apa-apa: Google css2
# mengabaikan icon_names yang tak dikenal (font sedikit lebih besar saja tidak,
# nama tak dikenal memang di-skip).
set -euo pipefail
cd "$(dirname "$0")/.."

# Ekstraksi KOMPREHENSIF: tangkap SEMUA token snake_case ber-quote di src/web
# sebagai KANDIDAT ikon — apa pun posisinya dalam tuple. Ikon dipakai di banyak
# bentuk: <span ...>"home"</span>, icon="home", ("home", label, path) [staf.rs],
# (path, label, "home") [menu.rs] — pola per-posisi dulu selalu ada yang bocor.
# Google css2 MENGABAIKAN nama tak dikenal SELAMA daftar terurut alfabet (sort -u
# menjamin ini); token non-ikon (status, dsb) jadi cuma sedikit noise tak dipakai.
ICONS=$(grep -rohE '"[a-z][a-z0-9_]*"' src/web --include="*.rs" | tr -d '"' | sort -u | paste -sd, - )
echo "Kandidat nama ikon: $(echo "$ICONS" | tr ',' '\n' | grep -c .)"

UA="Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36"
# Axis: opsz 24, wght 400..700, FILL 0..1, GRAD 0 (mencakup ikon terisi + tebal).
URL="https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:opsz,wght,FILL,GRAD@24,400..700,0..1,0&icon_names=${ICONS}&display=block"

SRC=$(curl -sf -A "$UA" "$URL" | grep -oE 'src: url\([^)]+\)' | head -1 | sed -E 's/src: url\(([^)]+)\)/\1/')
[ -n "$SRC" ] || { echo "GAGAL ambil URL font dari Google (cek nama ikon tidak valid?)"; exit 1; }

mkdir -p public/fonts
curl -sf -A "$UA" -o public/fonts/material-symbols.woff2 "$SRC"
ls -la public/fonts/material-symbols.woff2
echo "Selesai. Rebuild: cargo leptos watch  (lalu hard-refresh browser)"
