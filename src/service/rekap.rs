//! service/rekap.rs — Rekap kehadiran mingguan per-santri (laporan kontrol staf).
//! Pekan = Senin–Minggu WIB; `offset` mundur per pekan (0 = pekan ini).

use anyhow::Result;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use deadpool_postgres::Pool;

use super::fmt::wib;
use crate::models::{
    pemanggilan_tier, sp_level, weekly_reward_points, PemanggilanItem, SpItem, WeeklyRecapData,
    WeeklyRecapRow, WeeklyRewardRow,
};
use crate::repository as repo;

/// Senin–Minggu WIB untuk pekan `offset` (0 = pekan berjalan).
fn week_range(offset: i32) -> (NaiveDate, NaiveDate) {
    let today = Utc::now().with_timezone(&wib()).date_naive();
    // Senin pekan ini (weekday: Senin=0 … Minggu=6).
    let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
    let start = monday - Duration::weeks(offset.max(0) as i64);
    (start, start + Duration::days(6))
}

/// Angkatan dari 4 digit awal NIS (mis. "2023001" → "2023").
fn angkatan_of(nis: &str) -> String {
    let digits: String = nis.chars().take(4).collect();
    if digits.len() == 4 && digits.chars().all(|c| c.is_ascii_digit()) {
        digits
    } else {
        "-".to_string()
    }
}

fn fmt_range(start: NaiveDate, end: NaiveDate) -> String {
    const BULAN: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des",
    ];
    let b = |d: NaiveDate| BULAN[(d.month() - 1) as usize];
    if start.month() == end.month() {
        format!("{} – {} {} {}", start.day(), end.day(), b(end), end.year())
    } else {
        format!(
            "{} {} – {} {} {}",
            start.day(),
            b(start),
            end.day(),
            b(end),
            end.year()
        )
    }
}

/// Label tampilan jenis kegiatan (untuk rincian reward).
fn cat_label(activity_type: &str) -> &'static str {
    match activity_type {
        "kbm" => "KBM",
        "non_kbm" => "Non-KBM",
        "piket" => "Piket",
        _ => "Lainnya",
    }
}

/// Hitung reward mingguan per santri dari hitungan per-kategori (PRD hal. 8).
/// `credited` = set user_id yang sudah dikreditkan pekan itu.
fn compute_rewards(
    counts: Vec<repo::WeeklyCatCount>,
    credited: &[i64],
) -> Vec<WeeklyRewardRow> {
    use std::collections::BTreeMap;
    // user_id → (name, nis, total_points, detail_parts)
    let mut acc: BTreeMap<i64, (String, String, i32, Vec<String>)> = BTreeMap::new();
    for c in counts {
        let (na, nt, fh) = weekly_reward_points(&c.activity_type);
        if na == 0 && nt == 0 && fh == 0 {
            continue; // kategori tanpa reward (other)
        }
        let attended = c.hadir + c.telat;
        if attended == 0 {
            continue; // tak benar-benar hadir (mis. semua sakit/alfa) → tak berhak
        }
        let label = cat_label(&c.activity_type);
        let mut pts = 0;
        let mut parts: Vec<String> = Vec::new();
        // No-Alfa: tak ada alfa.
        if na > 0 && c.alfa == 0 {
            pts += na;
            parts.push(format!("{label}: No Alfa +{na}"));
        }
        // No-Telat: tak ada telat.
        if nt > 0 && c.telat == 0 {
            pts += nt;
            parts.push(format!("{label}: No Telat +{nt}"));
        }
        // Full-Hadir: sempurna (tanpa alfa/telat/izin/sakit).
        if fh > 0 && c.alfa == 0 && c.telat == 0 && c.izin == 0 && c.sakit == 0 {
            pts += fh;
            parts.push(format!("{label}: Full Hadir +{fh}"));
        }
        if pts == 0 {
            continue;
        }
        let e = acc.entry(c.user_id).or_insert_with(|| {
            (c.name.clone(), c.nis.clone().unwrap_or_default(), 0, Vec::new())
        });
        e.2 += pts;
        e.3.extend(parts);
    }
    acc.into_iter()
        .filter(|(_, (_, _, pts, _))| *pts > 0)
        .map(|(user_id, (name, nis, points, parts))| WeeklyRewardRow {
            user_id,
            name,
            nis,
            points,
            detail: parts.join("; "),
            credited: credited.contains(&user_id),
        })
        .collect()
}

