//! web/components.rs — Komponen bersama.

use std::collections::HashMap;

use leptos::prelude::*;
use leptos_router::hooks::use_location;

use crate::models::{BookProgressItem, SessionUser, Surah};

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
        "/kelas-saya",
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

// CATATAN: santri_finance memakai NAV_SANTRI yang SAMA PERSIS (tak ada item
// tambahan di navbar). Akses kelola pembayaran (/tagihan) lewat tombol di
// BERANDA santri — pola sama seperti ketua/staf yang punya grid "Alat
// Administrasi" di dashboard-nya, bukan lewat navbar.

// ── Navbar STAF SERAGAM ──────────────────────────────────────────────────────
// admin / pamong / guru / dewan-guru memakai item YANG SAMA (Beranda · Students ·
// Kelas · Laporan · User Control) supaya navbar tak "berubah-ubah" antar
// halaman. Yang beda HANYA tujuan "Beranda" (dashboard tiap peran, dari
// models::role_home). Kelas & Sesi DIGABUNG jadi satu halaman/nav (/kelas
// dgn tab Kelas/Sesi) — dulu 2 item terpisah; santri/ortu tetap via /sesi
// standalone (nav mereka tak berubah).

/// Deret nav STAF — satu-satunya yang berbeda antar peran adalah tujuan
/// "Beranda", jadi itulah satu-satunya yang jadi parameter.
///
/// Dulu tiap peran menuliskan keenam itemnya sendiri. Hasilnya tiga salinan
/// identik (empat, dengan `NAV_GURU` yang bahkan tak pernah dipakai): menambah
/// satu menu berarti menyunting semuanya, dan yang terlewat baru ketahuan saat
/// navbar seorang peran terlihat beda sendiri. Makro dipakai — bukan fungsi —
/// supaya hasilnya tetap `&'static [NavDef]` yang bisa dipakai di `const`.
macro_rules! nav_staf {
    ($beranda:expr) => {
        &[
            NavDef { icon: "dashboard", label: "Beranda", href: $beranda },
            NavDef { icon: "calendar_month", label: "Kalender", href: "/kalender" },
            NavDef { icon: "groups", label: "Students", href: "/students" },
            NavDef { icon: "school", label: "Kelas", href: "/kelas" },
            NavDef { icon: "bar_chart", label: "Laporan", href: "/laporan" },
            NavDef { icon: "group_add", label: "User Control", href: "/kontrol-pengguna" },
        ]
    };
}

/// Nav peran: pamong (supervisor). Beranda → /verifikasi-pamong.
pub const NAV_PAMONG: &[NavDef] = nav_staf!("/verifikasi-pamong");

/// Apakah pesan galat ini berarti "pengguna harus masuk lagi"?
///
/// SATU tempat, dipakai ~26 halaman yang dulu masing-masing mencocokkan string
/// sendiri. Saat penanda baru muncul (mis. `session_expired` ketika umur token
/// dipangkas dari 100 tahun jadi 30 hari), cukup ditambahkan di sini — tanpa
/// itu, ada halaman yang lupa diperbarui dan penggunanya terjebak di layar
/// galat tanpa pernah dialihkan ke /login.
pub fn is_auth_error(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("unauth") || m.contains("session_expired") || m.contains("forbidden")
}

