//! models/kelas.rs — Payload sisi STAF: dashboard, manajemen kelas, tinjau izin.

use serde::{Deserialize, Serialize};

/// Sesi live di dashboard staf.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveSesi {
    pub id: i64,
    pub title: String,
    pub teacher: String,
    pub santri_count: i64,
    /// live|upcoming|break — status tampilan kartu sesi.
    pub state: String,
    pub time_label: String,
    /// Jam sesi sudah lewat menurut WIB. Dipakai kartu "Jadwal Berikutnya"
    /// untuk berhenti menawarkan sesi yang sebenarnya sudah usai — `state`
    /// tak bisa dipakai karena sesi tetap `scheduled` sampai akhir hari.
    #[serde(default)]
    pub past: bool,
}

/// Satu baris tabel "Kehadiran Terbaru" di dashboard staf.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LatestAtt {
    pub name: String,
    pub initial: String,
    pub class_name: String,
    pub time_label: String,
    /// "HADIR" | "TERLAMBAT" | "IZIN" | "ALPA"
    pub status_label: String,
    /// present|late|permit|sick|absent → warna badge.
    pub kind: String,
}

/// Dashboard staf (admin/dewan guru/pamong).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StafHome {
    pub name: String,
    pub total_santri: i64,
    pub santri_growth_month: i64,
    pub hadir_today: i64,
    /// Persentase kehadiran hari ini.
    pub pct: i32,
    pub izin_pending: i64,
    pub live: Vec<LiveSesi>,
    pub latest: Vec<LatestAtt>,
}

// ── Guru / Dewan Guru — analisis kelas ──────────────────────────────────────────

/// Satu baris ranking kelas (dipakai guru.html & dewan_guru.html).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassRank {
    pub name: String,
    /// Persentase kehadiran kelas (semester berjalan).
    pub attendance_pct: i32,
    /// Rata-rata poin santri di kelas ini.
    pub avg_points: i32,
    pub santri_count: i64,
}

/// Satu baris "Insight/Laporan Kinerja Pengajar".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeacherInsight {
    pub name: String,
    pub sessions_count: i64,
    pub attendance_pct: i32,
}

/// Titik data tren kehadiran (per hari, 7 hari terakhir).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrendPoint {
    /// Label singkat: "Sen", "Sel", ...
    pub label: String,
    pub pct: i32,
}

/// Payload dashboard analisis (/guru untuk guru, /dewan-guru untuk dewan guru —
/// `is_dewan=true` memperluas cakupan dari "kelas milik guru ybs" → SEMUA kelas).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalisisData {
    pub name: String,
    pub is_dewan: bool,
    pub attendance_pct: i32,
    pub avg_points: i32,
    pub sessions_verified: i64,
    pub trend: Vec<TrendPoint>,
    pub class_ranking: Vec<ClassRank>,
    pub teacher_insight: Vec<TeacherInsight>,
    /// Sesi hari ini (hero "Jadwal Berikutnya" + daftar).
    #[serde(default)]
    pub today: Vec<LiveSesi>,
}

// ── Poin santri ─────────────────────────────────────────────────────────────────

/// Satu baris papan poin santri.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointRow {
    pub user_id: i64,
    pub name: String,
    pub nis: Option<String>,
    pub class_name: Option<String>,
    pub points: i32,
    pub initial: String,
}

/// Payload halaman Pantauan Poin (/poin staf, /poin-dewan dewan guru).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoinData {
    /// true → boleh menambah/mengurangi poin manual (dewan guru/admin).
    pub can_adjust: bool,
    pub avg_points: i32,
    pub total_santri: i64,
    pub top: Vec<PointRow>,
}

/// Satu entri riwayat poin (point_logs) — dipakai di kartu detail santri.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointLogItem {
    pub delta: i32,
    pub reason: String,
    pub category: String,
    pub when_label: String,
}

/// Satu kelas di halaman Manajemen Kelas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KelasItem {
    pub id: i64,
    pub name: String,
    pub description: String,
    /// Kategori kelas (teks bebas: Cepatan/Lambatan/…). Kosong = belum diisi.
    pub category: String,
    /// Golongan (migrasi 16): sumbu klasifikasi TERPISAH dari category —
    /// "Bacaan" (Lambatan/Cepatan) atau "Makna" (Hadist Besar/…). Kosong =
    /// kelas di luar sistem dua-sumbu ini.
    pub golongan: String,
    /// Pengajar sesi terakhir kelas ini (kolom teacher tak ada di classes).
    pub teacher: String,
    pub member_count: i64,
    pub schedule_count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KelasData {
    /// Peran pengguna (untuk memilih nav bawah).
    pub role: String,
    /// Boleh membuat/menata kelas? Hanya admin/ketua.
    pub can_manage: bool,
    pub total_kelas: i64,
    pub total_santri: i64,
    pub items: Vec<KelasItem>,
}

