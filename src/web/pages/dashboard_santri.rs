//! web/pages/dashboard_santri.rs — Beranda Santri (mockup stitch: Poin Saya,
//! Jadwal Kelas Mendatang, Riwayat Terakhir ber-border warna, Progress bulan).
//! Data ASLI dari DB via server fn `santri_home`.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{AttendanceItem, SantriHome};
use crate::web::api::{connection_requests, respond_connection_action, santri_home};
use crate::web::components::{FetchError, DeviceFrame, MobileNav, NotifBell, NAV_SANTRI};

/// Warna aksen per jenis kehadiran (border kiri kartu + ikon).
fn kind_colors(kind: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    // (border_css, icon, icon_wrap_cls, badge_cls)
    match kind {
        "late" => (
            "border-left:4px solid #f59e0b",
            "schedule",
            "w-11 h-11 rounded-full flex items-center justify-center shrink-0 bg-warning/10 text-warning",
            "px-3 py-1.5 rounded-full text-label-md whitespace-nowrap bg-warning/10 text-warning",
        ),
        "permit" | "sick" => (
            "border-left:4px solid #2563eb",
            "medical_services",
            "w-11 h-11 rounded-full flex items-center justify-center shrink-0 bg-info/10 text-info",
            "px-3 py-1.5 rounded-full text-label-md whitespace-nowrap bg-info/10 text-info",
        ),
        "absent" => (
            "border-left:4px solid #dc2626",
            "close",
            "w-11 h-11 rounded-full flex items-center justify-center shrink-0 bg-error-container text-error",
            "px-3 py-1.5 rounded-full text-label-md whitespace-nowrap bg-error-container text-error",
        ),
        _ => (
            "border-left:4px solid #059669",
            "login",
            "w-11 h-11 rounded-full flex items-center justify-center shrink-0 bg-secondary-container text-primary",
            "px-3 py-1.5 rounded-full text-label-md whitespace-nowrap bg-success/10 text-success",
        ),
    }
}

#[component]
pub fn SantriDashboardPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { santri_home().await });
    // Sheet QR absensi (dibuka FAB).
    let show_qr = RwSignal::new(false);

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
        <Title text="Beranda Santri — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto">
                <Suspense fallback=|| {
                    view! {
                        <div class="px-5 pt-6 space-y-4 animate-pulse">
                            <div class="h-12 bg-surface-container rounded-xl"></div>
                            <div class="h-44 bg-surface-container rounded-2xl"></div>
                            <div class="h-40 bg-surface-container rounded-2xl"></div>
                            <div class="h-24 bg-surface-container rounded-2xl"></div>
                        </div>
                    }
                }>
                    {move || {
                        data.get()
                            .map(|res| match res {
                                Ok(home) => view! { <HomeContent home=home /> }.into_any(),
                                Err(e) => view! { <FetchError err=e.to_string() /> }.into_any()
                            })
                    }}
                </Suspense>

                // FAB QR (scan absensi) → buka bottom-sheet QR
                <button
                    class="ppm-fab fixed bottom-24 right-5 w-14 h-14 spiritual-gradient rounded-2xl flex items-center justify-center text-on-primary shadow-lg shadow-primary/30 z-20"
                    on:click=move |_| show_qr.set(true)
                    aria-label="Tampilkan QR absensi"
                >
                    <span class="material-symbols-outlined text-3xl">"qr_code_2"</span>
                </button>

                // ── Bottom-sheet QR absensi ─────────────────────────────────
                {move || {
                    show_qr
                        .get()
                        .then(|| {
                            view! {
                                <div
                                    class="fixed inset-0 z-40 bg-black/45 fade-in"
                                    on:click=move |_| show_qr.set(false)
                                ></div>
                                <div class="fixed bottom-0 inset-x-0 z-50 max-w-md mx-auto bg-surface rounded-t-3xl p-6 pb-10 sheet-in">
                                    <div class="w-10 h-1.5 bg-outline-variant rounded-full mx-auto mb-5"></div>
                                    <h3 class="text-headline-sm text-on-background text-center">
                                        "QR Absensi Saya"
                                    </h3>
                                    <div class="w-52 h-52 mx-auto mt-5 bg-surface-container-lowest border-2 border-outline-variant rounded-2xl flex items-center justify-center">
                                        <span class="material-symbols-outlined text-[150px] text-primary">
                                            "qr_code_2"
                                        </span>
                                    </div>
                                    <p class="text-body-sm text-on-surface-variant text-center mt-4">
                                        "Tunjukkan kode ini ke perangkat pemindai di gerbang."
                                    </p>
                                    <p class="text-[11px] text-on-surface-variant/70 text-center mt-1">
                                        "(QR unik per-santri segera hadir — gunakan kartu RFID)"
                                    </p>
                                    <button
                                        class="w-full mt-6 py-3.5 bg-primary text-on-primary rounded-xl font-semibold"
                                        on:click=move |_| show_qr.set(false)
                                    >
                                        "Tutup"
                                    </button>
                                </div>
                            }
                        })
                }}

                <MobileNav items=NAV_SANTRI active="/santri" />
            </div>
        </DeviceFrame>
    }
}