/// Tampilan galat fetch yang JUJUR: galat autentikasi → ajak masuk lagi; galat
/// lain → pesan + tombol Coba Lagi. Mencegah "Sesi berakhir" yang menyesatkan
/// untuk galat non-auth.
///
/// `err` di sini SUDAH disaring `api::err()`: hanya pesan `UserError` (aturan
/// bisnis) dan penanda sesi yang lolos apa adanya, sedangkan galat server
/// datang sebagai kalimat generik. Jadi merender isinya ke layar aman — rantai
/// galat Postgres tak pernah sampai ke sini.
#[component]
pub fn FetchError(err: String) -> impl IntoView {
    // "session_expired" = umur token habis (wajar); "unauth" = token cacat/tak
    // ada. Keduanya berujung sama bagi pengguna: harus masuk lagi.
    let expired = err.contains("session_expired");
    // "forbidden" BUKAN sesi habis — pengguna sudah masuk, hanya perannya tak
    // berwenang. Menyuruhnya login ulang menyesatkan: ia akan masuk lagi
    // dengan akun yang sama dan menemui layar yang sama.
    let forbidden = err.contains("forbidden") && !expired && !err.contains("unauth");
    if forbidden {
        return view! {
            <div class="pt-10 text-center space-y-3 anim-in">
                <span class="material-symbols-outlined text-5xl text-on-surface-variant">"lock"</span>
                <p class="text-body-md font-semibold text-on-background">"Tidak berwenang"</p>
                <p class="text-body-sm text-on-surface-variant px-6">
                    "Peran akun Anda tak punya akses ke halaman ini. Hubungi admin bila menurut Anda ini keliru."
                </p>
            </div>
        }
            .into_any();
    }
    let is_auth = is_auth_error(&err);
    if is_auth {
        // Bedakan sebabnya: sesi 30 hari yang habis itu WAJAR dan bukan salah
        // pengguna — jangan disamakan dengan "tidak berwenang".
        let pesan = if expired {
            "Sesi Anda sudah berakhir (berlaku 30 hari). Silakan masuk kembali."
        } else {
            "Sesi tidak berlaku. Silakan masuk kembali."
        };
        view! {
            <div class="pt-10 text-center space-y-4 anim-in">
                <p class="text-body-md text-on-surface-variant">{pesan}</p>
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
pub const NAV_STAF: &[NavDef] = nav_staf!("/staf");

/// Nav peran: dewan guru (termasuk 'teacher' lama, digabung di migrasi 36).
/// Beranda → /dewan-guru.
pub const NAV_DEWAN: &[NavDef] = nav_staf!("/dewan-guru");

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
        // santri_finance = navbar SAMA PERSIS dengan santri. Akses kelola
        // pembayaran lewat tombol di beranda, bukan navbar.
        _ => NAV_SANTRI, // santri + santri_finance + fallback aman
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

/// Satu foto kegiatan di dalam bingkainya, lengkap dengan mode isi & bidikan.
///
/// Dipakai di SEMUA tempat foto kegiatan tampil — beranda publik (3:4), grid
/// pengelola (1:1), dan pratinjau editor — supaya ketiganya tak bisa berbeda.
/// Rasio bingkainya ditentukan pemanggil lewat `class`; itulah gunanya menyimpan
/// titik fokus alih-alih hasil potongan.
///
/// Pada mode "muat seluruhnya" foto tidak memenuhi bingkai, dan ruang sisanya
/// diisi versi buram foto itu sendiri — bukan blok abu-abu — supaya foto tegak
/// di antara foto lanskap terlihat disengaja, bukan seperti tata letak yang rusak.
#[component]
pub fn PhotoFrame(
    #[prop(into)] src: String,
    #[prop(into)] style: String,
    /// Tampilkan latar buram di belakang foto (mode `contain`).
    backdrop: bool,
    #[prop(into, optional)] alt: String,
    /// Kelas bingkai luar — di sinilah rasio ditentukan, mis. `aspect-[3/4]`.
    #[prop(into, optional)] class: String,
    #[prop(optional)] lazy: bool,
) -> impl IntoView {
    let frame_class = format!("relative overflow-hidden {class}");
    view! {
        <div class=frame_class>
            {backdrop
                .then(|| {
                    view! {
                        <img
                            src=src.clone()
                            style=crate::models::BACKDROP_STYLE
                            alt=""
                            aria-hidden="true"
                        />
                    }
                })}
            <img
                src=src
                style=style
                alt=alt
                loading=if lazy { "lazy" } else { "eager" }
                class="relative"
            />
        </div>
    }
}

/// Panel modal: **bottom-sheet di ponsel, dialog terpusat di desktop**.
///
/// Menggantikan markup scrim+panel yang sebelumnya disalin di empat halaman
/// (progres materi santri dari sisi staf, dari sisi wali kelas, dari sisi orang
/// tua, dan QR absensi). Selain menghilangkan duplikasi, ini memperbaiki
/// tampilan desktop yang rusak: dengan sidebar terbuka, aturan CSS untuk bilah
/// melayang ikut mengenai sheet lama sehingga scrim-nya menyusut jadi pita
/// selebar 36rem — latar gelap tak menutup layar — dan panelnya terdorong ke
/// kiri lalu menggantung di dasar layar. Lihat catatan lengkap di `.ppm-sheet`
/// (style/tailwind.css).
///
/// Perilaku dialog yang ikut didapat semua pemakai: tutup dengan tombol Esc,
/// klik di luar panel, atau tombol ×; serta atribut `role="dialog"` +
/// `aria-modal` supaya pembaca layar memperlakukannya sebagai modal.
#[component]
pub fn Sheet(
    #[prop(into)] title: String,
    /// Dipanggil saat sheet ditutup lewat cara apa pun.
    on_close: impl Fn() + Copy + Send + Sync + 'static,
    /// Judul di tengah tanpa tombol × di kanan (dipakai sheet QR absensi).
    #[prop(optional)] center_title: bool,
    children: Children,
) -> impl IntoView {
    let panel: NodeRef<leptos::html::Div> = NodeRef::new();

    // Esc menutup — bawaan yang diharapkan dari sebuah dialog, dan satu-satunya
    // jalan keluar lewat papan ketik. Listener dipasang di `document` (bukan
    // panel) supaya tetap bekerja sebelum ada elemen yang terfokus.
    //
    // Tab DIKURUNG di dalam panel. Tanpa itu, `aria-modal="true"` berbohong:
    // pembaca layar diberi tahu ini modal, tapi Tab tetap berjalan ke tautan
    // dan tombol di halaman DI BALIK scrim — yang tak terlihat, tak bisa
    // diklik, dan tak jelas di mana fokusnya berada.
    //
    // Listener juga DILEPAS saat sheet ditutup. Versi lama memakai
    // `cb.forget()`, jadi tiap kali sheet dibuka satu listener menumpuk lagi di
    // `document` dan yang lama tetap memanggil `on_close` milik sheet yang
    // sudah tak ada.
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;

        /// Elemen yang bisa menerima fokus papan ketik, urut sesuai dokumen.
        const FOKUSABEL: &str = "a[href], button:not([disabled]), \
             input:not([disabled]), select:not([disabled]), \
             textarea:not([disabled]), [tabindex]:not([tabindex='-1'])";

        Effect::new(move |_| {
            let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
                return;
            };
            let Some(panel_el) = panel.get() else {
                return;
            };

            // Elemen yang terfokus SEBELUM sheet muncul — dikembalikan saat
            // ditutup supaya pengguna papan ketik kembali ke tempat semula,
            // bukan terlempar ke awal halaman.
            let pemicu = doc
                .active_element()
                .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok());

            // Fokus dipindah ke elemen pertama di dalam panel; kalau tak ada,
            // ke panelnya sendiri (tabindex=-1) agar Esc & pembaca layar
            // langsung berada di konteks yang benar.
            let fokus_pertama = panel_el
                .query_selector(FOKUSABEL)
                .ok()
                .flatten()
                .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok());
            match fokus_pertama {
                Some(el) => {
                    let _ = el.focus();
                }
                None => {
                    let _ = panel_el.focus();
                }
            }

            let trap_el = panel_el.clone();
            let trap_doc = doc.clone();
            let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |e: web_sys::Event| {
                let Some(k) = e.dyn_ref::<web_sys::KeyboardEvent>() else {
                    return;
                };
                match k.key().as_str() {
                    "Escape" => on_close(),
                    "Tab" => {
                        let Ok(list) = trap_el.query_selector_all(FOKUSABEL) else {
                            return;
                        };
                        let n = list.length();
                        if n == 0 {
                            return;
                        }
                        let el_ke = |i: u32| {
                            list.item(i)
                                .and_then(|n| n.dyn_into::<web_sys::HtmlElement>().ok())
                        };
                        let (Some(awal), Some(akhir)) = (el_ke(0), el_ke(n - 1)) else {
                            return;
                        };
                        let aktif = trap_doc.active_element();
                        // Hanya ujung-ujungnya yang dibelokkan; Tab di tengah
                        // dibiarkan berjalan normal.
                        let di_awal = aktif.as_ref().map(|a| a == awal.as_ref()) == Some(true);
                        let di_akhir = aktif.as_ref().map(|a| a == akhir.as_ref()) == Some(true);
                        if k.shift_key() && di_awal {
                            e.prevent_default();
                            let _ = akhir.focus();
                        } else if !k.shift_key() && di_akhir {
                            e.prevent_default();
                            let _ = awal.focus();
                        }
                    }
                    _ => {}
                }
            });

            let _ =
                doc.add_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref());

            // SendWrapper: Closure/HtmlElement bukan Send, on_cleanup butuh Send
            // — aman karena cleanup jalan di thread browser yang sama.
            let held = send_wrapper::SendWrapper::new((doc, cb, pemicu));
            on_cleanup(move || {
                let (doc, cb, pemicu) = held.take();
                let _ = doc
                    .remove_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref());
                if let Some(el) = pemicu {
                    let _ = el.focus();
                }
            });
        });
    }

    view! {
        <div class="ppm-scrim" on:click=move |_| on_close()></div>
        <div
            node_ref=panel
            class="ppm-sheet"
            role="dialog"
            aria-modal="true"
            // Agar panel bisa menerima fokus saat tak ada elemen fokusabel di
            // dalamnya — tanpa ini fokus tertinggal di luar modal.
            tabindex="-1"
        >
            <div class="ppm-sheet-grip w-10 h-1.5 bg-outline-variant rounded-full mx-auto mb-5"></div>
            {if center_title {
                view! {
                    <h3 class="text-headline-sm text-on-background text-center">{title}</h3>
                }
                    .into_any()
            } else {
                view! {
                    <div class="flex items-center justify-between mb-4">
                        <h3 class="text-headline-sm text-on-background">{title}</h3>
                        <button
                            class="w-8 h-8 rounded-full flex items-center justify-center text-on-surface-variant hover:bg-surface-container"
                            on:click=move |_| on_close()
                            aria-label="Tutup"
                        >
                            <span class="material-symbols-outlined text-lg">"close"</span>
                        </button>
                    </div>
                }
                    .into_any()
            }}
            {children()}
        </div>
    }
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

