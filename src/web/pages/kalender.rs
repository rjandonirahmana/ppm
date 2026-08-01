//! web/pages/kalender.rs — /kalender (semua peran). Kalender akademik bulanan:
//! grid tanggal + daftar sesi hari terpilih. Data di-scope peran di server
//! (staf = semua kelas; santri = kelasnya; ortu = kelas anak terhubung).
//!
//! Bulan berjalan diminta lewat sentinel (0,0) → server balas bulan ini beserta
//! prev/next; klien tak melakukan aritmetika tanggal (hindari ketergantungan
//! chrono di WASM).

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{CalendarItem, SemesterItem, SessionUser};
use crate::web::api::{
    academic_calendar_data, create_semester_action, delete_semester_action, semesters_data,
};
use crate::web::components::{DeviceFrame, FetchError, MobileHeader};

const HARI: [&str; 7] = ["Sen", "Sel", "Rab", "Kam", "Jum", "Sab", "Min"];

fn status_badge(kind: &str) -> &'static str {
    match kind {
        "ongoing" => "ppm-chip-sm bg-success/10 text-success",
        "finished" => "ppm-chip-sm bg-surface-container-highest text-on-surface-variant",
        "cancelled" => "ppm-chip-sm bg-error-container text-error",
        _ => "ppm-chip-sm bg-primary/10 text-primary",
    }
}

