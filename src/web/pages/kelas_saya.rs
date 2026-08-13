//! Halaman "Kelas Saya" — dipakai TIGA peran dengan isi yang sama:
//!   • santri            → kelas yang ia IKUTI
//!   • wali kelas (guru) → kelas yang ia PEGANG
//!   • wali kelas        → kelas yang ia PEGANG (semua kategori, migrasi 84)
//!
//! Yang berbeda hanya kelas mana yang diambil (ditentukan server dari peran di
//! sesi) dan beberapa kalimatnya. Isi kartunya identik — kurikulum, materi yang
//! sedang dibahas, daftar santri, dan wali kelasnya — karena pertanyaan yang
//! ingin dijawab ketiganya memang sama: "kelas ini sedang di mana?".
//!
//! Semuanya BACA SAJA: tak ada tombol ubah/hapus. Pengelolaan tetap di
//! /kelas/:id. id pemirsa diambil dari sesi di server, tak pernah dari URL.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{KelasSayaItem, SessionUser};
use crate::web::api::kelas_saya_data;
use crate::web::components::{
    guard_sesi, kartu_grid, DeviceFrame, EmptyState, FetchError, MobileHeader,
    Skeleton,
};
// Komponen kurikulum dipakai BERSAMA dengan tab di /kelas/:id — lihat
// `PanelKurikulum` di bawah.
use crate::web::pages::{KurikulumTab, PosisiBerjalan};

#[component]
pub fn KelasSayaPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { kelas_saya_data().await });

    // Galat sesi → /login, sama seperti halaman santri lain. `forbidden`
    // sengaja tidak ikut: itu ditampilkan FetchError, bukan diusir ke login.
    guard_sesi(data);

    view! {
        <Title text="Kelas Saya — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Kelas Saya" subtitle="Kurikulum & materi berjalan" />
                <div class="px-5 pt-5 space-y-4 stagger">
                    <Suspense fallback=|| view! { <Skeleton baris=2 tinggi="h-40" /> }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Err(e) => {
                                        view! { <FetchError err=e.to_string() /> }.into_any()
                                    }
                                    Ok(d) if d.items.is_empty() => {
                                        let (judul, sub) = if d.sebagai_staf {
                                            (
                                                "Belum ditugaskan di kelas mana pun",
                                                "Kelas akan muncul di sini setelah kamu ditunjuk sebagai wali kelas.",
                                            )
                                        } else {
                                            (
                                                "Belum terdaftar di kelas mana pun",
                                                "Hubungi pengelola bila menurutmu ini keliru.",
                                            )
                                        };
                                        view! { <EmptyState icon="school" title=judul subtitle=sub /> }
                                            .into_any()
                                    }
                                    Ok(d) => {
                                        kartu_grid(
                                                d.items
                                                    .into_iter()
                                                    .map(|k| view! { <KelasCard k=k /> }.into_any())
                                                    .collect(),
                                            )
                                            .into_any()
                                    }
                                })
                        }}
                    </Suspense>
                </div>
            </div>
        </DeviceFrame>
    }
}

