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
use crate::web::pages::MaterialsWidget;

fn status_badge(kind: &str) -> &'static str {
    match kind {
        "late" => "ppm-chip bg-warning/10 text-warning",
        "permit" | "sick" => "ppm-chip bg-info/10 text-info",
        "absent" => "ppm-chip bg-error-container text-error",
        _ => "ppm-chip bg-success/10 text-success",
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
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Dashboard Staf" subtitle="Ringkasan aktivitas hari ini" />
                <div class="px-5 pt-5 space-y-5 stagger">
                    <Suspense fallback=|| {
                        view! {
                            <div class="space-y-3 animate-pulse">
                                <div class="grid grid-cols-2 md:grid-cols-3 gap-3">
                                    <div class="h-24 bg-surface-container rounded-2xl"></div>
                                    <div class="h-24 bg-surface-container rounded-2xl"></div>
                                    <div class="h-24 bg-surface-container rounded-2xl col-span-2 md:col-span-1"></div>
                                </div>
                                <div class="h-28 bg-surface-container rounded-2xl"></div>
                                <div class="grid md:grid-cols-3 gap-3">
                                    <div class="h-40 bg-surface-container rounded-2xl md:col-span-2"></div>
                                    <div class="h-40 bg-surface-container rounded-2xl"></div>
                                </div>
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

        // ── Statistik — mobile: 2 kartu + izin di bawah; DESKTOP (mockup
        // framed_desktop): SATU baris 3 kartu. ────────────────────────────
        {
            let live_count = live.iter().filter(|s| s.state == "live").count();
            let absen = (total_santri - hadir_today).max(0);
            view! {
                <div class="grid grid-cols-2 md:grid-cols-3 gap-3">
                    <div class="ppm-card p-4">
                        <div class="flex items-start justify-between">
                            <span class="w-10 h-10 ppm-tile">
                                <span class="material-symbols-outlined">"group"</span>
                            </span>
                            <span class="text-[11px] font-bold text-success">{format!("+{santri_growth_month} baru")}</span>
                        </div>
                        <p class="text-2xl font-bold text-on-background mt-3" data-count=total_santri.to_string()>{total_santri}</p>
                        <p class="text-body-sm text-on-surface-variant">"Total Santri"</p>
                    </div>
                    <div class="ppm-card p-4">
                        <div class="flex items-start justify-between">
                            <span class="w-10 h-10 ppm-tile">
                                <span class="material-symbols-outlined">"school"</span>
                            </span>
                            {(live_count > 0)
                                .then(|| view! {
                                    <span class="text-[11px] font-bold text-success flex items-center gap-1">
                                        <span class="w-1.5 h-1.5 rounded-full bg-success pulse-dot"></span>
                                        "Live"
                                    </span>
                                })}
                        </div>
                        <p class="text-2xl font-bold text-on-background mt-3">{live.len()}</p>
                        <p class="text-body-sm text-on-surface-variant">"Sesi Hari Ini"</p>
                    </div>
                    <a
                        href="/izin-staf"
                        class="col-span-2 md:col-span-1 block bg-error-container/40 border border-error/30 rounded-2xl p-4 hover:bg-error-container/60 transition-colors"
                    >
                        <div class="flex items-center justify-between h-full">
                            <div>
                                <p class="text-body-sm text-on-surface-variant">"Permohonan Izin"</p>
                                <p class="text-xl font-bold text-on-background">{format!("{izin_pending} Menunggu")}</p>
                            </div>
                            <span class="material-symbols-outlined text-error">"pending_actions"</span>
                        </div>
                    </a>
                </div>

                // ── Hero: Kehadiran Hari Ini (kartu gradien mockup) ──────
                <div class="spiritual-gradient rounded-2xl p-5 text-on-primary shadow-lg shadow-primary/20">
                    <div class="flex items-center justify-between">
                        <p class="text-body-lg font-bold">"Kehadiran Hari Ini"</p>
                        <p class="text-display-md" data-count=pct.to_string()>{format!("{pct}%")}</p>
                    </div>
                    <div class="w-full h-2 bg-white/20 rounded-full overflow-hidden mt-3">
                        <div class="bg-primary-fixed h-full bar-grow" style=format!("width: {pct}%")></div>
                    </div>
                    <div class="flex items-center justify-between mt-2.5 text-body-sm">
                        <span class="flex items-center gap-1.5 opacity-90">
                            <span class="w-1.5 h-1.5 rounded-full bg-primary-fixed"></span>
                            {format!("{hadir_today} Hadir")}
                        </span>
                        <span class="flex items-center gap-1.5 opacity-70">
                            <span class="w-1.5 h-1.5 rounded-full bg-white/40"></span>
                            {format!("{absen} Belum/Absen")}
                        </span>
                    </div>
                </div>
            }
        }

        // ── Alat Administrasi (grid mockup) ──────────────────────
        <div>
            <h3 class="text-body-lg font-bold text-on-background mb-2">"Alat Administrasi"</h3>
            <div class="grid grid-cols-4 gap-2">
                {[
                    ("school", "Kelas", "/kelas"),
                    ("groups", "Santri", "/students"),
                    ("stars", "Poin", "/poin"),
                    ("cast_for_education", "Sesi", "/sesi"),
                    ("summarize", "Rekap", "/rekap-mingguan"),
                    ("shield", "Kontrol", "/kontrol-pengguna"),
                    ("settings", "Setelan", "/setelan"),
                ]
                    .into_iter()
                    .map(|(icon, label, href)| {
                        view! {
                            <a href=href class="bg-surface-container rounded-2xl p-3 flex flex-col items-center gap-1.5 press">
                                <span class="material-symbols-outlined text-primary">{icon}</span>
                                <span class="text-[11px] font-medium text-on-surface-variant">{label}</span>
                            </a>
                        }
                    })
                    .collect_view()}
            </div>
        </div>

        <MaterialsWidget manage=true />

        // ── Area 2 KOLOM di desktop (mockup: antrean kiri, panel "Selesai
        // Hari Ini" lavender kanan); mobile tetap bertumpuk. ──────────────
        <div class="space-y-5 md:space-y-0 md:grid md:grid-cols-3 md:gap-5 md:items-start">
            // ── Sesi Live ────────────────────────────────────────
            <div class="md:col-span-2">
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

            // ── Kehadiran Terbaru (desktop: panel lavender ala mockup) ──
            <div class="md:bg-surface-container-low md:rounded-2xl md:p-4">
                <h3 class="text-title-md text-on-background font-semibold mb-3">"Kehadiran Terbaru"</h3>
                <div class="ppm-card divide-y divide-outline-variant/40">
                    {if latest.is_empty() {
                        view! { <p class="text-body-sm text-on-surface-variant text-center py-6">"Belum ada catatan hari ini."</p> }
                            .into_any()
                    } else {
                        latest.into_iter().map(|a| view! { <LatestRow a=a /> }).collect_view().into_any()
                    }}
                </div>
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
