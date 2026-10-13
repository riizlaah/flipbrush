#!/usr/bin/env bash
# DESIGN.md §10: "Tidak boleh ada hex literal di luar theme.rs."
# Dijalankan oleh CI dan bisa dipakai lokal: ./scripts/check-hex-literals.sh
set -euo pipefail

cd "$(dirname "$0")/.."

# Pola hex warna: #RRGGBB / 0xRRGGBB (case-insensitive), di file .rs
# kecuali theme.rs. Komentar dikecualikan agar boleh menyebut token hex.
matches=$(grep -rnE '(#|0[xX])[0-9a-fA-F]{6}\b' \
    --include='*.rs' \
    --exclude='theme.rs' \
    flipbrush-app flipbrush-core flipbrush-platform 2>/dev/null \
    | grep -vE '^\S+:[0-9]+:\s*//' || true)

if [[ -n "$matches" ]]; then
    echo "FAIL: hex literal di luar theme.rs (DESIGN.md §10):"
    echo "$matches"
    exit 1
fi

echo "OK: tidak ada hex literal di luar theme.rs."
