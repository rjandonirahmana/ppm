//! web/components.rs — Komponen bersama.

use std::collections::HashMap;

use leptos::prelude::*;
use leptos_router::hooks::use_location;

use crate::models::{BookProgressItem, SessionUser, Surah};

/// Tombol gerigi pengaturan di header — SELALU ke `/profil`. `wide` = ikut
/// tampil di desktop.
///
/// Tujuannya sengaja tidak bergantung peran. Sempat dibuat begitu (admin →
/// `/setelan`), dan itu keliru: halaman itu konfigurasi APLIKASI, bukan
/// pengaturan milik orang yang sedang memakainya. Gerigi di sebelah lonceng
/// dibaca semua orang sebagai "akun saya", dan itu `/profil`, halaman yang sama
/// yang ditunjuk "Pengaturan" di sidebar desktop.
///
/// `/setelan` sendiri sudah TIDAK ADA (Ags 2026): alur persetujuan izin kini
/// diatur per-kelas di `/kelas/:id`, dan reset saldo poin awal semester pindah
/// ke `/poin` — tempat poin memang dikelola.
#[component]
fn SettingsLink(wide: bool) -> impl IntoView {
    let cls = if wide {
        "w-9 h-9 rounded-full flex items-center justify-center text-on-surface hover:bg-surface-container press"
    } else {
        "md:hidden w-9 h-9 rounded-full flex items-center justify-center text-on-surface hover:bg-surface-container press"
    };
    view! {
        <a href="/profil" class=cls aria-label="Pengaturan" title="Pengaturan">
            <span class="material-symbols-outlined">"settings"</span>
        </a>
    }
}

/// Header: mobile = sticky bar (judul + lonceng + setting). Desktop (md+, ala
/// TOPBAR mockup Admin Portal) = judul lebih besar non-sticky + **identitas
/// user** (avatar inisial + nama + peran) menggantikan tombol setting — sidebar
/// desktop sudah punya Settings/Logout sendiri, jadi header di sana cukup jadi
/// heading halaman + identitas, bukan duplikasi kontrol.
/// Tumpukan jalur yang sudah dilalui DI DALAM aplikasi ini sejak tab dibuka.
/// Disediakan `App`, dibaca tombol kembali [`MobileHeader`].
///
/// Tanpa ini tombol kembali tak punya pilihan selain `back_href` — alamat TETAP
/// yang ditulis di tiap halaman. Itulah sebab keluhan "kembali malah ke halaman
/// lain": dari `/students` menekan poin seorang santri lalu kembali, yang
/// terbuka `/poin` (tulisan di `poin.rs`), bukan `/students` tempat ia tadi
/// berada. Hal sama pada `/sesi/:id` (selalu ke `/sesi` walau datang dari detail
/// kelas) dan `/kelas/:id` (selalu ke `/kelas`).
///
/// KENAPA TUMPUKAN, bukan sekadar penghitung maju. Halaman yang dibuka LANGSUNG
/// (tautan WhatsApp, bookmark, hasil refresh) tak punya riwayat milik aplikasi
/// ini, dan `history.back()` di sana melempar pengguna keluar — ke Google atau
/// tab kosong. Penghitung yang hanya naik juga tak cukup: setelah pengguna
/// menekan tombol back PERAMBAN, hitungannya jadi terlalu besar dan tombol di
/// header ikut melempar keluar. Dengan tumpukan, langkah mundur dikenali
/// (jalur baru = satu tingkat di bawah puncak) dan puncaknya di-pop.
#[derive(Clone, Copy)]
pub struct RiwayatNav(pub RwSignal<Vec<String>>);

impl RiwayatNav {
    /// Masih ada halaman aplikasi di belakang layar ini?
    pub fn bisa_mundur(&self) -> bool {
        self.0.with_untracked(|v| v.len() > 1)
    }
}

/// Sediakan [`RiwayatNav`] — dipanggil di badan `App`, SEBELUM `<Router>`.
///
/// Harus di sana, bukan di komponen anak: `provide_context` hanya terlihat oleh
/// TURUNAN pemiliknya, jadi konteks yang dipasang di dalam sebuah anak Router
/// tak akan pernah sampai ke halaman yang jadi saudaranya.
pub fn sediakan_riwayat_nav() {
    provide_context(RiwayatNav(RwSignal::new(Vec::new())));
}

/// Catat tiap perpindahan halaman — dipanggil dari komponen DI DALAM `<Router>`
/// (`use_location` butuh konteks router).
///
/// Efeknya hanya berjalan di klien, jadi tumpukan ini tak pernah ikut ke HTML
/// server; markup tombol kembali sengaja SAMA di kedua sisi (selalu `<a href>`),
/// hanya perilakunya yang ditingkatkan setelah hidrasi.
pub fn lacak_perpindahan() {
    let Some(riwayat) = use_context::<RiwayatNav>() else { return };
    let pathname = use_location().pathname;
    Effect::new(move |_| {
        let kini = pathname.get();
        riwayat.0.update(|v| langkah_nav(v, &kini));
    });
}

/// Satu langkah perpindahan → perubahan tumpukan. Dipisah dari efeknya supaya
/// bisa diuji tanpa peramban; ini inti dari benar-tidaknya tombol kembali.
pub(crate) fn langkah_nav(tumpukan: &mut Vec<String>, kini: &str) {
    match tumpukan.last() {
        // Jalur yang sama dirender ulang (mis. query berubah) — bukan pindah.
        Some(atas) if atas == kini => {}
        // Jalur baru = tepat satu tingkat di bawah puncak → pengguna MUNDUR
        // (tombol back peramban ATAU tombol kembali di header). Puncaknya
        // dilepas, bukan ditumpuk lagi — kalau tidak, tumpukan terus tumbuh
        // saat orang bolak-balik dan tombol kembali akhirnya melempar keluar
        // aplikasi.
        _ if tumpukan.len() >= 2 && tumpukan[tumpukan.len() - 2] == kini => {
            tumpukan.pop();
        }
        _ => tumpukan.push(kini.to_string()),
    }
}

#[cfg(test)]
mod tests_nav {
    use super::langkah_nav;

    fn jalan(langkah: &[&str]) -> Vec<String> {
        let mut v = Vec::new();
        for l in langkah {
            langkah_nav(&mut v, l);
        }
        v
    }

    #[test]
    fn maju_menumpuk() {
        assert_eq!(jalan(&["/students", "/poin/7"]).len(), 2);
    }

    /// Halaman pertama = tak ada yang bisa dimunduri; tombol kembali WAJIB
    /// jatuh ke `back_href`, kalau tidak pengguna terlempar keluar aplikasi.
    #[test]
    fn halaman_pertama_tak_bisa_mundur() {
        assert_eq!(jalan(&["/poin/7"]).len(), 1);
    }

    #[test]
    fn mundur_melepas_puncak() {
        // /students → /poin/7 → kembali
        assert_eq!(jalan(&["/students", "/poin/7", "/students"]), vec!["/students"]);
    }

    /// Bolak-balik berkali-kali tak boleh menggelembungkan tumpukan — inilah
    /// yang membuat penghitung-maju-saja salah dan tombolnya keluar aplikasi.
    #[test]
    fn bolak_balik_tak_menggelembung() {
        let v = jalan(&["/sesi", "/sesi/3", "/sesi", "/sesi/3", "/sesi"]);
        assert_eq!(v, vec!["/sesi"]);
    }

    #[test]
    fn render_ulang_jalur_sama_diabaikan() {
        assert_eq!(jalan(&["/kelas", "/kelas", "/kelas"]), vec!["/kelas"]);
    }
}