// ═══════════════════════════════════════════════════════════════════════════════
// Detail Progres Materi (read-only) — ditampilkan orang tua / staf / guru.
// ═══════════════════════════════════════════════════════════════════════════════

/// Panel detail progres materi satu santri dalam bentuk grid per-unit (ayat/
/// halaman) — MIRIP tampilan santri saat mengisi di /akademik, tapi read-only.
/// Dipakai oleh modal/sheet "Lihat Detail Progres" di beranda orang tua dan
/// panel progres buku halaman Students.
#[component]
pub fn BookProgressDetail(
    student_name: String,
    items: Vec<BookProgressItem>,
) -> impl IntoView {
    view! {
        <div class="space-y-4">
            <p class="text-body-sm font-bold text-on-background flex items-center gap-1.5">
                <span class="material-symbols-outlined text-primary text-[18px]">"auto_stories"</span>
                {format!("Detail Progres Materi — {student_name}")}
            </p>

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

            {if items.is_empty() {
                view! {
                    <div class="bg-surface-container rounded-2xl p-5 text-center text-body-sm text-on-surface-variant">
                        "Belum ada materi terdaftar."
                    </div>
                }
                    .into_any()
            } else {
                items
                    .into_iter()
                    .map(|b| view! { <BookProgressDetailCard b=b /> })
                    .collect_view()
                    .into_any()
            }}
        </div>
    }
}

