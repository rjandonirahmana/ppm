//! web/pages/profil.rs — Profil pengguna (mockup Halaqah Manager): kartu hero
//! gradient, informasi kontak, status akademik, pengaturan akun + Logout ASLI.
//! Data via server fn `profil_data` (semua peran).

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::ProfilData;
use crate::web::api::{
    add_ipk_action, delete_ipk_action, kalender_langganan_path, logout_action, profil_data,
    update_contact_action,
    update_profile_action,
};
use crate::web::components::{DeviceFrame, MobileHeader};

#[component]
pub fn ProfilPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { profil_data().await });

    // Form data mahasiswa (santri isi sendiri) — diinisialisasi dari resource.
    let campus = RwSignal::new(String::new());
    let major = RwSignal::new(String::new());
    let gender = RwSignal::new(String::new());
    let entry_year = RwSignal::new(String::new());
    let is_santri = RwSignal::new(false);
    let sem_input = RwSignal::new(String::new());
    let ipk_input = RwSignal::new(String::new());
    let msg = RwSignal::new(String::new());
    let saving = RwSignal::new(false);

    // Form kontak (email + alamat) — semua peran.
    let email = RwSignal::new(String::new());
    let address = RwSignal::new(String::new());
    let contact_msg = RwSignal::new(String::new());
    let saving_contact = RwSignal::new(false);

    Effect::new(move |_| {
        if let Some(Ok(p)) = data.get() {
            campus.set(p.campus.clone().unwrap_or_default());
            major.set(p.major.clone().unwrap_or_default());
            gender.set(p.gender.clone().unwrap_or_default());
            entry_year.set(p.entry_year.map(|y| y.to_string()).unwrap_or_default());
            email.set(p.email.clone().unwrap_or_default());
            address.set(p.address.clone().unwrap_or_default());
            is_santri.set(matches!(p.role.as_str(), "santri" | "santri_finance"));
        }
    });

    let save_profile = move |_| {
        saving.set(true);
        msg.set(String::new());
        leptos::task::spawn_local(async move {
            let r = update_profile_action(
                campus.get_untracked(),
                major.get_untracked(),
                gender.get_untracked(),
                entry_year.get_untracked(),
            )
            .await;
            saving.set(false);
            match r {
                Ok(_) => {
                    msg.set("Data mahasiswa tersimpan.".into());
                    data.refetch();
                }
                Err(e) => {
                                        msg.set(crate::web::components::pesan_galat(e));
                }
            }
        });
    };

    let save_contact = move |_| {
        saving_contact.set(true);
        contact_msg.set(String::new());
        leptos::task::spawn_local(async move {
            let r = update_contact_action(email.get_untracked(), address.get_untracked()).await;
            saving_contact.set(false);
            match r {
                Ok(_) => {
                    contact_msg.set("Kontak tersimpan.".into());
                    data.refetch();
                }
                Err(e) => {
                                        contact_msg.set(crate::web::components::pesan_galat(e));
                }
            }
        });
    };

    let add_ipk = move |_| {
        let sem = sem_input.get_untracked();
        let val = ipk_input.get_untracked();
        msg.set(String::new());
        leptos::task::spawn_local(async move {
            match add_ipk_action(sem, val).await {
                Ok(_) => {
                    sem_input.set(String::new());
                    ipk_input.set(String::new());
                    data.refetch();
                }
                Err(e) => msg.set(e.to_string()),
            }
        });
    };

    let delete_ipk = move |id: i64| {
        leptos::task::spawn_local(async move {
            if delete_ipk_action(id).await.is_ok() {
                data.refetch();
            }
        });
    };

    crate::web::components::guard_sesi(data);

    view! {
        <Title text="Profil — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Profil Pengguna" />

                // Desktop: kartu profil (hero+kontak+akademik) kolom kiri sempit,
                // Pengaturan Akun + versi di kolom kanan lebar — pola halaman akun.
                <div class="px-5 pt-5 space-y-5 md:space-y-0 md:grid md:grid-cols-3 md:gap-5 md:items-start stagger">
                    <div class="space-y-5 md:col-span-1">
                        <Suspense fallback=|| {
                            view! {
                                <div class="space-y-3 animate-pulse">
                                    <div class="h-56 bg-surface-container rounded-2xl"></div>
                                    <div class="h-44 bg-surface-container rounded-2xl"></div>
                                </div>
                            }
                        }>
                            {move || {
                                data.get()
                                    .and_then(|r| r.ok())
                                    .map(|p| view! { <ProfilContent p=p /> })
                            }}
                        </Suspense>
                    </div>

                    <div class="space-y-5 md:col-span-2">
                        // ── Ubah Kontak (email + alamat) — semua peran ────────
                        <div class="ppm-card p-5">
                            <div class="flex items-center gap-2 mb-4">
                                <span class="material-symbols-outlined text-on-background">"contact_mail"</span>
                                <h2 class="text-body-lg font-bold text-on-background">"Ubah Kontak"</h2>
                            </div>
                            <div class="space-y-3 md:grid md:grid-cols-2 md:gap-3 md:space-y-0">
                                <label class="block">
                                    <span class="text-[11px] font-bold tracking-wider text-on-surface-variant uppercase">"Email"</span>
                                    <input
                                        class="mt-1 w-full rounded-xl border border-outline-variant bg-surface px-3 py-2.5 text-body-md text-on-background focus:outline-none focus:ring-2 focus:ring-primary"
                                        placeholder="nama@email.com"
                                        type="email"
                                        inputmode="email"
                                        prop:value=move || email.get()
                                        on:input=move |e| email.set(event_target_value(&e))
                                    />
                                </label>
                                <label class="block">
                                    <span class="text-[11px] font-bold tracking-wider text-on-surface-variant uppercase">"Alamat"</span>
                                    <input
                                        class="mt-1 w-full rounded-xl border border-outline-variant bg-surface px-3 py-2.5 text-body-md text-on-background focus:outline-none focus:ring-2 focus:ring-primary"
                                        placeholder="Alamat domisili"
                                        prop:value=move || address.get()
                                        on:input=move |e| address.set(event_target_value(&e))
                                    />
                                </label>
                            </div>
                            <button
                                class="mt-4 w-full md:w-auto px-5 py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-md cursor-pointer press disabled:opacity-60"
                                prop:disabled=move || saving_contact.get()
                                on:click=save_contact
                            >
                                {move || if saving_contact.get() { "Menyimpan…" } else { "Simpan Kontak" }}
                            </button>
                            <Show when=move || !contact_msg.get().is_empty()>
                                <p class="mt-3 text-body-sm text-on-surface-variant">{move || contact_msg.get()}</p>
                            </Show>
                        </div>

                        // ── Data Mahasiswa + Riwayat IPK (santri isi sendiri) ─
                        <Show when=move || is_santri.get()>
                            <div class="ppm-card p-5">
                                <div class="flex items-center gap-2 mb-4">
                                    <span class="material-symbols-outlined text-on-background">"school"</span>
                                    <h2 class="text-body-lg font-bold text-on-background">"Data Mahasiswa"</h2>
                                </div>
                                <div class="space-y-3 md:grid md:grid-cols-2 md:gap-3 md:space-y-0">
                                    <label class="block">
                                        <span class="text-[11px] font-bold tracking-wider text-on-surface-variant uppercase">"Kampus"</span>
                                        <input
                                            class="mt-1 w-full rounded-xl border border-outline-variant bg-surface px-3 py-2.5 text-body-md text-on-background focus:outline-none focus:ring-2 focus:ring-primary"
                                            placeholder="mis. Universitas Indonesia"
                                            prop:value=move || campus.get()
                                            on:input=move |e| campus.set(event_target_value(&e))
                                        />
                                    </label>
                                    <label class="block">
                                        <span class="text-[11px] font-bold tracking-wider text-on-surface-variant uppercase">"Jurusan"</span>
                                        <input
                                            class="mt-1 w-full rounded-xl border border-outline-variant bg-surface px-3 py-2.5 text-body-md text-on-background focus:outline-none focus:ring-2 focus:ring-primary"
                                            placeholder="mis. Teknik Informatika"
                                            prop:value=move || major.get()
                                            on:input=move |e| major.set(event_target_value(&e))
                                        />
                                    </label>
                                    <label class="block">
                                        <span class="text-[11px] font-bold tracking-wider text-on-surface-variant uppercase">"Jenis Kelamin"</span>
                                        <select
                                            class="mt-1 w-full rounded-xl border border-outline-variant bg-surface px-3 py-2.5 text-body-md text-on-background focus:outline-none focus:ring-2 focus:ring-primary cursor-pointer"
                                            prop:value=move || gender.get()
                                            on:change=move |e| gender.set(event_target_value(&e))
                                        >
                                            // "Belum dipilih", bukan "—": pada <select>,
                                            // opsi kosong WAJIB ada (tanpa itu browser
                                            // menampilkan opsi pertama seolah terpilih
                                            // padahal nilainya kosong). Tapi labelnya harus
                                            // jelas menyatakan KETIADAAN, bukan tampak
                                            // seperti pilihan ketiga.
                                            <option value="">"Belum dipilih"</option>
                                            <option value="L">"Laki-laki"</option>
                                            <option value="P">"Perempuan"</option>
                                        </select>
                                    </label>
                                    <label class="block">
                                        <span class="text-[11px] font-bold tracking-wider text-on-surface-variant uppercase">"Tahun Masuk PPM"</span>
                                        <input
                                            class="mt-1 w-full rounded-xl border border-outline-variant bg-surface px-3 py-2.5 text-body-md text-on-background focus:outline-none focus:ring-2 focus:ring-primary"
                                            placeholder="mis. 2024"
                                            inputmode="numeric"
                                            maxlength="4"
                                            prop:value=move || entry_year.get()
                                            on:input=move |e| entry_year.set(event_target_value(&e))
                                        />
                                    </label>
                                </div>
                                <button
                                    class="mt-4 w-full md:w-auto px-5 py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-md cursor-pointer press disabled:opacity-60"
                                    prop:disabled=move || saving.get()
                                    on:click=save_profile
                                >
                                    {move || if saving.get() { "Menyimpan…" } else { "Simpan Data" }}
                                </button>
                            </div>

                            // ── Riwayat IPK ─────────────────────────────────
                            <div class="ppm-card p-5">
                                <div class="flex items-center gap-2 mb-4">
                                    <span class="material-symbols-outlined text-on-background">"trending_up"</span>
                                    <h2 class="text-body-lg font-bold text-on-background">"Riwayat IPK"</h2>
                                </div>
                                <div class="space-y-2">
                                    // WAJIB di dalam <Suspense>: membaca Resource di luarnya membuat
                                    // server & klien merender hal berbeda, dan selisih itu membatalkan
                                    // hidrasi SELURUH halaman — bukan cuma blok ini.
                                    <Suspense fallback=|| ()>
                                    {move || {
                                        data.get()
                                            .and_then(|r| r.ok())
                                            .map(|p| {
                                                if p.ipk_history.is_empty() {
                                                    view! {
                                                        <p class="text-body-sm text-on-surface-variant py-2">
                                                            "Belum ada riwayat IPK. Tambahkan per semester di bawah."
                                                        </p>
                                                    }.into_any()
                                                } else {
                                                    p.ipk_history.into_iter().map(|it| {
                                                        let id = it.id;
                                                        view! {
                                                            <div class="flex items-center gap-3 bg-surface-container rounded-xl px-3.5 py-3">
                                                                <div class="flex-1 min-w-0">
                                                                    <p class="text-body-md font-semibold text-on-background truncate">{it.semester}</p>
                                                                </div>
                                                                <span class="text-title-md font-bold text-primary tabular-nums">
                                                                    {format!("{:.2}", it.ipk)}
                                                                </span>
                                                                <button
                                                                    class="w-9 h-9 rounded-full flex items-center justify-center text-error hover:bg-error-container cursor-pointer press"
                                                                    aria-label="Hapus"
                                                                    on:click=move |_| delete_ipk(id)
                                                                >
                                                                    <span class="material-symbols-outlined text-[20px]">"delete"</span>
                                                                </button>
                                                            </div>
                                                        }
                                                    }).collect_view().into_any()
                                                }
                                            })
                                    }}
                                    </Suspense>
                                </div>

                                // Form tambah entri IPK
                                <div class="mt-4 flex flex-col sm:flex-row gap-2">
                                    <input
                                        class="flex-1 rounded-xl border border-outline-variant bg-surface px-3 py-2.5 text-body-md text-on-background focus:outline-none focus:ring-2 focus:ring-primary"
                                        placeholder="Semester (mis. 2024/2025 Ganjil)"
                                        prop:value=move || sem_input.get()
                                        on:input=move |e| sem_input.set(event_target_value(&e))
                                    />
                                    <input
                                        class="sm:w-28 rounded-xl border border-outline-variant bg-surface px-3 py-2.5 text-body-md text-on-background focus:outline-none focus:ring-2 focus:ring-primary"
                                        placeholder="IPK"
                                        inputmode="decimal"
                                        prop:value=move || ipk_input.get()
                                        on:input=move |e| ipk_input.set(event_target_value(&e))
                                    />
                                    <button
                                        class="px-5 py-2.5 rounded-xl bg-secondary-container text-primary font-semibold text-body-md cursor-pointer press whitespace-nowrap"
                                        on:click=add_ipk
                                    >
                                        "Tambah"
                                    </button>
                                </div>
                                <Show when=move || !msg.get().is_empty()>
                                    <p class="mt-3 text-body-sm text-on-surface-variant">{move || msg.get()}</p>
                                </Show>
                            </div>
                        </Show>

                        <LanggananKalender />

                        // ── Pengaturan Akun ────────────────────────────────
                        <div class="ppm-card p-5">
                            <div class="flex items-center gap-2 mb-4">
                                <span class="material-symbols-outlined text-on-background">"settings"</span>
                                <h2 class="text-body-lg font-bold text-on-background">"Pengaturan Akun"</h2>
                            </div>
                            <div class="space-y-1 md:grid md:grid-cols-2 md:gap-x-4 md:space-y-0">
                                <SettingLink icon="lock" label="Ganti Kata Sandi" href="/ganti-sandi" />
                                // Placeholder mockup lain (Bahasa/Privasi/Tentang/Bantuan) DIHAPUS
                                // — belum berfungsi, bikin klik-nihil saat demo.

                                // Logout ASLI: hapus cookie sesi → /login.
                                <button
                                    class="w-full flex items-center gap-4 py-3.5 text-error font-semibold"
                                    on:click=move |_| {
                                        leptos::task::spawn_local(async move {
                                            let _ = logout_action().await;
                                            #[cfg(target_arch = "wasm32")]
                                            if let Some(w) = web_sys::window() {
                                                let _ = w.location().replace("/login");
                                            }
                                        });
                                    }
                                >
                                    <span class="w-11 h-11 rounded-full bg-error-container flex items-center justify-center">
                                        <span class="material-symbols-outlined text-error">"logout"</span>
                                    </span>
                                    "Keluar"
                                </button>
                            </div>
                        </div>

                        // ── Versi aplikasi ─────────────────────────────────
                        <div class="bg-surface-container rounded-2xl p-5 text-center">
                            <p class="text-body-md font-bold text-on-background">"AFM SMART v0.1.0"</p>
                            <p class="text-body-sm text-on-surface-variant mt-1">
                                "Absensi & pembinaan santri — dibuat dengan ♥ untuk keluarga PPM AFM."
                            </p>
                        </div>
                    </div>
                </div>

            </div>
        </DeviceFrame>
    }
}

