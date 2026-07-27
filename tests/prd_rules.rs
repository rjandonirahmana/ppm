//! tests/prd_rules.rs — Uji SELURUH logika bisnis murni "Sistem Poin 2.0" PRD.
//! Fungsi-fungsi ini deterministik (tanpa DB/jaringan) → inti kebenaran aplikasi:
//! nilai poin per kategori, izin mengurangi, reward mingguan, prestasi, tier
//! pemanggilan, SP, dan rute tahap izin. Jalankan: `cargo test --test prd_rules`.

use ppm::models::{
    attendance_delta, category_points, pemanggilan_tier, permit_kind_label, permit_stage,
    point_rule, prestasi_label, sp_level, weekly_reward_points,
};

// ── Nilai poin per kategori kegiatan (PRD hal. 7) ────────────────────────────

#[test]
fn category_points_sesuai_prd() {
    let kbm = category_points("kbm");
    assert_eq!((kbm.present, kbm.telat, kbm.alfa, kbm.izin), (4, 1, 10, 3));

    let non = category_points("non_kbm");
    assert_eq!((non.present, non.telat, non.alfa, non.izin), (3, 1, 5, 2));

    let piket = category_points("piket");
    assert_eq!((piket.present, piket.telat, piket.alfa, piket.izin), (1, 0, 2, 0));

    let apel = category_points("apel_kepulangan");
    assert_eq!((apel.present, apel.telat, apel.alfa, apel.izin), (0, 0, 20, 5));

    // Tak dikenal → legacy default (10/0/15/0).
    let other = category_points("apa_saja");
    assert_eq!((other.present, other.telat, other.alfa, other.izin), (10, 0, 15, 0));
}

// ── attendance_delta: arah + preset + override + sakit/cuti ──────────────────

fn delta(status: &str, atype: &str) -> i32 {
    attendance_delta(status, atype, None, None, None, None).0
}

#[test]
fn delta_preset_per_kategori() {
    // Hadir tepat waktu → BONUS (positif) sesuai preset.
    assert_eq!(delta("present", "kbm"), 4);
    assert_eq!(delta("present", "non_kbm"), 3);
    assert_eq!(delta("present", "piket"), 1);
    assert_eq!(delta("present", "apel_kepulangan"), 0);
    assert_eq!(delta("present", "other"), 10);

    // Telat → DIKURANGI.
    assert_eq!(delta("late", "kbm"), -1);
    assert_eq!(delta("late", "piket"), 0);

    // Alfa → DIKURANGI.
    assert_eq!(delta("absent", "kbm"), -10);
    assert_eq!(delta("absent", "non_kbm"), -5);
    assert_eq!(delta("absent", "apel_kepulangan"), -20);
    assert_eq!(delta("absent", "other"), -15);

    // Izin biasa → DIKURANGI (PRD).
    assert_eq!(delta("permit", "kbm"), -3);
    assert_eq!(delta("permit", "non_kbm"), -2);
    assert_eq!(delta("permit", "apel_kepulangan"), -5);
    assert_eq!(delta("permit", "piket"), 0);
}

#[test]
fn delta_sakit_cuti_dan_luar_jadwal_nol() {
    // Sakit/Cuti TIDAK mengurangi poin (PRD hal. 7 NB).
    assert_eq!(delta("sick", "kbm"), 0);
    assert_eq!(delta("sick", "apel_kepulangan"), 0);
    // Hadir di luar jadwal → netral.
    assert_eq!(delta("outside_schedule", "kbm"), 0);
    // Status tak dikenal → netral.
    assert_eq!(delta("entah", "kbm"), 0);
}

#[test]
fn delta_override_per_jadwal_pakai_magnitudo_positif() {
    // Override present 25 → +25 (abaikan preset kbm 4).
    assert_eq!(attendance_delta("present", "kbm", Some(25), None, None, None).0, 25);
    // Override late 7 (magnitudo) → -7.
    assert_eq!(attendance_delta("late", "kbm", None, Some(7), None, None).0, -7);
    // Override absent 30 → -30.
    assert_eq!(attendance_delta("absent", "kbm", None, None, Some(30), None).0, -30);
    // Override izin 9 → -9.
    assert_eq!(attendance_delta("permit", "kbm", None, None, None, Some(9)).0, -9);
    // Nilai negatif di override tetap dipakai sbagai MAGNITUDO (abs).
    assert_eq!(attendance_delta("absent", "kbm", None, None, Some(-8), None).0, -8);
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

fn stage_kind(parent: &str, pamong: &str, guru: &str, req: bool) -> &'static str {
    permit_stage(parent, pamong, guru, req).1
}

#[test]
fn permit_stage_tahap_orang_tua() {
    // Orang tua menolak / menunggu → terminal di tahap ortu, apa pun tahap lain.
    assert_eq!(stage_kind("rejected", "pending", "pending", true), "rejected");
    assert_eq!(stage_kind("pending", "pending", "pending", true), "pending_parent");
    assert_eq!(stage_kind("pending", "approved", "approved", false), "pending_parent");
}

#[test]
fn permit_stage_final_guru_didahulukan() {
    // Keputusan final wali kelas terminal walau rute berubah.
    assert_eq!(stage_kind("approved", "approved", "approved", true), "approved");
    assert_eq!(stage_kind("approved", "pending", "approved", false), "approved");
    assert_eq!(stage_kind("approved", "approved", "rejected", true), "rejected");
}

#[test]
fn permit_stage_dua_tahap_vs_langsung() {
    // require_pamong=true: menunggu pamong dulu.
    assert_eq!(stage_kind("approved", "pending", "pending", true), "pending_pamong");
    assert_eq!(stage_kind("approved", "rejected", "pending", true), "rejected");
    // pamong sudah approve → menunggu wali kelas.
    assert_eq!(stage_kind("approved", "approved", "pending", true), "pending_guru");

    // require_pamong=false: langsung menunggu wali kelas (abaikan pamong).
    assert_eq!(stage_kind("approved", "pending", "pending", false), "pending_guru");
    assert_eq!(stage_kind("approved", "rejected", "pending", false), "pending_guru");
}
