//! web/pages/kelas.rs — Manajemen Kelas (admin/dewan guru/pamong).
//!
//! Kelola kurikulum & pembagian santri: statistik total kelas/santri, cari
//! kelas, buat kelas baru, dan buka detail tiap kelas ("Lihat Santri" →
//! /kelas/:id untuk anggota, jadwal, sesi). Data ASLI dari DB.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{BookItem, KelasItem, SessionUser, StudentAcademicItem};
use crate::web::api::{
    academic_audit_data, books_list, create_book_action, create_class_action, delete_book_action,
    kelas_list,
};
use crate::web::components::{DeviceFrame, FetchError, MobileHeader};
use crate::web::pages::SesiContent;

#[component]
pub fn KelasPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { kelas_list().await });
    let session = use_context::<Resource<Option<SessionUser>>>();

    Effect::new(move |_| {
        if let Some(Err(e)) = data.get() {
            // Hanya lempar ke login bila BELUM login (unauth). `forbidden` (login
            // tapi peran tak diizinkan) ditangani FetchError, bukan bounce ke login.
            if e.to_string().contains("unauth") {
                #[cfg(target_arch = "wasm32")]
                if let Some(w) = web_sys::window() {
                    let _ = w.location().replace("/login");
                }
            }
        }
    });

    let query = RwSignal::new(String::new());
    let show_form = RwSignal::new(false);
    // "kelas" | "sesi" | "akademik" | "buku" — kurikulum digabung satu
    // nav/halaman utk staf (dulu Kelas/Sesi terpisah; Akademik & Buku pindah
    // dari /students agar seluruh pengelolaan kurikulum ada di satu tempat).
    let tab = RwSignal::new("kelas".to_string());

    view! {
        <Title text="Manajemen Kelas — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Manajemen Kelas" subtitle="Kelola kurikulum & pembagian santri" />

                <div class="px-5 pt-5 space-y-4 stagger">
                    // grid-cols-2 di mobile (2×2, SEMUA tab selalu terlihat — dulu
                    // "flex-1 + overflow-x-auto" bisa mendorong tab ke-4 (Buku) di
                    // luar layar tanpa indikasi bisa di-scroll, terkesan hilang) →
                    // md:flex 1 baris di desktop (cukup lebar).
                    <div class="grid grid-cols-2 md:flex gap-1 bg-surface-container rounded-xl p-1">
                        <KelasTabBtn tab=tab value="kelas" label="Kelas" />
                        <KelasTabBtn tab=tab value="sesi" label="Sesi" />
                        <KelasTabBtn tab=tab value="akademik" label="Akademik" />
                        // "Buku" utk admin/pamong/guru/dewan guru (kelola daftar buku
                        // hafalan — guru & dewan guru kini SAMA dgn admin) — baca role
                        // dari context sesi GLOBAL (bukan resource kelas_list ini)
                        // supaya tak menunggu data kelas dulu. WAJIB Transition (baca
                        // Resource sesi harus di dalam Suspense/Transition).
                        <Transition fallback=|| ()>
                            {move || {
                                let can_manage = session
                                    .and_then(|s| s.get())
                                    .flatten()
                                    .map(|u| {
                                        matches!(
                                            u.role.as_str(),
                                            "admin" | "supervisor" | "teacher" | "dewan_guru"
                                        )
                                    })
                                    .unwrap_or(false);
                                can_manage.then(|| view! { <KelasTabBtn tab=tab value="buku" label="Materi" /> })
                            }}
                        </Transition>
                    </div>

                    {move || {
                        (tab.get() == "sesi").then(|| view! { <SesiContent /> })
                    }}

                    {move || {
                        (tab.get() == "akademik").then(|| view! { <AcademicAuditTab /> })
                    }}

                    {move || {
                        (tab.get() == "buku")
                            .then(|| {
                                view! {
                                    <Transition fallback=|| ()>
                                        {move || {
                                            let can_manage = session
                                                .and_then(|s| s.get())
                                                .flatten()
                                                .map(|u| {
                                                    matches!(
                                                        u.role.as_str(),
                                                        "admin" | "supervisor" | "teacher" | "dewan_guru"
                                                    )
                                                })
                                                .unwrap_or(false);
                                            view! { <BooksTab can_manage=can_manage /> }
                                        }}
                                    </Transition>
                                }
                            })
                    }}

                    <div class:hidden=move || tab.get() != "kelas">
                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="h-20 bg-surface-container rounded-2xl"></div>
                                <div class="grid gap-4 md:grid-cols-2">
                                    <div class="h-32 bg-surface-container rounded-2xl"></div>
                                    <div class="h-32 bg-surface-container rounded-2xl"></div>
                                    <div class="h-32 bg-surface-container rounded-2xl hidden md:block"></div>
                                    <div class="h-32 bg-surface-container rounded-2xl hidden md:block"></div>
                                </div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => {
                                        let items = d.items.clone();
                                        let mut cats: Vec<String> = items
                                            .iter()
                                            .map(|k| k.category.clone())
                                            .filter(|c| !c.is_empty())
                                            .collect();
                                        cats.sort();
                                        cats.dedup();
                                        let mut golongans: Vec<String> = items
                                            .iter()
                                            .map(|k| k.golongan.clone())
                                            .filter(|g| !g.is_empty())
                                            .collect();
                                        golongans.sort();
                                        golongans.dedup();
                                        view! {
                                            <div class="space-y-4">
                                            // ── Statistik: baris ringkas (kartu ukuran-konten,
                                            // rata kiri) — dulu 2 kartu melar penuh 72rem. ──
                                            <div class="grid grid-cols-2 gap-3 md:flex md:gap-3">
                                                <div class="ppm-card p-4 flex items-center gap-3 card-hover md:min-w-[11rem]">
                                                    <div class="w-10 h-10 rounded-xl bg-secondary-container text-primary flex items-center justify-center shrink-0">
                                                        <span class="material-symbols-outlined">"school"</span>
                                                    </div>
                                                    <div>
                                                        <p class="text-2xl font-bold text-on-background leading-none" data-count=d.total_kelas.to_string()>
                                                            {d.total_kelas}
                                                        </p>
                                                        <p class="text-[11px] font-bold tracking-wider text-on-surface-variant mt-1">
                                                            "TOTAL KELAS"
                                                        </p>
                                                    </div>
                                                </div>
                                                <div class="ppm-card p-4 flex items-center gap-3 card-hover md:min-w-[11rem]">
                                                    <div class="w-10 h-10 rounded-xl bg-secondary-container text-primary flex items-center justify-center shrink-0">
                                                        <span class="material-symbols-outlined">"groups"</span>
                                                    </div>
                                                    <div>
                                                        <p class="text-2xl font-bold text-on-background leading-none" data-count=d.total_santri.to_string()>
                                                            {d.total_santri}
                                                        </p>
                                                        <p class="text-[11px] font-bold tracking-wider text-on-surface-variant mt-1">
                                                            "TOTAL SANTRI"
                                                        </p>
                                                    </div>
                                                </div>
                                            </div>

                                            // ── Cari: bilah penuh di atas grid (dulu justify-between
                                            // dgn stats → celah kosong besar di tengah kanvas lebar). ──
                                            <div class="relative">
                                                <span class="material-symbols-outlined absolute left-3.5 top-1/2 -translate-y-1/2 text-outline">
                                                    "search"
                                                </span>
                                                <input
                                                    type="text"
                                                    class="w-full pl-11 pr-4 py-3 bg-surface-container border-0 rounded-xl text-body-md text-on-surface"
                                                    placeholder="Cari kelas atau ustadz…"
                                                    prop:value=move || query.get()
                                                    on:input=move |ev| query.set(event_target_value(&ev))
                                                />
                                            </div>

                                            // Datalist autocomplete kategori + golongan (tak terlihat).
                                            <datalist id="kategori-kelas">
                                                {cats
                                                    .into_iter()
                                                    .map(|c| view! { <option value=c></option> })
                                                    .collect_view()}
                                            </datalist>
                                            <datalist id="golongan-kelas">
                                                {golongans
                                                    .into_iter()
                                                    .map(|g| view! { <option value=g></option> })
                                                    .collect_view()}
                                            </datalist>

                                            // ── Daftar kelas: kartu "Tambah" (statis, sel pertama)
                                            // + kartu kelas (reaktif filter pencarian). Kartu Tambah
                                            // KINI menyatu di grid (dulu terpisah di kolom sempit di
                                            // atas). items-start: form Tambah yang terbuka tak
                                            // meregangkan tinggi kartu kelas di baris yang sama. ──
                                            <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3 items-start">
                                                <TambahKelas show_form=show_form refetch=move || data.refetch() />
                                                {move || {
                                                    let q = query.get().to_lowercase();
                                                    let list: Vec<KelasItem> = items
                                                        .clone()
                                                        .into_iter()
                                                        .filter(|k| {
                                                            q.is_empty()
                                                                || k.name.to_lowercase().contains(&q)
                                                                || k.teacher.to_lowercase().contains(&q)
                                                        })
                                                        .collect();
                                                    if list.is_empty() && !q.is_empty() {
                                                        view! {
                                                            <p class="col-span-full text-body-sm text-on-surface-variant px-1 py-2">
                                                                "Tidak ada kelas yang cocok dengan pencarian."
                                                            </p>
                                                        }
                                                            .into_any()
                                                    } else {
                                                        list.into_iter()
                                                            .map(|k| view! { <KelasCard k=k /> })
                                                            .collect_view()
                                                            .into_any()
                                                    }
                                                }}
                                            </div>
                                            </div>
                                        }
                                            .into_any()
                                    }
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                })
                        }}
                    </Suspense>
                    </div>
                </div>

            </div>
        </DeviceFrame>
    }
}

