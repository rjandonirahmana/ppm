//! web/pages/poin.rs — Pantauan Poin Santri (/poin, staf) + detail satu
//! santri (/poin/:id staf, /poin-saya santri).
//!
//! Tiga hal yang dulu tak ada di layar ini, dan ketiganya membuat halaman ini
//! setengah berguna:
//!
//! 1. **Papannya berhenti di 20 santri.** Query-nya `LIMIT 20` tanpa offset dan
//!    tanpa penanda apa pun — santri ke-21 bukan "belum termuat", melainkan tak
//!    ada jalan memuatnya. Sekarang bergulir tak berujung, pola sama dengan
//!    `/students`.
//! 2. **Penyesuaian poin cuma empat tombol ±1/±5.** Pengurus meminta ANGKA
//!    BEBAS: ketik nilainya, tulis alasannya, selesai. Siapa yang mengubah tetap
//!    tercatat (`point_logs.given_by`) dan kini benar-benar TERBACA di detail.
//! 3. **Nama santri tak bisa diklik.** Sekarang menuju `/poin/:id`: profil
//!    singkat + seluruh buku besar poinnya — kehadiran, penyesuaian pengurus,
//!    sampai baris reset saldo awal semester.
//!
//! Reset saldo awal semester juga pindah ke sini dari `/setelan` (halaman itu
//! dihapus): satu layar dengan saldo yang di-resetnya.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use crate::models::{PointLogItem, PointRow};
use crate::web::api::{
    adjust_points_action, poin_data_action, poin_detail_action, poin_history_page_action,
    poin_page_action, reset_semester_points_action,
};
use crate::web::components::{DeviceFrame, FetchError, FlashMsg, MobileHeader, Skeleton};

/// Sama dengan `service::dashboard::POIN_PER_PAGE`. Klien perlu tahu untuk
/// menyimpulkan "sudah halaman terakhir" dari jumlah baris yang datang.
const PER_HALAMAN: i64 = 20;
/// Sama dengan `service::dashboard::RIWAYAT_POIN_PER_PAGE`.
const RIWAYAT_PER_HALAMAN: i64 = 25;

#[component]
pub fn PoinPage() -> impl IntoView {
    view! { <PoinPageInner /> }
}

// `/poin-dewan` DIHAPUS: halamannya kembar persis dengan `/poin` (isi & hak
// aksesnya ditentukan peran, bukan alamatnya), jadi yang ada hanyalah dua pintu
// menuju satu ruangan — dan satu di antaranya tak pernah disebut navbar mana pun.

#[component]
fn PoinPageInner() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { poin_data_action().await });

    crate::web::components::guard_sesi(data);

    view! {
        <Title text="Pantauan Poin Santri — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Pantauan Poin Santri" subtitle="Papan peringkat & penyesuaian poin" />
                <div class="px-5 pt-5 space-y-5 stagger">
                    <Suspense fallback=|| view! { <Skeleton baris=4 tinggi="h-20" /> }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => {
                                        view! {
                                            <div class="grid grid-cols-2 gap-3 md:max-w-lg">
                                                <div class="ppm-card p-4">
                                                    <p class="text-body-sm text-on-surface-variant">"Total Santri"</p>
                                                    <p class="text-2xl font-bold text-on-background mt-1">
                                                        {d.total_santri}
                                                    </p>
                                                </div>
                                                <div class="ppm-card p-4">
                                                    <p class="text-body-sm text-on-surface-variant">"Rata-rata Poin"</p>
                                                    <p class="text-2xl font-bold text-primary mt-1">{d.avg_points}</p>
                                                </div>
                                            </div>
                                            {d.can_reset.then(|| view! { <ResetSaldoCard refetch=move || data.refetch() /> })}
                                            <PapanPoin
                                                awal=d.top
                                                total=d.total_santri
                                                can_adjust=d.can_adjust
                                            />
                                        }
                                            .into_any()
                                    }
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                })
                        }}
                    </Suspense>
                </div>
            </div>
        </DeviceFrame>
    }
}