#[component]
pub fn MobileHeader(
    title: &'static str,
    #[prop(optional)] back_href: Option<&'static str>,
    #[prop(optional)] subtitle: Option<&'static str>,
    /// Halaman BERANDA sebuah peran: gerigi setelan ikut tampil di desktop.
    ///
    /// Di halaman dalam, gerigi cukup ada di ponsel — di desktop sidebar sudah
    /// memuat "Pengaturan", dan dua jalan ke tempat yang sama pada satu layar
    /// hanya menambah keraguan. Di beranda justru sebaliknya: itu halaman yang
    /// dibuka pertama dan tempat orang mencari setelan.
    #[prop(optional)]
    settings: bool,
) -> impl IntoView {
    let session = use_context::<Resource<Option<SessionUser>>>();
    // Tombol kembali: pakai riwayat SUNGGUHAN bila pengguna sampai ke sini
    // lewat navigasi di dalam aplikasi; `back_href` hanya jaring pengaman untuk
    // halaman yang dibuka langsung. Lihat [`RiwayatNav`].
    let riwayat = use_context::<RiwayatNav>();
    let kembali = move |ev: leptos::ev::MouseEvent| {
        // Klik dengan Ctrl/Cmd/Shift/Alt atau tombol tengah = "buka di tab
        // baru". Membajaknya jadi history.back() akan membuat tab baru berisi
        // halaman yang salah — biarkan peramban yang mengurus.
        if ev.button() != 0 || ev.ctrl_key() || ev.meta_key() || ev.shift_key() || ev.alt_key() {
            return;
        }
        let ada_riwayat = riwayat.map(|r| r.bisa_mundur()).unwrap_or(false);
        if !ada_riwayat {
            // Tak ada riwayat aplikasi (halaman dibuka langsung / hasil
            // refresh) → `back_href` yang dipakai. Tapi delapan halaman staf
            // menulis "/staf" sebagai tujuan kembalinya, dan /staf adalah
            // BERANDA ADMIN. Dewan guru boleh membukanya (lihat
            // `staf_home_data`), jadi ia tak ditolak — ia cuma mendarat di
            // dashboard yang bukan miliknya, lalu menekan Beranda dan kembali
            // ke tampilan guru. Persis keluhan "beranda guru kadang seperti
            // admin, lalu balik lagi".
            //
            // Tujuannya dikoreksi SAAT KLIK, bukan saat render: membaca peran
            // dari Resource sesi di tengah render (di luar Suspense) memicu
            // hydration-mismatch, dan atribut `href` yang berbeda antara HTML
            // server dan hasil hidrasi persis bentuk masalah itu. Di sini
            // pembacaannya untracked dan hanya terjadi setelah jari menyentuh.
            // `back_href` disalin ke sini (Option<&'static str> itu Copy) —
            // `href` hanya ada di dalam closure penyusun tautannya, di bawah.
            #[cfg(target_arch = "wasm32")]
            if back_href == Some("/staf") {
                let peran = session
                    .and_then(|s| s.get_untracked())
                    .flatten()
                    .map(|u| u.role)
                    .unwrap_or_default();
                let beranda = crate::models::role_home(&peran);
                if !peran.is_empty() && beranda != "/staf" {
                    ev.prevent_default();
                    if let Some(w) = web_sys::window() {
                        let _ = w.location().assign(beranda);
                    }
                }
            }
            return; // selain itu: biarkan <a href> berjalan normal
        }
        ev.prevent_default();
        #[cfg(target_arch = "wasm32")]
        if let Some(w) = web_sys::window() {
            if let Ok(h) = w.history() {
                let _ = h.back();
            }
        }
    };
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
                            aria-label="Kembali"
                            on:click=kembali
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
            // Tujuannya tetap /profil untuk semua peran, jadi tak perlu membaca
            // sesi sama sekali — tak ada Transition, tak ada risiko
            // hydration-mismatch, dan tautannya sudah benar di HTML pertama.
            <SettingsLink wide=settings />
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
                        // SATU sumber label peran (`models::role_label`) — dua
                        // salinan `match` di berkas ini sempat menyimpang dan
                        // tetap menulis "Pamong" berbulan-bulan setelah peran
                        // itu dihapus (migrasi 84), karena tak ada yang
                        // mengingatkan bahwa keduanya harus diedit bersamaan.
                        let role_label = crate::models::role_label(&u.role);
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

/// Ikon & warna untuk tiap jenis notifikasi.
///
/// Ditaruh DI SINI, bukan di `models::notifikasi` yang lebih rapi secara
/// domain, karena `scripts/fetch-icons.sh` menyusun subset font hanya dari
/// token yang ditemukannya di `src/web`. Nama ikon yang hidup di luar direktori
/// itu tak pernah masuk subset, dan ikon yang tak ada di subset TIDAK tampil
/// sebagai kotak kosong — ia tampil sebagai TULISAN "check_circle" di tengah
/// daftar. Kerapian domain tak sebanding dengan cara gagal seperti itu.
///
/// Menambah baris di sini = jalankan `./scripts/fetch-icons.sh`.
fn ikon_notif(kind: &str) -> (&'static str, &'static str) {
    use crate::models::notifikasi::jenis;
    match kind {
        jenis::IZIN_DISETUJUI => ("check_circle", "text-tertiary"),
        jenis::IZIN_DITOLAK => ("cancel", "text-error"),
        _ => ("mail", "text-primary"),
    }
}

/// Lonceng notifikasi: klik → popover berisi feed sungguhan.
///
/// Resource-nya TIDAK dibuat di sini melainkan diambil dari konteks (lihat
/// `web::app::App`): komponen ini dirender dua kali per halaman — versi ponsel
/// dan versi desktop — dan resource per-komponen berarti dua permintaan untuk
/// jawaban yang sama.
#[component]
pub fn NotifBell() -> impl IntoView {
    let open = RwSignal::new(false);
    let notif = use_context::<Resource<crate::models::NotifData>>();

    // DUA pembaca untuk sumber yang sama, dan perbedaannya penting.
    //
    // `belum` dibaca saat RENDER, jadi ia harus berlangganan — dan karena itu
    // pula ia WAJIB berada di dalam `<Transition/>`. Membaca resource saat
    // render di luar Suspense/Transition membuat Leptos memperingatkan
    // kemungkinan hydration mismatch: server merender dengan data yang belum
    // ada, klien merender dengan data yang sudah ada, dan kedua pohon DOM itu
    // tak lagi cocok.
    //
    // `belum_kini` dibaca dari PENANGAN PERISTIWA, yang berjalan jauh setelah
    // render selesai. Di sana berlangganan tak ada gunanya — tak ada yang perlu
    // dirender ulang karena sebuah klik — dan `get_untracked` menyatakan itu
    // alih-alih membuat langganan yang menggantung.
    let belum = move || notif.and_then(|n| n.get()).map(|d| d.belum_dibaca).unwrap_or(0);
    let belum_kini =
        move || notif.and_then(|n| n.get_untracked()).map(|d| d.belum_dibaca).unwrap_or(0);

    // Menandai terbaca dilakukan saat lonceng DITUTUP, bukan saat dibuka.
    //
    // Kalau ditandai saat dibuka, `refetch` yang menyusul mengembalikan semuanya
    // sebagai "sudah dibaca" dan sorotan yang membedakan mana yang baru hilang
    // tepat pada detik orang mulai membacanya — persis informasi yang ia buka
    // loncengnya untuk mencarinya. Ditunda sampai ditutup, sorotannya bertahan
    // selama panel terbuka, lalu bersih untuk kunjungan berikutnya.
    let tandai_semua = move || {
        let Some(res) = notif else { return };
        leptos::task::spawn_local(async move {
            if crate::web::api::tandai_semua_notifikasi().await.is_ok() {
                res.refetch();
            }
        });
    };

    // Ditutup dari tiga tempat — ikon lonceng, backdrop, tombol silang — dan
    // ketiganya harus ikut menandai terbaca. Disatukan supaya menambah jalan
    // keluar keempat tak diam-diam melewatkannya.
    let tutup = move || {
        open.set(false);
        if belum_kini() > 0 {
            tandai_semua();
        }
    };

    view! {
        <div class="ppm-bell">
            <button
                class="w-10 h-10 rounded-full flex items-center justify-center text-on-surface-variant hover:bg-surface-container relative"
                on:click=move |_| {
                    if open.get_untracked() {
                        tutup();
                    } else {
                        open.set(true);
                        // Isi lonceng diambil sekali saat halaman dimuat; kalau
                        // ada yang datang sesudah itu, inilah saat yang tepat
                        // untuk menyusul — orangnya memang sedang bertanya
                        // "apa yang baru?".
                        if let Some(res) = notif {
                            res.refetch();
                        }
                    }
                }
                aria-label="Notifikasi"
            >
                <span class="material-symbols-outlined">"notifications"</span>
                // Titik HANYA muncul kalau memang ada yang belum dibaca. Dulu ia
                // menyala permanen, dan penanda yang selalu menyala tak
                // memberitahu apa-apa — ia hanya melatih orang mengabaikannya.
                //
                // `fallback=|| ()` — sebelum datanya ada, TIDAK ADA titik. Itu
                // jawaban yang benar untuk "ada yang baru?" saat kita belum
                // tahu: penanda yang menyala lalu padam sendiri lebih buruk
                // daripada yang terlambat sedetik.
                <Transition fallback=|| ()>
                    {move || (belum() > 0).then(|| view! { <span class="ppm-badge pulse-dot"></span> })}
                </Transition>
            </button>
            {move || {
                open.get()
                    .then(|| {
                        view! {
                            // Backdrop transparan: klik di luar menutup popover.
                            // z tinggi (55/60) supaya popover PASTI di atas ikon/
                            // elemen lain di header (header sendiri sudah z-40).
                            <div class="fixed inset-0 z-[55]" on:click=move |_| tutup()></div>
                            // `.ppm-notif-panel` (position:fixed di CSS), BUKAN
                            // `absolute` di dalam lonceng: header memakai
                            // `backdrop-blur` yang membentuk stacking context,
                            // jadi panel yang lahir di dalamnya terkurung di
                            // level header dan tertutup elemen lain. Di ponsel
                            // ia juga memenuhi lebar layar alih-alih jadi
                            // dropdown 18rem yang sesak.
                            <div class="ppm-notif-panel">
                                <div class="flex items-center justify-between mb-2">
                                    <p class="text-body-md font-bold text-on-background">"Notifikasi"</p>
                                    <button
                                        class="w-7 h-7 rounded-full flex items-center justify-center text-on-surface-variant hover:bg-surface-container"
                                        on:click=move |_| tutup()
                                    >
                                        <span class="material-symbols-outlined text-lg">"close"</span>
                                    </button>
                                </div>
                                // Isi daftar dibaca DI DALAM Transition — lihat
                                // catatan di `belum` di atas. Kerangkanya (judul
                                // + tombol tutup) sengaja di LUAR: ia tak
                                // bergantung pada data, dan menaruhnya di dalam
                                // berarti panel yang baru dibuka sempat tampil
                                // tanpa cara menutupnya.
                                <Transition fallback=|| {
                                    view! {
                                        <p class="py-6 text-center text-body-sm text-on-surface-variant">
                                            "Memuat…"
                                        </p>
                                    }
                                }>
                                    {move || {
                                        let items = notif
                                            .and_then(|n| n.get())
                                            .map(|d| d.items)
                                            .unwrap_or_default();
                                        if items.is_empty() {
                                            view! {
                                                <div class="py-6 text-center text-on-surface-variant">
                                                    <span class="material-symbols-outlined text-4xl opacity-60">
                                                        "notifications_off"
                                                    </span>
                                                    <p class="text-body-sm mt-2">"Belum ada notifikasi baru."</p>
                                                </div>
                                            }
                                                .into_any()
                                        } else {
                                            view! {
                                                <div class="ppm-notif-list">
                                                    {items
                                                        .into_iter()
                                                        .map(|n| {
                                                            let (ikon, warna) = ikon_notif(&n.kind);
                                                            // Tautan kosong tetap dirender sebagai <a> tanpa href
                                                            // supaya susunannya tak berubah antar-jenis.
                                                            let href = (!n.link.is_empty()).then_some(n.link.clone());
                                                            view! {
                                                                <a
                                                                    href=href
                                                                    class="ppm-notif-item"
                                                                    class:ppm-notif-item--baru=!n.dibaca
                                                                >
                                                                    <span class=format!(
                                                                        "material-symbols-outlined {warna}",
                                                                    )>{ikon}</span>
                                                                    <span class="min-w-0">
                                                                        <span class="ppm-notif-judul">{n.title}</span>
                                                                        // `white-space: pre-line` di CSS —
                                                                        // body-nya memang ditulis multi-baris
                                                                        // (nama, rentang, pengaju).
                                                                        <span class="ppm-notif-isi">{n.body}</span>
                                                                        <span class="ppm-notif-waktu">{n.waktu_label}</span>
                                                                    </span>
                                                                </a>
                                                            }
                                                        })
                                                        .collect_view()}
                                                </div>
                                            }
                                                .into_any()
                                        }
                                    }}
                                </Transition>
                            </div>
                        }
                    })
            }}
        </div>
    }
}

#[cfg(test)]
mod tests_notif {
    use super::*;
    use crate::models::notifikasi::jenis;

    /// Tiap jenis punya ikon & warnanya sendiri — kalau dua jenis berbagi ikon,
    /// daftar notifikasi berhenti bisa dipindai sekilas.
    #[test]
    fn tiap_jenis_punya_ikon_sendiri() {
        assert_eq!(ikon_notif(jenis::IZIN_DISETUJUI), ("check_circle", "text-tertiary"));
        assert_eq!(ikon_notif(jenis::IZIN_DITOLAK), ("cancel", "text-error"));
        assert_eq!(ikon_notif(jenis::IZIN_BARU), ("mail", "text-primary"));
    }

    /// Jenis yang belum dikenal — notifikasi lama sesudah jenis baru
    /// ditambahkan, atau sebaliknya — tetap dapat ikon yang masuk akal, bukan
    /// baris tanpa ikon sama sekali.
    #[test]
    fn jenis_tak_dikenal_jatuh_ke_ikon_bawaan() {
        assert_eq!(ikon_notif("jenis_masa_depan"), ("mail", "text-primary"));
        assert_eq!(ikon_notif(""), ("mail", "text-primary"));
    }

    /// Nama ikon HARUS berupa token Material Symbols yang bisa ditemukan
    /// `scripts/fetch-icons.sh` — huruf kecil, angka, garis bawah. Nama dengan
    /// spasi atau huruf besar tak pernah masuk subset, dan ikonnya tampil
    /// sebagai TULISAN di tengah daftar.
    #[test]
    fn nama_ikon_berbentuk_token_yang_terpindai() {
        for kind in [jenis::IZIN_BARU, jenis::IZIN_DISETUJUI, jenis::IZIN_DITOLAK, "lain"] {
            let (ikon, _) = ikon_notif(kind);
            assert!(!ikon.is_empty(), "ikon kosong untuk {kind}");
            assert!(
                ikon.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "`{ikon}` bukan token ikon yang sah — lihat scripts/fetch-icons.sh"
            );
        }
    }

    /// Pemetaan ikon HARUS hidup di `src/web`, karena hanya direktori itu yang
    /// dipindai `scripts/fetch-icons.sh` (lihat catatan di `ikon_notif`).
    /// Tes ini ada di berkas yang sama dengan fungsinya — memindahkan fungsinya
    /// keluar dari `src/web` akan membawa tes ini ikut keluar, dan itu satu-
    /// satunya cara ia bisa gagal. Yang dijaga di sini: nama ikonnya benar-benar
    /// muncul sebagai literal di berkas ini, sehingga ekstraktor menemukannya.
    #[test]
    fn nama_ikon_ada_sebagai_literal_di_berkas_ini() {
        let sumber = include_str!("components.rs");
        for kind in [jenis::IZIN_BARU, jenis::IZIN_DISETUJUI, jenis::IZIN_DITOLAK] {
            let (ikon, _) = ikon_notif(kind);
            assert!(
                sumber.contains(&format!("\"{ikon}\"")),
                "`{ikon}` tak muncul sebagai literal di src/web/components.rs — \
                 scripts/fetch-icons.sh tak akan menemukannya dan ikonnya jadi TULISAN"
            );
        }
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
        "/dewan-guru", "/poin", "/poin-saya", "/verifikasi-tahap-2", "/students", "/kelas", "/orang-tua", "/kontrol-pengguna",
        "/akademik", "/kalender", "/izin-staf", "/izin-aktif", "/materi", "/rekap-mingguan",
        "/galeri", "/tagihan", "/tagihan-saya", "/kelola-artikel", "/manajemen-user",
        "/status-server",
        // Beranda peran PENJAGA. Sempat terlewat: tanpa prefix ini, satu-satunya
        // halamannya tampil TANPA navbar sama sekali (dan tanpa kanvas lebar di
        // desktop) — lihat catatan `nav_visible` di atas.
        "/tamu-masuk",
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
                    class="ppm-bottomnav md:hidden fixed bottom-0 inset-x-0 max-w-md mx-auto bg-surface-container-lowest border-t border-outline-variant/60 z-20"
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
            ke_login();
        });
    };
    view! {
        <Transition fallback=|| ()>
        {move || {
            let user = session.and_then(|s| s.get()).flatten();
            let (role, name) = user.map(|u| (u.role, u.name)).unwrap_or_default();
            let has_role = !role.is_empty();
            // Lihat catatan di `MobileHeader`: satu sumber, bukan salinan.
            let role_label = if has_role { crate::models::role_label(&role) } else { "" };
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
                        <div class="w-10 h-10 rounded-xl bg-white flex items-center justify-center overflow-hidden">
                            <img src="/icons/logo.png" alt="" class="w-full h-full object-contain p-1" />
                        </div>
                        <div class="leading-tight min-w-0">
                            <p class="font-bold text-body-lg">"AFM SMART"</p>
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
// admin / guru / dewan-guru memakai item YANG SAMA (Beranda · Students ·
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


/// Navbar PENJAGA — hanya dua tujuan.
///
/// Penjaga gerbang tak berkepentingan dengan santri, kelas, atau laporan; menu
/// yang menawarkannya cuma memperbesar permukaan salah tekan di pos jaga. Yang
/// ia butuhkan: daftar tamu yang perlu diperiksa, dan pintu keluar akunnya.
pub const NAV_PENJAGA: &[NavDef] = &[
    NavDef { icon: "how_to_reg", label: "Tinjau Tamu", href: "/tamu-masuk" },
    NavDef { icon: "person", label: "Profil", href: "/profil" },
];

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
///   • STAF (admin/guru/dewan) → item SAMA (Beranda·Students·Kelas·Sesi·
///     Laporan); hanya tujuan "Beranda" beda per peran.
pub fn nav_for(role: &str) -> &'static [NavDef] {
    match role {
        "parent" => NAV_ORTU,
        "teacher" => NAV_DEWAN, // 'teacher' digabung ke dewan_guru (migrasi 36)
        "dewan_guru" => NAV_DEWAN,
        "admin" | "ketua" => NAV_STAF, // ketua = admin + finance
        "penjaga" => NAV_PENJAGA,
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

/// Bingkai satu media galeri — `<img>` untuk foto, `<video>` untuk video
/// (migrasi 69).
///
/// Dipisah dari [`PhotoFrame`] alih-alih menambah satu flag ke dalamnya karena
/// yang dibutuhkan video bukan cuma tag yang berbeda: ia butuh `muted` +
/// `playsinline` (tanpa keduanya iOS/Android menolak memutar otomatis dan
/// justru membuka pemutar layar penuh), `loop`, dan `poster` yang tak punya
/// padanan pada gambar. `PhotoFrame` tetap dipakai apa adanya di jalur yang
/// memang cuma menampilkan foto.
///
/// `object-fit`/`object-position` di `style` berlaku untuk `<video>` persis
/// seperti untuk `<img>`, jadi bidikan tersimpan (migrasi 54 & 55) ikut
/// menentukan bagian video yang tampil.
#[component]
pub fn MediaFrame(
    #[prop(into)] src: String,
    #[prop(into)] style: String,
    /// `true` → render `<video>`.
    video: bool,
    /// Tampilkan latar buram di belakang media (mode `contain`).
    backdrop: bool,
    #[prop(into, optional)] alt: String,
    /// Kelas bingkai luar — di sinilah rasio ditentukan, mis. `aspect-[3/4]`.
    #[prop(into, optional)] class: String,
    #[prop(optional)] lazy: bool,
    /// Video berjalan sendiri, membisu & berulang (kepala halaman depan).
    /// Tanpa ini video tampil dengan kontrol pemutar biasa.
    #[prop(optional)] ambient: bool,
) -> impl IntoView {
    let frame_class = format!("relative overflow-hidden {class}");
    view! {
        <div class=frame_class>
            {(backdrop && !video)
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
            {if video {
                view! {
                    <video
                        src=src.clone()
                        style=style.clone()
                        class="relative"
                        autoplay=ambient
                        // Atribut DAN properti: atributnya yang membuat HTML
                        // SSR sudah membisu saat pertama dirender, propertinya
                        // yang bertahan setelah hidrasi. Video yang tak
                        // membisu ditolak putar-otomatis oleh semua browser
                        // seluler — kepala halaman akan tampil sebagai kotak
                        // hitam diam.
                        muted=ambient
                        prop:muted=ambient
                        r#loop=ambient
                        controls=!ambient
                        playsinline="playsinline"
                        preload=if lazy { "metadata" } else { "auto" }
                        aria-label=alt.clone()
                    ></video>
                }
                    .into_any()
            } else {
                view! {
                    <img
                        src=src.clone()
                        style=style.clone()
                        alt=alt.clone()
                        loading=if lazy { "lazy" } else { "eager" }
                        class="relative"
                    />
                }
                    .into_any()
            }}
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
            // KEPALA — tinggal diam saat isi bergulir. Sebelumnya judul,
            // tombol tutup, dan isi berada dalam satu kotak yang bergulir
            // seluruhnya: begitu isinya panjang, judulnya hilang tergulir dan
            // padding panel ikut terbawa, sehingga isi menempel ke tepi bingkai.
            <div class="ppm-sheet-head">
                <div class="ppm-sheet-grip w-10 h-1.5 bg-outline-variant rounded-full mx-auto mb-4"></div>
                {if center_title {
                    view! {
                        <h3 class="text-headline-sm text-on-background text-center">{title}</h3>
                    }
                        .into_any()
                } else {
                    view! {
                        <div class="flex items-center justify-between gap-3">
                            <h3 class="text-headline-sm text-on-background min-w-0">{title}</h3>
                            <button
                                class="w-8 h-8 shrink-0 rounded-full flex items-center justify-center text-on-surface-variant hover:bg-surface-container cursor-pointer"
                                on:click=move |_| on_close()
                                aria-label="Tutup"
                            >
                                <span class="material-symbols-outlined text-lg">"close"</span>
                            </button>
                        </div>
                    }
                        .into_any()
                }}
            </div>
            // BADAN — satu-satunya yang bergulir.
            <div class="ppm-sheet-body">{children()}</div>
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
                ke_login();
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
                None => ke_login(),
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
/// Dipakai beranda guru/dewan guru (`analisis`) supaya tak ada salinan markup
/// kedua.
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
    /// Siapa yang sebenarnya boleh — default "admin atau ketua". Jadwal dan
    /// anggota kelas juga terbuka untuk wali kelas, dan menyebut "hanya
    /// admin" di sana membuat wali mengira aplikasinya rusak.
    #[prop(into, default = String::from("admin atau ketua"))]
    siapa: String,
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
                <p class="text-body-sm font-semibold text-on-background">"Terkunci"</p>
                <p class="text-[11px] text-on-surface-variant">
                    {format!("Hanya {siapa} yang boleh {apa}. Hubungi pengelola bila perlu diubah.")}
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
// ── Warna status kehadiran ───────────────────────────────────────────────────
//
// Pemetaan status → warna dulu disalin di TUJUH halaman (riwayat, ortu_riwayat,
// dashboard_santri, sesi_detail, staf, laporan) dengan empat
// gaya pembungkus berbeda. Warnanya kebetulan sama di semuanya — tapi
// "kebetulan" itulah masalahnya: tak ada yang menjamin salinan kedelapan ikut
// sama, dan tiap kali istilah atau paletnya berubah, tujuh berkas harus diedit
// serentak tanpa ada yang mengingatkan bila satu terlewat.
//
// Yang BENAR-BENAR berbeda antar halaman cuma UKURAN pembungkusnya, jadi itulah
// yang dipisah: warnanya di sini, ukurannya tetap milik pemanggil.

/// Kelas latar + teks untuk satu status kehadiran. Tanpa ukuran, tanpa bentuk.
///
/// Status tak dikenal jatuh ke cabang `_` yang NETRAL, bukan hijau: pernah
/// terjadi status di luar daftar tampil bercentang hijau seperti hadir penuh.
pub fn warna_kehadiran(kind: &str) -> &'static str {
    match kind {
        "present" => "bg-success/10 text-success",
        "late" => "bg-warning/10 text-warning",
        "permit" | "sick" => "bg-info/10 text-info",
        "absent" => "bg-error-container text-error",
        _ => "bg-surface-container-highest text-on-surface-variant",
    }
}

/// Chip ukuran biasa untuk satu status kehadiran.
///
/// Mengembalikan `&'static str`, bukan `String` hasil `format!`: fungsi ini
/// dipanggil sekali per baris pada daftar yang bisa berisi ratusan santri, dan
/// satu alokasi per baris per render adalah biaya yang tak perlu ada.
pub fn chip_kehadiran(kind: &str) -> &'static str {
    match kind {
        "present" => "ppm-chip bg-success/10 text-success",
        "late" => "ppm-chip bg-warning/10 text-warning",
        "permit" | "sick" => "ppm-chip bg-info/10 text-info",
        "absent" => "ppm-chip bg-error-container text-error",
        _ => "ppm-chip bg-surface-container-highest text-on-surface-variant",
    }
}

/// Chip ukuran kecil — dipakai daftar padat (verifikasi sesi).
pub fn chip_kehadiran_sm(kind: &str) -> &'static str {
    match kind {
        "present" => "ppm-chip-sm bg-success/10 text-success",
        "late" => "ppm-chip-sm bg-warning/10 text-warning",
        "permit" | "sick" => "ppm-chip-sm bg-info/10 text-info",
        "absent" => "ppm-chip-sm bg-error-container text-error",
        _ => "ppm-chip-sm bg-surface-container-highest text-on-surface-variant",
    }
}

/// Kelas garis aksen kiri kartu riwayat.
///
/// Kelas palet (`style/tailwind.css`), bukan `style="border-left:…#hex"`:
/// warnanya milik tema, bukan angka yang disalin per halaman.
pub fn aksen_kehadiran(kind: &str) -> &'static str {
    match kind {
        "present" => "ppm-accent-success",
        "late" => "ppm-accent-warning",
        "permit" | "sick" => "ppm-accent-info",
        "absent" => "ppm-accent-error",
        _ => "ppm-accent-info",
    }
}

// CATATAN untuk yang menambahkan pemetaan IKON di sini kelak: subset font hanya
// memuat ikon yang terpungut `scripts/fetch-icons.sh`, dan skripnya harus
// dijalankan ulang sesudahnya — kalau tidak, ikonnya tampil sebagai TEKS. Satu
// pemetaan ikon bersama sempat ditulis di sini lalu dibuang: nilainya berbeda
// dari ikon yang sudah dipakai dashboard santri, jadi menyatukannya berarti
// mengubah tampilan halaman yang sudah benar sekaligus memaksa font tumbuh —
// dua hal yang tak diminta siapa pun.

///
/// `ServerFnError` merangkai konteksnya dengan ": " (mis.
/// "error running server function: Poin telat wajib diisi"). Yang berguna cuma
/// potongan terakhir. Pola `e.to_string().rsplit(": ").next()...` ini dulu
/// disalin di 37 tempat — satu salinan yang lupa diperbaiki cukup untuk
/// menampilkan jargon internal ke layar santri.
pub fn pesan_galat(e: impl ToString) -> String {
    let s = e.to_string();
    let inti = s.rsplit(": ").next().unwrap_or(&s).trim();
    // KODE, bukan kalimat. `require_roles` dan `require_session` menolak dengan
    // penanda sependek "forbidden" — dimaksudkan supaya klien bisa MEMBEDAKAN
    // jenis penolakan, bukan untuk dibaca orang. Tanpa penerjemahan ini, kata
    // "forbidden" muncul apa adanya di tengah layar berbahasa Indonesia: tak
    // memberi tahu apa yang salah, dan tak memberi tahu apa yang harus
    // dilakukan.
    match inti {
        "forbidden" => "Peran Anda tidak berwenang melakukan tindakan ini. Bila menurut Anda \
                        seharusnya boleh, hubungi admin — mungkin penugasan kelasnya belum \
                        diatur."
            .to_string(),
        "unauth" => "Anda belum masuk. Silakan masuk lebih dulu.".to_string(),
        "session_expired" => "Sesi Anda berakhir. Silakan masuk kembali.".to_string(),
        lain => lain.to_string(),
    }
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
                ke_login();
            }
        }
    });
}