#[component]
fn TambahKelas(show_form: RwSignal<bool>, refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let category = RwSignal::new(String::new());
    let golongan = RwSignal::new(String::new());
    let desc = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        error.set(None);
        let (n, cat, g, d) = (
            name.get_untracked(),
            category.get_untracked(),
            golongan.get_untracked(),
            desc.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            match create_class_action(n, cat, g, d).await {
                Ok(_) => {
                    name.set(String::new());
                    category.set(String::new());
                    golongan.set(String::new());
                    desc.set(String::new());
                    show_form.set(false);
                    refetch();
                }
                Err(e) => {
                    let m = e.to_string();
                    error.set(Some(m.rsplit(": ").next().unwrap_or(&m).to_string()));
                }
            }
            busy.set(false);
        });
    };

    view! {
        {move || {
            if show_form.get() {
                view! {
                    <form
                        class="ppm-card p-5 space-y-3 anim-in"
                        method="post"
                        on:submit=submit
                    >
                        <div class="flex items-center gap-2">
                            <span class="material-symbols-outlined text-primary">"add_circle"</span>
                            <h2 class="text-body-lg font-bold text-on-background">"Kelas Baru"</h2>
                        </div>
                        {move || {
                            error
                                .get()
                                .map(|e| {
                                    view! {
                                        <div class="p-3 bg-error-container text-on-error-container rounded-xl text-body-sm anim-in">
                                            {e}
                                        </div>
                                    }
                                })
                        }}
                        <input
                            type="text"
                            class="w-full bg-surface-container border-0 rounded-xl px-4 py-3 text-body-md text-on-surface"
                            placeholder="Nama kelas (mis. Kelas Lambatan A1)"
                            prop:value=move || name.get()
                            on:input=move |ev| name.set(event_target_value(&ev))
                            required=true
                        />
                        <input
                            type="text"
                            list="kategori-kelas"
                            class="w-full bg-surface-container border-0 rounded-xl px-4 py-3 text-body-md text-on-surface"
                            placeholder="Kategori (mis. Lambatan) — ketik baru bila belum ada"
                            prop:value=move || category.get()
                            on:input=move |ev| category.set(event_target_value(&ev))
                        />
                        <input
                            type="text"
                            list="golongan-kelas"
                            class="w-full bg-surface-container border-0 rounded-xl px-4 py-3 text-body-md text-on-surface"
                            placeholder="Golongan (mis. Bacaan/Makna) — ketik baru bila belum ada"
                            prop:value=move || golongan.get()
                            on:input=move |ev| golongan.set(event_target_value(&ev))
                        />
                        <textarea
                            rows="2"
                            class="w-full bg-surface-container border-0 rounded-xl px-4 py-3 text-body-md text-on-surface resize-none"
                            placeholder="Deskripsi singkat (opsional)"
                            prop:value=move || desc.get()
                            on:input=move |ev| desc.set(event_target_value(&ev))
                        ></textarea>
                        <div class="grid grid-cols-2 gap-3">
                            <button
                                type="button"
                                class="py-3 rounded-xl border border-outline-variant text-on-surface font-semibold text-body-sm"
                                on:click=move |_| show_form.set(false)
                            >
                                "Batal"
                            </button>
                            <button
                                type="submit"
                                class="py-3 rounded-xl bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                                disabled=move || busy.get()
                            >
                                {move || if busy.get() { "Menyimpan…" } else { "Simpan Kelas" }}
                            </button>
                        </div>
                    </form>
                }
                    .into_any()
            } else {
                view! {
                    // md:min-h menyamakan tinggi dgn KelasCard (badge+judul+guru+
                    // statistik+tombol) supaya kartu Tambah tak kelihatan kerdil
                    // sebagai sel pertama grid saat form tertutup.
                    <button
                        class="w-full h-full md:min-h-[13.5rem] border-2 border-dashed border-outline-variant rounded-2xl p-6 flex flex-col items-center justify-center gap-2 text-on-surface-variant hover:border-primary hover:text-primary transition-colors press"
                        on:click=move |_| show_form.set(true)
                    >
                        <span class="w-12 h-12 rounded-full bg-surface-container flex items-center justify-center">
                            <span class="material-symbols-outlined text-2xl">"add"</span>
                        </span>
                        <span class="text-body-md font-bold">"Tambah Kelas Baru"</span>
                        <span class="text-body-sm">"Mulai kurikulum baru hari ini"</span>
                    </button>
                }
                    .into_any()
            }
        }}
    }
}

