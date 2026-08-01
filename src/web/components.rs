//! web/components.rs — Komponen bersama.

use leptos::prelude::*;
use leptos_router::hooks::use_location;

use crate::models::SessionUser;

/// Header: mobile = sticky bar (judul + lonceng + setting). Desktop (md+, ala
/// TOPBAR mockup Admin Portal) = judul lebih besar non-sticky + **identitas
/// user** (avatar inisial + nama + peran) menggantikan tombol setting — sidebar
/// desktop sudah punya Settings/Logout sendiri, jadi header di sana cukup jadi
/// heading halaman + identitas, bukan duplikasi kontrol.
#[component]
pub fn MobileHeader(
    title: &'static str,
    #[prop(optional)] back_href: Option<&'static str>,
    #[prop(optional)] subtitle: Option<&'static str>,
) -> impl IntoView {
    let session = use_context::<Resource<Option<SessionUser>>>();
    view! {
        // z-40 (bukan z-20): `backdrop-blur` bikin stacking-context baru, jadi
        // popover NotifBell (z-40 internal) ikut "terkurung" di level header.
        // Header harus di atas bottom-nav (z-20), FAB (z-20), sidebar (z-30) agar
        // popover notif tak tertutup ikon/elemen lain. Sheet modal (z-50) tetap menang.
        <header class="sticky top-0 md:relative z-40 bg-surface/90 md:bg-transparent backdrop-blur md:backdrop-blur-none border-b border-outline-variant/50 md:border-0 px-5 md:px-0 py-4 md:pt-2 md:pb-5 flex items-center gap-3">
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
                <h1 class="text-headline-sm md:text-display-md text-on-background truncate">{title}</h1>
                {subtitle
                    .map(|s| {
                        view! { <p class="text-body-sm text-on-surface-variant truncate">{s}</p> }
                    })}
            </div>
            <span class="md:hidden">
                <NotifBell />
            </span>
            <a
                href="/profil"
                class="md:hidden w-9 h-9 rounded-full flex items-center justify-center text-on-surface hover:bg-surface-container press"
                aria-label="Pengaturan"
            >
                <span class="material-symbols-outlined">"settings"</span>
            </a>
            // ── Identitas user (desktop saja) ────────────────────────────
            <Transition fallback=|| ()>
                {move || {
                    let user = session.and_then(|s| s.get()).flatten();
                    user.map(|u| {
                        let initial: String = u
                            .name
                            .split_whitespace()
                            .take(2)
                            .filter_map(|w| w.chars().next())
                            .collect();
                        let role_label = match u.role.as_str() {
                            "admin" => "Administrator",
                            "ketua" => "Ketua",
                            "supervisor" => "Pamong",
                            "teacher" | "dewan_guru" => "Dewan Guru",
                            "parent" => "Orang Tua",
                            _ => "Santri",
                        };
                        view! {
                            <div class="hidden md:flex items-center gap-3 pl-4 ml-1 border-l border-outline-variant/50 shrink-0">
                                <NotifBell />
                                <div class="text-right leading-tight">
                                    <p class="text-body-sm font-bold text-on-background">{u.name}</p>
                                    <p class="text-[11px] text-on-surface-variant">{role_label}</p>
                                </div>
                                <div class="w-10 h-10 rounded-full bg-secondary-container text-primary flex items-center justify-center text-body-sm font-bold shrink-0">
                                    {initial.to_uppercase()}
                                </div>
                            </div>
                        }
                    })
                }}
            </Transition>
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
                            // z tinggi (55/60) supaya popover PASTI di atas ikon/
                            // elemen lain di header (header sendiri sudah z-40).
                            <div class="fixed inset-0 z-[55]" on:click=move |_| open.set(false)></div>
                            <div class="absolute right-0 top-12 z-[60] w-72 ppm-card shadow-xl p-4 anim-in">
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

/// Path (prefix) yang MENAMPILKAN bottom-nav. Selain ini (/, /login, /menu,
/// /halaqah*, /rekaman, /koneksi-ortu) tak ada nav.
fn nav_visible(path: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "/santri", "/izin", "/riwayat", "/sesi", "/profil", "/ganti-sandi", "/laporan", "/staf", "/guru",
        "/dewan-guru", "/poin", "/poin-dewan", "/verifikasi-pamong",
        "/verifikasi-tahap-2", "/students", "/kelas", "/orang-tua", "/kontrol-pengguna",
        "/akademik", "/kalender", "/izin-staf", "/materi", "/rekap-mingguan", "/setelan",
        "/galeri", "/tagihan", "/tagihan-saya",
    ];
    PREFIXES
        .iter()
        .any(|p| path == *p || path.starts_with(&format!("{p}/")))
}

/// Item aktif = href sama atau path adalah sub-rute-nya (mis. /kelas/5 → /kelas).
fn item_active(path: &str, href: &str) -> bool {
    path == href || (href != "/" && path.starts_with(&format!("{href}/")))
}

/// Navbar bawah PERSISTEN — dirender SEKALI di `App` (di luar `<FlatRoutes>`),
/// jadi TIDAK ikut ter-swap/shimmer saat pindah halaman. Item dari role (context
/// sesi global), halaman aktif dari URL. Struktur dibangun sekali per-role; saat
/// navigasi hanya atribut `class`/`style` yang re-eval (highlight bergeser),
/// nav TIDAK dibangun ulang → tanpa kedip.
#[component]
pub fn BottomNav() -> impl IntoView {
    let pathname = use_location().pathname;
    let session = use_context::<Resource<Option<SessionUser>>>();
    // <Transition> WAJIB: membaca Resource sesi harus di dalam Suspense/Transition
    // (kalau tidak → hydration-mismatch). Transition (bukan Suspense) menjaga nav
    // tetap tampil, tak blank, saat resource resolve.
    view! {
        <Transition fallback=|| ()>
        {move || {
            // Hanya melacak `session` (bukan pathname) → tak rebuild saat navigasi.
            let role = session
                .and_then(|s| s.get())
                .flatten()
                .map(|u| u.role)
                .unwrap_or_default();
            let items = nav_for(&role);
            let cols = match items.len() {
                3 => "grid grid-cols-3",
                5 => "grid grid-cols-5",
                6 => "grid grid-cols-6",
                _ => "grid grid-cols-4",
            };
            view! {
                // md:hidden — di desktop digantikan <DesktopSidebar/> (kondisi
                // tampil keduanya identik: role ada + nav_visible).
                <nav
                    class="md:hidden fixed bottom-0 inset-x-0 max-w-md mx-auto bg-surface-container-lowest border-t border-outline-variant/60 z-20"
                    style=move || if nav_visible(&pathname.get()) { "" } else { "display:none" }
                >
                    <div class=cols>
                        {items
                            .iter()
                            .map(|it| {
                                let href = it.href;
                                view! {
                                    // Item aktif = PILL SAGE di ikon (bahasa
                                    // desain ppm-design-new).
                                    <a
                                        href=href
                                        class=move || {
                                            if item_active(&pathname.get(), href) {
                                                "flex flex-col items-center gap-0.5 py-2 text-primary"
                                            } else {
                                                "flex flex-col items-center gap-0.5 py-2 text-on-surface-variant"
                                            }
                                        }
                                    >
                                        <span class=move || {
                                            if item_active(&pathname.get(), href) {
                                                "material-symbols-outlined px-4 py-0.5 rounded-full bg-secondary-container"
                                            } else {
                                                "material-symbols-outlined px-4 py-0.5 rounded-full"
                                            }
                                        }>{it.icon}</span>
                                        <span class=move || {
                                            if item_active(&pathname.get(), href) {
                                                "text-[11px] font-bold"
                                            } else {
                                                "text-[11px] font-medium"
                                            }
                                        }>{it.label}</span>
                                    </a>
                                }
                            })
                            .collect_view()}
                    </div>
                </nav>
            }
        }}
        </Transition>
    }
}

/// Sidebar DESKTOP (md+) — bahasa desain ppm-design-new "Admin Portal": panel
/// emerald pekat kiri (logo → nav → identitas + Pengaturan + Keluar), item aktif
/// = pill sage. Dirender persisten di `App` seperti BottomNav (context sesi +
/// URL aktif); di desktop BottomNav disembunyikan, sidebar menggantikannya.
/// CSS pendamping (margin konten, fixed offset) di style/tailwind.css via
/// `body:has(.ppm-sidebar[data-open="1"])`.
#[component]
pub fn DesktopSidebar() -> impl IntoView {
    let pathname = use_location().pathname;
    let session = use_context::<Resource<Option<SessionUser>>>();
    let logout = move |_| {
        #[cfg(target_arch = "wasm32")]
        leptos::task::spawn_local(async move {
            let _ = crate::web::api::logout_action().await;
            if let Some(w) = web_sys::window() {
                let _ = w.location().replace("/login");
            }
        });
    };
    view! {
        <Transition fallback=|| ()>
        {move || {
            let user = session.and_then(|s| s.get()).flatten();
            let (role, name) = user.map(|u| (u.role, u.name)).unwrap_or_default();
            let has_role = !role.is_empty();
            let role_label = match role.as_str() {
                "admin" => "Administrator",
                "ketua" => "Ketua",
                "supervisor" => "Pamong",
                "teacher" | "dewan_guru" => "Dewan Guru",
                "parent" => "Orang Tua",
                "santri" | "santri_finance" => "Santri",
                _ => "",
            };
            let initial: String =
                name.split_whitespace().take(2).filter_map(|w| w.chars().next()).collect();
            let items = nav_for(&role);
            view! {
                <aside
                    class="ppm-sidebar hidden md:flex fixed inset-y-0 left-0 w-64 z-30 flex-col bg-primary text-on-primary"
                    data-open=move || {
                        if has_role && nav_visible(&pathname.get()) { "1" } else { "0" }
                    }
                >
                    // ── Brand ────────────────────────────────────────────
                    <div class="flex items-center gap-3 px-5 pt-6 pb-5">
                        <div class="w-10 h-10 rounded-xl bg-white/10 flex items-center justify-center">
                            <span class="material-symbols-outlined">"mosque"</span>
                        </div>
                        <div class="leading-tight min-w-0">
                            <p class="font-bold text-body-lg">"PPM AFM"</p>
                            <p class="text-[10px] uppercase tracking-[0.18em] opacity-70">
                                "Portal Absensi"
                            </p>
                        </div>
                    </div>
                    // ── Navigasi (item sama dgn navbar bawah) ────────────
                    <nav class="flex-1 px-3 space-y-1 overflow-y-auto">
                        {items
                            .iter()
                            .map(|it| {
                                let href = it.href;
                                view! {
                                    <a
                                        href=href
                                        class=move || {
                                            if item_active(&pathname.get(), href) {
                                                "flex items-center gap-3 px-4 py-3 rounded-xl bg-secondary-container text-primary text-body-sm font-bold press"
                                            } else {
                                                "flex items-center gap-3 px-4 py-3 rounded-xl text-on-primary/75 hover:bg-white/10 text-body-sm font-semibold press"
                                            }
                                        }
                                    >
                                        <span class="material-symbols-outlined text-[20px]">
                                            {it.icon}
                                        </span>
                                        {it.label}
                                    </a>
                                }
                            })
                            .collect_view()}
                    </nav>
                    // ── Identitas + Pengaturan + Keluar ──────────────────
                    <div class="px-3 pb-5 pt-3 border-t border-white/10 space-y-1">
                        <div class="flex items-center gap-3 px-4 py-2.5">
                            <div class="w-9 h-9 rounded-full bg-white/15 flex items-center justify-center text-body-sm font-bold shrink-0">
                                {initial.to_uppercase()}
                            </div>
                            <div class="min-w-0 leading-tight">
                                <p class="text-body-sm font-bold truncate">{name.clone()}</p>
                                <p class="text-[11px] opacity-70">{role_label}</p>
                            </div>
                        </div>
                        <a
                            href="/profil"
                            class="flex items-center gap-3 px-4 py-2.5 rounded-xl text-on-primary/75 hover:bg-white/10 text-body-sm font-semibold press"
                        >
                            <span class="material-symbols-outlined text-[20px]">"settings"</span>
                            "Pengaturan"
                        </a>
                        <button
                            class="w-full flex items-center gap-3 px-4 py-2.5 rounded-xl text-on-primary/75 hover:bg-white/10 text-body-sm font-semibold press"
                            on:click=logout
                        >
                            <span class="material-symbols-outlined text-[20px]">"logout"</span>
                            "Keluar"
                        </button>
                    </div>
                </aside>
            }
        }}
        </Transition>
    }
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

/// Nav peran: santri. Item "Laporan" DIGANTI "Akademik" (self-report progres
/// buku/hafalan sendiri, /akademik) — rapor pribadi pindah jadi bagian dari
/// /riwayat (lihat pages/riwayat.rs).
pub const NAV_SANTRI: &[NavDef] = &[
    NavDef { icon: "space_dashboard", label: "Beranda", href: "/santri" },
    NavDef { icon: "calendar_month", label: "Kalender", href: "/kalender" },
    NavDef { icon: "history", label: "Riwayat", href: "/riwayat" },
    NavDef { icon: "groups", label: "Sesi", href: "/sesi" },
    NavDef { icon: "event_available", label: "Izin", href: "/izin" },
    NavDef { icon: "auto_stories", label: "Akademik", href: "/akademik" },
];

/// Nav peran: santri_finance = santri PENUH + akses halaman finance (cek & tandai
/// pembayaran SEMUA santri di /tagihan). Beranda tetap /santri (dia tetap santri).
pub const NAV_SANTRI_FINANCE: &[NavDef] = &[
    NavDef { icon: "space_dashboard", label: "Beranda", href: "/santri" },
    NavDef { icon: "calendar_month", label: "Kalender", href: "/kalender" },
    NavDef { icon: "payment", label: "Pembayaran", href: "/tagihan" },
    NavDef { icon: "history", label: "Riwayat", href: "/riwayat" },
    NavDef { icon: "groups", label: "Sesi", href: "/sesi" },
];

// ── Navbar STAF SERAGAM ──────────────────────────────────────────────────────
// admin / pamong / guru / dewan-guru memakai item YANG SAMA (Beranda · Students ·
// Kelas · Laporan · User Control) supaya navbar tak "berubah-ubah" antar
// halaman. Yang beda HANYA tujuan "Beranda" (dashboard tiap peran, dari
// models::role_home). Kelas & Sesi DIGABUNG jadi satu halaman/nav (/kelas
// dgn tab Kelas/Sesi) — dulu 2 item terpisah; santri/ortu tetap via /sesi
// standalone (nav mereka tak berubah).

/// Nav peran: pamong (supervisor). Beranda → /verifikasi-pamong.
pub const NAV_PAMONG: &[NavDef] = &[
    NavDef { icon: "dashboard", label: "Beranda", href: "/verifikasi-pamong" },
    NavDef { icon: "calendar_month", label: "Kalender", href: "/kalender" },
    NavDef { icon: "groups", label: "Students", href: "/students" },
    NavDef { icon: "school", label: "Kelas", href: "/kelas" },
    NavDef { icon: "bar_chart", label: "Laporan", href: "/laporan" },
    NavDef { icon: "group_add", label: "User Control", href: "/kontrol-pengguna" },
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

/// Kartu "belum ada data" — ikon + judul ramah + ajakan opsional. Pengganti
/// `.ppm-empty` teks polos; dipakai utk state kosong yg WAJAR (bukan loading,
/// bukan error) spy tak terasa seperti halaman rusak.
#[component]
pub fn EmptyState(
    icon: &'static str,
    title: &'static str,
    #[prop(optional)] subtitle: Option<&'static str>,
) -> impl IntoView {
    view! {
        <div class="ppm-empty space-y-1.5 anim-in">
            <span class="material-symbols-outlined text-4xl text-on-surface-variant/60">{icon}</span>
            <p class="text-body-md font-semibold text-on-background">{title}</p>
            {subtitle.map(|s| view! { <p class="text-body-sm text-on-surface-variant">{s}</p> })}
        </div>
    }
}

/// Nav peran: admin. Beranda → /staf.
pub const NAV_STAF: &[NavDef] = &[
    NavDef { icon: "dashboard", label: "Beranda", href: "/staf" },
    NavDef { icon: "calendar_month", label: "Kalender", href: "/kalender" },
    NavDef { icon: "groups", label: "Students", href: "/students" },
    NavDef { icon: "school", label: "Kelas", href: "/kelas" },
    NavDef { icon: "bar_chart", label: "Laporan", href: "/laporan" },
    NavDef { icon: "group_add", label: "User Control", href: "/kontrol-pengguna" },
];

/// Nav peran: guru (teacher). Beranda → /guru.
pub const NAV_GURU: &[NavDef] = &[
    NavDef { icon: "dashboard", label: "Beranda", href: "/guru" },
    NavDef { icon: "calendar_month", label: "Kalender", href: "/kalender" },
    NavDef { icon: "groups", label: "Students", href: "/students" },
    NavDef { icon: "school", label: "Kelas", href: "/kelas" },
    NavDef { icon: "bar_chart", label: "Laporan", href: "/laporan" },
    NavDef { icon: "group_add", label: "User Control", href: "/kontrol-pengguna" },
];

/// Nav peran: dewan guru. Beranda → /dewan-guru.
pub const NAV_DEWAN: &[NavDef] = &[
    NavDef { icon: "dashboard", label: "Beranda", href: "/dewan-guru" },
    NavDef { icon: "calendar_month", label: "Kalender", href: "/kalender" },
    NavDef { icon: "groups", label: "Students", href: "/students" },
    NavDef { icon: "school", label: "Kelas", href: "/kelas" },
    NavDef { icon: "bar_chart", label: "Laporan", href: "/laporan" },
    NavDef { icon: "group_add", label: "User Control", href: "/kontrol-pengguna" },
];

/// Nav peran: orang tua.
pub const NAV_ORTU: &[NavDef] = &[
    NavDef { icon: "home", label: "Beranda", href: "/orang-tua" },
    NavDef { icon: "calendar_month", label: "Kalender", href: "/kalender" },
    NavDef { icon: "history", label: "Riwayat", href: "/orang-tua/riwayat" },
    NavDef { icon: "event_available", label: "Izin", href: "/orang-tua/izin" },
    NavDef { icon: "bar_chart", label: "Laporan", href: "/laporan" },
];

/// SATU sumber kebenaran navbar bawah per-PERAN. Semua halaman WAJIB memakai ini
/// (jangan hardcode / duplikat match) agar navbar konsisten saat pindah halaman.
///   • santri  → NAV_SANTRI (Beranda·Riwayat·Sesi·Izin·Laporan)
///   • parent  → NAV_ORTU   (Beranda·Riwayat·Izin·Laporan)
///   • STAF (admin/pamong/guru/dewan) → item SAMA (Beranda·Students·Kelas·Sesi·
///     Laporan); hanya tujuan "Beranda" beda per peran.
pub fn nav_for(role: &str) -> &'static [NavDef] {
    match role {
        "parent" => NAV_ORTU,
        "supervisor" => NAV_PAMONG,
        "teacher" => NAV_DEWAN, // 'teacher' digabung ke dewan_guru (migrasi 36)
        "dewan_guru" => NAV_DEWAN,
        "admin" | "ketua" => NAV_STAF, // ketua = admin + finance
        "santri_finance" => NAV_SANTRI_FINANCE, // santri + akses finance /tagihan
        _ => NAV_SANTRI, // santri + fallback aman
    }
}

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

/// Higher-Order Component: Guard halaman yang wajib authenticated.
/// Otomatis redirect ke /login jika session tidak ada/invalid.
/// Menggantikan `Effect::new(|_| { if !authed { redirect("/login") }})`
/// yang duplikat di 10+ halaman.
///
/// # Usage
/// ```leptos
/// <AuthGuard>
///     <YourPageComponent />
/// </AuthGuard>
/// ```
#[component]
pub fn AuthGuard(#[prop(into)] children: ChildrenFn) -> impl IntoView {
    let session = use_context::<Resource<Option<SessionUser>>>();

    Effect::new(move |_| {
        if let Some(s) = session {
            let is_authed = s.get().flatten().is_some();
            if !is_authed {
                #[cfg(target_arch = "wasm32")]
                if let Some(w) = web_sys::window() {
                    let _ = w.location().replace("/login");
                }
            }
        }
    });

    view! { <Suspense>{children()}</Suspense> }
}

/// Guard dengan role requirement.
/// Redirect ke /login jika tidak authenticated, atau /forbidden jika role tidak match.
///
/// # Usage
/// ```leptos
/// <RoleGuard required_roles=vec!["admin", "ketua"]>
///     <AdminOnlyComponent />
/// </RoleGuard>
/// ```
#[component]
pub fn RoleGuard(
    #[prop(into)] required_roles: Vec<&'static str>,
    #[prop(into)] children: ChildrenFn,
) -> impl IntoView {
    let session = use_context::<Resource<Option<SessionUser>>>();

    Effect::new(move |_| {
        if let Some(s) = session {
            match s.get().flatten() {
                None => {
                    #[cfg(target_arch = "wasm32")]
                    if let Some(w) = web_sys::window() {
                        let _ = w.location().replace("/login");
                    }
                }
                Some(user) => {
                    let has_role = required_roles
                        .iter()
                        .any(|&r| crate::models::role_satisfies(&user.role, &[r]));
                    if !has_role {
                        #[cfg(target_arch = "wasm32")]
                        if let Some(w) = web_sys::window() {
                            let _ = w.location().replace("/forbidden");
                        }
                    }
                }
            }
        }
    });

    view! { <Suspense>{children()}</Suspense> }
}