#[component]
fn ProfilContent(p: ProfilData) -> impl IntoView {
    let initial = p.name.chars().next().unwrap_or('P').to_string();
    view! {
        // ── Kartu hero ──────────────────────────────────────────────────────
        <div class="spiritual-gradient rounded-2xl p-6 text-on-primary text-center shadow-lg shadow-primary/20">
            <div class="w-24 h-24 mx-auto rounded-full bg-primary-fixed text-primary flex items-center justify-center text-4xl font-bold ring-4 ring-white/20">
                {initial}
            </div>
            <h2 class="text-display-md mt-4">{p.name.clone()}</h2>
            <div class="flex items-center justify-center gap-2 mt-3">
                <span class="px-3 py-1 rounded-full bg-primary-fixed text-primary text-[11px] font-bold tracking-wider">
                    {p.role_label.clone()}
                </span>
                <span class="px-3 py-1 rounded-full bg-white/15 text-[11px] font-bold tracking-wider">
                    "AFM SMART"
                </span>
            </div>
            {(!p.username.is_empty())
                .then(|| {
                    view! {
                        <p class="text-body-sm opacity-80 mt-3">"@" {p.username.clone()}</p>
                    }
                })}
        </div>

        // ── Informasi kontak ────────────────────────────────────────────────
        <div class="ppm-card p-5">
            <div class="flex items-center gap-2 mb-4">
                <span class="material-symbols-outlined text-on-background">"contact_mail"</span>
                <h2 class="text-body-lg font-bold text-on-background">"Informasi Kontak"</h2>
            </div>
            <div class="space-y-3">
                <ContactRow icon="mail" label="Email" value=p.email.unwrap_or_else(|| "—".into()) />
                <ContactRow icon="call" label="Nomor Telepon" value=p.phone.unwrap_or_else(|| "—".into()) />
                <ContactRow icon="location_on" label="Alamat" value=p.address.unwrap_or_else(|| "—".into()) />
            </div>
        </div>

        // ── Status akademik ─────────────────────────────────────────────────
        <div class="spiritual-gradient rounded-2xl p-6 text-on-primary shadow-lg shadow-primary/20">
            <h2 class="text-body-lg font-bold mb-4">"Status Akademik"</h2>
            <div class="space-y-3">
                <div class="flex items-center justify-between border-b border-white/10 pb-3">
                    <span class="text-[11px] font-bold tracking-[0.15em] opacity-80">"NIS"</span>
                    <span class="text-body-lg font-bold">{p.nis.unwrap_or_else(|| "—".into())}</span>
                </div>
                <div class="flex items-center justify-between border-b border-white/10 pb-3">
                    <span class="text-[11px] font-bold tracking-[0.15em] opacity-80">"TOTAL POIN"</span>
                    <span class="text-body-lg font-bold" data-count=p.points.to_string()>{p.points}</span>
                </div>
                <div class="flex items-center justify-between">
                    <span class="text-[11px] font-bold tracking-[0.15em] opacity-80">"PRESTASI KETERTIBAN"</span>
                    <span class="px-3 py-1 rounded-full bg-white/15 text-body-sm font-bold">
                        {crate::models::prestasi_label(p.points).0}
                    </span>
                </div>
                {crate::models::sp_level(p.points)
                    .map(|(level, _, treatment)| {
                        view! {
                            <div class="mt-3 rounded-xl bg-white/15 p-3 border border-white/20">
                                <div class="flex items-center gap-2">
                                    <span class="material-symbols-outlined text-[18px]">"gavel"</span>
                                    <span class="text-body-md font-bold">"Status " {level}</span>
                                </div>
                                <p class="text-body-sm opacity-80 mt-1">{treatment}</p>
                            </div>
                        }
                    })}
            </div>
        </div>
    }
}

