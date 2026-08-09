//! web/pages/server_status.rs — Status Server (/status-server, ADMIN saja).
//!
//! Menjawab satu pertanyaan yang selama ini hanya bisa dijawab dengan SSH ke
//! VPS: berapa CPU dan memori yang masih tersisa sekarang? Sebelum halaman ini,
//! tanda pertama bahwa servernya kehabisan napas adalah aplikasi yang mati
//! sendiri — dan yang tahu cara memeriksanya cuma satu orang.
//!
//! Sengaja TIDAK menyegarkan diri otomatis. Halaman yang berdenyut tiap
//! beberapa detik menahan satu koneksi database dan 300 ms pencuplikan CPU
//! setiap kali, selamanya, hanya karena ada yang lupa menutup tabnya. Tombol
//! "Segarkan" membuat biayanya selalu ada yang meminta.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{fmt_bytes, tingkat_pakai, ServerStatus};
use crate::web::api::server_status_data;
use crate::web::components::{DeviceFrame, FetchError, MobileHeader};

#[component]
pub fn StatusServerPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { server_status_data().await });
    let memuat = RwSignal::new(false);
    let segarkan = move |_| {
        memuat.set(true);
        data.refetch();
    };
    // Penanda "sedang memuat" dimatikan begitu data baru tiba. Tanpa ini
    // tombolnya tinggal redup selamanya setelah sekali ditekan.
    Effect::new(move |_| {
        if data.get().is_some() {
            memuat.set(false);
        }
    });

    view! {
        <Title text="Status Server — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide ppm-content">
                <MobileHeader
                    title="Status Server"
                    subtitle="Pemakaian CPU, memori, dan koneksi"
                    back_href="/staf"
                />

                <div class="px-5 pt-5 space-y-4">
                    <div class="flex items-center justify-between gap-2">
                        <p class="text-body-sm text-on-surface-variant min-w-0">
                            "Potret saat halaman dimuat — bukan grafik langsung."
                        </p>
                        <button
                            class="px-3 py-1.5 rounded-lg bg-primary text-on-primary text-body-sm font-semibold press cursor-pointer shrink-0 disabled:opacity-60"
                            prop:disabled=move || memuat.get()
                            on:click=segarkan
                        >
                            {move || if memuat.get() { "Memuat…" } else { "Segarkan" }}
                        </button>
                    </div>

                    <Suspense fallback=|| {
                        view! {
                            <div class="space-y-3 animate-pulse">
                                <div class="h-28 bg-surface-container rounded-2xl"></div>
                                <div class="h-28 bg-surface-container rounded-2xl"></div>
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                    Ok(s) => view! { <IsiStatus s=s /> }.into_any(),
                                })
                        }}
                    </Suspense>
                </div>
            </div>
        </DeviceFrame>
    }
}

