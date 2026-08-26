//! web/pages/server_status.rs — Status Server (/status-server, ADMIN saja).
//!
//! Menjawab satu pertanyaan yang selama ini hanya bisa dijawab dengan SSH ke
//! VPS: berapa CPU, memori, dan RUANG DISK yang masih tersisa sekarang? Sebelum halaman ini,
//! tanda pertama bahwa servernya kehabisan napas adalah aplikasi yang mati
//! sendiri — dan yang tahu cara memeriksanya cuma satu orang.
//!
//! Sengaja TIDAK menyegarkan diri otomatis. Halaman yang berdenyut tiap
//! beberapa detik menahan satu koneksi database dan 300 ms pencuplikan CPU
//! setiap kali, selamanya, hanya karena ada yang lupa menutup tabnya. Tombol
//! "Segarkan" membuat biayanya selalu ada yang meminta.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{fmt_bytes, tingkat_pakai, ServerStatus, WahaStatus};
use crate::web::api::{server_status_data, waha_status_data};
use crate::web::components::{DeviceFrame, FetchError, MobileHeader};

#[component]
pub fn StatusServerPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { server_status_data().await });
    // Terpisah dari `data`: pemeriksaannya menembak jaringan ke WAHA dan bisa
    // menggantung sampai 15 detik — angka memori & disk tak boleh ikut
    // menunggunya. Kartunya punya <Suspense> sendiri di bawah.
    let waha = Resource::new(|| (), |_| async move { waha_status_data().await });
    let memuat = RwSignal::new(false);
    let segarkan = move |_| {
        memuat.set(true);
        data.refetch();
        waha.refetch();
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
                    subtitle="Sisa disk, memori, CPU, dan koneksi"
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
                            <div class="h-24 bg-surface-container rounded-2xl animate-pulse"></div>
                        }
                    }>
                        {move || {
                            waha.get()
                                .map(|res| match res {
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                    Ok(w) => view! { <KartuWaha w=w /> }.into_any(),
                                })
                        }}
                    </Suspense>

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

/// Kartu "WhatsApp (WAHA)" — lihat [`WahaStatus`] untuk kenapa ia ada.
#[component]
fn KartuWaha(w: WahaStatus) -> impl IntoView {
    let terhubung = w.terhubung;
    view! {
        <div class="ppm-card p-4">
            <div class="flex items-center justify-between gap-2 mb-1.5">
                <p class="text-body-sm font-semibold text-on-background">"WhatsApp (WAHA)"</p>
                <span class=if terhubung {
                    "px-2 py-0.5 rounded-full bg-tertiary-container text-on-tertiary-container text-[11px] font-semibold"
                } else {
                    "px-2 py-0.5 rounded-full bg-error-container text-on-error-container text-[11px] font-semibold"
                }>{if terhubung { "Tersambung" } else { "Tidak tersambung" }}</span>
            </div>
            <p class="text-body-sm text-on-surface-variant break-words">{w.keterangan}</p>
            <p class="text-[11px] text-on-surface-variant mt-1.5 break-all">
                {format!("{} · sesi \"{}\"", w.base_url, w.session)}
            </p>
            // Keterangan ini yang mengubah kartu dari angka jadi TINDAKAN:
            // tanpa WAHA, tiga alur pemulihan akun berhenti bekerja dan
            // semuanya gagal tanpa pesan apa pun ke penggunanya.
            <Show when=move || !terhubung>
                <p class="text-[11px] text-error mt-2">
                    "Selama ini merah: lupa sandi, OTP pendaftaran, dan ganti nomor TIDAK terkirim — \
                     dan pengguna tak melihat galat apa pun. Periksa kontainer WAHA & sesinya."
                </p>
            </Show>
        </div>
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
    let disk = s.disk.clone();

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

        // ── Penyimpanan (SSD/NVMe) ───────────────────────────────────────
        // Ditaruh SEBELUM koneksi database dan sesudah memori: dari semua angka
        // di halaman ini, disk penuh adalah satu-satunya yang tak pulih sendiri
        // setelah restart.
        {(!disk.is_empty())
            .then(|| {
                view! {
                    <div class="ppm-card p-4 space-y-3">
                        <div class="flex items-baseline justify-between gap-2">
                            <p class="text-body-sm font-semibold text-on-background">
                                "Penyimpanan (SSD/NVMe)"
                            </p>
                            <p class="text-[11px] text-on-surface-variant">"Sisa = yang menentukan"</p>
                        </div>
                        {disk.into_iter().map(|d| view! { <KartuDisk d=d /> }).collect_view()}
                        <p class="text-[11px] text-on-surface-variant">
                            "Yang menghabiskan disk di server ini, berurutan: rekaman sesi (.webm, \
                             menumpuk tiap siaran), berkas sementara unggahan, image & log Docker, \
                             lalu data Postgres. Bila disk penuh, Postgres BERHENTI menerima \
                             tulisan — absensi & pembayaran gagal disimpan, dan restart tak \
                             menolong sampai ruangnya dikosongkan."
                        </p>
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

/// Satu filesystem. Angka BESARNYA adalah SISA, bukan yang terpakai —
/// pertanyaan yang dibawa admin ke halaman ini selalu "masih muat berapa lagi",
/// dan memaksanya mengurangi dua angka besar di kepala adalah cara termudah
/// salah baca.
#[component]
fn KartuDisk(d: crate::models::DiskInfo) -> impl IntoView {
    let (label, warna) = tingkat_pakai(d.pct);
    let peringatan = crate::models::peringatan_disk(d.tersedia, d.pct);
    view! {
        <div class="rounded-xl bg-surface-container-low p-3 space-y-2">
            <div class="flex items-baseline justify-between gap-2">
                <p class="text-body-sm font-semibold text-on-background truncate">{d.label}</p>
                <p class=format!("text-body-sm font-bold {warna} shrink-0")>{label}</p>
            </div>
            <div class="flex items-baseline gap-2 flex-wrap">
                <p class="text-2xl font-bold text-on-background tabular-nums">
                    {fmt_bytes(d.tersedia)}
                </p>
                <p class="text-body-sm text-on-surface-variant">
                    {format!(
                        "sisa · {} terpakai dari {}",
                        fmt_bytes(d.terpakai),
                        fmt_bytes(d.total),
                    )}
                </p>
            </div>
            <Bar pct=d.pct />
            <p class="text-[11px] text-on-surface-variant break-all">
                {format!("{} · {:.0}% terpakai", d.path, d.pct)}
            </p>
            {peringatan
                .map(|p| {
                    view! {
                        <p class="text-[11px] text-warning bg-warning/10 rounded-lg px-2.5 py-1.5">
                            {p}
                        </p>
                    }
                })}
        </div>
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