#[component]
pub fn KalenderPage() -> impl IntoView {
    // (0,0) = sentinel bulan berjalan (server yang tentukan).
    let ym = RwSignal::new((0i32, 0u32));
    let sel = RwSignal::new(0u32);
    // Reset tanggal terpilih tiap ganti bulan.
    Effect::new(move |_| {
        ym.get();
        sel.set(0);
    });
    let data = Resource::new(move || ym.get(), |(y, m)| async move { academic_calendar_data(y, m).await });

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

    view! {
        <Title text="Kalender Akademik — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Kalender Akademik" subtitle="Jadwal sesi kelas bulan ini" />

                <div class="px-5 pt-5 space-y-4">
                    <SemesterManager />
                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-4">
                                <div class="h-12 bg-surface-container rounded-xl"></div>
                                <div class="h-72 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                    Ok(d) => {
                                        let items = StoredValue::new(d.items.clone());
                                        let month_label = StoredValue::new(d.month_label.clone());
                                        let leading = d.leading_blanks;
                                        let days = d.days_in_month;
                                        let today_day = d.today_day;
                                        let scope_label = d.scope_label.clone();
                                        let active_semester = d.active_semester.clone();
                                        let (py, pm) = (d.prev_year, d.prev_month);
                                        let (ny, nm) = (d.next_year, d.next_month);

                                        // Tanggal efektif: pilihan user, else hari ini, else
                                        // tanggal pertama yang ada sesi, else 1.
                                        let eff_day = move || {
                                            let s = sel.get();
                                            if s != 0 {
                                                s
                                            } else if today_day != 0 {
                                                today_day
                                            } else {
                                                items
                                                    .with_value(|v| v.iter().map(|it| it.day).min())
                                                    .unwrap_or(1)
                                            }
                                        };

                                        view! {
                                            <div class="space-y-4 md:grid md:grid-cols-3 md:gap-5 md:items-start md:space-y-0">
                                                // ── Kalender (kiri, col-span-2) ──────────
                                                <div class="md:col-span-2 space-y-3">
                                                    // Header bulan + navigasi
                                                    <div class="flex items-center justify-between">
                                                        <button
                                                            class="w-9 h-9 rounded-lg bg-surface-container text-on-surface flex items-center justify-center press"
                                                            on:click=move |_| ym.set((py, pm))
                                                            aria-label="Bulan sebelumnya"
                                                        >
                                                            <span class="material-symbols-outlined">"chevron_left"</span>
                                                        </button>
                                                        <div class="text-center">
                                                            <p class="text-body-lg font-bold text-on-background">
                                                                {month_label.get_value()}
                                                            </p>
                                                            <p class="text-[11px] text-on-surface-variant">
                                                                {scope_label}
                                                            </p>
                                                            {(!active_semester.is_empty())
                                                                .then(|| {
                                                                    view! {
                                                                        <span class="mt-1 inline-block px-2 py-0.5 rounded-full bg-secondary-container text-primary text-[10px] font-bold">
                                                                            {active_semester.clone()}
                                                                        </span>
                                                                    }
                                                                })}
                                                        </div>
                                                        <button
                                                            class="w-9 h-9 rounded-lg bg-surface-container text-on-surface flex items-center justify-center press"
                                                            on:click=move |_| ym.set((ny, nm))
                                                            aria-label="Bulan berikutnya"
                                                        >
                                                            <span class="material-symbols-outlined">"chevron_right"</span>
                                                        </button>
                                                    </div>

                                                    <div class="ppm-card p-3">
                                                        // Baris nama hari
                                                        <div class="grid grid-cols-7 gap-1 mb-1">
                                                            {HARI
                                                                .iter()
                                                                .map(|h| {
                                                                    view! {
                                                                        <div class="text-center text-[11px] font-bold text-on-surface-variant py-1">
                                                                            {*h}
                                                                        </div>
                                                                    }
                                                                })
                                                                .collect_view()}
                                                        </div>
                                                        // Grid tanggal
                                                        <div class="grid grid-cols-7 gap-1">
                                                            {(0..leading)
                                                                .map(|_| view! { <div></div> })
                                                                .collect_view()}
                                                            {(1..=days)
                                                                .map(|day| {
                                                                    let count = items
                                                                        .with_value(|v| {
                                                                            v.iter().filter(|it| it.day == day).count()
                                                                        });
                                                                    let is_today = day == today_day;
                                                                    let cls = move || {
                                                                        let selected = eff_day() == day;
                                                                        if selected {
                                                                            "aspect-square rounded-xl flex flex-col items-center justify-center gap-0.5 text-body-sm bg-primary text-on-primary font-bold press"
                                                                        } else if is_today {
                                                                            "aspect-square rounded-xl flex flex-col items-center justify-center gap-0.5 text-body-sm ring-1 ring-primary text-primary font-bold press"
                                                                        } else {
                                                                            "aspect-square rounded-xl flex flex-col items-center justify-center gap-0.5 text-body-sm text-on-surface hover:bg-surface-container press"
                                                                        }
                                                                    };
                                                                    let dot = move || {
                                                                        if count == 0 {
                                                                            return ().into_any();
                                                                        }
                                                                        let selected = eff_day() == day;
                                                                        let d_cls = if selected {
                                                                            "w-1.5 h-1.5 rounded-full bg-on-primary"
                                                                        } else {
                                                                            "w-1.5 h-1.5 rounded-full bg-primary"
                                                                        };
                                                                        view! { <span class=d_cls></span> }.into_any()
                                                                    };
                                                                    view! {
                                                                        <button
                                                                            class=cls
                                                                            on:click=move |_| sel.set(day)
                                                                        >
                                                                            <span>{day}</span>
                                                                            {dot}
                                                                        </button>
                                                                    }
                                                                })
                                                                .collect_view()}
                                                        </div>
                                                    </div>
                                                </div>

                                                // ── Daftar sesi hari terpilih (kanan) ────
                                                <div class="md:bg-surface-container-low md:rounded-2xl md:p-4 space-y-3">
                                                    {move || {
                                                        let ed = eff_day();
                                                        let day_items: Vec<CalendarItem> = items
                                                            .with_value(|v| {
                                                                v.iter().filter(|it| it.day == ed).cloned().collect()
                                                            });
                                                        let header = format!("{ed} {}", month_label.get_value());
                                                        view! {
                                                            <h3 class="text-title-md font-semibold text-on-background">
                                                                {header}
                                                            </h3>
                                                            {if day_items.is_empty() {
                                                                view! {
                                                                    <p class="text-body-sm text-on-surface-variant py-4 text-center">
                                                                        "Tidak ada sesi terjadwal pada tanggal ini."
                                                                    </p>
                                                                }
                                                                    .into_any()
                                                            } else {
                                                                view! {
                                                                    <div class="space-y-2">
                                                                        {day_items
                                                                            .into_iter()
                                                                            .map(|it| view! { <SesiItem it=it /> })
                                                                            .collect_view()}
                                                                    </div>
                                                                }
                                                                    .into_any()
                                                            }}
                                                        }
                                                    }}
                                                </div>
                                            </div>
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
fn SesiItem(it: CalendarItem) -> impl IntoView {
    let meta = format!("{} • {}", it.class_name, it.teacher);
    view! {
        <div class="ppm-card p-3 flex items-center gap-3 anim-in">
            <div class="w-12 shrink-0 text-center">
                <p class="text-body-sm font-bold text-primary leading-none">{it.time_label}</p>
                <p class="text-[10px] text-on-surface-variant mt-0.5 truncate">{it.category}</p>
            </div>
            <div class="flex-1 min-w-0 border-l border-outline-variant/40 pl-3">
                <p class="text-body-sm font-semibold text-on-background truncate">{it.title}</p>
                <p class="text-[11px] text-on-surface-variant truncate">{meta}</p>
            </div>
            <span class=status_badge(&it.status_kind)>{it.status_label}</span>
        </div>
    }
}

/// Panel kelola semester akademik (admin/dewan guru). Tambah semester
/// (ganjil/genap + tahun + rentang tanggal), aktifkan salah satu (jadi acuan
/// kehadiran %/laporan), atau hapus. Tak tampil untuk peran lain.
#[component]
fn SemesterManager() -> impl IntoView {
    let session = use_context::<Resource<Option<SessionUser>>>();
    let can_manage = move || {
        session
            .and_then(|s| s.get())
            .flatten()
            .map(|u| matches!(u.role.as_str(), "admin" | "dewan_guru" | "ketua"))
            .unwrap_or(false)
    };

    let data = Resource::new(|| (), |_| async move { semesters_data().await });
    let open = RwSignal::new(false);
    let kind = RwSignal::new("ganjil".to_string());
    let year = RwSignal::new(String::new());
    let start = RwSignal::new(String::new());
    let end = RwSignal::new(String::new());
    let msg = RwSignal::new(Option::<(bool, String)>::None);
    let busy = RwSignal::new(false);

    let add = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let (k, y, s, e) =
            (kind.get_untracked(), year.get_untracked(), start.get_untracked(), end.get_untracked());
        leptos::task::spawn_local(async move {
            match create_semester_action(k, y, s, e).await {
                Ok(_) => {
                    msg.set(Some((true, "Semester dibuat. Klik \"Aktifkan\" agar jadi acuan.".into())));
                    year.set(String::new());
                    start.set(String::new());
                    end.set(String::new());
                    data.refetch();
                }
                Err(er) => {
                    let m = er.to_string();
                    msg.set(Some((false, m.rsplit(": ").next().unwrap_or(&m).to_string())));
                }
            }
            busy.set(false);
        });
    };
    let del = move |id: i64| {
        leptos::task::spawn_local(async move {
            if delete_semester_action(id).await.is_ok() {
                data.refetch();
            }
        });
    };

    let field = "w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface";
    view! {
        // Resource sesi WAJIB dibaca di dalam Suspense — di luar itu Leptos
        // memperingatkan hydration mismatch (dan kehilangan optimasi SSR).
        <Suspense fallback=|| ()>
        <Show when=can_manage fallback=|| ()>
            <div class="ppm-card p-4 space-y-3">
                <button class="w-full flex items-center justify-between" on:click=move |_| open.update(|o| *o = !*o)>
                    <span class="text-body-md font-bold text-on-background flex items-center gap-2">
                        <span class="material-symbols-outlined text-primary">"event_note"</span>
                        "Kelola Semester"
                    </span>
                    <span
                        class="material-symbols-outlined text-on-surface-variant transition-transform"
                        class:rotate-180=move || open.get()
                    >
                        "expand_more"
                    </span>
                </button>
                {move || {
                    open.get()
                        .then(|| {
                            view! {
                                <div class="space-y-3 pt-1">
                                    <form class="space-y-2" method="post" on:submit=add>
                                        <div class="grid grid-cols-2 gap-2">
                                            <select
                                                class=field
                                                prop:value=move || kind.get()
                                                on:change=move |e| kind.set(event_target_value(&e))
                                            >
                                                <option value="ganjil">"Ganjil"</option>
                                                <option value="genap">"Genap"</option>
                                            </select>
                                            <input
                                                class=field
                                                placeholder="Tahun (mis. 2026)"
                                                inputmode="numeric"
                                                prop:value=move || year.get()
                                                on:input=move |e| year.set(event_target_value(&e))
                                            />
                                        </div>
                                        <div class="grid grid-cols-2 gap-2">
                                            <label class="block text-[11px] text-on-surface-variant">
                                                "Mulai"
                                                <input
                                                    type="date"
                                                    class=field
                                                    prop:value=move || start.get()
                                                    on:input=move |e| start.set(event_target_value(&e))
                                                />
                                            </label>
                                            <label class="block text-[11px] text-on-surface-variant">
                                                "Selesai"
                                                <input
                                                    type="date"
                                                    class=field
                                                    min=move || start.get()
                                                    prop:value=move || end.get()
                                                    on:input=move |e| end.set(event_target_value(&e))
                                                />
                                            </label>
                                        </div>
                                        <p class="text-[10px] text-on-surface-variant">
                                            "Rentang tak boleh bertabrakan/mundur. Semester yang sedang berjalan terdeteksi otomatis dari tanggal — tak perlu diaktifkan."
                                        </p>
                                        {move || {
                                            msg.get()
                                                .map(|(ok, t)| {
                                                    let cls = if ok {
                                                        "p-2 bg-secondary-container text-on-secondary-container rounded-lg text-[11px]"
                                                    } else {
                                                        "p-2 bg-error-container text-on-error-container rounded-lg text-[11px]"
                                                    };
                                                    view! { <div class=cls>{t}</div> }
                                                })
                                        }}
                                        <button
                                            type="submit"
                                            class="w-full py-2.5 rounded-lg bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                                            disabled=move || busy.get()
                                        >
                                            {move || if busy.get() { "Menyimpan…" } else { "Tambah Semester" }}
                                        </button>
                                    </form>
                                    <Suspense fallback=|| ()>
                                        {move || {
                                            data.get()
                                                .map(|res| match res {
                                                    Ok(list) if list.is_empty() => {
                                                        view! {
                                                            <p class="text-[11px] text-on-surface-variant">
                                                                "Belum ada semester ditetapkan."
                                                            </p>
                                                        }
                                                            .into_any()
                                                    }
                                                    Ok(list) => {
                                                        view! {
                                                            <div class="space-y-1.5">
                                                                {list
                                                                    .into_iter()
                                                                    .map(|s| view! { <SemesterRow s=s del=del /> })
                                                                    .collect_view()}
                                                            </div>
                                                        }
                                                            .into_any()
                                                    }
                                                    Err(_) => ().into_any(),
                                                })
                                        }}
                                    </Suspense>
                                </div>
                            }
                        })
                }}
            </div>
        </Show>
        </Suspense>
    }
}

#[component]
fn SemesterRow(s: SemesterItem, del: impl Fn(i64) + Copy + Send + 'static) -> impl IntoView {
    let id = s.id;
    // is_active = SEDANG BERJALAN (dihitung dari tanggal hari ini di server).
    let running = s.is_active;
    view! {
        <div class="flex items-center gap-2 bg-surface-container rounded-lg px-3 py-2">
            <div class="flex-1 min-w-0">
                <p class="text-body-sm font-semibold text-on-background truncate">{s.label}</p>
                <p class="text-[10px] text-on-surface-variant">
                    {format!("{} → {}", s.start_date, s.end_date)}
                </p>
            </div>
            {running
                .then(|| {
                    view! {
                        <span class="text-[10px] font-bold text-success bg-success/10 px-2 py-0.5 rounded-full">
                            "SEDANG BERJALAN"
                        </span>
                    }
                })}
            <button
                class="w-8 h-8 rounded-lg text-error hover:bg-error-container flex items-center justify-center press"
                on:click=move |_| del(id)
                aria-label="Hapus semester"
            >
                <span class="material-symbols-outlined text-[18px]">"delete"</span>
            </button>
        </div>
    }
}