// ── Reset saldo awal semester ────────────────────────────────────────────────

/// Tombol reset saldo poin seluruh santri ke 300 (pindahan dari `/setelan`).
///
/// Dua pagar, bukan satu: `confirm()` peramban DAN kalimat yang menyebut
/// akibatnya. Tindakannya menyentuh setiap santri sekaligus dan tak ada tombol
/// urungnya — yang bisa dilakukan hanyalah menyesuaikan ulang satu per satu.
#[component]
fn ResetSaldoCard(refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let do_reset = move |_| {
        if busy.get_untracked() {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let ok = web_sys::window()
                .and_then(|w| {
                    w.confirm_with_message(
                        "Reset saldo poin SEMUA santri ke 300? Tindakan ini untuk awal semester baru dan tidak bisa dibatalkan.",
                    )
                    .ok()
                })
                .unwrap_or(false);
            if !ok {
                return;
            }
        }
        busy.set(true);
        msg.set(None);
        leptos::task::spawn_local(async move {
            match reset_semester_points_action().await {
                Ok(n) => {
                    msg.set(Some((true, format!("Saldo {n} santri direset ke 300."))));
                    refetch();
                }
                Err(e) => msg.set(Some((false, crate::web::components::pesan_galat(e)))),
            }
            busy.set(false);
        });
    };

    view! {
        <div class="ppm-card p-4 space-y-3 md:max-w-lg">
            <div class="flex items-center gap-2">
                <span class="material-symbols-outlined text-primary">"restart_alt"</span>
                <h2 class="text-body-lg font-bold text-on-background">"Awal Semester Baru"</h2>
            </div>
            <p class="text-body-sm text-on-surface-variant">
                "Kembalikan saldo poin SEMUA santri ke 300. Riwayat poin lama tetap tersimpan — resetnya sendiri ikut tercatat sebagai satu baris di riwayat tiap santri."
            </p>
            <FlashMsg pesan=msg />
            <button
                class="px-5 py-2.5 rounded-xl border border-error/40 text-error font-semibold text-body-md cursor-pointer press disabled:opacity-60"
                prop:disabled=move || busy.get()
                on:click=do_reset
            >
                {move || if busy.get() { "Memproses…" } else { "Reset Saldo → 300" }}
            </button>
        </div>
    }
}

// ── Papan poin (gulir tak berujung) ──────────────────────────────────────────

