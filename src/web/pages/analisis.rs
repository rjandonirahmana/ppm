//! web/pages/analisis.rs — Dashboard Analisis Guru (/guru) & Dewan Guru
//! (/dewan-guru), data ASLI lewat `analisis_data`.
//!
//! Satu komponen dipakai untuk dua rute: guru biasa hanya melihat kelas yang
//! ia ampu sendiri, dewan guru/admin melihat seluruh pesantren + insight
//! kinerja tiap pengajar (`AnalisisData::is_dewan`).

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{AnalisisData, ClassRank, TeacherInsight, TrendPoint};
use crate::web::api::analisis_data;
use crate::web::components::{
    DeviceFrame, FetchError, MobileHeader,
};

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
        <Title text=format!("{title} — PPM AFM") />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto">
                <MobileHeader title=title subtitle="Portal Administrasi" />
                <div class="px-5 pt-5 space-y-5 stagger">
                    <Suspense fallback=|| {
                        view! {
                            <div class="space-y-3 animate-pulse">
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                                <div class="h-40 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => view! { <AnalisisBody d=d /> }.into_any(),
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
fn AnalisisBody(d: AnalisisData) -> impl IntoView {
    let AnalisisData { name, is_dewan, attendance_pct, avg_points, sessions_verified, trend, class_ranking, teacher_insight } = d;
    let scope_note = if is_dewan { "Seluruh pesantren" } else { "Kelas yang Anda ampu" };

    view! {
        <div>
            <p class="text-body-sm text-on-surface-variant">{name}</p>
            <p class="text-[11px] text-on-surface-variant/70 mt-0.5">{scope_note}" • 30 hari terakhir"</p>
        </div>

        <div class="grid grid-cols-3 gap-2">
            <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-3 text-center">
                <p class="text-[10px] uppercase tracking-wider text-on-surface-variant">"Kehadiran"</p>
                <p class="text-xl font-bold text-primary mt-1">{format!("{attendance_pct}%")}</p>
            </div>
            <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-3 text-center">
                <p class="text-[10px] uppercase tracking-wider text-on-surface-variant">"Rata² Poin"</p>
                <p class="text-xl font-bold text-primary mt-1">{avg_points}</p>
            </div>
            <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-3 text-center">
                <p class="text-[10px] uppercase tracking-wider text-on-surface-variant">"Terverifikasi"</p>
                <p class="text-xl font-bold text-primary mt-1">{sessions_verified}</p>
            </div>
        </div>

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
    }
}

#[component]
fn TrendChart(trend: Vec<TrendPoint>) -> impl IntoView {
    view! {
        <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4">
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
        <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-3 flex items-center justify-between">
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
        <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-3 flex items-center justify-between">
            <div>
                <p class="font-semibold text-on-background text-body-sm">{t.name}</p>
                <p class="text-[11px] text-on-surface-variant">{format!("{} sesi (30 hari)", t.sessions_count)}</p>
            </div>
            <span class="text-body-sm font-bold text-primary">{format!("{}%", t.attendance_pct)}</span>
        </div>
    }
}
