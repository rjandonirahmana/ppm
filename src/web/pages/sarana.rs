//! web/pages/sarana.rs — Sarana & Prasarana (/sarana).
//!
//! Menjawab empat pertanyaan yang hari ini dijawab dengan berkeliling: pondok
//! punya apa, ada berapa, di mana, dan keadaannya bagaimana. Lihat catatan
//! rancangan di `migration/94_sarana_prasarana.sql`.
//!
//! MEMBACA terbuka untuk seluruh staf; MENGUBAH hanya admin/ketua — gerbangnya
//! di `web/api.rs` (`SARANA_VIEW_ROLES` / `SARANA_MANAGE_ROLES`). Tombol ubah &
//! hapus di sini disembunyikan untuk yang bukan pengelola, tapi itu KERAPIAN,
//! bukan pengamanan: yang menolak tetap server.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{sarana_kondisi_warna, SaranaItem, SARANA_KATEGORI, SARANA_KONDISI};
use crate::web::api::{hapus_sarana_action, sarana_data, simpan_sarana_action};
use crate::web::components::{
    kartu_grid, DeviceFrame, EmptyState, FetchError, MobileHeader, Sheet, Skeleton,
};

/// Baris yang sedang disunting. `id = None` = sedang menambah yang baru.
#[derive(Clone, Default, PartialEq)]
struct Draf {
    id: Option<i64>,
    nama: String,
    kategori: String,
    lokasi: String,
    jumlah: String,
    kondisi: String,
    catatan: String,
}

impl Draf {
    fn baru() -> Self {
        Self {
            kategori: "perabot".into(),
            kondisi: "baik".into(),
            jumlah: "1".into(),
            ..Default::default()
        }
    }

    fn dari(i: &SaranaItem) -> Self {
        Self {
            id: Some(i.id),
            nama: i.nama.clone(),
            kategori: i.kategori.clone(),
            lokasi: i.lokasi.clone(),
            jumlah: i.jumlah.to_string(),
            kondisi: i.kondisi.clone(),
            catatan: i.catatan.clone(),
        }
    }
}

