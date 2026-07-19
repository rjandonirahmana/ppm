//! web/pages/sesi.rs — Daftar Sesi Kelas (/sesi).
//!
//! Santri → sesi kelas yang diikutinya; admin/pamong/dewan guru → SEMUA sesi
//! (kelak bisa mengelola/update sesi dari sini). Nav bawah menyesuaikan peran.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::SessionItem;
use crate::web::api::sessions_list;
use crate::web::components::{DeviceFrame, FetchError, MobileHeader};

fn status_badge(kind: &str) -> &'static str {
    match kind {
        "ongoing" => "px-2.5 py-1 rounded-full text-[10px] font-bold tracking-wider bg-success/10 text-success flex items-center gap-1",
        "finished" => "px-2.5 py-1 rounded-full text-[10px] font-bold tracking-wider bg-surface-container-highest text-on-surface-variant flex items-center gap-1",
        "cancelled" => "px-2.5 py-1 rounded-full text-[10px] font-bold tracking-wider bg-error-container text-error flex items-center gap-1",
        _ => "px-2.5 py-1 rounded-full text-[10px] font-bold tracking-wider bg-info/10 text-info flex items-center gap-1",
    }
}

#[component]
pub fn SesiPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { sessions_list().await });

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

    view! {
        <Title text="Sesi Kelas — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto">
                <MobileHeader title="Sesi Kelas" />

                <div class="px-5 pt-5 space-y-3 stagger">
                    <Suspense fallback=|| {
                        view! {
                            <div class="space-y-3 animate-pulse">
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => {
                                        view! {
                                            <p class="text-body-sm text-on-surface-variant">
                                                {if d.all_scope {
                                                    "Semua sesi kelas (kelola & pantau)."
                                                } else {
                                                    "Sesi kelas yang kamu ikuti."
                                                }}
                                            </p>
                                            {if d.items.is_empty() {
                                                view! {
                                                    <div class="bg-surface-container rounded-2xl p-8 text-center text-body-sm text-on-surface-variant">
                                                        "Belum ada sesi kelas."
                                                    </div>
                                                }
                                                    .into_any()
                                            } else {
                                                d.items
                                                    .into_iter()
                                                    .map(|it| view! { <SessionCard it=it /> })
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
fn SessionCard(it: SessionItem) -> impl IntoView {
    let badge = status_badge(&it.status_kind);
    let is_ongoing = it.status_kind == "ongoing";
    let meta = format!("{} • {}", it.class_name, it.teacher);
    let when = format!("{} • {}", it.date_label, it.time_label);
    view! {
        <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4 card-hover anim-in">
            <div class="flex items-start gap-3">
                <div class="w-11 h-11 rounded-xl bg-secondary-container flex items-center justify-center text-primary shrink-0">
                    <span class="material-symbols-outlined">"menu_book"</span>
                </div>
                <div class="flex-1 min-w-0">
                    <div class="flex items-center gap-2">
                        <p class="text-body-md font-bold text-on-background truncate flex-1">
                            {it.title}
                        </p>
                        <span class=badge>
                            {is_ongoing
                                .then(|| {
                                    view! {
                                        <span class="w-1.5 h-1.5 rounded-full bg-success pulse-dot"></span>
                                    }
                                })}
                            {it.status_label}
                        </span>
                    </div>
                    <p class="text-body-sm text-on-surface-variant truncate mt-0.5">{meta}</p>
                    <p class="text-body-sm text-on-surface-variant flex items-center gap-1 mt-1">
                        <span class="material-symbols-outlined text-[15px]">"schedule"</span>
                        {when}
                    </p>
                </div>
            </div>
        </div>
    }
}