#[component]
fn KelasCard(k: KelasItem) -> impl IntoView {
    let href = format!("/kelas/{}", k.id);
    view! {
        <div
            class="ppm-card p-4 card-hover anim-in"
            style="border-left:4px solid #064e3b"
        >
            <div class="flex flex-wrap gap-1.5 mb-1.5">
                {(!k.golongan.is_empty())
                    .then(|| {
                        view! {
                            <span class="inline-block px-2.5 py-1 rounded-full bg-primary/10 text-primary text-[10px] font-bold tracking-wider uppercase">
                                {k.golongan.clone()}
                            </span>
                        }
                    })}
                {(!k.category.is_empty())
                    .then(|| {
                        view! {
                            <span class="inline-block px-2.5 py-1 rounded-full bg-secondary-container text-primary text-[10px] font-bold tracking-wider uppercase">
                                {k.category.clone()}
                            </span>
                        }
                    })}
            </div>
            <h3 class="text-body-lg font-bold text-on-background">{k.name}</h3>
            <p class="text-body-sm text-on-surface-variant flex items-center gap-1 mt-1">
                <span class="material-symbols-outlined text-[15px]">"person"</span>
                {k.teacher}
            </p>
            <div class="flex items-center gap-4 mt-3 text-body-sm text-on-surface-variant">
                <span class="flex items-center gap-1">
                    <span class="material-symbols-outlined text-[16px] text-primary">"groups"</span>
                    <b class="text-on-background">{k.member_count}</b>
                    " Santri"
                </span>
                <span class="flex items-center gap-1">
                    <span class="material-symbols-outlined text-[16px] text-primary">"event"</span>
                    <b class="text-on-background">{k.schedule_count}</b>
                    " Jadwal"
                </span>
            </div>
            <a
                href=href
                class="mt-3 w-full py-2.5 rounded-xl bg-secondary-container text-primary font-semibold text-body-sm flex items-center justify-center gap-2 press"
            >
                <span class="material-symbols-outlined text-[18px]">"visibility"</span>
                "Lihat Santri & Kelola"
            </a>
        </div>
    }
}

