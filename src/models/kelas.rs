//! models/kelas.rs — Payload sisi STAF: dashboard, manajemen kelas, tinjau izin.

use serde::{Deserialize, Serialize};

/// Jenis kelas — TIGA (migrasi 65).
///
/// `kbm` = kelas belajar-mengajar: berjenjang, satu santri hanya boleh satu,
/// dan wali kelasnyalah yang memutuskan izin.
/// `bacaan` = Bacaan Al-Quran: berdiri sendiri, bukan KBM (tak berjenjang, tak
/// terikat aturan satu-kelas) tapi juga bukan kegiatan seperti piket/apel.
/// `non_kbm` = kegiatan lain (piket, apel malam, apel mingguan, sholat,
/// totalan) — santri boleh ikut berapa pun.
pub const KATEGORI_KELAS: &[(&str, &str)] = &[
    ("kbm", "KBM — kelas belajar mengajar"),
    ("bacaan", "Bacaan — Bacaan Al-Quran"),
    ("non_kbm", "Non-KBM — piket, apel, sholat, dll"),
];

/// Jenjang kelas KBM, BERURUTAN — santri naik ke jenjang berikutnya setelah
/// kurikulum jenjangnya tuntas.
///
/// Ada di `models` (bukan `service`) karena dipakai kedua sisi: server saat
/// memvalidasi, dan WASM saat menyusun dropdown. `service` tak dikompilasi
/// untuk WASM.
/// URUTANNYA ADALAH ATURAN, bukan sekadar tata letak dropdown:
/// [`jenjang_berikutnya`] menentukan kenaikan jenjang dari posisi di daftar ini.
/// Menyisipkan di tengah berarti mengubah ke mana santri naik — `dasar` di
/// depan dan `pra_saringan` sebelum `saringan` memang dimaksudkan begitu.
pub const JENJANG: &[(&str, &str)] = &[
    ("dasar", "Dasar"),
    ("lambatan", "Lambatan"),
    ("cepatan", "Cepatan"),
    ("pra_saringan", "Pra Saringan"),
    ("saringan", "Saringan"),
    ("hadist_besar", "Hadist Besar"),
];

/// Label tampilan sebuah kode jenjang; kode tak dikenal → apa adanya.
pub fn jenjang_label(kode: &str) -> String {
    JENJANG
        .iter()
        .find(|(k, _)| *k == kode)
        .map(|(_, l)| l.to_string())
        .unwrap_or_else(|| kode.to_string())
}

/// Label tampilan kategori kelas — dipakai lencana di kartu & detail.
pub fn kategori_label(kode: &str) -> &'static str {
    match kode {
        "kbm" => "KBM",
        "bacaan" => "Bacaan",
        "non_kbm" => "Non-KBM",
        _ => "",
    }
}

