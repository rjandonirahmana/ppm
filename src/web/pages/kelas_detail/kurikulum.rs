//! Tab KURIKULUM (migrasi 17) — materi yang direncanakan untuk kelas ini.

use leptos::prelude::*;

use crate::models::{
    CurriculumItem, KelasDetail,
};
use crate::web::api::{
    create_curriculum_action,
    delete_curriculum_action, update_curriculum_action,
};
use crate::web::components::{kartu_grid, EmptyState};

// ── Tab KURIKULUM (migrasi 17) ───────────────────────────────────────────────

#[component]
pub(super) fn KurikulumTab(
    class_id: i64,
    d: KelasDetail,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let items = d.curriculum.clone();
    let book_opts = StoredValue::new(d.book_options.clone());

    view! {
        <div class="space-y-3 stagger">
            <div class="md:max-w-md">
                // Kurikulum boleh disusun GURU & PAMONG, bukan admin saja:
                // merekalah yang tahu kelasnya sedang membaca kitab apa dan
                // sampai mana. Yang tetap admin-saja adalah struktur kelasnya
                // (anggota, jadwal, wali/pamong).
                <BuatKurikulumForm class_id=class_id book_options=book_opts refetch=refetch />
            </div>

            {if items.is_empty() {
                view! {
                    <EmptyState
                        icon="menu_book"
                        title="Belum ada materi/kitab"
                        subtitle="Tambahkan cakupan materi kelas ini lewat form di atas."
                    />
                }
                    .into_any()
            } else {
                kartu_grid(
                        items
                            .into_iter()
                            .map(|c| {
                                view! { <KurikulumCard c=c book_options=book_opts refetch=refetch /> }
                                    .into_any()
                            })
                            .collect(),
                    )
                    .into_any()
            }}
        </div>
    }
}