#[component]
fn HomeContent(home: SantriHome) -> impl IntoView {
    let initial = home.name.chars().next().unwrap_or('S').to_string();
    let first_name = home
        .name
        .split_whitespace()
        .next()
        .unwrap_or("Santri")
        .to_string();
    let pct = home.month_pct;
    let month_pts = home.month_points;

    // Toggle pengingat jadwal (interaksi lokal).
    let reminder = RwSignal::new(false);

    view! {
        <div class="px-5 pt-6 space-y-6 stagger">
            // ── Header ──────────────────────────────────────────────────────
            <header class="flex items-center justify-between">
                <div class="flex items-center gap-3">
                    <div class="w-12 h-12 rounded-full bg-secondary-container flex items-center justify-center text-primary font-bold ring-2 ring-primary/20">
                        {initial}
                    </div>
                    <div>
                        <p class="text-body-sm text-on-surface-variant">"Assalamualaikum,"</p>
                        <p class="text-headline-sm text-on-background">{first_name} "!"</p>
                    </div>
                </div>
                <NotifBell />
            </header>

            // ── Kartu Poin ──────────────────────────────────────────────────
            <div class="spiritual-gradient rounded-2xl p-6 text-on-primary relative overflow-hidden shadow-lg shadow-primary/20">
                <span class="material-symbols-outlined absolute -right-4 -bottom-4 text-[120px] opacity-10">
                    "qr_code_2"
                </span>
                <p class="text-label-md opacity-80">"POIN SAYA"</p>
                <div class="flex items-end gap-2 mt-1">
                    // data-count → angka beranimasi naik 0→target saat terlihat.
                    <span class="text-5xl font-bold leading-none" data-count=home.points.to_string()>
                        {home.points}
                    </span>
                    <span class="text-body-md opacity-80 mb-1">"Poin"</span>
                </div>
                <div class="flex items-center justify-between gap-3 mt-4">
                    <span class="inline-flex items-center gap-1.5 bg-white/15 px-3 py-1.5 rounded-full text-label-md">
                        <span class="material-symbols-outlined text-[16px]">"star"</span>
                        {if home.points >= 500 { "Mahasiswa Teladan" } else { "Terus Semangat" }}
                    </span>
                    <span class="text-body-sm opacity-90">
                        {format!("{month_pts:+} poin bulan ini")}
                    </span>
                </div>
            </div>

            // ── Permintaan koneksi orang tua (setujui/tolak oleh SANTRI) ────
            <ConnRequestsSection />

            // ── Jadwal Kelas Mendatang ──────────────────────────────────────
            <section>
                <div class="flex items-center justify-between gap-3 mb-3">
                    <h2 class="text-headline-sm text-on-background leading-tight">
                        "Jadwal Kelas Mendatang"
                    </h2>
                    <a href="#" class="text-label-md text-primary font-bold text-right shrink-0">
                        "Lihat Semua"<br/>"Kalender"
                    </a>
                </div>
                {match home.schedule {
                    Some(s) => {
                        view! {
                            <div
                                class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4"
                                style="border-left:4px solid #059669"
                            >
                                <div class="flex gap-3">
                                    <div class="w-12 h-12 rounded-xl bg-secondary-container flex items-center justify-center text-primary shrink-0">
                                        <span class="material-symbols-outlined">"menu_book"</span>
                                    </div>
                                    <div class="flex-1 min-w-0">
                                        <p class="text-body-lg font-semibold text-on-background leading-snug">
                                            {s.title}
                                        </p>
                                        <p class="text-body-sm text-on-surface-variant mt-1">{s.class_name}</p>
                                        <div class="flex flex-wrap items-center gap-x-4 gap-y-1 mt-2 text-body-sm text-on-surface-variant">
                                            <span class="flex items-center gap-1">
                                                <span class="material-symbols-outlined text-[16px]">"schedule"</span>
                                                {s.time_label}
                                            </span>
                                        </div>
                                    </div>
                                </div>
                                // Toggle: Set Pengingat ↔ Pengingat Aktif ✓
                                <button
                                    class=move || {
                                        if reminder.get() {
                                            "w-full mt-4 py-3 bg-secondary-container text-primary rounded-xl text-body-md font-semibold border border-primary/30 transition-colors flex items-center justify-center gap-2"
                                        } else {
                                            "w-full mt-4 py-3 bg-primary text-on-primary rounded-xl text-body-md font-semibold hover:bg-primary-container transition-colors flex items-center justify-center gap-2"
                                        }
                                    }
                                    on:click=move |_| reminder.update(|r| *r = !*r)
                                >
                                    {move || {
                                        if reminder.get() {
                                            view! {
                                                <span class="material-symbols-outlined text-xl">"notifications_active"</span>
                                                "Pengingat Aktif"
                                            }
                                                .into_any()
                                        } else {
                                            view! { "Set Pengingat" }.into_any()
                                        }
                                    }}
                                </button>
                            </div>
                        }
                            .into_any()
                    }
                    None => {
                        view! {
                            <div class="bg-surface-container rounded-2xl p-5 text-center text-body-sm text-on-surface-variant">
                                "Belum ada jadwal kelas aktif."
                            </div>
                        }
                            .into_any()
                    }
                }}
            </section>

            // ── Riwayat Terakhir ────────────────────────────────────────────
            <section class="space-y-3">
                <h2 class="text-headline-sm text-on-background">"Riwayat Terakhir"</h2>
                {if home.recent.is_empty() {
                    view! {
                        <div class="bg-surface-container rounded-2xl p-5 text-center text-body-sm text-on-surface-variant">
                            "Belum ada catatan kehadiran."
                        </div>
                    }
                        .into_any()
                } else {
                    home.recent
                        .into_iter()
                        .map(|it| view! { <AttendanceRow item=it /> })
                        .collect_view()
                        .into_any()
                }}
                <a
                    href="/riwayat"
                    class="block w-full py-3.5 border-2 border-dashed border-outline-variant rounded-2xl text-body-md text-on-surface-variant hover:border-primary hover:text-primary transition-colors text-center"
                >
                    "Lihat Semua Riwayat"
                </a>
            </section>

            // ── Progress bulan ini ──────────────────────────────────────────
            <section class="bg-surface-container rounded-2xl p-5">
                <h3 class="text-body-lg font-bold text-on-background">"Progress Kehadiran Bulan Ini"</h3>
                {match pct {
                    Some(p) => {
                        let width = format!("width:{}%", p.clamp(0, 100));
                        view! {
                            <div class="flex items-center justify-between mt-3 text-body-sm">
                                <span class="text-on-surface-variant">"Target Kehadiran (95%)"</span>
                                <span class="font-bold text-on-background">{p} "%"</span>
                            </div>
                            <div class="w-full h-3 bg-secondary-fixed-dim rounded-full mt-2 overflow-hidden">
                                <div class="h-full bg-primary rounded-full bar-grow" style=width></div>
                            </div>
                        }
                            .into_any()
                    }
                    None => {
                        view! {
                            <p class="text-body-sm text-on-surface-variant mt-2">
                                "Belum ada catatan bulan ini."
                            </p>
                        }
                            .into_any()
                    }
                }}
                <p class="text-body-sm italic text-on-surface-variant mt-4">
                    "\"Sebaik-baik manusia adalah yang paling bermanfaat bagi orang lain.\""
                </p>
            </section>
        </div>
    }
}

