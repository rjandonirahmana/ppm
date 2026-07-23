//! web/pages/ortu_izin.rs — Daftar Izin Santri sisi ORANG TUA (mockup stitch):
//! ajukan izin untuk putra/putri (pilih anak), filter status, kartu izin
//! dengan alasan & status. Data ASLI (children_permits / submit_child_permit).

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{ParentPermitItem, PendingParentConfirm};
use crate::web::api::{
    children_permits, confirm_child_permit_action, parent_home, submit_child_permit_action,
};
use crate::web::components::{DeviceFrame, EmptyState, MobileHeader};

#[component]
pub fn OrtuIzinPage() -> impl IntoView {
    let permits = Resource::new(|| (), |_| async move { children_permits().await });
    // Daftar anak utk form (ambil dari parent_home tanpa monitor detail).
    let home = Resource::new(|| (), |_| async move { parent_home(None).await });

    Effect::new(move |_| {
        if let Some(Err(e)) = permits.get() {
            let msg = e.to_string();
            if msg.contains("unauth") || msg.contains("forbidden") {
                #[cfg(target_arch = "wasm32")]
                if let Some(w) = web_sys::window() {
                    let _ = w.location().replace("/login");
                }
            }
        }
    });

    let show_form = RwSignal::new(false);
    let filter = RwSignal::new("all".to_string());

    // ── Form state ──────────────────────────────────────────────────────────
    let child = RwSignal::new(Option::<i64>::None);
    let kind = RwSignal::new("sick".to_string());
    let start = RwSignal::new(String::new());
    let end = RwSignal::new(String::new());
    let reason = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(cid) = child.get_untracked() else {
            msg.set(Some((false, "Pilih santri terlebih dahulu.".into())));
            return;
        };
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let (k, s, e2, r) = (
            kind.get_untracked(),
            start.get_untracked(),
            end.get_untracked(),
            reason.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            match submit_child_permit_action(cid, k, s, e2, r).await {
                Ok(_) => {
                    msg.set(Some((true, "Izin terkirim — menunggu persetujuan pengurus.".into())));
                    start.set(String::new());
                    end.set(String::new());
                    reason.set(String::new());
                    permits.refetch();
                }
                Err(e) => {
                    let m = e.to_string();
                    let m = m.rsplit(": ").next().unwrap_or(&m).to_string();
                    msg.set(Some((false, m)));
                }
            }
            busy.set(false);
        });
    };

    view! {
        <Title text="Daftar Izin Santri — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Daftar Izin Santri" subtitle="Kelola permohonan izin putra/putri Anda" />

                <div class="px-5 pt-5 space-y-4 stagger">
                    // Tombol + form ajukan izin dibatasi lebar di desktop (md:max-w-md,
                    // sama dgn kolom mobile) — form pendek jangan melebar penuh kanvas
                    // 72rem, itu yg bikin "kartu mobile melar" (lihat tailwind.css).
                    // ── Perlu Konfirmasi (izin diajukan santri sendiri) ────
                    <Suspense fallback=|| ()>
                        {move || {
                            home.get()
                                .and_then(|r| r.ok())
                                .filter(|h| !h.pending_permits.is_empty())
                                .map(|h| {
                                    view! {
                                        <div class="space-y-2">
                                            <h2 class="text-headline-sm text-on-background flex items-center gap-2">
                                                <span class="material-symbols-outlined text-warning">"pending_actions"</span>
                                                "Perlu Konfirmasi Anda"
                                            </h2>
                                            <div class="space-y-3 md:space-y-0 md:grid md:grid-cols-2 md:gap-3">
                                                {h.pending_permits
                                                    .into_iter()
                                                    .map(|p| {
                                                        view! {
                                                            <ConfirmPermitCard
                                                                p=p
                                                                on_done=move || {
                                                                    home.refetch();
                                                                    permits.refetch();
                                                                }
                                                            />
                                                        }
                                                    })
                                                    .collect_view()}
                                            </div>
                                        </div>
                                    }
                                })
                        }}
                    </Suspense>

                    <div class="md:max-w-md">
                    // ── Tombol buka form ───────────────────────────────────
                    <button
                        class="w-full py-4 bg-primary text-on-primary font-bold rounded-2xl shadow-lg shadow-primary/20 flex items-center justify-center gap-2 tracking-wider"
                        on:click=move |_| show_form.update(|s| *s = !*s)
                    >
                        <span class="material-symbols-outlined">"add"</span>
                        "AJUKAN IZIN BARU"
                    </button>

                    // ── Form ajukan izin utk anak ──────────────────────────
                    {move || {
                        show_form
                            .get()
                            .then(|| {
                                view! {
                                    <form
                                        class="ppm-card p-5 space-y-4 anim-in"
                                        method="post"
                                        on:submit=on_submit
                                    >
                                        {move || {
                                            msg.get()
                                                .map(|(ok, text)| {
                                                    let cls = if ok {
                                                        "flex items-center gap-2 p-3 bg-secondary-container text-on-secondary-container rounded-xl text-body-sm anim-in"
                                                    } else {
                                                        "flex items-center gap-2 p-3 bg-error-container text-on-error-container rounded-xl text-body-sm anim-in"
                                                    };
                                                    view! { <div class=cls>{text}</div> }
                                                })
                                        }}

                                        <div class="space-y-2">
                                            <label class="text-label-md text-on-surface-variant">"Pilih Santri"</label>
                                            <Suspense fallback=|| ()>
                                                {move || {
                                                    home.get()
                                                        .and_then(|r| r.ok())
                                                        .map(|h| {
                                                            if child.get_untracked().is_none() {
                                                                if let Some(first) = h.children.first() {
                                                                    child.set(Some(first.id));
                                                                }
                                                            }
                                                            view! {
                                                                <div class="flex gap-2 flex-wrap">
                                                                    {h.children
                                                                        .into_iter()
                                                                        .map(|c| {
                                                                            let id = c.id;
                                                                            let cls = move || {
                                                                                if child.get() == Some(id) {
                                                                                    "px-4 py-2.5 rounded-full bg-primary text-on-primary text-body-sm font-semibold press"
                                                                                } else {
                                                                                    "px-4 py-2.5 rounded-full bg-surface-container text-on-surface-variant text-body-sm press"
                                                                                }
                                                                            };
                                                                            view! {
                                                                                <button type="button" class=cls on:click=move |_| child.set(Some(id))>
                                                                                    {c.name}
                                                                                </button>
                                                                            }
                                                                        })
                                                                        .collect_view()}
                                                                </div>
                                                            }
                                                        })
                                                }}
                                            </Suspense>
                                        </div>

                                        <div class="space-y-2">
                                            <label class="text-label-md text-on-surface-variant">"Jenis Izin"</label>
                                            <div class="grid grid-cols-3 gap-3">
                                                <KindBtn kind=kind value="sick" icon="medical_services" label="Sakit" />
                                                <KindBtn kind=kind value="leave" icon="home" label="Pulang" />
                                                <KindBtn kind=kind value="keperluan" icon="event_note" label="Keperluan" />
                                            </div>
                                        </div>

                                        <div class="grid grid-cols-2 gap-3">
                                            <div class="space-y-2">
                                                <label class="text-label-md text-on-surface-variant">"Mulai"</label>
                                                <input
                                                    type="date"
                                                    class="w-full bg-surface-container border-0 rounded-xl px-3 py-3 text-body-sm text-on-surface"
                                                    prop:value=move || start.get()
                                                    on:input=move |ev| start.set(event_target_value(&ev))
                                                    required=true
                                                />
                                            </div>
                                            <div class="space-y-2">
                                                <label class="text-label-md text-on-surface-variant">"Sampai"</label>
                                                <input
                                                    type="date"
                                                    class="w-full bg-surface-container border-0 rounded-xl px-3 py-3 text-body-sm text-on-surface"
                                                    prop:value=move || end.get()
                                                    on:input=move |ev| end.set(event_target_value(&ev))
                                                />
                                            </div>
                                        </div>

                                        <div class="space-y-2">
                                            <label class="text-label-md text-on-surface-variant">"Alasan"</label>
                                            <textarea
                                                rows="3"
                                                class="w-full bg-surface-container border-0 rounded-xl px-4 py-3 text-body-md text-on-surface resize-none"
                                                placeholder="Tuliskan alasan lengkap…"
                                                prop:value=move || reason.get()
                                                on:input=move |ev| reason.set(event_target_value(&ev))
                                            ></textarea>
                                        </div>

                                        <button
                                            type="submit"
                                            class="w-full py-3.5 bg-primary text-on-primary font-bold rounded-xl disabled:opacity-60 flex items-center justify-center gap-2"
                                            disabled=move || busy.get()
                                        >
                                            <span class="material-symbols-outlined">"send"</span>
                                            {move || if busy.get() { "Mengirim…" } else { "Ajukan Izin" }}
                                        </button>
                                    </form>
                                }
                            })
                    }}
                    </div>

                    // ── Filter status ──────────────────────────────────────
                    <div class="flex gap-2 overflow-x-auto pb-1">
                        <FilterChip filter=filter value="all" label="Semua Status" />
                        <FilterChip filter=filter value="pending" label="Menunggu" />
                        <FilterChip filter=filter value="approved" label="Disetujui" />
                        <FilterChip filter=filter value="rejected" label="Ditolak" />
                    </div>

                    // ── Daftar izin ────────────────────────────────────────
                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse grid gap-3 md:grid-cols-2">
                                <div class="h-28 bg-surface-container rounded-2xl"></div>
                                <div class="h-28 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            permits
                                .get()
                                .and_then(|r| r.ok())
                                .map(|list| {
                                    let f = filter.get();
                                    let list: Vec<ParentPermitItem> = list
                                        .into_iter()
                                        .filter(|p| {
                                            f == "all"
                                                || p.status_kind == f
                                                || (f == "pending"
                                                    && (p.status_kind == "pending_parent"
                                                        || p.status_kind == "pending_pamong"))
                                        })
                                        .collect();
                                    if list.is_empty() {
                                        view! {
                                            <EmptyState
                                                icon="event_available"
                                                title="Tidak ada izin pada filter ini"
                                            />
                                        }
                                            .into_any()
                                    } else {
                                        view! {
                                            <div class="space-y-3 md:space-y-0 md:grid md:grid-cols-2 md:gap-3">
                                                {list
                                                    .into_iter()
                                                    .map(|p| view! { <PermitCard p=p /> })
                                                    .collect_view()}
                                            </div>
                                        }
                                            .into_any()
                                    }
                                })
                        }}
                    </Suspense>
                </div>

            </div>
        </DeviceFrame>
    }
}

