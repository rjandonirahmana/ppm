//! tests/prd_rules.rs — Uji SELURUH logika bisnis murni "Sistem Poin 2.0" PRD.
//! Fungsi-fungsi ini deterministik (tanpa DB/jaringan) → inti kebenaran aplikasi:
//! nilai poin per kategori, izin mengurangi, reward mingguan, prestasi, tier
//! pemanggilan, SP, dan rute tahap izin. Jalankan: `cargo test --test prd_rules`.

use ppm::models::{
    attendance_note, pemanggilan_tier, permit_kind_label, permit_stage, point_rule, prestasi_label,
    sp_level, weekly_reward_points,
};

// CATATAN: uji `category_points` dan `attendance_delta` DIHAPUS bersama kedua
// fungsinya. Nilai poin per kategori kini hanya hidup di SQL — fungsi
// `cat_default_points()` (migrasi 28) yang dipanggil `DELTA_SQL` di
// repository::attendance — supaya aturannya tak punya dua salinan di dua
// bahasa. Berkas ini sempat gagal DIKOMPILASI berbulan-bulan karena masih
// mengimpornya, dan `cargo test` melaporkannya sebagai galat build satu target
// sementara target lain tetap hijau, jadi kegagalannya tak terbaca.
//
// Yang tersisa di sini tetap murni Rust: fallback poin, reward, prestasi, tier
// pemanggilan, SP, dan rute tahap izin.

#[test]
fn attendance_note_sesuai_status() {
    assert_eq!(attendance_note("present"), "Kedisiplinan");
    assert_eq!(attendance_note("late"), "Kedisiplinan");
    assert_eq!(attendance_note("absent"), "Pelanggaran");
    assert_eq!(attendance_note("permit"), "Izin");
    // Sakit/Cuti bersurat sah — dicatat terpisah, tak dihitung pelanggaran.
    assert_eq!(attendance_note("sick"), "Sakit/Cuti");
    assert_eq!(attendance_note("outside_schedule"), "Di luar jadwal");
    assert_eq!(attendance_note("entah"), "Keterangan");
}

#[test]
fn point_rule_fallback_tanpa_konteks() {
    assert_eq!(point_rule("present").0, 10);
    assert_eq!(point_rule("permit").0, 0);
    assert_eq!(point_rule("sick").0, 0);
    assert_eq!(point_rule("outside_schedule").0, 0);
    assert!(point_rule("absent").0 < 0);
}

// ── Reward mingguan (PRD hal. 8) ─────────────────────────────────────────────

#[test]
fn reward_points_per_kategori() {
    assert_eq!(weekly_reward_points("kbm"), (5, 13, 20));
    assert_eq!(weekly_reward_points("non_kbm"), (3, 8, 12));
    assert_eq!(weekly_reward_points("piket"), (2, 0, 0));
    assert_eq!(weekly_reward_points("other"), (0, 0, 0));
}

// ── Prestasi Ketertiban dari sisa saldo (PRD hal. 11) ────────────────────────

#[test]
fn prestasi_sesuai_ambang() {
    assert_eq!(prestasi_label(1051).0, "Istimewa");
    assert_eq!(prestasi_label(1050).0, "Sangat Baik");
    assert_eq!(prestasi_label(750).0, "Sangat Baik");
    assert_eq!(prestasi_label(749).0, "Baik");
    assert_eq!(prestasi_label(451).0, "Baik");
    assert_eq!(prestasi_label(450).0, "Cukup");
    assert_eq!(prestasi_label(151).0, "Cukup");
    assert_eq!(prestasi_label(150).0, "Kurang");
    assert_eq!(prestasi_label(101).0, "Kurang");
    assert_eq!(prestasi_label(100).0, "Sangat Kurang");
    assert_eq!(prestasi_label(1).0, "Sangat Kurang");
    assert_eq!(prestasi_label(0).0, "Habis");
    assert_eq!(prestasi_label(-50).0, "Habis");
}

// ── Pemanggilan mingguan tier (PRD hal. 12) ──────────────────────────────────

#[test]
fn pemanggilan_tier_sesuai_net() {
    assert_eq!(pemanggilan_tier(-9).0, "KoorSantri");
    assert_eq!(pemanggilan_tier(-11).0, "KoorSantri");
    assert_eq!(pemanggilan_tier(-12).0, "Pamong");
    assert_eq!(pemanggilan_tier(-17).0, "Pamong");
    assert_eq!(pemanggilan_tier(-18).0, "Wali Kelas");
    assert_eq!(pemanggilan_tier(-100).0, "Wali Kelas");
}

// ── Sistem SP dari saldo (PRD hal. 14) ───────────────────────────────────────

#[test]
fn sp_level_sesuai_ambang() {
    assert!(sp_level(151).is_none());
    assert!(sp_level(200).is_none());
    assert_eq!(sp_level(150).unwrap().0, "SP 1");
    assert_eq!(sp_level(101).unwrap().0, "SP 1");
    assert_eq!(sp_level(100).unwrap().0, "SP 2");
    assert_eq!(sp_level(51).unwrap().0, "SP 2");
    assert_eq!(sp_level(50).unwrap().0, "SP 3");
    assert_eq!(sp_level(0).unwrap().0, "SP 3");
    assert_eq!(sp_level(-10).unwrap().0, "SP 3");
}

// ── Label jenis izin ─────────────────────────────────────────────────────────

#[test]
fn kind_label_izin() {
    assert_eq!(permit_kind_label("sick"), "Izin Sakit");
    assert_eq!(permit_kind_label("leave"), "Izin Pulang");
    assert_eq!(permit_kind_label("keperluan"), "Keperluan");
    assert_eq!(permit_kind_label("xxx"), "Izin Lainnya");
}

