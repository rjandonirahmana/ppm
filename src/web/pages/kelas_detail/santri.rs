//! Tab SANTRI — anggota kelas: daftar, keluarkan, dan tambah lewat pencarian.

use leptos::prelude::*;

use crate::models::{
    BookProgressItem, KelasDetail, StudentSearchItem,
};
use crate::web::api::{
    add_members_action, remove_member_action, staff_search_students, student_book_progress_for_viewer,
};
use crate::web::components::AdminOnly;
use crate::web::components::{BookProgressDetail, EmptyState, Sheet};

// ── Tab SANTRI ────────────────────────────────────────────────────────────────

#[component]
pub(super) fn SantriTab(
    class_id: i64,
    d: KelasDetail,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    // Jadwal & anggota kelas kini juga wewenang PAMONG kelas ini, bukan admin
    // saja (wali kelas tetap tidak). Flag-nya dihitung server — lihat
    // KelasDetail::can_manage_jadwal.
    let can_manage = d.can_manage_jadwal;
    let members = StoredValue::new(d.members.clone());
    let total = d.members.len();
    let query = RwSignal::new(String::new());
    let busy = RwSignal::new(Option::<i64>::None);

    // Detail progres materi santri (sheet).
    let detail_student = RwSignal::new(Option::<(i64, String)>::None);
    let detail_data = Resource::new(
        move || detail_student.get(),
        |st| async move {
            if let Some((sid, _)) = st {
                student_book_progress_for_viewer(sid).await.ok()
            } else {
                None
            }
        },
    );

    let remove = move |sid: i64| {
        if busy.get_untracked().is_some() {
            return;
        }
        busy.set(Some(sid));
        leptos::task::spawn_local(async move {
            let _ = remove_member_action(class_id, sid).await;
            busy.set(None);
            refetch();
        });
    };

    view! {
        <div class="space-y-3 stagger">
            <p class="text-body-sm text-on-surface-variant">
                "Total " <b class="text-on-background">{total}</b> " santri dalam kelas ini"
            </p>

            // Tambah santri + cari (md:max-w-md — form/input tunggal, bukan
            // grid; daftar anggota di bawah TETAP full-width via grid).
            <div class="space-y-3 md:max-w-md">
            // Santri masuk KELAS (migrasi 61), bukan jadwal — jadi tak perlu
            // lagi menunggu ada jadwal sebelum anggota bisa ditambahkan.
            <AdminOnly can_manage=can_manage apa="menambah atau mengeluarkan santri dari kelas" siapa="admin, ketua, atau pamong kelas ini">
                <AddMemberForm class_id=class_id refetch=refetch />
            </AdminOnly>

            // Cari peserta (filter klien)
            {(total > 0)
                .then(|| {
                    view! {
                        <div class="relative">
                            <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-outline text-[20px]">
                                "search"
                            </span>
                            <input
                                type="text"
                                class="w-full pl-10 pr-3 py-2.5 bg-surface-container border-0 rounded-xl text-body-sm text-on-surface"
                                placeholder="Cari nama atau NIS santri…"
                                prop:value=move || query.get()
                                on:input=move |ev| query.set(event_target_value(&ev))
                            />
                        </div>
                    }
                })}
            </div>

            // Daftar anggota
            {move || {
                let q = query.get().to_lowercase();
                let list: Vec<_> = members
                    .get_value()
                    .into_iter()
                    .filter(|m| {
                        q.is_empty() || m.name.to_lowercase().contains(&q) || m.nis.contains(&q)
                    })
                    .collect();
                if list.is_empty() {
                    return view! {
                        <EmptyState
                            icon="group"
                            title=if total == 0 {
                                "Belum ada santri di kelas ini."
                            } else {
                                "Tidak ada santri yang cocok."
                            }
                        />
                    }
                        .into_any();
                }
                view! {
                    <div class="ppm-card-grid">
                        {list.into_iter()
                    .map(|m| {
                        let sid = m.id;
                        let name = m.name.clone();
                        let initial = name.chars().next().unwrap_or('S').to_string();
                        let meta = format!("NIS: {}", m.nis);
                        let ang = m.angkatan.clone();
                        view! {
                            <div class="ppm-card p-3 flex items-center gap-3 card-hover anim-in ppm-accent">
                                <div class="w-10 h-10 rounded-full bg-secondary-container flex items-center justify-center text-primary font-bold shrink-0">
                                    {initial}
                                </div>
                                <div class="flex-1 min-w-0">
                                    <p class="text-body-md font-semibold text-on-background truncate">
                                        {name.clone()}
                                    </p>
                                    <p class="text-body-sm text-on-surface-variant">{meta}</p>
                                    {(!ang.is_empty())
                                        .then(|| {
                                            view! {
                                                <span class="inline-block mt-1 px-2 py-0.5 rounded-full bg-secondary-container text-primary text-[10px] font-bold">
                                                    "Angkatan " {ang}
                                                </span>
                                            }
                                        })}
                                </div>
                                <div class="flex items-center gap-1.5 shrink-0">
                                    <button
                                        class="w-9 h-9 rounded-lg bg-secondary-container text-primary flex items-center justify-center press"
                                        on:click=move |_| {
                                            detail_student.set(Some((sid, name.clone())));
                                        }
                                        aria-label="Lihat progres materi"
                                    >
                                        <span class="material-symbols-outlined text-[18px]">"auto_stories"</span>
                                    </button>
                                    // Mengeluarkan santri = wewenang admin/ketua
                                    // atau pamong kelas ini; tombolnya tak
                                    // ditampilkan sama sekali untuk yang lain.
                                    {can_manage
                                        .then(|| {
                                            view! {
                                                <button
                                                    class="w-9 h-9 rounded-lg bg-error-container/60 text-error flex items-center justify-center press disabled:opacity-50"
                                                    disabled=move || busy.get() == Some(sid)
                                                    on:click=move |_| remove(sid)
                                                    aria-label="Keluarkan dari kelas"
                                                >
                                                    <span class="material-symbols-outlined text-[20px]">"person_remove"</span>
                                                </button>
                                            }
                                        })}
                                </div>
                            </div>
                        }
                    })
                    .collect_view()}
                    </div>
                }
                    .into_any()
            }}

            // ── Bottom-sheet detail progres materi ─────────────────────────────
            {move || {
                detail_student
                    .get()
                    .map(|(_sid, name)| {
                        view! {
                            <Sheet
                                title="Detail Progres Materi"
                                on_close=move || detail_student.set(None)
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
                                                        student_name=name.clone()
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
fn AddMemberForm(
    class_id: i64,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let q = RwSignal::new(String::new());
    let results = RwSignal::new(Vec::<StudentSearchItem>::new());
    let selected = RwSignal::new(Vec::<i64>::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);
    let toggle = move |id: i64| {
        selected.update(|v| {
            if let Some(pos) = v.iter().position(|&x| x == id) {
                v.remove(pos);
            } else {
                v.push(id);
            }
        });
    };

    // Selalu memanggil server (query pendek/kosong → daftar default beberapa
    // santri), agar daftar tampil tanpa harus mengetik.
    let do_search = move || {
        let query = q.get_untracked();
        leptos::task::spawn_local(async move {
            // class_id → server mengecualikan santri yang sudah di kelas ini.
            if let Ok(r) = staff_search_students(query, class_id).await {
                results.set(r);
            }
        });
    };

    // Muat daftar default begitu form dirender.
    Effect::new(move |prev: Option<()>| {
        if prev.is_none() {
            do_search();
        }
    });

    let add_selected = move |_| {
        if busy.get_untracked() {
            return;
        }
        let ids = selected.get_untracked();
        if ids.is_empty() {
            return;
        }
        busy.set(true);
        msg.set(None);
        leptos::task::spawn_local(async move {
            match add_members_action(class_id, ids).await {
                Ok(n) => {
                    msg.set(Some((true, format!("{n} santri ditambahkan ke kelas."))));
                    selected.set(Vec::new());
                    q.set(String::new());
                    refetch();
                    do_search(); // refresh daftar → yg baru ditambah hilang dari pilihan
                }
                Err(e) => {
                    msg.set(Some((false, crate::web::components::pesan_galat(e))));
                }
            }
            busy.set(false);
        });
    };

    view! {
        <div class="ppm-card p-4 space-y-3">
            <div class="flex items-center gap-2">
                <span class="material-symbols-outlined text-primary">"person_add"</span>
                <h3 class="text-body-md font-bold text-on-background">"Tambah Santri"</h3>
            </div>

            // Cari santri
            <div class="relative">
                <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-outline text-[20px]">
                    "search"
                </span>
                <input
                    type="text"
                    class="w-full pl-10 pr-3 py-2.5 bg-surface-container border-0 rounded-xl text-body-sm text-on-surface"
                    placeholder="Cari nama atau NIS santri…"
                    prop:value=move || q.get()
                    on:input=move |ev| {
                        q.set(event_target_value(&ev));
                        do_search();
                    }
                />
            </div>

            {move || {
                msg.get()
                    .map(|(ok, text)| {
                        let cls = if ok {
                            "p-2.5 bg-secondary-container text-on-secondary-container rounded-lg text-body-sm anim-in"
                        } else {
                            "p-2.5 bg-error-container text-on-error-container rounded-lg text-body-sm anim-in"
                        };
                        view! { <div class=cls>{text}</div> }
                    })
            }}

            {move || {
                let list = results.get();
                (!list.is_empty())
                    .then(|| {
                        view! {
                            <p class="text-[11px] text-on-surface-variant">
                                "Centang santri (boleh banyak), lalu tekan Tambah."
                            </p>
                            <div class="space-y-2">
                                {list
                                    .into_iter()
                                    .map(|s| {
                                        let id = s.id;
                                        let meta = format!("NIS: {} • {}", s.nis, s.class_name);
                                        let checked = move || selected.get().contains(&id);
                                        view! {
                                            <label class="flex items-center gap-3 p-2.5 bg-surface-container rounded-lg anim-in cursor-pointer">
                                                <input
                                                    type="checkbox"
                                                    class="w-5 h-5 accent-primary cursor-pointer shrink-0"
                                                    prop:checked=checked
                                                    on:change=move |_| toggle(id)
                                                />
                                                <div class="flex-1 min-w-0">
                                                    <p class="text-body-sm font-semibold text-on-background truncate">
                                                        {s.name}
                                                    </p>
                                                    <p class="text-[12px] text-on-surface-variant truncate">{meta}</p>
                                                </div>
                                            </label>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                            <button
                                class="w-full py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-sm cursor-pointer press disabled:opacity-60"
                                prop:disabled=move || busy.get() || selected.get().is_empty()
                                on:click=add_selected
                            >
                                {move || {
                                    let n = selected.get().len();
                                    if busy.get() {
                                        "Menambahkan…".to_string()
                                    } else if n == 0 {
                                        "Pilih santri dulu".to_string()
                                    } else {
                                        format!("Tambah {n} santri")
                                    }
                                }}
                            </button>
                        }
                    })
            }}
        </div>
    }
}