/// Anggota kelas (LIHAT SANTRI).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberItem {
    pub id: i64,
    pub name: String,
    pub nis: String,
    /// Angkatan (4 digit awal NIS, mis. "2023"). Kosong bila NIS tak berpola.
    pub angkatan: String,
}

/// Satu jadwal kelas (class_schedules) di halaman detail kelas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleItem {
    pub id: i64,
    pub title: String,
    /// "04:30 – 05:30 WIB"
    pub time_label: String,
    /// "Harian" / "Mingguan" / ...
    pub recurrence_label: String,
    /// "Mulai 16 Jul 2026"
    pub date_label: String,
    /// recurrence mentah (daily/weekly/monthly/once) untuk form edit.
    pub recurrence: String,
    /// "05:30" & "07:00" untuk mengisi input <time> saat edit.
    pub start_hm: String,
    pub end_hm: String,
    /// Batas terlambat "HH:MM" untuk form edit.
    pub limit_hm: String,
    /// Tanggal mentah ISO "YYYY-MM-DD" (start) + end (kosong bila NULL) utk edit.
    pub start_date: String,
    pub end_date: String,
    /// Durasi menit (untuk statistik "Durasi Rata-rata").
    pub duration_min: i64,
    /// Kategori jadwal (mis. "Pengajian"/"Sholat") — kosong bila belum diisi;
    /// override kategori kelas utk sesi lahir dari jadwal ini (migrasi 10).
    pub category: String,
    /// Poin BONUS saat TEPAT WAKTU (present) di jadwal ini (migrasi 21). ""
    /// = default (+10); DITAMBAHKAN ke poin santri. Magnitude positif.
    pub present_points: String,
    /// Poin DIPOTONG saat TERLAMBAT (migrasi 13, semantik disederhanakan migrasi
    /// 21). "" = default (0 = tak dipotong); DIKURANGKAN. Magnitude positif.
    pub late_points: String,
    /// Poin DIPOTONG saat ALPA (absent) (migrasi 15). "" = default (15);
    /// DIKURANGKAN. Magnitude positif. Semua poin kini positif & konsisten:
    /// present ditambah, late/absent dikurangi (tak ada nilai minus di UI/DB).
    pub absent_points: String,
    /// Ruang = perangkat RFID (migrasi 24). 0 = belum diset; >0 = rfid_devices.id.
    /// room_label = nama perangkat utk tampilan.
    pub room_id: i64,
    pub room_label: String,
    /// Tanggal manual (migrasi 23) utk recurrence 'custom', ISO dipisah koma
    /// ("2026-07-24,2026-08-01") — prefill picker tanggal di form edit.
    pub custom_dates: String,
    /// Jenis kegiatan PRD (migrasi 28): kbm|non_kbm|piket|apel_kepulangan; ""
    /// = legacy. Menentukan preset poin default — presetnya ada di fungsi SQL
    /// `cat_default_points()` (migrasi 28), satu-satunya sumber angka poin.
    pub activity_type: String,
    /// Poin DIPOTONG saat IZIN biasa (migrasi 28). "" = preset kategori.
    pub izin_points: String,
    /// Materi yang sedang DIBAHAS jadwal rutin ini (migrasi 57). 0 = belum
    /// diset. Hanya "materi apa" — "sampai mana" milik baris kurikulum
    /// (migrasi 59), tempat rentangnya berada.
    pub current_book_id: i64,
    pub current_book_title: String,
    /// "quran" | "hadist" | "" — menentukan bentuk posisinya.
    pub current_book_category: String,
    /// Posisi milik JADWAL INI: hadist → halaman; quran → ayat `current_unit`
    /// pada surat ke-`current_surah` (indeks 1-based ke daftar surat materi).
    /// 0 = belum diisi.
    ///
    /// Sengaja terpisah dari posisi di kurikulum: yang ini menjawab "jadwal ini
    /// sampai mana", sedangkan kurikulum menjawab "kelas ini sampai mana atas
    /// materi itu" — dan itulah yang jadi dasar persen progres.
    pub current_surah: i32,
    pub current_unit: i32,
    /// Posisi jadwal ini siap-tampil, mis. "Al Baqarah ayat 20". Kosong bila
    /// belum diisi.
    pub current_label: String,
}

/// Opsi jadwal (dropdown tambah santri / buat sesi).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleOption {
    pub id: i64,
    pub label: String,
}