#[component]
fn PapanPoin(
    /// Halaman pertama dari server; sisanya menyusul saat digulir.
    awal: Vec<PointRow>,
    /// COUNT(*) santri di cakupan pemirsa — bukan panjang daftar yang kebetulan
    /// sudah termuat.
    total: i64,
    can_adjust: bool,
) -> impl IntoView {
    let baris = RwSignal::new(awal);
    let memuat = RwSignal::new(false);
    let habis = RwSignal::new(false);
    // Santri yang panel penyesuaiannya sedang terbuka (satu saja).
    let editing = RwSignal::new(Option::<i64>::None);
    let feedback = RwSignal::new(Option::<(bool, String)>::None);

    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
    let ambil = move |offset: i64| {
        if memuat.get_untracked() {
            return;
        }
        memuat.set(true);
        leptos::task::spawn_local(async move {
            match poin_page_action(offset).await {
                Ok(rows) => {
                    // Halaman yang datang lebih pendek dari jatahnya = baris
                    // terakhir. Menunggu halaman KOSONG berarti satu permintaan
                    // sia-sia di tiap daftar.
                    habis.set((rows.len() as i64) < PER_HALAMAN);
                    baris.update(|v| v.extend(rows));
                }
                Err(_) => habis.set(true),
            }
            memuat.set(false);
        });
    };

    // Sentinel gulir: IntersectionObserver, bukan listener `scroll` (yang
    // terakhir menyala puluhan kali per detik dan harus di-throttle sendiri).
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
            // Sengaja dibocorkan: keduanya harus hidup selama halaman terbuka,
            // dan halaman ini tak pernah di-mount ulang tanpa memuat ulang data.
            cb.forget();
            std::mem::forget(obs);
        }
        true
    });

    let apply_delta = move |student_id: i64, delta: i32, reason: String| {
        leptos::task::spawn_local(async move {
            match adjust_points_action(student_id, delta, reason).await {
                Ok(_) => {
                    feedback.set(Some((true, "Poin tersimpan. Tercatat atas nama Anda di riwayat santri.".into())));
                    editing.set(None);
                    // Saldo di baris ini ikut bergeser — perbarui di tempat
                    // ketimbang memuat ulang seluruh daftar (dan membuang
                    // halaman-halaman yang sudah digulir pengguna).
                    baris.update(|v| {
                        if let Some(r) = v.iter_mut().find(|r| r.user_id == student_id) {
                            r.points += delta;
                        }
                    });
                }
                Err(e) => feedback.set(Some((false, crate::web::components::pesan_galat(e)))),
            }
        });
    };

    view! {
        <FlashMsg pesan=feedback />
        <p class="text-body-sm text-on-surface-variant">
            {move || {
                let dimuat = baris.get().len();
                if (dimuat as i64) < total {
                    format!("Menampilkan {dimuat} dari {total} santri")
                } else {
                    format!("Total {total} santri")
                }
            }}
        </p>
        <div class="ppm-card divide-y divide-outline-variant/40 overflow-hidden stagger">
            {move || {
                let list = baris.get();
                if list.is_empty() {
                    return view! {
                        <div class="p-8 text-center text-body-sm text-on-surface-variant">
                            "Belum ada santri di cakupan Anda."
                        </div>
                    }
                        .into_any();
                }
                list.into_iter()
                    .enumerate()
                    .map(|(i, row)| {
                        view! {
                            <PointRowView
                                rank=i + 1
                                row=row
                                can_adjust=can_adjust
                                editing=editing
                                apply_delta=apply_delta
                            />
                        }
                    })
                    .collect_view()
                    .into_any()
            }}
        </div>

        <div node_ref=sentinel class="h-px"></div>
        <Show when=move || memuat.get()>
            <p class="py-4 text-center text-body-sm text-on-surface-variant flex items-center justify-center gap-2">
                <span class="material-symbols-outlined text-[18px] pulse-dot">"sync"</span>
                "Memuat…"
            </p>
        </Show>
        <Show when=move || habis.get() && (baris.get().len() > PER_HALAMAN as usize)>
            <p class="py-4 text-center text-[11px] text-on-surface-variant/70">
                "Semua santri sudah ditampilkan."
            </p>
        </Show>
    }
}

