//! web/pages/students.rs — Halaman STUDENTS (gabungan daftar santri + verifikasi).
//!
//! Dua tab: "Daftar Santri" (semua santri + poin/angkatan) dan "Verifikasi"
//! (antrean sesuai peran: pamong → tahap 1, dewan guru/admin → tahap 2). Guru
//! biasa (teacher) hanya melihat daftar. Menggabungkan halaman students &
//! verifikasi lama menjadi satu. Tab "Akademik"/"Buku" PINDAH ke /kelas
//! (bagian pengelolaan kurikulum) — halaman ini hanya menyisakan panel
//! "Progres Buku" per-santri (expand baris) yang datang dari sana via
//! `?student=<id>`.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_query_map;

use crate::models::{BookProgressItem, PendingAtt, StudentRowItem};
use crate::web::api::{
    angkatan_tersedia_data, students_page_data,
    decide_pamong, decide_verify, student_book_progress_data,
    student_book_progress_for_viewer, students_data,
};
use crate::web::components::{
    kartu_grid, BookProgressDetail, DeviceFrame, FetchError, MobileHeader, Sheet,
};

#[component]
pub fn StudentsPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { students_data().await });

    Effect::new(move |_| {
        if let Some(Err(e)) = data.get() {
            if crate::web::components::is_auth_error(&e.to_string()) {
                #[cfg(target_arch = "wasm32")]
                if let Some(w) = web_sys::window() {
                    let _ = w.location().replace("/login");
                }
            }
        }
    });

    // "list" | "verify"
    let tab = RwSignal::new("list".to_string());
    let query = RwSignal::new(String::new());
    // Penyaring angkatan. Kosong = semua. Disaring DI KLIEN bersama pencarian:
    // seluruh daftar (maks 300) memang sudah ada di memori, jadi menyaringnya
    // tak perlu perjalanan baru ke server.
    let angkatan = RwSignal::new(String::new());
    // Daftar tahun angkatan — dibaca sekali, tak bergantung penyaring mana pun.
    let angkatan_ada = Resource::new(|| (), |_| async move { angkatan_tersedia_data().await });
    let busy_id = RwSignal::new(Option::<i64>::None);
    // Santri yang panel "Progres Buku"-nya terbuka di tab Daftar Santri.
    // Dibuka otomatis bila datang dari tab Akademik /kelas (?student=id).
    let expanded = RwSignal::new(Option::<i64>::None);
    let query_map = use_query_map();
    Effect::new(move |_| {
        if let Some(id) = query_map.read().get("student").and_then(|s| s.parse::<i64>().ok()) {
            expanded.set(Some(id));
        }
    });

    view! {
        <Title text="Santri — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Santri" />

                // Bar cari SELALU tampil di tab Daftar (bukan cuma di dalam
                // Suspense) — santri bisa mulai mengetik walau data masih
                // memuat; filter jalan begitu resource selesai. Prominent di
                // mobile (dulu cuma muncul di desktop lewat kondisi md:).
                // Disembunyikan saat tab Verifikasi aktif (tak relevan di sana).
                // Cari + saring angkatan sebaris di desktop; bertumpuk di ponsel.
                <div
                    class="px-5 pt-5 flex flex-col sm:flex-row sm:items-center gap-2 md:max-w-2xl"
                    class:hidden=move || tab.get() == "verify"
                >
                    <div class="relative flex-1 min-w-0">
                        <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-outline text-[20px]">
                            "search"
                        </span>
                        <input
                            type="text"
                            class="w-full pl-10 pr-3 py-3 bg-surface-container border-0 rounded-xl text-body-sm text-on-surface"
                            placeholder="Cari nama atau NIS santri…"
                            prop:value=move || query.get()
                            on:input=move |ev| query.set(event_target_value(&ev))
                        />
                    </div>
                    // Pilihan angkatan diambil dari SERVER (`SELECT DISTINCT
                    // entry_year`), bukan disimpulkan dari daftar yang tampil.
                    //
                    // Sejak daftarnya dipaginasi, yang ada di layar cuma sepuluh
                    // baris pertama — menyusun pilihan dari situ hanya
                    // menawarkan angkatan yang kebetulan sudah termuat, dan
                    // angkatan lain tak pernah bisa dipilih sama sekali.
                    //
                    // Dibungkus <Suspense>: membaca resource di luar
                    // Suspense/Transition memicu hydration-mismatch (Leptos
                    // memperingatkannya di log) dan membuang optimasi streaming.
                    <Suspense fallback=|| {
                        view! {
                            <select class="sm:w-48 shrink-0 bg-surface-container border-0 rounded-xl px-3 py-3 text-body-sm text-on-surface">
                                <option value="">"Semua angkatan"</option>
                            </select>
                        }
                    }>
                        {move || {
                            let tahun = angkatan_ada.get().and_then(|r| r.ok()).unwrap_or_default();
                            view! {
                                <select
                                    class="sm:w-48 shrink-0 bg-surface-container border-0 rounded-xl px-3 py-3 text-body-sm text-on-surface"
                                    on:change=move |ev| angkatan.set(event_target_value(&ev))
                                >
                                    <option value="">"Semua angkatan"</option>
                                    {tahun
                                        .into_iter()
                                        .map(|t| {
                                            let v = t.to_string();
                                            let v2 = v.clone();
                                            view! {
                                                <option value=v.clone() selected=move || angkatan.get() == v2>
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

                <div class="px-5 pt-4 space-y-4">
                    <Suspense fallback=|| {
                        view! {
                            <div class="space-y-3 animate-pulse">
                                <div class="h-10 bg-surface-container rounded-xl md:hidden"></div>
                                <div class="space-y-0 rounded-2xl overflow-hidden divide-y divide-outline-variant/30">
                                    <div class="h-16 bg-surface-container"></div>
                                    <div class="h-16 bg-surface-container-low"></div>
                                    <div class="h-16 bg-surface-container"></div>
                                    <div class="h-16 bg-surface-container-low hidden md:block"></div>
                                    <div class="h-16 bg-surface-container hidden md:block"></div>
                                </div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => {
                                        let has_verify = d.verify_stage != "none";
                                        let stage = d.verify_stage.clone();
                                        let pending_n = d.pending.len();
                                        let students = StoredValue::new(d.students.clone());
                                        let total = d.students.len();
                                        let pending = StoredValue::new(d.pending.clone());
                                        let verified_today = d.verified_today;

                                        // Aksi verifikasi: cabang sesuai tahap peran.
                                        // `is_t1` bool (Copy) agar closure tetap Copy.
                                        let is_t1 = stage == "tahap1";
                                        let decide = move |id: i64, approve: bool| {
                                            if busy_id.get_untracked().is_some() {
                                                return;
                                            }
                                            busy_id.set(Some(id));
                                            leptos::task::spawn_local(async move {
                                                let _ = if is_t1 {
                                                    decide_pamong(id, approve).await
                                                } else {
                                                    decide_verify(id, approve).await
                                                };
                                                busy_id.set(None);
                                                data.refetch();
                                            });
                                        };

                                        view! {
                                            // ── Tab bar ────────────────────────
                                            <div class="flex gap-1 bg-surface-container rounded-xl p-1 overflow-x-auto">
                                                <TabBtn tab=tab value="list" label="Daftar Santri" badge=0 />
                                                {has_verify
                                                    .then(|| {
                                                        view! {
                                                            <TabBtn
                                                                tab=tab
                                                                value="verify"
                                                                label="Verifikasi"
                                                                badge=pending_n
                                                            />
                                                        }
                                                    })}
                                            </div>

                                            {move || {
                                                if has_verify && tab.get() == "verify" {
                                                    let stage2 = stage.clone();
                                                    view! {
                                                        <VerifyPanel
                                                            pending=pending.get_value()
                                                            stage=stage2
                                                            verified_today=verified_today
                                                            busy_id=busy_id
                                                            decide=decide
                                                        />
                                                    }
                                                        .into_any()
                                                } else {
                                                    view! {
                                                        <StudentList
                                                            students=students.get_value()
                                                            total=total
                                                            query=query
                                                            angkatan=angkatan
                                                            expanded=expanded
                                                        />
                                                    }
                                                        .into_any()
                                                }
                                            }}
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
fn TabBtn(tab: RwSignal<String>, value: &'static str, label: &'static str, badge: usize) -> impl IntoView {
    let cls = move || {
        if tab.get() == value {
            "flex-1 py-2.5 rounded-lg bg-surface-container-lowest text-primary font-semibold text-body-sm shadow-sm press flex items-center justify-center gap-1.5"
        } else {
            "flex-1 py-2.5 rounded-lg text-on-surface-variant font-medium text-body-sm press flex items-center justify-center gap-1.5"
        }
    };
    view! {
        <button class=cls on:click=move |_| tab.set(value.to_string())>
            {label}
            {(badge > 0)
                .then(|| {
                    view! {
                        <span class="px-1.5 min-w-5 h-5 rounded-full bg-error text-on-error text-[10px] font-bold flex items-center justify-center">
                            {badge}
                        </span>
                    }
                })}
        </button>
    }
}

/// Sama dengan `service::kelas::STUDENTS_PER_PAGE`. Klien perlu tahu untuk
/// menyimpulkan "sudah halaman terakhir" dari jumlah baris yang datang.
const PER_HALAMAN: i64 = 10;

#[allow(clippy::too_many_arguments)]
#[component]
fn StudentList(
    /// HALAMAN PERTAMA dari server. Sisanya menyusul saat digulir.
    students: Vec<StudentRowItem>,
    /// Jumlah SEBENARNYA santri yang cocok — dari `COUNT(*)`, bukan panjang
    /// daftar yang kebetulan sudah termuat.
    total: usize,
    query: RwSignal<String>,
    /// Penyaring angkatan (kosong = semua). Dimiliki halaman supaya kotak cari
    /// dan pemilih angkatan bisa berdiri berdampingan di satu bilah.
    angkatan: RwSignal<String>,
    expanded: RwSignal<Option<i64>>,
) -> impl IntoView {
    // ── Daftar yang BERTAMBAH ────────────────────────────────────────────────
    // `baris` menampung yang sudah termuat; `jumlah` adalah COUNT(*) dari
    // server untuk kombinasi penyaring saat ini. Keduanya diganti seluruhnya
    // tiap penyaring berubah — bukan disaring dari yang sudah ada, karena yang
    // sudah ada baru sepotong.
    let baris = RwSignal::new(students);
    let jumlah = RwSignal::new(total as i64);
    let memuat = RwSignal::new(false);
    let habis = RwSignal::new(false);

    // Ambil satu halaman. `offset` 0 = ganti seluruh daftar (penyaring berubah),
    // selain itu ditambahkan di belakang.
    // Dipanggil HANYA dari jalur wasm (penundaan ketikan & sentinel gulir);
    // di build server keduanya tak ada, jadi fungsinya menganggur di sana.
    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
    let ambil = move |offset: i64| {
        if memuat.get_untracked() {
            return;
        }
        memuat.set(true);
        let q = query.get_untracked();
        let ang = angkatan.get_untracked().parse::<i32>().unwrap_or(0);
        leptos::task::spawn_local(async move {
            match students_page_data(q, ang, offset).await {
                Ok((rows, total)) => {
                    let n = rows.len();
                    jumlah.set(total);
                    // Halaman yang datang lebih pendek dari jatahnya = sudah
                    // baris terakhir. Menunggu halaman KOSONG berarti satu
                    // permintaan sia-sia di setiap daftar.
                    habis.set((n as i64) < PER_HALAMAN);
                    if offset == 0 {
                        baris.set(rows);
                    } else {
                        baris.update(|v| v.extend(rows));
                    }
                }
                Err(_) => habis.set(true),
            }
            memuat.set(false);
        });
    };

    // Penyaring berubah → mulai lagi dari awal. Ditunda 300 ms supaya mengetik
    // "Ahmad" tak menembakkan lima permintaan.
    let sentinel: NodeRef<leptos::html::Div> = NodeRef::new();
    // Observer dipasang SEKALI setelah sentinelnya ada di DOM.
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
            // Berhenti bertanya kalau daftarnya sudah habis atau satu
            // permintaan masih berjalan — sentinel tetap terlihat selama
            // pemuatan, dan tanpa penjagaan ini ia menembakkan permintaan
            // beruntun untuk offset yang sama.
            if terlihat && !habis.get_untracked() && !memuat.get_untracked() {
                ambil(baris.get_untracked().len() as i64);
            }
        });
        let opts = web_sys::IntersectionObserverInit::new();
        opts.set_root_margin("400px");
        if let Ok(obs) = web_sys::IntersectionObserver::new_with_options(
            cb.as_ref().unchecked_ref(),
            &opts,
        ) {
            obs.observe(&el);
            // Closure & observer sengaja dibocorkan: keduanya harus hidup
            // selama halaman terbuka, dan halaman ini tak pernah di-mount
            // ulang tanpa memuat ulang datanya.
            cb.forget();
            std::mem::forget(obs);
        }
        true
    });

    // Pegangan timer penundaan ketikan. Hanya terpakai di jalur wasm — di
    // build server tak ada `setTimeout` yang perlu dibatalkan.
    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
    let tunda = StoredValue::new(0_i32);
    Effect::new(move |sebelum: Option<(String, String)>| {
        let kunci = (query.get(), angkatan.get());
        if sebelum.as_ref() == Some(&kunci) {
            return kunci;
        }
        if sebelum.is_some() {
            #[cfg(target_arch = "wasm32")]
            {
                use wasm_bindgen::closure::Closure;
                use wasm_bindgen::JsCast;
                if let Some(w) = web_sys::window() {
                    if tunda.get_value() != 0 {
                        w.clear_timeout_with_handle(tunda.get_value());
                    }
                    let cb = Closure::once_into_js(move || {
                        habis.set(false);
                        ambil(0);
                    });
                    if let Ok(h) = w.set_timeout_with_callback_and_timeout_and_arguments_0(
                        cb.unchecked_ref(),
                        300,
                    ) {
                        tunda.set_value(h);
                    }
                }
            }
        }
        kunci
    });

    view! {
        <p class="text-body-sm text-on-surface-variant">
            {move || {
                let dimuat = baris.get().len();
                let n = jumlah.get();
                if query.get().is_empty() && angkatan.get().is_empty() {
                    // Menyebut KEDUANYA: berapa yang terlihat dan berapa
                    // seluruhnya. Versi lama cuma menulis "Total 300" —
                    // angka batas pengambilan yang dipajang sebagai jumlah
                    // santri, sementara 200 sisanya tak disebut sama sekali.
                    if (dimuat as i64) < n {
                        format!("Menampilkan {dimuat} dari {n} santri")
                    } else {
                        format!("Total {n} santri terdaftar")
                    }
                } else if (dimuat as i64) < n {
                    format!("Menampilkan {dimuat} dari {n} santri cocok")
                } else {
                    format!("{n} santri cocok")
                }
            }}
        </p>
        // Daftar ala TABEL mockup: satu kartu, baris ber-divider + hover.
        <div class="ppm-card divide-y divide-outline-variant/40 overflow-hidden stagger">
            {move || {
                let list = baris.get();
                if list.is_empty() {
                    return view! {
                        <div class="p-8 text-center text-body-sm text-on-surface-variant">
                            "Tidak ada santri yang cocok."
                        </div>
                    }
                        .into_any();
                }
                list.into_iter()
                    .map(|s| {
                        let nis_label = format!("NIS: {}", s.nis);
                        let ang = s.angkatan.clone();
                        let classes = s.classes.clone();
                        let sid = s.id;
                        let sname = s.name.clone();
                        view! {
                            <div
                                class="p-3 md:px-4 flex items-center gap-3 hover:bg-surface-container-low transition-colors anim-in cursor-pointer"
                                on:click=move |_| {
                                    expanded
                                        .update(|e| {
                                            *e = if *e == Some(sid) { None } else { Some(sid) };
                                        })
                                }
                            >
                                <div class="w-10 h-10 rounded-full bg-secondary-container flex items-center justify-center text-primary font-bold shrink-0">
                                    {s.initial}
                                </div>
                                <div class="flex-1 min-w-0">
                                    <p class="text-body-md font-semibold text-on-background truncate">{s.name}</p>
                                    <div class="flex flex-wrap items-center gap-x-2 gap-y-1 mt-0.5">
                                        <span class="text-body-sm text-on-surface-variant">{nis_label}</span>
                                        // Satu chip PER kelas (biasanya dua: satu jenjang Bacaan
                                        // + satu Makna, migrasi 16) — dulu cuma satu class_name
                                        // ditampilkan (LIMIT 1), sekarang tampil semua.
                                        {classes
                                            .into_iter()
                                            .map(|c| {
                                                let label = if c.jenjang.is_empty() {
                                                    c.name
                                                } else {
                                                    format!("{} • {}", c.jenjang, c.name)
                                                };
                                                view! {
                                                    <span class="ppm-chip-sm bg-secondary-container/60 text-primary">
                                                        {label}
                                                    </span>
                                                }
                                            })
                                            .collect_view()}
                                    </div>
                                </div>
                                {(!ang.is_empty())
                                    .then(|| {
                                        view! {
                                            <span class="ppm-chip-sm bg-secondary-container text-primary shrink-0">
                                                "Angkatan " {ang}
                                            </span>
                                        }
                                    })}
                                <div class="text-right shrink-0 w-14">
                                    <p class="text-body-lg font-bold text-primary">{s.points}</p>
                                    <p class="text-[10px] text-on-surface-variant">"Poin"</p>
                                </div>
                                // `expand_less` TAK ada di subset font (jadi teks
                                // mentah) → pakai `expand_more` (ada) lalu rotasi
                                // 180° saat terbuka = panah-atas.
                                <span
                                    class="material-symbols-outlined text-on-surface-variant shrink-0 text-[20px] transition-transform"
                                    class:rotate-180=move || expanded.get() == Some(sid)
                                >
                                    "expand_more"
                                </span>
                            </div>
                            {move || {
                                (expanded.get() == Some(sid))
                                    .then(|| {
                                        view! {
                                            <div class="px-3 md:px-4 pb-4 bg-surface-container-low/60">
                                                <StudentBookPanel
                                                    student_id=sid
                                                    student_name=sname.clone()
                                                />
                                            </div>
                                        }
                                    })
                            }}
                        }
                    })
                    .collect_view()
                    .into_any()
            }}
        </div>

        // ── Sentinel gulir-tak-berujung ──────────────────────────────────────
        // Kotak setinggi 1px di DASAR daftar. Begitu ia masuk viewport
        // (ditambah margin 400px, jadi pemuatan mulai SEBELUM pengguna
        // benar-benar mentok), halaman berikutnya diambil.
        //
        // IntersectionObserver, bukan listener `scroll`: yang terakhir menyala
        // puluhan kali per detik dan harus di-throttle sendiri, sementara yang
        // ini diam selama sentinelnya tak terlihat.
        <div node_ref=sentinel class="h-px"></div>

        <Show when=move || memuat.get()>
            <p class="py-4 text-center text-body-sm text-on-surface-variant flex items-center justify-center gap-2">
                <span class="material-symbols-outlined text-[18px] pulse-dot">"sync"</span>
                "Memuat…"
            </p>
        </Show>
        <Show when=move || habis.get() && (baris.get().len() > 20)>
            <p class="py-4 text-center text-[11px] text-on-surface-variant/70">
                "Semua santri sudah ditampilkan."
            </p>
        </Show>
    }
}

// ── Progres Buku (migrasi 18) ────────────────────────────────────────────────

#[component]
pub fn StudentBookPanel(student_id: i64, student_name: String) -> impl IntoView {
    let data = Resource::new(
        move || student_id,
        |id| async move { student_book_progress_data(id).await },
    );

    // Sheet detail progres materi (grid per-unit seperti tampilan santri).
    let show_detail = RwSignal::new(false);
    let sid = student_id;
    let sname = RwSignal::new(student_name.clone());
    let detail_data = Resource::new(
        move || show_detail.get(),
        move |show| async move {
            if show {
                student_book_progress_for_viewer(sid).await.ok()
            } else {
                None
            }
        },
    );

    view! {
        <div class="ppm-card p-3.5 space-y-3">
            <p class="text-body-sm font-bold text-on-background flex items-center gap-1.5">
                <span class="material-symbols-outlined text-primary text-[18px]">"menu_book"</span>
                {format!("Progres Materi — {student_name}")}
            </p>
            <Suspense fallback=|| {
                view! { <div class="h-12 bg-surface-container rounded-xl animate-pulse"></div> }
            }>
                {move || {
                    data.get()
                        .map(|res| match res {
                            Ok(items) => {
                                if items.is_empty() {
                                    view! {
                                        <p class="text-body-sm text-on-surface-variant">
                                            "Belum ada materi terdaftar."
                                        </p>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <div class="space-y-2">
                                            {items
                                                .into_iter()
                                                .map(|b| {
                                                    view! { <BookProgressRow b=b /> }
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

            <button
                class="w-full py-2.5 border-2 border-primary/30 text-primary rounded-xl text-body-sm font-semibold press flex items-center justify-center gap-2"
                on:click=move |_| show_detail.set(true)
            >
                <span class="material-symbols-outlined text-lg">"auto_stories"</span>
                "Lihat Detail Progres"
            </button>

            // ── Panel detail progres materi (sheet di ponsel, dialog di desktop) ──
            {move || {
                show_detail
                    .get()
                    .then(|| {
                        view! {
                            <Sheet
                                title="Detail Progres Materi"
                                on_close=move || show_detail.set(false)
                            >
                                <Suspense fallback=|| {
                                    view! {
                                        <div class="space-y-3 animate-pulse">
                                            <div class="h-40 bg-surface-container rounded-2xl"></div>
                                            <div class="h-40 bg-surface-container rounded-2xl"></div>
                                        </div>
                                    }
                                }>
                                    {move || {
                                        detail_data
                                            .get()
                                            .flatten()
                                            .map(|items: Vec<BookProgressItem>| {
                                                view! {
                                                    <BookProgressDetail
                                                        student_name=sname.get()
                                                        items=items
                                                    />
                                                }
                                                    .into_any()
                                            })
                                    }}
                                </Suspense>
                            </Sheet>
                        }
                    })
            }}
        </div>
    }
}

#[component]
fn BookProgressRow(b: BookProgressItem) -> impl IntoView {
    // Read-only: progres diisi SENDIRI oleh santri via grid /akademik (migrasi 25).
    let title = b.book_title.clone();
    let pct = b.percentage;
    let meta = if b.category == "quran" {
        format!("Qur'an · {} surat · {} ayat", b.surahs.len(), b.total_pages)
    } else {
        format!("Hadist · {} halaman", b.total_pages)
    };
    view! {
        <div class="bg-surface-container-lowest rounded-xl p-3 border border-outline-variant/30">
            <div class="flex items-center justify-between gap-2">
                <p class="text-body-sm font-semibold text-on-background truncate">{title}</p>
                <span class="text-body-sm font-bold text-primary shrink-0">{format!("{pct}%")}</span>
            </div>
            <p class="text-[11px] text-on-surface-variant">{meta}</p>
            <div class="h-1.5 bg-surface-container rounded-full overflow-hidden mt-2">
                <div class="h-full bg-primary" style=format!("width: {pct}%")></div>
            </div>
        </div>
    }
}

#[component]
fn VerifyPanel(
    pending: Vec<PendingAtt>,
    stage: String,
    verified_today: i64,
    busy_id: RwSignal<Option<i64>>,
    decide: impl Fn(i64, bool) + Copy + Send + 'static,
) -> impl IntoView {
    let pending_n = pending.len();
    let stage_label = if stage == "tahap1" {
        "Verifikasi Tahap 1 (Pamong)"
    } else {
        "Verifikasi Tahap 2 (Dewan Guru)"
    };
    let action_label = if stage == "tahap1" { "Setujui" } else { "Verifikasi" };
    view! {
        <div class="space-y-3 stagger">
            <p class="text-body-sm text-on-surface-variant">{stage_label}</p>
            <div class="grid grid-cols-2 gap-3 md:max-w-lg">
                <div class="ppm-card p-4">
                    <div class="flex items-center gap-2 text-warning">
                        <span class="material-symbols-outlined pulse-dot">"pending_actions"</span>
                        <span class="text-2xl font-bold text-on-background" data-count=pending_n.to_string()>
                            {pending_n}
                        </span>
                    </div>
                    <p class="text-body-sm text-on-surface-variant mt-1">"Menunggu"</p>
                </div>
                <div class="ppm-card p-4">
                    <div class="flex items-center gap-2 text-success">
                        <span class="material-symbols-outlined">"done_all"</span>
                        <span class="text-2xl font-bold text-on-background" data-count=verified_today.to_string()>
                            {verified_today}
                        </span>
                    </div>
                    <p class="text-body-sm text-on-surface-variant mt-1">"Selesai Hari Ini"</p>
                </div>
            </div>

            {if pending.is_empty() {
                view! {
                    <div class="bg-surface-container rounded-2xl p-8 text-center">
                        <span class="material-symbols-outlined text-5xl text-success">"task_alt"</span>
                        <p class="text-body-md text-on-surface-variant mt-3">
                            "Tidak ada kehadiran menunggu verifikasi."
                        </p>
                    </div>
                }
                    .into_any()
            } else {
                // Desktop: antrean verifikasi 2 kolom (mockup dashboard pamong).
                kartu_grid(
                    pending
                    .into_iter()
                    .map(|p| {
                        let id = p.id;
                        let initial = p.name.chars().next().unwrap_or('S').to_string();
                        let meta = format!("NIS: {} • {}", p.nis, p.class_name);
                        let scan = format!("{} • {}", p.time_label, p.gate);
                        let is_busy = move || busy_id.get() == Some(id);
                        view! {
                            <div class="ppm-card p-4 space-y-3 card-hover anim-in">
                                <div class="flex items-center gap-3">
                                    <div class="w-11 h-11 rounded-full bg-secondary-container flex items-center justify-center text-primary font-bold shrink-0">
                                        {initial}
                                    </div>
                                    <div class="flex-1 min-w-0">
                                        <p class="text-body-md font-semibold text-on-background truncate">
                                            {p.name}
                                        </p>
                                        <p class="text-body-sm text-on-surface-variant truncate">{meta}</p>
                                        <p class="text-body-sm text-on-surface-variant flex items-center gap-1 mt-0.5">
                                            <span class="material-symbols-outlined text-[15px]">"schedule"</span>
                                            {scan}
                                        </p>
                                    </div>
                                </div>
                                <div class="grid grid-cols-2 gap-3">
                                    <button
                                        class="py-2.5 rounded-xl border border-error/40 text-error font-semibold text-body-sm disabled:opacity-50"
                                        disabled=is_busy
                                        on:click=move |_| decide(id, false)
                                    >
                                        "Tolak"
                                    </button>
                                    <button
                                        class="py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-50"
                                        disabled=is_busy
                                        on:click=move |_| decide(id, true)
                                    >
                                        {action_label}
                                    </button>
                                </div>
                            </div>
                        }
                            .into_any()
                    })
                    .collect(),
                )
                    .into_any()
            }}
        </div>
    }
}
