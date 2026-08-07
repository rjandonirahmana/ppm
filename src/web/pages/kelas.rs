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
    kelas_list, update_book_action,
};
use crate::web::components::{AdminOnly, DeviceFrame, FetchError, MobileHeader};
use crate::web::pages::{SesiContent, StudentBookPanel};

#[component]
pub fn KelasPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { kelas_list().await });
    let session = use_context::<Resource<Option<SessionUser>>>();

    Effect::new(move |_| {
        if let Some(Err(e)) = data.get() {
            // Hanya lempar ke login bila BELUM login (unauth). `forbidden` (login
            // tapi peran tak diizinkan) ditangani FetchError, bukan bounce ke login.
            if crate::web::components::is_auth_error(&e.to_string()) {
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
        <Title text="Manajemen Kelas — AFM SMART" />
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
                                            "admin" | "ketua" | "supervisor" | "teacher" | "dewan_guru"
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
                                                        "admin" | "ketua" | "supervisor" | "teacher" | "dewan_guru"
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
                                        let d_can_manage = d.can_manage;
                                        let items = d.items.clone();
                                        // Daftar kategori/jenjang tak lagi
                                        // dikumpulkan dari data: keduanya kini
                                        // himpunan tetap (models::KATEGORI_KELAS,
                                        // models::JENJANG), bukan apa pun yang
                                        // kebetulan sudah pernah diketik orang.
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

                                            // ── Daftar kelas: kartu "Tambah" (statis, sel pertama)
                                            // + kartu kelas (reaktif filter pencarian). Kartu Tambah
                                            // KINI menyatu di grid (dulu terpisah di kolom sempit di
                                            // atas). items-start: form Tambah yang terbuka tak
                                            // meregangkan tinggi kartu kelas di baris yang sama. ──
                                            <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3 items-start">
                                                // Hanya admin yang boleh membuat kelas — dikunci di
                                                // sini, bukan dibiarkan gagal "forbidden" saat disimpan.
                                                <AdminOnly can_manage=d_can_manage apa="membuat kelas baru">
                                                    <TambahKelas
                                                        show_form=show_form
                                                        guru=d.teacher_options.clone()
                                                        refetch=move || data.refetch()
                                                    />
                                                </AdminOnly>
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
fn TambahKelas(
    show_form: RwSignal<bool>,
    /// Guru calon wali kelas — wajib dipilih bila kelasnya KBM.
    guru: Vec<crate::models::TeacherOption>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let name = RwSignal::new(String::new());
    // Default KBM: itu kelas yang paling sering dibuat, dan pilihan default
    // yang salah membuat orang melewatkan bidang wali yang wajib.
    let category = RwSignal::new("kbm".to_string());
    let jenjang = RwSignal::new(String::new());
    let wali = RwSignal::new(0i64);
    let guru = StoredValue::new(guru);
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
        let (n, cat, g, w, d) = (
            name.get_untracked(),
            category.get_untracked(),
            jenjang.get_untracked(),
            wali.get_untracked(),
            desc.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            match create_class_action(n, cat, g, w, d).await {
                Ok(_) => {
                    name.set(String::new());
                    category.set("kbm".to_string());
                    jenjang.set(String::new());
                    wali.set(0);
                    desc.set(String::new());
                    show_form.set(false);
                    refetch();
                }
                Err(e) => {
                                        error.set(Some(crate::web::components::pesan_galat(e)));
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
                        // Dropdown TERTUTUP, bukan teks bebas: kategori kini
                        // hanya dua (migrasi 65). Sebelumnya kolom ini boleh
                        // diketik apa saja dan di produksi terisi enam nilai
                        // yang mencampur jenis kegiatan dengan jenjang.
                        <select
                            class="w-full bg-surface-container border-0 rounded-xl px-4 py-3 text-body-md text-on-surface"
                            prop:value=move || category.get()
                            on:change=move |ev| category.set(event_target_value(&ev))
                        >
                            {crate::models::KATEGORI_KELAS
                                .iter()
                                .map(|(k, l)| view! { <option value=*k>{*l}</option> })
                                .collect_view()}
                        </select>
                        // Jenjang hanya milik KBM; untuk non-KBM tak ada yang
                        // perlu dipilih, jadi bidangnya tak ditampilkan sama
                        // sekali alih-alih ditampilkan lalu diabaikan.
                        <Show when=move || category.get() == "kbm">
                            <select
                                class="w-full bg-surface-container border-0 rounded-xl px-4 py-3 text-body-md text-on-surface"
                                prop:value=move || jenjang.get()
                                on:change=move |ev| jenjang.set(event_target_value(&ev))
                                required=true
                            >
                                <option value="">"— pilih jenjang —"</option>
                                {crate::models::JENJANG
                                    .iter()
                                    .map(|(k, l)| view! { <option value=*k>{*l}</option> })
                                    .collect_view()}
                            </select>
                            // WALI KELAS wajib sejak kelas KBM dibuat: dialah
                            // satu-satunya penyetuju izin santri kelas itu, jadi
                            // kelas KBM tanpa wali berarti izin santrinya
                            // menggantung tanpa tujuan.
                            <select
                                class="w-full bg-surface-container border-0 rounded-xl px-4 py-3 text-body-md text-on-surface"
                                on:change=move |ev| {
                                    wali.set(event_target_value(&ev).parse().unwrap_or(0))
                                }
                                required=true
                            >
                                <option value="">"— pilih wali kelas (wajib) —"</option>
                                {guru
                                    .get_value()
                                    .into_iter()
                                    .map(|t| view! { <option value=t.id.to_string()>{t.name}</option> })
                                    .collect_view()}
                            </select>
                            <p class="text-[11px] text-on-surface-variant -mt-1">
                                "Wali kelas menyetujui izin santri kelas ini."
                            </p>
                        </Show>
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
    // Kelas KBM WAJIB punya wali (dialah penyetuju izin santrinya). Aturannya
    // ditegakkan di jalur tulis, tapi baris lama bisa lolos dari sebelum aturan
    // ini ada — ditandai di sini supaya diperbaiki, bukan didiamkan.
    let wali_kosong = k.category == "kbm" && k.wali_kelas.trim().is_empty();
    view! {
        <div class="ppm-card p-4 card-hover anim-in ppm-accent">
            <div class="flex flex-wrap gap-1.5 mb-1.5">
                // Kode disimpan, LABEL yang dipajang: "hadist_besar" bukan
                // kalimat yang pantas dibaca orang.
                {(!k.jenjang.is_empty())
                    .then(|| {
                        view! {
                            <span class="inline-block px-2.5 py-1 rounded-full bg-primary/10 text-primary text-[10px] font-bold tracking-wider uppercase">
                                {crate::models::jenjang_label(&k.jenjang)}
                            </span>
                        }
                    })}
                <span class="inline-block px-2.5 py-1 rounded-full bg-secondary-container text-primary text-[10px] font-bold tracking-wider uppercase">
                    {crate::models::kategori_label(&k.category)}
                </span>
            </div>
            <h3 class="text-body-lg font-bold text-on-background">{k.name}</h3>
            {wali_kosong
                .then(|| {
                    view! {
                        <p class="mt-1.5 flex items-start gap-1.5 rounded-xl bg-warning/10 px-2.5 py-2 text-[11px] text-on-background">
                            <span class="material-symbols-outlined text-[15px] text-warning shrink-0">
                                "warning"
                            </span>
                            "Belum ada wali kelas — izin santri kelas ini tak punya penyetuju. Tetapkan di detail kelas."
                        </p>
                    }
                })}
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
                        <div class="ppm-card p-4 space-y-3 md:max-w-md">
                            <h3 class="text-body-md font-bold text-on-background flex items-center gap-2">
                                <span class="material-symbols-outlined text-primary">"add_circle"</span>
                                "Tambah Materi"
                            </h3>
                            <BookForm
                                edit_id=None
                                init_title=String::new()
                                init_category="hadist".into()
                                init_pages=String::new()
                                init_surahs=Vec::new()
                                refetch=move || data.refetch()
                                on_done=|| ()
                            />
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
    let editing = RwSignal::new(false);
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

    // Nilai awal utk form edit (di-clone tiap render branch editing).
    let init_title = b.title.clone();
    let init_category = b.category.clone();
    let init_pages = if b.category != "quran" { b.total_pages.to_string() } else { String::new() };
    let init_surahs: Vec<(String, i32)> =
        b.surahs.iter().map(|s| (s.name.clone(), s.ayat)).collect();
    let title_ro = b.title.clone();
    let meta = if b.category == "quran" {
        format!("Qur'an · {} surat · {} ayat", b.surahs.len(), b.total_pages)
    } else {
        format!("Hadist · {} halaman", b.total_pages)
    };

    view! {
        <div class="p-3.5">
            {move || {
                if editing.get() {
                    view! {
                        <BookForm
                            edit_id=Some(id)
                            init_title=init_title.clone()
                            init_category=init_category.clone()
                            init_pages=init_pages.clone()
                            init_surahs=init_surahs.clone()
                            refetch=refetch
                            on_done=move || editing.set(false)
                        />
                    }
                        .into_any()
                } else {
                    view! {
                        <div class="flex items-center gap-3">
                            <div class="w-9 h-9 rounded-lg bg-secondary-container flex items-center justify-center text-primary shrink-0">
                                <span class="material-symbols-outlined text-[18px]">"menu_book"</span>
                            </div>
                            <div class="flex-1 min-w-0">
                                <p class="text-body-sm font-semibold text-on-background truncate">
                                    {title_ro.clone()}
                                </p>
                                <p class="text-[11px] text-on-surface-variant">{meta.clone()}</p>
                            </div>
                            {can_manage
                                .then(|| {
                                    view! {
                                        <button
                                            class="w-8 h-8 rounded-lg bg-surface-container text-primary flex items-center justify-center shrink-0 press"
                                            on:click=move |_| editing.set(true)
                                            aria-label="Edit materi"
                                        >
                                            <span class="material-symbols-outlined text-[18px]">"edit"</span>
                                        </button>
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
                        .into_any()
                }
            }}
        </div>
    }
}

#[component]
fn BookForm(
    edit_id: Option<i64>,
    init_title: String,
    init_category: String,
    init_pages: String,
    init_surahs: Vec<(String, i32)>,
    refetch: impl Fn() + Copy + Send + 'static,
    on_done: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let is_edit = edit_id.is_some();
    let title = RwSignal::new(init_title);
    let category =
        RwSignal::new(if init_category.is_empty() { "hadist".to_string() } else { init_category });
    let pages = RwSignal::new(init_pages);
    let surahs = RwSignal::new(init_surahs);
    let s_name = RwSignal::new(String::new());
    let s_ayat = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let add_surah = move |_| {
        let n = s_name.get_untracked().trim().to_string();
        let a: i32 = s_ayat.get_untracked().trim().parse().unwrap_or(0);
        if n.is_empty() || a <= 0 {
            return;
        }
        surahs.update(|v| v.push((n, a)));
        s_name.set(String::new());
        s_ayat.set(String::new());
    };

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let (t, cat, p) = (title.get_untracked(), category.get_untracked(), pages.get_untracked());
        let surahs_json = if cat == "quran" {
            let arr: Vec<_> = surahs
                .get_untracked()
                .iter()
                .map(|(n, a)| serde_json::json!({"name": n, "ayat": a}))
                .collect();
            serde_json::to_string(&arr).unwrap_or_else(|_| "[]".into())
        } else {
            String::new()
        };
        leptos::task::spawn_local(async move {
            let res = match edit_id {
                Some(id) => update_book_action(id, t, cat, p, surahs_json).await.map(|_| ()),
                None => create_book_action(t, cat, p, surahs_json).await.map(|_| ()),
            };
            match res {
                Ok(_) => {
                    refetch();
                    if is_edit {
                        on_done();
                    } else {
                        title.set(String::new());
                        pages.set(String::new());
                        surahs.set(Vec::new());
                    }
                }
                Err(e) => {
                    msg.set(Some((false, crate::web::components::pesan_galat(e))));
                }
            }
            busy.set(false);
        });
    };

    let field =
        "w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface";
    view! {
        <form class="space-y-3" method="post" on:submit=submit>
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
            <label class="space-y-1 block">
                <span class="text-label-md text-on-surface-variant">"Kategori"</span>
                <select class=field on:change=move |ev| category.set(event_target_value(&ev))>
                    <option value="hadist" selected=move || category.get() == "hadist">"Hadist (per halaman)"</option>
                    <option value="quran" selected=move || category.get() == "quran">"Qur'an (per ayat, isi surat)"</option>
                </select>
            </label>
            {move || {
                if category.get() == "quran" {
                    view! {
                        <div class="rounded-xl bg-surface-container/60 p-3 space-y-2">
                            <span class="text-label-md text-on-surface-variant">
                                "Daftar surat — tambah nama + jumlah ayat satu per satu"
                            </span>
                            <div class="flex gap-2">
                                <input
                                    type="text"
                                    class=field
                                    placeholder="Nama surat (mis. Al-Fatihah)"
                                    prop:value=move || s_name.get()
                                    on:input=move |ev| s_name.set(event_target_value(&ev))
                                />
                                <input
                                    type="number"
                                    min="1"
                                    class="w-24 bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface shrink-0"
                                    placeholder="Ayat"
                                    prop:value=move || s_ayat.get()
                                    on:input=move |ev| s_ayat.set(event_target_value(&ev))
                                />
                                <button
                                    type="button"
                                    class="px-3 rounded-xl bg-secondary-container text-primary font-semibold text-body-sm shrink-0 press"
                                    on:click=add_surah
                                >
                                    "+"
                                </button>
                            </div>
                            {move || {
                                let list = surahs.get();
                                (!list.is_empty())
                                    .then(|| {
                                        view! {
                                            <div class="flex flex-wrap gap-1.5">
                                                {list
                                                    .into_iter()
                                                    .enumerate()
                                                    .map(|(i, (n, a))| {
                                                        view! {
                                                            <span class="inline-flex items-center gap-1 pl-2.5 pr-1 py-1 rounded-full bg-surface-container-highest text-[11px] text-on-surface">
                                                                {format!("{n} ({a})")}
                                                                <button
                                                                    type="button"
                                                                    class="w-4 h-4 flex items-center justify-center text-on-surface-variant hover:text-error"
                                                                    on:click=move |_| surahs.update(|v| { v.remove(i); })
                                                                    aria-label="Hapus surat"
                                                                >
                                                                    <span class="material-symbols-outlined text-[14px]">"close"</span>
                                                                </button>
                                                            </span>
                                                        }
                                                    })
                                                    .collect_view()}
                                            </div>
                                        }
                                    })
                            }}
                        </div>
                    }
                        .into_any()
                } else {
                    view! {
                        <input
                            type="number"
                            min="1"
                            class=field
                            placeholder="Jumlah halaman"
                            prop:value=move || pages.get()
                            on:input=move |ev| pages.set(event_target_value(&ev))
                        />
                    }
                        .into_any()
                }
            }}
            <div class="flex gap-2">
                {is_edit
                    .then(|| {
                        view! {
                            <button
                                type="button"
                                class="flex-1 py-2.5 rounded-xl border border-outline-variant text-on-surface font-semibold text-body-sm"
                                on:click=move |_| on_done()
                            >
                                "Batal"
                            </button>
                        }
                    })}
                <button
                    type="submit"
                    class="flex-1 py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                    disabled=move || busy.get()
                >
                    {move || {
                        if busy.get() {
                            "Menyimpan…"
                        } else if is_edit {
                            "Simpan Perubahan"
                        } else {
                            "Simpan Materi"
                        }
                    }}
                </button>
            </div>
        </form>
    }
}

#[component]
fn AcademicAuditTab() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { academic_audit_data().await });
    // Baris santri mana yang sedang di-expand (progres materi inline).
    let expanded = RwSignal::new(Option::<i64>::None);

    view! {
        <div class="space-y-3 stagger">
            <p class="text-body-sm text-on-surface-variant">
                "Rata-rata progres tiap santri di semua materi (hadits/Qur'an) — paling tertinggal ditampilkan lebih dulu. Klik baris untuk lihat progres per materi."
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
                                                .map(|s| view! { <AcademicAuditRow s=s expanded=expanded /> })
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
fn AcademicAuditRow(s: StudentAcademicItem, expanded: RwSignal<Option<i64>>) -> impl IntoView {
    let pct = s.avg_percentage;
    let sid = s.user_id;
    let sname = s.name.clone();
    let bar_color = if pct >= 75 {
        "bg-success"
    } else if pct >= 40 {
        "bg-warning"
    } else {
        "bg-error"
    };
    let progress_label = format!("{}/{} materi dimulai", s.books_started, s.total_books);
    view! {
        <div>
            // Baris klik → toggle expand (progres materi inline, seperti Students).
            <div
                class="p-3 md:px-4 flex items-center gap-3 hover:bg-surface-container-low transition-colors cursor-pointer"
                on:click=move |_| {
                    expanded.update(|e| *e = if *e == Some(sid) { None } else { Some(sid) });
                }
            >
                <div class="flex-1 min-w-0">
                    <p class="text-body-md font-semibold text-on-background truncate">{s.name}</p>
                    <p class="text-[11px] text-on-surface-variant">{format!("NIS: {} • {progress_label}", s.nis)}</p>
                    <div class="h-1.5 bg-surface-container rounded-full overflow-hidden mt-1.5 max-w-xs">
                        <div class=format!("h-full {bar_color}") style=format!("width: {pct}%")></div>
                    </div>
                </div>
                <p class="text-body-lg font-bold text-primary shrink-0">{format!("{pct}%")}</p>
                <span
                    class="material-symbols-outlined text-on-surface-variant shrink-0 text-[20px] transition-transform"
                    class:rotate-180=move || expanded.get() == Some(sid)
                >
                    "expand_more"
                </span>
            </div>
            // Panel progres materi (fokus akademik) — reuse dari Students.
            {move || {
                (expanded.get() == Some(sid)).then(|| view! {
                    <div class="px-3 md:px-4 pb-4 bg-surface-container-low/60">
                        <StudentBookPanel student_id=sid student_name=sname.clone() />
                    </div>
                })
            }}
        </div>
    }
}