#[component]
pub fn SaranaPage() -> impl IntoView {
    // Penyaring jadi KUNCI resource, bukan disaring di klien seperti
    // `/izin-aktif`. Bedanya disengaja: di sana daftarnya kecil dan tetap, di
    // sini ia tumbuh terus seiring pondok mendata, dan menyaring di server
    // berarti yang dikirim ke ponsel hanya yang memang dilihat.
    let kategori = RwSignal::new(String::new());
    let kondisi = RwSignal::new(String::new());
    let data = Resource::new(
        move || (kategori.get(), kondisi.get()),
        |(kat, kon)| async move { sarana_data(kat, kon).await },
    );

    crate::web::components::guard_sesi(data);

    let draf = RwSignal::new(Option::<Draf>::None);
    let bisa_kelola = RwSignal::new(false);
    let session = use_context::<Resource<Option<crate::models::SessionUser>>>();
    // Dibaca lewat Effect (klien saja) — membaca sesi saat render membuat pohon
    // SSR dan pohon hidrasi berbeda. Pola yang sama dipakai `pages/staf.rs`.
    Effect::new(move |_| {
        let ok = session
            .and_then(|s| s.get())
            .flatten()
            // Cermin `SARANA_MANAGE_ROLES` di server. Ini KERAPIAN — yang
            // menolak tetap server; tapi menawarkan tombol yang pasti ditolak
            // hanya melatih orang mengabaikan pesan galat.
            .map(|u| {
                crate::models::role_satisfies(&u.role, &["admin"])
                    || u.role == "dewan_guru_sarpras"
            })
            .unwrap_or(false);
        if bisa_kelola.get_untracked() != ok {
            bisa_kelola.set(ok);
        }
    });

    view! {
        <Title text="Sarana & Prasarana — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader
                    title="Sarana & Prasarana"
                    subtitle="Inventaris milik pondok"
                    back_href="/staf"
                />

                <div class="px-5 pt-5 space-y-4 stagger">
                    // ── Bilah alat ───────────────────────────────────────────
                    //
                    // `ppm-wide` melebarkan kanvas desktop jadi 72rem (lihat
                    // catatannya di `style/tailwind.css`), dan di lebar itu
                    // `flex-1` maupun `w-full` berubah jadi kolom raksasa: dua
                    // dropdown selebar 35rem dan satu tombol yang membentang
                    // dari tepi ke tepi. Aturan mainnya sudah tertulis di sana —
                    // yang dilebarkan hanya DAFTARNYA; kendali tetap seukuran
                    // tangan. Sama seperti `pages/izin_aktif.rs`.
                    //
                    // Di ponsel susunannya tetap: dua dropdown berdampingan,
                    // tombol di bawah selebar layar. Di desktop ketiganya jadi
                    // satu baris, tombol didorong ke kanan oleh `md:ml-auto`.
                    <div class="space-y-4 md:space-y-0 md:flex md:items-center md:gap-3">
                        <div class="flex gap-2 md:max-w-lg md:flex-1">
                            <select
                                class="flex-1 min-w-0 bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface"
                                on:change=move |ev| kategori.set(event_target_value(&ev))
                            >
                                <option value="">"Semua kategori"</option>
                                {SARANA_KATEGORI
                                    .iter()
                                    .map(|(v, l)| view! { <option value=*v>{*l}</option> })
                                    .collect_view()}
                            </select>
                            <select
                                class="flex-1 min-w-0 bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface"
                                on:change=move |ev| kondisi.set(event_target_value(&ev))
                            >
                                <option value="">"Semua kondisi"</option>
                                {SARANA_KONDISI
                                    .iter()
                                    .map(|(v, l)| view! { <option value=*v>{*l}</option> })
                                    .collect_view()}
                            </select>
                        </div>

                        {move || {
                            bisa_kelola
                                .get()
                                .then(|| {
                                    view! {
                                        <button
                                            class="w-full md:w-auto md:ml-auto md:shrink-0 flex items-center justify-center gap-2 px-5 py-3 rounded-xl bg-primary text-on-primary font-semibold press cursor-pointer"
                                            on:click=move |_| draf.set(Some(Draf::baru()))
                                        >
                                            <span class="material-symbols-outlined text-lg">"add"</span>
                                            "Tambah Sarana"
                                        </button>
                                    }
                                })
                        }}
                    </div>

                    <Suspense fallback=|| view! { <Skeleton baris=4 tinggi="h-24" /> }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                    Ok(d) => {
                                        let r = d.ringkas.clone();
                                        let kosong = d.items.is_empty();
                                        view! {
                                            // Ringkasan dihitung atas SELURUH data, bukan atas
                                            // hasil yang tersaring — lihat `service::sarana::list`.
                                            // Dibatasi di desktop: tiga kartu berisi
                                            // satu angka saja tak menjadi lebih
                                            // terbaca dengan melebar 24rem
                                            // masing-masing — ia hanya menjauhkan
                                            // angkanya dari labelnya.
                                            <div class="grid grid-cols-3 gap-3 md:max-w-xl">
                                                <div class="ppm-card p-4">
                                                    <p class="text-[11px] text-on-surface-variant">"Jenis"</p>
                                                    <p class="text-2xl font-bold text-on-background mt-1">
                                                        {r.jenis}
                                                    </p>
                                                </div>
                                                <div class="ppm-card p-4">
                                                    <p class="text-[11px] text-on-surface-variant">"Unit"</p>
                                                    <p class="text-2xl font-bold text-on-background mt-1">
                                                        {r.unit}
                                                    </p>
                                                </div>
                                                <div class="ppm-card p-4">
                                                    <p class="text-[11px] text-on-surface-variant">"Perlu Perbaikan"</p>
                                                    <p class="text-2xl font-bold text-error mt-1">
                                                        {r.perlu_perbaikan}
                                                    </p>
                                                </div>
                                            </div>

                                            {if kosong {
                                                view! {
                                                    <EmptyState
                                                        icon="inventory_2"
                                                        title="Belum ada data sarana"
                                                        subtitle="Mulai dari yang paling sering ditanyakan: kursi, meja, kipas, dan perlengkapan ibadah."
                                                    />
                                                }
                                                    .into_any()
                                            } else {
                                                kartu_grid(
                                                        d
                                                            .items
                                                            .into_iter()
                                                            .map(|i| {
                                                                view! {
                                                                    <KartuSarana
                                                                        i=i
                                                                        bisa_kelola=bisa_kelola
                                                                        draf=draf
                                                                        refetch=move || data.refetch()
                                                                    />
                                                                }
                                                                    .into_any()
                                                            })
                                                            .collect(),
                                                    )
                                                    .into_any()
                                            }}
                                        }
                                            .into_any()
                                    }
                                })
                        }}
                    </Suspense>
                </div>

                {move || {
                    draf.get()
                        .map(|d| {
                            view! {
                                <FormSarana
                                    awal=d
                                    on_close=move || draf.set(None)
                                    refetch=move || data.refetch()
                                />
                            }
                        })
                }}
            </div>
        </DeviceFrame>
    }
}

