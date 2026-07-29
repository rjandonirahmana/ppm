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
    /// "belum" | "lunas"
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
