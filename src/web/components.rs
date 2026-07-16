//! web/components.rs — Komponen bersama.

use leptos::prelude::*;

/// Header mobile sticky: judul + lonceng notifikasi interaktif (popover).
#[component]
pub fn MobileHeader(
    title: &'static str,
    #[prop(optional)] back_href: Option<&'static str>,
    #[prop(optional)] subtitle: Option<&'static str>,
) -> impl IntoView {
    view! {
        <header class="sticky top-0 z-20 bg-surface/90 backdrop-blur border-b border-outline-variant/50 px-5 py-4 flex items-center gap-3">
            {back_href
                .map(|href| {
                    view! {
                        <a
                            href=href
                            class="w-9 h-9 -ml-1 rounded-full flex items-center justify-center text-on-surface hover:bg-surface-container"
                        >
                            <span class="material-symbols-outlined">"arrow_back"</span>
                        </a>
                    }
                })}
            <div class="flex-1 min-w-0">
                <h1 class="text-headline-sm text-on-background truncate">{title}</h1>
                {subtitle
                    .map(|s| {
                        view! { <p class="text-body-sm text-on-surface-variant truncate">{s}</p> }
                    })}
            </div>
            <NotifBell />
        </header>
    }
}

/// Lonceng notifikasi: klik → popover kecil. (Feed notifikasi asli menyusul;
/// popover jujur menampilkan "belum ada notifikasi".)
#[component]
pub fn NotifBell() -> impl IntoView {
    let open = RwSignal::new(false);
    view! {
        <div class="relative">
            <button
                class="w-10 h-10 rounded-full flex items-center justify-center text-on-surface-variant hover:bg-surface-container relative"
                on:click=move |_| open.update(|o| *o = !*o)
                aria-label="Notifikasi"
            >
                <span class="material-symbols-outlined">"notifications"</span>
                <span class="absolute top-2 right-2 w-2 h-2 rounded-full bg-error pulse-dot"></span>
            </button>
            {move || {
                open.get()
                    .then(|| {
                        view! {
                            // Backdrop transparan: klik di luar menutup popover.
                            <div class="fixed inset-0 z-30" on:click=move |_| open.set(false)></div>
                            <div class="absolute right-0 top-12 z-40 w-72 bg-surface-container-lowest border border-outline-variant/60 rounded-2xl shadow-xl p-4 anim-in">
                                <div class="flex items-center justify-between mb-2">
                                    <p class="text-body-md font-bold text-on-background">"Notifikasi"</p>
                                    <button
                                        class="w-7 h-7 rounded-full flex items-center justify-center text-on-surface-variant hover:bg-surface-container"
                                        on:click=move |_| open.set(false)
                                    >
                                        <span class="material-symbols-outlined text-lg">"close"</span>
                                    </button>
                                </div>
                                <div class="py-6 text-center text-on-surface-variant">
                                    <span class="material-symbols-outlined text-4xl opacity-60">
                                        "notifications_off"
                                    </span>
                                    <p class="text-body-sm mt-2">"Belum ada notifikasi baru."</p>
                                </div>
                            </div>
                        }
                    })
            }}
        </div>
    }
}

/// Item navigasi bawah.
#[derive(Clone, Copy)]
pub struct NavDef {
    pub icon: &'static str,
    pub label: &'static str,
    pub href: &'static str,
}

/// Navigasi bawah mobile (fixed di dalam DeviceFrame).
#[component]
pub fn MobileNav(items: &'static [NavDef], active: &'static str) -> impl IntoView {
    let cols = match items.len() {
        3 => "grid grid-cols-3",
        5 => "grid grid-cols-5",
        _ => "grid grid-cols-4",
    };
    view! {
        <nav class="fixed bottom-0 inset-x-0 max-w-md mx-auto bg-surface-container-lowest border-t border-outline-variant/60 z-20">
            <div class=cols>
                {items
                    .iter()
                    .map(|it| {
                        let is_active = it.href == active;
                        let cls = if is_active {
                            "flex flex-col items-center gap-0.5 py-2.5 text-primary"
                        } else {
                            "flex flex-col items-center gap-0.5 py-2.5 text-on-surface-variant"
                        };
                        view! {
                            <a href=it.href class=cls>
                                <span class="material-symbols-outlined">{it.icon}</span>
                                <span class="text-[11px] font-medium">{it.label}</span>
                            </a>
                        }
                    })
                    .collect_view()}
            </div>
        </nav>
    }
}

