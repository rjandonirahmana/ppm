//! web/pages/sesi_live.rs — Ruang sesi LIVE (/sesi/:id/live) ala mockup Halaqah.
//!
//! Staf (dewan/guru/pamong/admin): mulai/akhiri sesi. Santri peserta kelas: ikut
//! & bertanya lewat chat. Chat & status di-refresh polling 4 dtk (tanpa WS).
//! CATATAN: SUARA online (WebRTC) BELUM tersedia — ruang ini menyiapkan sesi
//! live + tanya-jawab; audio menyusul (butuh subsistem SFU terpisah).

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use crate::models::SessionLiveData;
use crate::web::api::{post_session_chat, session_live_data, set_session_live};
use crate::web::components::{DeviceFrame, FetchError};

#[component]
pub fn SesiLivePage() -> impl IntoView {
    let params = use_params_map();
    let session_id =
        Memo::new(move |_| params.read().get("id").and_then(|s| s.parse::<i64>().ok()).unwrap_or(0));
    let data =
        Resource::new(move || session_id.get(), |id| async move { session_live_data(id).await });

    // Polling chat/status tiap 4 dtk (klien saja; SSR tak boleh set_interval).
    #[cfg(target_arch = "wasm32")]
    {
        let handle = set_interval_with_handle(
            move || data.refetch(),
            std::time::Duration::from_secs(4),
        )
        .ok();
        on_cleanup(move || {
            if let Some(h) = handle {
                h.clear();
            }
        });
    }

    view! {
        <Title text="Sesi Live — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-28 max-w-md mx-auto">
                <Suspense fallback=|| {
                    view! {
                        <div class="px-5 pt-5 space-y-3 animate-pulse">
                            <div class="h-40 bg-surface-container rounded-2xl"></div>
                            <div class="h-64 bg-surface-container rounded-2xl"></div>
                        </div>
                    }
                }>
                    {move || {
                        data.get()
                            .map(|res| match res {
                                Ok(d) => {
                                    view! { <LiveBody d=d refetch=move || data.refetch() /> }
                                        .into_any()
                                }
                                Err(e) => {
                                    view! {
                                        <div class="px-5 pt-5">
                                            <FetchError err=e.to_string() />
                                        </div>
                                    }
                                        .into_any()
                                }
                            })
                    }}
                </Suspense>
            </div>
        </DeviceFrame>
    }
}