#[component]
fn ContactRow(icon: &'static str, label: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="bg-surface-container rounded-xl p-3.5 flex items-center gap-3">
            <span class="material-symbols-outlined text-on-surface-variant">{icon}</span>
            <div class="flex-1 min-w-0">
                <p class="text-[11px] font-bold tracking-wider text-on-surface-variant uppercase">
                    {label}
                </p>
                <p class="text-body-md text-on-background truncate">{value}</p>
            </div>
        </div>
    }
}

/// Link setelan fungsional (ikon + label → halaman tujuan).
#[component]
fn SettingLink(icon: &'static str, label: &'static str, href: &'static str) -> impl IntoView {
    view! {
        <a href=href class="flex items-center gap-4 py-3.5 px-2 -mx-2 rounded-xl text-on-background hover:bg-surface-container transition-colors press">
            <span class="w-11 h-11 rounded-full bg-secondary-container flex items-center justify-center">
                <span class="material-symbols-outlined text-primary">{icon}</span>
            </span>
            <span class="flex-1 text-body-md font-medium">{label}</span>
            <span class="material-symbols-outlined text-on-surface-variant">"arrow_forward"</span>
        </a>
    }
}

/// Kartu "Langganan Kalender": alamat `.ics` pribadi + cara memasangnya.
///
/// KENAPA TAUTAN, BUKAN KIRIMAN MINGGUAN. Menulis langsung ke Google Calendar
/// seseorang mustahil tanpa izin OAuth per-orang, dan mengirim jadwal sepekan
/// sebagai deretan tautan "Tambah ke Calendar" berarti belasan tautan dalam
/// satu pesan. Dengan berlangganan, pemasangannya cukup SEKALI dan sesudah itu
/// Google yang menarik sendiri — jadwal yang digeser ikut bergeser, yang libur
/// berubah jadi dicoret, tanpa satu pun pesan yang perlu dikirim.
///
/// Alamatnya diambil dari server, tak pernah disusun di browser: tokennya
/// diturunkan dari rahasia server, dan kalau browser bisa menghitungnya, siapa
/// pun tinggal mengganti angka id di URL untuk membaca jadwal orang lain.
#[component]
fn LanggananKalender() -> impl IntoView {
    let path = Resource::new(|| (), |_| async move { kalender_langganan_path().await });
    let buka = RwSignal::new(false);
    let disalin = RwSignal::new(false);

    // Origin ditambahkan di klien — server tak selalu tahu nama domain
    // publiknya sendiri (di belakang proxy, header Host bisa apa saja).
    let url_penuh = move || {
        let p = path.get().and_then(|r| r.ok()).unwrap_or_default();
        if p.is_empty() {
            return String::new();
        }
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(o) = web_sys::window().and_then(|w| w.location().origin().ok()) {
                return format!("{o}{p}");
            }
        }
        p
    };

    let salin = move |_| {
        let u = url_penuh();
        if u.is_empty() {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(w) = web_sys::window() {
                let _ = w.navigator().clipboard().write_text(&u);
            }
        }
        disalin.set(true);
    };

    view! {
        <div class="ppm-card p-5">
            <div class="flex items-center gap-2 mb-1">
                <span class="material-symbols-outlined text-on-background">"event_repeat"</span>
                <h2 class="text-body-lg font-bold text-on-background">"Langganan Kalender"</h2>
            </div>
            <p class="text-body-sm text-on-surface-variant mb-3">
                "Pasang sekali di Google Calendar — sesudah itu jadwalmu ikut terbarui sendiri, \
                 termasuk kalau ada sesi yang digeser atau libur."
            </p>

            <Suspense fallback=|| {
                view! { <div class="h-10 bg-surface-container rounded-xl animate-pulse"></div> }
            }>
                {move || {
                    let u = url_penuh();
                    (!u.is_empty())
                        .then(|| {
                            view! {
                                <div class="bg-surface-container rounded-xl p-3 space-y-2">
                                    <p class="text-[11px] text-on-surface-variant break-all font-mono">
                                        {u.clone()}
                                    </p>
                                    <button
                                        class="w-full py-2.5 rounded-lg bg-primary text-on-primary text-body-sm font-semibold press cursor-pointer"
                                        on:click=salin
                                    >
                                        {move || {
                                            if disalin.get() { "Tersalin ✓" } else { "Salin alamat" }
                                        }}
                                    </button>
                                </div>
                            }
                        })
                }}
            </Suspense>

            // Petunjuknya dilipat: yang sudah pernah memasang tak perlu membaca
            // ulang enam langkah setiap membuka profilnya.
            <button
                class="w-full flex items-center justify-between pt-3 cursor-pointer"
                on:click=move |_| buka.update(|o| *o = !*o)
                aria-expanded=move || buka.get().to_string()
            >
                <span class="text-body-sm font-semibold text-primary">"Cara memasang"</span>
                <span
                    class="material-symbols-outlined text-on-surface-variant transition-transform"
                    class:rotate-180=move || buka.get()
                >
                    "expand_more"
                </span>
            </button>
            <Show when=move || buka.get() fallback=|| ()>
                <ol class="text-body-sm text-on-surface-variant space-y-1.5 pt-2 list-decimal list-inside">
                    <li>"Salin alamat di atas."</li>
                    <li>
                        "Buka " <span class="font-semibold">"calendar.google.com"</span>
                        " lewat peramban (bukan aplikasi Google Calendar)."
                    </li>
                    <li>
                        "Di kiri, klik tanda " <span class="font-semibold">"+"</span>
                        " di sebelah \"Kalender lain\" → \"Dari URL\"."
                    </li>
                    <li>"Tempel alamatnya, lalu \"Tambahkan kalender\"."</li>
                </ol>
                <p class="text-[11px] text-on-surface-variant pt-2">
                    "Google menarik pembaruan setiap beberapa jam, bukan seketika — untuk kabar \
                     mendadak tetap lihat pengumuman di aplikasi. Jangan bagikan alamat ini: \
                     siapa pun yang memilikinya bisa melihat jadwalmu."
                </p>
                <p class="text-[11px] text-on-surface-variant pt-1">
                    "Aplikasi Google Calendar di HP tidak bisa menambah alamat langsung — pasang \
                     sekali lewat peramban, nanti otomatis muncul juga di HP."
                </p>
            </Show>
        </div>
    }
}
