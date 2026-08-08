//! web/pages/artikel.rs — Artikel PUBLIK (migrasi 69): daftar `/artikel` dan
//! satu tulisan `/artikel/<slug>`.
//!
//! Halaman ini memakai bilah navigasi & kaki halaman yang SAMA dengan beranda
//! publik (`PublicNav`/`PublicFooter`) — pengunjung yang mengklik "Artikel"
//! tidak boleh terlempar ke halaman yang kehilangan menunya.
//!
//! Isi artikel dirender sebagai TEKS BIASA yang dipecah per paragraf, bukan
//! HTML. Yang diketik pengelola berakhir di halaman publik apa adanya; kalau
//! disisipkan sebagai HTML, satu tempelan dari sumber luar cukup untuk
//! menjalankan skrip di peramban setiap pengunjung.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use crate::web::api::{article_data, articles_data};
use crate::web::pages::beranda::{ArtikelCard, PublicFooter, PublicNav};

#[component]
pub fn ArtikelListPage() -> impl IntoView {
    // 0 = tanpa batas: halaman ini memang daftar lengkapnya.
    let data = Resource::new(|| (), |_| async move { articles_data(0).await });

    view! {
        <Title text="Artikel — PPM Al-Faqih Mandiri" />
        <div class="min-h-screen bg-surface text-on-surface">
            <PublicNav />

            <header class="spiritual-gradient text-on-primary">
                <div class="max-w-6xl mx-auto px-5 py-16 md:py-20 text-center">
                    <p class="text-label-md uppercase tracking-[0.3em] text-primary-fixed">
                        "Artikel AFM"
                    </p>
                    <h1 class="text-display-lg font-bold mt-4">"Kabar & Tulisan"</h1>
                    <p class="text-body-lg opacity-85 max-w-2xl mx-auto mt-5">
                        "Catatan kegiatan, kajian, dan kabar terbaru dari Pondok Pesantren Mahasiswa Al-Faqih Mandiri."
                    </p>
                </div>
            </header>

            <section class="max-w-6xl mx-auto px-5 py-16 md:py-20">
                <Suspense fallback=|| {
                    view! {
                        <div class="grid sm:grid-cols-2 lg:grid-cols-3 gap-5 animate-pulse">
                            <div class="h-64 bg-surface-container rounded-2xl"></div>
                            <div class="h-64 bg-surface-container rounded-2xl"></div>
                            <div class="h-64 bg-surface-container rounded-2xl"></div>
                        </div>
                    }
                }>
                    {move || {
                        let list = data.get().and_then(|r| r.ok()).unwrap_or_default();
                        if list.is_empty() {
                            view! {
                                <div class="ppm-card p-12 text-center max-w-lg mx-auto">
                                    <span class="material-symbols-outlined text-5xl text-on-surface-variant/60">
                                        "article"
                                    </span>
                                    <p class="text-body-lg font-semibold text-on-background mt-3">
                                        "Belum ada artikel"
                                    </p>
                                    <p class="text-body-sm text-on-surface-variant mt-1">
                                        "Tulisan yang diterbitkan pengurus akan tampil di sini."
                                    </p>
                                </div>
                            }
                                .into_any()
                        } else {
                            view! {
                                <div class="grid sm:grid-cols-2 lg:grid-cols-3 gap-5 stagger">
                                    {list
                                        .into_iter()
                                        .map(|a| view! { <ArtikelCard a=a /> })
                                        .collect_view()}
                                </div>
                            }
                                .into_any()
                        }
                    }}
                </Suspense>
            </section>

            <PublicFooter />
        </div>
    }
}

#[component]
pub fn ArtikelDetailPage() -> impl IntoView {
    let params = use_params_map();
    // Slug ikut jadi kunci resource: berpindah dari satu artikel ke artikel
    // lain adalah navigasi SPA yang tak me-mount ulang komponen ini, jadi tanpa
    // kunci yang berubah pembaca akan melihat tulisan yang sebelumnya.
    let data = Resource::new(
        move || params.read().get("slug").unwrap_or_default(),
        |slug| async move { article_data(slug).await },
    );

    view! {
        <div class="min-h-screen bg-surface text-on-surface">
            <PublicNav />

            <Suspense fallback=|| {
                view! {
                    <div class="max-w-3xl mx-auto px-5 py-16 animate-pulse space-y-4">
                        <div class="h-10 w-3/4 bg-surface-container rounded-xl"></div>
                        <div class="h-64 bg-surface-container rounded-2xl"></div>
                        <div class="h-4 bg-surface-container rounded"></div>
                        <div class="h-4 bg-surface-container rounded"></div>
                    </div>
                }
            }>
                {move || {
                    match data.get().and_then(|r| r.ok()).flatten() {
                        None => {
                            view! {
                                <Title text="Artikel Tidak Ditemukan — PPM AFM" />
                                <div class="max-w-lg mx-auto px-5 py-24 text-center">
                                    <span class="material-symbols-outlined text-5xl text-on-surface-variant/60">
                                        "search_off"
                                    </span>
                                    <h1 class="text-headline-sm text-on-background mt-3">
                                        "Artikel tidak ditemukan"
                                    </h1>
                                    <p class="text-body-md text-on-surface-variant mt-2">
                                        "Tulisan ini mungkin sudah dihapus atau belum diterbitkan."
                                    </p>
                                    <a
                                        href="/artikel"
                                        class="inline-block mt-6 px-7 py-3.5 bg-primary text-on-primary rounded-xl font-semibold hover:bg-primary-container transition-colors"
                                    >
                                        "Lihat Semua Artikel"
                                    </a>
                                </div>
                            }
                                .into_any()
                        }
                        Some(a) => {
                            view! {
                                <Title text=format!("{} — PPM AFM", a.title) />
                                <article class="max-w-3xl mx-auto px-5 py-12 md:py-16">
                                    <a
                                        href="/artikel"
                                        class="inline-flex items-center gap-1.5 text-body-sm font-semibold text-on-surface-variant hover:text-primary transition-colors"
                                    >
                                        <span class="material-symbols-outlined text-[18px]">
                                            "arrow_back"
                                        </span>
                                        "Semua Artikel"
                                    </a>
                                    <p class="text-[11px] text-on-surface-variant uppercase tracking-widest mt-6">
                                        {a.created_at.clone()}
                                    </p>
                                    <h1 class="text-display-md md:text-display-lg font-bold text-on-background mt-2">
                                        {a.title.clone()}
                                    </h1>
                                    {(!a.excerpt.trim().is_empty())
                                        .then(|| {
                                            view! {
                                                <p class="text-body-lg text-on-surface-variant mt-4 leading-relaxed">
                                                    {a.excerpt.clone()}
                                                </p>
                                            }
                                        })}
                                    {a
                                        .cover_url
                                        .clone()
                                        .map(|url| {
                                            view! {
                                                <img
                                                    src=url
                                                    alt=a.title.clone()
                                                    class="w-full rounded-2xl mt-8 bg-surface-container"
                                                />
                                            }
                                        })}
                                    <div class="mt-8 space-y-4">
                                        {a
                                            .body
                                            .split("\n\n")
                                            .map(str::trim)
                                            .filter(|p| !p.is_empty())
                                            .map(|p| {
                                                view! {
                                                    <p class="text-body-md text-on-surface leading-relaxed whitespace-pre-line">
                                                        {p.to_string()}
                                                    </p>
                                                }
                                            })
                                            .collect_view()}
                                    </div>
                                </article>
                            }
                                .into_any()
                        }
                    }
                }}
            </Suspense>

            <PublicFooter />
        </div>
    }
}
