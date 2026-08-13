//! web/pages/tamu_masuk.rs — Tinjau Tamu (/tamu-masuk), peran PENJAGA.
//!
//! Buku tamu sudah berjalan sejak migrasi 35: tamu mengisi /tamu, dapat kode 6
//! digit, lalu mengetiknya di mesin gerbang yang memotret wajahnya. Baris yang
//! lahir dari situ TAK PERNAH DIBACA siapa pun — datanya terkumpul rapi selama
//! ini dan tak ada satu layar pun yang menampilkannya.
//!
//! Halaman ini pembacanya. Pekerjaan penjaga cuma satu: melihat wajah yang
//! terpotret di sebelah nama dan keperluan yang diketik, lalu menyatakan cocok
//! atau menuliskan apa yang janggal.
//!
//! KENAPA TAK ADA TOMBOL "TOLAK". Tamunya sudah masuk — fotonya bukti ia sudah
//! berdiri di gerbang. Yang bisa dilakukan penjaga bukan membatalkan kejadian,
//! melainkan MENCATAT bahwa datanya tak cocok supaya pengurus menindaklanjuti.
//! Tombol yang menjanjikan pembatalan akan berbohong.
//!
//! ADMIN & KETUA juga membuka halaman ini (server sudah mengizinkannya sejak
//! awal lewat `TAMU_REVIEW_ROLES`; yang kurang cuma jalan masuknya — kini ada
//! petak "Buku Tamu" di /staf). Mereka membacanya sebagai RIWAYAT, bukan
//! antrean, jadi ada penyaring rentang waktu dan daftarnya bergulir tak
//! berujung — buku tamu hanya tumbuh, dan versi pertama yang `LIMIT 100` tanpa
//! offset membuat kunjungan ke-101 mustahil dilihat dari mana pun.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{TamuMasukItem, RENTANG_TAMU};
use crate::web::api::{periksa_tamu_action, tamu_masuk_data, tamu_masuk_page};
use crate::web::components::{
    kartu_grid, DeviceFrame, EmptyState, FetchError, FlashMsg, MediaFrame, MobileHeader,
};

/// Sama dengan `service::guest::TAMU_PER_PAGE` — klien memakainya untuk
/// menyimpulkan "sudah halaman terakhir" dari jumlah baris yang datang.
const PER_HALAMAN: i64 = 20;