// ── Rute tahap izin (permit_stage) — per-kelas require_pamong ─────────────────
//
// Migrasi 46: tahap ORANG TUA dihapus. Satu pengajuan izin dipecah jadi
// beberapa baris (satu per wali kelas yang kelasnya dilewati); tiap baris
// jalan sendiri: pamong kelas (bila require_pamong) → wali kelas (FINAL).

fn stage_kind(pamong: &str, guru: &str, req: bool) -> &'static str {
    permit_stage(pamong, guru, req).1
}

#[test]
fn permit_stage_final_guru_didahulukan() {
    // Keputusan final wali kelas terminal walau rute berubah.
    assert_eq!(stage_kind("approved", "approved", true), "approved");
    assert_eq!(stage_kind("pending", "approved", false), "approved");
    assert_eq!(stage_kind("approved", "rejected", true), "rejected");
    // Guru sudah memutus → pamong yang masih pending tak lagi relevan.
    assert_eq!(stage_kind("pending", "approved", true), "approved");
}

#[test]
fn permit_stage_dua_tahap_vs_langsung() {
    // require_pamong=true: menunggu pamong dulu.
    assert_eq!(stage_kind("pending", "pending", true), "pending_pamong");
    assert_eq!(stage_kind("rejected", "pending", true), "rejected");
    // pamong sudah approve → menunggu wali kelas.
    assert_eq!(stage_kind("approved", "pending", true), "pending_guru");

    // require_pamong=false: langsung menunggu wali kelas (abaikan pamong).
    assert_eq!(stage_kind("pending", "pending", false), "pending_guru");
    assert_eq!(stage_kind("rejected", "pending", false), "pending_guru");
}

// ── Pemecahan izin per WALI KELAS (migrasi 46) ────────────────────────────────
//
// Logika pengelompokan di `service::permits::split_permit_per_wali` tak bisa
// diuji langsung tanpa DB (butuh pool), jadi yang diuji di sini adalah
// KONTRAK-nya: pengelompokan per wali kelas unik dengan urutan stabil.
// Bila logika grouping di service berubah, uji ini harus ikut berubah.

/// Tiruan minimal `repository::AffectedClass` untuk menguji grouping.
struct Kelas {
    id: i64,
    nama: &'static str,
    wali: Option<i64>,
}

/// Replika grouping `split_permit_per_wali`: kelompokkan per wali kelas,
/// pertahankan urutan kemunculan pertama. Return (wali, class_id acuan, nama kelas).
fn group_per_wali(kelas: &[Kelas]) -> Vec<(Option<i64>, i64, Vec<&'static str>)> {
    use std::collections::HashMap;
    let mut order: Vec<Option<i64>> = Vec::new();
    let mut groups: HashMap<Option<i64>, Vec<&Kelas>> = HashMap::new();
    for k in kelas {
        let e = groups.entry(k.wali).or_default();
        if e.is_empty() {
            order.push(k.wali);
        }
        e.push(k);
    }
    order
        .into_iter()
        .map(|w| {
            let g = &groups[&w];
            (w, g[0].id, g.iter().map(|k| k.nama).collect())
        })
        .collect()
}

#[test]
fn izin_dua_kelas_wali_berbeda_jadi_dua_permit() {
    // Kasus inti: izin melewati 2 kelas dengan wali BERBEDA → 2 baris izin.
    let kelas = [
        Kelas { id: 1, nama: "Fiqih", wali: Some(10) },
        Kelas { id: 2, nama: "Nahwu", wali: Some(20) },
    ];
    let g = group_per_wali(&kelas);
    assert_eq!(g.len(), 2, "wali berbeda harus jadi permit terpisah");
    assert_eq!(g[0], (Some(10), 1, vec!["Fiqih"]));
    assert_eq!(g[1], (Some(20), 2, vec!["Nahwu"]));
}

#[test]
fn izin_beberapa_kelas_wali_sama_jadi_satu_permit() {
    // Wali yang sama tak perlu dimintai persetujuan dua kali — kelasnya digabung.
    let kelas = [
        Kelas { id: 1, nama: "Fiqih", wali: Some(10) },
        Kelas { id: 2, nama: "Tauhid", wali: Some(10) },
        Kelas { id: 3, nama: "Nahwu", wali: Some(20) },
    ];
    let g = group_per_wali(&kelas);
    assert_eq!(g.len(), 2, "wali sama digabung jadi satu permit");
    assert_eq!(g[0], (Some(10), 1, vec!["Fiqih", "Tauhid"]));
    assert_eq!(g[1], (Some(20), 3, vec!["Nahwu"]));
}

#[test]
fn izin_satu_kelas_saja_jadi_satu_permit() {
    // Hanya melewati 1 wali kelas → cukup 1 permit (tak ada pemecahan).
    let kelas = [Kelas { id: 7, nama: "Fiqih", wali: Some(10) }];
    let g = group_per_wali(&kelas);
    assert_eq!(g.len(), 1);
    assert_eq!(g[0], (Some(10), 7, vec!["Fiqih"]));
}

#[test]
fn izin_kelas_tanpa_wali_tetap_dapat_permit() {
    // Kelas tanpa wali kelas (wali NULL) tetap jadi satu grup — diputus
    // dewan guru/admin lewat oversight, jangan hilang diam-diam.
    let kelas = [
        Kelas { id: 1, nama: "Fiqih", wali: Some(10) },
        Kelas { id: 2, nama: "Ekstra", wali: None },
    ];
    let g = group_per_wali(&kelas);
    assert_eq!(g.len(), 2);
    assert_eq!(g[1], (None, 2, vec!["Ekstra"]));
}