#[component]
fn BookProgressDetailCard(b: BookProgressItem) -> impl IntoView {
    let total = b.total_pages.max(1);
    let is_quran = b.category == "quran";
    let title = b.book_title.clone();
    let surahs = b.surahs.clone();
    let status = b.unit_status.clone();
    let pct = b.percentage as i32;

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
                <span class="text-body-sm font-bold text-primary shrink-0">{format!("{pct}%")}</span>
            </div>
            <div class="h-1.5 bg-surface-container rounded-full overflow-hidden">
                <div class="h-full bg-primary" style=format!("width: {pct}%")></div>
            </div>

            {if is_quran {
                view! {
                    <BookProgressQuran surahs=surahs status=status />
                }
                    .into_any()
            } else {
                view! {
                    <BookProgressHadist total=total status=status />
                }
                    .into_any()
            }}
        </div>
    }
}

#[component]
fn BookProgressQuran(surahs: Vec<Surah>, status: HashMap<String, u8>) -> impl IntoView {
    let sel_surah = RwSignal::new(0usize);
    let surahs = StoredValue::new(surahs);

    view! {
        <div class="space-y-2">
            <div class="flex flex-wrap gap-1.5">
                {surahs
                    .get_value()
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        let nm = s.name.clone();
                        let cls = move || {
                            if sel_surah.get() == i {
                                "px-2.5 py-1 rounded-full text-[11px] font-semibold bg-primary text-on-primary whitespace-nowrap"
                            } else {
                                "px-2.5 py-1 rounded-full text-[11px] bg-surface-container text-on-surface whitespace-nowrap"
                            }
                        };
                        view! {
                            <button type="button" class=cls on:click=move |_| sel_surah.set(i)>
                                {nm}
                            </button>
                        }
                    })
                    .collect_view()}
            </div>
            {move || {
                let idx = sel_surah.get();
                let list = surahs.get_value();
                let Some(s) = list.get(idx) else { return ().into_any() };
                let ayat = s.ayat.max(0);
                let sname = s.name.clone();
                view! {
                    <p class="text-[11px] text-on-surface-variant">
                        {format!("{sname} — {ayat} ayat")}
                    </p>
                    <div class="grid grid-cols-10 gap-1 max-h-72 overflow-y-auto">
                        {(1..=ayat)
                            .map(|a| {
                                let key = format!("{idx}:{a}");
                                let st = status.get(&key).copied().unwrap_or(0);
                                view! { <ProgressUnitCell label=a status=st /> }
                            })
                            .collect_view()}
                    </div>
                }
                    .into_any()
            }}
        </div>
    }
}

