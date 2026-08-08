//! web/pages/manajemen_user.rs — Manajemen User (/manajemen-user, admin & ketua).
//!
//! KENAPA HALAMAN INI ADA. Seluruh aplikasi menyaring `is_active = TRUE` —
//! pemilih peserta kelas, papan poin, auto-absen, statistik. Itu benar: alumni
//! tak boleh ikut dihitung, apalagi ditandai alpa tiap hari. Tapi akibatnya
//! user nonaktif tak terlihat di mana pun, dan 512 santri dari daftar induk
//! (migrasi 74) masuk sebagai nonaktif. Tanpa halaman ini, satu-satunya cara
//! mengaktifkan mereka adalah menjalankan SQL langsung ke produksi.
//!
//! BEDANYA DENGAN /kontrol-pengguna: yang itu admin-only dan mencampur banyak
//! urusan (perangkat RFID, undangan, log aktivitas). Yang ini khusus tentang
//! ORANG — cari, sunting profil, aktifkan kembali — dan terbuka untuk KETUA,
//! karena dialah yang tahu siapa yang masih mondok.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{ManagedUser, ProfilEdit};
use crate::web::api::{
    angkatan_tersedia_data, change_user_role_action, managed_users_data, set_user_active_action,
    update_managed_user_action,
};
use crate::web::components::{DeviceFrame, EmptyState, FetchError, MobileHeader, Sheet};

const STATUS: &[(&str, &str)] = &[
    ("nonaktif", "Nonaktif"),
    ("aktif", "Aktif"),
    ("", "Semua"),
];

const PERAN: &[(&str, &str)] = &[
    ("", "Semua peran"),
    ("santri", "Santri"),
    ("santri_finance", "Santri (Finance)"),
    ("dewan_guru", "Dewan Guru"),
    ("supervisor", "Pamong"),
    ("parent", "Orang Tua"),
    ("ketua", "Ketua"),
    ("admin", "Admin"),
];

const MUBALEGH: &[(&str, &str)] =
    &[("", "—"), ("belum", "Belum"), ("iya", "Mubaligh"), ("tugasan", "Mubaligh Tugasan")];
const PENDIDIKAN: &[(&str, &str)] = &[
    ("", "—"),
    ("belum", "Belum kuliah"),
    ("kuliah", "Sedang kuliah"),
    ("sarjana", "Sarjana"),
];

