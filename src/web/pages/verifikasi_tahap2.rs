//! web/pages/verifikasi_tahap2.rs — Verifikasi Kehadiran TAHAP 2 (dewan guru).
//!
//! Antrean = absensi yang sudah disetujui pamong (pamong_status=approved) namun
//! belum diverifikasi final (verify_status=pending). Setujui/Tolak memanggil
//! server fn `decide_verify` (final), lalu daftar di-refetch. Poin TIDAK berubah
//! di tahap ini (sudah diberikan saat verifikasi pamong).

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::PendingAtt;
use crate::web::api::{decide_verify, verify_data};
use crate::web::components::{DeviceFrame, FetchError};

#[component]
pub fn VerifikasiTahap2Page() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { verify_data().await });

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

    let busy_id = RwSignal::new(Option::<i64>::None);
    let decide = move |id: i64, approve: bool| {
        if busy_id.get_untracked().is_some() {
            return;
        }
        busy_id.set(Some(id));
        leptos::task::spawn_local(async move {
            let _ = decide_verify(id, approve).await;
            busy_id.set(None);
            data.refetch();
        });
    };

    view! {
        <Title text="Verifikasi Tahap 2 — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto">
                <header class="sticky top-0 z-10 bg-surface/90 backdrop-blur border-b border-outline-variant/60 px-5 py-4 flex items-center justify-between">
                    <div>
                        <h1 class="text-headline-sm text-on-background">"Verifikasi Tahap 2"</h1>
                        <p class="text-body-sm text-on-surface-variant">
                            "Persetujuan final dewan guru"
                        </p>
                    </div>
                    <span class="material-symbols-outlined text-primary">"verified_user"</span>
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
                                            <div class="grid grid-cols-2 gap-3">
                                                <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4">
                                                    <div class="flex items-center gap-2 text-warning">
                                                        <span class="material-symbols-outlined pulse-dot">
                                                            "hourglass_top"
                                                        </span>
                                                        <span
                                                            class="text-2xl font-bold text-on-background"
                                                            data-count=pending_count.to_string()
                                                        >
                                                            {pending_count}
                                                        </span>
                                                    </div>
                                                    <p class="text-body-sm text-on-surface-variant mt-1">
                                                        "Menunggu Final"
                                                    </p>
                                                </div>
                                                <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4">
                                                    <div class="flex items-center gap-2 text-success">
                                                        <span class="material-symbols-outlined">"verified"</span>
                                                        <span
                                                            class="text-2xl font-bold text-on-background"
                                                            data-count=d.approved_today.to_string()
                                                        >
                                                            {d.approved_today}
                                                        </span>
                                                    </div>
                                                    <p class="text-body-sm text-on-surface-variant mt-1">
                                                        "Terverifikasi Hari Ini"
                                                    </p>
                                                </div>
                                            </div>

                                            {if d.pending.is_empty() {
                                                view! {
                                                    <div class="bg-surface-container rounded-2xl p-8 text-center">
                                                        <span class="material-symbols-outlined text-5xl text-success">
                                                            "task_alt"
                                                        </span>
                                                        <p class="text-body-md text-on-surface-variant mt-3">
                                                            "Tidak ada kehadiran menunggu verifikasi final."
                                                        </p>
                                                    </div>
                                                }
                                                    .into_any()
                                            } else {
                                                d.pending
                                                    .into_iter()
                                                    .map(|p| {
                                                        view! { <VerifyCard p=p busy_id=busy_id decide=decide /> }
                                                    })
                                                    .collect_view()
                                                    .into_any()
                                            }}
                                        }
                                            .into_any()
                                    }
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
fn VerifyCard(
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
                <span class="px-2.5 py-1 rounded-full text-[10px] font-bold tracking-wider bg-secondary-container text-primary shrink-0 self-start flex items-center gap-1">
                    <span class="material-symbols-outlined text-[13px]">"how_to_reg"</span>
                    "Pamong OK"
                </span>
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
                    "Verifikasi"
                </button>
            </div>
        </div>
    }
}