/// Kategori sesi SIAP TAMPIL — dipakai di mana pun kategori muncul di layar.
///
/// Kenapa bukan `match` tertutup seperti [`kategori_label`]: kategori efektif
/// sebuah sesi adalah `COALESCE(class_schedules.category, classes.category)`,
/// dan yang pertama TEKS BEBAS (migrasi 10 — "dropdown diisi DISTINCT yang ada
/// + boleh ketik baru"). Hanya `classes.category` yang terkunci CHECK.
///
/// Akibatnya di layar sempat muncul apa adanya: `non_kbm` lengkap dengan garis
/// bawahnya, `piket` dan `apel` huruf kecil. Tapi memaksakan `match` tertutup
/// justru membuang yang diketik admin — "piket habis ngaji" akan berubah jadi
/// "Kegiatan", dan keterangan yang ia tulis sendiri hilang.
///
/// Jadi: kode yang dikenal dipetakan ke ejaan bakunya, sisanya DIRAPIKAN —
/// garis bawah jadi spasi, tiap kata berhuruf besar di depan. Admin tetap
/// melihat tulisannya sendiri, hanya lebih rapi.
pub fn kategori_tampil(kode: &str) -> String {
    match kode.trim() {
        "" | "-" => String::new(),
        // Singkatan & ejaan baku yang tak bisa ditebak dari perapian biasa.
        "kbm" => "KBM".into(),
        "non_kbm" => "Non-KBM".into(),
        "bacaan" => "Bacaan".into(),
        "apel_kepulangan" => "Apel Kepulangan".into(),
        lain => lain
            .split(|c: char| c == '_' || c.is_whitespace())
            .filter(|w| !w.is_empty())
            .map(|w| {
                let mut ch = w.chars();
                // `to_uppercase` per-karakter, bukan slicing byte: kategori bisa
                // saja diketik dengan huruf non-ASCII, dan mengiris di tengah
                // karakter multi-byte membuat String panik.
                match ch.next() {
                    Some(c0) => c0.to_uppercase().collect::<String>() + ch.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// Jenjang sesudah `kode`, atau None bila sudah yang terakhir (Hadist Besar).
pub fn jenjang_berikutnya(kode: &str) -> Option<&'static str> {
    let i = JENJANG.iter().position(|(k, _)| *k == kode)?;
    JENJANG.get(i + 1).map(|(k, _)| *k)
}

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

/// Payload halaman Pantauan Poin (/poin).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoinData {
    /// true → boleh menambah/mengurangi poin manual (dewan guru/admin).
    pub can_adjust: bool,
    /// true → boleh me-reset saldo poin seluruh santri (admin/ketua saja).
    /// Dipisah dari `can_adjust`: menyesuaikan poin SATU santri dan mengembalikan
    /// saldo SELURUH pesantren adalah dua kewenangan yang berbeda jauh.
    pub can_reset: bool,
    pub avg_points: i32,
    pub total_santri: i64,
    /// HALAMAN PERTAMA papan poin; sisanya diambil `poin_page_action` saat digulir.
    pub top: Vec<PointRow>,
}

/// Satu entri riwayat poin (point_logs) — layar detail poin santri.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointLogItem {
    pub delta: i32,
    pub reason: String,
    pub category: String,
    pub when_label: String,
    /// "Ustadz Fulan (Admin)" bila dicatat orang; kosong bila oleh SISTEM
    /// (kehadiran otomatis, reset saldo semester, saldo awal).
    pub by_label: String,
}

/// Payload halaman detail poin satu santri (/poin/:id): profil singkat + buku
/// besar poinnya.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoinDetailData {
    pub user_id: i64,
    pub name: String,
    pub initial: String,
    pub nis: String,
    pub angkatan: String,
    pub phone: String,
    pub points: i32,
    /// Nama kelas yang diikuti (chip di layar).
    pub classes: Vec<String>,
    /// Jumlah poin yang MASUK dan KELUAR sepanjang riwayat — supaya saldo bisa
    /// dibaca sebagai hasil, bukan angka yang muncul begitu saja.
    pub total_plus: i64,
    pub total_minus: i64,
    /// Halaman pertama riwayat; sisanya lewat `poin_history_page_action`.
    pub history: Vec<PointLogItem>,
    pub history_total: i64,
    pub can_adjust: bool,
}

/// Satu kelas di halaman Manajemen Kelas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KelasItem {
    pub id: i64,
    pub name: String,
    pub description: String,
    /// Kategori kelas (teks bebas: Cepatan/Lambatan/…). Kosong = belum diisi.
    pub category: String,
    /// Jenjang (migrasi 16): sumbu klasifikasi TERPISAH dari category —
    /// "Bacaan" (Lambatan/Cepatan) atau "Makna" (Hadist Besar/…). Kosong =
    /// kelas di luar sistem dua-sumbu ini.
    pub jenjang: String,
    /// Pengajar sesi terakhir kelas ini (kolom teacher tak ada di classes).
    pub teacher: String,
    pub member_count: i64,
    pub schedule_count: i64,
    /// Nama wali kelas; kosong = belum ditunjuk. Untuk KBM itu keadaan yang
    /// HARUS diperbaiki (izin santrinya tak punya penyetuju), jadi kartunya
    /// menandainya alih-alih diam.
    #[serde(default)]
    pub wali_kelas: String,
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
    /// Guru yang bisa dipilih jadi wali kelas saat MEMBUAT kelas KBM — wali
    /// wajib sejak awal, jadi pilihannya harus sudah ada di halaman ini.
    #[serde(default)]
    pub teacher_options: Vec<TeacherOption>,
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
    /// Jenjang KBM: lambatan|cepatan|hadist_besar (migrasi 65). Kosong utk
    /// kelas non-KBM.
    pub jenjang: String,
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
    /// Boleh menata JADWAL & ANGGOTA kelas ini? admin/ketua ATAU wali kelasnya.
    /// Sengaja terpisah dari `can_manage`: wali kelas menata susunan kelas
    /// sehari-hari, tapi identitas kelas dan penunjukan wali tetap admin.
    #[serde(default)]
    pub can_manage_jadwal: bool,
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

/// Satu kelas yang diikuti santri, berlabel jenjang (migrasi 16) — santri
/// biasanya punya satu tag per jenjang (satu Bacaan + satu Makna).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudentClassTag {
    /// "Bacaan" / "Makna" / kosong (kelas di luar sistem jenjang).
    pub jenjang: String,
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
    /// Jumlah SEBENARNYA santri aktif di database.
    ///
    /// Dipisah dari `students.len()`: daftarnya dibatasi (lihat
    /// `service::kelas::students_data`), dan sebelumnya halaman memajang
    /// panjang daftar itu sebagai "Total N santri terdaftar" — pada pondok
    /// dengan 500 santri, layar menulis "Total 300" dengan yakin sementara 200
    /// sisanya tak disebut sama sekali.
    #[serde(default)]
    pub total_santri: i64,
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
    /// Sesi yang akan TERLEWAT bila izin ini disetujui: "kelas lambatan (3
    /// sesi)". Wali kelas memutuskan sambil melihat akibatnya, bukan menebak
    /// dari rentang tanggal.
    #[serde(default)]
    pub sesi_terlewat: Vec<String>,
    /// Total sesi terlewat lintas kelas.
    #[serde(default)]
    pub total_sesi: i64,
    /// "09:00 – 11:00 WIB" bila izinnya per jam; kosong = sehari penuh.
    #[serde(default)]
    pub jam_label: String,
    /// Kelas tujuan memakai verifikasi dua langkah (pamong lalu wali kelas).
    #[serde(default)]
    pub dua_tahap: bool,
    /// Tahap pamong sudah disetujui.
    #[serde(default)]
    pub pamong_ok: bool,
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
    /// Peran PEMIRSA di kelas ini: "Wali Kelas", atau kosong untuk santri —
    /// ia peserta, bukan petugas. Sejak migrasi 84 hanya ada satu jabatan.
    pub peran_saya: String,
    pub category: String,
    pub jenjang: String,
    /// Kosong = belum ditunjuk.
    pub wali_kelas: String,
    pub curriculum: Vec<CurriculumItem>,
    pub schedules: Vec<KelasSayaJadwal>,
    pub members: Vec<MemberItem>,
}