#[component]
pub fn ManajemenUserPage() -> impl IntoView {
    // Bawaan "nonaktif": itulah alasan halaman ini dibuka. Yang aktif sudah
    // terlihat di seluruh aplikasi.
    let status = RwSignal::new("nonaktif".to_string());
    let peran = RwSignal::new(String::new());
    let cari = RwSignal::new(String::new());
    // Kata kunci yang BENAR-BENAR dikirim ke server, dipisah dari yang sedang
    // diketik: tanpa itu tiap penekanan tombol menembakkan satu query ke daftar
    // ribuan baris.
    let cari_kirim = RwSignal::new(String::new());
    // 0 = semua angkatan.
    let angkatan = RwSignal::new(0_i32);

    let data = Resource::new(
        move || (status.get(), peran.get(), angkatan.get(), cari_kirim.get()),
        |(s, p, a, q)| async move { managed_users_data(s, p, a, q).await },
    );
    // Daftar angkatan dibaca SEKALI (kunci `()`), bukan ikut berubah tiap
    // penyaring digeser — isinya tak bergantung pada filter mana pun.
    let angkatan_ada = Resource::new(|| (), |_| async move { angkatan_tersedia_data().await });

    Effect::new(move |_| {
        if let Some(Err(e)) = data.get() {
            let m = e.to_string();
            if crate::web::components::is_auth_error(&m) || m.contains("forbidden") {
                #[cfg(target_arch = "wasm32")]
                if let Some(w) = web_sys::window() {
                    let _ = w.location().replace("/login");
                }
            }
        }
    });

    let msg = RwSignal::new(Option::<(bool, String)>::None);
    let busy = RwSignal::new(false);
    let editing: RwSignal<Option<ManagedUser>> = RwSignal::new(None);

    let ubah_status = move |u: ManagedUser| {
        if busy.get_untracked() {
            return;
        }
        let aktifkan = !u.is_active;
        #[cfg(target_arch = "wasm32")]
        if !aktifkan {
            let ok = web_sys::window()
                .and_then(|w| {
                    w.confirm_with_message(&format!(
                        "Nonaktifkan {}? Ia akan hilang dari kelas, papan poin, dan statistik.",
                        u.full_name
                    ))
                    .ok()
                })
                .unwrap_or(false);
            if !ok {
                return;
            }
        }
        busy.set(true);
        let id = u.id;
        leptos::task::spawn_local(async move {
            match set_user_active_action(id, aktifkan).await {
                Ok(pesan) => {
                    msg.set(Some((true, pesan)));
                    data.refetch();
                }
                Err(e) => msg.set(Some((false, e.to_string()))),
            }
            busy.set(false);
        });
    };

    view! {
        <Title text="Manajemen User — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide ppm-content">
                <MobileHeader
                    title="Manajemen User"
                    subtitle="Cari, edit, dan aktifkan kembali"
                    back_href="/staf"
                />

                <div class="px-5 pt-5 space-y-4">
                    // ── Penyaring ─────────────────────────────────────────
                    <div class="ppm-card p-4 space-y-3">
                        <div class="flex gap-1 bg-surface-container rounded-xl p-1">
                            {STATUS
                                .iter()
                                .map(|(kode, label)| {
                                    let k = *kode;
                                    view! {
                                        <button
                                            class=move || {
                                                if status.get() == k {
                                                    "flex-1 py-2 rounded-lg bg-surface text-on-background shadow-sm text-body-sm font-semibold cursor-pointer"
                                                } else {
                                                    "flex-1 py-2 rounded-lg text-on-surface-variant text-body-sm font-semibold cursor-pointer"
                                                }
                                            }
                                            aria-pressed=move || (status.get() == k).to_string()
                                            on:click=move |_| status.set(k.to_string())
                                        >
                                            {*label}
                                        </button>
                                    }
                                })
                                .collect_view()}
                        </div>

                        <div class="flex gap-2">
                            <select
                                class="flex-1 min-w-0 bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface"
                                on:change=move |ev| peran.set(event_target_value(&ev))
                            >
                                {PERAN
                                    .iter()
                                    .map(|(k, l)| view! { <option value=*k>{*l}</option> })
                                    .collect_view()}
                            </select>
                            // Angkatan: isinya dari data, bukan rentang tahun
                            // yang dikarang — daftar induk membentang 2010–2025
                            // dan terus bertambah.
                            <Suspense fallback=|| ()>
                                {move || {
                                    let tahun = angkatan_ada.get().and_then(|r| r.ok()).unwrap_or_default();
                                    view! {
                                        <select
                                            class="flex-1 min-w-0 bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface"
                                            on:change=move |ev| {
                                                angkatan
                                                    .set(event_target_value(&ev).parse::<i32>().unwrap_or(0));
                                            }
                                        >
                                            <option value="0">"Semua angkatan"</option>
                                            {tahun
                                                .into_iter()
                                                .map(|t| {
                                                    view! {
                                                        <option value=t.to_string()>
                                                            {format!("Angkatan {t}")}
                                                        </option>
                                                    }
                                                })
                                                .collect_view()}
                                        </select>
                                    }
                                }}
                            </Suspense>
                        </div>

                        // Pencarian dikirim saat Enter / tombol, bukan tiap
                        // ketikan — daftarnya ratusan baris dan tiap huruf
                        // berarti satu perjalanan ke database.
                        <div class="flex gap-2">
                            <input
                                type="search"
                                placeholder="Cari nama, NIS, atau nomor HP…"
                                class="flex-1 min-w-0 bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface"
                                prop:value=move || cari.get()
                                on:input=move |ev| cari.set(event_target_value(&ev))
                                on:keydown=move |ev| {
                                    if ev.key() == "Enter" {
                                        cari_kirim.set(cari.get_untracked());
                                    }
                                }
                            />
                            <button
                                class="px-4 py-2.5 rounded-xl bg-primary text-on-primary text-body-sm font-semibold press cursor-pointer shrink-0"
                                on:click=move |_| cari_kirim.set(cari.get_untracked())
                            >
                                "Cari"
                            </button>
                        </div>
                    </div>

                    {move || {
                        msg.get()
                            .map(|(ok, t)| {
                                let cls = if ok {
                                    "p-2.5 bg-secondary-container text-on-secondary-container rounded-lg text-body-sm"
                                } else {
                                    "p-2.5 bg-error-container text-on-error-container rounded-lg text-body-sm"
                                };
                                view! { <div class=cls>{t}</div> }
                            })
                    }}

                    <Suspense fallback=|| {
                        view! {
                            <div class="space-y-3 animate-pulse">
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                    Ok(list) if list.is_empty() => {
                                        view! {
                                            <EmptyState
                                                icon="person_search"
                                                title="Tak ada yang cocok"
                                                subtitle="Ubah penyaring status/peran, atau cari dengan kata lain."
                                            />
                                        }
                                            .into_any()
                                    }
                                    Ok(list) => {
                                        let n = list.len();
                                        view! {
                                            <p class="text-body-sm text-on-surface-variant">
                                                {if n >= 500 {
                                                    format!("Menampilkan {n} teratas — persempit dengan pencarian.")
                                                } else {
                                                    format!("{n} pengguna")
                                                }}
                                            </p>
                                            <div class="space-y-3 md:grid md:grid-cols-2 md:gap-3 md:space-y-0">
                                                {list
                                                    .into_iter()
                                                    .map(|u| {
                                                        // StoredValue, bukan `u.clone()` langsung: closure
                                                        // yang MEMINDAHKAN sebuah String tidak `Copy`, dan
                                                        // prop callback di sini menuntutnya (satu closure
                                                        // dipanggil berkali-kali). StoredValue sendiri Copy,
                                                        // isinya diambil saat dipanggil.
                                                        let simpan = StoredValue::new(u.clone());
                                                        view! {
                                                            <BarisUser
                                                                u=u
                                                                busy=busy
                                                                on_edit=move || editing.set(Some(simpan.get_value()))
                                                                on_toggle=move || ubah_status(simpan.get_value())
                                                            />
                                                        }
                                                    })
                                                    .collect_view()}
                                            </div>
                                        }
                                            .into_any()
                                    }
                                })
                        }}
                    </Suspense>
                </div>

                {move || {
                    editing
                        .get()
                        .map(|u| {
                            view! {
                                <Sheet
                                    title="Edit Profil"
                                    on_close=move || editing.set(None)
                                >
                                    <FormProfil
                                        u=u
                                        on_saved=move |pesan| {
                                            editing.set(None);
                                            data.refetch();
                                            msg.set(Some((true, pesan)));
                                        }
                                    />
                                </Sheet>
                            }
                        })
                }}
            </div>
        </DeviceFrame>
    }
}

