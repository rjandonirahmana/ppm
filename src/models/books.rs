//! models/books.rs — Materi (Qur'an/Hadist, migrasi 18/25) + progres per santri.
//! Quran: unit = ayat per surat. Hadist: unit = halaman. Progres = peta per-unit
//! 3-status (kosong/setengah/penuh) yang diisi santri via grid (mirip kalender).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Satu surat dalam materi Qur'an (nama + jumlah ayat).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Surah {
    pub name: String,
    pub ayat: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookItem {
    pub id: i64,
    pub title: String,
    /// "quran" | "hadist".
    pub category: String,
    /// Total unit: halaman (hadist) atau TOTAL ayat (quran).
    pub total_pages: i32,
    /// Daftar surat (hanya untuk quran; kosong utk hadist).
    pub surahs: Vec<Surah>,
}

/// Progres SATU santri pada SATU materi (books + academic_user gabungan).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookProgressItem {
    pub book_id: i64,
    pub book_title: String,
    pub category: String,
    pub total_pages: i32,
    pub surahs: Vec<Surah>,
    /// Peta status per-unit: key "<halaman>" (hadist) / "<surahIdx>:<ayat>"
    /// (quran) → 1 (setengah) | 2 (penuh). Unit tak ada = kosong.
    pub unit_status: HashMap<String, u8>,
    pub percentage: i16,
}

/// Satu baris audit akademik (tab "Akademik" /students) — ringkasan progres
/// SEMUA materi satu santri, utk lihat siapa yang paling tertinggal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudentAcademicItem {
    pub user_id: i64,
    pub name: String,
    pub nis: String,
    pub avg_percentage: i32,
    pub books_started: i64,
    pub total_books: i64,
}