#[component]
fn KartuSarana(
    i: SaranaItem,
    bisa_kelola: RwSignal<bool>,
    draf: RwSignal<Option<Draf>>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let id = i.id;
    // `StoredValue` juga untuk namanya, bukan `String` telanjang.
    //
    // Closure `hapus` di bawah dipasang ke `on:click` DI DALAM closure reaktif
    // yang dijalankan berulang. Menangkap sebuah `String` membuat `hapus` tak
    // `Copy`, jadi memasangnya berarti MEMINDAHKANNYA — dan closure yang
    // memindahkan tangkapannya hanya `FnOnce`, sementara sebuah tombol harus
    // bisa diketuk lebih dari sekali. `StoredValue` itu `Copy`, jadi `hapus`
    // ikut `Copy` dan boleh dipasang berkali-kali.
    // Hanya DIBACA di cabang wasm (dialog konfirmasi); di build SSR ia memang
    // tak terpakai. Pola `cfg_attr` yang sama dipakai `pages/students.rs`.
    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
    let nama = StoredValue::new(i.nama.clone());
    // Draf disiapkan DI MUKA dan disimpan di `StoredValue`.
    //
    // Versi pertama meng-clone `SaranaItem` di dalam closure tombol. Itu membuat
    // closure-nya memindahkan nilainya, jadi ia hanya `FnOnce` — sementara
    // penangan `on:click` menuntut `FnMut` (tombolnya bisa diketuk lebih dari
    // sekali). Menariknya galat itu HANYA muncul di target wasm, jadi ia lolos
    // dari `cargo check --features ssr` sepenuhnya.
    //
    // `StoredValue` sendiri `Copy`, jadi tak ada yang perlu dipindahkan: tiap
    // ketukan cukup mengambil salinannya. Pola yang sama dipakai
    // `pages/izin_aktif.rs` untuk alasan yang sama.
    let draf_sunting = StoredValue::new(Draf::dari(&i));
    // Lokasi kosong TIDAK menghasilkan " · " yang menggantung tanpa apa-apa di
    // belakangnya — banyak barang memang tak punya letak tetap.
    let meta = if i.lokasi.is_empty() {
        i.kategori_label.clone()
    } else {
        format!("{} · {}", i.kategori_label, i.lokasi)
    };

    let hapus = move |_| {
        // Konfirmasi karena penghapusannya permanen dan tak ada urungkan.
        #[cfg(target_arch = "wasm32")]
        {
            let pesan = format!("Hapus \"{}\" dari daftar sarana?", nama.get_value());
            let setuju = web_sys::window()
                .and_then(|w| w.confirm_with_message(&pesan).ok())
                .unwrap_or(false);
            if !setuju {
                return;
            }
        }
        leptos::task::spawn_local(async move {
            if hapus_sarana_action(id).await.is_ok() {
                refetch();
            }
        });
    };

    view! {
        <div class="ppm-card p-4">
            <div class="flex items-start justify-between gap-2">
                <div class="min-w-0">
                    <p class="text-body-md font-bold text-on-background truncate">{i.nama}</p>
                    <p class="text-body-sm text-on-surface-variant truncate">{meta}</p>
                </div>
                <span class=sarana_kondisi_warna(&i.kondisi)>{i.kondisi_label}</span>
            </div>
            <div class="flex items-center justify-between mt-3">
                <p class="text-body-sm text-on-surface-variant">
                    <span class="font-bold text-on-background">{i.jumlah}</span>
                    " unit · diperbarui "
                    {i.diperbarui_label}
                </p>
                {move || {
                    bisa_kelola
                        .get()
                        .then(|| {
                            view! {
                                <div class="flex gap-1 shrink-0">
                                    <button
                                        class="w-8 h-8 rounded-full flex items-center justify-center text-on-surface-variant hover:bg-surface-container"
                                        aria-label="Ubah"
                                        on:click=move |_| draf.set(Some(draf_sunting.get_value()))
                                    >
                                        <span class="material-symbols-outlined text-lg">"edit"</span>
                                    </button>
                                    <button
                                        class="w-8 h-8 rounded-full flex items-center justify-center text-error hover:bg-error/10"
                                        aria-label="Hapus"
                                        on:click=hapus
                                    >
                                        <span class="material-symbols-outlined text-lg">"delete"</span>
                                    </button>
                                </div>
                            }
                        })
                }}
            </div>
            {(!i.catatan.is_empty())
                .then(|| {
                    view! {
                        <p class="text-body-sm text-on-surface-variant mt-2 pt-2 border-t border-outline-variant/40">
                            {i.catatan}
                        </p>
                    }
                })}
        </div>
    }
}