/// Satu baris pengguna di daftar.
#[component]
fn BarisUser(
    u: ManagedUser,
    busy: RwSignal<bool>,
    on_edit: impl Fn() + Copy + Send + 'static,
    on_toggle: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let aktif = u.is_active;
    // Identitas pembeda ditampilkan berdampingan: pada daftar induk ada 90 nama
    // yang muncul lebih dari sekali, jadi nama saja tak cukup untuk memastikan
    // yang disunting orang yang benar.
    let identitas = [
        u.nis.clone().filter(|s| !s.is_empty()),
        u.entry_year.map(|t| format!("Angkatan {t}")),
        u.phone_number.clone().filter(|s| !s.is_empty()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");

    view! {
        <div class="ppm-card p-4 flex gap-3">
            <span class=if aktif {
                "w-11 h-11 ppm-tile shrink-0"
            } else {
                "w-11 h-11 rounded-xl bg-surface-container text-on-surface-variant flex items-center justify-center shrink-0"
            }>
                <span class="material-symbols-outlined">
                    {if aktif { "person" } else { "person_off" }}
                </span>
            </span>
            <div class="flex-1 min-w-0">
                <div class="flex items-start gap-2">
                    <p class="text-body-md font-bold text-on-background flex-1 min-w-0">
                        {u.full_name.clone()}
                    </p>
                    <span class=if aktif {
                        "ppm-chip-sm bg-success/10 text-success shrink-0"
                    } else {
                        "ppm-chip-sm bg-surface-container-high text-on-surface-variant shrink-0"
                    }>{if aktif { "AKTIF" } else { "NONAKTIF" }}</span>
                </div>
                <p class="text-body-sm text-on-surface-variant mt-0.5">{u.role_label.clone()}</p>
                {(!identitas.is_empty())
                    .then(|| {
                        view! {
                            <p class="text-[11px] text-on-surface-variant/80 mt-0.5 truncate">
                                {identitas.clone()}
                            </p>
                        }
                    })}
                <div class="flex flex-wrap items-center gap-2 mt-2">
                    <button
                        class="px-3 py-1.5 rounded-lg border border-outline-variant text-body-sm font-semibold text-on-surface hover:border-primary hover:text-primary transition-colors cursor-pointer"
                        on:click=move |_| on_edit()
                    >
                        "Edit"
                    </button>
                    <button
                        class=if aktif {
                            "px-3 py-1.5 rounded-lg border border-error/40 text-body-sm font-semibold text-error cursor-pointer disabled:opacity-50"
                        } else {
                            "px-3 py-1.5 rounded-lg bg-primary text-on-primary text-body-sm font-semibold press cursor-pointer disabled:opacity-50"
                        }
                        prop:disabled=move || busy.get()
                        on:click=move |_| on_toggle()
                    >
                        {if aktif { "Nonaktifkan" } else { "Aktifkan" }}
                    </button>
                    // Saldo hanya bermakna bagi santri.
                    {u.role.starts_with("santri")
                        .then(|| {
                            view! {
                                <span class="text-[11px] text-on-surface-variant">
                                    {format!("{} poin", u.points)}
                                </span>
                            }
                        })}
                </div>
            </div>
        </div>
    }
}

/// Form sunting profil satu pengguna.
#[component]
fn FormProfil(
    u: ManagedUser,
    on_saved: impl Fn(String) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let id = u.id;
    let santri = u.role.starts_with("santri");
    let nama = RwSignal::new(u.full_name.clone());
    let nis = RwSignal::new(u.nis.clone().unwrap_or_default());
    let hp = RwSignal::new(u.phone_number.clone().unwrap_or_default());
    let angkatan = RwSignal::new(u.entry_year.map(|t| t.to_string()).unwrap_or_default());
    let gender = RwSignal::new(u.gender.clone().unwrap_or_default());
    let kampus = RwSignal::new(u.campus.clone().unwrap_or_default());
    let jurusan = RwSignal::new(u.major.clone().unwrap_or_default());
    let mub = RwSignal::new(u.mubalegh_status.clone().unwrap_or_default());
    let pen = RwSignal::new(u.pendidikan_status.clone().unwrap_or_default());

    let busy = RwSignal::new(false);
    let err = RwSignal::new(String::new());

    // `peran_awal` disimpan sebagai StoredValue supaya closure pembanding tetap
    // Copy; tombol "Ubah" mati selama pilihannya belum berubah.
    let peran_awal = StoredValue::new(u.role.clone());
    let peran_baru = RwSignal::new(u.role.clone());
    let ganti_peran = move |_| {
        if busy.get_untracked() {
            return;
        }
        let r = peran_baru.get_untracked();
        if r == peran_awal.get_value() {
            return;
        }
        busy.set(true);
        err.set(String::new());
        leptos::task::spawn_local(async move {
            let hasil = change_user_role_action(id, r).await;
            busy.set(false);
            match hasil {
                Ok(()) => on_saved("Peran diubah.".into()),
                Err(e) => err.set(e.to_string()),
            }
        });
    };

    let simpan = move |_| {
        if busy.get_untracked() {
            return;
        }
        if nama.get_untracked().trim().is_empty() {
            err.set("Nama tidak boleh kosong.".into());
            return;
        }
        // Angkatan diketik bebas; yang bukan angka masuk akal dibuang alih-alih
        // menggagalkan simpan — sisanya tetap tersimpan.
        let tahun = angkatan
            .get_untracked()
            .trim()
            .parse::<i16>()
            .ok()
            .filter(|t| (2000..=2100).contains(t));
        err.set(String::new());
        busy.set(true);
        let p = ProfilEdit {
            full_name: nama.get_untracked(),
            nis: nis.get_untracked(),
            phone_number: hp.get_untracked(),
            entry_year: tahun,
            gender: gender.get_untracked(),
            campus: kampus.get_untracked(),
            major: jurusan.get_untracked(),
            mubalegh_status: mub.get_untracked(),
            pendidikan_status: pen.get_untracked(),
        };
        leptos::task::spawn_local(async move {
            let hasil = update_managed_user_action(id, p).await;
            busy.set(false);
            match hasil {
                Ok(()) => on_saved("Profil tersimpan.".into()),
                Err(e) => err.set(e.to_string()),
            }
        });
    };

    view! {
        <Isian label="Nama Lengkap" nilai=nama tipe="text" />
        {santri
            .then(|| {
                view! {
                    <Isian label="NIS" nilai=nis tipe="text" />
                    <Isian label="Angkatan" nilai=angkatan tipe="number" />
                }
            })}
        <Isian label="Nomor HP" nilai=hp tipe="tel" />

        <label class="block text-body-sm font-semibold text-on-background mb-1.5">
            "Jenis Kelamin"
        </label>
        // DUA pilihan saja. Sebelumnya ada tombol ketiga berlabel "—" yang
        // maksudnya "belum diisi", tapi bentuknya sama persis dengan dua
        // lainnya — jadi ia terbaca sebagai PILIHAN KETIGA. Di lingkungan
        // pesantren itu bukan sekadar tak rapi, itu keliru.
        //
        // Belum diisi tetap mungkin (tersimpan NULL), tapi ia KEADAAN, bukan
        // pilihan: ditandai tulisan kecil di bawah, bukan tombol yang bisa
        // ditekan.
        <div class="flex gap-1 bg-surface-container rounded-xl p-1 mb-1.5">
            {[("L", "Laki-laki"), ("P", "Perempuan")]
                .into_iter()
                .map(|(k, l)| {
                    view! {
                        <button
                            class=move || {
                                if gender.get() == k {
                                    "flex-1 py-2 rounded-lg bg-surface text-on-background shadow-sm text-body-sm font-semibold cursor-pointer"
                                } else {
                                    "flex-1 py-2 rounded-lg text-on-surface-variant text-body-sm font-semibold cursor-pointer"
                                }
                            }
                            aria-pressed=move || (gender.get() == k).to_string()
                            on:click=move |_| gender.set(k.to_string())
                        >
                            {l}
                        </button>
                    }
                })
                .collect_view()}
        </div>
        <Show when=move || gender.get().is_empty()>
            <p class="text-[11px] text-on-surface-variant mb-4">
                "Belum diisi — pilih salah satu bila datanya sudah diketahui."
            </p>
        </Show>
        <Show when=move || !gender.get().is_empty()>
            <div class="mb-4"></div>
        </Show>

        {santri
            .then(|| {
                view! {
                    <Isian label="Kampus" nilai=kampus tipe="text" />
                    <Isian label="Jurusan" nilai=jurusan tipe="text" />
                    <Pilihan label="Status Mubaligh" nilai=mub opsi=MUBALEGH />
                    <Pilihan label="Status Pendidikan" nilai=pen opsi=PENDIDIKAN />
                }
            })}

        // ── Ganti peran ──────────────────────────────────────────────────
        // Dipisah dari tombol "Simpan": mengubah peran mengubah HAK AKSES, dan
        // itu tak boleh ikut terkirim diam-diam saat seseorang cuma
        // membetulkan ejaan nama. Server tetap membatasi ke admin — ketua bisa
        // menyunting profil & mengaktifkan, tapi tidak menaikkan peran orang
        // (termasuk dirinya sendiri).
        <div class="mt-2 mb-4 p-3 rounded-xl border border-outline-variant/60">
            <p class="text-body-sm font-semibold text-on-background mb-1.5">"Peran"</p>
            <div class="flex gap-2">
                <select
                    class="flex-1 min-w-0 bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface"
                    on:change=move |ev| peran_baru.set(event_target_value(&ev))
                >
                    {PERAN
                        .iter()
                        .filter(|(k, _)| !k.is_empty())
                        .map(|(k, l)| {
                            let k = *k;
                            view! {
                                <option value=k selected=move || peran_baru.get() == k>{*l}</option>
                            }
                        })
                        .collect_view()}
                </select>
                <button
                    class="px-4 py-2.5 rounded-xl border border-outline-variant text-body-sm font-semibold text-on-surface hover:border-primary hover:text-primary transition-colors cursor-pointer disabled:opacity-50"
                    prop:disabled=move || busy.get() || peran_baru.get() == peran_awal.get_value()
                    on:click=ganti_peran
                >
                    "Ubah"
                </button>
            </div>
            <p class="text-[11px] text-on-surface-variant mt-1.5">
                "Khusus admin. Mengubah peran mengubah hak akses."
            </p>
        </div>

        <Show when=move || !err.get().is_empty()>
            <div class="mt-2 mb-2 p-2.5 bg-error-container text-on-error-container rounded-lg text-body-sm">
                {move || err.get()}
            </div>
        </Show>

        <button
            class="w-full mt-3 py-3 rounded-xl bg-primary text-on-primary font-semibold press cursor-pointer disabled:opacity-60"
            prop:disabled=move || busy.get()
            on:click=simpan
        >
            {move || if busy.get() { "Menyimpan…" } else { "Simpan" }}
        </button>
    }
}

/// Satu isian teks berlabel — dipakai berulang di form profil.
#[component]
fn Isian(
    label: &'static str,
    nilai: RwSignal<String>,
    tipe: &'static str,
) -> impl IntoView {
    view! {
        <label class="block text-body-sm font-semibold text-on-background mb-1.5">{label}</label>
        <input
            type=tipe
            class="w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface mb-4"
            prop:value=move || nilai.get()
            on:input=move |ev| nilai.set(event_target_value(&ev))
        />
    }
}

/// Satu dropdown berlabel.
#[component]
fn Pilihan(
    label: &'static str,
    nilai: RwSignal<String>,
    opsi: &'static [(&'static str, &'static str)],
) -> impl IntoView {
    view! {
        <label class="block text-body-sm font-semibold text-on-background mb-1.5">{label}</label>
        <select
            class="w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface mb-4"
            on:change=move |ev| nilai.set(event_target_value(&ev))
        >
            {opsi
                .iter()
                .map(|(k, l)| {
                    let k = *k;
                    view! {
                        <option value=k selected=move || nilai.get() == k>
                            {*l}
                        </option>
                    }
                })
                .collect_view()}
        </select>
    }
}
