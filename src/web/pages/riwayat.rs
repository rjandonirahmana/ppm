//! web/pages/riwayat.rs — Riwayat & Rapor santri (mockup stitch + gabungan
//! rapor pribadi): kartu rapor (poin/kehadiran/gerbang/hafalan/prestasi —
//! REUSE komponen dari pages/laporan.rs) + daftar riwayat dikelompokkan per
//! bulan (+10 Kedisiplinan / -15 Pelanggaran). Item navbar "Laporan" sisi
//! santri sudah diganti "Akademik" (self-report progres buku, lihat
//! pages/akademik.rs) — rapor pribadi yang tadinya di /laporan kini digabung
//! ke sini agar santri tak perlu 2 halaman terpisah. Data via `riwayat_data`
//! + `laporan_santri_data`.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{LaporanSantriData, RiwayatData, RiwayatItem};
use crate::web::api::{laporan_santri_data, riwayat_data};
use crate::web::components::{DeviceFrame, EmptyState, FetchError, MobileHeader};
use crate::web::pages::laporan::{attendance_card, gate_status_card, hafalan_card, points_lists};

fn kind_border(kind: &str) -> &'static str {
    match kind {
        "late" => "border-left:4px solid #f59e0b",
        "permit" => "border-left:4px solid #2563eb",
        "absent" => "border-left:4px solid #dc2626",
        _ => "border-left:4px solid #059669",
    }
}

fn kind_badge(kind: &str) -> &'static str {
    match kind {
        "late" => "ppm-chip bg-warning/10 text-warning",
        "permit" => "ppm-chip bg-info/10 text-info",
        "absent" => "ppm-chip bg-error-container text-error",
        _ => "ppm-chip bg-success/10 text-success",
    }
}

#[component]
pub fn RiwayatPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { riwayat_data().await });
    let laporan_res = Resource::new(|| (), |_| async move { laporan_santri_data().await });

    Effect::new(move |_| {
        let unauth = matches!(&data.get(), Some(Err(e)) if e.to_string().contains("unauth"))
            || matches!(&laporan_res.get(), Some(Err(e)) if e.to_string().contains("unauth"));
        if unauth {
            #[cfg(target_arch = "wasm32")]
            if let Some(w) = web_sys::window() {
                let _ = w.location().replace("/login");
            }
        }
    });

    // Filter bulan (klien): "" = semua.
    let month_filter = RwSignal::new(String::new());

    view! {
        <Title text="Riwayat & Rapor — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Riwayat & Rapor" subtitle="Kehadiran, poin, & capaian hafalan" />

                <div class="px-5 pt-5 space-y-4 stagger">
                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                                <div class="grid grid-cols-1 md:grid-cols-3 gap-3">
                                    <div class="h-24 bg-surface-container rounded-2xl"></div>
                                    <div class="h-24 bg-surface-container rounded-2xl"></div>
                                    <div class="h-24 bg-surface-container rounded-2xl"></div>
                                </div>
                                <div class="h-10 bg-surface-container rounded-xl"></div>
                                <div class="grid gap-3 md:grid-cols-2">
                                    <div class="h-20 bg-surface-container rounded-2xl"></div>
                                    <div class="h-20 bg-surface-container rounded-2xl"></div>
                                </div>
                            </div>
                        }
                    }>
                        {move || {
                            match (data.get(), laporan_res.get()) {
                                (Some(Ok(d)), Some(Ok(l))) => {
                                    view! {
                                        <RiwayatContent d=d l=l month_filter=month_filter />
                                    }
                                        .into_any()
                                }
                                (Some(Err(e)), _) | (_, Some(Err(e))) => {
                                    view! { <FetchError err=e.to_string() /> }.into_any()
                                }
                                _ => ().into_any(),
                            }
                        }}
                    </Suspense>
                </div>

            </div>
        </DeviceFrame>
    }
}

