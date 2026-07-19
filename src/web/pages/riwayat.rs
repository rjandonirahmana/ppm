//! web/pages/riwayat.rs — Riwayat Kehadiran santri (mockup stitch):
//! stat HADIR/IZIN/ALPA, chip semester, daftar dikelompokkan per bulan dengan
//! poin (+10 Kedisiplinan / -15 Pelanggaran). Data ASLI via `riwayat_data`.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{RiwayatData, RiwayatItem};
use crate::web::api::riwayat_data;
use crate::web::components::{FetchError, DeviceFrame, MobileHeader};

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
        "late" => "px-2.5 py-1 rounded-full text-[10px] font-bold tracking-wider bg-warning/10 text-warning",
        "permit" => "px-2.5 py-1 rounded-full text-[10px] font-bold tracking-wider bg-info/10 text-info",
        "absent" => "px-2.5 py-1 rounded-full text-[10px] font-bold tracking-wider bg-error-container text-error",
        _ => "px-2.5 py-1 rounded-full text-[10px] font-bold tracking-wider bg-success/10 text-success",
    }
}

#[component]
pub fn RiwayatPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { riwayat_data().await });

    Effect::new(move |_| {
        if let Some(Err(e)) = data.get() {
            let msg = e.to_string();
            if msg.contains("unauth") || msg.contains("forbidden") {
                #[cfg(target_arch = "wasm32")]
                if let Some(w) = web_sys::window() {
                    let _ = w.location().replace("/login");
                }
            }
        }
    });

    // Filter bulan (klien): "" = semua.
    let month_filter = RwSignal::new(String::new());

    view! {
        <Title text="Riwayat Kehadiran — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto">
                <MobileHeader title="Riwayat Kehadiran" />

                <div class="px-5 pt-5 space-y-4 stagger">
                    <Suspense fallback=|| {
                        view! {
                            <div class="space-y-3 animate-pulse">
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => {
                                        view! { <RiwayatContent d=d month_filter=month_filter /> }
                                            .into_any()
                                    }
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any()
                                })
                        }}
                    </Suspense>
                </div>

            </div>
        </DeviceFrame>
    }
}

#[component]
fn RiwayatContent(d: RiwayatData, month_filter: RwSignal<String>) -> impl IntoView {
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
        // ── Statistik semester ──────────────────────────────────────────────
        <div class="space-y-3">
            <StatCard
                icon="check_circle" tone="text-success" num=d.hadir
                label="HADIR" sub="Sesi Semester Ini"
            />
            <StatCard
                icon="info" tone="text-info" num=d.izin
                label="IZIN" sub="Sesi Disetujui"
            />
            <StatCard
                icon="cancel" tone="text-error" num=d.alpa
                label="ALPA" sub="Sesi Tanpa Keterangan"
            />
        </div>

        // ── Chip semester + filter bulan ────────────────────────────────────
        <div class="flex gap-3 overflow-x-auto pb-1">
            <span class="px-5 py-2.5 bg-primary text-on-primary rounded-full text-body-sm font-semibold whitespace-nowrap shrink-0">
                {d.semester_label}
            </span>
        </div>
        <div class="relative">
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
                    <div class="bg-surface-container rounded-2xl p-8 text-center text-body-sm text-on-surface-variant">
                        "Belum ada catatan kehadiran."
                    </div>
                }
                    .into_any();
            }
            let mut out: Vec<AnyView> = Vec::new();
            let mut last_month = String::new();
            for it in list {
                if it.month != last_month {
                    last_month = it.month.clone();
                    let m = it.month.clone();
                    out.push(
                        view! {
                            <div class="flex items-center gap-2 pt-3">
                                <span class="material-symbols-outlined text-on-surface-variant text-xl">
                                    "calendar_month"
                                </span>
                                <h3 class="text-body-lg font-bold text-on-background">{m}</h3>
                            </div>
                        }
                            .into_any(),
                    );
                }
                out.push(view! { <RiwayatCard it=it /> }.into_any());
            }
            out.into_any()
        }}
    }
}

#[component]
fn StatCard(
    icon: &'static str,
    tone: &'static str,
    num: i64,
    label: &'static str,
    sub: &'static str,
) -> impl IntoView {
    let head_cls = format!("flex items-center gap-1.5 {tone}");
    let num_cls = format!("text-3xl font-bold {tone} mt-1");
    view! {
        <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4 card-hover">
            <div class=head_cls>
                <span class="material-symbols-outlined text-xl">{icon}</span>
                <span class="text-[11px] font-bold tracking-[0.15em]">{label}</span>
            </div>
            <p class=num_cls data-count=num.to_string()>{num}</p>
            <p class="text-body-sm text-on-surface-variant">{sub}</p>
        </div>
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
            class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4 flex items-start gap-3 card-hover anim-in"
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
