//! service/telegram.rs — Notifikasi error ke Telegram (pola sama e-ticketing).
//! Dipakai untuk: (1) error server function / API, (2) kegagalan background task,
//! (3) monitor WAHA (WhatsApp) putus/koneksi. Aktif bila TELEGRAM_BOT_TOKEN &
//! TELEGRAM_ADMIN_CHAT_ID di-set (kalau tidak, semua jadi no-op).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use reqwest::Client;
use serde_json::json;
use tracing::warn;

use super::fmt::wib;

#[derive(Clone)]
pub struct TelegramService {
    bot_token: String,
    pub admin_chat_id: i64,
    http: Client,
}

impl TelegramService {
    pub fn new(bot_token: String, admin_chat_id: i64) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("build reqwest client for Telegram");
        Self { bot_token, admin_chat_id, http }
    }

    /// Kirim teks HTML ke chat_id.
    pub async fn send_message(&self, chat_id: i64, text: &str) -> anyhow::Result<()> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        let res = self
            .http
            .post(&url)
            .json(&json!({ "chat_id": chat_id, "text": text, "parse_mode": "HTML" }))
            .send()
            .await?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            anyhow::bail!("Telegram API error {status}: {body}");
        }
        Ok(())
    }

    /// Kirim SERVER ERROR ALERT ke admin. `status_code = 0` → background task.
    pub async fn send_error_alert(&self, status_code: u16, error_kind: &str, detail: &str) {
        let timestamp = chrono::Utc::now()
            .with_timezone(&wib())
            .format("%Y-%m-%d %H:%M:%S WIB")
            .to_string();
        let safe_detail = truncate_str(&html_escape(detail), 800);
        let status_line = if status_code == 0 {
            "Background Task".to_string()
        } else {
            status_code.to_string()
        };
        let text = format!(
            "🚨 <b>SERVER ERROR ALERT</b> 🚨\n\
             \n\
             📅 <b>Waktu:</b> {timestamp}\n\
             🔧 <b>Service:</b> AFM SMART\n\
             📊 <b>Status Code:</b> {status_line}\n\
             💬 <b>Error Type:</b> {kind}\n\
             ❌ <b>Detail:</b>\n<pre>{safe_detail}</pre>\n\
             \n\
             #ServerError #Alert #Monitoring",
            kind = html_escape(error_kind)
        );
        if let Err(e) = self.send_message(self.admin_chat_id, &text).await {
            warn!("Gagal kirim Telegram error alert: {e}");
        }
    }

    /// Kirim info umum (mis. WAHA pulih) — bukan format alert error.
    pub async fn send_info(&self, text: &str) {
        if let Err(e) = self.send_message(self.admin_chat_id, text).await {
            warn!("Gagal kirim Telegram info: {e}");
        }
    }
}

// ── Notifier global (di-init sekali di main.rs) ──────────────────────────────

static TELEGRAM: OnceLock<TelegramService> = OnceLock::new();
/// Dedup: (kind+detail) → waktu terakhir dikirim, agar tak membanjiri Telegram
/// (mis. brute-force login / DB down berulang). Jendela default 60 detik.
static DEDUP: OnceLock<Mutex<HashMap<u64, Instant>>> = OnceLock::new();
const DEDUP_WINDOW: Duration = Duration::from_secs(60);

/// Panggil sekali di main.rs setelah config di-load (bila token+chat_id ada).
pub fn init_telegram(svc: TelegramService) {
    let _ = TELEGRAM.set(svc);
    let _ = DEDUP.set(Mutex::new(HashMap::new()));
}

/// Ambil service global (untuk task yang mau kirim langsung, mis. health WAHA).
pub fn global() -> Option<&'static TelegramService> {
    TELEGRAM.get()
}

fn dedup_allow(key: u64) -> bool {
    let Some(m) = DEDUP.get() else { return true };
    let mut map = m.lock().unwrap();
    let now = Instant::now();
    // Bersihkan entri kedaluwarsa sesekali (map kecil, murah).
    map.retain(|_, t| now.duration_since(*t) < DEDUP_WINDOW);
    match map.get(&key) {
        Some(t) if now.duration_since(*t) < DEDUP_WINDOW => false,
        _ => {
            map.insert(key, now);
            true
        }
    }
}

fn hash_key(kind: &str, detail: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    kind.hash(&mut h);
    // Hanya prefix detail (pesan panjang/SQL sering beda di ekor) → dedup lebih efektif.
    detail.chars().take(120).collect::<String>().hash(&mut h);
    h.finish()
}

/// Fire-and-forget alert error ke Telegram (no-op bila belum di-init). Dedup
/// 60 detik per (kind, prefix-detail). `status` 0 = background task.
pub fn report_error(status: u16, kind: &'static str, detail: impl Into<String>) {
    let Some(tg) = TELEGRAM.get() else { return };
    let detail = detail.into();
    if !dedup_allow(hash_key(kind, &detail)) {
        return;
    }
    let tg = tg.clone();
    tokio::spawn(async move {
        tg.send_error_alert(status, kind, &detail).await;
    });
}

/// Alert error background task (status 0).
pub fn report_background_error(kind: &'static str, detail: impl Into<String>) {
    report_error(0, kind, detail);
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let taken: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{taken}\n…(truncated)")
    } else {
        taken
    }
}
