//! service/enrollment.rs — Pendaftaran kartu RFID ke pengguna.
//!
//! MASALAH yang diselesaikan: `users.rfid_cards` tak punya satu pun jalur
//! pengisian — tak di repository, service, maupun UI. Kartu santri baru
//! praktis tak bisa didaftarkan, jadi mesin absensi tak berguna bagi mereka.
//!
//! ALUR (tanpa mengetik nomor kartu sama sekali — 10 digit terlalu rawan
//! salah ketik):
//!   1. Kartu asing ditempel di mesin mana pun → `record_scan` menolaknya
//!      seperti biasa, TAPI nomornya dititipkan ke Redis `kartu:pending:{n}`
//!      dengan TTL 1 jam.
//!   2. Admin membuka Kontrol Pengguna → daftar kartu yang baru saja ditempel.
//!   3. Admin memilih penggunanya (peran apa pun) → kartu terpasang, titipan
//!      Redis dihapus.
//!
//! Kenapa Redis dan bukan tabel: titipan ini memang berumur pendek — alurnya
//! santri berdiri di depan mesin, admin langsung memasangkan. Kedaluwarsa
//! otomatis berarti tak ada sampah yang perlu dibersihkan, dan kartu asing
//! milik orang lewat (kartu KRL, kartu kantor) hilang sendiri.

use anyhow::Result;
use redis::{aio::ConnectionManager, AsyncCommands};
use serde::{Deserialize, Serialize};

use crate::models::PendingCardItem;
use crate::repository as repo;

/// Umur titipan kartu tak dikenal. Cukup untuk satu sesi pendaftaran; lewat
/// itu santri tinggal menempel ulang.
const PENDING_TTL: u64 = 3600;

/// Batas kartu tertunda yang ditampilkan — pagar terhadap mesin yang ditempeli
/// banyak kartu asing (mis. antrean orang lewat membawa kartu apa pun).
const PENDING_MAX: usize = 50;

fn key(card: i64) -> String {
    format!("kartu:pending:{card}")
}

#[derive(Serialize, Deserialize)]
struct Pending {
    /// Perangkat tempat kartu terakhir ditempel — membantu admin memastikan
    /// ini memang kartu yang barusan, bukan sisa titipan orang lain.
    device: String,
    /// Waktu tempel terakhir (UTC RFC3339).
    at: String,
}

/// Titipkan kartu tak dikenal. Best-effort: Redis mati TIDAK boleh
/// menggagalkan respons ke mesin — mesin cuma perlu tahu kartunya ditolak.
pub async fn remember_unknown_card(redis: &mut ConnectionManager, card: i64, device: &str) {
    let p = Pending {
        device: device.to_string(),
        at: chrono::Utc::now().to_rfc3339(),
    };
    let Ok(json) = serde_json::to_string(&p) else {
        return;
    };
    // set_ex menimpa titipan lama → `at` selalu tempel TERAKHIR, dan TTL
    // ikut diperpanjang. Kartu yang ditempel berulang tetap satu baris.
    let _: Result<(), _> = redis.set_ex(key(card), json, PENDING_TTL).await;
}

/// Daftar kartu tak dikenal yang masih hidup, terbaru dulu.
pub async fn pending_cards(redis: &mut ConnectionManager) -> Result<Vec<PendingCardItem>> {
    // SCAN, bukan KEYS: KEYS memblokir Redis selama seluruh keyspace dipindai.
    let mut it = redis
        .scan_match::<_, String>("kartu:pending:*")
        .await
        .map_err(|e| anyhow::anyhow!("Redis SCAN gagal: {e}"))?;
    let mut keys: Vec<String> = Vec::new();
    while let Some(k) = it.next_item().await {
        // Item gagal (koneksi putus di tengah iterasi) dilewati saja — daftar
        // ini sekadar bantuan visual, bukan sumber kebenaran.
        if let Ok(k) = k {
            keys.push(k);
        }
        if keys.len() >= PENDING_MAX {
            break;
        }
    }
    drop(it);

    let mut out = Vec::with_capacity(keys.len());
    for k in keys {
        let Some(card) = k.rsplit(':').next().and_then(|s| s.parse::<i64>().ok()) else {
            continue;
        };
        let json: Option<String> = redis.get(&k).await.unwrap_or(None);
        let Some(p) = json.and_then(|j| serde_json::from_str::<Pending>(&j).ok()) else {
            continue;
        };
        let at = chrono::DateTime::parse_from_rfc3339(&p.at)
            .map(|d| d.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());
        out.push(PendingCardItem {
            card,
            device: p.device,
            when_label: super::fmt::fmt_when(at),
            sort_key: at.timestamp(),
        });
    }
    out.sort_by(|a, b| b.sort_key.cmp(&a.sort_key));
    Ok(out)
}

/// Pasang kartu ke pengguna, lalu buang titipannya.
///
/// Kartu yang sudah dipakai pengguna LAIN ditolak (kolom `rfid_cards` UNIQUE) —
/// tanpa pesan yang jelas, admin akan mengira sistemnya rusak.
pub async fn assign_card(
    pool: &deadpool_postgres::Pool,
    redis: &mut ConnectionManager,
    user_id: i64,
    card: i64,
    actor_id: i64,
) -> Result<()> {
    if card <= 0 {
        bail_user!("Nomor kartu tidak valid.");
    }
    if let Some((owner_id, owner_name)) = repo::find_user_by_card(pool, card).await? {
        if owner_id != user_id {
            bail_user!("Kartu ini sudah terpasang pada {owner_name}. Lepaskan dulu dari sana.");
        }
    }
    repo::set_rfid_card(pool, user_id, Some(card)).await?;
    let _: Result<(), _> = redis.del(key(card)).await;

    // Jejak di log aktivitas: pemasangan kartu = memberi seseorang kemampuan
    // mencatatkan kehadiran, sepadan dengan ganti peran yang juga dicatat.
    let _ = repo::insert_log(
        pool,
        actor_id,
        Some(user_id),
        "assign_rfid",
        Some(&format!("Kartu {card} dipasang")),
    )
    .await;
    Ok(())
}

/// Lepas kartu dari pengguna (hilang/rusak). Setelah ini kartu lamanya tak
/// dikenali lagi, dan kartu pengganti bisa dipasang lewat alur yang sama.
pub async fn unassign_card(
    pool: &deadpool_postgres::Pool,
    user_id: i64,
    actor_id: i64,
) -> Result<()> {
    repo::set_rfid_card(pool, user_id, None).await?;
    let _ = repo::insert_log(pool, actor_id, Some(user_id), "unassign_rfid", Some("Kartu dilepas"))
        .await;
    Ok(())
}