#[component]
fn KelasTabBtn(tab: RwSignal<String>, value: &'static str, label: &'static str) -> impl IntoView {
    let cls = move || {
        if tab.get() == value {
            "w-full md:flex-1 py-2.5 rounded-lg bg-surface-container-lowest text-primary font-semibold text-body-sm shadow-sm press whitespace-nowrap text-center"
        } else {
            "w-full md:flex-1 py-2.5 rounded-lg text-on-surface-variant font-medium text-body-sm press whitespace-nowrap text-center"
        }
    };
    view! {
        <button class=cls on:click=move |_| tab.set(value.to_string())>
            {label}
        </button>
    }
}

// ── Tab MATERI: kelola daftar materi (kitab/Qur'an) — dulu "Buku" ──────────

#[component]
fn BooksTab(can_manage: bool) -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { books_list().await });

    view! {
        <div class="space-y-3 stagger">
            {can_manage
                .then(|| {
                    view! {
                        <div class="md:max-w-md">
                            <AddBookForm refetch=move || data.refetch() />
                        </div>
                    }
                })}
            <Suspense fallback=|| {
                view! { <div class="h-24 bg-surface-container rounded-2xl animate-pulse"></div> }
            }>
                {move || {
                    data.get()
                        .map(|res| match res {
                            Ok(items) => {
                                if items.is_empty() {
                                    view! {
                                        <p class="text-body-sm text-on-surface-variant text-center py-4">
                                            "Belum ada materi terdaftar."
                                        </p>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <div class="ppm-card divide-y divide-outline-variant/40">
                                            {items
                                                .into_iter()
                                                .map(|b| {
                                                    view! {
                                                        <BookRow
                                                            b=b
                                                            can_manage=can_manage
                                                            refetch=move || data.refetch()
                                                        />
                                                    }
                                                })
                                                .collect_view()}
                                        </div>
                                    }
                                        .into_any()
                                }
                            }
                            Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                        })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn BookRow(b: BookItem, can_manage: bool, refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let id = b.id;
    let busy = RwSignal::new(false);
    let del = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            let _ = delete_book_action(id).await;
            busy.set(false);
            refetch();
        });
    };
    view! {
        <div class="p-3.5 flex items-center gap-3">
            <div class="w-9 h-9 rounded-lg bg-secondary-container flex items-center justify-center text-primary shrink-0">
                <span class="material-symbols-outlined text-[18px]">"menu_book"</span>
            </div>
            <div class="flex-1 min-w-0">
                <p class="text-body-sm font-semibold text-on-background truncate">{b.title}</p>
                <p class="text-[11px] text-on-surface-variant">{format!("{} halaman", b.total_pages)}</p>
            </div>
            {can_manage
                .then(|| {
                    view! {
                        <button
                            class="w-8 h-8 rounded-lg bg-error-container/60 text-error flex items-center justify-center shrink-0 disabled:opacity-50"
                            disabled=move || busy.get()
                            on:click=del
                            aria-label="Hapus materi"
                        >
                            <span class="material-symbols-outlined text-[18px]">"delete"</span>
                        </button>
                    }
                })}
        </div>
    }
}

