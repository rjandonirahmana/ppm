//! web/pages/kelas.rs — Manajemen Kelas (admin/dewan guru/pamong).
//!
//! Kelola kurikulum & pembagian santri: statistik total kelas/santri, cari
//! kelas, buat kelas baru, dan buka detail tiap kelas ("Lihat Santri" →
//! /kelas/:id untuk anggota, jadwal, sesi). Data ASLI dari DB.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::KelasItem;
use crate::web::api::{create_class_action, kelas_list};
use crate::web::components::{DeviceFrame, FetchError, MobileHeader};

#[component]
pub fn KelasPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { kelas_list().await });

    Effect::new(move |_| {
        if let Some(Err(e)) = data.get() {
            // Hanya lempar ke login bila BELUM login (unauth). `forbidden` (login
            // tapi peran tak diizinkan) ditangani FetchError, bukan bounce ke login.
            if e.to_string().contains("unauth") {
                #[cfg(target_arch = "wasm32")]
                if let Some(w) = web_sys::window() {
                    let _ = w.location().replace("/login");
                }
            }
        }
    });

    let query = RwSignal::new(String::new());
    let show_form = RwSignal::new(false);

    view! {
        <Title text="Manajemen Kelas — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Manajemen Kelas" subtitle="Kelola kurikulum & pembagian santri" />

                <div class="px-5 pt-5 space-y-4 stagger">
                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="h-20 bg-surface-container rounded-2xl"></div>
                                <div class="grid gap-4 md:grid-cols-2">
                                    <div class="h-32 bg-surface-container rounded-2xl"></div>
                                    <div class="h-32 bg-surface-container rounded-2xl"></div>
                                    <div class="h-32 bg-surface-container rounded-2xl hidden md:block"></div>
                                    <div class="h-32 bg-surface-container rounded-2xl hidden md:block"></div>
                                </div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => {
                                        let items = d.items.clone();
                                        view! {
                                            // ── Statistik ──────────────────────
                                            <div class="grid grid-cols-2 gap-3">
                                                <div class="ppm-card p-4 flex items-center gap-3 card-hover">
                                                    <div class="w-10 h-10 rounded-xl bg-secondary-container text-primary flex items-center justify-center">
                                                        <span class="material-symbols-outlined">"school"</span>
                                                    </div>
                                                    <div>
                                                        <p class="text-2xl font-bold text-on-background" data-count=d.total_kelas.to_string()>
                                                            {d.total_kelas}
                                                        </p>
                                                        <p class="text-[11px] font-bold tracking-wider text-on-surface-variant">
                                                            "TOTAL KELAS"
                                                        </p>
                                                    </div>
                                                </div>
                                                <div class="ppm-card p-4 flex items-center gap-3 card-hover">
                                                    <div class="w-10 h-10 rounded-xl bg-secondary-container text-primary flex items-center justify-center">
                                                        <span class="material-symbols-outlined">"groups"</span>
                                                    </div>
                                                    <div>
                                                        <p class="text-2xl font-bold text-on-background" data-count=d.total_santri.to_string()>
                                                            {d.total_santri}
                                                        </p>
                                                        <p class="text-[11px] font-bold tracking-wider text-on-surface-variant">
                                                            "TOTAL SANTRI"
                                                        </p>
                                                    </div>
                                                </div>
                                            </div>

                                            // ── Cari ───────────────────────────
                                            <div class="relative">
                                                <span class="material-symbols-outlined absolute left-3.5 top-1/2 -translate-y-1/2 text-outline">
                                                    "search"
                                                </span>
                                                <input
                                                    type="text"
                                                    class="w-full pl-11 pr-4 py-3.5 bg-surface-container border-0 rounded-xl text-body-md text-on-surface"
                                                    placeholder="Cari nama kelas atau ustadz…"
                                                    prop:value=move || query.get()
                                                    on:input=move |ev| query.set(event_target_value(&ev))
                                                />
                                            </div>

                                            // ── Form tambah kelas ──────────────
                                            // Datalist kategori (autocomplete + boleh ketik baru)
                                            {
                                                let mut cats: Vec<String> = items
                                                    .iter()
                                                    .map(|k| k.category.clone())
                                                    .filter(|c| !c.is_empty())
                                                    .collect();
                                                cats.sort();
                                                cats.dedup();
                                                view! {
                                                    <datalist id="kategori-kelas">
                                                        {cats
                                                            .into_iter()
                                                            .map(|c| view! { <option value=c></option> })
                                                            .collect_view()}
                                                    </datalist>
                                                }
                                            }

                                            <TambahKelas show_form=show_form refetch=move || data.refetch() />

                                            // ── Daftar kelas (filter klien) ────
                                            {move || {
                                                let q = query.get().to_lowercase();
                                                let list: Vec<KelasItem> = items
                                                    .clone()
                                                    .into_iter()
                                                    .filter(|k| {
                                                        q.is_empty()
                                                            || k.name.to_lowercase().contains(&q)
                                                            || k.teacher.to_lowercase().contains(&q)
                                                    })
                                                    .collect();
                                                if list.is_empty() {
                                                    view! {
                                                        <div class="ppm-empty">
                                                            "Belum ada kelas. Tambahkan kelas baru di atas."
                                                        </div>
                                                    }
                                                        .into_any()
                                                } else {
                                                    // Desktop: kartu kelas 2 kolom (mockup manajemen kelas).
                                                    view! {
                                                        <div class="space-y-4 md:space-y-0 md:grid md:grid-cols-2 md:gap-4">
                                                            {list.into_iter()
                                                                .map(|k| view! { <KelasCard k=k /> })
                                                                .collect_view()}
                                                        </div>
                                                    }
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
                </div>

            </div>
        </DeviceFrame>
    }
}

#[component]
fn TambahKelas(show_form: RwSignal<bool>, refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let category = RwSignal::new(String::new());
    let desc = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        error.set(None);
        let (n, cat, d) = (
            name.get_untracked(),
            category.get_untracked(),
            desc.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            match create_class_action(n, cat, d).await {
                Ok(_) => {
                    name.set(String::new());
                    category.set(String::new());
                    desc.set(String::new());
                    show_form.set(false);
                    refetch();
                }
                Err(e) => {
                    let m = e.to_string();
                    error.set(Some(m.rsplit(": ").next().unwrap_or(&m).to_string()));
                }
            }
            busy.set(false);
        });
    };

    view! {
        {move || {
            if show_form.get() {
                view! {
                    <form
                        class="ppm-card p-5 space-y-3 anim-in"
                        method="post"
                        on:submit=submit
                    >
                        <div class="flex items-center gap-2">
                            <span class="material-symbols-outlined text-primary">"add_circle"</span>
                            <h2 class="text-body-lg font-bold text-on-background">"Kelas Baru"</h2>
                        </div>
                        {move || {
                            error
                                .get()
                                .map(|e| {
                                    view! {
                                        <div class="p-3 bg-error-container text-on-error-container rounded-xl text-body-sm anim-in">
                                            {e}
                                        </div>
                                    }
                                })
                        }}
                        <input
                            type="text"
                            class="w-full bg-surface-container border-0 rounded-xl px-4 py-3 text-body-md text-on-surface"
                            placeholder="Nama kelas (mis. Kelas Lambatan A1)"
                            prop:value=move || name.get()
                            on:input=move |ev| name.set(event_target_value(&ev))
                            required=true
                        />
                        <input
                            type="text"
                            list="kategori-kelas"
                            class="w-full bg-surface-container border-0 rounded-xl px-4 py-3 text-body-md text-on-surface"
                            placeholder="Kategori (mis. Lambatan) — ketik baru bila belum ada"
                            prop:value=move || category.get()
                            on:input=move |ev| category.set(event_target_value(&ev))
                        />
                        <textarea
                            rows="2"
                            class="w-full bg-surface-container border-0 rounded-xl px-4 py-3 text-body-md text-on-surface resize-none"
                            placeholder="Deskripsi singkat (opsional)"
                            prop:value=move || desc.get()
                            on:input=move |ev| desc.set(event_target_value(&ev))
                        ></textarea>
                        <div class="grid grid-cols-2 gap-3">
                            <button
                                type="button"
                                class="py-3 rounded-xl border border-outline-variant text-on-surface font-semibold text-body-sm"
                                on:click=move |_| show_form.set(false)
                            >
                                "Batal"
                            </button>
                            <button
                                type="submit"
                                class="py-3 rounded-xl bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                                disabled=move || busy.get()
                            >
                                {move || if busy.get() { "Menyimpan…" } else { "Simpan Kelas" }}
                            </button>
                        </div>
                    </form>
                }
                    .into_any()
            } else {
                view! {
                    <button
                        class="w-full border-2 border-dashed border-outline-variant rounded-2xl p-6 flex flex-col items-center gap-2 text-on-surface-variant hover:border-primary hover:text-primary transition-colors press"
                        on:click=move |_| show_form.set(true)
                    >
                        <span class="w-12 h-12 rounded-full bg-surface-container flex items-center justify-center">
                            <span class="material-symbols-outlined text-2xl">"add"</span>
                        </span>
                        <span class="text-body-md font-bold">"Tambah Kelas Baru"</span>
                        <span class="text-body-sm">"Mulai kurikulum baru hari ini"</span>
                    </button>
                }
                    .into_any()
            }
        }}
    }
}

#[component]
fn KelasCard(k: KelasItem) -> impl IntoView {
    let href = format!("/kelas/{}", k.id);
    view! {
        <div
            class="ppm-card p-4 card-hover anim-in"
            style="border-left:4px solid #064e3b"
        >
            {(!k.category.is_empty())
                .then(|| {
                    view! {
                        <span class="inline-block px-2.5 py-1 rounded-full bg-secondary-container text-primary text-[10px] font-bold tracking-wider uppercase mb-1.5">
                            {k.category.clone()}
                        </span>
                    }
                })}
            <h3 class="text-body-lg font-bold text-on-background">{k.name}</h3>
            <p class="text-body-sm text-on-surface-variant flex items-center gap-1 mt-1">
                <span class="material-symbols-outlined text-[15px]">"person"</span>
                {k.teacher}
            </p>
            <div class="flex items-center gap-4 mt-3 text-body-sm text-on-surface-variant">
                <span class="flex items-center gap-1">
                    <span class="material-symbols-outlined text-[16px] text-primary">"groups"</span>
                    <b class="text-on-background">{k.member_count}</b>
                    " Santri"
                </span>
                <span class="flex items-center gap-1">
                    <span class="material-symbols-outlined text-[16px] text-primary">"event"</span>
                    <b class="text-on-background">{k.schedule_count}</b>
                    " Jadwal"
                </span>
            </div>
            <a
                href=href
                class="mt-3 w-full py-2.5 rounded-xl bg-secondary-container text-primary font-semibold text-body-sm flex items-center justify-center gap-2 press"
            >
                <span class="material-symbols-outlined text-[18px]">"visibility"</span>
                "Lihat Santri & Kelola"
            </a>
        </div>
    }
}
