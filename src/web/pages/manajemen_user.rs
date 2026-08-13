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

use crate::models::{AnakOrtu, ManagedUser, OrtuSantri, ProfilEdit, StudentSearchItem};
use crate::web::api::{
    anak_ortu_data, anak_ortu_search, angkatan_tersedia_data, change_user_role_action,
    managed_users_data, set_anak_ortu_action, set_user_active_action, set_users_active_action,
    update_managed_user_action,
    ortu_santri_data, ortu_santri_search, set_ortu_santri_action,};
use crate::web::components::{
    DeviceFrame, EmptyState, FetchError, FlashMsg, MobileHeader, Sheet,
};

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
    ("parent", "Orang Tua"),
    // Penjaga gerbang (migrasi 83) — sempat terlewat di sini, jadi akun penjaga
    // tak bisa disaring maupun DIBUAT dari layar ini sama sekali.
    ("penjaga", "Penjaga"),
    ("ketua", "Ketua"),
    ("admin", "Admin"),
];

/// Status keanggotaan di PPM (migrasi 82).
///
/// Kosong = biarkan seperti adanya. Bukan "tidak diketahui": sebagian besar
/// santri memang tak pernah berubah statusnya, dan memaksa pengelola memilih
/// "Aktif" untuk ratusan orang hanya menambah pekerjaan tanpa menambah
/// keterangan apa pun.
const STATUS_PPM: &[(&str, &str)] = &[
    ("", "— (masih aktif)"),
    ("aktif", "Santri aktif"),
    ("lulus", "Lulus PPM"),
    ("mengundurkan_diri", "Mengundurkan diri"),
    ("pindah", "Pindah"),
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

    // ── Pilihan untuk aksi massal ────────────────────────────────────────────
    // Dikosongkan tiap penyaring berubah: id yang tercentang lalu tak lagi
    // terlihat karena filter bergeser adalah jebakan — pengelola menekan
    // "Aktifkan 40" sambil melihat 5 baris di layar.
    let dipilih = RwSignal::new(Vec::<i64>::new());
    Effect::new(move |_| {
        let _ = (status.get(), peran.get(), angkatan.get(), cari_kirim.get());
        dipilih.set(Vec::new());
    });
    let toggle_pilih = move |id: i64| {
        dipilih.update(|v| {
            if let Some(i) = v.iter().position(|&x| x == id) {
                v.remove(i);
            } else {
                v.push(id);
            }
        });
    };
    let id_tampil = move || {
        data.get()
            .and_then(|r| r.ok())
            .map(|v| v.into_iter().map(|u| u.id).collect::<Vec<i64>>())
            .unwrap_or_default()
    };
    let semua_tercentang = move || {
        let t = id_tampil();
        !t.is_empty() && {
            let d = dipilih.get();
            t.iter().all(|id| d.contains(id))
        }
    };
    let toggle_semua = move |_: leptos::ev::MouseEvent| {
        let t = id_tampil();
        if t.is_empty() {
            return;
        }
        let hapus = semua_tercentang();
        dipilih.set(if hapus { Vec::new() } else { t });
    };

    // Aksi massal. Menonaktifkan selalu dikonfirmasi — ia mencabut orang dari
    // kelas, papan poin, dan statistik sekaligus.
    let massal = move |aktifkan: bool| {
        if busy.get_untracked() {
            return;
        }
        let ids = dipilih.get_untracked();
        if ids.is_empty() {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let n = ids.len();
            let pesan = if aktifkan {
                format!(
                    "Aktifkan {n} user? Santri yang belum punya riwayat poin akan \
                     mendapat saldo awal {} poin.",
                    crate::models::SEMESTER_START_POINTS
                )
            } else {
                format!(
                    "Nonaktifkan {n} user? Mereka akan hilang dari kelas, papan poin, \
                     dan statistik."
                )
            };
            let ok = web_sys::window()
                .and_then(|w| w.confirm_with_message(&pesan).ok())
                .unwrap_or(false);
            if !ok {
                return;
            }
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            match set_users_active_action(ids, aktifkan).await {
                Ok(pesan) => {
                    msg.set(Some((true, pesan)));
                    dipilih.set(Vec::new());
                    data.refetch();
                }
                Err(e) => msg.set(Some((false, e.to_string()))),
            }
            busy.set(false);
        });
    };

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

                    <FlashMsg pesan=msg />

                    // ── Bilah aksi massal ────────────────────────────────
                    // Hanya muncul saat ada yang tercentang: bilah yang selalu
                    // ada dengan tombol mati hanya menambah kebisingan.
                    <Show when=move || !dipilih.get().is_empty()>
                        <div class="ppm-card p-3 flex flex-wrap items-center gap-2 anim-in">
                            <span class="text-body-sm font-semibold text-on-background flex-1 min-w-0">
                                {move || format!("{} dipilih", dipilih.get().len())}
                            </span>
                            <button
                                class="px-3 py-1.5 rounded-lg bg-primary text-on-primary text-body-sm font-semibold press cursor-pointer disabled:opacity-50"
                                prop:disabled=move || busy.get()
                                on:click=move |_| massal(true)
                            >
                                "Aktifkan"
                            </button>
                            <button
                                class="px-3 py-1.5 rounded-lg border border-error/40 text-error text-body-sm font-semibold cursor-pointer disabled:opacity-50"
                                prop:disabled=move || busy.get()
                                on:click=move |_| massal(false)
                            >
                                "Nonaktifkan"
                            </button>
                            <button
                                class="px-3 py-1.5 rounded-lg text-on-surface-variant text-body-sm font-semibold cursor-pointer"
                                on:click=move |_| dipilih.set(Vec::new())
                            >
                                "Batal"
                            </button>
                        </div>
                    </Show>

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
                                            <div class="flex items-center justify-between gap-2">
                                                <p class="text-body-sm text-on-surface-variant min-w-0">
                                                    {if n >= 500 {
                                                        format!("Menampilkan {n} teratas — persempit dengan pencarian.")
                                                    } else {
                                                        format!("{n} pengguna")
                                                    }}
                                                </p>
                                                <button
                                                    class="px-3 py-1.5 rounded-lg border border-outline-variant text-[11px] font-semibold text-on-surface hover:border-primary hover:text-primary transition-colors cursor-pointer shrink-0 whitespace-nowrap"
                                                    on:click=toggle_semua
                                                >
                                                    {move || {
                                                        if semua_tercentang() {
                                                            "Hapus centang".to_string()
                                                        } else {
                                                            format!("Centang semua ({n})")
                                                        }
                                                    }}
                                                </button>
                                            </div>
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
                                                        let uid = u.id;
                                                        view! {
                                                            <BarisUser
                                                                u=u
                                                                busy=busy
                                                                tercentang=Signal::derive(move || dipilih.with(|v| v.contains(&uid)))
                                                                on_centang=move || toggle_pilih(uid)
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
    tercentang: Signal<bool>,
    on_centang: impl Fn() + Copy + Send + 'static,
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
            // Centang untuk aksi massal. Di luar <label> pembungkus baris agar
            // menekan nama/tombol tak ikut mencentang.
            <input
                type="checkbox"
                class="w-5 h-5 accent-primary cursor-pointer shrink-0 mt-0.5"
                prop:checked=move || tercentang.get()
                on:change=move |_| on_centang()
                aria-label="Pilih untuk aksi massal"
            />
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
    let sppm = RwSignal::new(u.status_ppm.clone().unwrap_or_default());

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
            status_ppm: sppm.get_untracked(),
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
                    <Pilihan label="Status PPM" nilai=sppm opsi=STATUS_PPM />
                    <p class="text-[11px] text-on-surface-variant -mt-1">
                        "Riwayat keanggotaan, TERPISAH dari aktif/nonaktif akun.                          Alumni tetap boleh punya akses; santri yang aksesnya                          dicabut sementara statusnya tak ikut berubah."
                    </p>
                }
            })}

        // Panel relasi keluarga — DUA arah, dipilih menurut peran akun yang
        // sedang disunting. Keduanya menulis ke junction yang sama; yang
        // berbeda cuma dari sisi mana pengelola sedang melihat.
        {(u.role == "parent").then(|| view! { <PanelAnak parent_id=id /> })}
        {crate::models::role_satisfies(&u.role, &["santri"])
            .then(|| view! { <PanelOrtu student_id=id /> })}

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

