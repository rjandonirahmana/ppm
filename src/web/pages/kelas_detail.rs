//! web/pages/kelas_detail.rs — Detail & kelola satu kelas (/kelas/:id).
//!
//! Tiga tab: SANTRI (anggota + tambah santri via cari+jadwal), JADWAL (daftar +
//! buat jadwal baru), SESI (daftar + buat sesi baru dgn pengajar). Semua aksi
//! memanggil server fn manajemen kelas lalu me-refetch detail.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use crate::models::{
    CurriculumItem, KelasDetail, ScheduleItem, ScheduleOption, StudentSearchItem, TeacherOption,
};
use crate::web::api::{
    add_member_action, create_curriculum_action, create_schedule_action, create_session_action,
    delete_curriculum_action, delete_schedule_action, kelas_detail, remove_member_action,
    set_session_libur_action, set_session_teacher_action, staff_search_students,
    update_class_action, update_curriculum_action, update_schedule_action,
};
use crate::web::components::{DeviceFrame, EmptyState, FetchError, MobileHeader};

#[component]
pub fn KelasDetailPage() -> impl IntoView {
    let params = use_params_map();
    let class_id = Memo::new(move |_| {
        params
            .read()
            .get("id")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    });
    let data = Resource::new(
        move || class_id.get(),
        |id| async move { kelas_detail(id).await },
    );

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

    // Tab aktif: "santri" | "jadwal" | "sesi".
    let tab = RwSignal::new("santri".to_string());

    view! {
        <Title text="Detail Kelas — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Detail Kelas" back_href="/kelas" />

                <div class="px-5 pt-5 space-y-4">
                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                                <div class="grid gap-3 md:grid-cols-2">
                                    <div class="h-32 bg-surface-container rounded-2xl"></div>
                                    <div class="h-32 bg-surface-container rounded-2xl hidden md:block"></div>
                                </div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => {
                                        view! { <DetailBody d=d tab=tab refetch=move || data.refetch() /> }
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
fn DetailBody(
    d: KelasDetail,
    tab: RwSignal<String>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let class_id = d.id;
    let member_count = d.members.len();
    let sched_count = d.schedules.len();
    let sesi_count = d.sessions.len();
    let category = d.category.clone();
    let cat_opts = StoredValue::new(d.category_options.clone());
    let golongan = d.golongan.clone();
    let gol_opts = StoredValue::new(d.golongan_options.clone());

    // Mode edit hero (Edit Detail Kelas: nama + kategori + golongan).
    let editing = RwSignal::new(false);
    let name_v = RwSignal::new(d.name.clone());
    let cat_v = RwSignal::new(d.category.clone());
    let gol_v = RwSignal::new(d.golongan.clone());
    let busy = RwSignal::new(false);
    let err = RwSignal::new(Option::<String>::None);
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        err.set(None);
        let (n, c, g) = (name_v.get_untracked(), cat_v.get_untracked(), gol_v.get_untracked());
        leptos::task::spawn_local(async move {
            match update_class_action(class_id, n, c, g).await {
                Ok(_) => {
                    editing.set(false);
                    refetch();
                }
                Err(e) => {
                    let m = e.to_string();
                    err.set(Some(m.rsplit(": ").next().unwrap_or(&m).to_string()));
                }
            }
            busy.set(false);
        });
    };

    view! {
        // ── Hero kelas (dgn Edit Detail: nama + kategori) ───────────────────
        <div class="spiritual-gradient rounded-2xl p-5 text-on-primary shadow-lg shadow-primary/20 anim-in">
            {move || {
                if editing.get() {
                    view! {
                        <form class="space-y-3" method="post" on:submit=save>
                            <p class="text-body-lg font-bold flex items-center gap-2">
                                <span class="material-symbols-outlined">"edit"</span>
                                "Edit Detail Kelas"
                            </p>
                            {move || {
                                err.get()
                                    .map(|e| {
                                        view! {
                                            <div class="p-2.5 bg-white/15 rounded-lg text-body-sm">{e}</div>
                                        }
                                    })
                            }}
                            <div class="space-y-1">
                                <label class="text-[11px] font-bold tracking-wider opacity-80">"NAMA KELAS"</label>
                                <input
                                    type="text"
                                    class="w-full bg-white/15 border-0 rounded-lg px-3 py-2.5 text-body-md text-on-primary placeholder-white/50"
                                    prop:value=move || name_v.get()
                                    on:input=move |ev| name_v.set(event_target_value(&ev))
                                    required=true
                                />
                            </div>
                            <div class="space-y-1">
                                <label class="text-[11px] font-bold tracking-wider opacity-80">"KATEGORI KELAS"</label>
                                <input
                                    type="text"
                                    list="kategori-detail"
                                    class="w-full bg-white/15 border-0 rounded-lg px-3 py-2.5 text-body-md text-on-primary placeholder-white/50"
                                    placeholder="mis. Lambatan — ketik baru bila belum ada"
                                    prop:value=move || cat_v.get()
                                    on:input=move |ev| cat_v.set(event_target_value(&ev))
                                />
                                <datalist id="kategori-detail">
                                    {cat_opts
                                        .get_value()
                                        .into_iter()
                                        .map(|c| view! { <option value=c></option> })
                                        .collect_view()}
                                </datalist>
                            </div>
                            <div class="space-y-1">
                                <label class="text-[11px] font-bold tracking-wider opacity-80">"GOLONGAN"</label>
                                <input
                                    type="text"
                                    list="golongan-detail"
                                    class="w-full bg-white/15 border-0 rounded-lg px-3 py-2.5 text-body-md text-on-primary placeholder-white/50"
                                    placeholder="mis. Bacaan/Makna — ketik baru bila belum ada"
                                    prop:value=move || gol_v.get()
                                    on:input=move |ev| gol_v.set(event_target_value(&ev))
                                />
                                <datalist id="golongan-detail">
                                    {gol_opts
                                        .get_value()
                                        .into_iter()
                                        .map(|g| view! { <option value=g></option> })
                                        .collect_view()}
                                </datalist>
                            </div>
                            <div class="grid grid-cols-2 gap-2 pt-1">
                                <button
                                    type="button"
                                    class="py-2.5 rounded-lg bg-white/10 border border-white/20 font-semibold text-body-sm"
                                    on:click=move |_| editing.set(false)
                                >
                                    "Batal"
                                </button>
                                <button
                                    type="submit"
                                    class="py-2.5 rounded-lg bg-primary-fixed text-primary font-bold text-body-sm disabled:opacity-60"
                                    disabled=move || busy.get()
                                >
                                    {move || if busy.get() { "Menyimpan…" } else { "Simpan Perubahan" }}
                                </button>
                            </div>
                        </form>
                    }
                        .into_any()
                } else {
                    let category = category.clone();
                    let golongan = golongan.clone();
                    view! {
                        <div class="flex items-start justify-between gap-3">
                            <div class="min-w-0">
                                <div class="flex flex-wrap gap-1.5 mb-1">
                                    {(!golongan.is_empty())
                                        .then(|| {
                                            view! {
                                                <span class="inline-block px-2.5 py-0.5 rounded-full bg-primary-fixed text-primary text-[10px] font-bold tracking-wider uppercase">
                                                    {golongan}
                                                </span>
                                            }
                                        })}
                                    {(!category.is_empty())
                                        .then(|| {
                                            view! {
                                                <span class="inline-block px-2.5 py-0.5 rounded-full bg-white/20 text-[10px] font-bold tracking-wider uppercase">
                                                    {category}
                                                </span>
                                            }
                                        })}
                                </div>
                                <h2 class="text-headline-sm font-bold">{name_v.get()}</h2>
                            </div>
                            <button
                                class="w-9 h-9 rounded-full bg-white/15 flex items-center justify-center shrink-0 press"
                                on:click=move |_| editing.set(true)
                                aria-label="Edit detail kelas"
                            >
                                <span class="material-symbols-outlined text-[20px]">"edit"</span>
                            </button>
                        </div>
                        <div class="flex items-center gap-4 mt-3 text-body-sm">
                            <span class="flex items-center gap-1">
                                <span class="material-symbols-outlined text-[16px]">"groups"</span>
                                {member_count}
                                " Santri"
                            </span>
                            <span class="flex items-center gap-1">
                                <span class="material-symbols-outlined text-[16px]">"event"</span>
                                {sched_count}
                                " Jadwal"
                            </span>
                            <span class="flex items-center gap-1">
                                <span class="material-symbols-outlined text-[16px]">"cast_for_education"</span>
                                {sesi_count}
                                " Sesi"
                            </span>
                        </div>
                    }
                        .into_any()
                }
            }}
        </div>

        // ── Tab ─────────────────────────────────────────────────────────────
        <div class="flex gap-1 bg-surface-container rounded-xl p-1">
            <TabBtn tab=tab value="santri" label="Santri" />
            <TabBtn tab=tab value="jadwal" label="Jadwal" />
            <TabBtn tab=tab value="sesi" label="Sesi" />
            <TabBtn tab=tab value="kurikulum" label="Kurikulum" />
        </div>

        // ── Konten tab ──────────────────────────────────────────────────────
        {move || {
            match tab.get().as_str() {
                "jadwal" => {
                    view! { <JadwalTab class_id=class_id d=d.clone() refetch=refetch /> }.into_any()
                }
                "sesi" => {
                    view! { <SesiTab class_id=class_id d=d.clone() refetch=refetch /> }.into_any()
                }
                "kurikulum" => {
                    view! { <KurikulumTab class_id=class_id d=d.clone() refetch=refetch /> }
                        .into_any()
                }
                _ => {
                    view! { <SantriTab class_id=class_id d=d.clone() refetch=refetch /> }.into_any()
                }
            }
        }}
    }
}

#[component]
fn TabBtn(tab: RwSignal<String>, value: &'static str, label: &'static str) -> impl IntoView {
    let cls = move || {
        if tab.get() == value {
            "flex-1 py-2.5 rounded-lg bg-surface-container-lowest text-primary font-semibold text-body-sm shadow-sm press"
        } else {
            "flex-1 py-2.5 rounded-lg text-on-surface-variant font-medium text-body-sm press"
        }
    };
    view! {
        <button class=cls on:click=move |_| tab.set(value.to_string())>
            {label}
        </button>
    }
}

// ── Tab SANTRI ────────────────────────────────────────────────────────────────

#[component]
fn SantriTab(
    class_id: i64,
    d: KelasDetail,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let sched_opts = StoredValue::new(d.schedule_options.clone());
    let has_sched = !d.schedule_options.is_empty();
    let members = StoredValue::new(d.members.clone());
    let total = d.members.len();
    let query = RwSignal::new(String::new());
    let busy = RwSignal::new(Option::<i64>::None);

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
            {if has_sched {
                view! { <AddMemberForm class_id=class_id schedule_options=sched_opts refetch=refetch /> }
                    .into_any()
            } else {
                view! {
                    <div class="bg-warning/10 border border-warning/30 rounded-2xl p-4 text-body-sm text-on-surface-variant">
                        <b class="text-on-background">"Buat jadwal dulu."</b>
                        " Santri ditempatkan pada sebuah jadwal, jadi tambahkan minimal satu jadwal di tab Jadwal sebelum menambah santri."
                    </div>
                }
                    .into_any()
            }}

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
                    <div class="space-y-3 md:space-y-0 md:grid md:grid-cols-2 md:gap-3">
                        {list.into_iter()
                    .map(|m| {
                        let sid = m.id;
                        let initial = m.name.chars().next().unwrap_or('S').to_string();
                        let meta = format!("NIS: {}", m.nis);
                        let ang = m.angkatan.clone();
                        view! {
                            <div
                                class="ppm-card p-3 flex items-center gap-3 card-hover anim-in"
                                style="border-left:4px solid #064e3b"
                            >
                                <div class="w-10 h-10 rounded-full bg-secondary-container flex items-center justify-center text-primary font-bold shrink-0">
                                    {initial}
                                </div>
                                <div class="flex-1 min-w-0">
                                    <p class="text-body-md font-semibold text-on-background truncate">
                                        {m.name}
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
                                <button
                                    class="w-9 h-9 rounded-lg bg-error-container/60 text-error flex items-center justify-center shrink-0 press disabled:opacity-50"
                                    disabled=move || busy.get() == Some(sid)
                                    on:click=move |_| remove(sid)
                                    aria-label="Keluarkan dari kelas"
                                >
                                    <span class="material-symbols-outlined text-[20px]">"person_remove"</span>
                                </button>
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

#[component]
fn AddMemberForm(
    class_id: i64,
    schedule_options: StoredValue<Vec<ScheduleOption>>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let first_sched = schedule_options.with_value(|v| v.first().map(|s| s.id).unwrap_or(0));
    let sched = RwSignal::new(first_sched);
    let q = RwSignal::new(String::new());
    let results = RwSignal::new(Vec::<StudentSearchItem>::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    // Selalu memanggil server (query pendek/kosong → daftar default beberapa
    // santri), agar daftar tampil tanpa harus mengetik.
    let do_search = move || {
        let query = q.get_untracked();
        leptos::task::spawn_local(async move {
            if let Ok(r) = staff_search_students(query).await {
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

    let add = move |student_id: i64| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let sid = sched.get_untracked();
        leptos::task::spawn_local(async move {
            match add_member_action(class_id, sid, student_id).await {
                Ok(_) => {
                    msg.set(Some((true, "Santri ditambahkan ke kelas.".into())));
                    results.set(Vec::new());
                    q.set(String::new());
                    refetch();
                }
                Err(e) => {
                    let m = e.to_string();
                    msg.set(Some((
                        false,
                        m.rsplit(": ").next().unwrap_or(&m).to_string(),
                    )));
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

            // Pilih jadwal penempatan
            <div class="space-y-1">
                <label class="text-label-md text-on-surface-variant">"Tempatkan pada jadwal"</label>
                <select
                    class="w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface"
                    on:change=move |ev| {
                        sched.set(event_target_value(&ev).parse().unwrap_or(0))
                    }
                >
                    {schedule_options
                        .get_value()
                        .into_iter()
                        .map(|o| {
                            let val = o.id.to_string();
                            view! { <option value=val>{o.label}</option> }
                        })
                        .collect_view()}
                </select>
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
                            <div class="space-y-2">
                                {list
                                    .into_iter()
                                    .map(|s| {
                                        let id = s.id;
                                        let meta = format!("NIS: {} • {}", s.nis, s.class_name);
                                        view! {
                                            <div class="flex items-center gap-2 p-2.5 bg-surface-container rounded-lg anim-in">
                                                <div class="flex-1 min-w-0">
                                                    <p class="text-body-sm font-semibold text-on-background truncate">
                                                        {s.name}
                                                    </p>
                                                    <p class="text-[12px] text-on-surface-variant truncate">{meta}</p>
                                                </div>
                                                <button
                                                    class="px-3 py-1.5 bg-primary text-on-primary rounded-lg text-body-sm font-semibold disabled:opacity-60"
                                                    disabled=move || busy.get()
                                                    on:click=move |_| add(id)
                                                >
                                                    "Tambah"
                                                </button>
                                            </div>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                    })
            }}
        </div>
    }
}

// ── Tab JADWAL ────────────────────────────────────────────────────────────────

#[component]
fn JadwalTab(
    class_id: i64,
    d: KelasDetail,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let schedules = d.schedules.clone();
    let weekly = d.weekly_sessions;
    let avg = d.avg_duration_min;
    let status = if schedules.is_empty() {
        "Belum diatur"
    } else {
        "Terjadwal"
    };
    view! {
        <div class="space-y-3 stagger">
            <div class="md:max-w-md">
                <BuatJadwalForm class_id=class_id refetch=refetch />
            </div>

            // ── Statistik jadwal (mockup Jadwal Kelas) ──────────────────────
            <div class="grid grid-cols-3 gap-2 md:max-w-xl">
                <div class="ppm-card p-3 text-center">
                    <span class="material-symbols-outlined text-primary">"calendar_month"</span>
                    <p class="text-headline-sm font-bold text-on-background" data-count=weekly.to_string()>
                        {weekly}
                    </p>
                    <p class="text-[10px] text-on-surface-variant tracking-wide">"Sesi / Minggu"</p>
                </div>
                <div class="ppm-card p-3 text-center">
                    <span class="material-symbols-outlined text-primary">"schedule"</span>
                    <p class="text-headline-sm font-bold text-on-background" data-count=avg.to_string()>
                        {avg}
                    </p>
                    <p class="text-[10px] text-on-surface-variant tracking-wide">"Menit Rata²"</p>
                </div>
                <div class="ppm-card p-3 text-center">
                    <span class="material-symbols-outlined text-primary">"autorenew"</span>
                    <p class="text-body-md font-bold text-on-background mt-2">{status}</p>
                    <p class="text-[10px] text-on-surface-variant tracking-wide">"Rutinitas"</p>
                </div>
            </div>

            <h3 class="text-body-lg font-bold text-on-background pt-1">"Jadwal Terdaftar"</h3>

            {if schedules.is_empty() {
                view! {
                    <EmptyState
                        icon="calendar_month"
                        title="Belum ada jadwal"
                        subtitle="Buat jadwal baru lewat form di atas."
                    />
                }
                    .into_any()
            } else {
                view! {
                    <div class="space-y-3 md:space-y-0 md:grid md:grid-cols-2 md:gap-3">
                        {schedules
                            .into_iter()
                            .map(|s| view! { <JadwalCard s=s refetch=refetch /> })
                            .collect_view()}
                    </div>
                }
                    .into_any()
            }}
        </div>
    }
}

#[component]
fn JadwalCard(s: ScheduleItem, refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let sid = s.id;
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    // Form EDIT jadwal (pre-fill dari data sekarang).
    let editing = RwSignal::new(false);
    let e_title = RwSignal::new(s.title.clone());
    let e_start = RwSignal::new(s.start_hm.clone());
    let e_end = RwSignal::new(s.end_hm.clone());
    let e_limit = RwSignal::new(s.limit_hm.clone());
    let e_rec = RwSignal::new(s.recurrence.clone());
    let e_sd = RwSignal::new(s.start_date.clone());
    let e_ed = RwSignal::new(s.end_date.clone());
    let e_cat = RwSignal::new(s.category.clone());
    let e_late = RwSignal::new(s.late_points.clone());
    let e_absent = RwSignal::new(s.absent_points.clone());
    let e_room = RwSignal::new(s.room.clone());

    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let a = (
            e_title.get_untracked(),
            e_start.get_untracked(),
            e_end.get_untracked(),
            e_limit.get_untracked(),
            e_rec.get_untracked(),
            e_sd.get_untracked(),
            e_ed.get_untracked(),
            e_cat.get_untracked(),
            e_late.get_untracked(),
            e_absent.get_untracked(),
            e_room.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            match update_schedule_action(
                sid, a.0, a.1, a.2, a.3, a.4, a.5, a.6, a.7, a.8, a.9, a.10,
            )
            .await
            {
                Ok(_) => {
                    editing.set(false);
                    refetch();
                }
                Err(e) => {
                    let s = e.to_string();
                    msg.set(Some((
                        false,
                        s.rsplit(": ").next().unwrap_or(&s).to_string(),
                    )));
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
            let _ = delete_schedule_action(sid).await;
            busy.set(false);
            refetch();
        });
    };

    // Ekstrak field tampilan ke lokal (hindari partial-move `s` ke dalam closure).
    let title_ro = s.title.clone();
    let recurrence_label = s.recurrence_label.clone();
    let time_label = s.time_label.clone();
    let date_label = s.date_label.clone();
    let category_ro = s.category.clone();
    let late_ro = s.late_points.clone();
    let absent_ro = s.absent_points.clone();
    let room_ro = s.room.clone();
    let field =
        "w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface";
    view! {
        <div
            class="ppm-card p-4 card-hover anim-in"
            style="border-left:4px solid #064e3b"
        >
            <div class="flex items-center justify-between gap-2">
                <p class="text-body-md font-bold text-on-background truncate">{title_ro}</p>
                <div class="flex items-center gap-2 shrink-0">
                    {(!category_ro.is_empty())
                        .then(|| {
                            view! {
                                <span class="ppm-chip bg-primary/10 text-primary">
                                    {category_ro.clone()}
                                </span>
                            }
                        })}
                    <span class="ppm-chip bg-secondary-container text-primary">
                        {recurrence_label}
                    </span>
                    <button
                        class="w-8 h-8 rounded-lg bg-surface-container text-primary flex items-center justify-center press"
                        on:click=move |_| editing.update(|e| *e = !*e)
                        aria-label="Edit jadwal"
                    >
                        <span class="material-symbols-outlined text-[18px]">"edit"</span>
                    </button>
                </div>
            </div>

            {move || {
                msg.get()
                    .map(|(ok, t)| {
                        let cls = if ok {
                            "mt-2 p-2 bg-secondary-container text-on-secondary-container rounded-lg text-body-sm anim-in"
                        } else {
                            "mt-2 p-2 bg-error-container text-on-error-container rounded-lg text-body-sm anim-in"
                        };
                        view! { <div class=cls>{t}</div> }
                    })
            }}

            {move || {
                if editing.get() {
                    // ── Form EDIT jadwal (tanggal/jam/recurrence) ──────────
                    view! {
                        <form class="mt-3 space-y-2 anim-in" method="post" on:submit=save>
                            <input
                                type="text"
                                class=field
                                placeholder="Nama jadwal"
                                prop:value=move || e_title.get()
                                on:input=move |ev| e_title.set(event_target_value(&ev))
                            />
                            <input
                                type="text"
                                list="kategori-detail"
                                class=field
                                placeholder="Kategori (mis. Pengajian/Sholat) — kosongkan = ikut kategori kelas"
                                prop:value=move || e_cat.get()
                                on:input=move |ev| e_cat.set(event_target_value(&ev))
                            />
                            <input
                                type="text"
                                class=field
                                placeholder="Ruang (mis. Room 302 (Al-Azhar))"
                                prop:value=move || e_room.get()
                                on:input=move |ev| e_room.set(event_target_value(&ev))
                            />
                            <label class="space-y-1 block">
                                <span class="text-[11px] text-on-surface-variant">
                                    "Poin jika telat di jadwal ini (kosongkan = default +2)"
                                </span>
                                <input
                                    type="number"
                                    class=field
                                    placeholder="mis. -5 utk sholat"
                                    prop:value=move || e_late.get()
                                    on:input=move |ev| e_late.set(event_target_value(&ev))
                                />
                            </label>
                            <label class="space-y-1 block">
                                <span class="text-[11px] text-on-surface-variant">
                                    "Poin dipotong jika alpa di jadwal ini (kosongkan = default 15, isi angka positif)"
                                </span>
                                <input
                                    type="number"
                                    min="0"
                                    class=field
                                    placeholder="mis. 20"
                                    prop:value=move || e_absent.get()
                                    on:input=move |ev| e_absent.set(event_target_value(&ev))
                                />
                            </label>
                            <div class="grid grid-cols-2 gap-2">
                                <label class="space-y-1">
                                    <span class="text-[11px] text-on-surface-variant">"Jam mulai"</span>
                                    <input
                                        type="time"
                                        class=field
                                        prop:value=move || e_start.get()
                                        on:input=move |ev| e_start.set(event_target_value(&ev))
                                        required=true
                                    />
                                </label>
                                <label class="space-y-1">
                                    <span class="text-[11px] text-on-surface-variant">"Jam selesai"</span>
                                    <input
                                        type="time"
                                        class=field
                                        prop:value=move || e_end.get()
                                        on:input=move |ev| e_end.set(event_target_value(&ev))
                                        required=true
                                    />
                                </label>
                            </div>
                            <div class="grid grid-cols-2 gap-2">
                                <label class="space-y-1">
                                    <span class="text-[11px] text-on-surface-variant">"Batas terlambat"</span>
                                    <input
                                        type="time"
                                        class=field
                                        prop:value=move || e_limit.get()
                                        on:input=move |ev| e_limit.set(event_target_value(&ev))
                                    />
                                </label>
                                <label class="space-y-1">
                                    <span class="text-[11px] text-on-surface-variant">"Pengulangan"</span>
                                    <select
                                        class=field
                                        on:change=move |ev| e_rec.set(event_target_value(&ev))
                                    >
                                        <option value="daily" selected=move || e_rec.get() == "daily">"Harian"</option>
                                        <option value="weekly" selected=move || e_rec.get() == "weekly">"Mingguan"</option>
                                        <option value="monthly" selected=move || e_rec.get() == "monthly">"Bulanan"</option>
                                        <option value="once" selected=move || e_rec.get() == "once">"Sekali"</option>
                                    </select>
                                </label>
                            </div>
                            <div class="grid grid-cols-2 gap-2">
                                <label class="space-y-1">
                                    <span class="text-[11px] text-on-surface-variant">"Mulai tanggal"</span>
                                    <input
                                        type="date"
                                        class=field
                                        prop:value=move || e_sd.get()
                                        on:input=move |ev| e_sd.set(event_target_value(&ev))
                                        required=true
                                    />
                                </label>
                                <label class="space-y-1">
                                    <span class="text-[11px] text-on-surface-variant">"Selesai (opsional)"</span>
                                    <input
                                        type="date"
                                        class=field
                                        prop:value=move || e_ed.get()
                                        on:input=move |ev| e_ed.set(event_target_value(&ev))
                                    />
                                </label>
                            </div>
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
                        </form>
                    }
                        .into_any()
                } else {
                    // ── Ringkas + Generate/Hapus ───────────────────────────
                    view! {
                        <p class="text-body-sm text-on-surface-variant flex items-center gap-1 mt-1.5">
                            <span class="material-symbols-outlined text-[15px]">"schedule"</span>
                            {time_label.clone()}
                        </p>
                        <p class="text-body-sm text-on-surface-variant flex items-center gap-1">
                            <span class="material-symbols-outlined text-[15px]">"calendar_month"</span>
                            {date_label.clone()}
                        </p>
                        {(!room_ro.is_empty())
                            .then(|| {
                                view! {
                                    <p class="text-body-sm text-on-surface-variant flex items-center gap-1">
                                        <span class="material-symbols-outlined text-[15px]">"room"</span>
                                        {room_ro.clone()}
                                    </p>
                                }
                            })}
                        {(!late_ro.is_empty())
                            .then(|| {
                                view! {
                                    <p class="text-body-sm text-on-surface-variant flex items-center gap-1">
                                        <span class="material-symbols-outlined text-[15px]">"bolt"</span>
                                        "Telat: " {late_ro.clone()} " poin"
                                    </p>
                                }
                            })}
                        {(!absent_ro.is_empty())
                            .then(|| {
                                view! {
                                    <p class="text-body-sm text-on-surface-variant flex items-center gap-1">
                                        <span class="material-symbols-outlined text-[15px]">"gpp_bad"</span>
                                        "Alpa: -" {absent_ro.clone()} " poin"
                                    </p>
                                }
                            })}
                        <div class="flex justify-end mt-3 pt-3 border-t border-outline-variant/40">
                            <button
                                class="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-error-container/60 text-error text-body-sm font-semibold press disabled:opacity-60"
                                disabled=move || busy.get()
                                on:click=del
                            >
                                <span class="material-symbols-outlined text-[18px]">"delete"</span>
                                "Hapus Jadwal"
                            </button>
                        </div>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}

#[component]
fn BuatJadwalForm(class_id: i64, refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let open = RwSignal::new(false);
    let title = RwSignal::new(String::new());
    let start_t = RwSignal::new(String::new());
    let end_t = RwSignal::new(String::new());
    let limit_t = RwSignal::new(String::new());
    let recurrence = RwSignal::new("daily".to_string());
    let start_d = RwSignal::new(String::new());
    let end_d = RwSignal::new(String::new());
    let category = RwSignal::new(String::new());
    let point = RwSignal::new(String::new());
    let absent_point = RwSignal::new(String::new());
    let room = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let args = (
            title.get_untracked(),
            start_t.get_untracked(),
            end_t.get_untracked(),
            limit_t.get_untracked(),
            recurrence.get_untracked(),
            start_d.get_untracked(),
            end_d.get_untracked(),
            category.get_untracked(),
            point.get_untracked(),
            absent_point.get_untracked(),
            room.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            match create_schedule_action(
                class_id, args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8,
                args.9, args.10,
            )
            .await
            {
                Ok(_) => {
                    msg.set(Some((true, "Jadwal dibuat.".into())));
                    title.set(String::new());
                    start_t.set(String::new());
                    end_t.set(String::new());
                    limit_t.set(String::new());
                    start_d.set(String::new());
                    end_d.set(String::new());
                    category.set(String::new());
                    point.set(String::new());
                    absent_point.set(String::new());
                    room.set(String::new());
                    refetch();
                }
                Err(e) => {
                    let m = e.to_string();
                    msg.set(Some((
                        false,
                        m.rsplit(": ").next().unwrap_or(&m).to_string(),
                    )));
                }
            }
            busy.set(false);
        });
    };

    let field =
        "w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface";
    view! {
        {move || {
            if !open.get() {
                return view! {
                    <button
                        class="w-full py-3.5 rounded-2xl bg-primary text-on-primary font-bold flex items-center justify-center gap-2 press"
                        on:click=move |_| open.set(true)
                    >
                        <span class="material-symbols-outlined">"add"</span>
                        "Buat Jadwal Baru"
                    </button>
                }
                    .into_any();
            }
            let field = field;
            view! {
                <form
                    class="ppm-card p-4 space-y-3 anim-in"
                    method="post"
                    on:submit=submit
                >
                    <h3 class="text-body-md font-bold text-on-background">"Jadwal Baru"</h3>
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
                    <input
                        type="text"
                        class=field
                        placeholder="Nama jadwal (mis. Ngaji Subuh)"
                        prop:value=move || title.get()
                        on:input=move |ev| title.set(event_target_value(&ev))
                    />
                    <input
                        type="text"
                        list="kategori-detail"
                        class=field
                        placeholder="Kategori (mis. Pengajian/Sholat) — kosongkan = ikut kategori kelas"
                        prop:value=move || category.get()
                        on:input=move |ev| category.set(event_target_value(&ev))
                    />
                    <input
                        type="text"
                        class=field
                        placeholder="Ruang (mis. Room 302 (Al-Azhar)) — opsional"
                        prop:value=move || room.get()
                        on:input=move |ev| room.set(event_target_value(&ev))
                    />
                    <label class="space-y-1 block">
                        <span class="text-label-md text-on-surface-variant">
                            "Poin jika telat (kosongkan = default +2)"
                        </span>
                        <input
                            type="number"
                            class=field
                            placeholder="mis. -5 utk sholat"
                            prop:value=move || point.get()
                            on:input=move |ev| point.set(event_target_value(&ev))
                        />
                    </label>
                    <label class="space-y-1 block">
                        <span class="text-label-md text-on-surface-variant">
                            "Poin dipotong jika alpa (kosongkan = default 15, isi angka positif)"
                        </span>
                        <input
                            type="number"
                            min="0"
                            class=field
                            placeholder="mis. 20"
                            prop:value=move || absent_point.get()
                            on:input=move |ev| absent_point.set(event_target_value(&ev))
                        />
                    </label>
                    <div class="grid grid-cols-2 gap-2">
                        <label class="space-y-1">
                            <span class="text-label-md text-on-surface-variant">"Jam mulai"</span>
                            <input
                                type="time"
                                class=field
                                prop:value=move || start_t.get()
                                on:input=move |ev| start_t.set(event_target_value(&ev))
                                required=true
                            />
                        </label>
                        <label class="space-y-1">
                            <span class="text-label-md text-on-surface-variant">"Jam selesai"</span>
                            <input
                                type="time"
                                class=field
                                prop:value=move || end_t.get()
                                on:input=move |ev| end_t.set(event_target_value(&ev))
                                required=true
                            />
                        </label>
                    </div>
                    <div class="grid grid-cols-2 gap-2">
                        <label class="space-y-1">
                            <span class="text-label-md text-on-surface-variant">"Batas terlambat"</span>
                            <input
                                type="time"
                                class=field
                                prop:value=move || limit_t.get()
                                on:input=move |ev| limit_t.set(event_target_value(&ev))
                            />
                        </label>
                        <label class="space-y-1">
                            <span class="text-label-md text-on-surface-variant">"Pengulangan"</span>
                            <select
                                class=field
                                on:change=move |ev| recurrence.set(event_target_value(&ev))
                            >
                                <option value="daily">"Harian"</option>
                                <option value="weekly">"Mingguan"</option>
                                <option value="monthly">"Bulanan"</option>
                                <option value="once">"Sekali"</option>
                            </select>
                        </label>
                    </div>
                    <div class="grid grid-cols-2 gap-2">
                        <label class="space-y-1">
                            <span class="text-label-md text-on-surface-variant">"Mulai tanggal"</span>
                            <input
                                type="date"
                                class=field
                                prop:value=move || start_d.get()
                                on:input=move |ev| start_d.set(event_target_value(&ev))
                                required=true
                            />
                        </label>
                        <label class="space-y-1">
                            <span class="text-label-md text-on-surface-variant">"Selesai (opsional)"</span>
                            <input
                                type="date"
                                class=field
                                prop:value=move || end_d.get()
                                on:input=move |ev| end_d.set(event_target_value(&ev))
                            />
                        </label>
                    </div>
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
                            {move || if busy.get() { "Menyimpan…" } else { "Simpan Jadwal" }}
                        </button>
                    </div>
                </form>
            }
                .into_any()
        }}
    }
}

// ── Tab SESI ──────────────────────────────────────────────────────────────────

#[component]
fn SesiTab(
    class_id: i64,
    d: KelasDetail,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let sessions = d.sessions.clone();
    let sched_opts = StoredValue::new(d.schedule_options.clone());
    let teacher_opts = StoredValue::new(d.teacher_options.clone());

    view! {
        <div class="space-y-3 stagger">
            <div class="md:max-w-md">
                <BuatSesiForm
                    class_id=class_id
                    schedule_options=sched_opts
                    teacher_options=teacher_opts
                    refetch=refetch
                />
            </div>

            <p class="text-body-sm text-on-surface-variant">
                "Sesi mendatang dibuat otomatis dari jadwal. Isi pengajar atau tandai libur di bawah."
            </p>

            {if sessions.is_empty() {
                view! {
                    <EmptyState
                        icon="event"
                        title="Belum ada sesi"
                        subtitle="Buat jadwal dulu agar sesi ter-generate otomatis."
                    />
                }
                    .into_any()
            } else {
                view! {
                    <div class="space-y-3 md:space-y-0 md:grid md:grid-cols-2 md:gap-3">
                        {sessions
                            .into_iter()
                            .map(|s| view! { <SesiCard s=s teacher_options=teacher_opts refetch=refetch /> })
                            .collect_view()}
                    </div>
                }
                    .into_any()
            }}
        </div>
    }
}

#[component]
fn SesiCard(
    s: crate::models::SessionItem,
    teacher_options: StoredValue<Vec<TeacherOption>>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let sid = s.id;
    let is_libur = s.status_kind == "cancelled";
    let cur_teacher = s.teacher_id.unwrap_or(0);
    let busy = RwSignal::new(false);

    let badge = if is_libur {
        "ppm-chip bg-error-container text-error"
    } else {
        match s.status_kind.as_str() {
            "ongoing" => "ppm-chip bg-success/10 text-success",
            "finished" => "ppm-chip bg-surface-container-highest text-on-surface-variant",
            _ => "ppm-chip bg-info/10 text-info",
        }
    };
    let status_text = if is_libur {
        "Libur".to_string()
    } else {
        s.status_label.clone()
    };
    let when = format!("{} • {}", s.date_label, s.time_label);

    let set_teacher = move |tid: i64| {
        busy.set(true);
        leptos::task::spawn_local(async move {
            let _ = set_session_teacher_action(sid, tid).await;
            busy.set(false);
            refetch();
        });
    };
    let toggle_libur = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            let _ = set_session_libur_action(sid, !is_libur).await;
            busy.set(false);
            refetch();
        });
    };

    let border = if is_libur {
        "border-left:4px solid #dc2626"
    } else {
        "border-left:4px solid #064e3b"
    };
    let btn_cls = if is_libur {
        "mt-3 w-full py-2 rounded-lg text-body-sm font-semibold press disabled:opacity-60 flex items-center justify-center gap-1.5 bg-secondary-container text-primary"
    } else {
        "mt-3 w-full py-2 rounded-lg text-body-sm font-semibold press disabled:opacity-60 flex items-center justify-center gap-1.5 bg-error-container/60 text-error"
    };

    view! {
        <div
            class="ppm-card p-4 card-hover anim-in"
            style=border
        >
            <div class="flex items-center justify-between gap-2">
                <p class="text-body-md font-bold text-on-background truncate flex-1">{s.title}</p>
                <span class=badge>{status_text}</span>
            </div>
            <p class="text-body-sm text-on-surface-variant flex items-center gap-1 mt-1">
                <span class="material-symbols-outlined text-[15px]">"schedule"</span>
                {when}
            </p>

            {(!is_libur)
                .then(|| {
                    view! {
                        <div class="mt-3 space-y-1">
                            <label class="text-label-md text-on-surface-variant flex items-center gap-1">
                                <span class="material-symbols-outlined text-[15px]">"person"</span>
                                "Pengajar"
                            </label>
                            <select
                                class="w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface disabled:opacity-60"
                                disabled=move || busy.get()
                                on:change=move |ev| set_teacher(
                                    event_target_value(&ev).parse().unwrap_or(0),
                                )
                            >
                                <option value="0" selected=cur_teacher == 0>
                                    "— Belum ditentukan —"
                                </option>
                                {teacher_options
                                    .get_value()
                                    .into_iter()
                                    .map(|o| {
                                        let val = o.id.to_string();
                                        let sel = o.id == cur_teacher;
                                        view! {
                                            <option value=val selected=sel>
                                                {o.name}
                                            </option>
                                        }
                                    })
                                    .collect_view()}
                            </select>
                        </div>
                    }
                })}

            <button
                class=btn_cls
                disabled=move || busy.get()
                on:click=toggle_libur
            >
                <span class="material-symbols-outlined text-[18px]">
                    {if is_libur { "event_available" } else { "event_busy" }}
                </span>
                {if is_libur { "Batalkan Libur (aktifkan)" } else { "Tandai Libur" }}
            </button>
        </div>
    }
}

#[component]
fn BuatSesiForm(
    class_id: i64,
    schedule_options: StoredValue<Vec<ScheduleOption>>,
    teacher_options: StoredValue<Vec<TeacherOption>>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let title = RwSignal::new(String::new());
    let date = RwSignal::new(String::new());
    let sched = RwSignal::new(0i64);
    let teacher = RwSignal::new(0i64);
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let (t, dt, sc, tc) = (
            title.get_untracked(),
            date.get_untracked(),
            sched.get_untracked(),
            teacher.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            match create_session_action(class_id, sc, tc, t, dt).await {
                Ok(_) => {
                    msg.set(Some((true, "Sesi dibuat.".into())));
                    title.set(String::new());
                    date.set(String::new());
                    refetch();
                }
                Err(e) => {
                    let m = e.to_string();
                    msg.set(Some((
                        false,
                        m.rsplit(": ").next().unwrap_or(&m).to_string(),
                    )));
                }
            }
            busy.set(false);
        });
    };

    let field =
        "w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface";
    view! {
        {move || {
            if !open.get() {
                return view! {
                    <button
                        class="w-full py-3.5 rounded-2xl bg-primary text-on-primary font-bold flex items-center justify-center gap-2 press"
                        on:click=move |_| open.set(true)
                    >
                        <span class="material-symbols-outlined">"add"</span>
                        "Buat Sesi Baru"
                    </button>
                }
                    .into_any();
            }
            let field = field;
            view! {
                <form
                    class="ppm-card p-4 space-y-3 anim-in"
                    method="post"
                    on:submit=submit
                >
                    <h3 class="text-body-md font-bold text-on-background">"Sesi Baru"</h3>
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
                    <input
                        type="text"
                        class=field
                        placeholder="Judul sesi (mis. Tadarus Juz 5)"
                        prop:value=move || title.get()
                        on:input=move |ev| title.set(event_target_value(&ev))
                    />
                    <label class="space-y-1 block">
                        <span class="text-label-md text-on-surface-variant">"Tanggal sesi"</span>
                        <input
                            type="date"
                            class=field
                            prop:value=move || date.get()
                            on:input=move |ev| date.set(event_target_value(&ev))
                            required=true
                        />
                    </label>
                    <label class="space-y-1 block">
                        <span class="text-label-md text-on-surface-variant">"Jadwal (opsional)"</span>
                        <select class=field on:change=move |ev| sched.set(event_target_value(&ev).parse().unwrap_or(0))>
                            <option value="0">"— Tanpa jadwal —"</option>
                            {schedule_options
                                .get_value()
                                .into_iter()
                                .map(|o| {
                                    let val = o.id.to_string();
                                    view! { <option value=val>{o.label}</option> }
                                })
                                .collect_view()}
                        </select>
                    </label>
                    <label class="space-y-1 block">
                        <span class="text-label-md text-on-surface-variant">"Pengajar (opsional)"</span>
                        <select class=field on:change=move |ev| teacher.set(event_target_value(&ev).parse().unwrap_or(0))>
                            <option value="0">"— Belum ditentukan —"</option>
                            {teacher_options
                                .get_value()
                                .into_iter()
                                .map(|o| {
                                    let val = o.id.to_string();
                                    view! { <option value=val>{o.name}</option> }
                                })
                                .collect_view()}
                        </select>
                    </label>
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
                            {move || if busy.get() { "Menyimpan…" } else { "Simpan Sesi" }}
                        </button>
                    </div>
                </form>
            }
                .into_any()
        }}
    }
}

// ── Tab KURIKULUM (migrasi 17) ───────────────────────────────────────────────

#[component]
fn KurikulumTab(
    class_id: i64,
    d: KelasDetail,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let items = d.curriculum.clone();

    view! {
        <div class="space-y-3 stagger">
            <div class="md:max-w-md">
                <BuatKurikulumForm class_id=class_id refetch=refetch />
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
                view! {
                    <div class="space-y-3 md:space-y-0 md:grid md:grid-cols-2 md:gap-3">
                        {items
                            .into_iter()
                            .map(|c| view! { <KurikulumCard c=c refetch=refetch /> })
                            .collect_view()}
                    </div>
                }
                    .into_any()
            }}
        </div>
    }
}

#[component]
fn KurikulumCard(c: CurriculumItem, refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let cid = c.id;
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let editing = RwSignal::new(false);
    let e_title = RwSignal::new(c.title.clone());
    let e_desc = RwSignal::new(c.description.clone());
    let e_start = RwSignal::new(c.scope_start.clone());
    let e_end = RwSignal::new(c.scope_end.clone());
    let e_pct = RwSignal::new(c.progress_pct.to_string());
    let e_status = RwSignal::new(c.status.clone());

    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let a = (
            e_title.get_untracked(),
            e_desc.get_untracked(),
            e_start.get_untracked(),
            e_end.get_untracked(),
            e_pct.get_untracked(),
            e_status.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            match update_curriculum_action(cid, a.0, a.1, a.2, a.3, a.4, a.5).await {
                Ok(_) => {
                    editing.set(false);
                    refetch();
                }
                Err(e) => {
                    let s = e.to_string();
                    msg.set(Some((false, s.rsplit(": ").next().unwrap_or(&s).to_string())));
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
    let scope_label = if c.scope_start.is_empty() && c.scope_end.is_empty() {
        String::new()
    } else {
        format!("{} → {}", c.scope_start, c.scope_end)
    };
    let badge = match c.status.as_str() {
        "completed" => "ppm-chip bg-success/10 text-success",
        "upcoming" => "ppm-chip bg-surface-container-highest text-on-surface-variant",
        _ => "ppm-chip bg-primary/10 text-primary",
    };
    let field =
        "w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface";

    view! {
        <div class="ppm-card p-4 card-hover anim-in" style="border-left:4px solid #064e3b">
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
                            <input
                                type="text"
                                class=field
                                placeholder="Judul materi/kitab"
                                prop:value=move || e_title.get()
                                on:input=move |ev| e_title.set(event_target_value(&ev))
                            />
                            <input
                                type="text"
                                class=field
                                placeholder="Sub-judul/topik (opsional)"
                                prop:value=move || e_desc.get()
                                on:input=move |ev| e_desc.set(event_target_value(&ev))
                            />
                            <div class="grid grid-cols-2 gap-2">
                                <input
                                    type="text"
                                    class=field
                                    placeholder="Dari (mis. Al-Fatihah)"
                                    prop:value=move || e_start.get()
                                    on:input=move |ev| e_start.set(event_target_value(&ev))
                                />
                                <input
                                    type="text"
                                    class=field
                                    placeholder="Sampai (mis. Juz 15)"
                                    prop:value=move || e_end.get()
                                    on:input=move |ev| e_end.set(event_target_value(&ev))
                                />
                            </div>
                            <div class="grid grid-cols-2 gap-2">
                                <label class="space-y-1">
                                    <span class="text-[11px] text-on-surface-variant">"Progres (%)"</span>
                                    <input
                                        type="number"
                                        min="0"
                                        max="100"
                                        class=field
                                        prop:value=move || e_pct.get()
                                        on:input=move |ev| e_pct.set(event_target_value(&ev))
                                    />
                                </label>
                                <label class="space-y-1">
                                    <span class="text-[11px] text-on-surface-variant">"Status"</span>
                                    <select class=field on:change=move |ev| e_status.set(event_target_value(&ev))>
                                        <option value="active" selected=move || e_status.get() == "active">"Berjalan"</option>
                                        <option value="completed" selected=move || e_status.get() == "completed">"Selesai"</option>
                                        <option value="upcoming" selected=move || e_status.get() == "upcoming">"Akan Datang"</option>
                                    </select>
                                </label>
                            </div>
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
                            {(!scope_label.is_empty())
                                .then(|| {
                                    view! {
                                        <p class="text-body-sm text-on-surface-variant">{scope_label.clone()}</p>
                                    }
                                })}
                            <div class="flex items-center justify-between text-xs font-semibold mt-2">
                                <span class="text-on-surface-variant">"Progres"</span>
                                <span class="text-on-background">{format!("{pct}%")}</span>
                            </div>
                            <div class="h-2 bg-surface-container rounded-full overflow-hidden mt-1">
                                <div class="h-full bg-primary bar-grow" style=format!("width: {pct}%")></div>
                            </div>
                        </div>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}

#[component]
fn BuatKurikulumForm(class_id: i64, refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let open = RwSignal::new(false);
    let title = RwSignal::new(String::new());
    let desc = RwSignal::new(String::new());
    let scope_start = RwSignal::new(String::new());
    let scope_end = RwSignal::new(String::new());
    let pct = RwSignal::new(String::new());
    let status = RwSignal::new("active".to_string());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let a = (
            title.get_untracked(),
            desc.get_untracked(),
            scope_start.get_untracked(),
            scope_end.get_untracked(),
            pct.get_untracked(),
            status.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            match create_curriculum_action(class_id, a.0, a.1, a.2, a.3, a.4, a.5).await {
                Ok(_) => {
                    msg.set(Some((true, "Materi ditambahkan.".into())));
                    title.set(String::new());
                    desc.set(String::new());
                    scope_start.set(String::new());
                    scope_end.set(String::new());
                    pct.set(String::new());
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
            let field = field;
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
                    <input
                        type="text"
                        class=field
                        placeholder="Judul (mis. Al-Qur'an, Sahih Bukhari)"
                        prop:value=move || title.get()
                        on:input=move |ev| title.set(event_target_value(&ev))
                    />
                    <input
                        type="text"
                        class=field
                        placeholder="Sub-judul/topik (opsional)"
                        prop:value=move || desc.get()
                        on:input=move |ev| desc.set(event_target_value(&ev))
                    />
                    <div class="grid grid-cols-2 gap-2">
                        <input
                            type="text"
                            class=field
                            placeholder="Dari (mis. Al-Fatihah)"
                            prop:value=move || scope_start.get()
                            on:input=move |ev| scope_start.set(event_target_value(&ev))
                        />
                        <input
                            type="text"
                            class=field
                            placeholder="Sampai (mis. Juz 15)"
                            prop:value=move || scope_end.get()
                            on:input=move |ev| scope_end.set(event_target_value(&ev))
                        />
                    </div>
                    <label class="space-y-1 block">
                        <span class="text-label-md text-on-surface-variant">"Progres awal (%)"</span>
                        <input
                            type="number"
                            min="0"
                            max="100"
                            class=field
                            placeholder="0"
                            prop:value=move || pct.get()
                            on:input=move |ev| pct.set(event_target_value(&ev))
                        />
                    </label>
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