#[component]
pub fn TamuMasukPage() -> impl IntoView {
    // Bawaannya HANYA yang belum diperiksa: itulah pekerjaan yang menunggu.
    // Riwayat lengkap ada di balik satu ketukan, bukan sebaliknya.
    let hanya_belum = RwSignal::new(true);
    // Bawaan "30 hari": penjaga hanya peduli hari ini, tapi admin yang membuka
    // riwayat hampir selalu mencari kunjungan beberapa pekan terakhir — dan
    // memuat SELURUH buku tamu sejak awal hanya untuk itu adalah pemborosan
    // yang bertambah tiap bulan.
    let rentang = RwSignal::new("30".to_string());
    let data = Resource::new(
        move || (hanya_belum.get(), rentang.get()),
        |(belum, r)| async move { tamu_masuk_data(belum, r).await },
    );

    crate::web::components::guard_sesi(data);

    let msg = RwSignal::new(Option::<(bool, String)>::None);

    view! {
        <Title text="Tinjau Tamu — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Tinjau Tamu" subtitle="Cocokkan data tamu dengan wajahnya" />

                <div class="px-5 pt-5 space-y-4 stagger">
                    <FlashMsg pesan=msg />

                    // ── Rentang waktu ────────────────────────────────────
                    <div class="flex gap-1.5 overflow-x-auto pb-0.5">
                        {RENTANG_TAMU
                            .iter()
                            .map(|(nilai, label)| {
                                let v = *nilai;
                                let aktif = move || rentang.get() == v;
                                view! {
                                    <button
                                        class=move || {
                                            if aktif() {
                                                "px-3.5 py-1.5 rounded-full bg-primary text-on-primary text-body-sm font-semibold whitespace-nowrap shrink-0 press"
                                            } else {
                                                "px-3.5 py-1.5 rounded-full bg-surface-container text-on-surface-variant text-body-sm font-medium whitespace-nowrap shrink-0 press"
                                            }
                                        }
                                        on:click=move |_| rentang.set(v.to_string())
                                    >
                                        {*label}
                                    </button>
                                }
                            })
                            .collect_view()}
                    </div>

                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="h-20 bg-surface-container rounded-2xl"></div>
                                <div class="grid gap-3 md:grid-cols-2">
                                    <div class="h-64 bg-surface-container rounded-2xl"></div>
                                    <div class="h-64 bg-surface-container rounded-2xl hidden md:block"></div>
                                </div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                    Ok(d) => {
                                        let kosong = d.items.is_empty();
                                        view! {
                                            <div class="ppm-card p-4 flex items-center justify-between gap-3">
                                                <div>
                                                    <div class="flex items-center gap-2 text-warning">
                                                        <span class="material-symbols-outlined">
                                                            "pending_actions"
                                                        </span>
                                                        <span class="text-2xl font-bold text-on-background">
                                                            {d.belum_diperiksa}
                                                        </span>
                                                    </div>
                                                    <p class="text-body-sm text-on-surface-variant mt-1">
                                                        "Menunggu diperiksa · " {d.rentang_label.clone()}
                                                    </p>
                                                </div>
                                                // Tombol, bukan tab: hanya ada dua keadaan, dan
                                                // yang satu jelas lebih sering dipakai.
                                                <button
                                                    class="px-4 py-2.5 rounded-xl border border-outline-variant text-body-sm font-semibold text-on-surface hover:border-primary hover:text-primary transition-colors cursor-pointer"
                                                    on:click=move |_| hanya_belum.update(|b| *b = !*b)
                                                >
                                                    {move || {
                                                        if hanya_belum.get() {
                                                            "Lihat semua"
                                                        } else {
                                                            "Yang belum saja"
                                                        }
                                                    }}
                                                </button>
                                            </div>

                                            {if kosong {
                                                view! {
                                                    <EmptyState
                                                        icon="task_alt"
                                                        title="Tidak ada tamu di rentang ini"
                                                        subtitle="Coba lebarkan rentang waktunya, atau semua kunjungan memang sudah diperiksa."
                                                    />
                                                }
                                                    .into_any()
                                            } else {
                                                view! {
                                                    <DaftarTamu
                                                        awal=d.items
                                                        total=d.total
                                                        hanya_belum=hanya_belum
                                                        rentang=rentang
                                                        msg=msg
                                                        refetch=move || data.refetch()
                                                    />
                                                }
                                                    .into_any()
                                            }}
                                        }
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

#[component]
fn KartuTamu(
    t: TamuMasukItem,
    msg: RwSignal<Option<(bool, String)>>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let id = t.id;
    let busy = RwSignal::new(false);
    let catatan = RwSignal::new(String::new());
    let sudah = t.diperiksa;
    let ada_foto = !t.face_url.is_empty();

    let kirim = move |c: String| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        leptos::task::spawn_local(async move {
            match periksa_tamu_action(id, c).await {
                Ok(_) => msg.set(Some((true, "Tercatat. Terima kasih.".into()))),
                Err(e) => msg.set(Some((false, crate::web::components::pesan_galat(e)))),
            }
            busy.set(false);
            refetch();
        });
    };

    view! {
        <div class="ppm-card p-4 space-y-3 anim-in">
            // Wajah lebih dulu, besar. Inilah yang dibandingkan penjaga dengan
            // orang di depannya; menaruhnya sebagai lampiran kecil di bawah
            // membalik urutan pekerjaannya.
            {if ada_foto {
                view! {
                    <MediaFrame
                        src=t.face_url.clone()
                        style="width:100%;height:100%;object-fit:cover".to_string()
                        video=false
                        backdrop=false
                        alt=format!("Wajah tamu {}", t.name)
                        class="rounded-xl aspect-[4/3] bg-surface-container"
                        lazy=true
                    />
                }
                    .into_any()
            } else {
                // Tanpa foto justru baris yang PALING perlu diperiksa — mesin
                // tak sempat memotret, jadi tak ada bukti siapa yang masuk.
                view! {
                    <div class="rounded-xl aspect-[4/3] bg-warning/10 flex flex-col items-center justify-center gap-1 text-warning">
                        <span class="material-symbols-outlined text-3xl">"no_photography"</span>
                        <p class="text-body-sm font-semibold">"Tanpa foto wajah"</p>
                        <p class="text-[11px] text-on-surface-variant px-4 text-center">
                            "Mesin gerbang tak sempat memotret. Pastikan langsung ke orangnya."
                        </p>
                    </div>
                }
                    .into_any()
            }}

            <div class="min-w-0">
                <p class="text-body-lg font-bold text-on-background truncate">{t.name}</p>
                <p class="text-body-sm text-on-surface-variant">{t.phone}</p>
                {(!t.purpose.trim().is_empty())
                    .then(|| {
                        view! {
                            <p class="text-body-sm text-on-surface mt-1">
                                "Keperluan: "
                                {t.purpose.clone()}
                            </p>
                        }
                    })}
                <p class="text-[11px] text-on-surface-variant mt-1">{t.waktu_label}</p>
            </div>

            {if sudah {
                let oleh = t.diperiksa_oleh.clone();
                let cat = t.catatan.clone();
                view! {
                    <div class="rounded-xl p-3 bg-surface-container">
                        <p class="text-body-sm font-semibold text-on-background">
                            {if cat.trim().is_empty() { "Data cocok" } else { "Ada catatan" }}
                        </p>
                        {(!cat.trim().is_empty())
                            .then(|| {
                                view! { <p class="text-body-sm text-error mt-1">{cat.clone()}</p> }
                            })}
                        {(!oleh.trim().is_empty())
                            .then(|| {
                                view! {
                                    <p class="text-[11px] text-on-surface-variant mt-1">
                                        "Diperiksa " {oleh.clone()}
                                    </p>
                                }
                            })}
                    </div>
                }
                    .into_any()
            } else {
                view! {
                    <div class="space-y-2">
                        <textarea
                            class="w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface"
                            rows="2"
                            placeholder="Kosongkan bila data cocok. Isi bila ada yang janggal."
                            prop:value=move || catatan.get()
                            on:input=move |ev| catatan.set(event_target_value(&ev))
                            aria-label="Catatan kejanggalan"
                        ></textarea>
                        <div class="grid grid-cols-2 gap-2">
                            <button
                                class="py-2.5 rounded-xl border border-outline-variant text-body-sm font-semibold text-error hover:border-error transition-colors cursor-pointer press disabled:opacity-60"
                                prop:disabled=move || busy.get() || catatan.get().trim().is_empty()
                                on:click=move |_| kirim(catatan.get_untracked())
                            >
                                {move || if busy.get() { "Menyimpan…" } else { "Ada yang janggal" }}
                            </button>
                            <button
                                class="py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-sm cursor-pointer press disabled:opacity-60"
                                prop:disabled=move || busy.get()
                                on:click=move |_| kirim(String::new())
                            >
                                {move || if busy.get() { "Menyimpan…" } else { "Data cocok" }}
                            </button>
                        </div>
                        <p class="text-[11px] text-on-surface-variant">
                            "Tombol kiri aktif setelah catatan diisi — catatan kosong berarti data cocok."
                        </p>
                    </div>
                }
                    .into_any()
            }}
        </div>
    }
}

/// Daftar kunjungan yang BERTAMBAH saat digulir.
///
/// Pola sama dengan `/students` dan `/poin`: sentinel setinggi 1px di dasar
/// daftar, diamati `IntersectionObserver` (bukan listener `scroll`, yang menyala
/// puluhan kali per detik dan harus di-throttle sendiri).
///
/// Halaman pertama datang bersama payload utama; komponen ini hanya menyusul
/// sisanya. Saat penyaring berubah, `Suspense` di induk membangun ulang
/// komponen ini dari nol — jadi tumpukan lamanya ikut hilang dengan sendirinya,
/// tanpa perlu efek pembersih.
#[component]
fn DaftarTamu(
    awal: Vec<TamuMasukItem>,
    total: i64,
    hanya_belum: RwSignal<bool>,
    rentang: RwSignal<String>,
    msg: RwSignal<Option<(bool, String)>>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let baris = RwSignal::new(awal);
    let memuat = RwSignal::new(false);
    let habis = RwSignal::new(false);

    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
    let ambil = move |offset: i64| {
        if memuat.get_untracked() {
            return;
        }
        memuat.set(true);
        let (belum, r) = (hanya_belum.get_untracked(), rentang.get_untracked());
        leptos::task::spawn_local(async move {
            match tamu_masuk_page(belum, r, offset).await {
                Ok(rows) => {
                    habis.set((rows.len() as i64) < PER_HALAMAN);
                    baris.update(|v| v.extend(rows));
                }
                Err(_) => habis.set(true),
            }
            memuat.set(false);
        });
    };

    let sentinel: NodeRef<leptos::html::Div> = NodeRef::new();
    #[cfg(target_arch = "wasm32")]
    Effect::new(move |sudah: Option<bool>| {
        if sudah == Some(true) {
            return true;
        }
        let Some(el) = sentinel.get() else { return false };
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;
        let cb = Closure::<dyn FnMut(js_sys::Array)>::new(move |entries: js_sys::Array| {
            let terlihat = entries.iter().any(|e| {
                e.dyn_into::<web_sys::IntersectionObserverEntry>()
                    .map(|e| e.is_intersecting())
                    .unwrap_or(false)
            });
            // Sentinel tetap terlihat SELAMA pemuatan; tanpa penjagaan ini ia
            // menembakkan permintaan beruntun untuk offset yang sama.
            if terlihat && !habis.get_untracked() && !memuat.get_untracked() {
                ambil(baris.get_untracked().len() as i64);
            }
        });
        let opts = web_sys::IntersectionObserverInit::new();
        opts.set_root_margin("400px");
        if let Ok(obs) =
            web_sys::IntersectionObserver::new_with_options(cb.as_ref().unchecked_ref(), &opts)
        {
            obs.observe(&el);
            cb.forget();
            std::mem::forget(obs);
        }
        true
    });

    view! {
        <p class="text-body-sm text-on-surface-variant">
            {move || {
                let dimuat = baris.get().len();
                if (dimuat as i64) < total {
                    format!("Menampilkan {dimuat} dari {total} kunjungan")
                } else {
                    format!("{total} kunjungan")
                }
            }}
        </p>
        {move || {
            kartu_grid(
                baris
                    .get()
                    .into_iter()
                    .map(|t| view! { <KartuTamu t=t msg=msg refetch=refetch /> }.into_any())
                    .collect::<Vec<_>>(),
            )
        }}
        <div node_ref=sentinel class="h-px"></div>
        <Show when=move || memuat.get()>
            <p class="py-4 text-center text-body-sm text-on-surface-variant flex items-center justify-center gap-2">
                <span class="material-symbols-outlined text-[18px] pulse-dot">"sync"</span>
                "Memuat…"
            </p>
        </Show>
        <Show when=move || habis.get() && (baris.get().len() as i64 > PER_HALAMAN)>
            <p class="py-4 text-center text-[11px] text-on-surface-variant/70">
                "Semua kunjungan di rentang ini sudah ditampilkan."
            </p>
        </Show>
    }
}
