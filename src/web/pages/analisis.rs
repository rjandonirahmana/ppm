//! web/pages/analisis.rs — Dashboard Analisis Guru (/guru) & Dewan Guru
//! (/dewan-guru), data ASLI lewat `analisis_data`.
//!
//! Satu komponen dipakai untuk dua rute: guru biasa hanya melihat kelas yang
//! ia ampu sendiri, dewan guru/admin melihat seluruh pesantren + insight
//! kinerja tiap pengajar (`AnalisisData::is_dewan`).

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{AnalisisData, ClassRank, PermitQueueData, TeacherInsight, TrendPoint};
use crate::web::api::{analisis_data, permit_queue_data};
use crate::web::components::{
    DeviceFrame, FetchError, JadwalDeck, MobileHeader,
};
use crate::web::pages::MaterialsWidget;

#[component]
pub fn GuruDashboardPage() -> impl IntoView {
    view! { <AnalisisPage title="Dashboard Analisis" /> }
}

#[component]
pub fn DewanGuruDashboardPage() -> impl IntoView {
    view! { <AnalisisPage title="Dashboard Analisis Dewan Guru" /> }
}

#[component]
fn AnalisisPage(title: &'static str) -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { analisis_data().await });
    // Hanya dewan_guru/admin punya akses (guru biasa → forbidden diam-diam,
    // kartu tak dirender — lihat KELAS_ROLES vs izin_staf role guard).
    let permits = Resource::new(|| (), |_| async move { permit_queue_data().await });

    crate::web::components::guard_sesi(data);

    view! {
        <Title text=format!("{title} — AFM SMART") />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title=title subtitle="Portal Administrasi" settings=true />
                <div class="px-5 pt-5 space-y-5 stagger">
                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="h-28 bg-surface-container rounded-2xl"></div>
                                <div class="grid gap-3 md:grid-cols-3 md:items-start">
                                    <div class="md:col-span-2 grid grid-cols-2 gap-3">
                                        <div class="h-20 bg-surface-container rounded-2xl"></div>
                                        <div class="h-20 bg-surface-container rounded-2xl"></div>
                                    </div>
                                    <div class="h-40 bg-surface-container rounded-2xl"></div>
                                </div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => view! { <AnalisisBody d=d permits=permits /> }.into_any(),
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
fn AnalisisBody(
    d: AnalisisData,
    permits: Resource<Result<PermitQueueData, ServerFnError>>,
) -> impl IntoView {
    let AnalisisData {
        name,
        is_dewan,
        attendance_pct,
        avg_points,
        sessions_verified,
        trend,
        class_ranking,
        teacher_insight,
        today,
    } = d;
    let scope_note = if is_dewan { "Seluruh pesantren" } else { "Kelas yang Anda ampu" };

    view! {
        <div>
            <p class="text-body-sm text-on-surface-variant">{format!("Assalamu'alaikum, {name}")}</p>
            <p class="text-[11px] text-on-surface-variant/70 mt-0.5">{scope_note}" • 30 hari terakhir"</p>
        </div>

        // ── Pintasan: Rekap (guru+dewan), Galeri (dewan) ────────────────────
        <div class="grid grid-cols-2 gap-3">
            <a href="/rekap-mingguan" class="ppm-card p-3 flex items-center gap-2 press">
                <span class="material-symbols-outlined text-primary">"summarize"</span>
                <span class="text-body-sm font-semibold text-on-background">"Rekap Mingguan"</span>
            </a>
            {is_dewan
                .then(|| view! {
                    <a href="/galeri" class="ppm-card p-3 flex items-center gap-2 press">
                        <span class="material-symbols-outlined text-primary">"grid_on"</span>
                        <span class="text-body-sm font-semibold text-on-background">"Galeri Foto"</span>
                    </a>
                })}
            // Kelas yang DIPEGANG sendiri (wali kelas) — beda dari /kelas yang
            // memuat seluruh kelas pesantren.
            <a href="/kelas-saya" class="ppm-card p-3 flex items-center gap-2 press">
                <span class="material-symbols-outlined text-primary">"school"</span>
                <span class="text-body-sm font-semibold text-on-background">"Kelas Saya"</span>
            </a>
        </div>

        // Desktop: ringkasan harian (hero+progres+sesi hari ini) kolom utama
        // kiri, analitik (tren+ranking+kinerja pengajar) jadi sidebar kanan —
        // konten sama, disusun 2 kolom di layar lebar (pola dashboard_santri).
        <div class="space-y-6 md:space-y-0 md:grid md:grid-cols-3 md:gap-6 md:items-start">
        <div class="md:col-span-2 space-y-5">

        // ── Hero: jadwal berikutnya, bisa digeser ke jadwal sesudahnya ──────
        <JadwalDeck sesi=today.clone() />

        // ── Progres + statistik ─────────────────────────────────────────────
        <div class="ppm-card p-4 flex items-center justify-between">
            <div>
                <p class="text-body-sm text-on-surface-variant">"Kehadiran 30 Hari"</p>
                <p class="text-headline-sm font-bold text-on-background">{format!("{attendance_pct}%")}</p>
            </div>
            <svg viewBox="0 0 36 36" class="w-14 h-14 -rotate-90">
                <circle cx="18" cy="18" r="15.9" fill="none" stroke-width="3.5"
                    class="stroke-surface-container-high"></circle>
                <circle cx="18" cy="18" r="15.9" fill="none" stroke-width="3.5" stroke-linecap="round"
                    class="stroke-primary" pathLength="100"
                    stroke-dasharray=format!("{attendance_pct} 100")></circle>
            </svg>
        </div>
        <div class="grid grid-cols-2 gap-3">
            <div class="ppm-card p-4">
                <span class="w-10 h-10 rounded-xl bg-secondary-container text-primary flex items-center justify-center">
                    <span class="material-symbols-outlined">"task_alt"</span>
                </span>
                <p class="text-2xl font-bold text-on-background mt-2" data-count=sessions_verified.to_string()>
                    {sessions_verified}
                </p>
                <p class="text-body-sm text-on-surface-variant">"Absensi Terverifikasi"</p>
            </div>
            <div class="ppm-card p-4">
                <span class="w-10 h-10 rounded-xl bg-secondary-container text-primary flex items-center justify-center">
                    <span class="material-symbols-outlined">"stars"</span>
                </span>
                <p class="text-2xl font-bold text-on-background mt-2" data-count=avg_points.to_string()>
                    {avg_points}
                </p>
                <p class="text-body-sm text-on-surface-variant">"Rata-rata Poin"</p>
            </div>
        </div>

        // ── Tautan Tinjau Izin (migrasi 17; admin/dewan_guru saja) ──────────
        <Suspense fallback=|| ()>
            {move || {
                permits
                    .get()
                    .and_then(|r| r.ok())
                    .map(|p| {
                        view! {
                            <a
                                href="/izin-staf"
                                class="ppm-card p-4 flex items-center justify-between card-hover md:max-w-lg"
                            >
                                <div class="flex items-center gap-3">
                                    <span class="w-10 h-10 ppm-tile">
                                        <span class="material-symbols-outlined">"event_available"</span>
                                    </span>
                                    <div>
                                        <p class="text-body-md font-semibold text-on-background">"Tinjau Izin"</p>
                                        <p class="text-body-sm text-on-surface-variant">
                                            {format!("{} menunggu keputusan", p.pending_count)}
                                        </p>
                                    </div>
                                </div>
                                <span class="material-symbols-outlined text-on-surface-variant">"chevron_right"</span>
                            </a>
                        }
                    })
            }}
        </Suspense>

        // ── Sesi hari ini ───────────────────────────────────────────────────
        {(!today.is_empty())
            .then(|| {
                view! {
                    <div>
                        <div class="flex items-center justify-between mb-2">
                            <h3 class="text-title-md text-on-background font-semibold">"Sesi Hari Ini"</h3>
                            <a href="/sesi" class="text-body-sm font-semibold text-primary">"Lihat Semua"</a>
                        </div>
                        <div class="space-y-2 md:grid md:grid-cols-2 md:gap-2 md:space-y-0">
                            {today
                                .iter()
                                .cloned()
                                .map(|s| {
                                    let live = s.state == "live";
                                    view! {
                                        <a
                                            href=format!("/sesi/{}", s.id)
                                            class="ppm-card p-3.5 flex items-center gap-3 card-hover press"
                                        >
                                            <div class="w-10 h-10 rounded-xl bg-secondary-container text-primary flex items-center justify-center shrink-0">
                                                <span class="material-symbols-outlined">"menu_book"</span>
                                            </div>
                                            <div class="flex-1 min-w-0">
                                                <p class="text-body-sm font-semibold text-on-background truncate">{s.title.clone()}</p>
                                                <p class="text-[11px] text-on-surface-variant truncate">
                                                    {format!("{} • {}", s.time_label, s.teacher)}
                                                </p>
                                            </div>
                                            {live
                                                .then(|| view! {
                                                    <span class="w-2 h-2 rounded-full bg-success pulse-dot shrink-0"></span>
                                                })}
                                        </a>
                                    }
                                })
                                .collect_view()}
                        </div>
                    </div>
                }
            })}
        </div>

        <div class="space-y-5">
        <div>
            <h3 class="text-title-md text-on-background font-semibold mb-3">"Tren Kehadiran (7 hari)"</h3>
            <TrendChart trend=trend />
        </div>

        <div>
            <h3 class="text-title-md text-on-background font-semibold mb-3">"Ranking Kelas"</h3>
            <div class="space-y-2">
                {if class_ranking.is_empty() {
                    view! { <p class="text-body-sm text-on-surface-variant text-center py-4">"Belum ada data kelas."</p> }
                        .into_any()
                } else {
                    class_ranking
                        .into_iter()
                        .enumerate()
                        .map(|(i, r)| view! { <ClassRankRow rank=i + 1 r=r /> })
                        .collect_view()
                        .into_any()
                }}
            </div>
        </div>

        {is_dewan
            .then(|| {
                view! {
                    <div>
                        <h3 class="text-title-md text-on-background font-semibold mb-3">
                            "Laporan Kinerja Pengajar"
                        </h3>
                        <div class="space-y-2">
                            {if teacher_insight.is_empty() {
                                view! {
                                    <p class="text-body-sm text-on-surface-variant text-center py-4">
                                        "Belum ada sesi tercatat."
                                    </p>
                                }
                                    .into_any()
                            } else {
                                teacher_insight
                                    .into_iter()
                                    .map(|t| view! { <TeacherRow t=t /> })
                                    .collect_view()
                                    .into_any()
                            }}
                        </div>
                    </div>
                }
            })}

        {is_dewan.then(|| view! { <MaterialsWidget manage=true /> })}
        </div>
        </div>
    }
}

#[component]
fn TrendChart(trend: Vec<TrendPoint>) -> impl IntoView {
    view! {
        <div class="ppm-card p-4">
            <div class="flex items-end justify-between gap-2 h-32">
                {trend
                    .into_iter()
                    .map(|p| {
                        let h = p.pct.clamp(2, 100);
                        view! {
                            <div class="flex-1 flex flex-col items-center gap-1.5">
                                <div class="w-full flex-1 flex items-end">
                                    <div
                                        class="w-full bg-primary/80 rounded-t-md"
                                        style=format!("height: {h}%")
                                    ></div>
                                </div>
                                <span class="text-[10px] text-on-surface-variant">{p.label}</span>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        </div>
    }
}

#[component]
fn ClassRankRow(rank: usize, r: ClassRank) -> impl IntoView {
    view! {
        <div class="ppm-card p-3 flex items-center justify-between">
            <div class="flex items-center gap-3">
                <span class="w-6 h-6 rounded-full bg-primary/10 text-primary text-[11px] font-bold flex items-center justify-center shrink-0">
                    {rank}
                </span>
                <div>
                    <p class="font-semibold text-on-background text-body-sm">{r.name}</p>
                    <p class="text-[11px] text-on-surface-variant">{format!("{} santri • {} poin rata²", r.santri_count, r.avg_points)}</p>
                </div>
            </div>
            <span class="text-body-sm font-bold text-primary">{format!("{}%", r.attendance_pct)}</span>
        </div>
    }
}

#[component]
fn TeacherRow(t: TeacherInsight) -> impl IntoView {
    view! {
        <div class="ppm-card p-3 flex items-center justify-between">
            <div>
                <p class="font-semibold text-on-background text-body-sm">{t.name}</p>
                <p class="text-[11px] text-on-surface-variant">{format!("{} sesi (30 hari)", t.sessions_count)}</p>
            </div>
            <span class="text-body-sm font-bold text-primary">{format!("{}%", t.attendance_pct)}</span>
        </div>
    }
}
