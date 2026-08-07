//! web/pages/dashboard_santri.rs — Beranda Santri (mockup stitch: Poin Saya,
//! Jadwal Kelas Mendatang, Riwayat Terakhir ber-border warna, Progress bulan).
//! Data ASLI dari DB via server fn `santri_home`.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{AttendanceItem, SantriHome, SessionUser};
use crate::web::api::{connection_requests, respond_connection_action, santri_home};
use crate::web::components::{DeviceFrame, FetchError, NotifBell, Sheet};

/// Warna aksen per jenis kehadiran (border kiri kartu + ikon).
fn kind_colors(kind: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    // (border_cls, icon, icon_wrap_cls, badge_cls) — garis aksennya kelas dari
    // palet, bukan hex yang ditulis ulang di tiap halaman.
    match kind {
        "late" => (
            "ppm-accent-warning",
            "schedule",
            "w-11 h-11 rounded-full flex items-center justify-center shrink-0 bg-warning/10 text-warning",
            "px-3 py-1.5 rounded-full text-label-md whitespace-nowrap bg-warning/10 text-warning",
        ),
        "permit" | "sick" => (
            "ppm-accent-info",
            "medical_services",
            "w-11 h-11 rounded-full flex items-center justify-center shrink-0 bg-info/10 text-info",
            "px-3 py-1.5 rounded-full text-label-md whitespace-nowrap bg-info/10 text-info",
        ),
        "absent" => (
            "ppm-accent-error",
            "close",
            "w-11 h-11 rounded-full flex items-center justify-center shrink-0 bg-error-container text-error",
            "px-3 py-1.5 rounded-full text-label-md whitespace-nowrap bg-error-container text-error",
        ),
        _ => (
            "ppm-accent-success",
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
            if crate::web::components::is_auth_error(&msg) {
                #[cfg(target_arch = "wasm32")]
                if let Some(w) = web_sys::window() {
                    let _ = w.location().replace("/login");
                }
            }
        }
    });

    view! {
        <Title text="Beranda Santri — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <Suspense fallback=|| {
                    view! {
                        <div class="px-5 pt-6 animate-pulse">
                            <div class="h-12 bg-surface-container rounded-xl"></div>
                            <div class="mt-4 grid gap-4 md:grid-cols-3 md:items-start">
                                <div class="md:col-span-2 space-y-4">
                                    <div class="h-44 bg-surface-container rounded-2xl"></div>
                                    <div class="h-40 bg-surface-container rounded-2xl"></div>
                                </div>
                                <div class="h-56 bg-surface-container rounded-2xl"></div>
                            </div>
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
                    aria-label="Cara absensi gerbang"
                >
                    <span class="material-symbols-outlined text-3xl">"nfc"</span>
                </button>

                // ── Bottom-sheet QR absensi ─────────────────────────────────
                {move || {
                    show_qr
                        .get()
                        .then(|| {
                            view! {
                                <Sheet
                                    title="Absensi Gerbang"
                                    on_close=move || show_qr.set(false)
                                    center_title=true
                                >
                                    // Dulu di sini tampil ikon QR besar dengan kalimat
                                    // "tunjukkan kode ini ke pemindai" — padahal barisnya
                                    // sendiri mengaku QR per-santri "segera hadir". Kode itu
                                    // hiasan: dipindai pasti gagal, dan santri baru tahu
                                    // setelah berdiri di depan gerbang. Diganti keterangan
                                    // jujur + cara yang benar-benar berlaku sekarang.
                                    // Dibatasi lebarnya + rata kiri di layar
                                    // lebar. Sheet desktop selebar 40rem, dan
                                    // isi sependek ini bila dipusatkan hanya
                                    // menghasilkan segumpal teks kecil yang
                                    // mengambang di tengah kotak besar.
                                    <div class="mt-4 md:max-w-md md:mx-auto">
                                        <div class="flex flex-col items-center text-center gap-3 md:flex-row md:items-start md:text-left md:gap-4">
                                            <span class="w-16 h-16 shrink-0 rounded-2xl bg-secondary-container flex items-center justify-center">
                                                <span class="material-symbols-outlined text-[32px] text-primary">
                                                    "nfc"
                                                </span>
                                            </span>
                                            <div class="min-w-0">
                                                <p class="text-body-md font-semibold text-on-background">
                                                    "Absensi pakai kartu RFID"
                                                </p>
                                                <p class="text-body-sm text-on-surface-variant mt-1">
                                                    "Tempelkan kartumu ke pemindai di gerbang. QR per-santri belum tersedia."
                                                </p>
                                            </div>
                                        </div>
                                        <p class="text-[11px] text-on-surface-variant/70 mt-4 text-center md:text-left">
                                            "Kartu hilang atau belum punya? Hubungi pengelola."
                                        </p>
                                        <button
                                            class="w-full mt-5 py-3.5 bg-primary text-on-primary rounded-xl font-semibold press"
                                            on:click=move |_| show_qr.set(false)
                                        >
                                            "Tutup"
                                        </button>
                                    </div>
                                </Sheet>
                            }
                        })
                }}

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

            // ── Alat Keuangan (HANYA santri_finance) ────────────────────────
            <FinanceTools />

            // Desktop: jadwal+riwayat kolom utama (kiri), progress jadi
            // sidebar kanan — konten sama, cuma disusun 2 kolom di layar lebar.
            <div class="space-y-6 md:space-y-0 md:grid md:grid-cols-3 md:gap-6 md:items-start">
            <div class="md:col-span-2 space-y-6">
            // ── Jadwal Kelas Mendatang ──────────────────────────────────────
            <section>
                <div class="flex items-center justify-between gap-3 mb-3">
                    <h2 class="text-headline-sm text-on-background leading-tight">
                        "Jadwal Kelas Mendatang"
                    </h2>
                    <a href="/kalender" class="text-label-md text-primary font-bold text-right shrink-0">
                        "Lihat Semua"<br/>"Kalender"
                    </a>
                </div>
                {match home.schedule {
                    Some(s) => {
                        view! {
                            <div class="ppm-card p-4 ppm-accent-success">
                                <div class="flex gap-3">
                                    <div class="w-12 h-12 ppm-tile">
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
                <div class="grid grid-cols-2 gap-2">
                    <a
                        href="/riwayat"
                        class="block w-full py-3.5 border-2 border-dashed border-outline-variant rounded-2xl text-body-md text-on-surface-variant hover:border-primary hover:text-primary transition-colors text-center"
                    >
                        "Riwayat"
                    </a>
                    <a
                        href="/tagihan-saya"
                        class="flex items-center justify-center gap-2 w-full py-3.5 border-2 border-dashed border-outline-variant rounded-2xl text-body-md text-on-surface-variant hover:border-primary hover:text-primary transition-colors text-center"
                    >
                        <span class="material-symbols-outlined text-[20px]">"payment"</span>
                        "Pembayaran"
                    </a>
                </div>
            </section>
            </div>

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
        </div>
    }
}

/// Alat Keuangan — HANYA untuk peran `santri_finance` (santri yang diberi
/// amanah mengelola pembayaran). Pola sama dengan grid "Alat Administrasi" di
/// beranda ketua/staf: akses lewat tombol di beranda, BUKAN item navbar
/// (navbar santri_finance sengaja dibuat identik dengan santri biasa).
#[component]
fn FinanceTools() -> impl IntoView {
    let session = use_context::<Resource<Option<SessionUser>>>();
    let is_finance = move || {
        session
            .and_then(|s| s.get())
            .flatten()
            .map(|u| u.role == "santri_finance")
            .unwrap_or(false)
    };

    view! {
        // Sama seperti SemesterManager: baca resource sesi di dalam Suspense.
        <Suspense fallback=|| ()>
        // Pintu ke "Kelas Saya" — kelas yang diikuti, kurikulumnya, materi
        // yang sedang dibahas, dan teman sekelas. Lewat kartu, bukan navbar:
        // navbar santri sudah enam item dan menambah satu lagi membuatnya
        // sesak di layar ponsel.
        <a
            href="/kelas-saya"
            class="ppm-card p-4 flex items-center gap-3 press hover:border-primary transition-colors"
        >
            <span class="w-11 h-11 ppm-tile shrink-0">
                <span class="material-symbols-outlined">"school"</span>
            </span>
            <div class="flex-1 min-w-0">
                <p class="text-body-md font-semibold text-on-background">"Kelas Saya"</p>
                <p class="text-[11px] text-on-surface-variant">
                    "Kurikulum, materi sekarang, wali kelas & teman sekelas"
                </p>
            </div>
            <span class="material-symbols-outlined text-on-surface-variant shrink-0">
                "chevron_right"
            </span>
        </a>

        <Show when=is_finance fallback=|| ()>
            <section>
                <h2 class="text-headline-sm text-on-background mb-3">"Alat Keuangan"</h2>
                // Satu pintu saja: /tagihan sudah punya tab "Belum Bayar" &
                // "Riwayat Pembayaran" di dalamnya — tak perlu kartu terpisah.
                <a
                    href="/tagihan"
                    class="ppm-card p-4 flex items-center gap-3 press hover:border-primary transition-colors"
                >
                    <span class="w-11 h-11 ppm-tile shrink-0">
                        <span class="material-symbols-outlined">"payments"</span>
                    </span>
                    <div class="flex-1 min-w-0">
                        <p class="text-body-md font-semibold text-on-background">"Pembayaran Santri"</p>
                        <p class="text-[11px] text-on-surface-variant">
                            "Cek, verifikasi & riwayat pembayaran"
                        </p>
                    </div>
                    <span class="material-symbols-outlined text-on-surface-variant shrink-0">
                        "chevron_right"
                    </span>
                </a>
            </section>
        </Show>
        </Suspense>
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
        <div class=format!("ppm-card p-3 flex items-center gap-3 card-hover {border}")>
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
