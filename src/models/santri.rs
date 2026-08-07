//! models/santri.rs — Payload halaman santri: riwayat, izin, profil.

use serde::{Deserialize, Serialize};

// ── Riwayat kehadiran ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiwayatItem {
    /// Nama kelas/sesi (fallback: label gerbang).
    pub title: String,
    /// "21 Okt 2025, 20:00 WIB"
    pub time_label: String,
    /// HADIR | TERLAMBAT | IZIN | ALPA
    pub status_label: String,
    /// present|late|permit|absent → warna kartu.
    pub kind: String,
    /// Poin tampilan (aturan models::attendance::point_rule).
    pub points: i32,
    /// "Kedisiplinan" / "Keterangan" / "Pelanggaran"
    pub points_note: String,
    /// Grup bulan, mis. "Oktober 2025".
    pub month: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiwayatData {
    /// Hadir (present+late) semester ini.
    pub hadir: i64,
    /// Izin (permit+sick) semester ini.
    pub izin: i64,
    /// Alpa (absent) semester ini.
    pub alpa: i64,
    /// Label semester, mis. "Semester Ganjil 25/26".
    pub semester_label: String,
    pub items: Vec<RiwayatItem>,
}

/// Rentang hari kalender WIB untuk kartu "Prestasi Terbaru" & "Pelanggaran &
/// Teguran" di rapor santri/orang tua.
///
/// SATU sumber untuk dua sisi: service memakainya menghitung batas query, UI
/// memakainya menulis label. Dulu angkanya ditulis terpisah di label — sekali
/// batas query diubah, label langsung berbohong tanpa ada yang sadar.
pub const RECENT_POINTS_DAYS: i64 = 3;

// ── Izin / perizinan ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermitItem {
    #[serde(default)]
    pub id: i64,
    /// "Diajukan orang tua — Budi" bila bukan santri sendiri; kosong bila
    /// santri mengajukan sendiri (kasus lazim, tak perlu diberi tahu).
    #[serde(default)]
    pub diajukan_oleh: String,
    /// "Izin Sakit" / "Izin Pulang" / "Keperluan"
    pub kind_label: String,
    /// "12 – 13 Nov 2025"
    pub range_label: String,
    /// Kelas tujuan izin ini (migrasi 46). Kosong = izin lama / santri tanpa
    /// kelas terjadwal. Ditampilkan agar santri paham kenapa satu ajuan muncul
    /// beberapa baris: tiap wali kelas memutus sendiri.
    pub class_label: String,
    /// "Menunggu Pamong" / "Menunggu Wali Kelas" / "Disetujui" / "Ditolak…"
    pub status_label: String,
    /// pending_pamong|pending_guru|approved|rejected → warna badge.
    pub status_kind: String,
}

/// "Izin Sakit" / "Izin Pulang" / "Keperluan" / "Izin Lainnya".
pub fn permit_kind_label(kind: &str) -> &'static str {
    match kind {
        "sick" => "Izin Sakit",
        "leave" => "Izin Pulang",
        "keperluan" => "Keperluan",
        _ => "Izin Lainnya",
    }
}