#[component]
fn IsiStatus(s: ServerStatus) -> impl IntoView {
    let (cpu_label, cpu_warna) = tingkat_pakai(s.cpu_pct);
    let (mem_label, mem_warna) = tingkat_pakai(s.mem_pct);
    let mem_sisa = s.mem_total.saturating_sub(s.mem_terpakai);
    // Beban dibandingkan terhadap jumlah inti: 2,0 pada mesin 2 inti berarti
    // antreannya tepat penuh, pada 8 inti berarti mesinnya santai. Angka
    // telanjang tanpa pembanding ini rutin dibaca sebagai "persen".
    let beban_pct = if s.cpu_cores > 0 { s.load1 / s.cpu_cores as f32 * 100.0 } else { 0.0 };
    let (beban_label, beban_warna) = tingkat_pakai(beban_pct);
    let swap_ada = s.swap_total > 0;
    // Kolam habis = permintaan berikutnya ANTRE. Ini gejala paling awal dari
    // "aplikasinya lambat padahal CPU-nya santai", jadi ia diberi warna sendiri.
    let pool_ketat = s.pool_size >= s.pool_max && s.pool_idle == 0;
    // Dipakai di DUA cabang view (banner "tak tersedia" & catatan kaki), dan
    // closure `Show` MEMINDAHKAN apa yang disentuhnya — jadi salinannya diambil
    // di sini, sebelum satu pun closure terbentuk.
    let catatan_banner = s.catatan.clone();
    let catatan_kaki = s.catatan.clone();
    let tersedia = s.tersedia;

    view! {
        <Show when=move || !tersedia fallback=|| ()>
            <div class="ppm-card p-4 bg-surface-container-high">
                <p class="text-body-sm text-on-surface-variant">{catatan_banner.clone()}</p>
            </div>
        </Show>

        {s
            .tersedia
            .then(|| {
                view! {
                    // ── CPU ──────────────────────────────────────────────
                    <div class="ppm-card p-4 space-y-3">
                        <div class="flex items-baseline justify-between gap-2">
                            <p class="text-body-sm font-semibold text-on-background">"CPU"</p>
                            <p class=format!("text-body-sm font-bold {cpu_warna}")>{cpu_label}</p>
                        </div>
                        <div class="flex items-baseline gap-2">
                            <p class="text-2xl font-bold text-on-background tabular-nums">
                                {format!("{:.0}%", s.cpu_pct)}
                            </p>
                            <p class="text-body-sm text-on-surface-variant">
                                {format!("terpakai · sisa {:.0}%", (100.0 - s.cpu_pct).max(0.0))}
                            </p>
                        </div>
                        <Bar pct=s.cpu_pct />
                        <div class="flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-on-surface-variant">
                            <span>{format!("{} inti", s.cpu_cores)}</span>
                            <span class=beban_warna>
                                {format!(
                                    "Beban 1/5/15 mnt: {:.2} · {:.2} · {:.2} ({})",
                                    s.load1,
                                    s.load5,
                                    s.load15,
                                    beban_label,
                                )}
                            </span>
                        </div>
                    </div>

                    // ── Memori ───────────────────────────────────────────
                    <div class="ppm-card p-4 space-y-3">
                        <div class="flex items-baseline justify-between gap-2">
                            <p class="text-body-sm font-semibold text-on-background">"Memori (RAM)"</p>
                            <p class=format!("text-body-sm font-bold {mem_warna}")>{mem_label}</p>
                        </div>
                        <div class="flex items-baseline gap-2 flex-wrap">
                            <p class="text-2xl font-bold text-on-background tabular-nums">
                                {fmt_bytes(s.mem_terpakai)}
                            </p>
                            <p class="text-body-sm text-on-surface-variant">
                                {format!(
                                    "dari {} · sisa {}",
                                    fmt_bytes(s.mem_total),
                                    fmt_bytes(mem_sisa),
                                )}
                            </p>
                        </div>
                        <Bar pct=s.mem_pct />
                        <div class="flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-on-surface-variant">
                            <span>{format!("Sumber: {}", s.mem_sumber)}</span>
                            <span>{format!("Aplikasi ini: {}", fmt_bytes(s.app_rss))}</span>
                            {swap_ada
                                .then(|| {
                                    view! {
                                        <span>
                                            {format!(
                                                "Swap: {} / {}",
                                                fmt_bytes(s.swap_terpakai),
                                                fmt_bytes(s.swap_total),
                                            )}
                                        </span>
                                    }
                                })}
                        </div>
                    </div>
                }
            })}

        // ── Koneksi database ─────────────────────────────────────────────
        <div class="ppm-card p-4 space-y-2">
            <p class="text-body-sm font-semibold text-on-background">"Koneksi Database"</p>
            <div class="flex items-baseline gap-2 flex-wrap">
                <p class="text-2xl font-bold text-on-background tabular-nums">
                    {format!("{} / {}", s.pool_size, s.pool_max)}
                </p>
                <p class="text-body-sm text-on-surface-variant">
                    {format!("terbentuk · {} menganggur", s.pool_idle)}
                </p>
            </div>
            <p class=if pool_ketat {
                "text-[11px] text-warning"
            } else {
                "text-[11px] text-on-surface-variant"
            }>
                {if pool_ketat {
                    "Kolam penuh dan tak ada yang menganggur — permintaan baru akan antre \
                     menunggu koneksi. Bila ini bertahan saat jam sibuk, naikkan \
                     DB_POOL_MAX_SIZE."
                } else {
                    "Koneksi dibuat sesuai kebutuhan; menganggur > 0 berarti permintaan \
                     baru langsung dilayani."
                }}
            </p>
        </div>

        // ── Waktu hidup ──────────────────────────────────────────────────
        <div class="ppm-card p-4 space-y-1.5">
            <p class="text-body-sm font-semibold text-on-background">"Waktu Hidup"</p>
            <div class="flex justify-between gap-2">
                <span class="text-body-sm text-on-surface-variant">"Aplikasi"</span>
                <span class="text-body-sm font-semibold text-on-background">{s.uptime_app.clone()}</span>
            </div>
            <div class="flex justify-between gap-2">
                <span class="text-body-sm text-on-surface-variant">"Mesin"</span>
                <span class="text-body-sm font-semibold text-on-background">
                    {s.uptime_mesin.clone()}
                </span>
            </div>
            <p class="text-[11px] text-on-surface-variant pt-1">
                "Aplikasi jauh lebih muda dari mesin = pernah restart sendiri. Bila itu \
                 berulang, periksa memori di atas."
            </p>
        </div>

        {tersedia
            .then(|| {
                view! { <p class="text-[11px] text-on-surface-variant">{catatan_kaki}</p> }
            })}
    }
}

/// Bilah pemakaian. Warnanya mengikuti ambang yang sama dengan labelnya —
/// dua isyarat yang berbeda pendapat lebih buruk daripada satu.
#[component]
fn Bar(pct: f32) -> impl IntoView {
    let lebar = pct.clamp(0.0, 100.0);
    let warna = if lebar >= 90.0 {
        "bg-error"
    } else if lebar >= 75.0 {
        "bg-warning"
    } else {
        "bg-primary"
    };
    view! {
        <div
            class="h-2 w-full bg-surface-container-high rounded-full overflow-hidden"
            role="progressbar"
            aria-valuemin="0"
            aria-valuemax="100"
            aria-valuenow=format!("{:.0}", lebar)
        >
            <div
                class=format!("h-full rounded-full bar-grow {warna}")
                style=format!("width:{lebar:.1}%")
            ></div>
        </div>
    }
}
