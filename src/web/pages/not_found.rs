//! web/pages/not_found.rs — Halaman 404.

use leptos::prelude::*;
use leptos_meta::Title;

#[component]
pub fn NotFoundPage() -> impl IntoView {
    view! {
        <Title text="Tidak Ditemukan — AFM SMART" />
        <div class="min-h-screen flex flex-col items-center justify-center text-center p-6 bg-surface">
            <div class="w-20 h-20 spiritual-gradient rounded-2xl flex items-center justify-center mb-6">
                <span class="material-symbols-outlined text-on-primary text-5xl">"mosque"</span>
            </div>
            <h1 class="text-display-md text-on-background">"Halaman tidak ditemukan"</h1>
            <p class="text-body-md text-on-surface-variant mt-2 max-w-sm">
                "Maaf, halaman yang Anda cari tidak tersedia."
            </p>
            <a href="/" class="mt-8 px-6 py-3 bg-primary text-on-primary rounded-xl font-semibold hover:bg-primary-container transition-colors">
                "Kembali ke Portal"
            </a>
        </div>
    }
}
