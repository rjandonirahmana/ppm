//! web/pages/verifikasi_pamong.rs — Verifikasi Pamong (tahap 1), data ASLI.
//!
//! Antrean absensi pamong_status=pending → tombol Setujui/Tolak memanggil
//! server fn `decide_pamong`, lalu daftar di-refetch.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::PendingAtt;
use crate::web::api::{decide_pamong, pamong_data, permit_queue_data};
use crate::web::components::{kartu_grid, DeviceFrame, FetchError, JadwalDeck};

#[component]
pub fn VerifikasiPamongPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { pamong_data().await });
    let permits = Resource::new(|| (), |_| async move { permit_queue_data().await });

    // Guard: belum login / bukan pamong → login.
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

    // Keputusan sedang diproses (id) → disable tombol baris tsb.
    let busy_id = RwSignal::new(Option::<i64>::None);
    let decide = move |id: i64, approve: bool| {
        if busy_id.get_untracked().is_some() {
            return;
        }
        busy_id.set(Some(id));
        leptos::task::spawn_local(async move {
            let _ = decide_pamong(id, approve).await;
            busy_id.set(None);
            data.refetch();
        });
    };

    view! {
        <Title text="Verifikasi Pamong — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <header class="sticky top-0 z-10 bg-surface/90 backdrop-blur border-b border-outline-variant/60 px-5 py-4 flex items-center justify-between">
                    <div>
                        <h1 class="text-headline-sm text-on-background">"Verifikasi Pamong"</h1>
                        <p class="text-body-sm text-on-surface-variant">"Kehadiran menunggu tindakan"</p>
                    </div>
                    <span class="material-symbols-outlined text-primary">"how_to_reg"</span>
                </header>

                <div class="px-5 pt-5 space-y-5 stagger">
                    // Pintasan dikumpulkan SEBARIS di atas — sebelumnya "Rekap
                    // Mingguan" di sini sementara "Kelas Saya" terselip jauh di
                    // bawah, di tengah badan Suspense, sehingga dua tombol yang
                    // sejenis terlihat berserak. Pola dua kolom ini sama dengan
                    // beranda guru/dewan guru.
                    <div class="grid grid-cols-2 gap-3 md:max-w-lg">
                        <a href="/rekap-mingguan" class="ppm-card p-3 flex items-center gap-2 press">
                            <span class="material-symbols-outlined text-primary">"summarize"</span>
                            <span class="text-body-sm font-semibold text-on-background">"Rekap Mingguan"</span>
                        </a>
                        <a href="/kelas-saya" class="ppm-card p-3 flex items-center gap-2 press">
                            <span class="material-symbols-outlined text-primary">"school"</span>
                            <span class="text-body-sm font-semibold text-on-background">"Kelas Saya"</span>
                        </a>
                    </div>
                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="h-28 bg-surface-container rounded-2xl"></div>
                                <div class="grid grid-cols-2 gap-3 md:max-w-lg">
                                    <div class="h-20 bg-surface-container rounded-2xl"></div>
                                    <div class="h-20 bg-surface-container rounded-2xl"></div>
                                </div>
                                <div class="grid gap-2 md:grid-cols-2">
                                    <div class="h-24 bg-surface-container rounded-2xl"></div>
                                    <div class="h-24 bg-surface-container rounded-2xl"></div>
                                </div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => {
                                        let pending_count = d.pending.len();
                                        let hadir_label = format!("{} / {}", d.hadir_today, d.total_santri);
                                        let pct = d.pct;
                                        view! {
                                            // ── Hero: santri aktif hari ini ──────
                                            <div class="spiritual-gradient rounded-2xl p-5 text-on-primary shadow-lg shadow-primary/20">
                                                <p class="text-[11px] font-bold tracking-[0.2em] opacity-80">
                                                    "SANTRI AKTIF"
                                                </p>
                                                <div class="flex items-end justify-between mt-1">
                                                    <p class="text-display-lg">{hadir_label}</p>
                                                    <p class="text-body-sm opacity-85">{format!("{pct}% hadir hari ini")}</p>
                                                </div>
                                                <div class="w-full h-2 bg-white/20 rounded-full overflow-hidden mt-3">
                                                    <div
                                                        class="bg-primary-fixed h-full bar-grow"
                                                        style=format!("width: {pct}%")
                                                    ></div>
                                                </div>
                                            </div>

                                            // ── Statistik ────────────────────────
                                            <div class="grid grid-cols-2 gap-3 md:max-w-lg">
                                                <div class="ppm-card p-4">
                                                    <div class="flex items-center gap-2 text-warning">
                                                        <span class="material-symbols-outlined pulse-dot">"pending_actions"</span>
                                                        <span
                                                            class="text-2xl font-bold text-on-background"
                                                            data-count=pending_count.to_string()
                                                        >
                                                            {pending_count}
                                                        </span>
                                                    </div>
                                                    <p class="text-body-sm text-on-surface-variant mt-1">
                                                        "Menunggu Tindakan"
                                                    </p>
                                                </div>
                                                <div class="ppm-card p-4">
                                                    <div class="flex items-center gap-2 text-success">
                                                        <span class="material-symbols-outlined">"done_all"</span>
                                                        <span
                                                            class="text-2xl font-bold text-on-background"
                                                            data-count=d.approved_today.to_string()
                                                        >
                                                            {d.approved_today}
                                                        </span>
                                                    </div>
                                                    <p class="text-body-sm text-on-surface-variant mt-1">
                                                        "Disetujui Hari Ini"
                                                    </p>
                                                </div>
                                            </div>

                                            // ── Jadwal berikutnya, bisa digeser ──
                                            <JadwalDeck sesi=d.today.clone() />

                                            // ── Tautan Tinjau Izin (migrasi 17) ──
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
                                                                            <p class="text-body-md font-semibold text-on-background">
                                                                                "Tinjau Izin"
                                                                            </p>
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

                                            // ── Antrean ──────────────────────────
                                            {if d.pending.is_empty() {
                                                view! {
                                                    <div class="bg-surface-container rounded-2xl p-8 text-center">
                                                        <span class="material-symbols-outlined text-5xl text-success">
                                                            "task_alt"
                                                        </span>
                                                        <p class="text-body-md text-on-surface-variant mt-3">
                                                            "Semua kehadiran sudah diverifikasi."
                                                        </p>
                                                    </div>
                                                }
                                                    .into_any()
                                            } else {
                                                // Desktop: antrean 2 kolom (mockup dashboard pamong).
                                                kartu_grid(
                                                        d.pending
                                                            .into_iter()
                                                            .map(|p| {
                                                                view! { <PendingCard p=p busy_id=busy_id decide=decide /> }
                                                                    .into_any()
                                                            })
                                                            .collect(),
                                                    )
                                                    .into_any()
                                            }}

                                            // ── Sesi hari ini (verifikasi kelas) ──
                                            {(!d.today.is_empty())
                                                .then(|| {
                                                    view! {
                                                        <div>
                                                            <h3 class="text-body-lg font-bold text-on-background mb-2">
                                                                "Sesi Hari Ini"
                                                            </h3>
                                                            <div class="ppm-card-grid">
                                                                {d.today
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
                                                                                        {format!("{} • {} • {} santri", s.teacher, s.time_label, s.santri_count)}
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

                                            // ── Kehadiran terbaru ────────────────
                                            {(!d.latest.is_empty())
                                                .then(|| {
                                                    view! {
                                                        <div>
                                                            <h3 class="text-body-lg font-bold text-on-background mb-2">
                                                                "Kehadiran Terbaru"
                                                            </h3>
                                                            <div class="ppm-card px-4 py-1">
                                                                {d.latest
                                                                    .iter()
                                                                    .cloned()
                                                                    .map(|a| {
                                                                        let badge = match a.kind.as_str() {
                                                                            "present" => "px-2 py-0.5 rounded-full text-[10px] font-bold bg-success/10 text-success",
                                                                            "late" => "px-2 py-0.5 rounded-full text-[10px] font-bold bg-warning/10 text-warning",
                                                                            "absent" => "px-2 py-0.5 rounded-full text-[10px] font-bold bg-error-container text-error",
                                                                            _ => "px-2 py-0.5 rounded-full text-[10px] font-bold bg-surface-container-highest text-on-surface-variant",
                                                                        };
                                                                        view! {
                                                                            <div class="flex items-center gap-3 py-2.5 border-b border-outline-variant/40 last:border-0">
                                                                                <div class="w-8 h-8 rounded-full bg-secondary-container text-primary flex items-center justify-center text-[11px] font-bold shrink-0">
                                                                                    {a.initial.clone()}
                                                                                </div>
                                                                                <div class="flex-1 min-w-0">
                                                                                    <p class="text-body-sm font-semibold text-on-background truncate">{a.name.clone()}</p>
                                                                                    <p class="text-[10px] text-on-surface-variant">{a.time_label.clone()}</p>
                                                                                </div>
                                                                                <span class=badge>{a.status_label.clone()}</span>
                                                                            </div>
                                                                        }
                                                                    })
                                                                    .collect_view()}
                                                            </div>
                                                        </div>
                                                    }
                                                })}
                                        }
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
fn PendingCard(
    p: PendingAtt,
    busy_id: RwSignal<Option<i64>>,
    decide: impl Fn(i64, bool) + Copy + Send + 'static,
) -> impl IntoView {
    let id = p.id;
    let initial = p.name.chars().next().unwrap_or('S').to_string();
    let meta = format!("NIS: {} • {}", p.nis, p.class_name);
    let scan = format!("{} • {}", p.time_label, p.gate);
    let is_busy = move || busy_id.get() == Some(id);

    view! {
        <div class="ppm-card p-4 space-y-3 card-hover anim-in">
            <div class="flex items-center gap-3">
                <div class="w-11 h-11 rounded-full bg-secondary-container flex items-center justify-center text-primary font-bold shrink-0">
                    {initial}
                </div>
                <div class="flex-1 min-w-0">
                    <p class="text-body-md font-semibold text-on-background truncate">{p.name}</p>
                    <p class="text-body-sm text-on-surface-variant truncate">{meta}</p>
                    <p class="text-body-sm text-on-surface-variant flex items-center gap-1 mt-0.5">
                        <span class="material-symbols-outlined text-[15px]">"schedule"</span>
                        {scan}
                    </p>
                </div>
            </div>
            <div class="grid grid-cols-2 gap-3">
                <button
                    class="py-2.5 rounded-xl border border-error/40 text-error font-semibold text-body-sm hover:bg-error-container transition-colors disabled:opacity-50"
                    disabled=is_busy
                    on:click=move |_| decide(id, false)
                >
                    "Tolak"
                </button>
                <button
                    class="py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-sm hover:bg-primary-container transition-colors disabled:opacity-50"
                    disabled=is_busy
                    on:click=move |_| decide(id, true)
                >
                    "Setujui"
                </button>
            </div>
        </div>
    }
}
