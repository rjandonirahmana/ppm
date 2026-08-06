//! models/schedule.rs — Tipe jadwal kelas (tampilan).

use serde::{Deserialize, Serialize};

/// Kategori kelas yang boleh siaran suara + rekaman. HANYA "Pengajian" — sholat
/// dan kategori lain (Tahfidz, Cepatan, dst.) tidak butuh/boleh rekam suara.
/// Dipakai KEDUA sisi: klien (sembunyikan AudioDock) & server (tolak upload
/// chunk — web/live_audio.rs) — satu sumber kebenaran, bukan enum (kategori
/// kelas tetap teks bebas, lihat migration 6_class_category.sql).
pub fn category_allows_recording(category: &str) -> bool {
    category.trim().eq_ignore_ascii_case("pengajian")
}

#[cfg(test)]
mod tests {
    use super::category_allows_recording;

    #[test]
    fn hanya_pengajian_boleh_rekam() {
        assert!(category_allows_recording("Pengajian"));
        assert!(category_allows_recording("  pengajian  "));
        assert!(category_allows_recording("PENGAJIAN"));
        assert!(!category_allows_recording("Sholat"));
        assert!(!category_allows_recording("Tahfidz"));
        assert!(!category_allows_recording(""));
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleInfo {
    pub title: String,
    pub class_name: String,
    /// "Hari ini, 04:30 WIB" / "Besok, 04:30 WIB"
    pub time_label: String,
}

/// Satu sesi kelas (halaman /sesi).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionItem {
    pub id: i64,
    pub title: String,
    pub class_name: String,
    /// "Hari ini" / "Besok" / "16 Jul 2026"
    pub date_label: String,
    /// "04:30 WIB" / "-"
    pub time_label: String,
    /// Terjadwal | Berlangsung | Selesai | Dibatalkan
    pub status_label: String,
    /// scheduled|ongoing|finished|cancelled ("cancelled" = libur)
    pub status_kind: String,
    pub teacher: String,
    /// Pengajar terpasang (untuk pre-select dropdown assign). None = belum diisi.
    pub teacher_id: Option<i64>,
    /// Pamong bertugas verifikasi sesi (migrasi 33; pre-select dropdown). None =
    /// pakai pamong kelas.
    pub pamong_id: Option<i64>,
    /// Kategori kelas (chip tampilan; "-" bila kosong).
    pub category: String,
}

/// Payload halaman /sesi (nav dipilih dari role).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionsData {
    pub role: String,
    /// true = melihat SEMUA sesi (admin/pamong/dewan guru — guru & dewan guru
    /// SATU entitas, tidak dibedakan).
    pub all_scope: bool,
    /// KANAN: sesi yang BELUM lewat (hari ini ke depan), urut tanggal DESC.
    pub upcoming: Vec<SessionItem>,
    /// KIRI: sesi yang SUDAH lewat, maksimal 7 hari ke belakang, urut DESC.
    pub past: Vec<SessionItem>,
}

/// Satu baris absensi pada detail sesi (anggota kelas + status di sesi itu).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionAttRow {
    pub user_id: i64,
    pub name: String,
    pub nis: String,
    /// "HADIR"/"TERLAMBAT"/"ALPA"/… atau "BELUM TERCATAT"
    pub status_label: String,
    /// present|late|absent|permit|sick|none
    pub status_kind: String,
    /// "05:02 WIB" bila tercatat
    pub time_label: String,
    /// Id baris absensi — untuk tombol koreksi. None = belum ada catatan, jadi
    /// yang berlaku tombol "tandai hadir", bukan koreksi.
    pub att_id: Option<i64>,
}

/// Satu pesan chat sesi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionChatItem {
    pub name: String,
    pub message: String,
    pub time_label: String,
}