/// Payload halaman "Kelas Saya".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KelasSayaData {
    /// true = pemirsa adalah STAF (wali kelas), bukan santri —
    /// dipakai halaman untuk menyesuaikan kalimatnya.
    pub sebagai_staf: bool,
    pub items: Vec<KelasSayaItem>,
}

#[cfg(test)]
mod tests_kelas_kategori {
    use super::*;

    /// Jenjang BERURUTAN — kenaikan santri bersandar pada urutan ini.
    #[test]
    fn jenjang_naik_berurutan() {
        assert_eq!(jenjang_berikutnya("dasar"), Some("lambatan"));
        assert_eq!(jenjang_berikutnya("lambatan"), Some("cepatan"));
        // Migrasi 81 menyisipkan `pra_saringan` DI ANTARA cepatan dan saringan:
        // santri cepatan kini naik ke pra saringan, bukan langsung saringan.
        assert_eq!(jenjang_berikutnya("cepatan"), Some("pra_saringan"));
        assert_eq!(jenjang_berikutnya("pra_saringan"), Some("saringan"));
        assert_eq!(jenjang_berikutnya("saringan"), Some("hadist_besar"));
        // Jenjang terakhir tak punya lanjutan.
        assert_eq!(jenjang_berikutnya("hadist_besar"), None);
        assert_eq!(jenjang_berikutnya("entah"), None);
    }

    #[test]
    fn label_dipajang_bukan_kode() {
        assert_eq!(jenjang_label("hadist_besar"), "Hadist Besar");
        assert_eq!(jenjang_label("saringan"), "Saringan");
        // Kode asing dikembalikan apa adanya — lebih baik daripada kosong.
        assert_eq!(jenjang_label("xyz"), "xyz");
        assert_eq!(kategori_label("kbm"), "KBM");
        assert_eq!(kategori_label("bacaan"), "Bacaan");
        assert_eq!(kategori_label("non_kbm"), "Non-KBM");
        assert_eq!(kategori_label("piket"), "");
    }

