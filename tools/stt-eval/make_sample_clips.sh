#!/bin/sh
# Generates synthetic Indonesian clips with the macOS "Damayanti" voice, as a smoke test of the
# pipeline. Real benchmarking needs REAL meeting/interview recordings (record them with BICARA).
set -e
cd "$(dirname "$0")/clips"
gen() { printf '%s' "$2" > "$1.txt"; say -v Damayanti -o "$1.aiff" "$2"; }
gen sintetis-rapat "Selamat pagi Bapak dan Ibu sekalian. Hari ini kita akan membahas rencana peluncuran produk baru. Apakah ada pertanyaan sebelum kita mulai?"
gen sintetis-campur "Jadi kita perlu align dulu sama stakeholder, terus deadline-nya kita set minggu depan ya. Gimana menurut kamu?"
gen sintetis-interview "Ceritakan tentang diri Anda dan kenapa Anda tertarik dengan posisi ini di perusahaan kami."
echo "OK: $(ls *.txt | wc -l) clip"