/// Payload halaman detail sesi /sesi/:id (staf).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionDetailData {
    /// Boleh mengoreksi status absensi sesi ini? TRUE hanya bila pemirsa adalah
    /// GURU PENGISI atau PAMONG bertugas sesi ini — cerminan persis penjagaan
    /// di repository::correct_attendance. Dihitung server supaya UI tak pernah
    /// menawarkan tombol yang pasti ditolak.
    pub can_correct: bool,
    pub id: i64,
    pub class_id: i64,
    pub title: String,
    pub class_name: String,
    pub date_label: String,
    pub time_label: String,
    pub status_label: String,
    pub status_kind: String,
    pub teacher: String,
    pub hadir: i64,
    pub total: i64,
    pub attendance: Vec<SessionAttRow>,
    pub chats: Vec<SessionChatItem>,
    /// URL/path rekaman bila sudah ada (kolom class_sessions.recording_path).
    pub recording_url: Option<String>,
    pub recording_label: String,
    /// Pengajar saat ini (None = belum ditentukan) + pilihan pengajar —
    /// dewan guru bisa ganti pengajar langsung dari halaman detail sesi.
    pub teacher_id: Option<i64>,
    pub teacher_options: Vec<super::kelas::TeacherOption>,
    /// Pamong bertugas verifikasi sesi (migrasi 33) — None = pakai pamong kelas.
    /// pamong_options = daftar pamong (role supervisor) utk dropdown.
    pub pamong_id: Option<i64>,
    pub pamong_options: Vec<super::kelas::TeacherOption>,
    pub category: String,
    /// Materi AKTUAL sesi ini (migrasi 20) — buku + halaman yang benar-benar
    /// dibahas. None = belum dipilih.
    pub book_id: Option<i64>,
    pub book_title: Option<String>,
    /// "11-20, 45-50" siap tampil DAN siap prefill form edit.
    pub book_pages_label: String,
    pub book_options: Vec<super::books::BookItem>,
    /// Materi TARGET/rencana sesi ini (migrasi 41) — None = belum diset.
    pub target_book_id: Option<i64>,
    pub target_book_title: Option<String>,
    pub target_pages_label: String,
    /// Catatan bebas ayat/hadith yang BENAR-BENAR dibahas (migrasi 41).
    pub actual_detail: String,
    /// Alasan tombol "Mulai Sesi" nonaktif (di luar jendela ±10 menit dari
    /// jadwal), None = boleh mulai. Dihitung server-side (satu sumber
    /// kebenaran — WIB "now" hanya diketahui server).
    pub start_blocked_reason: Option<String>,
}

/// Payload ruang sesi live /sesi/:id/live (staf + santri peserta).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionLiveData {
    pub id: i64,
    pub title: String,
    pub class_name: String,
    pub teacher: String,
    /// scheduled|ongoing|finished|cancelled
    pub status_kind: String,
    /// true = boleh mulai/akhiri sesi (staf).
    pub can_manage: bool,
    pub chats: Vec<SessionChatItem>,
    /// Jumlah peserta kelas (indikator "128" di header).
    pub member_count: i64,
    /// URL unduh rekaman — terisi HANYA saat sesi selesai & rekaman ada.
    pub recording_url: Option<String>,
    /// true = kategori kelas mengizinkan siaran suara (lihat
    /// category_allows_recording) — AudioDock disembunyikan bila false.
    pub can_record: bool,
    /// Alasan tombol "Mulai Sesi Live" nonaktif (di luar jendela ±10 menit),
    /// None = boleh mulai.
    pub start_blocked_reason: Option<String>,
}

/// Satu baris koreksi absensi dalam permintaan MASSAL (satu sesi, banyak santri).
///
/// Dikirim sekali jalan alih-alih satu request per tombol: petugas lazim
/// membetulkan beberapa santri sekaligus, dan tiap klik yang menembak API
/// sendiri membuat layar berkedip berkali-kali serta menyisakan keadaan
/// setengah tersimpan bila salah satunya gagal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KoreksiAbsensi {
    pub user_id: i64,
    /// present|late|absent|permit|sick. Untuk `present`/`late`, isi `jam` akan
    /// MENENTUKAN sendiri mana yang berlaku (lihat service).
    pub status: String,
    /// Jam masuk "HH:MM" WIB. Kosong = tak diubah/tak diketahui.
    #[serde(default)]
    pub jam: String,
}