#[component]
fn BookProgressHadist(total: i32, status: HashMap<String, u8>) -> impl IntoView {
    view! {
        <div class="grid grid-cols-10 gap-1 max-h-72 overflow-y-auto">
            {(1..=total)
                .map(|p| {
                    let key = p.to_string();
                    let st = status.get(&key).copied().unwrap_or(0);
                    view! { <ProgressUnitCell label=p status=st /> }
                })
                .collect_view()}
        </div>
    }
}

/// Satu unit (ayat/halaman) read-only: warna menunjukkan status.
#[component]
fn ProgressUnitCell(label: i32, status: u8) -> impl IntoView {
    let cls = match status {
        2 => "aspect-square rounded text-[10px] font-bold flex items-center justify-center bg-primary text-on-primary",
        1 => "aspect-square rounded text-[10px] font-bold flex items-center justify-center bg-warning/70 text-on-background",
        _ => "aspect-square rounded text-[10px] flex items-center justify-center bg-surface-container-highest text-on-surface-variant",
    };
    view! {
        <div class=cls>{label}</div>
    }
}

/// Dek "Jadwal Berikutnya" yang bisa DIGESER ke jadwal-jadwal sesudahnya.
///
/// Sebelumnya kartu ini hanya menampilkan SATU sesi — hasil `today.iter()
/// .find(..)` — sehingga pengajar yang ingin tahu "habis ini apa lagi" harus
/// turun ke daftar "Sesi Hari Ini" di bawah. Padahal isinya sesi yang sama,
/// hanya disajikan dua kali dengan bentuk berbeda. Sekarang kartu besarnya
/// sendiri yang bisa digeser, dan daftar di bawah tetap ada sebagai ikhtisar.
///
/// Sesi ber-`state == "break"` disaring — sama seperti aturan lama untuk
/// memilih "jadwal berikutnya" — karena jeda bukan sesuatu yang dibuka.
///
/// Dipakai beranda guru/dewan guru (`analisis`) dan pamong
/// (`verifikasi_pamong`) supaya keduanya tak punya salinan markup sendiri.
#[component]
pub fn JadwalDeck(sesi: Vec<crate::models::LiveSesi>) -> impl IntoView {
    // Yang layak ditawarkan di kartu besar hanyalah sesi yang MASIH relevan:
    //   • sedang berlangsung → selalu tampil, walau jam mulainya sudah lewat;
    //   • akan datang → tampil selama jamnya belum lewat;
    //   • jeda (`break`) & yang jamnya sudah lewat → tidak.
    //
    // Sesi yang sudah lewat TIDAK hilang — ia tetap ada di daftar "Sesi Hari
    // Ini" di bawah, yang memang perannya sebagai catatan hari ini. Tanpa
    // saringan ini kartu paling atas bisa menawarkan sesi subuh 04:40 pada
    // pukul 15:50, karena `status` sesi tak pernah bergerak sendiri.
    let sesi: Vec<_> = sesi
        .into_iter()
        .filter(|s| s.state == "live" || (s.state != "break" && !s.past))
        .collect();
    let total = sesi.len();
    let aktif = RwSignal::new(0usize);
    let track: NodeRef<leptos::html::Div> = NodeRef::new();

    if total == 0 {
        return ().into_any();
    }

    // Label kartu mengikuti POSISI, bukan disamakan semua: kartu ke-2 dan
    // seterusnya bukan "jadwal berikutnya" lagi, dan menyebutnya begitu di lima
    // kartu sekaligus membuat labelnya kehilangan arti.
    let kartu = sesi
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, s)| {
            let live = s.state == "live";
            let label = if live {
                "SEDANG BERLANGSUNG"
            } else if i == 0 {
                "JADWAL BERIKUTNYA"
            } else {
                "JADWAL SESUDAHNYA"
            };
            view! {
                <div class="spiritual-gradient rounded-2xl p-5 text-on-primary shadow-lg shadow-primary/20">
                    <p class="text-[11px] font-bold tracking-[0.2em] opacity-80">{label}</p>
                    <p class="text-display-md mt-1 truncate">{s.title.clone()}</p>
                    <p class="text-body-sm opacity-85 mt-1 truncate">
                        {format!("{} • {} • {} santri", s.time_label, s.teacher, s.santri_count)}
                    </p>
                    <a
                        href=format!("/sesi/{}", s.id)
                        class="mt-4 w-full py-3 rounded-xl bg-primary-fixed text-primary font-bold text-body-sm flex items-center justify-center gap-2 press"
                    >
                        <span class="material-symbols-outlined text-[18px]">"play_circle"</span>
                        "Lihat Sesi"
                    </a>
                </div>
            }
        })
        .collect_view();

    // Satu sesi saja: tak ada yang bisa digeser, jadi jangan pasang dek maupun
    // titik indikator yang menjanjikan sesuatu yang tak ada.
    if total == 1 {
        return view! { <div>{kartu}</div> }.into_any();
    }

    // Posisi dihitung dari scrollLeft supaya titik indikator tetap jujur
    // walau pengguna menggeser separuh jalan lalu melepas.
    let on_scroll = move |_| {
        #[cfg(target_arch = "wasm32")]
        if let Some(el) = track.get_untracked() {
            let lebar = el.client_width();
            if lebar > 0 {
                let i = (el.scroll_left() as f64 / lebar as f64 + 0.5).floor() as usize;
                aktif.set(i.min(total - 1));
            }
        }
    };

    view! {
        <div>
            <div
                node_ref=track
                on:scroll=on_scroll
                class="ppm-swipe"
                role="group"
                aria-label="Jadwal berikutnya — geser untuk melihat jadwal lain"
            >
                {kartu}
            </div>
            // Titik indikator: penanda "masih ada lagi di samping", sekaligus
            // penunjuk posisi. Sengaja TIDAK bisa diklik — ia melaporkan
            // keadaan, dan menggeser tetap cara utamanya.
            <div class="flex items-center justify-center gap-1.5 mt-2.5" aria-hidden="true">
                {(0..total)
                    .map(|i| {
                        view! {
                            <span class=move || {
                                if aktif.get() == i {
                                    "w-4 h-1.5 rounded-full bg-primary transition-all"
                                } else {
                                    "w-1.5 h-1.5 rounded-full bg-outline-variant transition-all"
                                }
                            }></span>
                        }
                    })
                    .collect_view()}
            </div>
        </div>
    }
    .into_any()
}