/// Opsi pengajar (dropdown buat sesi).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeacherOption {
    pub id: i64,
    pub name: String,
}

/// Opsi ruang = perangkat RFID (migrasi 24, dropdown jadwal). Hanya id+nama
/// (tanpa api_key).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoomOption {
    pub id: i64,
    pub name: String,
}

/// Payload halaman detail kelas (/kelas/:id).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KelasDetail {
    pub role: String,
    pub id: i64,
    pub name: String,
    pub description: String,
    pub category: String,
    /// Kategori yang sudah pernah dipakai (untuk dropdown + boleh ketik baru).
    pub category_options: Vec<String>,
    /// Golongan kelas ini (migrasi 16: "Bacaan"/"Makna"/…, kosong = tak diisi).
    pub golongan: String,
    /// Golongan yang sudah pernah dipakai (untuk dropdown + boleh ketik baru).
    pub golongan_options: Vec<String>,
    /// Wali kelas (migrasi 29): guru penyetuju FINAL izin santri kelas ini.
    /// 0 = belum diset; wali_kelas_name utk tampilan.
    pub wali_kelas_id: i64,
    pub wali_kelas_name: String,
    /// TRUE = izin santri kelas ini lewat Pamong dulu; FALSE = langsung wali kelas.
    pub require_pamong: bool,
    /// Mode verifikasi absensi kelas (migrasi 62):
    /// "dua_tahap" | "guru" | "pamong".
    pub verify_mode: String,
    /// Pemirsa boleh MENATA kelas ini (buat/ubah, wali & pamong, anggota,
    /// jadwal)? Hanya admin/ketua. Dihitung server supaya UI mengunci sendiri
    /// alih-alih menawarkan tombol yang pasti ditolak.
    pub can_manage: bool,
    /// Pamong kelas (migrasi 30): verifikasi kehadiran + tahap-1 izin + terima WA
    /// pengingat sesi. 0 = belum diset.
    pub pamong_id: i64,
    pub pamong_name: String,
    /// Opsi pamong (role supervisor) untuk dropdown.
    pub pamong_options: Vec<TeacherOption>,
    pub members: Vec<MemberItem>,
    pub schedules: Vec<ScheduleItem>,
    pub schedule_options: Vec<ScheduleOption>,
    pub teacher_options: Vec<TeacherOption>,
    /// Opsi ruang = perangkat RFID (migrasi 24, dropdown "Ruang" saat buat jadwal).
    pub room_options: Vec<RoomOption>,
    /// Daftar buku aktif (migrasi 20, dropdown "materi buku" saat buat sesi).
    pub book_options: Vec<super::books::BookItem>,
    pub sessions: Vec<super::schedule::SessionItem>,
    /// Statistik jadwal (untuk kartu "Jadwal Kelas").
    pub weekly_sessions: i64,
    pub avg_duration_min: i64,
    /// Cakupan materi/kitab kelas ini (migrasi 17, tab "Kurikulum").
    pub curriculum: Vec<CurriculumItem>,
}

/// Satu materi/kitab dalam cakupan kurikulum kelas (migrasi 17). Progres
/// keseluruhan KELAS pada materi ini — beda dari hafalan_logs (progres
/// HAFALAN per-santri).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurriculumItem {
    pub id: i64,
    /// Judul materi — IKUT dari `books` lewat `book_id`, bukan diketik di
    /// kurikulum. Baris lama yang belum tertaut memakai judul lamanya sendiri.
    pub title: String,
    pub progress_pct: i16,
    pub order_index: i16,
    /// "active" | "completed" | "upcoming" mentah (untuk form edit).
    pub status: String,
    /// "Berjalan" / "Selesai" / "Akan Datang".
    pub status_label: String,
    /// Tautan materi terdaftar (migrasi 22). 0 = tak tertaut (materi bebas-teks);
    /// >0 = id `books`. `book_title` untuk tampilan.
    ///
    /// Kurikulum BARU wajib menautkan materi; 0 hanya tersisa pada baris lama
    /// dari sebelum aturan itu — UI menandainya "belum tertaut".
    pub book_id: i64,
    pub book_title: String,
    /// "quran" | "hadist" | "" (bila tak tertaut) — menentukan bentuk rentang.
    pub book_category: String,
    /// Rentang terstruktur (migrasi 57). 0 = belum diisi.
    /// hadist → halaman `start_unit`..`end_unit`, surat diabaikan.
    /// quran  → ayat, dengan `start_surah`/`end_surah` = indeks 1-based ke
    ///          daftar surat materinya (rentang boleh melintasi surat).
    pub start_surah: i32,
    pub start_unit: i32,
    pub end_surah: i32,
    pub end_unit: i32,
    /// Rentang siap-tampil, mis. "Halaman 5–20" / "Al Baqarah 1 – An-Nisa 10".
    /// Kosong bila rentangnya belum diisi.
    pub range_label: String,
    /// Sudah sampai mana (migrasi 59). 0 = belum mulai. Dari SINI progres persen
    /// dan status diturunkan — keduanya tak lagi diisi tangan.
    pub current_surah: i32,
    pub current_unit: i32,
    /// Posisi siap-tampil, mis. "Halaman 42" / "Al Baqarah ayat 120".
    pub current_label: String,
}

