//! models/books.rs — Buku materi hafalan (Qur'an/Hadist, migrasi 18) +
//! progres per santri. Dipakai panel "Progres Buku" di /students.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookItem {
    pub id: i64,
    pub title: String,
    pub total_pages: i32,
}

/// Progres SATU santri pada SATU buku (books + academic_user gabungan).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookProgressItem {
    pub book_id: i64,
    pub book_title: String,
    pub total_pages: i32,
    pub percentage: i16,
    /// "11-20, 45-50" — teks siap tampil DAN siap prefill form edit.
    pub missing_pages_label: String,
}

/// Satu baris audit akademik (tab "Akademik" /students) — ringkasan progres
/// SEMUA buku satu santri, utk lihat siapa yang paling tertinggal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudentAcademicItem {
    pub user_id: i64,
    pub name: String,
    pub nis: String,
    pub avg_percentage: i32,
    pub books_started: i64,
    pub total_books: i64,
}