/// Panel yang HANYA boleh diubah admin/ketua.
///
/// Menampilkan `children` bila boleh; bila tidak, menggantinya dengan
/// keterangan bahwa bagian ini terkunci — BUKAN membiarkan tombolnya tampil
/// lalu gagal dengan "forbidden" saat ditekan. Pengguna berhak tahu batas
/// wewenangnya sebelum mencoba, bukan sesudah.
#[component]
pub fn AdminOnly(
    can_manage: bool,
    /// Apa yang tak bisa dikerjakan, mis. "menambah santri ke kelas".
    #[prop(into)]
    apa: String,
    // `Children` (FnOnce), bukan `ChildrenFn`: isinya dirender paling banyak
    // sekali, dan ChildrenFn menuntut Sync — yang tak dipenuhi closure
    // `refetch` milik halaman kelas.
    children: Children,
) -> impl IntoView {
    if can_manage {
        return view! { {children()} }.into_any();
    }
    view! {
        <div class="ppm-card p-4 flex items-start gap-3 border-dashed">
            <span class="w-9 h-9 rounded-xl bg-surface-container-highest text-on-surface-variant flex items-center justify-center shrink-0">
                <span class="material-symbols-outlined text-[18px]">"lock"</span>
            </span>
            <div class="min-w-0">
                <p class="text-body-sm font-semibold text-on-background">"Hanya admin"</p>
                <p class="text-[11px] text-on-surface-variant">
                    {format!("Hanya admin/ketua yang boleh {apa}. Hubungi admin bila perlu diubah.")}
                </p>
            </div>
        </div>
    }
    .into_any()
}

