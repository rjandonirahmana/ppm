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
    decide_pamong, decide_verify, student_book_progress_data,
    students_data,
};
use crate::web::components::{DeviceFrame, FetchError, MobileHeader};

#[component]
pub fn StudentsPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { students_data().await });

    Effect::new(move |_| {
        if let Some(Err(e)) = data.get() {
            if e.to_string().contains("unauth") {
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
        <Title text="Santri — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Santri" />

                // Bar cari SELALU tampil di tab Daftar (bukan cuma di dalam
                // Suspense) — santri bisa mulai mengetik walau data masih
                // memuat; filter jalan begitu resource selesai. Prominent di
                // mobile (dulu cuma muncul di desktop lewat kondisi md:).
                // Disembunyikan saat tab Verifikasi aktif (tak relevan di sana).
                <div class="px-5 pt-5 md:max-w-md" class:hidden=move || tab.get() == "verify">
                    <div class="relative">
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

#[component]
fn StudentList(
    students: Vec<StudentRowItem>,
    total: usize,
    query: RwSignal<String>,
    expanded: RwSignal<Option<i64>>,
) -> impl IntoView {
    let students = StoredValue::new(students);
    let shown = Memo::new(move |_| {
        let q = query.get().to_lowercase();
        students
            .get_value()
            .into_iter()
            .filter(|s| q.is_empty() || s.name.to_lowercase().contains(&q) || s.nis.contains(&q))
            .count()
    });
    view! {
        <p class="text-body-sm text-on-surface-variant">
            {move || {
                let n = shown.get();
                if query.get().is_empty() {
                    format!("Total {total} santri terdaftar")
                } else {
                    format!("{n} dari {total} santri cocok")
                }
            }}
        </p>
        // Daftar ala TABEL mockup: satu kartu, baris ber-divider + hover.
        <div class="ppm-card divide-y divide-outline-variant/40 overflow-hidden stagger">
            {move || {
                let q = query.get().to_lowercase();
                let list: Vec<_> = students
                    .get_value()
                    .into_iter()
                    .filter(|s| q.is_empty() || s.name.to_lowercase().contains(&q) || s.nis.contains(&q))
                    .collect();
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
                                        // Satu chip PER kelas (biasanya dua: satu golongan Bacaan
                                        // + satu Makna, migrasi 16) — dulu cuma satu class_name
                                        // ditampilkan (LIMIT 1), sekarang tampil semua.
                                        {classes
                                            .into_iter()
                                            .map(|c| {
                                                let label = if c.golongan.is_empty() {
                                                    c.name
                                                } else {
                                                    format!("{} • {}", c.golongan, c.name)
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
                                            <span class="hidden sm:inline-block ppm-chip-sm bg-secondary-container text-primary shrink-0">
                                                "Angkatan " {ang}
                                            </span>
                                        }
                                    })}
                                <div class="text-right shrink-0 w-14">
                                    <p class="text-body-lg font-bold text-primary">{s.points}</p>
                                    <p class="text-[10px] text-on-surface-variant">"Poin"</p>
                                </div>
                                <span class="material-symbols-outlined text-on-surface-variant shrink-0 text-[20px]">
                                    {move || {
                                        if expanded.get() == Some(sid) { "expand_less" } else { "expand_more" }
                                    }}
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
    }
}

// ── Progres Buku (migrasi 18) ────────────────────────────────────────────────

#[component]
fn StudentBookPanel(student_id: i64, student_name: String) -> impl IntoView {
    let data = Resource::new(
        move || student_id,
        |id| async move { student_book_progress_data(id).await },
    );

    view! {
        <div class="ppm-card p-3.5 space-y-2">
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
                view! {
                    <div class="space-y-3 md:space-y-0 md:grid md:grid-cols-2 md:gap-3">
                        {pending
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
                    })
                    .collect_view()}
                    </div>
                }
                    .into_any()
            }}
        </div>
    }
}