#[component]
fn LiveBody(d: SessionLiveData, refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let session_id = d.id;
    let is_live = d.status_kind == "ongoing";
    let is_done = d.status_kind == "finished";
    let can_manage = d.can_manage;
    let teacher_initial: String =
        d.teacher.split_whitespace().take(2).filter_map(|w| w.chars().next()).collect();

    let msg = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let err = RwSignal::new(Option::<String>::None);
    let send = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let m = msg.get_untracked();
        if m.trim().is_empty() || busy.get_untracked() {
            return;
        }
        busy.set(true);
        err.set(None);
        leptos::task::spawn_local(async move {
            match post_session_chat(session_id, m).await {
                Ok(_) => {
                    msg.set(String::new());
                    refetch();
                }
                Err(e) => {
                    let s = e.to_string();
                    err.set(Some(s.rsplit(": ").next().unwrap_or(&s).to_string()));
                }
            }
            busy.set(false);
        });
    };
    let toggle_live = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            let _ = set_session_live(session_id, !is_live).await;
            busy.set(false);
            refetch();
        });
    };

    view! {
        // ── Header ──────────────────────────────────────────────────────────
        <header class="sticky top-0 z-10 bg-surface/90 backdrop-blur border-b border-outline-variant/50 px-5 py-3.5 flex items-center gap-3">
            <a href="/sesi" class="press">
                <span class="material-symbols-outlined text-on-background">"close"</span>
            </a>
            <h1 class="text-body-lg font-bold text-primary flex-1 truncate">{d.title.clone()}</h1>
            <span class="flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-secondary-container text-primary text-body-sm font-bold">
                <span class="material-symbols-outlined text-[17px]">"groups"</span>
                {d.member_count}
            </span>
        </header>

        <div class="px-5 pt-6 space-y-5">
            // ── Pembicara ───────────────────────────────────────────────────
            <div class="text-center anim-in">
                <div class="relative w-28 h-28 mx-auto">
                    <div class="w-28 h-28 rounded-full bg-secondary-container text-primary flex items-center justify-center text-3xl font-bold ring-4 ring-primary/15">
                        {teacher_initial}
                    </div>
                    {is_live
                        .then(|| view! {
                            <span class="absolute bottom-1 right-1 w-9 h-9 rounded-full bg-primary text-on-primary flex items-center justify-center ring-2 ring-surface">
                                <span class="material-symbols-outlined text-[18px]">"mic"</span>
                            </span>
                        })}
                </div>
                <div class="mt-3">
                    {if is_live {
                        view! {
                            <span class="inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-error-container text-error text-[11px] font-bold tracking-wider">
                                <span class="w-1.5 h-1.5 rounded-full bg-error pulse-dot"></span>
                                "LIVE"
                            </span>
                        }
                            .into_any()
                    } else if is_done {
                        view! {
                            <span class="inline-flex px-3 py-1 rounded-full bg-surface-container-highest text-on-surface-variant text-[11px] font-bold tracking-wider">
                                "SESI SELESAI"
                            </span>
                        }
                            .into_any()
                    } else {
                        view! {
                            <span class="inline-flex px-3 py-1 rounded-full bg-info/10 text-info text-[11px] font-bold tracking-wider">
                                "BELUM DIMULAI"
                            </span>
                        }
                            .into_any()
                    }}
                </div>
                <h2 class="text-display-md text-on-background mt-2">{d.teacher.clone()}</h2>
                <p class="text-body-sm text-on-surface-variant mt-1">{d.class_name.clone()}</p>
            </div>

            // ── Kontrol staf: mulai/akhiri ──────────────────────────────────
            {can_manage
                .then(|| {
                    let (label, icon, cls) = if is_live {
                        ("Akhiri Sesi", "call_end", "w-full py-3.5 rounded-xl bg-error text-on-error font-bold text-body-md flex items-center justify-center gap-2 press disabled:opacity-50")
                    } else {
                        ("Mulai Sesi Live", "play_circle", "w-full py-3.5 rounded-xl bg-primary text-on-primary font-bold text-body-md flex items-center justify-center gap-2 press disabled:opacity-50")
                    };
                    view! {
                        <button class=cls disabled=move || busy.get() on:click=toggle_live>
                            <span class="material-symbols-outlined">{icon}</span>
                            {label}
                        </button>
                    }
                })}

            // ── Obrolan sesi ────────────────────────────────────────────────
            <div>
                <div class="flex items-center justify-between mb-2">
                    <h3 class="text-body-lg font-bold text-on-background">"Obrolan Sesi"</h3>
                    <p class="text-[11px] text-on-surface-variant">"diperbarui otomatis"</p>
                </div>
                {if d.chats.is_empty() {
                    view! {
                        <div class="bg-surface-container rounded-2xl p-6 text-center text-body-sm text-on-surface-variant">
                            "Belum ada obrolan. Silakan bertanya!"
                        </div>
                    }
                        .into_any()
                } else {
                    view! {
                        <div class="space-y-2.5">
                            {d.chats
                                .iter()
                                .cloned()
                                .map(|c| {
                                    let initial: String = c
                                        .name
                                        .split_whitespace()
                                        .take(2)
                                        .filter_map(|w| w.chars().next())
                                        .collect();
                                    view! {
                                        <div class="flex items-start gap-2.5 anim-in">
                                            <div class="w-9 h-9 rounded-full bg-secondary-container text-primary flex items-center justify-center text-[11px] font-bold shrink-0">
                                                {initial.to_uppercase()}
                                            </div>
                                            <div class="bg-surface-container-lowest border border-outline-variant/50 rounded-2xl rounded-tl-md px-3.5 py-2.5 max-w-[85%]">
                                                <div class="flex items-baseline gap-2">
                                                    <p class="text-[12px] font-bold text-primary">{c.name.clone()}</p>
                                                    <p class="text-[10px] text-on-surface-variant">{c.time_label.clone()}</p>
                                                </div>
                                                <p class="text-body-sm text-on-background break-words">{c.message.clone()}</p>
                                            </div>
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                        .into_any()
                }}
            </div>
        </div>

        // ── Input chat (fixed bawah, sejajar kolom) ─────────────────────────
        <form
            class="fixed bottom-0 inset-x-0 max-w-md mx-auto bg-surface-container-lowest border-t border-outline-variant/60 px-4 py-3 flex items-center gap-2 z-20"
            on:submit=send
        >
            {move || err.get().map(|e| view! {
                <p class="absolute -top-8 left-4 right-4 text-[11px] text-error bg-error-container rounded-lg px-3 py-1.5">{e}</p>
            })}
            <input
                type="text"
                class="flex-1 bg-surface-container border-0 rounded-full px-4 py-2.5 text-body-sm text-on-background placeholder:text-on-surface-variant"
                placeholder="Tulis pertanyaan…"
                prop:value=move || msg.get()
                on:input=move |ev| msg.set(event_target_value(&ev))
                maxlength="500"
            />
            <button
                type="submit"
                class="w-11 h-11 rounded-full bg-primary text-on-primary flex items-center justify-center press disabled:opacity-50 shrink-0"
                disabled=move || busy.get()
            >
                <span class="material-symbols-outlined">"send"</span>
            </button>
        </form>
    }
}