#[component]
fn FilterChip(filter: RwSignal<String>, value: &'static str, label: &'static str) -> impl IntoView {
    let cls = move || {
        if filter.get() == value {
            "px-4 py-2.5 rounded-full bg-secondary-container text-primary text-body-sm font-semibold whitespace-nowrap shrink-0 press"
        } else {
            "px-4 py-2.5 rounded-full bg-surface-container-lowest border border-outline-variant/60 text-on-surface-variant text-body-sm whitespace-nowrap shrink-0 press"
        }
    };
    view! {
        <button class=cls on:click=move |_| filter.set(value.to_string())>
            {label}
        </button>
    }
}

#[component]
fn KindBtn(
    kind: RwSignal<String>,
    value: &'static str,
    icon: &'static str,
    label: &'static str,
) -> impl IntoView {
    let cls = move || {
        if kind.get() == value {
            "flex items-center justify-center gap-2 py-3 rounded-xl border-2 border-primary bg-secondary-container/40 text-primary font-semibold"
        } else {
            "flex items-center justify-center gap-2 py-3 rounded-xl border border-outline-variant bg-surface-container-lowest text-on-surface font-semibold"
        }
    };
    view! {
        <button type="button" class=cls on:click=move |_| kind.set(value.to_string())>
            <span class="material-symbols-outlined">{icon}</span>
            {label}
        </button>
    }
}