/// Kunci sessionStorage: "sesi di tab ini sudah berakhir".
///
/// Per-TAB, bukan per-peramban (`localStorage`), karena yang dijawabnya juga
/// per-tab: tumpukan riwayat mana yang isinya sudah tak boleh diperlihatkan
/// lagi. Dua tab dengan akun berbeda tak saling mengganggu.
#[cfg(target_arch = "wasm32")]
const TANDA_KELUAR: &str = "ppm-keluar";

/// Alihkan ke `/login` karena sesi berakhir — keluar atas kemauan sendiri
/// maupun token yang sudah tak berlaku.
///
/// Menandai tab ini "sesi berakhir" SEBELUM berpindah. Skrip di `web::app`
/// membaca tanda itu saat `pageshow` dan hanya memuat ulang halaman yang
/// dipulihkan bfcache bila tandanya ada — lihat catatan panjang di sana untuk
/// alasannya (pemulihan bfcache mengembalikan WASM yang SUDAH terhidrasi;
/// memuat ulang tanpa perlu justru memulangkan pengguna ke jendela "terlihat
/// tapi belum bisa diklik").
///
/// Tandanya TIDAK dihapus saat dipakai, melainkan saat ada yang berhasil masuk
/// lagi ([`masuk_ke`]). Menekan Back berkali-kali bisa melewati beberapa
/// halaman lama sekaligus, dan semuanya sama-sama tak boleh dipulihkan apa
/// adanya — tanda yang habis sesudah pemakaian pertama hanya melindungi
/// halaman yang pertama muncul.
pub fn ke_login() {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(s) = web_sys::window().and_then(|w| w.session_storage().ok().flatten()) {
            let _ = s.set_item(TANDA_KELUAR, "1");
        }
        if let Some(w) = web_sys::window() {
            let _ = w.location().replace("/login");
        }
    }
}