/// Permintaan koneksi ORANG TUA yang menunggu persetujuan santri ini.
/// Hanya tampil bila ada permintaan.
#[component]
fn ConnRequestsSection() -> impl IntoView {
    let reqs = Resource::new(|| (), |_| async move { connection_requests().await });
    let busy = RwSignal::new(false);

    let respond = move |id: i64, approve: bool| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            let _ = respond_connection_action(id, approve).await;
            busy.set(false);
            reqs.refetch();
        });
    };

    view! {
        <Suspense fallback=|| ()>
            {move || {
                reqs.get()
                    .and_then(|r| r.ok())
                    .filter(|list| !list.is_empty())
                    .map(|list| {
                        view! {
                            <section class="bg-secondary-container/50 border border-secondary-container rounded-2xl p-4 anim-in">
                                <div class="flex items-center gap-2 text-primary mb-3">
                                    <span class="material-symbols-outlined pulse-dot">"family_restroom"</span>
                                    <h2 class="text-body-lg font-bold">"Permintaan Koneksi Orang Tua"</h2>
                                </div>
                                <div class="space-y-3">
                                    {list
                                        .into_iter()
                                        .map(|r| {
                                            let id = r.id;
                                            let initial = r.parent_name.chars().next().unwrap_or('O').to_string();
                                            view! {
                                                <div class="bg-surface-container-lowest rounded-xl p-3">
                                                    <div class="flex items-center gap-3">
                                                        <div class="w-10 h-10 rounded-full bg-secondary-container flex items-center justify-center text-primary font-bold shrink-0">
                                                            {initial}
                                                        </div>
                                                        <div class="flex-1 min-w-0">
                                                            <p class="text-body-md font-semibold text-on-background truncate">
                                                                {r.parent_name}
                                                            </p>
                                                            <p class="text-body-sm text-on-surface-variant">
                                                                "Ingin memantau kehadiranmu • " {r.since_label}
                                                            </p>
                                                        </div>
                                                    </div>
                                                    <div class="grid grid-cols-2 gap-2 mt-3">
                                                        <button
                                                            class="py-2 rounded-lg border border-error/40 text-error text-body-sm font-semibold disabled:opacity-50"
                                                            disabled=move || busy.get()
                                                            on:click=move |_| respond(id, false)
                                                        >
                                                            "Tolak"
                                                        </button>
                                                        <button
                                                            class="py-2 rounded-lg bg-primary text-on-primary text-body-sm font-semibold disabled:opacity-50"
                                                            disabled=move || busy.get()
                                                            on:click=move |_| respond(id, true)
                                                        >
                                                            "Setujui"
                                                        </button>
                                                    </div>
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            </section>
                        }
                    })
            }}
        </Suspense>
    }
}

#[component]
fn AttendanceRow(item: AttendanceItem) -> impl IntoView {
    let (border, icon, wrap_cls, badge_cls) = kind_colors(&item.kind);
    view! {
        <div
            class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-3 flex items-center gap-3 card-hover"
            style=border
        >
            <div class=wrap_cls>
                <span class="material-symbols-outlined">{icon}</span>
            </div>
            <div class="flex-1 min-w-0">
                <p class="text-body-md font-semibold text-on-background">{item.title}</p>
                <p class="text-body-sm text-on-surface-variant">{item.sub}</p>
            </div>
            <span class=badge_cls>{item.badge}</span>
        </div>
    }
}