#[component]
fn PointRowView(
    rank: usize,
    row: PointRow,
    can_adjust: bool,
    editing: RwSignal<Option<i64>>,
    apply_delta: impl Fn(i64, i32, String) + Copy + Send + 'static,
) -> impl IntoView {
    let uid = row.user_id;
    let meta = format!(
        "{}{}",
        row.nis.map(|n| format!("NIS: {n} • ")).unwrap_or_default(),
        row.class_name.unwrap_or_else(|| "-".into()),
    );

    view! {
        <div class="p-3 md:px-4 space-y-2 anim-in">
            <div class="flex items-center gap-3">
                // Baris menuju detail: profil + seluruh riwayat poinnya.
                <a
                    href=format!("/poin/{uid}")
                    class="flex-1 min-w-0 flex items-center gap-3 hover:opacity-80"
                >
                    <span class="w-6 h-6 rounded-full bg-primary/10 text-primary text-[11px] font-bold flex items-center justify-center shrink-0">
                        {rank}
                    </span>
                    <span class="w-9 h-9 rounded-full bg-secondary-container flex items-center justify-center text-primary font-bold text-body-sm shrink-0">
                        {row.initial}
                    </span>
                    <span class="flex-1 min-w-0">
                        <span class="block font-semibold text-on-background text-body-sm truncate">
                            {row.name}
                        </span>
                        <span class="block text-[11px] text-on-surface-variant truncate">{meta}</span>
                    </span>
                    <span class="text-lg font-bold text-primary shrink-0">{row.points}</span>
                    <span class="material-symbols-outlined text-on-surface-variant shrink-0 text-[20px]">
                        "chevron_right"
                    </span>
                </a>
                {can_adjust
                    .then(|| {
                        view! {
                            <button
                                class="w-8 h-8 rounded-full flex items-center justify-center text-on-surface-variant hover:bg-surface-container-high shrink-0"
                                aria-label="Sesuaikan poin"
                                on:click=move |_| {
                                    editing.update(|e| *e = if *e == Some(uid) { None } else { Some(uid) })
                                }
                            >
                                <span class="material-symbols-outlined text-lg">"tune"</span>
                            </button>
                        }
                    })}
            </div>
            {move || {
                (editing.get() == Some(uid))
                    .then(|| {
                        // Saldo baris diperbarui lewat `baris` di induk (yang
                        // sekaligus menyusun ulang daftar), jadi tak ada yang
                        // perlu dikerjakan di sini — dua tempat memperbarui
                        // angka yang sama justru berisiko dihitung dua kali.
                        view! { <FormPenyesuaian uid=uid apply_delta=apply_delta on_selesai=|_| {} /> }
                    })
            }}
        </div>
    }
}

/// Form penyesuaian poin: NILAI BEBAS + alasan wajib.
///
/// Tanda dipilih lewat dua tombol (Tambah/Kurangi), bukan diketik sebagai minus
/// di depan angka. Yang terakhir itu sumber kekeliruan yang mahal — "50" dan
/// "-50" berselisih seratus poin, dan bedanya cuma satu karakter yang mudah
/// hilang saat mengetik cepat di ponsel.
#[component]
fn FormPenyesuaian(
    uid: i64,
    apply_delta: impl Fn(i64, i32, String) + Copy + Send + 'static,
    on_selesai: impl Fn(i32) + Copy + Send + 'static,
) -> impl IntoView {
    let tambah = RwSignal::new(false);
    let nilai = RwSignal::new(String::new());
    let alasan = RwSignal::new(String::new());
    let galat = RwSignal::new(Option::<String>::None);

    let simpan = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let n = nilai.get_untracked().trim().parse::<i32>().unwrap_or(0);
        if n <= 0 {
            galat.set(Some("Isi jumlah poin (angka positif).".into()));
            return;
        }
        let a = alasan.get_untracked().trim().to_string();
        if a.chars().count() < 3 {
            galat.set(Some("Tulis alasannya — ikut terbaca di riwayat santri.".into()));
            return;
        }
        galat.set(None);
        let delta = if tambah.get_untracked() { n } else { -n };
        apply_delta(uid, delta, a);
        on_selesai(delta);
        nilai.set(String::new());
        alasan.set(String::new());
    };

    let tombol = move |aktif: bool, isi_tambah: bool| {
        if aktif {
            if isi_tambah {
                "flex-1 py-2 rounded-lg bg-success/15 text-success font-bold text-body-sm border border-success/40"
            } else {
                "flex-1 py-2 rounded-lg bg-error/10 text-error font-bold text-body-sm border border-error/40"
            }
        } else {
            "flex-1 py-2 rounded-lg text-on-surface-variant font-semibold text-body-sm border border-outline-variant"
        }
    };
    let field = "w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface";

    view! {
        <form
            class="bg-surface-container-low rounded-xl p-3 space-y-2"
            method="post"
            on:submit=simpan
        >
            <div class="flex gap-2">
                <button
                    type="button"
                    class=move || tombol(tambah.get(), true)
                    on:click=move |_| tambah.set(true)
                >
                    "Tambah"
                </button>
                <button
                    type="button"
                    class=move || tombol(!tambah.get(), false)
                    on:click=move |_| tambah.set(false)
                >
                    "Kurangi"
                </button>
            </div>
            <input
                type="number"
                min="1"
                max="300"
                inputmode="numeric"
                class=field
                placeholder="Jumlah poin (mis. 25)"
                prop:value=move || nilai.get()
                on:input=move |ev| nilai.set(event_target_value(&ev))
            />
            <input
                type="text"
                class=field
                placeholder="Alasan (mis. Juara lomba tahfidz)"
                prop:value=move || alasan.get()
                on:input=move |ev| alasan.set(event_target_value(&ev))
            />
            {move || {
                galat
                    .get()
                    .map(|g| {
                        view! {
                            <p class="text-[11px] text-error bg-error-container/60 rounded-lg px-3 py-1.5">
                                {g}
                            </p>
                        }
                    })
            }}
            <button
                type="submit"
                class="w-full py-2.5 rounded-lg bg-primary text-on-primary font-semibold text-body-sm press"
            >
                {move || {
                    let n = nilai.get().trim().parse::<i32>().unwrap_or(0);
                    let tanda = if tambah.get() { "+" } else { "−" };
                    if n > 0 { format!("Simpan {tanda}{n} poin") } else { "Simpan Penyesuaian".into() }
                }}
            </button>
            <p class="text-[10px] text-on-surface-variant">
                "Tercatat di riwayat santri lengkap dengan nama Anda dan alasannya."
            </p>
        </form>
    }
}

