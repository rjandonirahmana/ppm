//! web/pages/students.rs — Halaman STUDENTS (gabungan daftar santri + verifikasi).
//!
//! Dua tab: "Daftar Santri" (semua santri + poin/angkatan) dan "Verifikasi"
//! (antrean sesuai peran: pamong → tahap 1, dewan guru/admin → tahap 2). Guru
//! biasa (teacher) hanya melihat daftar. Menggabungkan halaman students &
//! verifikasi lama menjadi satu.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{PendingAtt, StudentRowItem};
use crate::web::api::{decide_pamong, decide_verify, students_data};
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

    view! {
        <Title text="Santri — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto">
                <MobileHeader title="Santri" />

                <div class="px-5 pt-5 space-y-4">
                    <Suspense fallback=|| {
                        view! {
                            <div class="space-y-3 animate-pulse">
                                <div class="h-12 bg-surface-container rounded-xl"></div>
                                <div class="h-20 bg-surface-container rounded-2xl"></div>
                                <div class="h-20 bg-surface-container rounded-2xl"></div>
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
                                            {has_verify
                                                .then(|| {
                                                    view! {
                                                        <div class="flex gap-1 bg-surface-container rounded-xl p-1">
                                                            <TabBtn tab=tab value="list" label="Daftar Santri" badge=0 />
                                                            <TabBtn
                                                                tab=tab
                                                                value="verify"
                                                                label="Verifikasi"
                                                                badge=pending_n
                                                            />
                                                        </div>
                                                    }
                                                })}

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
                                                        <StudentList students=students.get_value() total=total query=query />
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
fn StudentList(students: Vec<StudentRowItem>, total: usize, query: RwSignal<String>) -> impl IntoView {
    let students = StoredValue::new(students);
    view! {
        <p class="text-body-sm text-on-surface-variant">
            "Total " <b class="text-on-background">{total}</b> " santri terdaftar"
        </p>
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
        <div class="space-y-2 stagger">
            {move || {
                let q = query.get().to_lowercase();
                let list: Vec<_> = students
                    .get_value()
                    .into_iter()
                    .filter(|s| q.is_empty() || s.name.to_lowercase().contains(&q) || s.nis.contains(&q))
                    .collect();
                if list.is_empty() {
                    return view! {
                        <div class="bg-surface-container rounded-2xl p-8 text-center text-body-sm text-on-surface-variant">
                            "Tidak ada santri yang cocok."
                        </div>
                    }
                        .into_any();
                }
                list.into_iter()
                    .map(|s| {
                        let meta = format!("NIS: {} • {}", s.nis, s.class_name);
                        let ang = s.angkatan.clone();
                        view! {
                            <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-3 flex items-center gap-3 card-hover anim-in">
                                <div class="w-10 h-10 rounded-full bg-secondary-container flex items-center justify-center text-primary font-bold shrink-0">
                                    {s.initial}
                                </div>
                                <div class="flex-1 min-w-0">
                                    <p class="text-body-md font-semibold text-on-background truncate">{s.name}</p>
                                    <p class="text-body-sm text-on-surface-variant truncate">{meta}</p>
                                    {(!ang.is_empty())
                                        .then(|| {
                                            view! {
                                                <span class="inline-block mt-1 px-2 py-0.5 rounded-full bg-secondary-container text-primary text-[10px] font-bold">
                                                    "Angkatan " {ang}
                                                </span>
                                            }
                                        })}
                                </div>
                                <div class="text-right shrink-0">
                                    <p class="text-body-lg font-bold text-primary">{s.points}</p>
                                    <p class="text-[10px] text-on-surface-variant">"Poin"</p>
                                </div>
                            </div>
                        }
                    })
                    .collect_view()
                    .into_any()
            }}
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
            <div class="grid grid-cols-2 gap-3">
                <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4">
                    <div class="flex items-center gap-2 text-warning">
                        <span class="material-symbols-outlined pulse-dot">"pending_actions"</span>
                        <span class="text-2xl font-bold text-on-background" data-count=pending_n.to_string()>
                            {pending_n}
                        </span>
                    </div>
                    <p class="text-body-sm text-on-surface-variant mt-1">"Menunggu"</p>
                </div>
                <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4">
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
                pending
                    .into_iter()
                    .map(|p| {
                        let id = p.id;
                        let initial = p.name.chars().next().unwrap_or('S').to_string();
                        let meta = format!("NIS: {} • {}", p.nis, p.class_name);
                        let scan = format!("{} • {}", p.time_label, p.gate);
                        let is_busy = move || busy_id.get() == Some(id);
                        view! {
                            <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4 space-y-3 card-hover anim-in">
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
                    .collect_view()
                    .into_any()
            }}
        </div>
    }
}