/// Kartu izin anak (diajukan santri sendiri) menunggu konfirmasi orang tua.
#[component]
fn ConfirmPermitCard(
    p: PendingParentConfirm,
    on_done: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let id = p.id;
    let busy = RwSignal::new(false);
    let quote = format!("\u{201C}{}\u{201D}", p.reason);

    let decide = move |approve: bool| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            let _ = confirm_child_permit_action(id, approve).await;
            busy.set(false);
            on_done();
        });
    };

    view! {
        <div class="ppm-card p-4 card-hover anim-in border border-warning/30">
            <p class="text-[11px] font-bold tracking-[0.15em] text-on-surface-variant uppercase">
                {p.kind_label}
            </p>
            <p class="text-body-lg font-bold text-on-background mt-1">{p.child_name}</p>
            <p class="text-body-sm text-on-surface-variant flex items-center gap-1.5 mt-1">
                <span class="material-symbols-outlined text-[15px]">"calendar_month"</span>
                {p.range_label}
            </p>
            <p class="text-body-sm italic text-on-surface-variant mt-2 pb-3">{quote}</p>
            <div class="grid grid-cols-2 gap-2 pt-2 border-t border-outline-variant/40">
                <button
                    class="py-2.5 rounded-xl border border-error/40 text-error font-semibold text-body-sm disabled:opacity-50"
                    disabled=move || busy.get()
                    on:click=move |_| decide(false)
                >
                    "Tolak"
                </button>
                <button
                    class="py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-50"
                    disabled=move || busy.get()
                    on:click=move |_| decide(true)
                >
                    "Konfirmasi"
                </button>
            </div>
        </div>
    }
}