#[component]
fn AddBookForm(refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let title = RwSignal::new(String::new());
    let pages = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let (t, p) = (title.get_untracked(), pages.get_untracked());
        leptos::task::spawn_local(async move {
            match create_book_action(t, p).await {
                Ok(_) => {
                    title.set(String::new());
                    pages.set(String::new());
                    refetch();
                }
                Err(e) => {
                    let m = e.to_string();
                    msg.set(Some((false, m.rsplit(": ").next().unwrap_or(&m).to_string())));
                }
            }
            busy.set(false);
        });
    };

    let field =
        "w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface";
    view! {
        <form class="ppm-card p-4 space-y-3" method="post" on:submit=submit>
            <h3 class="text-body-md font-bold text-on-background flex items-center gap-2">
                <span class="material-symbols-outlined text-primary">"add_circle"</span>
                "Tambah Materi"
            </h3>
            {move || {
                msg.get()
                    .map(|(_, t)| {
                        view! {
                            <div class="p-2.5 bg-error-container text-on-error-container rounded-lg text-body-sm">
                                {t}
                            </div>
                        }
                    })
            }}
            <input
                type="text"
                class=field
                placeholder="Judul materi (mis. Sahih Bukhari, Al-Qur'an)"
                prop:value=move || title.get()
                on:input=move |ev| title.set(event_target_value(&ev))
            />
            <input
                type="number"
                min="1"
                class=field
                placeholder="Jumlah halaman"
                prop:value=move || pages.get()
                on:input=move |ev| pages.set(event_target_value(&ev))
            />
            <button
                type="submit"
                class="w-full py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                disabled=move || busy.get()
            >
                {move || if busy.get() { "Menyimpan…" } else { "Simpan Materi" }}
            </button>
        </form>
    }
}