/// Masuk ke halaman `path` sesudah login/pendaftaran berhasil.
///
/// Mencabut tanda yang dipasang [`ke_login`]: sejak titik ini ada sesi yang
/// sah lagi di tab ini, dan halaman-halaman baru yang dibuka sesudahnya boleh
/// dipulihkan bfcache seperti biasa.
pub fn masuk_ke(path: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(s) = web_sys::window().and_then(|w| w.session_storage().ok().flatten()) {
            let _ = s.remove_item(TANDA_KELUAR);
        }
        if let Some(w) = web_sys::window() {
            let _ = w.location().replace(path);
        }
    }
    let _ = path;
}

/// Kotak hasil aksi: hijau bila berhasil, merah bila gagal.
///
/// Menggantikan blok `msg.get().map(|(ok, t)| { let cls = if ok {…} })` yang
/// disalin 25× di 13 halaman — semuanya dengan kelas Tailwind yang sama persis,
/// diketik ulang setiap kali.
///
/// `role="alert"` dipasang di SINI, bukan diserahkan ke pemanggil. Dari 25
/// salinan itu hanya satu yang memasangnya, jadi 24 perubahan status lain tak
/// pernah diumumkan ke pembaca layar — dan itulah harga sebenarnya dari
/// menyalin markup: perbaikan aksesibilitas berhenti di satu salinan.
#[component]
pub fn FlashMsg(pesan: RwSignal<Option<(bool, String)>>) -> impl IntoView {
    view! {
        {move || {
            pesan
                .get()
                .map(|(ok, t)| {
                    let cls = if ok {
                        "p-2.5 bg-secondary-container text-on-secondary-container rounded-lg text-body-sm"
                    } else {
                        "p-2.5 bg-error-container text-on-error-container rounded-lg text-body-sm"
                    };
                    view! {
                        <div class=cls role="alert">
                            {t}
                        </div>
                    }
                })
        }}
    }
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

/// Daftar kartu: satu kolom di ponsel, dua kolom di layar lebar.
///
/// Dulu ini memakai multi-kolom CSS (`.ppm-masonry`) agar kartu mengalir tanpa
/// baris — kartu pendek langsung menempel di bawah kartu pendek lain, tanpa
/// lubang menunggu baris berikutnya. Idenya benar, akibatnya tidak: kolom
/// kanan pada multi-kolom adalah PARUH BAWAH daftar, jadi tiap kali daftarnya
/// bertambah (refetch, gulir-tak-berujung) peramban menyeimbangkan ulang
/// keduanya — kartu meloncat antar kolom dan sisi kanan berkedip hilang-muncul.
/// Dan bila isinya cuma SATU kartu, separuh kanan tetap kosong melompong.
///
/// Sekarang `.ppm-card-grid`: tiap kartu punya selnya sendiri (tak pernah
/// dipecah, tak pernah dihitung ulang saat daftar tumbuh), dan kolom yang tak
/// terpakai diruntuhkan `auto-fit` sehingga satu kartu melebar penuh. Lubang
/// antar-baris memang kembali — itu harga yang jauh lebih murah daripada
/// konten yang lenyap.
///
/// KARTU TIDAK DIPECAH KE DUA PEMBUNGKUS. Versi lampau membagi genap ke kiri
/// dan ganjil ke kanan lalu membetulkan tampilannya dengan `order` — dan
/// `order` hanya memindahkan yang TERLIHAT. Pembaca layar serta urutan Tab
/// mengikuti DOM, jadi keduanya membacakan 1,3,5,2,4 sementara mata melihat
/// 1,2,3,4,5. Di sini urutan DOM = urutan baca = urutan tampil.
pub fn kartu_grid(kartu: Vec<AnyView>) -> impl IntoView {
    view! { <div class="ppm-card-grid">{kartu}</div> }
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

/// Area yang bisa DIGESER kiri/kanan untuk berpindah (bulan, halaman, tab).
///
/// Memakai Pointer Events, bukan Touch Events: satu jalur kode melayani jari di
/// ponsel DAN seret tetikus/trackpad di desktop. Touch Events hanya ada di
/// perangkat sentuh, sehingga versi touch-only membuat gestur yang sama tak
/// bekerja di layar besar.
///
/// Ambang 48 px + syarat "horizontal DOMINAN" (`|dx| > |dy| * 1.5`): tanpa
/// keduanya, gulir vertikal biasa yang sedikit miring akan terbaca sebagai
/// geser dan memindahkan bulan tanpa diminta. Syarat lama `|dx| > |dy|` masih
/// meloloskan gerakan diagonal 46°, yang di ponsel adalah gulir biasa dengan
/// ibu jari. `touch-action:pan-y` (lihat `.ppm-swipe-area` di tailwind.css)
/// membiarkan gulir vertikal tetap milik browser, sementara sumbu X jadi milik
/// kita.
///
/// Gestur yang DIMULAI di atas elemen interaktif diabaikan. Grid kalender penuh
/// dengan `<button>` tanggal: tanpa saringan ini, menekan satu tanggal lalu
/// jari bergeser sedikit ke samping akan sekaligus memindahkan bulan, dan
/// tanggal yang barusan ditekan berpindah ke bawah jari.
#[component]
pub fn SwipeArea(
    /// Geser ke KANAN (jari bergerak ke kanan) — lazimnya "sebelumnya".
    on_prev: impl Fn() + Copy + Send + 'static,
    /// Geser ke KIRI — lazimnya "berikutnya".
    on_next: impl Fn() + Copy + Send + 'static,
    /// Kunci petunjuk sekali-tampil (lihat [`SwipeHint`]). Geseran PERTAMA yang
    /// berhasil menandai kunci ini, sehingga petunjuknya tak muncul lagi —
    /// orang yang sudah tahu tak perlu terus diberi tahu.
    #[prop(optional, into)]
    hint_key: Option<String>,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    /// Jarak minimum yang dianggap sengaja digeser, dalam piksel. Di bawah ini
    /// terlalu sensitif (bulan berganti tanpa sengaja), jauh di atasnya terasa
    /// berat.
    const AMBANG: f64 = 48.0;
    /// Seberapa jauh gerakan harus lebih mendatar daripada menegak.
    const DOMINASI: f64 = 1.5;
    /// Ambang khusus RODA/TRACKPAD. Lebih besar dari ambang jari karena satu
    /// sentakan dua jari memuntahkan puluhan event kecil yang dijumlahkan.
    /// Hanya terpakai di jalur wasm — di build server penangan rodanya kosong.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    const AMBANG_RODA: f64 = 80.0;
    /// Setelah satu perpindahan, abaikan roda selama ini (ms). Trackpad terus
    /// mengirim event "momentum" setelah jari diangkat; tanpa jeda, satu
    /// sentakan akan melompat beberapa bulan sekaligus.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    const JEDA_RODA_MS: f64 = 500.0;

    let mulai = RwSignal::new(Option::<(f64, f64)>::None);
    // Akumulator roda + waktu kunci. `StoredValue`, bukan signal: keduanya
    // keadaan gestur yang sedang berjalan, tak ada yang perlu dirender ulang
    // karenanya.
    let roda_akum = StoredValue::new(0.0f64);
    let roda_kunci = StoredValue::new(0.0f64);
    let cls = format!("ppm-swipe-area {class}");
    let hint_key = StoredValue::new(hint_key);

    // Elemen interaktif di bawah titik sentuh? `closest` menaiki pohon, jadi
    // menekan <span> di DALAM sebuah tombol pun ikut terdeteksi.
    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
    let dari_interaktif = move |ev: &leptos::ev::PointerEvent| -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            return ev
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                .and_then(|el| {
                    el.closest("button, a, input, select, textarea, [data-no-swipe]").ok().flatten()
                })
                .is_some();
        }
        #[allow(unreachable_code)]
        false
    };

    view! {
        <div
            class=cls
            on:pointerdown=move |ev: leptos::ev::PointerEvent| {
                if dari_interaktif(&ev) {
                    mulai.set(None);
                    return;
                }
                mulai.set(Some((ev.client_x() as f64, ev.client_y() as f64)));
            }
            on:pointercancel=move |_| mulai.set(None)
            // Jari/kursor terangkat DI LUAR elemen: `pointerup` tak pernah
            // sampai ke sini, dan titik awal yang tertinggal akan dipakai oleh
            // gestur berikutnya — geseran hantu dari posisi yang sudah basi.
            on:pointerleave=move |_| mulai.set(None)
            on:pointerup=move |ev: leptos::ev::PointerEvent| {
                let Some((x0, y0)) = mulai.get_untracked() else {
                    return;
                };
                mulai.set(None);
                let dx = ev.client_x() as f64 - x0;
                let dy = ev.client_y() as f64 - y0;
                if dx.abs() < AMBANG || dx.abs() <= dy.abs() * DOMINASI {
                    return;
                }
                tandai_hint_terpakai(hint_key.get_value().as_deref());
                if dx > 0.0 { on_prev() } else { on_next() }
            }
            // ── TRACKPAD / RODA MENDATAR (laptop) ────────────────────────────
            // Gulir dua jari TIDAK menghasilkan Pointer Event sama sekali — ia
            // event `wheel`. Jadi tanpa penangan ini, seluruh gestur di atas
            // tak pernah tersentuh di laptop, dan satu-satunya jalan tersisa
            // adalah tombol panah.
            //
            // `prevent_default` penting: gulir mendatar di banyak browser
            // memicu navigasi "kembali". Tanpa dicegah, menggeser bulan justru
            // melempar pengguna keluar halaman.
            on:wheel=move |ev: leptos::ev::WheelEvent| {
                #[cfg(target_arch = "wasm32")]
                {
                    let (dx, dy) = (ev.delta_x(), ev.delta_y());
                    // Gulir menegak tetap milik browser — halaman harus tetap
                    // bisa di-scroll seperti biasa di atas area ini.
                    if dx.abs() <= dy.abs() {
                        return;
                    }
                    ev.prevent_default();
                    let sekarang = js_sys::Date::now();
                    if sekarang < roda_kunci.get_value() {
                        return; // masih dalam jeda momentum
                    }
                    let akum = roda_akum.get_value() + dx;
                    roda_akum.set_value(akum);
                    if akum.abs() < AMBANG_RODA {
                        return;
                    }
                    roda_akum.set_value(0.0);
                    roda_kunci.set_value(sekarang + JEDA_RODA_MS);
                    tandai_hint_terpakai(hint_key.get_value().as_deref());
                    // Dua jari bergerak ke KIRI → deltaX positif → "berikutnya",
                    // arah yang sama dengan jari menggeser ke kiri di ponsel.
                    if akum < 0.0 { on_prev() } else { on_next() }
                }
                let _ = (&ev, &roda_akum, &roda_kunci);
            }
        >
            {children()}
        </div>
    }
}