#[component]
fn PermitCard(p: ParentPermitItem) -> impl IntoView {
    let (badge, icon_badge) = match p.status_kind.as_str() {
        "approved" => (
            "px-2.5 py-1 rounded-lg text-body-sm font-semibold bg-success/10 text-success flex items-center gap-1",
            "check_circle",
        ),
        "rejected" => (
            "px-2.5 py-1 rounded-lg text-body-sm font-semibold bg-error-container text-error flex items-center gap-1",
            "cancel",
        ),
        "pending_parent" => (
            "px-2.5 py-1 rounded-lg text-body-sm font-semibold bg-info/10 text-info flex items-center gap-1",
            "family_restroom",
        ),
        _ => (
            "px-2.5 py-1 rounded-lg text-body-sm font-semibold bg-warning/10 text-warning flex items-center gap-1",
            "schedule",
        ),
    };
    let quote = format!("\u{201C}{}\u{201D}", p.reason);
    view! {
        <div class="ppm-card p-4 card-hover anim-in">
            <div class="flex items-start justify-between gap-2">
                <p class="text-[11px] font-bold tracking-[0.15em] text-on-surface-variant uppercase">
                    {p.kind_label}
                </p>
                <span class=badge>
                    <span class="material-symbols-outlined text-[15px]">{icon_badge}</span>
                    {p.status_label}
                </span>
            </div>
            <p class="text-body-lg font-bold text-on-background mt-1">{p.child_name}</p>
            <p class="text-body-sm text-on-surface-variant flex items-center gap-1.5 mt-1">
                <span class="material-symbols-outlined text-[15px]">"calendar_month"</span>
                {p.range_label}
            </p>
            <p class="text-body-sm italic text-on-surface-variant mt-2 border-b border-outline-variant/40 pb-3">
                {quote}
            </p>
            <p class="text-[10px] tracking-[0.12em] text-on-surface-variant uppercase mt-2.5">
                {p.when_label}
            </p>
        </div>
    }
}