// ── Tab AKADEMIK: audit progres hadist/quran semua santri ───────────────────

#[component]
fn AcademicAuditTab() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { academic_audit_data().await });

    view! {
        <div class="space-y-3 stagger">
            <p class="text-body-sm text-on-surface-variant">
                "Rata-rata progres tiap santri di semua materi (hadits/Qur'an) — paling tertinggal ditampilkan lebih dulu. Klik baris untuk buka detail di Students."
            </p>
            <Suspense fallback=|| {
                view! {
                    <div class="space-y-2 animate-pulse">
                        <div class="h-14 bg-surface-container rounded-xl"></div>
                        <div class="h-14 bg-surface-container rounded-xl"></div>
                    </div>
                }
            }>
                {move || {
                    data.get()
                        .map(|res| match res {
                            Ok(items) => {
                                if items.is_empty() {
                                    view! {
                                        <p class="text-body-sm text-on-surface-variant text-center py-8">
                                            "Belum ada materi terdaftar untuk diaudit."
                                        </p>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <div class="ppm-card divide-y divide-outline-variant/40">
                                            {items
                                                .into_iter()
                                                .map(|s| view! { <AcademicAuditRow s=s /> })
                                                .collect_view()}
                                        </div>
                                    }
                                        .into_any()
                                }
                            }
                            Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                        })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn AcademicAuditRow(s: StudentAcademicItem) -> impl IntoView {
    let pct = s.avg_percentage;
    let bar_color = if pct >= 75 {
        "bg-success"
    } else if pct >= 40 {
        "bg-warning"
    } else {
        "bg-error"
    };
    let progress_label = format!("{}/{} materi dimulai", s.books_started, s.total_books);
    let href = format!("/students?student={}", s.user_id);
    view! {
        <a
            href=href
            class="p-3 md:px-4 flex items-center gap-3 hover:bg-surface-container-low transition-colors"
        >
            <div class="flex-1 min-w-0">
                <p class="text-body-md font-semibold text-on-background truncate">{s.name}</p>
                <p class="text-[11px] text-on-surface-variant">{format!("NIS: {} • {progress_label}", s.nis)}</p>
                <div class="h-1.5 bg-surface-container rounded-full overflow-hidden mt-1.5 max-w-xs">
                    <div class=format!("h-full {bar_color}") style=format!("width: {pct}%")></div>
                </div>
            </div>
            <p class="text-body-lg font-bold text-primary shrink-0">{format!("{pct}%")}</p>
        </a>
    }
}
