//! models/finance.rs — Tagihan santri (migrasi 37). Shared (SSR + hydrate).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BillItem {
    pub id: i64,
    pub user_id: i64,
    pub student_name: String,
    pub nis: String,
    pub class_name: String,
    pub title: String,
    pub price: i64,
    /// "2026-07-01"
    pub started_date: String,
    pub expired_date: String,
    /// "belum" | "menunggu" | "lunas" | "ditolak" (migrasi 75).
    ///
    /// `menunggu` = diajukan santri/orang tua, periodenya BELUM ditentukan —
    /// pada baris ini `started_date`/`expired_date` kosong, dan layar tak boleh
    /// menampilkannya sebagai rentang yang berlaku.
    pub status: String,
    /// "20 Jul 2026 14:30" atau kosong.
    pub paid_at: String,
    pub paid_amount: Option<i64>,
    pub method: String,
    pub proof_url: String,
    pub verified_by_name: String,
    pub note: String,
    /// Sudah lewat expired & belum lunas.
    pub overdue: bool,
    /// Alasan penolakan yang DIBACA KELUARGA. Kosong bila tak ditolak.
    ///
    /// Kolom sendiri, bukan menumpang `note`: `note` catatan internal pengurus,
    /// dan menggabungkannya berarti catatan internal ikut terbaca santri.
    #[serde(default)]
    pub reject_reason: String,
    /// "09 Agu 2026 14:30" — kapan diajukan. Kosong untuk baris yang dicatat
    /// langsung oleh pengurus (bukan hasil pengajuan).
    #[serde(default)]
    pub submitted_at: String,
    /// Siapa yang mengajukan — santri sendiri atau orang tuanya. Kosong bila
    /// dicatat langsung pengurus.
    #[serde(default)]
    pub submitted_by_name: String,
}

/// Satu santri yang masa berlaku pembayarannya sudah habis (atau belum pernah
/// tercatat sama sekali) — halaman "Periode Terlewat".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TunggakanItem {
    pub user_id: i64,
    pub name: String,
    pub nis: String,
    pub class_name: String,
    /// Tanggal berakhirnya periode terakhir yang lunas ("2026-07-10"), kosong
    /// bila belum pernah ada catatan pembayaran.
    pub habis_tanggal: String,
    /// Berapa hari sejak periode itu habis. 0 bila belum pernah tercatat.
    pub hari_lewat: i64,
    /// Belum pernah ada catatan pembayaran sama sekali.
    pub belum_pernah: bool,
    /// Nomor HP santri (kosong bila belum diisi) — ditampilkan supaya pengurus
    /// tahu lebih dulu bahwa WA-nya akan gagal, bukan setelah menekan tombol.
    pub punya_hp: bool,
    /// Berapa nomor orang tua terhubung yang akan ikut menerima WA.
    pub jumlah_ortu: i64,
    /// "2 hari lalu" — kapan terakhir diingatkan lewat WA. Kosong = belum pernah.
    pub diingatkan: String,
}

/// Payload halaman "Periode Terlewat", dipisah dua kelompok atas permintaan
/// pengurus: yang pernah membayar lalu masa berlakunya habis adalah tagihan
/// NYATA, sedangkan yang belum pernah tercatat sebagian besar adalah 512 santri
/// hasil impor daftar induk — mencampur keduanya membuat kelompok pertama
/// tenggelam dan daftarnya tak bisa dipakai bekerja.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TunggakanData {
    pub terlewat: Vec<TunggakanItem>,
    pub belum_pernah: Vec<TunggakanItem>,
}

/// Rupiah → "Rp1.500.000".
pub fn fmt_rupiah(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    let digits: String = out.chars().rev().collect();
    format!("{}Rp{}", if n < 0 { "-" } else { "" }, digits)
}