pub async fn weekly_recap(pool: &Pool, offset: i32) -> Result<WeeklyRecapData> {
    let (start, end) = week_range(offset);
    let (raw, cat_counts, credited, nets) = tokio::join!(
        repo::weekly_recap(pool, start, end),
        repo::weekly_counts_by_category(pool, start, end),
        repo::credited_users_for_week(pool, start),
        repo::weekly_net_points(pool, start, end),
    );
    let raw = raw?;
    let credited = credited?;
    let rewards = compute_rewards(cat_counts?, &credited);
    let rewards_total: i32 = rewards.iter().map(|r| r.points).sum();
    let rewards_pending = rewards.iter().filter(|r| !r.credited).count() as i32;

    let pemanggilan: Vec<PemanggilanItem> = nets?
        .into_iter()
        .map(|n| {
            let (tier, tier_kind) = pemanggilan_tier(n.net);
            PemanggilanItem {
                name: n.name,
                nis: n.nis.unwrap_or_default(),
                class_name: n.class_name.unwrap_or_else(|| "-".into()),
                net: n.net,
                tier: tier.into(),
                tier_kind: tier_kind.into(),
            }
        })
        .collect();

    let mut classes: Vec<String> = Vec::new();
    let mut angkatans: Vec<String> = Vec::new();
    let mut pct_sum: i64 = 0;
    let mut pct_n: i64 = 0;

    let rows: Vec<WeeklyRecapRow> = raw
        .into_iter()
        .map(|r| {
            let nis = r.nis.unwrap_or_default();
            let angkatan = angkatan_of(&nis);
            let class_name = r.class_name.unwrap_or_else(|| "-".into());
            let attended = r.hadir + r.telat;
            let total = attended + r.izin + r.alpa;
            let pct = if total > 0 { ((attended * 100) / total) as i32 } else { 0 };
            if total > 0 {
                pct_sum += pct as i64;
                pct_n += 1;
            }
            if class_name != "-" && !classes.contains(&class_name) {
                classes.push(class_name.clone());
            }
            if angkatan != "-" && !angkatans.contains(&angkatan) {
                angkatans.push(angkatan.clone());
            }
            WeeklyRecapRow {
                name: r.name,
                nis,
                class_name,
                angkatan,
                hadir: r.hadir,
                telat: r.telat,
                izin: r.izin,
                alpa: r.alpa,
                pct,
                points: r.points,
            }
        })
        .collect();

    classes.sort();
    angkatans.sort();
    angkatans.reverse(); // angkatan terbaru dulu

    let total_santri = rows.len() as i64;
    let avg_pct = if pct_n > 0 { (pct_sum / pct_n) as i32 } else { 0 };

    // Daftar SP dari saldo poin (≤150). Saldo terendah dulu.
    let mut sp_list: Vec<SpItem> = rows
        .iter()
        .filter_map(|r| {
            sp_level(r.points).map(|(level, kind, treatment)| SpItem {
                name: r.name.clone(),
                nis: r.nis.clone(),
                class_name: r.class_name.clone(),
                points: r.points,
                level: level.into(),
                level_kind: kind.into(),
                treatment: treatment.into(),
            })
        })
        .collect();
    sp_list.sort_by_key(|s| s.points);

    Ok(WeeklyRecapData {
        week_label: fmt_range(start, end),
        offset: offset.max(0),
        classes,
        angkatans,
        rows,
        total_santri,
        avg_pct,
        rewards,
        rewards_total,
        rewards_pending,
        pemanggilan,
        sp_list,
    })
}

/// Kreditkan reward mingguan (admin) untuk pekan `offset`. Idempotent per santri
/// (weekly_rewards UNIQUE). Return (jumlah santri baru dikredit, total poin).
pub async fn credit_weekly_rewards(pool: &Pool, offset: i32) -> Result<(i64, i64)> {
    let (start, end) = week_range(offset);
    let counts = repo::weekly_counts_by_category(pool, start, end).await?;
    let credited = repo::credited_users_for_week(pool, start).await?;
    let rewards = compute_rewards(counts, &credited);
    let mut n = 0i64;
    let mut total = 0i64;
    for r in rewards {
        if r.credited {
            continue;
        }
        if repo::credit_weekly_reward(pool, r.user_id, start, r.points, &r.detail).await? {
            n += 1;
            total += r.points as i64;
        }
    }
    Ok((n, total))
}