/// Daftar & pengelolaan ANAK untuk satu akun Orang Tua.
///
/// KENAPA ADA DI SINI. Koneksi ortu↔santri lahir dari permintaan ortu yang
/// harus DISETUJUI SANTRI — dan itu benar sebagai bawaan, karena koneksi
/// membuka data pribadi seseorang. Tapi alurnya buntu justru untuk keluarga
/// yang paling butuh: santri yang belum pernah membuka aplikasi tak bisa
/// menyetujui apa pun, sementara walinya sudah menunggu. Sebelum panel ini,
/// satu-satunya jalan keluarnya adalah menulis SQL langsung ke produksi.
///
/// Berdiri SENDIRI dari tombol "Simpan" di atasnya: menautkan anak berlaku
/// seketika (dan bisa dibatalkan seketika), bukan menunggu penyimpanan profil.
/// Menggabungkannya akan membuat pengelola yang cuma membetulkan ejaan nama
/// ikut mengirim perubahan hak akses tanpa bermaksud demikian.
#[component]
fn PanelAnak(parent_id: i64) -> impl IntoView {
    let anak = Resource::new(move || parent_id, |id| async move { anak_ortu_data(id).await });
    let cari = RwSignal::new(String::new());
    let hasil = RwSignal::new(Vec::<StudentSearchItem>::new());
    let busy = RwSignal::new(false);
    let pesan = RwSignal::new(Option::<(bool, String)>::None);

    let telusuri = move |_| {
        let q = cari.get_untracked();
        if q.trim().len() < 2 {
            pesan.set(Some((false, "Ketik minimal 2 huruf nama, atau NIS lengkap.".into())));
            return;
        }
        pesan.set(None);
        leptos::task::spawn_local(async move {
            match anak_ortu_search(q).await {
                Ok(r) => hasil.set(r),
                Err(e) => pesan.set(Some((false, e.to_string()))),
            }
        });
    };

    let ubah = move |student_id: i64, sambung: bool| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            match set_anak_ortu_action(parent_id, student_id, sambung).await {
                Ok(t) => {
                    pesan.set(Some((true, t)));
                    // Hasil pencarian dikosongkan setelah menautkan: baris yang
                    // baru saja ditambahkan sudah pindah ke daftar di atasnya,
                    // dan membiarkannya di sini mengundang klik kedua.
                    if sambung {
                        hasil.set(Vec::new());
                        cari.set(String::new());
                    }
                    anak.refetch();
                }
                Err(e) => pesan.set(Some((false, e.to_string()))),
            }
            busy.set(false);
        });
    };

    view! {
        <div class="mt-2 mb-4 p-3 rounded-xl border border-outline-variant/60 space-y-2.5">
            <div>
                <p class="text-body-sm font-semibold text-on-background">"Anak"</p>
                <p class="text-[11px] text-on-surface-variant">
                    "Santri yang datanya boleh dilihat akun ini di halaman Orang Tua."
                </p>
            </div>

            {move || {
                pesan
                    .get()
                    .map(|(ok, t)| {
                        let cls = if ok {
                            "p-2 bg-secondary-container text-on-secondary-container rounded-lg text-[11px]"
                        } else {
                            "p-2 bg-error-container text-on-error-container rounded-lg text-[11px]"
                        };
                        view! { <div class=cls>{t}</div> }
                    })
            }}

            <Suspense fallback=|| {
                view! { <div class="h-10 bg-surface-container rounded-lg animate-pulse"></div> }
            }>
                {move || {
                    anak.get()
                        .map(|res| match res {
                            Err(e) => {
                                view! {
                                    <p class="text-[11px] text-error">{e.to_string()}</p>
                                }
                                    .into_any()
                            }
                            Ok(list) if list.is_empty() => {
                                view! {
                                    <p class="text-[11px] text-on-surface-variant">
                                        "Belum ada anak tertaut. Cari di bawah untuk menautkan."
                                    </p>
                                }
                                    .into_any()
                            }
                            Ok(list) => {
                                view! {
                                    <div class="space-y-1.5">
                                        {list
                                            .into_iter()
                                            .map(|a: AnakOrtu| {
                                                let sid = a.student_id;
                                                let terhubung = a.terhubung;
                                                view! {
                                                    <div class="flex items-center gap-2 bg-surface-container rounded-lg px-2.5 py-2">
                                                        <div class="flex-1 min-w-0">
                                                            <p class="text-body-sm font-semibold text-on-background truncate">
                                                                {a.full_name}
                                                            </p>
                                                            <p class="text-[10px] text-on-surface-variant truncate">
                                                                {format!("{} · {} · {}", a.nis, a.class_name, a.status_label)}
                                                            </p>
                                                        </div>
                                                        // Permintaan yang masih menunggu bisa langsung
                                                        // disetujui pengelola — itulah kebuntuan yang
                                                        // membuat panel ini ada.
                                                        {(!terhubung)
                                                            .then(|| {
                                                                view! {
                                                                    <button
                                                                        class="px-2.5 py-1 rounded-lg bg-primary text-on-primary text-[11px] font-semibold press cursor-pointer disabled:opacity-50 shrink-0"
                                                                        prop:disabled=move || busy.get()
                                                                        on:click=move |_| ubah(sid, true)
                                                                    >
                                                                        "Setujui"
                                                                    </button>
                                                                }
                                                            })}
                                                        <button
                                                            class="px-2.5 py-1 rounded-lg border border-error/40 text-error text-[11px] font-semibold cursor-pointer disabled:opacity-50 shrink-0"
                                                            prop:disabled=move || busy.get()
                                                            on:click=move |_| ubah(sid, false)
                                                        >
                                                            "Lepas"
                                                        </button>
                                                    </div>
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

            // ── Tautkan anak baru ────────────────────────────────────────
            <div class="flex gap-2">
                <input
                    type="search"
                    placeholder="Cari nama atau NIS santri…"
                    class="flex-1 min-w-0 bg-surface-container border-0 rounded-lg px-2.5 py-2 text-body-sm text-on-surface"
                    prop:value=move || cari.get()
                    on:input=move |ev| cari.set(event_target_value(&ev))
                    on:keydown=move |ev| {
                        if ev.key() == "Enter" {
                            ev.prevent_default();
                            telusuri(());
                        }
                    }
                />
                <button
                    class="px-3 py-2 rounded-lg border border-outline-variant text-body-sm font-semibold text-on-surface hover:border-primary hover:text-primary transition-colors cursor-pointer shrink-0"
                    on:click=move |_| telusuri(())
                >
                    "Cari"
                </button>
            </div>

            {move || {
                let r = hasil.get();
                (!r.is_empty())
                    .then(|| {
                        view! {
                            <div class="max-h-40 overflow-y-auto space-y-1">
                                {r
                                    .into_iter()
                                    .map(|s| {
                                        let sid = s.id;
                                        view! {
                                            <button
                                                class="w-full text-left px-2.5 py-2 bg-surface-container rounded-lg hover:bg-secondary-container/50 cursor-pointer disabled:opacity-50"
                                                prop:disabled=move || busy.get()
                                                on:click=move |_| ubah(sid, true)
                                            >
                                                <span class="text-body-sm text-on-surface">{s.name}</span>
                                                <span class="text-[10px] text-on-surface-variant">
                                                    {format!(" · {} · {}", s.nis, s.class_name)}
                                                </span>
                                            </button>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                    })
            }}
        </div>
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

/// Panel ORANG TUA pada Edit Profil seorang SANTRI — cermin dari [`PanelAnak`].
///
/// Kenapa dua panel untuk satu relasi. Pengelola datang dari dua arah: kadang
/// ia sedang membuka akun ortu yang baru dibuat ("anaknya siapa?"), kadang
/// sedang membuka akun santri ("ayah-ibunya sudah terdaftar belum?"). Selama
/// ini hanya arah pertama yang punya layar, jadi menautkan dari akun santri
/// berarti mencari dulu akun ortunya di daftar, membukanya, lalu mencari
/// santrinya kembali di sana.
///
/// Relasinya BANYAK-KE-BANYAK dan itu memang disengaja: satu santri boleh punya
/// ayah, ibu, atau wali dengan akun masing-masing, dan satu akun ortu boleh
/// punya beberapa anak di pondok. Tak ada batas jumlah di sisi mana pun —
/// `parent_connections` hanya melarang PASANGAN yang sama tercatat dua kali.
///
/// Berdiri SENDIRI dari tombol "Simpan" di atasnya, alasannya sama dengan
/// `PanelAnak`: menautkan berlaku seketika, bukan menunggu penyimpanan profil.
#[component]
fn PanelOrtu(student_id: i64) -> impl IntoView {
    let ortu = Resource::new(move || student_id, |id| async move { ortu_santri_data(id).await });
    let cari = RwSignal::new(String::new());
    let hasil = RwSignal::new(Vec::<OrtuSantri>::new());
    let busy = RwSignal::new(false);
    let pesan = RwSignal::new(Option::<(bool, String)>::None);

    let telusuri = move |_| {
        let q = cari.get_untracked();
        if q.trim().chars().count() < 2 {
            pesan.set(Some((false, "Ketik minimal 2 huruf nama, atau nomor HP.".into())));
            return;
        }
        pesan.set(None);
        leptos::task::spawn_local(async move {
            match ortu_santri_search(q).await {
                Ok(r) if r.is_empty() => {
                    pesan.set(Some((
                        false,
                        "Tak ada akun Orang Tua yang cocok. Akunnya harus dibuat dulu \
                         (peran: Orang Tua)."
                            .into(),
                    )));
                    hasil.set(Vec::new());
                }
                Ok(r) => hasil.set(r),
                Err(e) => pesan.set(Some((false, crate::web::components::pesan_galat(e)))),
            }
        });
    };

    let ubah = move |parent_id: i64, sambung: bool| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            match set_ortu_santri_action(student_id, parent_id, sambung).await {
                Ok(t) => {
                    pesan.set(Some((true, t)));
                    // Hasil pencarian dikosongkan setelah menautkan: barisnya
                    // sudah pindah ke daftar di atas, dan membiarkannya di sini
                    // mengundang klik kedua.
                    if sambung {
                        hasil.set(Vec::new());
                        cari.set(String::new());
                    }
                    ortu.refetch();
                }
                Err(e) => pesan.set(Some((false, crate::web::components::pesan_galat(e)))),
            }
            busy.set(false);
        });
    };

    view! {
        <div class="mt-2 mb-4 p-3 rounded-xl border border-outline-variant/60 space-y-2.5">
            <div>
                <p class="text-body-sm font-semibold text-on-background">"Orang Tua / Wali"</p>
                <p class="text-[11px] text-on-surface-variant">
                    "Akun yang boleh melihat data santri ini di halaman Orang Tua. Boleh lebih dari satu (ayah, ibu, wali)."
                </p>
            </div>

            <FlashMsg pesan=pesan />

            <Suspense fallback=|| {
                view! { <div class="h-10 bg-surface-container rounded-lg animate-pulse"></div> }
            }>
                {move || {
                    ortu.get()
                        .map(|res| match res {
                            Err(e) => {
                                view! {
                                    <p class="text-[11px] text-error">
                                        {crate::web::components::pesan_galat(e)}
                                    </p>
                                }
                                    .into_any()
                            }
                            Ok(list) if list.is_empty() => {
                                view! {
                                    <p class="text-[11px] text-on-surface-variant">
                                        "Belum ada orang tua tertaut. Cari di bawah untuk menautkan."
                                    </p>
                                }
                                    .into_any()
                            }
                            Ok(list) => {
                                view! {
                                    <div class="space-y-1.5">
                                        {list
                                            .into_iter()
                                            .map(|o: OrtuSantri| {
                                                let pid = o.parent_id;
                                                let terhubung = o.terhubung;
                                                view! {
                                                    <div class="flex items-center gap-2 bg-surface-container rounded-lg px-2.5 py-2">
                                                        <div class="flex-1 min-w-0">
                                                            <p class="text-body-sm font-semibold text-on-background truncate">
                                                                {o.full_name}
                                                            </p>
                                                            <p class="text-[10px] text-on-surface-variant truncate">
                                                                {format!("{} · {}", o.phone, o.status_label)}
                                                            </p>
                                                        </div>
                                                        // Permintaan ortu yang masih menunggu jawaban
                                                        // santri bisa langsung disetujui pengelola —
                                                        // santri yang belum pernah membuka aplikasi
                                                        // tak boleh jadi kebuntuan.
                                                        {(!terhubung)
                                                            .then(|| {
                                                                view! {
                                                                    <button
                                                                        class="px-2.5 py-1 rounded-lg bg-primary text-on-primary text-[11px] font-semibold press cursor-pointer disabled:opacity-50 shrink-0"
                                                                        prop:disabled=move || busy.get()
                                                                        on:click=move |_| ubah(pid, true)
                                                                    >
                                                                        "Setujui"
                                                                    </button>
                                                                }
                                                            })}
                                                        <button
                                                            class="px-2.5 py-1 rounded-lg border border-error/40 text-error text-[11px] font-semibold cursor-pointer disabled:opacity-50 shrink-0"
                                                            prop:disabled=move || busy.get()
                                                            on:click=move |_| ubah(pid, false)
                                                        >
                                                            "Lepas"
                                                        </button>
                                                    </div>
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

            // ── Tautkan orang tua ────────────────────────────────────────
            <div class="flex gap-2">
                <input
                    type="search"
                    placeholder="Cari nama atau nomor HP orang tua…"
                    class="flex-1 min-w-0 bg-surface-container border-0 rounded-lg px-2.5 py-2 text-body-sm text-on-surface"
                    prop:value=move || cari.get()
                    on:input=move |ev| cari.set(event_target_value(&ev))
                    on:keydown=move |ev| {
                        if ev.key() == "Enter" {
                            ev.prevent_default();
                            telusuri(());
                        }
                    }
                />
                <button
                    class="px-3 py-2 rounded-lg border border-outline-variant text-body-sm font-semibold text-on-surface hover:border-primary hover:text-primary transition-colors cursor-pointer shrink-0"
                    on:click=move |_| telusuri(())
                >
                    "Cari"
                </button>
            </div>

            {move || {
                let r = hasil.get();
                (!r.is_empty())
                    .then(|| {
                        view! {
                            <div class="max-h-40 overflow-y-auto space-y-1">
                                {r
                                    .into_iter()
                                    .map(|o| {
                                        let pid = o.parent_id;
                                        view! {
                                            <button
                                                class="w-full text-left px-2.5 py-2 bg-surface-container rounded-lg hover:bg-secondary-container/50 cursor-pointer disabled:opacity-50"
                                                prop:disabled=move || busy.get()
                                                on:click=move |_| ubah(pid, true)
                                            >
                                                <span class="text-body-sm text-on-surface">{o.full_name}</span>
                                                <span class="text-[10px] text-on-surface-variant">
                                                    {format!(" · {}", o.phone)}
                                                </span>
                                            </button>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                    })
            }}
        </div>
    }
}