/// Nav peran: santri.
pub const NAV_SANTRI: &[NavDef] = &[
    NavDef { icon: "space_dashboard", label: "Beranda", href: "/santri" },
    NavDef { icon: "history", label: "Riwayat", href: "/riwayat" },
    NavDef { icon: "groups", label: "Sesi", href: "/sesi" },
    NavDef { icon: "event_available", label: "Izin", href: "/izin" },
    NavDef { icon: "person", label: "Profil", href: "/profil" },
];

/// Nav peran: pamong/dewan guru/admin (halaman dinamis).
pub const NAV_PAMONG: &[NavDef] = &[
    NavDef { icon: "how_to_reg", label: "Verifikasi", href: "/verifikasi-pamong" },
    NavDef { icon: "groups", label: "Sesi", href: "/sesi" },
    NavDef { icon: "person", label: "Profil", href: "/profil" },
];

/// Tampilan error fetch yang JUJUR: error autentikasi → ajak login; error lain
/// (mis. DB/migrasi) → tampilkan pesannya + tombol Coba Lagi. Mencegah pesan
/// menyesatkan "Sesi berakhir" untuk error non-auth.
#[component]
pub fn FetchError(err: String) -> impl IntoView {
    let is_auth = err.contains("unauth") || err.contains("forbidden");
    if is_auth {
        view! {
            <div class="pt-10 text-center space-y-4 anim-in">
                <p class="text-body-md text-on-surface-variant">
                    "Sesi berakhir. Silakan masuk kembali."
                </p>
                <a
                    href="/login"
                    class="inline-block px-6 py-3 bg-primary text-on-primary rounded-xl font-semibold"
                >
                    "Ke Halaman Login"
                </a>
            </div>
        }
            .into_any()
    } else {
        let detail: String = err.chars().take(200).collect();
        view! {
            <div class="pt-8 text-center space-y-4 anim-in">
                <span class="material-symbols-outlined text-5xl text-error">"error"</span>
                <p class="text-body-md font-semibold text-on-background">"Gagal memuat data"</p>
                <p class="text-body-sm text-on-surface-variant break-words px-4">{detail}</p>
                <button
                    class="px-6 py-3 bg-primary text-on-primary rounded-xl font-semibold"
                    on:click=move |_| {
                        #[cfg(target_arch = "wasm32")]
                        if let Some(w) = web_sys::window() {
                            let _ = w.location().reload();
                        }
                    }
                >
                    "Coba Lagi"
                </button>
            </div>
        }
            .into_any()
    }
}

/// Nav peran: staf/admin.
pub const NAV_STAF: &[NavDef] = &[
    NavDef { icon: "dashboard", label: "Beranda", href: "/staf" },
    NavDef { icon: "groups", label: "Santri", href: "/poin" },
    NavDef { icon: "school", label: "Kelas", href: "/halaqah" },
    NavDef { icon: "monitoring", label: "Laporan", href: "/guru" },
];

/// Nav peran: dewan guru.
pub const NAV_GURU: &[NavDef] = &[
    NavDef { icon: "home", label: "Home", href: "/guru" },
    NavDef { icon: "stars", label: "Poin", href: "/poin-dewan" },
    NavDef { icon: "verified_user", label: "Verifikasi", href: "/verifikasi-tahap-2" },
    NavDef { icon: "insights", label: "Statistik", href: "/dewan-guru" },
];

/// Nav peran: orang tua.
pub const NAV_ORTU: &[NavDef] = &[
    NavDef { icon: "home", label: "Beranda", href: "/orang-tua" },
    NavDef { icon: "history", label: "Riwayat", href: "/orang-tua/riwayat" },
    NavDef { icon: "event_available", label: "Izin", href: "/orang-tua/izin" },
    NavDef { icon: "person", label: "Profil", href: "/profil" },
];

/// Bingkai halaman berorientasi MOBILE.
///
/// Desktop = SAMA dengan mobile (permintaan user): kolom ponsel terpusat dengan
/// scroll internal — bottom-nav & FAB menempel di kolom, bukan tepi layar.
/// CSS `.ppm-stage` di `web/app.rs`. (Login TIDAK memakai frame ini — layout
/// dua kolomnya dipertahankan.)
#[component]
pub fn DeviceFrame(children: Children) -> impl IntoView {
    view! { <div class="ppm-stage">{children()}</div> }
}