#[component]
fn RiwayatContent(d: RiwayatData, l: LaporanSantriData, month_filter: RwSignal<String>) -> impl IntoView {
    // Daftar bulan unik (urutan data = terbaru dulu).
    let months: Vec<String> = {
        let mut seen = Vec::new();
        for it in &d.items {
            if !seen.contains(&it.month) {
                seen.push(it.month.clone());
            }
        }
        seen
    };
    let items = StoredValue::new(d.items);

    view! {
        // ── Rapor pribadi (pindah dari /laporan — gabungan dgn riwayat) ──────
        <div class="spiritual-gradient rounded-2xl p-5 text-on-primary shadow-lg shadow-primary/20">
            <p class="text-[11px] font-bold tracking-[0.2em] opacity-80">"RAPOR PRIBADI"</p>
            <div class="flex items-center justify-between mt-1">
                <span class="text-body-sm opacity-85">"Total Poin"</span>
                <span class="text-display-md">{l.points}</span>
            </div>
        </div>
        <div class="md:grid md:grid-cols-2 md:gap-5 md:items-start space-y-5 md:space-y-0">
            <div class="space-y-5">
                {attendance_card(l.hadir, l.izin, l.alpa, l.attendance_pct)}
                {gate_status_card(&l.gate_status, &l.gate_at_label)}
                {hafalan_card(l.hafalan, l.juz_count)}
            </div>
            <div class="space-y-5">{points_lists(l.prestasi, l.pelanggaran, true)}</div>
        </div>

        // ── Riwayat kehadiran detail per sesi ─────────────────────────────────
        <h3 class="text-body-lg font-bold text-on-background pt-2">"Riwayat Kehadiran"</h3>

        // ── Chip semester + filter bulan ────────────────────────────────────
        <div class="flex gap-3 overflow-x-auto pb-1">
            <span class="px-5 py-2.5 bg-primary text-on-primary rounded-full text-body-sm font-semibold whitespace-nowrap shrink-0">
                {d.semester_label}
            </span>
        </div>
        <div class="relative md:max-w-xs">
            <select
                class="w-full appearance-none bg-surface-container border-0 rounded-xl px-4 py-3 text-body-md text-on-surface pr-10"
                on:change=move |ev| month_filter.set(event_target_value(&ev))
            >
                <option value="">"Semua Bulan"</option>
                {months
                    .iter()
                    .map(|m| {
                        let val = m.clone();
                        let label = m.clone();
                        view! { <option value=val>{label}</option> }
                    })
                    .collect_view()}
            </select>
            <span class="material-symbols-outlined absolute right-3 top-1/2 -translate-y-1/2 text-on-surface-variant pointer-events-none">
                "expand_more"
            </span>
        </div>

        // ── Daftar per bulan ────────────────────────────────────────────────
        {move || {
            let filter = month_filter.get();
            let list: Vec<RiwayatItem> = items
                .get_value()
                .into_iter()
                .filter(|it| filter.is_empty() || it.month == filter)
                .collect();
            if list.is_empty() {
                return view! {
                    <EmptyState icon="history" title="Belum ada catatan kehadiran" />
                }
                    .into_any();
            }
            // Kelompokkan per bulan dulu → tiap grup jadi grid 2 kolom di
            // desktop (header bulan tetap full-width, tak ikut grid).
            let mut groups: Vec<(String, Vec<RiwayatItem>)> = Vec::new();
            for it in list {
                match groups.last_mut() {
                    Some((m, v)) if *m == it.month => v.push(it),
                    _ => groups.push((it.month.clone(), vec![it])),
                }
            }
            groups
                .into_iter()
                .map(|(m, items)| {
                    view! {
                        <div class="pt-3">
                            <div class="flex items-center gap-2 mb-2">
                                <span class="material-symbols-outlined text-on-surface-variant text-xl">
                                    "calendar_month"
                                </span>
                                <h3 class="text-body-lg font-bold text-on-background">{m}</h3>
                            </div>
                            <div class="space-y-3 md:space-y-0 md:grid md:grid-cols-2 md:gap-3">
                                {items.into_iter().map(|it| view! { <RiwayatCard it=it /> }).collect_view()}
                            </div>
                        </div>
                    }
                })
                .collect_view()
                .into_any()
        }}
    }
}

#[component]
fn RiwayatCard(it: RiwayatItem) -> impl IntoView {
    let border = kind_border(&it.kind);
    let badge_cls = kind_badge(&it.kind);
    let pts_cls = if it.points > 0 {
        "text-body-md font-bold text-success"
    } else if it.points < 0 {
        "text-body-md font-bold text-error"
    } else {
        "text-body-md font-bold text-on-surface-variant"
    };
    let pts_label = format!("{:+} Poin", it.points);
    view! {
        <div
            class="ppm-card p-4 flex items-start gap-3 card-hover anim-in"
            style=border
        >
            <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                    <p class="text-body-md font-bold text-on-background truncate">{it.title}</p>
                    <span class=badge_cls>{it.status_label}</span>
                </div>
                <p class="text-body-sm text-on-surface-variant flex items-center gap-1 mt-1.5">
                    <span class="material-symbols-outlined text-[15px]">"schedule"</span>
                    {it.time_label}
                </p>
            </div>
            <div class="text-right shrink-0">
                <p class=pts_cls>{pts_label}</p>
                <p class="text-[10px] tracking-[0.12em] text-on-surface-variant uppercase mt-0.5">
                    {it.points_note}
                </p>
            </div>
        </div>
    }
}