/// Lencana kecil "Hanya admin" — untuk disematkan di judul panel yang isinya
/// tetap ditampilkan (baca-saja), bukan disembunyikan.
#[component]
pub fn LencanaAdmin() -> impl IntoView {
    view! {
        <span class="ppm-chip bg-surface-container-highest text-on-surface-variant inline-flex items-center gap-1">
            <span class="material-symbols-outlined text-[14px]">"lock"</span>
            "Hanya admin"
        </span>
    }
}

// ── Utilitas bersama halaman ─────────────────────────────────────────────────

/// Ambil bagian pesan galat yang layak dibaca pengguna.
///
/// `ServerFnError` merangkai konteksnya dengan ": " (mis.
/// "error running server function: Poin telat wajib diisi"). Yang berguna cuma
/// potongan terakhir. Pola `e.to_string().rsplit(": ").next()...` ini dulu
/// disalin di 37 tempat — satu salinan yang lupa diperbaiki cukup untuk
/// menampilkan jargon internal ke layar santri.
pub fn pesan_galat(e: impl ToString) -> String {
    let s = e.to_string();
    s.rsplit(": ").next().unwrap_or(&s).to_string()
}

/// Alihkan ke /login bila galat sebuah Resource menandakan sesi tak berlaku.
///
/// Menggantikan blok `Effect::new` + `is_auth_error` + `window().location()`
/// yang disalin di 24 halaman. Sengaja HANYA untuk galat sesi: `forbidden`
/// ditangani `FetchError` dengan tampilan sendiri, karena login ulang tak
/// menolong orang yang perannya memang tak berwenang.
pub fn guard_sesi<T>(data: Resource<Result<T, ServerFnError>>)
where
    T: Send + Sync + Clone + 'static,
{
    Effect::new(move |_| {
        if let Some(Err(e)) = data.get() {
            let msg = e.to_string();
            // `forbidden` sengaja dikecualikan — lihat FetchError.
            if msg.contains("unauth") || msg.contains("session_expired") {
                #[cfg(target_arch = "wasm32")]
                if let Some(w) = web_sys::window() {
                    let _ = w.location().replace("/login");
                }
            }
        }
    });
}

