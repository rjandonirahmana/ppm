//! web/pages/menu.rs — Menu dev: daftar semua halaman untuk pratinjau cepat.
//! (Sementara — nanti navigasi asli lewat login + peran.)

use leptos::prelude::*;
use leptos_meta::Title;

const LINKS: &[(&str, &str, &str)] = &[
    ("/", "Beranda Publik", "public"),
    ("/login", "Login Portal", "login"),
    ("/santri", "Dashboard Santri", "dashboard"),
    ("/izin", "Ajukan Perizinan", "event_available"),
    ("/riwayat", "Riwayat Kehadiran", "history"),
    ("/sesi", "Sesi Kelas", "groups"),
    ("/students", "Students (santri+verifikasi)", "groups"),
    ("/kelas", "Manajemen Kelas", "school"),
    ("/profil", "Profil Pengguna", "person"),
    ("/staf", "Dashboard Staf", "badge"),
    ("/galeri", "Galeri Media (video & foto)", "grid_on"),
    ("/artikel", "Artikel Publik", "article"),
    ("/kelola-artikel", "Kelola Artikel (admin)", "edit_note"),
    ("/tagihan", "Tagihan (Finance)", "card_giftcard"),
    ("/tagihan-saya", "Tagihan Saya (Santri)", "fact_check"),
    ("/guru", "Analisis Guru", "analytics"),
    ("/dewan-guru", "Analisis Dewan Guru", "insights"),
    ("/poin", "Pantauan Poin Santri", "stars"),
    ("/poin-saya", "Riwayat Poin Saya (Santri)", "history"),
    ("/verifikasi-tahap-2", "Verifikasi Tahap 2", "verified_user"),
    ("/halaqah", "Daftar Halaqah", "groups"),
    ("/halaqah/mulai", "Mulai Sesi Halaqah", "play_circle"),
    ("/halaqah/live", "Sesi Halaqah Live", "graphic_eq"),
    ("/rekaman", "Rekaman Materi", "video_library"),
    ("/orang-tua", "Pantauan Orang Tua", "family_restroom"),
    ("/orang-tua/izin", "Izin Anak (Ortu)", "event_available"),
    ("/orang-tua/riwayat", "Riwayat Anak (Ortu)", "history"),
    ("/koneksi-ortu", "Koneksi Orang Tua", "link"),
];

#[component]
pub fn MenuPage() -> impl IntoView {
    view! {
        <Title text="Menu Halaman — AFM SMART" />
        <div class="min-h-screen bg-surface p-6">
            <div class="max-w-3xl mx-auto">
                <header class="flex items-center gap-3 mb-8">
                    <div class="w-12 h-12 spiritual-gradient rounded-xl flex items-center justify-center">
                        <span class="material-symbols-outlined text-on-primary text-3xl">"mosque"</span>
                    </div>
                    <div class="flex-1">
                        <h1 class="text-display-md text-primary">"AFM SMART"</h1>
                        <p class="text-body-sm text-on-surface-variant">"Pratinjau semua halaman (dev)"</p>
                    </div>
                    <button
                        class="flex items-center gap-2 px-4 py-2.5 border border-outline-variant rounded-xl text-body-sm text-on-surface hover:border-error hover:text-error transition-colors"
                        on:click=move |_| {
                            leptos::task::spawn_local(async move {
                                let _ = crate::web::api::logout_action().await;
                                crate::web::components::ke_login();
                            });
                        }
                    >
                        <span class="material-symbols-outlined text-xl">"logout"</span>
                        "Keluar"
                    </button>
                </header>
                <div class="grid sm:grid-cols-2 gap-3 stagger">
                    {LINKS
                        .iter()
                        .map(|(href, label, icon)| {
                            view! {
                                <a
                                    href=*href
                                    class="flex items-center gap-3 p-4 bg-surface-container-lowest border border-outline-variant/60 rounded-xl hover:border-primary card-hover press"
                                >
                                    <div class="w-10 h-10 rounded-lg bg-secondary-container flex items-center justify-center text-primary">
                                        <span class="material-symbols-outlined">{*icon}</span>
                                    </div>
                                    <div class="flex-1">
                                        <p class="text-body-md font-semibold text-on-background">{*label}</p>
                                        <p class="text-body-sm text-on-surface-variant">{*href}</p>
                                    </div>
                                    <span class="material-symbols-outlined text-outline">"chevron_right"</span>
                                </a>
                            }
                        })
                        .collect_view()}
                </div>
            </div>
        </div>
    }
}