#[component]
fn FormSarana(
    awal: Draf,
    on_close: impl Fn() + Copy + Send + Sync + 'static,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let is_edit = awal.id.is_some();
    let id = awal.id;
    let nama = RwSignal::new(awal.nama);
    let kategori = RwSignal::new(awal.kategori);
    let lokasi = RwSignal::new(awal.lokasi);
    let jumlah = RwSignal::new(awal.jumlah);
    let kondisi = RwSignal::new(awal.kondisi);
    let catatan = RwSignal::new(awal.catatan);
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<String>::None);

    let simpan = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        // Jumlah dikirim sebagai angka; isian yang bukan angka jadi 0, dan
        // server menolaknya dengan pesan yang menyebut sebabnya. Menebak 1 di
        // sini akan menyimpan angka yang tak pernah diketik siapa pun.
        let j: i32 = jumlah.get_untracked().trim().parse().unwrap_or(0);
        let (n, kat, lok, kon, cat) = (
            nama.get_untracked(),
            kategori.get_untracked(),
            lokasi.get_untracked(),
            kondisi.get_untracked(),
            catatan.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            match simpan_sarana_action(id, n, kat, lok, j, kon, cat).await {
                Ok(_) => {
                    refetch();
                    on_close();
                }
                Err(e) => msg.set(Some(crate::web::components::pesan_galat(e))),
            }
            busy.set(false);
        });
    };

    let judul = if is_edit { "Ubah Sarana" } else { "Tambah Sarana" };
    let field = "w-full bg-surface-container border-0 rounded-xl px-4 py-3 text-body-sm text-on-surface";

    view! {
        <Sheet title=judul on_close=on_close>
            <form class="space-y-3" method="post" on:submit=simpan>
                {move || {
                    msg.get()
                        .map(|m| {
                            view! {
                                <p class="rounded-xl bg-error/10 px-4 py-3 text-body-sm text-error">
                                    {m}
                                </p>
                            }
                        })
                }}
                <input
                    class=field
                    placeholder="Nama sarana (mis. Kursi santri)"
                    prop:value=move || nama.get()
                    on:input=move |ev| nama.set(event_target_value(&ev))
                />
                <div class="flex gap-2">
                    <select
                        class=field
                        prop:value=move || kategori.get()
                        on:change=move |ev| kategori.set(event_target_value(&ev))
                    >
                        {SARANA_KATEGORI
                            .iter()
                            .map(|(v, l)| view! { <option value=*v>{*l}</option> })
                            .collect_view()}
                    </select>
                    <select
                        class=field
                        prop:value=move || kondisi.get()
                        on:change=move |ev| kondisi.set(event_target_value(&ev))
                    >
                        {SARANA_KONDISI
                            .iter()
                            .map(|(v, l)| view! { <option value=*v>{*l}</option> })
                            .collect_view()}
                    </select>
                </div>
                <div class="flex gap-2">
                    <input
                        class=field
                        placeholder="Lokasi (boleh dikosongkan)"
                        prop:value=move || lokasi.get()
                        on:input=move |ev| lokasi.set(event_target_value(&ev))
                    />
                    <input
                        class=field
                        type="number"
                        min="1"
                        placeholder="Jumlah"
                        prop:value=move || jumlah.get()
                        on:input=move |ev| jumlah.set(event_target_value(&ev))
                    />
                </div>
                <textarea
                    class=field
                    rows="2"
                    placeholder="Catatan (mis. hibah alumni 2024)"
                    prop:value=move || catatan.get()
                    on:input=move |ev| catatan.set(event_target_value(&ev))
                ></textarea>
                <button
                    class="w-full py-3 rounded-xl bg-primary text-on-primary font-semibold press cursor-pointer disabled:opacity-60"
                    prop:disabled=move || busy.get()
                >
                    {move || if busy.get() { "Menyimpan…" } else { "Simpan" }}
                </button>
            </form>
        </Sheet>
    }
}