/// Kunci localStorage petunjuk geser untuk sebuah fitur.
/// Hanya terpakai di jalur wasm — localStorage tak ada di build server.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn hint_storage_key(key: &str) -> String {
    format!("ppm-swipe-hint-{key}")
}

/// Catat bahwa pengguna sudah pernah menggeser fitur ini.
///
/// Disimpan per-FITUR, bukan satu penanda global: tahu bahwa kalender bisa
/// digeser tak berarti tahu bahwa rekap pekanan juga bisa, dan satu kunci
/// bersama akan menyembunyikan petunjuk kedua sebelum sempat terlihat.
fn tandai_hint_terpakai(key: Option<&str>) {
    #[cfg(target_arch = "wasm32")]
    if let Some(key) = key {
        if let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            let _ = s.set_item(&hint_storage_key(key), "1");
        }
    }
    let _ = key;
}

/// Petunjuk "ini bisa digeser" yang muncul SEKALI per fitur.
///
/// Gestur tak punya wujud: tak ada yang menandakan sebuah area bisa digeser
/// sampai seseorang kebetulan menggesernya. Petunjuk ini menutup jarak itu,
/// lalu menghilang selamanya setelah geseran pertama yang berhasil (ditandai
/// [`SwipeArea`] lewat `hint_key` yang sama).
///
/// Hanya di PONSEL. Di desktop yang bekerja adalah tombol panah di kepala
/// periode — menyuruh pengguna tetikus "menggeser" hanya membingungkan.
///
/// Dibaca lewat Effect (bukan saat render) supaya HTML dari server dan render
/// pertama di klien sama-sama menampilkan petunjuknya: localStorage tak ada di
/// server, dan membacanya saat render menghasilkan hydration-mismatch.
#[component]
pub fn SwipeHint(#[prop(into)] key: String, #[prop(into)] teks: String) -> impl IntoView {
    let tampil = RwSignal::new(true);
    Effect::new(move |_| {
        #[cfg(target_arch = "wasm32")]
        {
            let sudah = web_sys::window()
                .and_then(|w| w.local_storage().ok().flatten())
                .and_then(|s| s.get_item(&hint_storage_key(&key)).ok().flatten())
                .is_some();
            if sudah {
                tampil.set(false);
            }
        }
        let _ = &key;
    });
    view! {
        <Show when=move || tampil.get() fallback=|| ()>
            <p class="ppm-swipe-hint" aria-hidden="true">
                <span class="material-symbols-outlined text-base">"swipe"</span>
                {teks.clone()}
            </p>
        </Show>
    }
}