    /// Kategori jadwal itu TEKS BEBAS (migrasi 10), jadi yang tak dikenal harus
    /// tetap terbaca — bukan ditelan jadi label generik.
    #[test]
    fn kategori_tampil_merapikan_teks_bebas() {
        assert_eq!(kategori_tampil("kbm"), "KBM");
        assert_eq!(kategori_tampil("non_kbm"), "Non-KBM");
        assert_eq!(kategori_tampil("apel_kepulangan"), "Apel Kepulangan");
        // Yang tak dikenal: garis bawah jadi spasi, tiap kata berkapital.
        assert_eq!(kategori_tampil("piket"), "Piket");
        assert_eq!(kategori_tampil("apel"), "Apel");
        assert_eq!(kategori_tampil("piket habis ngaji"), "Piket Habis Ngaji");
        assert_eq!(kategori_tampil("sholat_berjamaah"), "Sholat Berjamaah");
    }

    /// Nilai kosong tak boleh jadi "-" atau spasi menggantung di layar.
    #[test]
    fn kategori_tampil_kosong_jadi_kosong() {
        assert_eq!(kategori_tampil(""), "");
        assert_eq!(kategori_tampil("-"), "");
        assert_eq!(kategori_tampil("   "), "");
    }

    /// Perapian memakai iterator karakter — mengiris byte akan panik di sini.
    #[test]
    fn kategori_tampil_aman_multibyte() {
        assert_eq!(kategori_tampil("ékstra_kurikuler"), "Ékstra Kurikuler");
    }

    /// Tiga kategori, dan hanya KBM yang berjenjang.
    #[test]
    fn kategori_tepat_tiga() {
        let kode: Vec<&str> = KATEGORI_KELAS.iter().map(|(k, _)| *k).collect();
        assert_eq!(kode, vec!["kbm", "bacaan", "non_kbm"]);
        // Enam jenjang sejak migrasi 81 (dasar & pra_saringan ditambahkan).
        // Angkanya dikunci di sini supaya penambahan jenjang berikutnya SELALU
        // melewati tinjauan urutan — posisinya menentukan kenaikan santri.
        assert_eq!(JENJANG.len(), 6);
    }
}

// ── Analisis kekosongan materi (per kelas, per kitab) ────────────────────────

/// Satu rentang unit (ayat/halaman) yang paling banyak KOSONG di kelas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KekosonganItem {
    /// "Ayat 45 – 60" / "Halaman 12 – 20" / "Ayat 45" bila satu unit.
    pub label: String,
    /// Unit awal & akhir (nomor ayat dalam surat, atau nomor halaman).
    pub start_unit: i32,
    pub end_unit: i32,
    /// Indeks surat (kitab Qur'an); 0 untuk kitab hadist.
    pub surah_idx: i32,
    /// Nama surat — kosong untuk kitab hadist.
    pub surah_name: String,
    /// Santri yang BELUM menyentuh bagian ini sama sekali.
    pub kosong: i64,
    /// Santri yang baru setengah (status 1).
    pub setengah: i64,
    /// Rentang dengan kekosongan TERBANYAK di kitab ini — ditandai server agar
    /// UI tak perlu membandingkan angka sendiri.
    #[serde(default)]
    pub terberat: bool,
}

/// Peta kekosongan satu kitab di satu kelas — dasar guru memilih bagian mana
/// yang paling perlu dibahas berikutnya.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KekosonganData {
    pub book_title: String,
    /// quran | hadist — menentukan kata "ayat" atau "halaman".
    pub category: String,
    /// "Ayat" atau "Halaman" — dipakai UI untuk label & placeholder pencarian.
    #[serde(default)]
    pub satuan: String,
    /// Santri kelas yang ikut dihitung.
    pub total_santri: i64,
    /// Rentang terlemah lebih dulu (kosong terbanyak).
    pub items: Vec<KekosonganItem>,
    /// Unit yang SUDAH tuntas seluruh santri — dipakai UI menyebut kemajuan
    /// alih-alih hanya menampilkan kekurangan.
    pub unit_tuntas: i64,
    pub unit_total: i64,
}
