//! web/pages/verifikasi_pamong.rs — Verifikasi Pamong (tahap 1), data ASLI.
//!
//! Antrean absensi pamong_status=pending → tombol Setujui/Tolak memanggil
//! server fn `decide_pamong`, lalu daftar di-refetch.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::PendingAtt;
use crate::web::api::{decide_pamong, pamong_data};
use crate::web::components::{FetchError, DeviceFrame, MobileNav, NAV_PAMONG};

#[component]
pub fn VerifikasiPamongPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { pamong_data().await });

    // Guard: belum login / bukan pamong → login.
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
        <Title text="Verifikasi Pamong — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto">
                <header class="sticky top-0 z-10 bg-surface/90 backdrop-blur border-b border-outline-variant/60 px-5 py-4 flex items-center justify-between">
                    <div>
                        <h1 class="text-headline-sm text-on-background">"Verifikasi Pamong"</h1>
                        <p class="text-body-sm text-on-surface-variant">"Kehadiran menunggu tindakan"</p>
                    </div>
                    <span class="material-symbols-outlined text-primary">"how_to_reg"</span>
                </header>

                <div class="px-5 pt-5 space-y-5 stagger">
                    <Suspense fallback=|| {
                        view! {
                            <div class="space-y-3 animate-pulse">
                                <div class="h-20 bg-surface-container rounded-2xl"></div>
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => {
                                        let pending_count = d.pending.len();
                                        view! {
                                            // ── Statistik ────────────────────────
                                            <div class="grid grid-cols-2 gap-3">
                                                <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4">
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
                                                <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4">
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
                                                d.pending
                                                    .into_iter()
                                                    .map(|p| {
                                                        view! { <PendingCard p=p busy_id=busy_id decide=decide /> }
                                                    })
                                                    .collect_view()
                                                    .into_any()
                                            }}
                                        }
                                            .into_any()
                                    }
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any()
                                })
                        }}
                    </Suspense>
                </div>

                <MobileNav items=NAV_PAMONG active="/verifikasi-pamong" />
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
        <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4 space-y-3 card-hover anim-in">
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