/// Panel detail satu pengajuan izin — SATU komponen untuk semua peran.
///
/// Isinya identik dari sisi mana pun dilihat; yang berbeda hanya apakah tombol
/// sunting muncul, dan itu ditentukan server (`can_edit`). Membuat panel
/// terpisah per peran berarti tiga tempat yang harus terus sepakat tentang apa
/// itu "izin ini".
#[component]
pub fn SheetIzin(
    permit_id: i64,
    /// `Sync` diwajibkan `Sheet` (lihat bound di sana): penutupnya dipasang ke
    /// listener dokumen yang bisa berjalan di luar thread render.
    on_close: impl Fn() + Copy + Send + Sync + 'static,
    /// Dipanggil setelah izin berhasil diubah, agar daftar pemanggil menyegar.
    /// Cukup `Send` — dipanggil dari Effect, bukan dari dalam pohon view.
    on_saved: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let data = Resource::new(
        move || permit_id,
        |id| async move { crate::web::api::permit_detail(id).await },
    );
    let editing = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let err = RwSignal::new(Option::<String>::None);

    // `on_saved` TIDAK dipanggil dari dalam view: children <Suspense> menuntut
    // Sync, sementara closure `refetch` milik halaman pemanggil hanya Send.
    // Jadi penyimpanan menaikkan penghitung ini, dan Effect di luar view yang
    // meneruskannya — closure-nya tak pernah ikut masuk pohon reaktif.
    let tersimpan = RwSignal::new(0u32);
    Effect::new(move |sebelumnya: Option<u32>| {
        let n = tersimpan.get();
        if sebelumnya.is_some_and(|p| p != n) {
            on_saved();
        }
        n
    });

    // Nilai form; diisi dari data saat tombol sunting ditekan.
    let f_kind = RwSignal::new(String::new());
    let f_start = RwSignal::new(String::new());
    let f_end = RwSignal::new(String::new());
    let f_jm = RwSignal::new(String::new());
    let f_js = RwSignal::new(String::new());
    let f_reason = RwSignal::new(String::new());

    let simpan = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        err.set(None);
        let a = (
            f_kind.get_untracked(),
            f_start.get_untracked(),
            f_end.get_untracked(),
            f_jm.get_untracked(),
            f_js.get_untracked(),
            f_reason.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            match crate::web::api::update_permit_action(
                permit_id, a.0, a.1, a.2, a.3, a.4, a.5,
            )
            .await
            {
                Ok(_) => {
                    editing.set(false);
                    data.refetch();
                    tersimpan.update(|n| *n += 1);
                }
                Err(e) => err.set(Some(pesan_galat(e))),
            }
            busy.set(false);
        });
    };

    let field = "w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface";

    view! {
        <Sheet title="Detail Perizinan" on_close=on_close>
            <Suspense fallback=|| view! { <Skeleton baris=2 tinggi="h-20" /> }>
                {move || {
                    let Some(res) = data.get() else { return ().into_any() };
                    let d = match res {
                        Ok(d) => d,
                        Err(e) => {
                            return view! { <FetchError err=e.to_string() /> }.into_any();
                        }
                    };
                    let badge = match d.status_kind.as_str() {
                        "approved" => "ppm-chip bg-success/10 text-success",
                        "rejected" => "ppm-chip bg-error-container text-error",
                        _ => "ppm-chip bg-warning/10 text-warning",
                    };
                    let can_edit = d.can_edit;
                    let d_form = d.clone();
                    view! {
                        // TANPA md:max-w-*: panelnya sendiri sudah dibatasi
                        // 40rem, dan membatasi isinya lagi hanya menyisakan
                        // pita kosong di kanan sementara teksnya menepi ke kiri.
                        <div class="mt-1 space-y-3.5">
                            <div class="flex items-center justify-between gap-2">
                                <div class="min-w-0">
                                    <p class="text-body-lg font-bold text-on-background truncate">
                                        {d.kind_label.clone()}
                                    </p>
                                    <p class="text-body-sm text-on-surface-variant truncate">
                                        {d.student_name.clone()}
                                    </p>
                                </div>
                                <span class=badge>{d.status_label.clone()}</span>
                            </div>

                            // Siapa yang meminta — wali kelas perlu tahu apakah
                            // ini permintaan santri sendiri atau orang tuanya.
                            <p class=if d.oleh_ortu {
                                "text-[11px] font-semibold text-info flex items-center gap-1"
                            } else {
                                "text-[11px] text-on-surface-variant flex items-center gap-1"
                            }>
                                <span class="material-symbols-outlined text-[14px]">
                                    {if d.oleh_ortu { "family_restroom" } else { "person" }}
                                </span>
                                {d.diajukan_oleh.clone()}
                            </p>

                            <div class="rounded-xl bg-surface-container px-3 py-2.5 space-y-1">
                                <p class="text-body-sm text-on-background flex items-center gap-1.5">
                                    <span class="material-symbols-outlined text-[15px]">"calendar_month"</span>
                                    {d.range_label.clone()}
                                    {(!d.jam_label.is_empty())
                                        .then(|| {
                                            view! {
                                                <span class="text-primary font-semibold">
                                                    {d.jam_label.clone()}
                                                </span>
                                            }
                                        })}
                                </p>
                                {(!d.class_label.is_empty())
                                    .then(|| {
                                        let w = if d.wali_name.is_empty() {
                                            "wali kelas belum ditunjuk".to_string()
                                        } else {
                                            d.wali_name.clone()
                                        };
                                        view! {
                                            <p class="text-[11px] text-on-surface-variant flex items-center gap-1.5">
                                                <span class="material-symbols-outlined text-[14px]">"school"</span>
                                                {format!("{} · {}", d.class_label.clone(), w)}
                                            </p>
                                        }
                                    })}
                            </div>

                            {(!d.sesi_terlewat.is_empty())
                                .then(|| {
                                    view! {
                                        <div class="rounded-xl bg-warning/5 border border-warning/30 px-3 py-2">
                                            <p class="text-[11px] font-bold text-on-background flex items-center gap-1">
                                                <span class="material-symbols-outlined text-[14px] text-warning">
                                                    "event_busy"
                                                </span>
                                                {format!("{} sesi terlewat", d.total_sesi)}
                                            </p>
                                            <p class="text-[11px] text-on-surface-variant mt-0.5">
                                                {d.sesi_terlewat.join(" · ")}
                                            </p>
                                        </div>
                                    }
                                })}

                            <p class="text-body-sm text-on-surface-variant italic">
                                {format!("\u{201C}{}\u{201D}", d.reason.clone())}
                            </p>
                            <p class="text-[10px] text-on-surface-variant/70">{d.when_label.clone()}</p>

                            {move || {
                                err.get()
                                    .map(|e| {
                                        view! {
                                            <div class="p-2.5 bg-error-container text-on-error-container rounded-xl text-body-sm">
                                                {e}
                                            </div>
                                        }
                                    })
                            }}

                            {if can_edit {
                                let dd = d_form.clone();
                                view! {
                                    <Show
                                        when=move || editing.get()
                                        fallback=move || {
                                            let dd = dd.clone();
                                            view! {
                                                <button
                                                    class="w-full py-3 rounded-xl bg-primary text-on-primary font-semibold press"
                                                    on:click=move |_| {
                                                        f_kind.set(dd.kind.clone());
                                                        f_start.set(dd.start_date.clone());
                                                        f_end.set(dd.end_date.clone());
                                                        f_jm.set(dd.jam_mulai.clone());
                                                        f_js.set(dd.jam_selesai.clone());
                                                        f_reason.set(dd.reason.clone());
                                                        editing.set(true);
                                                    }
                                                >
                                                    "Ubah Pengajuan"
                                                </button>
                                            }
                                        }
                                    >
                                        <form class="space-y-2 anim-in" method="post" on:submit=simpan>
                                            <select
                                                class=field
                                                prop:value=move || f_kind.get()
                                                on:change=move |ev| f_kind.set(event_target_value(&ev))
                                            >
                                                <option value="sick">"Izin Sakit"</option>
                                                <option value="leave">"Izin Pulang"</option>
                                                <option value="keperluan">"Keperluan"</option>
                                            </select>
                                            <div class="grid grid-cols-2 gap-2">
                                                <input
                                                    type="date"
                                                    class=field
                                                    aria-label="Mulai tanggal"
                                                    prop:value=move || f_start.get()
                                                    on:input=move |ev| f_start.set(event_target_value(&ev))
                                                    required=true
                                                />
                                                <input
                                                    type="date"
                                                    class=field
                                                    aria-label="Sampai tanggal"
                                                    prop:value=move || f_end.get()
                                                    on:input=move |ev| f_end.set(event_target_value(&ev))
                                                />
                                            </div>
                                            <div class="grid grid-cols-2 gap-2">
                                                <input
                                                    type="time"
                                                    class=field
                                                    aria-label="Jam mulai"
                                                    prop:value=move || f_jm.get()
                                                    on:input=move |ev| f_jm.set(event_target_value(&ev))
                                                />
                                                <input
                                                    type="time"
                                                    class=field
                                                    aria-label="Jam selesai"
                                                    prop:value=move || f_js.get()
                                                    on:input=move |ev| f_js.set(event_target_value(&ev))
                                                />
                                            </div>
                                            <p class="text-[11px] text-on-surface-variant">
                                                "Kosongkan jam untuk izin sehari penuh."
                                            </p>
                                            <textarea
                                                rows="3"
                                                class=format!("{field} resize-none")
                                                prop:value=move || f_reason.get()
                                                on:input=move |ev| f_reason.set(event_target_value(&ev))
                                            ></textarea>
                                            <div class="grid grid-cols-2 gap-2">
                                                <button
                                                    type="button"
                                                    class="py-2.5 rounded-xl border border-outline-variant text-on-surface font-semibold text-body-sm"
                                                    on:click=move |_| editing.set(false)
                                                >
                                                    "Batal"
                                                </button>
                                                <button
                                                    type="submit"
                                                    class="py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                                                    disabled=move || busy.get()
                                                >
                                                    {move || if busy.get() { "Menyimpan…" } else { "Simpan" }}
                                                </button>
                                            </div>
                                        </form>
                                    </Show>
                                }
                                    .into_any()
                            } else {
                                // Tombolnya TIDAK ADA, dan alasannya disebut —
                                // tombol yang hilang tanpa penjelasan terasa
                                // seperti aplikasi rusak.
                                view! {
                                    <p class="text-[11px] text-on-surface-variant flex items-start gap-1.5 rounded-xl bg-surface-container px-3 py-2">
                                        <span class="material-symbols-outlined text-[15px] shrink-0">"lock"</span>
                                        {d.lock_reason.clone()}
                                    </p>
                                }
                                    .into_any()
                            }}
                        </div>
                    }
                        .into_any()
                }}
            </Suspense>
        </Sheet>
    }
}
