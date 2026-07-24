//! web/pages/kalender.rs — /kalender (semua peran). Kalender akademik bulanan:
//! grid tanggal + daftar sesi hari terpilih. Data di-scope peran di server
//! (staf = semua kelas; santri = kelasnya; ortu = kelas anak terhubung).
//!
//! Bulan berjalan diminta lewat sentinel (0,0) → server balas bulan ini beserta
//! prev/next; klien tak melakukan aritmetika tanggal (hindari ketergantungan
//! chrono di WASM).

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::CalendarItem;
use crate::web::api::academic_calendar_data;
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

                <div class="px-5 pt-5">
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