/// Satu kelas: identitas + petugas + materi berjalan + kurikulum + teman.
#[component]
fn KelasCard(k: KelasSayaItem) -> impl IntoView {
    let session = use_context::<Resource<Option<SessionUser>>>();
    // Daftar santri bisa panjang; dilipat agar kartu tetap terbaca — yang
    // dicari lebih sering "materi sekarang", bukan daftar nama.
    let buka_teman = RwSignal::new(false);
    // Penyusun kurikulum — hanya untuk WALI KELAS kelas ini, dan hanya dimuat
    // saat dibuka (lihat `PanelKurikulum`).
    let buka_kurikulum = RwSignal::new(false);

    let jumlah_teman = k.members.len();
    let members = StoredValue::new(k.members);
    // Lencana kembar ("piket"/"piket") tak mungkin lagi sejak migrasi 65:
    // kategori dan jenjang kini dua himpunan terpisah, jadi tinggal dilabeli.
    let jenjang = crate::models::jenjang_label(&k.jenjang);
    let category = crate::models::kategori_label(&k.category).to_string();
    let peran = k.peran_saya.clone();
    let boleh_kelola = !peran.is_empty();
    let class_id = k.id;
    let wali = k.wali_kelas.clone();

    view! {
        <div class="ppm-card p-4 anim-in space-y-3 ppm-accent">
            // ── Identitas kelas ────────────────────────────────────────────
            <div>
                <div class="flex items-center gap-1.5 flex-wrap">
                    // Peran pemirsa di kelas ini — hanya untuk staf; santri
                    // adalah peserta, bukan petugas, jadi tak berlencana.
                    {(!peran.is_empty())
                        .then(|| view! { <span class="ppm-chip bg-primary text-on-primary">{peran.clone()}</span> })}
                    {(!jenjang.is_empty())
                        .then(|| view! { <span class="ppm-chip bg-secondary-container text-primary">{jenjang.clone()}</span> })}
                    {(!category.is_empty())
                        .then(|| view! { <span class="ppm-chip bg-surface-container-highest text-on-surface-variant">{category.clone()}</span> })}
                </div>
                <p class="text-body-lg font-bold text-on-background mt-1.5">{k.name}</p>
            </div>

            // ── Petugas ────────────────────────────────────────────────────
            // SATU jabatan saja sejak migrasi 84. Kotaknya melebar penuh —
            // dulu berdampingan dengan Pamong, dan menyisakan ruang kosong di
            // sebelahnya akan terbaca seperti data yang gagal dimuat.
            <Petugas peran="Wali Kelas" nama=wali icon="badge" />

            // ── Materi yang sedang dibahas per jadwal (BACAAN) ─────────────
            // Disembunyikan saat panel kelola dibuka: panel itu memuat daftar
            // jadwal yang SAMA, lengkap dengan tombol ubahnya. Menampilkan
            // keduanya sekaligus membuat satu kartu memajang jadwal yang sama
            // dua kali, dan yang di atas tampak seperti versi yang tak bisa
            // disunting entah kenapa.
            {(!k.schedules.is_empty())
                .then(|| {
                    let baca_saja = move || !buka_kurikulum.get();
                    view! {
                        <div class="space-y-1.5" class:hidden=move || !baca_saja()>
                            <p class="text-[10px] font-bold tracking-wide text-on-surface-variant">
                                "JADWAL & MATERI SEKARANG"
                            </p>
                            {k.schedules
                                .into_iter()
                                .map(|j| {
                                    let ada_materi = !j.current_book_title.is_empty();
                                    view! {
                                        <div class="bg-surface-container rounded-xl px-3 py-2">
                                            <div class="flex items-center justify-between gap-2">
                                                <p class="text-body-sm font-semibold text-on-background truncate">
                                                    {j.title}
                                                </p>
                                                <span class="text-[10px] text-on-surface-variant shrink-0">
                                                    {j.recurrence_label}
                                                </span>
                                            </div>
                                            <p class="text-[11px] text-on-surface-variant">{j.time_label}</p>
                                            {if ada_materi {
                                                view! {
                                                    <p class="text-[11px] text-primary font-semibold mt-0.5">
                                                        {j.current_book_title}
                                                        {(!j.current_label.is_empty())
                                                            .then(|| format!(" · {}", j.current_label))}
                                                    </p>
                                                }
                                                    .into_any()
                                            } else {
                                                view! {
                                                    <p class="text-[11px] text-on-surface-variant mt-0.5">
                                                        "Materi belum ditentukan"
                                                    </p>
                                                }
                                                    .into_any()
                                            }}
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                })}

            // ── Kurikulum kelas ────────────────────────────────────────────
            {(!k.curriculum.is_empty())
                .then(|| {
                    view! {
                        <div class="space-y-1.5">
                            <p class="text-[10px] font-bold tracking-wide text-on-surface-variant">
                                "KURIKULUM KELAS"
                            </p>
                            {k.curriculum
                                .into_iter()
                                .map(|c| {
                                    let pct = c.progress_pct;
                                    let bar = if pct >= 100 {
                                        "h-full bg-success bar-grow"
                                    } else {
                                        "h-full bg-primary bar-grow"
                                    };
                                    let badge = match c.status.as_str() {
                                        "completed" => "ppm-chip bg-success/10 text-success",
                                        "upcoming" => {
                                            "ppm-chip bg-surface-container-highest text-on-surface-variant"
                                        }
                                        _ => "ppm-chip bg-primary/10 text-primary",
                                    };
                                    view! {
                                        <div class="bg-surface-container rounded-xl px-3 py-2">
                                            <div class="flex items-center justify-between gap-2">
                                                <p class="text-body-sm font-semibold text-on-background truncate">
                                                    {c.title}
                                                </p>
                                                <span class=badge>{c.status_label}</span>
                                            </div>
                                            {(!c.range_label.is_empty())
                                                .then(|| {
                                                    view! {
                                                        <p class="text-[11px] text-on-surface-variant">{c.range_label}</p>
                                                    }
                                                })}
                                            {(!c.current_label.is_empty())
                                                .then(|| {
                                                    view! {
                                                        <p class="text-[11px] text-primary font-semibold">
                                                            "Sudah sampai " {c.current_label}
                                                        </p>
                                                    }
                                                })}
                                            <div class="flex items-center justify-between text-[10px] font-semibold mt-1.5">
                                                <span class="text-on-surface-variant">"Progres"</span>
                                                <span class="text-on-background">{format!("{pct}%")}</span>
                                            </div>
                                            <div class="h-1.5 bg-surface-container-highest rounded-full overflow-hidden mt-1">
                                                <div class=bar style=format!("width: {pct}%")></div>
                                            </div>
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                })}

            // ── Kelola kurikulum (wali kelas) ──────────────────────────────
            // Wali kelas mengurus kurikulumnya DARI SINI. Sebelumnya satu-
            // satunya jalannya /kelas/:id — halaman kelola milik admin, yang
            // harus dicari dulu di daftar seluruh kelas pondok, padahal wali
            // sudah berdiri di depan kartu kelasnya sendiri.
            {boleh_kelola
                .then(|| {
                    view! {
                        <div class="pt-1 border-t border-outline-variant/40">
                            <button
                                class="w-full flex items-center justify-between py-1.5 press"
                                on:click=move |_| buka_kurikulum.update(|b| *b = !*b)
                            >
                                <span class="text-[10px] font-bold tracking-wide text-primary flex items-center gap-1">
                                    <span class="material-symbols-outlined text-[16px]">"edit_note"</span>
                                    "KELOLA MATERI BERJALAN & KURIKULUM"
                                </span>
                                <span
                                    class="material-symbols-outlined text-on-surface-variant transition-transform text-[20px]"
                                    class:rotate-180=move || buka_kurikulum.get()
                                >
                                    "expand_more"
                                </span>
                            </button>
                            <Show when=move || buka_kurikulum.get()>
                                <PanelKurikulum class_id=class_id />
                            </Show>
                        </div>
                    }
                })}

            // ── Teman sekelas ──────────────────────────────────────────────
            <div>
                <button
                    class="w-full flex items-center justify-between py-1.5 press"
                    on:click=move |_| buka_teman.update(|b| *b = !*b)
                >
                    <span class="text-[10px] font-bold tracking-wide text-on-surface-variant">
                        {format!("SANTRI DI KELAS INI ({jumlah_teman})")}
                    </span>
                    <span
                        class="material-symbols-outlined text-on-surface-variant transition-transform text-[20px]"
                        class:rotate-180=move || buka_teman.get()
                    >
                        "expand_more"
                    </span>
                </button>
                {move || {
                    buka_teman
                        .get()
                        .then(|| {
                            // Santri yang sedang masuk ditandai supaya mudah
                            // menemukan dirinya di daftar yang panjang.
                            let saya = session.and_then(|s| s.get()).flatten().map(|u| u.id);
                            view! {
                                // Dibatasi tinggi + digulir sendiri: kelas berisi
                                // puluhan santri kalau tidak akan membuat kartunya
                                // memanjang jauh melewati tetangganya, dan grid dua
                                // kolom menyisakan lubang sebesar itu di sebelahnya.
                                <div class="space-y-1 anim-in max-h-64 overflow-y-auto pr-1">
                                    {members
                                        .get_value()
                                        .into_iter()
                                        .map(|m| {
                                            let ini_saya = saya == Some(m.id);
                                            let cls = if ini_saya {
                                                "flex items-center gap-2 px-3 py-1.5 rounded-lg bg-secondary-container"
                                            } else {
                                                "flex items-center gap-2 px-3 py-1.5 rounded-lg"
                                            };
                                            view! {
                                                <div class=cls>
                                                    <span class="w-7 h-7 rounded-full bg-surface-container-highest text-on-surface-variant flex items-center justify-center text-[11px] font-bold shrink-0">
                                                        {m.name.chars().next().unwrap_or('S').to_string().to_uppercase()}
                                                    </span>
                                                    <div class="min-w-0 flex-1">
                                                        <p class="text-body-sm text-on-background truncate">
                                                            {m.name}
                                                            {ini_saya.then(|| " (kamu)".to_string())}
                                                        </p>
                                                    </div>
                                                    {(!m.angkatan.is_empty())
                                                        .then(|| {
                                                            view! {
                                                                <span class="text-[10px] text-on-surface-variant shrink-0">
                                                                    {m.angkatan}
                                                                </span>
                                                            }
                                                        })}
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            }
                        })
                }}
            </div>
        </div>
    }
}

/// Satu petugas kelas. Kosong → "Belum ditunjuk", bukan bidang kosong yang
/// menyisakan tanya.
#[component]
fn Petugas(peran: &'static str, nama: String, icon: &'static str) -> impl IntoView {
    let kosong = nama.is_empty();
    view! {
        <div class="bg-surface-container rounded-xl px-3 py-2">
            <p class="text-[10px] text-on-surface-variant flex items-center gap-1">
                <span class="material-symbols-outlined text-[14px]">{icon}</span>
                {peran}
            </p>
            {if kosong {
                view! {
                    <p class="text-body-sm text-on-surface-variant italic">"Belum ditunjuk"</p>
                }
                    .into_any()
            } else {
                view! {
                    <p class="text-body-sm font-semibold text-on-background truncate">{nama}</p>
                }
                    .into_any()
            }}
        </div>
    }
}

/// Penyusun kurikulum satu kelas, dipasang di dalam kartu "Kelas Saya".
///
/// Isinya BUKAN salinan: yang dirender komponen yang sama persis dengan tab
/// Kurikulum di `/kelas/:id` ([`KurikulumTab`]) — tambah materi, geser posisi
/// berjalan, dan peta "santri paling banyak kurang di ayat/halaman berapa".
/// Menyalin formnya ke sini berarti dua salinan yang harus diedit bersamaan
/// tiap kali aturan kurikulum berubah, dan yang kedua pasti tertinggal.
///
/// Payload detail kelas diambil SAAT PANELNYA DIBUKA, bukan ikut dimuat
/// bersama halaman: "Kelas Saya" bisa berisi banyak kartu, dan menarik detail
/// lengkap tiap kelas hanya untuk panel yang mungkin tak pernah disentuh
/// membuat halaman ini lambat bagi semua orang — termasuk santri, yang tak
/// punya tombol ini sama sekali.
///
/// Kewenangan tetap diperiksa SERVER (`require_petugas_kelas` pada tiap aksi
/// kurikulum); tombolnya hanya disembunyikan dari yang bukan wali.
#[component]
fn PanelKurikulum(class_id: i64) -> impl IntoView {
    let detail = Resource::new(
        move || class_id,
        |id| async move { crate::web::api::kelas_detail(id).await },
    );

    view! {
        <div class="pt-2 anim-in">
            <Suspense fallback=|| view! { <Skeleton baris=2 tinggi="h-24" /> }>
                {move || {
                    detail
                        .get()
                        .map(|res| match res {
                            Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                            Ok(d) => {
                                // MATERI BERJALAN itu milik JADWAL, bukan
                                // kurikulum: satu kelas bisa punya beberapa
                                // jadwal (KBM subuh, pesantren kilat) yang
                                // masing-masing sedang di halaman berbeda.
                                // Posisi di kartu kurikulum adalah kemajuan
                                // KELAS atas materi itu secara keseluruhan —
                                // makna yang berbeda, dan keduanya memang
                                // disunting di tempat yang berbeda.
                                //
                                // Materi jadwal disaring ke kurikulum kelas —
                                // aturan yang sama dengan tab Jadwal; jadwal
                                // mengajarkan apa yang direncanakan kelasnya.
                                let dalam_kurikulum: std::collections::HashSet<i64> = d
                                    .curriculum
                                    .iter()
                                    .map(|c| c.book_id)
                                    .filter(|b| *b > 0)
                                    .collect();
                                let buku_jadwal = StoredValue::new(
                                    d.book_options
                                        .iter()
                                        .filter(|b| dalam_kurikulum.contains(&b.id))
                                        .cloned()
                                        .collect::<Vec<_>>(),
                                );
                                // Saringan SAMA dengan daftar bacaan di atas:
                                // `class_schedules` menyimpan seluruh riwayat
                                // jadwal kelas, termasuk yang masanya sudah
                                // habis. Menawarkan "ubah materi berjalan" pada
                                // jadwal yang sudah berakhir hanya mengundang
                                // wali menggeser posisi yang tak lagi dibaca
                                // siapa pun.
                                let hari_ini = js_hari_ini();
                                let jadwal: Vec<crate::models::ScheduleItem> = d
                                    .schedules
                                    .iter()
                                    .filter(|s| {
                                        if !s.end_date.is_empty() {
                                            return s.end_date.as_str() >= hari_ini.as_str();
                                        }
                                        if s.recurrence == "custom" {
                                            return s
                                                .custom_dates
                                                .split(',')
                                                .map(|d| d.trim())
                                                .filter(|d| !d.is_empty())
                                                .max()
                                                .is_none_or(|terakhir| terakhir >= hari_ini.as_str());
                                        }
                                        true
                                    })
                                    .cloned()
                                    .collect();
                                view! {
                                    {(!jadwal.is_empty())
                                        .then(|| {
                                            view! {
                                                <div class="space-y-2 mb-4">
                                                    {jadwal
                                                        .into_iter()
                                                        .map(|s| {
                                                            let sid = s.id;
                                                            let judul = s.title.clone();
                                                            let jam = s.time_label.clone();
                                                            let ulang = s.recurrence_label.clone();
                                                            view! {
                                                                <div class="bg-surface-container-low rounded-xl px-3 py-2">
                                                                    <div class="flex items-center justify-between gap-2">
                                                                        <p class="text-body-sm font-semibold text-on-background truncate">
                                                                            {judul}
                                                                        </p>
                                                                        <span class="text-[10px] text-on-surface-variant shrink-0">
                                                                            {ulang}
                                                                        </span>
                                                                    </div>
                                                                    <p class="text-[11px] text-on-surface-variant">{jam}</p>
                                                                    <PosisiBerjalan
                                                                        class_id=class_id
                                                                        schedule_id=sid
                                                                        s=s
                                                                        books=buku_jadwal
                                                                        refetch=move || detail.refetch()
                                                                    />
                                                                </div>
                                                            }
                                                        })
                                                        .collect_view()}
                                                </div>
                                            }
                                        })}
                                    <KurikulumTab
                                        class_id=class_id
                                        d=d
                                        refetch=move || detail.refetch()
                                    />
                                }
                                    .into_any()
                            }
                        })
                }}
            </Suspense>
        </div>
    }
}

/// Tanggal hari ini (WIB) sebagai "YYYY-MM-DD".
///
/// Dibandingkan sebagai TEKS, bukan tanggal ter-parse: `ScheduleItem` memang
/// membawa tanggalnya dalam bentuk ISO, dan format itu sudah urut secara
/// leksikografis — "2026-08-13" < "2026-09-01" persis seperti tanggalnya.
/// Menariknya kembali jadi `NaiveDate` hanya untuk membandingkan berarti
/// memindahkan chrono ke bundel WASM tanpa menambah satu pun kepastian.
///
/// WIB, bukan waktu perangkat: santri yang ponselnya masih di zona lain tak
/// boleh melihat jadwal yang berbeda dari temannya. Selisihnya cuma penting
/// beberapa jam sehari, tapi justru jam-jam itulah kelas subuh dijadwalkan.
fn js_hari_ini() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let ms = js_sys::Date::now() + 7.0 * 3_600_000.0;
        let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms));
        return format!(
            "{:04}-{:02}-{:02}",
            d.get_utc_full_year(),
            d.get_utc_month() + 1,
            d.get_utc_date()
        );
    }
    // Di render server tanggalnya tak dipakai menyaring apa pun yang terlihat
    // (panel ini baru terbuka setelah diklik, jadi selalu di klien).
    #[cfg(not(target_arch = "wasm32"))]
    String::new()
}
