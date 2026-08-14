//! web/pages/laporan.rs — Halaman /laporan, MENGGANTIKAN item Profil di navbar
//! (profil tetap ada, dibuka lewat ikon setting di header — lihat components.rs).
//! Bentuk laporan beda per peran, mengikuti bahasa desain report-ppm-mobile /
//! report-desktop-ppm: admin & ketua → Laporan Institusi; guru & dewan guru →
//! Laporan Kelas Akademik (reuse `analisis_data` + ranking hafalan "Mengaji");
//! orang tua → Laporan Santri (anak terpilih); santri → Rapor Pribadi.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{
    AnalisisData, HafalanItem, LaporanAdminData, LaporanOrtuData, LaporanPointItem,
    LaporanSantriData, SessionUser,
};
use crate::web::api::{
    analisis_data, laporan_admin_data, laporan_guru_extra_data, laporan_ortu_data,
    laporan_santri_data, students_outside_action,
};
use crate::web::components::{DeviceFrame, FetchError, MobileHeader};

#[component]
pub fn LaporanPage() -> impl IntoView {
    let session = use_context::<Resource<Option<SessionUser>>>();
    view! {
        <Title text="Laporan — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Laporan" subtitle="Ringkasan performa & kehadiran" />
                <div class="px-5 pt-5 space-y-5 stagger">
                    <ExportBar />
                    <Transition fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                                <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
                                    <div class="h-24 bg-surface-container rounded-2xl"></div>
                                    <div class="h-24 bg-surface-container rounded-2xl"></div>
                                    <div class="h-24 bg-surface-container rounded-2xl hidden md:block"></div>
                                    <div class="h-24 bg-surface-container rounded-2xl hidden md:block"></div>
                                </div>
                                <div class="grid gap-3 md:grid-cols-3 md:items-start">
                                    <div class="h-64 bg-surface-container rounded-2xl md:col-span-2"></div>
                                    <div class="h-64 bg-surface-container rounded-2xl"></div>
                                </div>
                            </div>
                        }
                    }>
                        {move || {
                            let role = session.and_then(|s| s.get()).flatten().map(|u| u.role);
                            match role.as_deref() {
                                // ketua = admin (finance).
                                Some("admin") | Some("ketua") => {
                                    view! { <LaporanAdminBody /> }.into_any()
                                }
                                Some("teacher") | Some("dewan_guru") => {
                                    view! { <LaporanGuruBody /> }.into_any()
                                }
                                Some("parent") => view! { <LaporanOrtuBody /> }.into_any(),
                                // santri_finance = santri.
                                Some("santri") | Some("santri_finance") => {
                                    view! { <LaporanSantriBody /> }.into_any()
                                }
                                _ => view! {
                                    <div class="ppm-empty">"Sesi tidak valid — silakan masuk kembali."</div>
                                }
                                    .into_any(),
                            }
                        }}
                    </Transition>
                </div>
            </div>
        </DeviceFrame>
    }
}

/// Tombol ekspor laporan (biner via `/api/export/laporan`, di luar server-fn —
/// browser cukup diarahkan langsung, cookie sesi ikut terkirim otomatis).
///
/// ── `download` WAJIB ADA, DAN BUKAN SEKADAR KOSMETIK ─────────────────────────
/// `leptos_router` memasang satu pendengar klik di `window` dan MENCEGAT setiap
/// `<a>` se-origin. Ia hanya melepaskannya ke peramban bila tautan itu punya
/// `download`, `rel="external"`, `target`, atau origin berbeda.
///
/// Tanpa `download`, klik di sini tak mengunduh apa pun: router mendorong URL
/// `/api/export/laporan` sebagai rute SPA, tak ada rute yang cocok, dan yang
/// muncul halaman fallback. Yang membingungkan — dan inilah yang dilaporkan
/// pengguna — MENYEGARKAN halaman sesudah itu berhasil, karena muat-ulang
/// sungguhan tak lewat router sama sekali dan langsung mengenai Axum.
///
/// `download` dipilih ketimbang `rel="external"` karena maksudnya memang
/// mengunduh berkas, bukan pergi ke halaman lain.
#[component]
fn ExportBar() -> impl IntoView {
    view! {
        <div class="flex gap-2 md:max-w-md">
            <a
                href="/api/export/laporan?format=pdf"
                download=""
                class="flex-1 py-2.5 rounded-xl border border-outline-variant text-on-surface font-semibold text-body-sm flex items-center justify-center gap-2 press"
            >
                <span class="material-symbols-outlined text-[18px] text-error">"picture_as_pdf"</span>
                "Ekspor PDF"
            </a>
            <a
                href="/api/export/laporan?format=xlsx"
                download=""
                class="flex-1 py-2.5 rounded-xl border border-outline-variant text-on-surface font-semibold text-body-sm flex items-center justify-center gap-2 press"
            >
                <span class="material-symbols-outlined text-[18px] text-success">"grid_on"</span>
                "Ekspor Excel"
            </a>
        </div>
    }
}

