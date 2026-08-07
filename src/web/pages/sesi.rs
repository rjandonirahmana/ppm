//! web/pages/sesi.rs — Daftar Sesi Kelas (/sesi), dua TAB klik.
//!
//! Santri → sesi kelas yang diikutinya; admin/pamong/dewan guru (guru = dewan
//! guru, SATU entitas) → SEMUA sesi, klik kartu → /sesi/:id utk kelola (mulai
//! sesi, ganti pengajar). Tab "Terjadwal" = 7 hari ke depan; "Sudah Lewat" =
//! 7 hari ke belakang; keduanya urut tanggal DESC, daftar full-width.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::SessionItem;
use crate::web::api::sessions_list;
use crate::web::components::{kartu_grid, DeviceFrame, EmptyState, FetchError, MobileHeader};

fn status_badge(kind: &str) -> &'static str {
    match kind {
        "ongoing" => "ppm-chip bg-success/10 text-success flex items-center gap-1",
        "finished" => "ppm-chip bg-surface-container-highest text-on-surface-variant flex items-center gap-1",
        "cancelled" => "ppm-chip bg-error-container text-error flex items-center gap-1",
        _ => "ppm-chip bg-info/10 text-info flex items-center gap-1",
    }
}

fn tab_cls(active: bool) -> &'static str {
    if active {
        "py-2.5 rounded-lg bg-surface-container-lowest text-primary font-bold text-body-sm shadow-sm press"
    } else {
        "py-2.5 rounded-lg text-on-surface-variant font-semibold text-body-sm press"
    }
}

#[component]
pub fn SesiPage() -> impl IntoView {
    view! {
        <Title text="Sesi Kelas — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Sesi Kelas" />
                <div class="px-5 pt-5 space-y-3 stagger">
                    <SesiContent />
                </div>
            </div>
        </DeviceFrame>
    }
}

/// Konten daftar sesi TANPA bingkai halaman (DeviceFrame/MobileHeader) — dipakai
/// oleh `SesiPage` (/sesi, standalone) DAN sebagai tab "Sesi" di `/kelas`
/// (Kelas+Sesi digabung satu nav utk staf; santri/ortu tetap via /sesi).
#[component]
pub fn SesiContent() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { sessions_list().await });
    // Tab aktif: false = Terjadwal (default), true = Sudah Lewat. Hidup di level
    // komponen supaya pilihan tak reset saat resource refetch.
    let show_past = RwSignal::new(false);

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

    view! {
        <Suspense fallback=|| {
            view! {
                <div class="animate-pulse space-y-3">
                    <div class="h-10 bg-surface-container rounded-xl"></div>
                    <div class="grid gap-3 md:grid-cols-2">
                        <div class="h-24 bg-surface-container rounded-2xl"></div>
                        <div class="h-24 bg-surface-container rounded-2xl"></div>
                        <div class="h-24 bg-surface-container rounded-2xl"></div>
                        <div class="h-24 bg-surface-container rounded-2xl hidden md:block"></div>
                    </div>
                </div>
            }
        }>
            {move || {
                data.get()
                    .map(|res| match res {
                        Ok(d) => {
                            // Staf (admin/pamong/guru=dewan guru) → detail sesi
                            // (kelola); santri → RUANG LIVE (ikut & bertanya).
                            let is_santri = matches!(d.role.as_str(), "santri" | "santri_finance");
                            let n_up = d.upcoming.len();
                            let n_past = d.past.len();
                            let lists = StoredValue::new((d.upcoming, d.past));
                            view! {
                                <p class="text-body-sm text-on-surface-variant">
                                    {if d.all_scope {
                                        "Semua sesi kelas (kelola & pantau)."
                                    } else {
                                        "Sesi kelas yang kamu ikuti."
                                    }}
                                </p>
                                // Tab klik: Terjadwal (7 hari ke depan) vs
                                // Sudah Lewat (7 hari ke belakang), DESC.
                                <div class="grid grid-cols-2 gap-1 bg-surface-container rounded-xl p-1">
                                    <button
                                        class=move || tab_cls(!show_past.get())
                                        on:click=move |_| show_past.set(false)
                                    >
                                        {format!("Terjadwal ({n_up})")}
                                    </button>
                                    <button
                                        class=move || tab_cls(show_past.get())
                                        on:click=move |_| show_past.set(true)
                                    >
                                        {format!("Sudah Lewat ({n_past})")}
                                    </button>
                                </div>
                                // Desktop: kartu sesi 2 kolom.
                                {move || {
                                        let (up, past) = lists.get_value();
                                        let (items, empty) = if show_past.get() {
                                            (past, "Tidak ada sesi 7 hari terakhir.")
                                        } else {
                                            (up, "Belum ada sesi 7 hari ke depan.")
                                        };
                                        if items.is_empty() {
                                            view! {
                                                <EmptyState icon="cast_for_education" title=empty />
                                            }
                                                .into_any()
                                        } else {
                                            kartu_grid(
                                                    items
                                                        .into_iter()
                                                        .map(|it| {
                                                            let id = it.id;
                                                            let href = if is_santri {
                                                                format!("/sesi/{id}/live")
                                                            } else {
                                                                format!("/sesi/{id}")
                                                            };
                                                            view! {
                                                                <a href=href class="block">
                                                                    <SessionCard it=it />
                                                                </a>
                                                            }
                                                                .into_any()
                                                        })
                                                        .collect(),
                                                )
                                                .into_any()
                                        }
                                    }}
                            }
                                .into_any()
                        }
                        Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                    })
            }}
        </Suspense>
    }
}

#[component]
fn SessionCard(it: SessionItem) -> impl IntoView {
    let badge = status_badge(&it.status_kind);
    let is_ongoing = it.status_kind == "ongoing";
    let meta = format!("{} • {} • {}", it.class_name, it.category, it.teacher);
    let when = format!("{} • {}", it.date_label, it.time_label);
    view! {
        <div class="ppm-card p-4 card-hover anim-in">
            <div class="flex items-start gap-3">
                <div class="w-11 h-11 ppm-tile">
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
