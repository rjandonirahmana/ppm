//! web/pages/akademik.rs — /akademik (SANTRI): self-report progres per-unit
//! materi (migrasi 25). Tiap materi punya kategori:
//!   • hadist → unit = HALAMAN (grid 1..total_pages);
//!   • quran  → unit = AYAT per SURAT (pilih surat → grid 1..ayat).
//! Tiap unit 3-status: KOSONG → SETENGAH → PENUH (klik memutar, mirip kalender).
//! Simpan → server hitung ulang persentase.

use std::collections::HashMap;

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::BookProgressItem;
use crate::web::api::{own_book_progress_data, set_own_book_progress_action};
use crate::web::components::{DeviceFrame, FetchError, MobileHeader};

#[component]
pub fn AkademikSantriPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { own_book_progress_data().await });

    crate::web::components::guard_sesi(data);

    view! {
        <Title text="Akademik — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Akademik" subtitle="Isi progres bacaan & hafalanmu sendiri" />

                <div class="px-5 pt-5 space-y-4 stagger">
                    <div class="ppm-card p-3 flex items-center justify-around text-[11px]">
                        <span class="flex items-center gap-1.5">
                            <span class="w-3.5 h-3.5 rounded bg-surface-container-highest border border-outline-variant"></span>
                            "Kosong"
                        </span>
                        <span class="flex items-center gap-1.5">
                            <span class="w-3.5 h-3.5 rounded bg-warning/70"></span>
                            "Setengah"
                        </span>
                        <span class="flex items-center gap-1.5">
                            <span class="w-3.5 h-3.5 rounded bg-primary"></span>
                            "Penuh"
                        </span>
                    </div>
                    <p class="text-body-sm text-on-surface-variant">
                        "Ketuk tiap halaman/ayat untuk menandai penuh, setengah, atau kosong. Jangan lupa Simpan."
                    </p>

                    <Suspense fallback=|| {
                        view! {
                            <div class="space-y-3 animate-pulse">
                                <div class="h-40 bg-surface-container rounded-2xl"></div>
                                <div class="h-40 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(items) if items.is_empty() => {
                                        view! {
                                            <div class="ppm-empty space-y-1.5">
                                                <span class="material-symbols-outlined text-4xl text-on-surface-variant/60">
                                                    "menu_book"
                                                </span>
                                                <p class="text-body-md font-semibold text-on-background">
                                                    "Belum ada materi terdaftar"
                                                </p>
                                                <p class="text-body-sm text-on-surface-variant">
                                                    "Tunggu admin/ustadz menambahkan materi terlebih dahulu."
                                                </p>
                                            </div>
                                        }
                                            .into_any()
                                    }
                                    Ok(items) => {
                                        view! {
                                            <div class="space-y-4">
                                                {items
                                                    .into_iter()
                                                    .map(|b| view! { <MateriCard b=b refetch=move || data.refetch() /> })
                                                    .collect_view()}
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

/// Satu tombol unit (halaman/ayat) 3-status; klik memutar kosong→setengah→penuh.
#[component]
fn UnitCell(label: i32, ukey: String, status: RwSignal<HashMap<String, u8>>) -> impl IntoView {
    let k_cls = ukey.clone();
    let cls = move || {
        let s = status.with(|m| m.get(&k_cls).copied().unwrap_or(0));
        match s {
            2 => "aspect-square rounded text-[10px] font-bold flex items-center justify-center bg-primary text-on-primary press",
            1 => "aspect-square rounded text-[10px] font-bold flex items-center justify-center bg-warning/70 text-on-background press",
            _ => "aspect-square rounded text-[10px] flex items-center justify-center bg-surface-container-highest text-on-surface-variant press",
        }
    };
    let click = move |_| {
        status.update(|m| {
            let cur = m.get(&ukey).copied().unwrap_or(0);
            let next = (cur + 1) % 3;
            if next == 0 {
                m.remove(&ukey);
            } else {
                m.insert(ukey.clone(), next);
            }
        })
    };
    view! {
        <button type="button" class=cls on:click=click>
            {label}
        </button>
    }
}

#[component]
fn MateriCard(b: BookProgressItem, refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let book_id = b.book_id;
    let total = b.total_pages.max(1);
    let is_quran = b.category == "quran";
    let title = b.book_title.clone();
    let surahs = StoredValue::new(b.surahs.clone());
    let status = RwSignal::new(b.unit_status.clone());
    let sel_surah = RwSignal::new(0usize);
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    // Persentase live dari peta status.
    let pct = move || {
        let sum: i32 = status.with(|m| m.values().map(|&v| v as i32).sum());
        ((sum as f64) / (total as f64 * 2.0) * 100.0).round() as i32
    };

    let save = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let json = serde_json::to_string(&status.get_untracked()).unwrap_or_else(|_| "{}".into());
        leptos::task::spawn_local(async move {
            match set_own_book_progress_action(book_id, json).await {
                Ok(_) => {
                    msg.set(Some((true, "Tersimpan.".into())));
                    refetch();
                }
                Err(e) => {
                    msg.set(Some((false, crate::web::components::pesan_galat(e))));
                }
            }
            busy.set(false);
        });
    };

    let cat_badge = if is_quran {
        "ppm-chip-sm bg-primary/10 text-primary"
    } else {
        "ppm-chip-sm bg-secondary-container text-primary"
    };
    let cat_label = if is_quran { "QUR'AN" } else { "HADIST" };

    view! {
        <div class="ppm-card p-4 space-y-3 anim-in">
            <div class="flex items-center justify-between gap-2">
                <div class="flex items-center gap-2 min-w-0">
                    <span class=cat_badge>{cat_label}</span>
                    <p class="text-body-md font-semibold text-on-background truncate">{title}</p>
                </div>
                <span class="text-body-sm font-bold text-primary shrink-0">{move || format!("{}%", pct())}</span>
            </div>
            <div class="h-1.5 bg-surface-container rounded-full overflow-hidden">
                <div class="h-full bg-primary transition-all" style=move || format!("width: {}%", pct())></div>
            </div>

            {move || {
                if is_quran {
                    // Chips surat → grid ayat surat terpilih.
                    let list = surahs.get_value();
                    let chips = list
                        .iter()
                        .enumerate()
                        .map(|(i, s)| {
                            let nm = s.name.clone();
                            let cls = move || {
                                if sel_surah.get() == i {
                                    "px-2.5 py-1 rounded-full text-[11px] font-semibold bg-primary text-on-primary press whitespace-nowrap"
                                } else {
                                    "px-2.5 py-1 rounded-full text-[11px] bg-surface-container text-on-surface press whitespace-nowrap"
                                }
                            };
                            view! {
                                <button type="button" class=cls on:click=move |_| sel_surah.set(i)>
                                    {nm}
                                </button>
                            }
                        })
                        .collect_view();
                    view! {
                        <div class="flex flex-wrap gap-1.5">{chips}</div>
                        {move || {
                            let idx = sel_surah.get();
                            let list = surahs.get_value();
                            let Some(s) = list.get(idx) else { return ().into_any() };
                            let ayat = s.ayat.max(0);
                            let sname = s.name.clone();
                            view! {
                                <p class="text-[11px] text-on-surface-variant">
                                    {format!("{} — {} ayat", sname, ayat)}
                                </p>
                                <div class="grid grid-cols-10 gap-1 max-h-72 overflow-y-auto">
                                    {(1..=ayat)
                                        .map(|a| {
                                            view! { <UnitCell label=a ukey=format!("{idx}:{a}") status=status /> }
                                        })
                                        .collect_view()}
                                </div>
                            }
                                .into_any()
                        }}
                    }
                        .into_any()
                } else {
                    // Grid halaman.
                    view! {
                        <div class="grid grid-cols-10 gap-1 max-h-72 overflow-y-auto">
                            {(1..=total)
                                .map(|p| view! { <UnitCell label=p ukey=p.to_string() status=status /> })
                                .collect_view()}
                        </div>
                    }
                        .into_any()
                }
            }}

            {move || {
                msg.get()
                    .map(|(ok, t)| {
                        let cls = if ok {
                            "text-[11px] text-success"
                        } else {
                            "text-[11px] text-error"
                        };
                        view! { <p class=cls>{t}</p> }
                    })
            }}
            <button
                class="w-full py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-sm press disabled:opacity-60"
                disabled=move || busy.get()
                on:click=save
            >
                {move || if busy.get() { "Menyimpan…" } else { "Simpan Progres" }}
            </button>
        </div>
    }
}
