#!/usr/bin/env bash
# Regenerasi subset ikon Material Symbols → public/fonts/material-symbols.woff2
# dari ikon yang DIPAKAI di src/web/. Jalankan tiap menambah ikon baru
# (kalau tidak, ikon baru tak akan muncul karena tak ada di subset).
#
#   ./scripts/fetch-icons.sh
#
set -euo pipefail
cd "$(dirname "$0")/.."

# Ambil nama ikon dari SEMUA sumber: span inline (>name / >"name"),
# field struct `icon: "name"` (mis. NavDef), dan `data-icon="name"`. Nama ikon
# boleh mengandung digit (mis. qr_code_2).
ICONS=$( {
    grep -rohE 'material-symbols-outlined[^>]*>"?[a-z0-9_]+' src/web/ | grep -oE '[a-z0-9_]+$'
    grep -rohE 'icon:[[:space:]]*"[a-z0-9_]+"' src/web/ | grep -oE '"[a-z0-9_]+"' | tr -d '"'
    grep -rohE 'data-icon="[a-z0-9_]+"' src/web/ | grep -oE '"[a-z0-9_]+"' | tr -d '"'
  } | sort -u | paste -sd, - )
echo "Ikon dipakai: $(echo "$ICONS" | tr ',' '\n' | grep -c .)"

UA="Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36"
# Axis: opsz 24, wght 400..700, FILL 0..1, GRAD 0 (mencakup ikon terisi + tebal).
URL="https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:opsz,wght,FILL,GRAD@24,400..700,0..1,0&icon_names=${ICONS}&display=block"

SRC=$(curl -sf -A "$UA" "$URL" | grep -oE 'src: url\([^)]+\)' | head -1 | sed -E 's/src: url\(([^)]+)\)/\1/')
[ -n "$SRC" ] || { echo "GAGAL ambil URL font dari Google"; exit 1; }

mkdir -p public/fonts
curl -sf -A "$UA" -o public/fonts/material-symbols.woff2 "$SRC"
ls -la public/fonts/material-symbols.woff2
echo "Selesai. Rebuild: cargo leptos watch"