/// Kotak pencarian dengan ikon — dipakai daftar kelas, santri, pengguna, tagihan.
#[component]
pub fn KotakCari(
    #[prop(into)] placeholder: String,
    nilai: RwSignal<String>,
) -> impl IntoView {
    view! {
        <div class="relative">
            <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-outline text-[20px]">
                "search"
            </span>
            <input
                type="text"
                class="w-full pl-10 pr-3 py-2.5 bg-surface-container border-0 rounded-xl text-body-sm text-on-surface"
                placeholder=placeholder
                prop:value=move || nilai.get()
                on:input=move |ev| nilai.set(event_target_value(&ev))
            />
        </div>
    }
}

/// Kategori kelas yang layak dipajang DI SAMPING golongannya.
///
/// Kelas non-akademik kerap memakai kata yang sama untuk keduanya ("piket" /
/// "piket"), dan dua lencana kembar berdampingan tak menerangkan apa pun —
/// hanya bising. Kosong berarti "jangan tampilkan lencana kedua".
pub fn kategori_tampil(golongan: &str, category: &str) -> String {
    if category.trim().eq_ignore_ascii_case(golongan.trim()) {
        String::new()
    } else {
        category.to_string()
    }
}

/// Daftar kartu dua kolom yang tingginya berdiri sendiri (masonry).
///
/// `<div class="ppm-card-grid">` polos memakai CSS Grid, dan grid selalu
/// menyusun per BARIS: kartu pendek di sebelah kartu panjang meninggalkan
/// lubang kosong sampai baris berikutnya boleh mulai. Fungsi ini membagi kartu
/// selang-seling ke dua kolom terpisah — genap ke kiri, ganjil ke kanan —
/// sehingga tiap kolom mengalir sendiri dan tak ada lubang, sementara urutan
/// bacanya tetap kiri→kanan.
///
/// `order` ditempel per kartu karena di ponsel kedua kolom dilebur kembali
/// (`display:contents`); tanpa itu urutannya jadi 1,3,5,2,4. Lihat
/// `.ppm-card-col` di style/tailwind.css.
///
/// Dipakai untuk daftar kartu seragam. Kalau isinya campur (mis. satu pesan
/// "kosong" yang harus melebar penuh), pakai `.ppm-card-grid` langsung.
pub fn kartu_grid(kartu: Vec<AnyView>) -> impl IntoView {
    let (mut kiri, mut kanan) = (Vec::new(), Vec::new());
    for (i, k) in kartu.into_iter().enumerate() {
        let item = view! { <div style=format!("order:{i}")>{k}</div> };
        if i % 2 == 0 { kiri.push(item) } else { kanan.push(item) }
    }
    view! {
        <div class="ppm-card-grid">
            <div class="ppm-card-col">{kiri}</div>
            <div class="ppm-card-col">{kanan}</div>
        </div>
    }
}

/// Placeholder memuat berbentuk balok — menggantikan blok `animate-pulse`
/// yang ditulis ulang di puluhan halaman dengan tinggi berbeda-beda.
#[component]
pub fn Skeleton(
    /// Jumlah balok.
    #[prop(default = 3)]
    baris: usize,
    /// Kelas tinggi Tailwind, mis. "h-24".
    #[prop(default = "h-24")]
    tinggi: &'static str,
) -> impl IntoView {
    let cls = format!("{tinggi} bg-surface-container rounded-2xl");
    view! {
        <div class="animate-pulse space-y-3">
            {(0..baris).map(|_| view! { <div class=cls.clone()></div> }).collect_view()}
        </div>
    }
}