fn pct_bar(pct: i64) -> &'static str {
    if pct >= 90 {
        "bg-success"
    } else if pct >= 75 {
        "bg-primary"
    } else {
        "bg-error"
    }
}

fn class_status_cls(kind: &str) -> &'static str {
    match kind {
        "excellent" => "ppm-chip-sm bg-success/10 text-success",
        "attention" => "ppm-chip-sm bg-error-container text-error",
        _ => "ppm-chip-sm bg-info/10 text-info",
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ADMIN & PAMONG — Laporan Institusi
// ═══════════════════════════════════════════════════════════════════════════

#[component]
fn LaporanAdminBody() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { laporan_admin_data().await });
    let outside = Resource::new(|| (), |_| async move { students_outside_action().await });
    view! {
        <Suspense fallback=|| view! { <div class="ppm-empty">"Memuat…"</div> }>
            {move || {
                data.get()
                    .map(|res| match res {
                        Ok(d) => view! { <AdminReport d=d outside=outside /> }.into_any(),
                        Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                    })
            }}
        </Suspense>
    }
}

#[component]
fn AdminReport(
    d: LaporanAdminData,
    outside: Resource<Result<Vec<crate::models::OutsideRow>, ServerFnError>>,
) -> impl IntoView {
    let max_trend = d.trend.iter().map(|t| t.pct).max().unwrap_or(100).max(1);
    view! {
        <p class="text-body-sm text-on-surface-variant">
            "Ringkasan performa akademik & administratif institusi."
        </p>

        <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
            <div class="ppm-card p-4">
                <p class="text-[11px] font-bold tracking-wider uppercase text-on-surface-variant">
                    "Rata-rata Kehadiran"
                </p>
                <p class="text-2xl font-bold text-on-background mt-1">{format!("{}%", d.attendance_pct)}</p>
                <div class="w-full h-1.5 bg-surface-container-high rounded-full overflow-hidden mt-2">
                    <div
                        class=format!("h-full bar-grow {}", pct_bar(d.attendance_pct))
                        style=format!("width: {}%", d.attendance_pct)
                    ></div>
                </div>
            </div>
            <div class="ppm-card p-4">
                <p class="text-[11px] font-bold tracking-wider uppercase text-on-surface-variant">
                    "Poin Pelanggaran Aktif"
                </p>
                <p class="text-2xl font-bold text-error mt-1">{d.active_violation_points}</p>
                <p class="text-[11px] text-on-surface-variant mt-2">"30 hari terakhir"</p>
            </div>
            <div class="ppm-card p-4 col-span-2 md:col-span-2">
                <div class="flex items-center justify-between">
                    <div>
                        <p class="text-[11px] font-bold tracking-wider uppercase text-on-surface-variant">
                            "Santri di Luar Pondok"
                        </p>
                        <p class="text-2xl font-bold text-on-background mt-1">{d.santri_di_luar}</p>
                    </div>
                    <span class="w-11 h-11 ppm-tile">
                        <span class="material-symbols-outlined">"door_open"</span>
                    </span>
                </div>
            </div>
        </div>

        // ── Tren kehadiran mingguan ──────────────────────────────────────
        <div class="ppm-card p-4">
            <h3 class="text-body-lg font-bold text-on-background mb-3">"Tren Kehadiran 7 Hari"</h3>
            <div class="flex items-end justify-between gap-2 h-28">
                {d.trend
                    .iter()
                    .map(|t| {
                        let h = ((t.pct.max(4) * 100) / max_trend).min(100);
                        view! {
                            <div class="flex-1 flex flex-col items-center gap-1.5">
                                <div class="w-full bg-surface-container-high rounded-md overflow-hidden flex items-end h-24">
                                    <div class="w-full bg-primary/70 bar-grow" style=format!("height: {h}%")></div>
                                </div>
                                <span class="text-[10px] text-on-surface-variant">{t.label.clone()}</span>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        </div>

        <div class="space-y-5 md:space-y-0 md:grid md:grid-cols-3 md:gap-5 md:items-start">
            // ── Performa kelas ────────────────────────────────────────────
            <div class="ppm-card p-4 md:col-span-2">
                <h3 class="text-body-lg font-bold text-on-background mb-3">"Performa Kelas"</h3>
                {if d.classes.is_empty() {
                    view! { <p class="text-body-sm text-on-surface-variant py-3">"Belum ada data kelas."</p> }
                        .into_any()
                } else {
                    view! {
                        <div class="divide-y divide-outline-variant/40">
                            {d.classes
                                .into_iter()
                                .map(|c| {
                                    view! {
                                        <div class="py-3 flex items-center gap-3">
                                            <div class="flex-1 min-w-0">
                                                <p class="text-body-sm font-semibold text-on-background truncate">
                                                    {c.name}
                                                </p>
                                                <p class="text-[11px] text-on-surface-variant">
                                                    {format!("{} santri", c.member_count)}
                                                </p>
                                            </div>
                                            <div class="w-24 h-1.5 bg-surface-container-high rounded-full overflow-hidden hidden sm:block">
                                                <div
                                                    class=format!("h-full {}", pct_bar(c.attendance_pct))
                                                    style=format!("width: {}%", c.attendance_pct)
                                                ></div>
                                            </div>
                                            <span class="text-body-sm font-bold text-on-background w-10 text-right">
                                                {format!("{}%", c.attendance_pct)}
                                            </span>
                                            <span class=class_status_cls(&c.status_kind)>{c.status_label}</span>
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                        .into_any()
                }}
            </div>

            // ── Ringkasan poin + santri di luar ───────────────────────────
            <div class="space-y-5">
                <div class="ppm-card p-4">
                    <h3 class="text-body-lg font-bold text-on-background mb-3">"Ringkasan Poin"</h3>
                    {if d.points_recent.is_empty() {
                        view! { <p class="text-body-sm text-on-surface-variant py-3">"Belum ada catatan poin."</p> }
                            .into_any()
                    } else {
                        view! {
                            <div class="space-y-2.5">
                                {d.points_recent
                                    .into_iter()
                                    .map(|p| {
                                        let (cls, sign) = if p.delta >= 0 {
                                            ("text-success", "+")
                                        } else {
                                            ("text-error", "")
                                        };
                                        view! {
                                            <div class="flex items-center justify-between gap-2">
                                                <div class="min-w-0">
                                                    <p class="text-body-sm font-semibold text-on-background truncate">
                                                        {p.name}
                                                    </p>
                                                    <p class="text-[11px] text-on-surface-variant truncate">
                                                        {format!("{} • {}", p.class_name, p.reason)}
                                                    </p>
                                                </div>
                                                <span class=format!("text-body-sm font-bold shrink-0 {cls}")>
                                                    {format!("{sign}{}", p.delta)}
                                                </span>
                                            </div>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                            .into_any()
                    }}
                </div>

                <div class="ppm-card p-4">
                    <h3 class="text-body-lg font-bold text-on-background mb-3 flex items-center gap-2">
                        <span class="material-symbols-outlined text-[18px]">"door_open"</span>
                        "Sedang di Luar Pondok"
                    </h3>
                    <Suspense fallback=|| view! { <p class="text-body-sm text-on-surface-variant">"Memuat…"</p> }>
                        {move || {
                            outside
                                .get()
                                .map(|res| match res {
                                    Ok(rows) if rows.is_empty() => {
                                        view! {
                                            <p class="text-body-sm text-on-surface-variant py-2">
                                                "Semua santri sedang di dalam pondok."
                                            </p>
                                        }
                                            .into_any()
                                    }
                                    Ok(rows) => {
                                        view! {
                                            <div class="space-y-2">
                                                {rows
                                                    .into_iter()
                                                    .map(|o| {
                                                        view! {
                                                            <div class="flex items-center justify-between gap-2 text-body-sm">
                                                                <div class="min-w-0">
                                                                    <p class="font-semibold text-on-background truncate">
                                                                        {o.name}
                                                                    </p>
                                                                    <p class="text-[11px] text-on-surface-variant truncate">
                                                                        {format!("{} • sejak {}", o.class_name, o.since_label)}
                                                                    </p>
                                                                </div>
                                                            </div>
                                                        }
                                                    })
                                                    .collect_view()}
                                            </div>
                                        }
                                            .into_any()
                                    }
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                })
                        }}
                    </Suspense>
                </div>
            </div>
        </div>
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GURU & DEWAN GURU — Laporan Kelas Akademik (reuse analisis_data)
// ═══════════════════════════════════════════════════════════════════════════

#[component]
fn LaporanGuruBody() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { analisis_data().await });
    let extra = Resource::new(|| (), |_| async move { laporan_guru_extra_data().await });
    view! {
        <Suspense fallback=|| view! { <div class="ppm-empty">"Memuat…"</div> }>
            {move || {
                data.get()
                    .map(|res| match res {
                        Ok(d) => view! { <GuruReport d=d extra=extra /> }.into_any(),
                        Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                    })
            }}
        </Suspense>
    }
}

#[component]
fn GuruReport(
    d: AnalisisData,
    extra: Resource<Result<crate::models::LaporanGuruExtra, ServerFnError>>,
) -> impl IntoView {
    let scope_note = if d.is_dewan { "Seluruh pesantren" } else { "Kelas yang Anda ampu" };
    view! {
        <p class="text-body-sm text-on-surface-variant">"Laporan Kelas Akademik • " {scope_note}</p>

        <div class="grid grid-cols-3 gap-3">
            <div class="ppm-card p-4">
                <p class="text-2xl font-bold text-on-background">{format!("{}%", d.attendance_pct)}</p>
                <p class="text-[11px] text-on-surface-variant mt-1">"Rata-rata Kehadiran"</p>
            </div>
            <div class="ppm-card p-4">
                <p class="text-2xl font-bold text-on-background">{d.avg_points}</p>
                <p class="text-[11px] text-on-surface-variant mt-1">"Rata-rata Poin"</p>
            </div>
            <div class="ppm-card p-4">
                <p class="text-2xl font-bold text-on-background">{d.sessions_verified}</p>
                <p class="text-[11px] text-on-surface-variant mt-1">"Absensi Terverifikasi"</p>
            </div>
        </div>

        // ── Santri Teladan (ranking hafalan "Mengaji") ────────────────────
        <div class="ppm-card p-4">
            <h3 class="text-body-lg font-bold text-on-background mb-3 flex items-center gap-2">
                <span class="material-symbols-outlined text-[18px] text-primary">"military_tech"</span>
                "Santri Teladan — Hafalan"
            </h3>
            <Suspense fallback=|| view! { <p class="text-body-sm text-on-surface-variant">"Memuat…"</p> }>
                {move || {
                    extra
                        .get()
                        .map(|res| match res {
                            Ok(e) if e.hafalan_top.is_empty() => {
                                view! {
                                    <p class="text-body-sm text-on-surface-variant py-2">
                                        "Belum ada setoran hafalan tercatat."
                                    </p>
                                }
                                    .into_any()
                            }
                            Ok(e) => {
                                view! {
                                    <div class="space-y-2.5 md:grid md:grid-cols-2 md:gap-3 md:space-y-0">
                                        {e.hafalan_top
                                            .into_iter()
                                            .enumerate()
                                            .map(|(i, s)| {
                                                view! {
                                                    <div class="flex items-center gap-3 bg-surface-container rounded-xl p-3">
                                                        <span class="w-7 h-7 rounded-full bg-primary text-on-primary flex items-center justify-center text-[11px] font-bold shrink-0">
                                                            {i + 1}
                                                        </span>
                                                        <div class="flex-1 min-w-0">
                                                            <p class="text-body-sm font-semibold text-on-background truncate">
                                                                {s.name}
                                                            </p>
                                                            <p class="text-[11px] text-on-surface-variant truncate">
                                                                {format!("{} • {} Juz", s.class_name, s.juz_count)}
                                                            </p>
                                                        </div>
                                                        <span class="text-body-sm font-bold text-primary shrink-0">
                                                            {format!("{} pts", s.points)}
                                                        </span>
                                                    </div>
                                                }
                                            })
                                            .collect_view()}
                                    </div>
                                }
                                    .into_any()
                            }
                            Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                        })
                }}
            </Suspense>
        </div>

        // ── Ranking kelas ──────────────────────────────────────────────────
        <div class="ppm-card p-4">
            <h3 class="text-body-lg font-bold text-on-background mb-3">"Kehadiran per Kelas"</h3>
            {if d.class_ranking.is_empty() {
                view! { <p class="text-body-sm text-on-surface-variant py-3">"Belum ada data."</p> }.into_any()
            } else {
                view! {
                    <div class="space-y-2 md:grid md:grid-cols-2 md:gap-2 md:space-y-0">
                        {d
                            .class_ranking
                            .into_iter()
                            .map(|r| {
                                view! {
                                    <div class="flex items-center gap-3 py-1.5">
                                        <div class="flex-1 min-w-0">
                                            <p class="text-body-sm font-semibold text-on-background truncate">
                                                {r.name}
                                            </p>
                                            <p class="text-[11px] text-on-surface-variant">
                                                {format!("{} santri", r.santri_count)}
                                            </p>
                                        </div>
                                        <span class="text-body-sm font-bold text-on-background">
                                            {format!("{}%", r.attendance_pct)}
                                        </span>
                                    </div>
                                }
                            })
                            .collect_view()}
                    </div>
                }
                    .into_any()
            }}
        </div>
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ORANG TUA & SANTRI — kartu hasil bersama (poin, hafalan, prestasi)
// ═══════════════════════════════════════════════════════════════════════════

pub(crate) fn attendance_card(hadir: i64, izin: i64, alpa: i64, pct: i64) -> impl IntoView {
    view! {
        <div class="ppm-card p-4">
            <h3 class="text-body-lg font-bold text-on-background mb-3">"Kartu Hasil — Absensi"</h3>
            <div class="grid grid-cols-3 gap-2">
                <div class="bg-success/10 rounded-xl p-3 text-center">
                    <p class="text-xl font-bold text-success">{hadir}</p>
                    <p class="text-[10px] font-bold tracking-wider text-success uppercase mt-0.5">"Hadir"</p>
                </div>
                <div class="bg-warning/10 rounded-xl p-3 text-center">
                    <p class="text-xl font-bold text-warning">{izin}</p>
                    <p class="text-[10px] font-bold tracking-wider text-warning uppercase mt-0.5">"Izin"</p>
                </div>
                <div class="bg-error-container rounded-xl p-3 text-center">
                    <p class="text-xl font-bold text-error">{alpa}</p>
                    <p class="text-[10px] font-bold tracking-wider text-error uppercase mt-0.5">"Alpa"</p>
                </div>
            </div>
            <div class="flex items-center justify-between mt-3 text-body-sm">
                <span class="text-on-surface-variant">"Persentase Kehadiran"</span>
                <span class="font-bold text-on-background">{format!("{pct}%")}</span>
            </div>
            <div class="w-full h-2 bg-surface-container-high rounded-full overflow-hidden mt-1.5">
                <div class=format!("h-full bar-grow {}", pct_bar(pct)) style=format!("width: {pct}%")></div>
            </div>
        </div>
    }
}

pub(crate) fn hafalan_card(hafalan: Vec<HafalanItem>, juz_count: i64) -> impl IntoView {
    view! {
        <div class="ppm-card p-4">
            <div class="flex items-center justify-between mb-3">
                <h3 class="text-body-lg font-bold text-on-background flex items-center gap-2">
                    <span class="material-symbols-outlined text-[18px] text-primary">"auto_stories"</span>
                    "Capaian Hafalan"
                </h3>
                <span class="ppm-chip bg-secondary-container text-primary">{format!("{juz_count} Juz")}</span>
            </div>
            {if hafalan.is_empty() {
                view! {
                    <p class="text-body-sm text-on-surface-variant py-2">"Belum ada setoran hafalan tercatat."</p>
                }
                    .into_any()
            } else {
                view! {
                    <div class="divide-y divide-outline-variant/40">
                        {hafalan
                            .into_iter()
                            .map(|h| {
                                let range = if h.ayat_range.is_empty() {
                                    h.surah.clone()
                                } else {
                                    format!("{} ({})", h.surah, h.ayat_range)
                                };
                                let badge_cls = match h.quality.as_str() {
                                    "perlu_perbaikan" => "ppm-chip-sm bg-warning/10 text-warning",
                                    "mengulang" => "ppm-chip-sm bg-error-container text-error",
                                    _ => "ppm-chip-sm bg-success/10 text-success",
                                };
                                view! {
                                    <div class="py-2.5 flex items-center justify-between gap-2">
                                        <div class="min-w-0">
                                            <p class="text-body-sm font-semibold text-on-background truncate">
                                                {range}
                                            </p>
                                            <p class="text-[11px] text-on-surface-variant">
                                                {format!("{} • {}", h.date_label, h.recorded_by)}
                                            </p>
                                        </div>
                                        <span class=badge_cls>{h.quality_label}</span>
                                    </div>
                                }
                            })
                            .collect_view()}
                    </div>
                }
                    .into_any()
            }}
        </div>
    }
}

/// Kartu Prestasi & Pelanggaran. `recent_only` = isinya sudah dibatasi beberapa
/// hari terakhir di service (rapor santri) → label & pesan kosong menyesuaikan,
/// supaya santri tak mengira catatannya hilang padahal cuma di luar rentang.
pub(crate) fn points_lists(
    prestasi: Vec<LaporanPointItem>,
    pelanggaran: Vec<LaporanPointItem>,
    recent_only: bool,
) -> impl IntoView {
    let periode = if recent_only {
        format!(" ({} hari terakhir)", crate::models::RECENT_POINTS_DAYS)
    } else {
        String::new()
    };
    view! {
        <div class="ppm-card p-4">
            <h3 class="text-body-lg font-bold text-on-background mb-3 flex items-center gap-2">
                <span class="material-symbols-outlined text-[18px] text-success">"emoji_events"</span>
                {format!("Prestasi Terbaru{periode}")}
            </h3>
            {if prestasi.is_empty() {
                let t = if recent_only {
                    format!(
                        "Belum ada catatan prestasi dalam {} hari terakhir.",
                        crate::models::RECENT_POINTS_DAYS
                    )
                } else {
                    "Belum ada catatan prestasi.".to_string()
                };
                view! { <p class="text-body-sm text-on-surface-variant py-2">{t}</p> }.into_any()
            } else {
                view! {
                    <div class="space-y-2">
                        {prestasi
                            .into_iter()
                            .map(|p| {
                                view! {
                                    <div class="flex items-center justify-between gap-2 bg-success/10 rounded-xl p-3">
                                        <div class="min-w-0">
                                            <p class="text-body-sm font-semibold text-on-background truncate">
                                                {p.reason}
                                            </p>
                                            <p class="text-[11px] text-on-surface-variant">{p.date_label}</p>
                                        </div>
                                        <span class="text-body-sm font-bold text-success shrink-0">
                                            {format!("+{}", p.delta)}
                                        </span>
                                    </div>
                                }
                            })
                            .collect_view()}
                    </div>
                }
                    .into_any()
            }}
        </div>

        <div class="ppm-card p-4">
            <h3 class="text-body-lg font-bold text-on-background mb-3 flex items-center gap-2">
                <span class="material-symbols-outlined text-[18px] text-error">"gpp_bad"</span>
                {format!("Pelanggaran & Teguran{periode}")}
            </h3>
            {if pelanggaran.is_empty() {
                let t = if recent_only {
                    format!(
                        "Tidak ada pelanggaran dalam {} hari terakhir. Alhamdulillah.",
                        crate::models::RECENT_POINTS_DAYS
                    )
                } else {
                    "Tidak ada pelanggaran tercatat. Alhamdulillah.".to_string()
                };
                view! { <p class="text-body-sm text-on-surface-variant py-2">{t}</p> }.into_any()
            } else {
                view! {
                    <div class="space-y-2">
                        {pelanggaran
                            .into_iter()
                            .map(|p| {
                                view! {
                                    <div class="flex items-center justify-between gap-2 bg-error-container/50 rounded-xl p-3">
                                        <div class="min-w-0">
                                            <p class="text-body-sm font-semibold text-on-background truncate">
                                                {p.reason}
                                            </p>
                                            <p class="text-[11px] text-on-surface-variant">{p.date_label}</p>
                                        </div>
                                        <span class="text-body-sm font-bold text-error shrink-0">{p.delta}</span>
                                    </div>
                                }
                            })
                            .collect_view()}
                    </div>
                }
                    .into_any()
            }}
        </div>
    }
}

pub(crate) fn gate_status_card(gate_status: &str, gate_at_label: &str) -> impl IntoView {
    let (label, icon, cls) = if gate_status == "out" {
        ("Sedang di Luar Pondok", "door_open", "bg-error-container text-error")
    } else {
        ("Sedang di Dalam Pondok", "home", "bg-success/10 text-success")
    };
    let since = gate_at_label.to_string();
    view! {
        <div class=format!("ppm-card p-4 flex items-center gap-3 {cls}")>
            <span class="material-symbols-outlined text-2xl">{icon}</span>
            <div class="min-w-0">
                <p class="font-bold truncate">{label}</p>
                <p class="text-[11px] opacity-80 truncate">{format!("Sejak {since}")}</p>
            </div>
        </div>
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ORANG TUA — Laporan Santri
// ═══════════════════════════════════════════════════════════════════════════

#[component]
fn LaporanOrtuBody() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { laporan_ortu_data(None).await });
    view! {
        <Suspense fallback=|| view! { <div class="ppm-empty">"Memuat…"</div> }>
            {move || {
                data.get()
                    .map(|res| match res {
                        Ok(d) => view! { <OrtuReport d=d /> }.into_any(),
                        Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                    })
            }}
        </Suspense>
    }
}

#[component]
fn OrtuReport(d: LaporanOrtuData) -> impl IntoView {
    view! {
        <div class="spiritual-gradient rounded-2xl p-5 text-on-primary shadow-lg shadow-primary/20">
            <p class="text-[11px] font-bold tracking-[0.2em] opacity-80">"LAPORAN PERKEMBANGAN SANTRI"</p>
            <h2 class="text-display-md mt-1">{d.child_name}</h2>
            <p class="text-body-sm opacity-85 mt-1">{format!("{} • NIS {}", d.class_name, d.nis)}</p>
            <div class="flex items-center justify-between mt-3">
                <span class="text-[11px] font-bold tracking-[0.15em] opacity-80">"TOTAL POIN"</span>
                <span class="text-display-md">{d.points}</span>
            </div>
        </div>
        <div class="md:grid md:grid-cols-2 md:gap-5 md:items-start space-y-5 md:space-y-0">
            <div class="space-y-5">
                {attendance_card(d.hadir, d.izin, d.alpa, d.attendance_pct)}
                {gate_status_card(&d.gate_status, &d.gate_at_label)}
                {hafalan_card(d.hafalan, d.juz_count)}
            </div>
            <div class="space-y-5">{points_lists(d.prestasi, d.pelanggaran, true)}</div>
        </div>
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SANTRI — Rapor Pribadi
// ═══════════════════════════════════════════════════════════════════════════

#[component]
fn LaporanSantriBody() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { laporan_santri_data().await });
    view! {
        <Suspense fallback=|| view! { <div class="ppm-empty">"Memuat…"</div> }>
            {move || {
                data.get()
                    .map(|res| match res {
                        Ok(d) => view! { <SantriReport d=d /> }.into_any(),
                        Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                    })
            }}
        </Suspense>
    }
}

#[component]
fn SantriReport(d: LaporanSantriData) -> impl IntoView {
    view! {
        <div class="spiritual-gradient rounded-2xl p-5 text-on-primary shadow-lg shadow-primary/20">
            <p class="text-[11px] font-bold tracking-[0.2em] opacity-80">"RAPOR PRIBADI"</p>
            <div class="flex items-center justify-between mt-1">
                <span class="text-body-sm opacity-85">"Total Poin"</span>
                <span class="text-display-md">{d.points}</span>
            </div>
        </div>
        <div class="md:grid md:grid-cols-2 md:gap-5 md:items-start space-y-5 md:space-y-0">
            <div class="space-y-5">
                {attendance_card(d.hadir, d.izin, d.alpa, d.attendance_pct)}
                {gate_status_card(&d.gate_status, &d.gate_at_label)}
                {hafalan_card(d.hafalan, d.juz_count)}
            </div>
            <div class="space-y-5">{points_lists(d.prestasi, d.pelanggaran, true)}</div>
        </div>
    }
}