#[component]
fn KurikulumCard(
    c: CurriculumItem,
    book_options: StoredValue<Vec<crate::models::BookItem>>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let cid = c.id;
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let editing = RwSignal::new(false);
    let e_book = RwSignal::new(c.book_id);
    let e_ss = RwSignal::new(c.start_surah);
    let e_su = RwSignal::new(c.start_unit);
    let e_es = RwSignal::new(c.end_surah);
    let e_eu = RwSignal::new(c.end_unit);
    let e_cs = RwSignal::new(c.current_surah);
    let e_cu = RwSignal::new(c.current_unit);

    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let a = e_book.get_untracked();
        let p = (e_cs.get_untracked(), e_cu.get_untracked());
        let r = (e_ss.get_untracked(), e_su.get_untracked(), e_es.get_untracked(), e_eu.get_untracked());
        leptos::task::spawn_local(async move {
            match update_curriculum_action(cid, a, r.0, r.1, r.2, r.3, p.0, p.1).await {
                Ok(_) => {
                    editing.set(false);
                    refetch();
                }
                Err(e) => {
                    msg.set(Some((false, crate::web::components::pesan_galat(e))));
                }
            }
            busy.set(false);
        });
    };

    let del = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            let _ = delete_curriculum_action(cid).await;
            busy.set(false);
            refetch();
        });
    };

    let title_ro = c.title.clone();
    let status_label = c.status_label.clone();
    let pct = c.progress_pct;
    let book_ro = c.book_title.clone();
    // Label rentang sudah disusun di server (mengikuti jenis materi, atau teks
    // lama untuk baris yang belum tertaut) — di sini tinggal ditampilkan.
    let scope_label = c.range_label.clone();
    let belum_tertaut = c.book_id == 0;
    let current_ro = c.current_label.clone();
    // Bar hijau saat khatam — penanda visual bahwa statusnya "Selesai".
    let bar_cls = if pct >= 100 {
        "h-full bg-success bar-grow"
    } else {
        "h-full bg-primary bar-grow"
    };
    let badge = match c.status.as_str() {
        "completed" => "ppm-chip bg-success/10 text-success",
        "upcoming" => "ppm-chip bg-surface-container-highest text-on-surface-variant",
        _ => "ppm-chip bg-primary/10 text-primary",
    };

    view! {
        <div class="ppm-card p-4 card-hover anim-in ppm-accent">
            <div class="flex items-center justify-between gap-2">
                <p class="text-body-md font-bold text-on-background truncate">{title_ro}</p>
                <div class="flex items-center gap-2 shrink-0">
                    <span class=badge>{status_label}</span>
                    <button
                        class="w-8 h-8 rounded-lg bg-surface-container text-primary flex items-center justify-center press"
                        on:click=move |_| editing.update(|e| *e = !*e)
                        aria-label="Edit materi"
                    >
                        <span class="material-symbols-outlined text-[18px]">"edit"</span>
                    </button>
                </div>
            </div>

            {move || {
                msg.get()
                    .map(|(_, t)| {
                        view! {
                            <div class="mt-2 p-2 bg-error-container text-on-error-container rounded-lg text-body-sm anim-in">
                                {t}
                            </div>
                        }
                    })
            }}

            {move || {
                if editing.get() {
                    view! {
                        <form class="mt-3 space-y-2 anim-in" method="post" on:submit=save>
                            <PilihMateri
                                books=book_options
                                book=e_book
                                on_ganti=move || {
                                    e_ss.set(0);
                                    e_su.set(0);
                                    e_es.set(0);
                                    e_eu.set(0);
                                    e_cs.set(0);
                                    e_cu.set(0);
                                }
                            />
                            <RentangMateri
                                books=book_options
                                book=e_book
                                s_surah=e_ss
                                s_unit=e_su
                                e_surah=e_es
                                e_unit=e_eu
                            />
                            <TitikMateri books=book_options book=e_book surah=e_cs unit=e_cu />
                            <div class="grid grid-cols-2 gap-2 pt-1">
                                <button
                                    type="button"
                                    class="py-2.5 rounded-lg border border-outline-variant text-on-surface font-semibold text-body-sm"
                                    on:click=move |_| editing.set(false)
                                >
                                    "Batal"
                                </button>
                                <button
                                    type="submit"
                                    class="py-2.5 rounded-lg bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                                    disabled=move || busy.get()
                                >
                                    {move || if busy.get() { "Menyimpan…" } else { "Simpan" }}
                                </button>
                            </div>
                            <button
                                type="button"
                                class="w-full flex items-center justify-center gap-1.5 py-2 rounded-lg bg-error-container/60 text-error text-body-sm font-semibold press disabled:opacity-60"
                                disabled=move || busy.get()
                                on:click=del
                            >
                                <span class="material-symbols-outlined text-[18px]">"delete"</span>
                                "Hapus Materi"
                            </button>
                        </form>
                    }
                        .into_any()
                } else {
                    view! {
                        <div class="mt-2">
                            {(!book_ro.is_empty())
                                .then(|| {
                                    view! {
                                        <p class="text-body-sm text-primary flex items-center gap-1">
                                            <span class="material-symbols-outlined text-[15px]">"menu_book"</span>
                                            "Materi: " {book_ro.clone()}
                                        </p>
                                    }
                                })}
                            // Baris warisan dari sebelum materi diwajibkan. Ditandai
                            // supaya jelas ia perlu ditautkan, bukan dibiarkan diam.
                            {belum_tertaut
                                .then(|| {
                                    view! {
                                        <p class="text-[11px] text-warning flex items-center gap-1">
                                            <span class="material-symbols-outlined text-[15px]">"link_off"</span>
                                            "Belum tertaut materi — sunting untuk memilihnya."
                                        </p>
                                    }
                                })}
                            {(!scope_label.is_empty())
                                .then(|| {
                                    view! {
                                        <p class="text-body-sm text-on-surface-variant flex items-center gap-1">
                                            <span class="material-symbols-outlined text-[15px]">"straighten"</span>
                                            {scope_label.clone()}
                                        </p>
                                    }
                                })}
                            // Posisi terakhir — inilah SATU-SATUNYA angka yang
                            // diisi tangan; persen & status di bawah turunannya.
                            {if current_ro.is_empty() {
                                view! {
                                    <p class="text-body-sm text-on-surface-variant flex items-center gap-1">
                                        <span class="material-symbols-outlined text-[15px]">"more_horiz"</span>
                                        "Belum mulai — isi posisi lewat tombol sunting."
                                    </p>
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <p class="text-body-sm text-on-background font-semibold flex items-center gap-1">
                                        <span class="material-symbols-outlined text-[15px] text-primary">
                                            "trending_flat"
                                        </span>
                                        "Sudah sampai " {current_ro.clone()}
                                    </p>
                                }
                                    .into_any()
                            }}
                            <div class="flex items-center justify-between text-xs font-semibold mt-2.5">
                                <span class="text-on-surface-variant">"Progres"</span>
                                <span class="text-on-background">{format!("{pct}%")}</span>
                            </div>
                            <div class="h-2 bg-surface-container rounded-full overflow-hidden mt-1">
                                <div class=bar_cls style=format!("width: {pct}%")></div>
                            </div>
                            <p class="text-[10px] text-on-surface-variant mt-1">
                                "Persen & status dihitung otomatis dari posisi di atas."
                            </p>
                        </div>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}

#[component]
fn BuatKurikulumForm(
    class_id: i64,
    book_options: StoredValue<Vec<crate::models::BookItem>>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let book = RwSignal::new(0i64);
    let ss = RwSignal::new(0i32);
    let su = RwSignal::new(0i32);
    let es = RwSignal::new(0i32);
    let eu = RwSignal::new(0i32);
    let cs = RwSignal::new(0i32);
    let cu = RwSignal::new(0i32);
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let a = book.get_untracked();
        let p = (cs.get_untracked(), cu.get_untracked());
        let r = (ss.get_untracked(), su.get_untracked(), es.get_untracked(), eu.get_untracked());
        leptos::task::spawn_local(async move {
            match create_curriculum_action(class_id, a, r.0, r.1, r.2, r.3, p.0, p.1).await {
                Ok(_) => {
                    msg.set(Some((true, "Materi ditambahkan.".into())));
                    book.set(0);
                    ss.set(0);
                    su.set(0);
                    es.set(0);
                    eu.set(0);
                    refetch();
                }
                Err(e) => {
                    msg.set(Some((false, crate::web::components::pesan_galat(e))));
                }
            }
            busy.set(false);
        });
    };

    view! {
        {move || {
            if !open.get() {
                return view! {
                    <button
                        class="w-full py-3.5 rounded-2xl bg-primary text-on-primary font-bold flex items-center justify-center gap-2 press"
                        on:click=move |_| open.set(true)
                    >
                        <span class="material-symbols-outlined">"add"</span>
                        "Tambah Materi/Kitab"
                    </button>
                }
                    .into_any();
            }
            view! {
                <form class="ppm-card p-4 space-y-3 anim-in" method="post" on:submit=submit>
                    <h3 class="text-body-md font-bold text-on-background">"Materi/Kitab Baru"</h3>
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
                    // Judul & keterangan TIDAK diketik di sini — semuanya ikut
                    // dari materi yang dipilih (books), supaya tak ada dua
                    // versi judul untuk kitab yang sama.
                    <PilihMateri
                        books=book_options
                        book=book
                        on_ganti=move || {
                            ss.set(0);
                            su.set(0);
                            es.set(0);
                            eu.set(0);
                            cs.set(0);
                            cu.set(0);
                        }
                    />
                    <RentangMateri
                        books=book_options
                        book=book
                        s_surah=ss
                        s_unit=su
                        e_surah=es
                        e_unit=eu
                    />
                    <TitikMateri books=book_options book=book surah=cs unit=cu />
                    <div class="grid grid-cols-2 gap-3">
                        <button
                            type="button"
                            class="py-3 rounded-xl border border-outline-variant text-on-surface font-semibold text-body-sm"
                            on:click=move |_| open.set(false)
                        >
                            "Batal"
                        </button>
                        <button
                            type="submit"
                            class="py-3 rounded-xl bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                            disabled=move || busy.get()
                        >
                            {move || if busy.get() { "Menyimpan…" } else { "Simpan" }}
                        </button>
                    </div>
                </form>
            }
                .into_any()
        }}
    }
}

// ── Bidang materi & rentang (migrasi 57) ─────────────────────────────────────

const FIELD: &str =
    "w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface";

/// Pemilih materi terdaftar — WAJIB diisi untuk kurikulum baru.
///
/// Mengganti materi ikut MENGOSONGKAN rentangnya: angka halaman/ayat hanya
/// bermakna terhadap materi tertentu, dan membiarkannya berpindah kitab berarti
/// menyimpan rentang yang sudah pasti salah.
#[component]
pub(super) fn PilihMateri(
    books: StoredValue<Vec<crate::models::BookItem>>,
    book: RwSignal<i64>,
    /// Dipanggil saat materi BERGANTI — pemanggil mengosongkan angka miliknya
    /// sendiri. Lewat callback, bukan daftar sinyal tetap, karena tiap
    /// pemanggil punya angka berbeda: kurikulum punya rentang + posisi,
    /// jadwal tak punya angka sama sekali.
    on_ganti: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    view! {
        <label class="space-y-1 block">
            <span class="text-[11px] text-on-surface-variant">"Materi terdaftar"</span>
            <select
                class=FIELD
                on:change=move |ev| {
                    let baru = event_target_value(&ev).parse().unwrap_or(0);
                    if baru != book.get_untracked() {
                        on_ganti();
                    }
                    book.set(baru);
                }
            >
                <option value="0" selected=move || book.get() == 0>"— Pilih materi —"</option>
                {books
                    .get_value()
                    .into_iter()
                    .map(|b| {
                        let id = b.id;
                        let ket = if b.category == "quran" {
                            format!("{} · {} surat", b.title, b.surahs.len())
                        } else {
                            format!("{} · {} halaman", b.title, b.total_pages)
                        };
                        view! {
                            <option value=id.to_string() selected=move || book.get() == id>
                                {ket}
                            </option>
                        }
                    })
                    .collect_view()}
            </select>
        </label>
    }
}

/// Satu ujung rentang: [surat ▾][ayat] untuk Qur'an, [halaman] untuk Hadist.
#[component]
pub(super) fn UjungRentang(
    label: &'static str,
    surahs: Vec<crate::models::Surah>,
    quran: bool,
    maks_halaman: i32,
    surah: RwSignal<i32>,
    unit: RwSignal<i32>,
) -> impl IntoView {
    // Batas ayat mengikuti surat yang sedang dipilih, bukan angka tetap —
    // tiap surat panjangnya beda.
    let maks_ayat = {
        let surahs = surahs.clone();
        move || {
            let i = surah.get().max(1) as usize;
            surahs.get(i - 1).map(|s: &crate::models::Surah| s.ayat).unwrap_or(0)
        }
    };
    view! {
        <div class="space-y-1">
            <span class="text-[11px] text-on-surface-variant">{label}</span>
            {if quran {
                view! {
                    <div class="grid grid-cols-2 gap-1.5">
                        <select
                            class=FIELD
                            on:change=move |ev| surah.set(event_target_value(&ev).parse().unwrap_or(0))
                        >
                            {surahs
                                .clone()
                                .into_iter()
                                .enumerate()
                                .map(|(i, s)| {
                                    let idx = i as i32 + 1;
                                    view! {
                                        <option
                                            value=idx.to_string()
                                            selected=move || surah.get().max(1) == idx
                                        >
                                            {s.name}
                                        </option>
                                    }
                                })
                                .collect_view()}
                        </select>
                        <input
                            type="number"
                            min="1"
                            class=FIELD
                            placeholder="Ayat"
                            prop:max=maks_ayat
                            prop:value=move || {
                                let u = unit.get();
                                if u > 0 { u.to_string() } else { String::new() }
                            }
                            on:input=move |ev| unit.set(event_target_value(&ev).parse().unwrap_or(0))
                        />
                    </div>
                }
                    .into_any()
            } else {
                view! {
                    <input
                        type="number"
                        min="1"
                        max=maks_halaman.to_string()
                        class=FIELD
                        placeholder="Halaman"
                        prop:value=move || {
                            let u = unit.get();
                            if u > 0 { u.to_string() } else { String::new() }
                        }
                        on:input=move |ev| unit.set(event_target_value(&ev).parse().unwrap_or(0))
                    />
                }
                    .into_any()
            }}
        </div>
    }
}

/// Rentang materi — bentuknya MENGIKUTI jenis materi yang sedang dipilih.
/// Kosongkan keduanya = seluruh materi.
#[component]
pub(super) fn RentangMateri(
    books: StoredValue<Vec<crate::models::BookItem>>,
    book: RwSignal<i64>,
    s_surah: RwSignal<i32>,
    s_unit: RwSignal<i32>,
    e_surah: RwSignal<i32>,
    e_unit: RwSignal<i32>,
) -> impl IntoView {
    move || {
        let id = book.get();
        let Some(b) = books.get_value().into_iter().find(|b| b.id == id) else {
            return view! {
                <p class="text-[11px] text-on-surface-variant">
                    "Pilih materi dulu untuk menentukan rentang halaman/ayat."
                </p>
            }
                .into_any();
        };
        let quran = b.category == "quran";
        let surahs = b.surahs.clone();
        let total = b.total_pages;
        view! {
            <div class="space-y-1.5">
                <div class="grid grid-cols-2 gap-2">
                    <UjungRentang
                        label="Dari"
                        surahs=surahs.clone()
                        quran=quran
                        maks_halaman=total
                        surah=s_surah
                        unit=s_unit
                    />
                    <UjungRentang
                        label="Sampai"
                        surahs=surahs
                        quran=quran
                        maks_halaman=total
                        surah=e_surah
                        unit=e_unit
                    />
                </div>
                <p class="text-[10px] text-on-surface-variant">
                    {if quran {
                        "Boleh melintasi surat. Kosongkan keduanya = seluruh materi.".to_string()
                    } else {
                        format!("Materi ini {total} halaman. Kosongkan keduanya = seluruh materi.")
                    }}
                </p>
            </div>
        }
            .into_any()
    }
}

/// SATU titik posisi (bukan rentang) — dipakai penanda "sedang berjalan" di
/// kartu jadwal. Bentuknya mengikuti jenis materi, sama seperti ujung rentang.
#[component]
pub(super) fn TitikMateri(
    books: StoredValue<Vec<crate::models::BookItem>>,
    book: RwSignal<i64>,
    surah: RwSignal<i32>,
    unit: RwSignal<i32>,
) -> impl IntoView {
    move || {
        let id = book.get();
        let Some(b) = books.get_value().into_iter().find(|b| b.id == id) else {
            return view! {
                <p class="text-[11px] text-on-surface-variant">
                    "Pilih materi dulu untuk menandai posisinya."
                </p>
            }
                .into_any();
        };
        let quran = b.category == "quran";
        view! {
            <div class="space-y-1.5">
                <UjungRentang
                    label="Posisi sekarang"
                    surahs=b.surahs.clone()
                    quran=quran
                    maks_halaman=b.total_pages
                    surah=surah
                    unit=unit
                />
                <p class="text-[10px] text-on-surface-variant">
                    "Kosongkan angkanya bila materi sudah dipilih tapi belum mulai."
                </p>
            </div>
        }
            .into_any()
    }
}
