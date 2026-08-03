//! web/pages/poin.rs — Pantauan Poin Santri (/poin staf, /poin-dewan dewan
//! guru), data ASLI lewat `poin_data_action`. Dewan guru/admin bisa
//! menambah/mengurangi poin manual (`adjust_points_action`) — tercatat di
//! point_logs dengan kategori "manual".

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::PointRow;
use crate::web::api::{adjust_points_action, poin_data_action};
use crate::web::components::{DeviceFrame, FetchError, MobileHeader};

#[component]
pub fn PoinPage() -> impl IntoView {
    view! { <PoinPageInner /> }
}

#[component]
pub fn PoinDewanPage() -> impl IntoView {
    view! { <PoinPageInner /> }
}

#[component]
fn PoinPageInner() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { poin_data_action().await });

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

    // Santri sedang disesuaikan poinnya (buka panel inline).
    let editing = RwSignal::new(Option::<i64>::None);
    let busy = RwSignal::new(false);
    let feedback = RwSignal::new(Option::<String>::None);

    let apply_delta = move |student_id: i64, delta: i32, reason: String| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            match adjust_points_action(student_id, delta, reason).await {
                Ok(_) => {
                    feedback.set(Some("Poin berhasil diperbarui.".into()));
                    editing.set(None);
                    data.refetch();
                }
                Err(e) => feedback.set(Some(format!("Gagal: {e}"))),
            }
            busy.set(false);
        });
    };

    view! {
        <Title text="Pantauan Poin Santri — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Pantauan Poin Santri" subtitle="Papan peringkat & penyesuaian poin" />
                <div class="px-5 pt-5 space-y-5 stagger">
                    {move || {
                        feedback
                            .get()
                            .map(|f| {
                                view! {
                                    <div class="bg-info/10 text-info text-body-sm rounded-xl px-4 py-2.5">{f}</div>
                                }
                            })
                    }}
                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="grid grid-cols-2 gap-3 md:max-w-lg">
                                    <div class="h-20 bg-surface-container rounded-2xl"></div>
                                    <div class="h-20 bg-surface-container rounded-2xl"></div>
                                </div>
                                <div class="grid gap-2 md:grid-cols-2">
                                    <div class="h-16 bg-surface-container rounded-2xl"></div>
                                    <div class="h-16 bg-surface-container rounded-2xl"></div>
                                    <div class="h-16 bg-surface-container rounded-2xl hidden md:block"></div>
                                    <div class="h-16 bg-surface-container rounded-2xl hidden md:block"></div>
                                </div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => {
                                        view! {
                                            <div class="grid grid-cols-2 gap-3 md:max-w-lg">
                                                <div class="ppm-card p-4">
                                                    <p class="text-body-sm text-on-surface-variant">"Total Santri"</p>
                                                    <p class="text-2xl font-bold text-on-background mt-1">
                                                        {d.total_santri}
                                                    </p>
                                                </div>
                                                <div class="ppm-card p-4">
                                                    <p class="text-body-sm text-on-surface-variant">"Rata-rata Poin"</p>
                                                    <p class="text-2xl font-bold text-primary mt-1">
                                                        {d.avg_points}
                                                    </p>
                                                </div>
                                            </div>
                                            <div class="space-y-2 md:space-y-0 md:grid md:grid-cols-2 md:gap-2">
                                                {if d.top.is_empty() {
                                                    view! {
                                                        <p class="text-body-sm text-on-surface-variant text-center py-6 md:col-span-2">
                                                            "Belum ada santri di cakupan Anda."
                                                        </p>
                                                    }
                                                        .into_any()
                                                } else {
                                                    let can_adjust = d.can_adjust;
                                                    d.top
                                                        .into_iter()
                                                        .enumerate()
                                                        .map(|(i, row)| {
                                                            view! {
                                                                <PointCard
                                                                    rank=i + 1
                                                                    row=row
                                                                    can_adjust=can_adjust
                                                                    editing=editing
                                                                    busy=busy
                                                                    apply_delta=apply_delta
                                                                />
                                                            }
                                                        })
                                                        .collect_view()
                                                        .into_any()
                                                }}
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
        </DeviceFrame>
    }
}

#[component]
fn PointCard(
    rank: usize,
    row: PointRow,
    can_adjust: bool,
    editing: RwSignal<Option<i64>>,
    busy: RwSignal<bool>,
    apply_delta: impl Fn(i64, i32, String) + Copy + Send + 'static,
) -> impl IntoView {
    let uid = row.user_id;
    let is_open = move || editing.get() == Some(uid);
    let meta = format!(
        "{}{}",
        row.nis.map(|n| format!("NIS: {n} • ")).unwrap_or_default(),
        row.class_name.unwrap_or_else(|| "-".into()),
    );

    view! {
        <div class="ppm-card p-4 space-y-3">
            <div class="flex items-center gap-3">
                <span class="w-6 h-6 rounded-full bg-primary/10 text-primary text-[11px] font-bold flex items-center justify-center shrink-0">
                    {rank}
                </span>
                <div class="w-9 h-9 rounded-full bg-secondary-container flex items-center justify-center text-primary font-bold text-body-sm shrink-0">
                    {row.initial}
                </div>
                <div class="flex-1 min-w-0">
                    <p class="font-semibold text-on-background text-body-sm truncate">{row.name}</p>
                    <p class="text-[11px] text-on-surface-variant truncate">{meta}</p>
                </div>
                <p class="text-lg font-bold text-primary shrink-0">{row.points}</p>
                {can_adjust
                    .then(|| {
                        view! {
                            <button
                                class="w-8 h-8 rounded-full flex items-center justify-center text-on-surface-variant hover:bg-surface-container-high shrink-0"
                                on:click=move |_| {
                                    editing.update(|e| *e = if *e == Some(uid) { None } else { Some(uid) })
                                }
                            >
                                <span class="material-symbols-outlined text-lg">"tune"</span>
                            </button>
                        }
                    })}
            </div>
            {move || {
                is_open()
                    .then(|| {
                        view! {
                            <div class="flex gap-2 pt-1">
                                <button
                                    class="flex-1 py-2 rounded-xl border border-error/40 text-error font-semibold text-body-sm disabled:opacity-50"
                                    disabled=move || busy.get()
                                    on:click=move |_| apply_delta(uid, -5, "Pelanggaran (manual)".into())
                                >
                                    "-5"
                                </button>
                                <button
                                    class="flex-1 py-2 rounded-xl border border-error/40 text-error font-semibold text-body-sm disabled:opacity-50"
                                    disabled=move || busy.get()
                                    on:click=move |_| apply_delta(uid, -1, "Pelanggaran ringan (manual)".into())
                                >
                                    "-1"
                                </button>
                                <button
                                    class="flex-1 py-2 rounded-xl border border-success/40 text-success font-semibold text-body-sm disabled:opacity-50"
                                    disabled=move || busy.get()
                                    on:click=move |_| apply_delta(uid, 1, "Prestasi ringan (manual)".into())
                                >
                                    "+1"
                                </button>
                                <button
                                    class="flex-1 py-2 rounded-xl bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-50"
                                    disabled=move || busy.get()
                                    on:click=move |_| apply_delta(uid, 5, "Prestasi (manual)".into())
                                >
                                    "+5"
                                </button>
                            </div>
                        }
                    })
            }}
        </div>
    }
}