// ── Detail poin satu santri (/poin/:id) ──────────────────────────────────────

/// Buku besar poin SATU santri, dilihat STAF (`/poin/:id`).
#[component]
pub fn PoinDetailPage() -> impl IntoView {
    let params = use_params_map();
    let student_id = Memo::new(move |_| {
        params.read().get("id").and_then(|s| s.parse::<i64>().ok()).unwrap_or(0)
    });
    view! { <PoinDetailInner student_id=student_id milik_sendiri=false /> }
}

/// Buku besar poin MILIK SENDIRI (`/poin-saya`, santri).
///
/// Id-nya sengaja tidak ada di URL: `student_id = 0` berarti "diri sendiri" dan
/// server yang menerjemahkannya dari sesi (lihat `sasaran_poin` di api.rs).
/// Dengan begitu tak ada angka di alamat yang bisa diganti untuk mengintip poin
/// santri lain — pagarnya bukan sekadar penolakan, melainkan ketiadaan pintu.
#[component]
pub fn PoinSayaPage() -> impl IntoView {
    let student_id = Memo::new(|_| 0_i64);
    view! { <PoinDetailInner student_id=student_id milik_sendiri=true /> }
}

#[component]
fn PoinDetailInner(student_id: Memo<i64>, milik_sendiri: bool) -> impl IntoView {
    let data = Resource::new(
        move || student_id.get(),
        |id| async move { poin_detail_action(id).await },
    );

    crate::web::components::guard_sesi(data);

    let judul = if milik_sendiri { "Riwayat Poin Saya" } else { "Detail Poin Santri" };
    // Santri tak bisa membuka papan poin staf; tombol kembali yang mengarah ke
    // sana hanya berujung "akses ditolak".
    let kembali = if milik_sendiri { "/santri" } else { "/poin" };

    view! {
        <Title text="Riwayat Poin — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title=judul back_href=kembali />
                <div class="px-5 pt-5 space-y-4">
                    <Suspense fallback=|| view! { <Skeleton baris=3 tinggi="h-28" /> }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => {
                                        let riwayat_baru = RwSignal::new(Vec::<PointLogItem>::new());
                                        let uid = d.user_id;
                                        let nis = d.nis.clone();
                                        let angkatan = d.angkatan.clone();
                                        let phone = d.phone.clone();
                                        let kelas = d.classes.clone();
                                        view! {
                                            // ── Hero: identitas + saldo ─────────────
                                            <div class="spiritual-gradient rounded-2xl p-5 text-on-primary shadow-lg shadow-primary/20 anim-in">
                                                <div class="flex items-center gap-3">
                                                    <span class="w-12 h-12 rounded-full bg-white/20 flex items-center justify-center text-body-lg font-bold shrink-0">
                                                        {d.initial}
                                                    </span>
                                                    <div class="min-w-0">
                                                        <p class="text-body-lg font-bold truncate">{d.name}</p>
                                                        <p class="text-body-sm opacity-85">
                                                            {format!("NIS: {nis}")}
                                                            {(!angkatan.is_empty())
                                                                .then(|| format!(" • Angkatan {angkatan}"))}
                                                        </p>
                                                    </div>
                                                </div>
                                                <div class="flex flex-wrap gap-1.5 mt-3">
                                                    {kelas
                                                        .into_iter()
                                                        .map(|k| {
                                                            view! {
                                                                <span class="px-2.5 py-0.5 rounded-full bg-white/15 text-[10px] font-bold">
                                                                    {k}
                                                                </span>
                                                            }
                                                        })
                                                        .collect_view()}
                                                </div>
                                                <div class="mt-4 flex items-end justify-between gap-3">
                                                    <div>
                                                        <p class="text-[11px] tracking-wider opacity-80">"SALDO POIN"</p>
                                                        <p class="text-3xl font-bold">{d.points}</p>
                                                    </div>
                                                    <div class="text-right text-body-sm">
                                                        <p class="opacity-85">{format!("+{} masuk", d.total_plus)}</p>
                                                        <p class="opacity-85">{format!("−{} keluar", d.total_minus)}</p>
                                                    </div>
                                                </div>
                                                {(!phone.is_empty())
                                                    .then(|| {
                                                        view! {
                                                            <p class="text-body-sm opacity-85 flex items-center gap-1 mt-2">
                                                                <span class="material-symbols-outlined text-[15px]">"call"</span>
                                                                {phone}
                                                            </p>
                                                        }
                                                    })}
                                            </div>

                                            // ── Penyesuaian manual ──────────────────
                                            {d
                                                .can_adjust
                                                .then(|| {
                                                    view! {
                                                        <div class="ppm-card p-4 space-y-2 anim-in">
                                                            <div class="flex items-center gap-2">
                                                                <span class="material-symbols-outlined text-primary">"tune"</span>
                                                                <h3 class="text-body-md font-bold text-on-background">
                                                                    "Sesuaikan Poin"
                                                                </h3>
                                                            </div>
                                                            // Setelah tersimpan, seluruh payload diambil
                                                            // ulang: saldo, ringkasan masuk/keluar, DAN
                                                            // baris riwayat yang baru saja lahir semuanya
                                                            // datang dari server — tak ada angka di layar
                                                            // yang ditebak klien lalu meleset dari
                                                            // catatannya.
                                                            <FormPenyesuaian
                                                                uid=uid
                                                                apply_delta=move |id: i64, delta: i32, reason: String| {
                                                                    leptos::task::spawn_local(async move {
                                                                        if adjust_points_action(id, delta, reason).await.is_ok() {
                                                                            data.refetch();
                                                                        }
                                                                    });
                                                                }
                                                                on_selesai=|_| {}
                                                            />
                                                        </div>
                                                    }
                                                })}

                                            // ── Riwayat poin ────────────────────────
                                            // `0` diteruskan apa adanya saat melihat milik
                                            // sendiri: halaman berikutnya harus disasarkan
                                            // server dari sesi, bukan dari id yang dikirim
                                            // balik oleh klien.
                                            <RiwayatPoin
                                                student_id=if milik_sendiri { 0 } else { uid }
                                                awal=d.history
                                                total=d.history_total
                                                tambahan=riwayat_baru
                                            />
                                        }
                                            .into_any()
                                    }
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                })
                        }}
                    </Suspense>
                </div>
            </div>
        </DeviceFrame>
    }
}

