//! web/pages/staf.rs — Dashboard Staf/Admin (/staf), data ASLI.
//!
//! Statistik hari ini (total santri, hadir, izin pending) + sesi live +
//! kehadiran terbaru. Sebelumnya halaman ini HTML statis (staf.html via
//! include_str!) tanpa data sungguhan — sekarang disambung `staf_home_data`.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{LatestAtt, LiveSesi, StafHome};
use crate::web::api::staf_home_data;
use crate::web::components::{DeviceFrame, FetchError, MobileHeader};

fn status_badge(kind: &str) -> &'static str {
    match kind {
        "late" => "px-2.5 py-1 rounded-full text-[10px] font-bold tracking-wider bg-warning/10 text-warning",
        "permit" | "sick" => "px-2.5 py-1 rounded-full text-[10px] font-bold tracking-wider bg-info/10 text-info",
        "absent" => "px-2.5 py-1 rounded-full text-[10px] font-bold tracking-wider bg-error-container text-error",
        _ => "px-2.5 py-1 rounded-full text-[10px] font-bold tracking-wider bg-success/10 text-success",
    }
}

fn live_state_badge(state: &str) -> (&'static str, &'static str) {
    match state {
        "live" => ("AKTIF", "text-[10px] font-bold text-primary bg-primary/10 px-2 py-0.5 rounded-full"),
        "upcoming" => ("TERJADWAL", "text-[10px] font-bold text-on-surface-variant bg-surface-container-high px-2 py-0.5 rounded-full"),
        _ => ("ISTIRAHAT", "text-[10px] font-bold text-on-surface-variant bg-surface-container-high px-2 py-0.5 rounded-full"),
    }
}

#[component]
pub fn StafDashboardPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { staf_home_data().await });

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

    view! {
        <Title text="Dashboard Staf — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto">
                <MobileHeader title="Dashboard Staf" subtitle="Ringkasan aktivitas hari ini" />
                <div class="px-5 pt-5 space-y-5 stagger">
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
                                    Ok(d) => view! { <StafBody d=d /> }.into_any(),
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                })
                        }}
                    </Suspense>
                </div>
            </div>
        </DeviceFrame>
    }
}

#[component]
fn StafBody(d: StafHome) -> impl IntoView {
    let StafHome { name, total_santri, santri_growth_month, hadir_today, pct, izin_pending, live, latest } = d;

    view! {
        <div>
            <p class="text-headline-sm text-on-background">{format!("Assalamu'alaikum, {name}")}</p>
            <p class="text-body-sm text-on-surface-variant mt-1">"Ringkasan aktivitas dan laporan hari ini."</p>
        </div>

        // ── Statistik ────────────────────────────────────────────
        <div class="grid grid-cols-2 gap-3">
            <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4">
                <span class="material-symbols-outlined text-primary">"group"</span>
                <p class="text-body-sm text-on-surface-variant mt-2">"Total Santri"</p>
                <p class="text-2xl font-bold text-on-background">{total_santri}</p>
                <p class="text-[11px] text-success mt-1">{format!("+{santri_growth_month} bulan ini")}</p>
            </div>
            <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4">
                <span class="material-symbols-outlined text-primary">"how_to_reg"</span>
                <p class="text-body-sm text-on-surface-variant mt-2">"Hadir Hari Ini"</p>
                <p class="text-2xl font-bold text-on-background">
                    {hadir_today}
                    <span class="text-body-sm text-on-surface-variant font-normal">{format!(" / {total_santri}")}</span>
                </p>
                <div class="w-full h-1.5 bg-surface-container-high rounded-full overflow-hidden mt-2">
                    <div class="bg-primary h-full" style=format!("width: {pct}%")></div>
                </div>
            </div>
        </div>

        <a
            href="/verifikasi-pamong"
            class="block bg-error-container/40 border border-error/30 rounded-2xl p-4 hover:bg-error-container/60 transition-colors"
        >
            <div class="flex items-center justify-between">
                <div>
                    <p class="text-body-sm text-on-surface-variant">"Permohonan Izin"</p>
                    <p class="text-xl font-bold text-on-background">{format!("{izin_pending} Menunggu")}</p>
                </div>
                <span class="material-symbols-outlined text-error">"pending_actions"</span>
            </div>
        </a>

        // ── Sesi Live ────────────────────────────────────────────
        <div>
            <h3 class="text-title-md text-on-background font-semibold mb-3">"Sesi Kelas Hari Ini"</h3>
            <div class="space-y-2">
                {if live.is_empty() {
                    view! {
                        <p class="text-body-sm text-on-surface-variant text-center py-4">
                            "Belum ada sesi terjadwal hari ini."
                        </p>
                    }
                        .into_any()
                } else {
                    live.into_iter().map(|s| view! { <LiveSesiCard s=s /> }).collect_view().into_any()
                }}
            </div>
        </div>

        // ── Kehadiran Terbaru ────────────────────────────────────
        <div>
            <h3 class="text-title-md text-on-background font-semibold mb-3">"Kehadiran Terbaru"</h3>
            <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl divide-y divide-outline-variant/40">
                {if latest.is_empty() {
                    view! { <p class="text-body-sm text-on-surface-variant text-center py-6">"Belum ada catatan hari ini."</p> }
                        .into_any()
                } else {
                    latest.into_iter().map(|a| view! { <LatestRow a=a /> }).collect_view().into_any()
                }}
            </div>
        </div>
    }
}

#[component]
fn LiveSesiCard(s: LiveSesi) -> impl IntoView {
    let (label, badge_cls) = live_state_badge(&s.state);
    let border = if s.state == "live" { "border-l-4 border-primary" } else { "border-l-4 border-outline-variant" };
    view! {
        <div class=format!(
            "bg-surface-container-lowest p-3 rounded-xl {border} flex items-center justify-between",
        )>
            <div>
                <p class="font-semibold text-on-background">{s.title}</p>
                <p class="text-[11px] text-on-surface-variant">{format!("{} • {} santri • {}", s.teacher, s.santri_count, s.time_label)}</p>
            </div>
            <span class=badge_cls>{label}</span>
        </div>
    }
}

#[component]
fn LatestRow(a: LatestAtt) -> impl IntoView {
    view! {
        <div class="flex items-center justify-between p-3">
            <div class="flex items-center gap-3 min-w-0">
                <div class="w-8 h-8 rounded-full bg-secondary-container flex items-center justify-center text-[10px] font-bold text-primary shrink-0">
                    {a.initial}
                </div>
                <div class="min-w-0">
                    <p class="text-body-sm font-semibold text-on-background truncate">{a.name}</p>
                    <p class="text-[11px] text-on-surface-variant truncate">{format!("{} • {}", a.class_name, a.time_label)}</p>
                </div>
            </div>
            <span class=status_badge(&a.kind)>{a.status_label}</span>
        </div>
    }
}
