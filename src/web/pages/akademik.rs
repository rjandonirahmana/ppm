//! web/pages/akademik.rs — /akademik (SANTRI): self-report progres bacaan/
//! hafalan pada buku (Qur'an/Hadist dll) yang sudah didaftarkan admin.
//! MENGGANTIKAN item navbar "Laporan" di sisi santri (rapor pribadi pindah
//! ke /riwayat — lihat riwayat.rs) — santri sendiri yang mengisi bagian mana
//! yang masih "bolong" (belum lancar), agar ustadz bisa lihat halaman apa
//! yang paling banyak kosong di kelasnya (lewat tab Akademik /kelas).

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::BookProgressItem;
use crate::web::api::{own_book_progress_data, set_own_book_progress_action};
use crate::web::components::{DeviceFrame, FetchError, MobileHeader};

#[component]
pub fn AkademikSantriPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { own_book_progress_data().await });

    Effect::new(move |_| {
        if let Some(Err(e)) = data.get() {
            if e.to_string().contains("unauth") {
                #[cfg(target_arch = "wasm32")]
                if let Some(w) = web_sys::window() {
                    let _ = w.location().replace("/login");
                }
            }
        }
    });

    view! {
        <Title text="Akademik — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Akademik" subtitle="Isi progres bacaan & hafalanmu sendiri" />

                <div class="px-5 pt-5 space-y-4 stagger">
                    <p class="text-body-sm text-on-surface-variant">
                        "Tandai halaman yang masih perlu diulang/belum lancar pada tiap materi. Data ini membantu ustadz melihat bagian mana yang paling banyak kosong di kelas."
                    </p>
                    <Suspense fallback=|| {
                        view! {
                            <div class="space-y-3 animate-pulse">
                                <div class="h-28 bg-surface-container rounded-2xl"></div>
                                <div class="h-28 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(items) => {
                                        if items.is_empty() {
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
                                        } else {
                                            view! {
                                                <div class="space-y-3 md:grid md:grid-cols-2 md:gap-3 md:space-y-0">
                                                    {items
                                                        .into_iter()
                                                        .map(|b| {
                                                            view! {
                                                                <OwnBookCard b=b refetch=move || data.refetch() />
                                                            }
                                                        })
                                                        .collect_view()}
                                                </div>
                                            }
                                                .into_any()
                                        }
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
fn OwnBookCard(b: BookProgressItem, refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let book_id = b.book_id;
    let editing = RwSignal::new(false);
    let pct_v = RwSignal::new(b.percentage.to_string());
    let missing_v = RwSignal::new(b.missing_pages_label.clone());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<String>::None);

    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let (pct, missing) = (pct_v.get_untracked(), missing_v.get_untracked());
        leptos::task::spawn_local(async move {
            match set_own_book_progress_action(book_id, pct, missing).await {
                Ok(_) => {
                    editing.set(false);
                    refetch();
                }
                Err(e) => {
                    let m = e.to_string();
                    msg.set(Some(m.rsplit(": ").next().unwrap_or(&m).to_string()));
                }
            }
            busy.set(false);
        });
    };

    let title = b.book_title.clone();
    let total_pages = b.total_pages;
    let pct = b.percentage;
    let missing_label = b.missing_pages_label.clone();
    let field =
        "w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface";

    view! {
        <div class="ppm-card p-4 space-y-2 card-hover anim-in">
            <div class="flex items-center justify-between gap-2">
                <div class="flex items-center gap-2 min-w-0">
                    <span class="w-9 h-9 rounded-lg bg-secondary-container flex items-center justify-center text-primary shrink-0">
                        <span class="material-symbols-outlined text-[18px]">"menu_book"</span>
                    </span>
                    <div class="min-w-0">
                        <p class="text-body-md font-semibold text-on-background truncate">{title}</p>
                        <p class="text-[11px] text-on-surface-variant">{format!("{total_pages} halaman")}</p>
                    </div>
                </div>
                <button
                    class="w-8 h-8 rounded-lg bg-surface-container text-primary flex items-center justify-center shrink-0 press"
                    on:click=move |_| editing.update(|e| *e = !*e)
                    aria-label="Ubah progres"
                >
                    <span class="material-symbols-outlined text-[16px]">"edit"</span>
                </button>
            </div>
            <div class="h-1.5 bg-surface-container rounded-full overflow-hidden mt-1 bar-grow">
                <div class="h-full bg-primary" style=format!("width: {pct}%")></div>
            </div>
            <div class="flex items-center justify-between text-[11px] text-on-surface-variant">
                <span>{format!("{pct}% selesai")}</span>
                {(!missing_label.is_empty())
                    .then(|| view! { <span>{format!("Kosong: {missing_label}")}</span> })}
            </div>

            {move || {
                msg.get()
                    .map(|t| {
                        view! {
                            <div class="p-2 bg-error-container text-on-error-container rounded-lg text-[11px]">
                                {t}
                            </div>
                        }
                    })
            }}

            {move || {
                editing
                    .get()
                    .then(|| {
                        view! {
                            <form class="pt-1 space-y-2 anim-in" method="post" on:submit=save>
                                <label class="space-y-1 block">
                                    <span class="text-[11px] text-on-surface-variant">"Persentase selesai (0-100)"</span>
                                    <input
                                        type="number"
                                        min="0"
                                        max="100"
                                        class=field
                                        prop:value=move || pct_v.get()
                                        on:input=move |ev| pct_v.set(event_target_value(&ev))
                                    />
                                </label>
                                <label class="space-y-1 block">
                                    <span class="text-[11px] text-on-surface-variant">
                                        "Halaman yang masih bolong (mis. 11-20, 45-50) — kosongkan bila tak ada"
                                    </span>
                                    <input
                                        type="text"
                                        class=field
                                        placeholder="11-20, 45-50"
                                        prop:value=move || missing_v.get()
                                        on:input=move |ev| missing_v.set(event_target_value(&ev))
                                    />
                                </label>
                                <div class="grid grid-cols-2 gap-2">
                                    <button
                                        type="button"
                                        class="py-2 rounded-lg border border-outline-variant text-on-surface font-semibold text-[11px]"
                                        on:click=move |_| editing.set(false)
                                    >
                                        "Batal"
                                    </button>
                                    <button
                                        type="submit"
                                        class="py-2 rounded-lg bg-primary text-on-primary font-semibold text-[11px] disabled:opacity-60"
                                        disabled=move || busy.get()
                                    >
                                        {move || if busy.get() { "Menyimpan…" } else { "Simpan" }}
                                    </button>
                                </div>
                            </form>
                        }
                    })
            }}
        </div>
    }
}