/// Label + kind gabungan multi-tahap untuk satu baris permit_requests — dipakai
/// tampilan santri & orang tua. Alur (migrasi 46): Pamong Kelas (hanya bila
/// kelas `require_pamong`) → Wali Kelas (keputusan FINAL).
///
/// Orang tua TIDAK lagi jadi penyetuju — izin adalah urusan akademik antara
/// santri dan penanggung jawab kelas yang ditinggalkan. Orang tua tetap
/// menerima notifikasi, tapi tak memblokir alur.
///
/// `require_pamong` diturunkan per-permit dari KELAS yang izin ini tujukan
/// (kolom `class_id`), bukan lagi dari kelas utama santri.
pub fn permit_stage(
    pamong_status: &str,
    guru_status: &str,
    require_pamong: bool,
) -> (&'static str, &'static str) {
    // Keputusan final wali kelas (terminal — didahulukan agar aman saat rute berubah).
    match guru_status {
        "approved" => return ("Disetujui", "approved"),
        "rejected" => return ("Ditolak Wali Kelas", "rejected"),
        _ => {}
    }
    // Belum diputus wali kelas.
    if require_pamong {
        match pamong_status {
            "rejected" => ("Ditolak Pamong", "rejected"),
            "pending" => ("Menunggu Pamong", "pending_pamong"),
            _ => ("Menunggu Wali Kelas", "pending_guru"),
        }
    } else {
        ("Menunggu Wali Kelas", "pending_guru")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IzinData {
    /// Persentase kehadiran semester ini.
    pub pct: i32,
    pub hadir: i64,
    pub absen: i64,
    pub points: i32,
    /// "Halaqah Subuh • 05:12 WIB" — scan terakhir hari ini (bila ada).
    pub detected: Option<String>,
    pub permits: Vec<PermitItem>,
}

// ── Pratinjau dampak izin ──────────────────────────────────────────────────────

/// Satu kelas yang akan terlewat bila izin ini jadi diajukan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PratinjauKelas {
    pub nama: String,
    /// "KBM" / "Non-KBM" — santri perlu tahu mana yang kelas utamanya.
    pub kategori: String,
    /// Wali kelas yang akan menerima pengajuan ini. Kosong = belum ditunjuk
    /// (izinnya naik ke dewan guru).
    pub wali: String,
    /// Berapa kali kelas ini berlangsung selama rentang izin.
    pub sesi: i64,
}

/// Dampak izin SEBELUM diajukan — dihitung server dengan aturan yang sama persis
/// dengan yang nanti dipakai saat izin benar-benar dibuat.
///
/// Alasannya bukan sekadar informasi: santri mengisi tanggal dan jam tanpa tahu
/// jadwal mana yang tersentuh, dan izin "sehari" yang ternyata menelan lima
/// kelas berbeda punya arti lain dari yang ia bayangkan.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PratinjauIzin {
    pub kelas: Vec<PratinjauKelas>,
    pub total_sesi: i64,
    /// Berapa wali kelas yang akan menerima pengajuan (satu izin bisa pecah).
    pub total_wali: i64,
}

// ── Profil ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfilData {
    pub name: String,
    pub username: String,
    /// Peran mentah (santri/parent/teacher/dewan_guru/supervisor/admin) — utk memilih nav.
    pub role: String,
    /// Label peran tampilan, mis. "SANTRI".
    pub role_label: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub nis: Option<String>,
    pub points: i32,
    /// Profil mahasiswa (migrasi 26) — diisi santri sendiri.
    pub campus: Option<String>,
    pub major: Option<String>,
    /// "L" | "P" mentah (kosong = belum diisi).
    pub gender: Option<String>,
    /// Tahun masuk kuliah (mis. 2023) — profil mahasiswa (migrasi 39).
    pub entry_year: Option<i16>,
    /// Riwayat IPK per semester (terbaru dulu).
    pub ipk_history: Vec<IpkItem>,
}

/// Satu entri riwayat IPK.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpkItem {
    pub id: i64,
    pub semester: String,
    pub ipk: f64,
}

// ── Detail satu pengajuan izin (dipakai SEMUA peran) ─────────────────────────

/// Detail lengkap satu izin. SATU payload untuk santri, orang tua, wali kelas,
/// dan admin — yang berbeda hanya `can_edit`, dan itu dihitung server.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PermitDetail {
    pub id: i64,
    pub student_name: String,
    pub kind: String,
    pub kind_label: String,
    pub reason: String,
    /// "6 – 13 Agu 2026"
    pub range_label: String,
    /// "09:00 – 11:00 WIB"; kosong = sehari penuh.
    pub jam_label: String,
    /// Nilai mentah untuk mengisi form sunting.
    pub start_date: String,
    pub end_date: String,
    pub jam_mulai: String,
    pub jam_selesai: String,
    pub status_label: String,
    /// pending_pamong|pending_guru|approved|rejected
    pub status_kind: String,
    /// Kelas acuan persetujuan.
    pub class_label: String,
    /// Wali kelas yang memutuskan; kosong = belum ditunjuk (naik ke dewan guru).
    pub wali_name: String,
    /// Kelas terdampak + jumlah sesinya: "kelas lambatan (6 sesi)".
    pub sesi_terlewat: Vec<String>,
    pub total_sesi: i64,
    /// "Diajukan santri sendiri" / "Diajukan orang tua (Budi)" — supaya wali
    /// kelas tahu siapa yang meminta, bukan hanya untuk siapa.
    pub diajukan_oleh: String,
    /// true bila pengajunya ORANG TUA (bukan santri sendiri).
    pub oleh_ortu: bool,
    pub when_label: String,
    /// Boleh diubah oleh pemirsa ini? Hanya santri pemilik & wali kelasnya,
    /// dan hanya selama belum diputus wali kelas.
    pub can_edit: bool,
    /// Alasan tak bisa diubah — ditampilkan agar tombol yang hilang tak
    /// terasa seperti aplikasi rusak.
    pub lock_reason: String,
}