#[component]
fn RiwayatPoin(
    student_id: i64,
    awal: Vec<PointLogItem>,
    total: i64,
    /// Halaman-halaman berikutnya. Dipisah dari `awal` supaya `Resource` yang
    /// di-refetch (mis. setelah penyesuaian) tak menghapus yang sudah digulir.
    tambahan: RwSignal<Vec<PointLogItem>>,
) -> impl IntoView {
    let memuat = RwSignal::new(false);
    let habis = RwSignal::new((awal.len() as i64) >= total);
    let awal = StoredValue::new(awal);

    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
    let ambil = move |offset: i64| {
        if memuat.get_untracked() {
            return;
        }
        memuat.set(true);
        leptos::task::spawn_local(async move {
            match poin_history_page_action(student_id, offset).await {
                Ok(rows) => {
                    habis.set((rows.len() as i64) < RIWAYAT_PER_HALAMAN);
                    tambahan.update(|v| v.extend(rows));
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
            if terlihat && !habis.get_untracked() && !memuat.get_untracked() {
                let sudah_ada = awal.with_value(|v| v.len()) + tambahan.get_untracked().len();
                ambil(sudah_ada as i64);
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
        <div class="ppm-card p-4 anim-in">
            <div class="flex items-center justify-between gap-2 mb-1">
                <div class="flex items-center gap-2">
                    <span class="material-symbols-outlined text-primary">"history"</span>
                    <h3 class="text-body-md font-bold text-on-background">"Riwayat Poin"</h3>
                </div>
                <span class="text-[11px] text-on-surface-variant">{format!("{total} catatan")}</span>
            </div>
            <p class="text-[11px] text-on-surface-variant mb-2">
                "Semua yang menggerakkan saldo: kehadiran, penyesuaian pengurus, dan reset awal semester."
            </p>
            {move || {
                let semua: Vec<PointLogItem> = awal
                    .get_value()
                    .into_iter()
                    .chain(tambahan.get())
                    .collect();
                if semua.is_empty() {
                    return view! {
                        <p class="ppm-empty">"Belum ada catatan poin untuk santri ini."</p>
                    }
                        .into_any();
                }
                view! {
                    <div class="divide-y divide-outline-variant/40">
                        {semua.into_iter().map(|it| view! { <BarisRiwayat it=it /> }).collect_view()}
                    </div>
                }
                    .into_any()
            }}
            <div node_ref=sentinel class="h-px"></div>
            <Show when=move || memuat.get()>
                <p class="py-3 text-center text-body-sm text-on-surface-variant flex items-center justify-center gap-2">
                    <span class="material-symbols-outlined text-[18px] pulse-dot">"sync"</span>
                    "Memuat…"
                </p>
            </Show>
        </div>
    }
}

#[component]
fn BarisRiwayat(it: PointLogItem) -> impl IntoView {
    let naik = it.delta > 0;
    let delta_cls = if naik {
        "text-body-md font-bold text-success shrink-0"
    } else {
        "text-body-md font-bold text-error shrink-0"
    };
    let delta_txt = if naik { format!("+{}", it.delta) } else { it.delta.to_string() };
    // Baris yang bukan hasil klik seseorang (kehadiran otomatis, reset saldo,
    // saldo awal) memang tak punya pelaku — disebut apa adanya, bukan dibiarkan
    // kosong sehingga terbaca seperti data yang hilang.
    let oleh = if it.by_label.is_empty() { "Sistem".to_string() } else { it.by_label };

    view! {
        <div class="py-2.5 flex items-start gap-3">
            <div class="min-w-0 flex-1">
                <p class="text-body-sm text-on-background break-words">{it.reason}</p>
                <p class="text-[11px] text-on-surface-variant">
                    {it.when_label} " • " {oleh}
                </p>
            </div>
            <span class=delta_cls>{delta_txt}</span>
        </div>
    }
}