/// Satu kelas yang diikuti santri, berlabel golongan (migrasi 16) — santri
/// biasanya punya satu tag per golongan (satu Bacaan + satu Makna).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudentClassTag {
    /// "Bacaan" / "Makna" / kosong (kelas di luar sistem golongan).
    pub golongan: String,
    pub name: String,
}

/// Satu baris santri di halaman Students (daftar + verifikasi).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudentRowItem {
    pub id: i64,
    pub name: String,
    pub nis: String,
    pub angkatan: String,
    /// SEMUA kelas yang diikuti santri (biasanya satu Bacaan + satu Makna).
    pub classes: Vec<StudentClassTag>,
    pub points: i32,
    pub initial: String,
}

/// Payload halaman Students (gabungan daftar santri + antrean verifikasi).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudentsData {
    pub role: String,
    /// "tahap1" (pamong) | "tahap2" (dewan guru) | "none".
    pub verify_stage: String,
    pub students: Vec<StudentRowItem>,
    /// Antrean verifikasi sesuai peran (kosong bila tak ada tahap).
    pub pending: Vec<super::attendance::PendingAtt>,
    pub verified_today: i64,
}

/// Izin menunggu peninjauan pamong/dewan guru (halaman /izin-staf, migrasi 17
/// dua-tahap Orang Tua → Pamong). Hanya muncul di sini setelah
/// `parent_status = 'approved'` (lihat repository/permits.rs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermitReviewItem {
    pub id: i64,
    pub student_name: String,
    pub nis: String,
    pub class_name: String,
    pub kind_label: String,
    pub range_label: String,
    pub reason: String,
    pub when_label: String,
}

/// Payload halaman /izin-staf. Antrean disesuaikan peran peninjau (pamong →
/// tahap 1; guru/dewan guru/admin → tahap final). `stage_label` = nama tahap
/// yang ditinjau; `two_stage` = mode global saat ini.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermitQueueData {
    pub pending_count: i64,
    pub approved_today: i64,
    pub items: Vec<PermitReviewItem>,
    #[serde(default)]
    pub two_stage: bool,
    #[serde(default)]
    pub stage_label: String,
}

// ── Sisi SANTRI: "Kelas Saya" ────────────────────────────────────────────────

/// Satu jadwal kelas dilihat dari sisi santri — ringkas, tanpa pengaturan poin
/// atau tombol kelola yang bukan urusannya.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KelasSayaJadwal {
    pub title: String,
    /// "05:00 – 06:30 WIB"
    pub time_label: String,
    pub recurrence_label: String,
    /// Materi yang sedang dibahas jadwal ini + posisinya. Kosong = belum diatur.
    pub current_book_title: String,
    pub current_label: String,
}

/// Satu kelas dilihat dari sisi ORANG DI DALAMNYA — santri yang mengikutinya,
/// atau staf yang bertugas di sana. Isinya sama (petugas, kurikulum, materi
/// berjalan, daftar santri); yang berbeda hanya cara kelasnya dipilih.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KelasSayaItem {
    pub id: i64,
    pub name: String,
    /// Peran PEMIRSA di kelas ini: "Wali Kelas" / "Pamong" / "Wali Kelas &
    /// Pamong". Kosong untuk santri — ia peserta, bukan petugas.
    pub peran_saya: String,
    pub category: String,
    pub golongan: String,
    /// Kosong = belum ditunjuk.
    pub wali_kelas: String,
    pub pamong: String,
    pub curriculum: Vec<CurriculumItem>,
    pub schedules: Vec<KelasSayaJadwal>,
    pub members: Vec<MemberItem>,
}

/// Payload halaman "Kelas Saya".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KelasSayaData {
    /// true = pemirsa adalah STAF (wali kelas / pamong), bukan santri —
    /// dipakai halaman untuk menyesuaikan kalimatnya.
    pub sebagai_staf: bool,
    pub items: Vec<KelasSayaItem>,
}
