//! web/pages/izin_staf.rs — Tinjau Izin (tahap 2, pamong/dewan guru/admin),
//! migrasi 17. Antrean izin yang SUDAH lolos konfirmasi orang tua
//! (parent_status='approved') menunggu keputusan pamong/dewan guru/admin.
//! Pola sama verifikasi_pamong.rs (Resource + Suspense + kartu Setujui/Tolak).

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::PermitReviewItem;
use crate::web::api::{decide_permit_action, permit_queue_data};
use crate::web::components::{DeviceFrame, EmptyState, FetchError, MobileHeader};

#[component]
pub fn IzinStafPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { permit_queue_data().await });

    Effect::new(move |_| {
        if let Some(Err(e)) = data.get() {
            if crate::web::components::is_auth_error(&e.to_string()) {
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
            let _ = decide_permit_action(id, approve).await;
            busy_id.set(None);
            data.refetch();
        });
    };

    view! {
        <Title text="Tinjau Izin — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Tinjau Izin" subtitle="Antrean izin menunggu keputusan" back_href="/staf" />

                <div class="px-5 pt-5 space-y-5 stagger">
                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="grid grid-cols-2 gap-3 md:max-w-lg">
                                    <div class="h-20 bg-surface-container rounded-2xl"></div>
                                    <div class="h-20 bg-surface-container rounded-2xl"></div>
                                </div>
                                <div class="grid gap-2 md:grid-cols-2">
                                    <div class="h-28 bg-surface-container rounded-2xl"></div>
                                    <div class="h-28 bg-surface-container rounded-2xl"></div>
                                </div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => {
                                        let stage = d.stage_label.clone();
                                        view! {
                                            <div class="ppm-chip bg-secondary-container text-primary inline-flex items-center gap-1">
                                                <span class="material-symbols-outlined text-[15px]">"how_to_reg"</span>
                                                {format!("{} · rute per-kelas (via pamong diatur di tiap kelas)", stage.clone())}
                                            </div>
                                            <div class="grid grid-cols-2 gap-3 md:max-w-lg">
                                                <div class="ppm-card p-4">
                                                    <div class="flex items-center gap-2 text-warning">
                                                        <span class="material-symbols-outlined pulse-dot">"pending_actions"</span>
                                                        <span
                                                            class="text-2xl font-bold text-on-background"
                                                            data-count=d.pending_count.to_string()
                                                        >
                                                            {d.pending_count}
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
                                                        "Diputuskan Hari Ini"
                                                    </p>
                                                </div>
                                            </div>

                                            {if d.items.is_empty() {
                                                view! {
                                                    <EmptyState
                                                        icon="task_alt"
                                                        title="Tidak ada izin menunggu"
                                                        subtitle="Semua izin yang lolos konfirmasi orang tua sudah diputuskan."
                                                    />
                                                }
                                                    .into_any()
                                            } else {
                                                view! {
                                                    <div class="ppm-card-grid">
                                                        {d.items
                                                            .into_iter()
                                                            .map(|p| {
                                                                view! { <PermitCard p=p busy_id=busy_id decide=decide /> }
                                                            })
                                                            .collect_view()}
                                                    </div>
                                                }
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
fn PermitCard(
    p: PermitReviewItem,
    busy_id: RwSignal<Option<i64>>,
    decide: impl Fn(i64, bool) + Copy + Send + 'static,
) -> impl IntoView {
    let id = p.id;
    let meta = format!("NIS: {} • {}", p.nis, p.class_name);
    let is_busy = move || busy_id.get() == Some(id);

    view! {
        <div class="ppm-card p-4 space-y-3 card-hover anim-in">
            <div class="flex items-start justify-between gap-2">
                <div class="min-w-0">
                    <p class="text-body-md font-semibold text-on-background truncate">{p.student_name}</p>
                    <p class="text-body-sm text-on-surface-variant truncate">{meta}</p>
                </div>
                <span class="ppm-chip bg-primary/10 text-primary shrink-0">{p.kind_label}</span>
            </div>

            // ── Progress dua-tahap: PARENT (selesai) → PAMONG (aktif) ──────
            <div class="w-full">
                <div class="flex justify-between text-[10px] font-bold text-on-surface-variant mb-1">
                    <span class="text-primary">"ORANG TUA"</span>
                    <span class="text-primary">"PENGURUS"</span>
                </div>
                <div class="h-1.5 w-full bg-outline-variant rounded-full overflow-hidden">
                    <div class="h-full bg-primary w-1/2"></div>
                </div>
            </div>

            <p class="text-body-sm text-on-surface-variant flex items-center gap-1">
                <span class="material-symbols-outlined text-[15px]">"calendar_month"</span>
                {p.range_label}
            </p>
            <p class="text-body-sm text-on-surface-variant italic">
                {format!("\u{201C}{}\u{201D}", p.reason)}
            </p>
            <p class="text-[10px] text-on-surface-variant">{p.when_label}</p>

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
