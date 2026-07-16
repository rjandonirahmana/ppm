//! web/pages/ortu_riwayat.rs — Riwayat Kehadiran anak (sisi ORANG TUA, mockup):
//! chip pemilih anak, ringkasan bulan (tingkat kehadiran + hari hadir),
//! kartu terlambat/izin, daftar riwayat per bulan.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::RiwayatItem;
use crate::web::api::{child_riwayat, parent_home};
use crate::web::components::{DeviceFrame, MobileHeader, MobileNav, NAV_ORTU};

#[component]
pub fn OrtuRiwayatPage() -> impl IntoView {
    let selected = RwSignal::new(Option::<i64>::None);
    let home = Resource::new(|| (), |_| async move { parent_home(None).await });

    // Pilih anak pertama otomatis begitu daftar anak termuat.
    Effect::new(move |_| {
        if selected.get().is_none() {
            if let Some(Ok(h)) = home.get() {
                if let Some(first) = h.children.first() {
                    selected.set(Some(first.id));
                }
            }
        }
    });

    let data = Resource::new(
        move || selected.get(),
        |c| async move {
            match c {
                Some(id) => child_riwayat(id).await.map(Some),
                None => Ok(None),
            }
        },
    );

    Effect::new(move |_| {
        if let Some(Err(e)) = home.get() {
            let msg = e.to_string();
            if msg.contains("unauth") || msg.contains("forbidden") {
                #[cfg(target_arch = "wasm32")]
                if let Some(w) = web_sys::window() {
                    let _ = w.location().replace("/login");
                }
            }
        }
    });

    view! {
        <Title text="Riwayat Kehadiran Anak — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto">
                <MobileHeader title="Riwayat Kehadiran" />

                <div class="px-5 pt-5 space-y-4 stagger">
                    // ── Chip anak ──────────────────────────────────────────
                    <Suspense fallback=|| ()>
                        {move || {
                            home.get()
                                .and_then(|r| r.ok())
                                .map(|h| {
                                    if h.children.is_empty() {
                                        return view! {
                                            <div class="bg-surface-container rounded-2xl p-8 text-center">
                                                <p class="text-body-md text-on-surface-variant">
                                                    "Belum ada santri terhubung."
                                                </p>
                                                <a href="/orang-tua" class="text-primary font-bold text-body-sm mt-2 inline-block">
                                                    "Hubungkan sekarang →"
                                                </a>
                                            </div>
                                        }
                                            .into_any();
                                    }
                                    view! {
                                        <div class="flex gap-2 overflow-x-auto pb-1">
                                            {h.children
                                                .into_iter()
                                                .map(|c| {
                                                    let id = c.id;
                                                    let cls = move || {
                                                        if selected.get() == Some(id) {
                                                            "px-4 py-2.5 rounded-full bg-primary text-on-primary text-body-sm font-semibold whitespace-nowrap shrink-0 press"
                                                        } else {
                                                            "px-4 py-2.5 rounded-full bg-surface-container text-on-surface-variant text-body-sm whitespace-nowrap shrink-0 press"
                                                        }
                                                    };
                                                    view! {
                                                        <button class=cls on:click=move |_| selected.set(Some(id))>
                                                            {c.name}
                                                        </button>
                                                    }
                                                })
                                                .collect_view()}
                                        </div>
                                    }
                                        .into_any()
                                })
                        }}
                    </Suspense>

                    // ── Data riwayat anak terpilih ─────────────────────────
                    <Suspense fallback=|| {
                        view! {
                            <div class="space-y-3 animate-pulse">
                                <div class="h-32 bg-surface-container rounded-2xl"></div>
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .and_then(|r| r.ok())
                                .flatten()
                                .map(|cr| {
                                    let d = cr.data;
                                    let name = cr.child.name;
                                    let total = d.hadir + d.izin + d.alpa;
                                    let pct = if total > 0 { (d.hadir * 100 / total) as i32 } else { 0 };
                                    view! {
                                        // Ringkasan
                                        <div class="spiritual-gradient rounded-2xl p-5 text-on-primary shadow-lg shadow-primary/20">
                                            <p class="text-body-sm opacity-85">"Ringkasan Semester Ini"</p>
                                            <p class="text-display-md mt-0.5">{name}</p>
                                            <div class="flex items-center gap-6 mt-4">
                                                <div>
                                                    <p class="text-3xl font-bold">
                                                        <span data-count=pct.to_string()>{pct}</span>
                                                        "%"
                                                    </p>
                                                    <p class="text-[10px] tracking-[0.15em] opacity-80 mt-0.5">
                                                        "TINGKAT KEHADIRAN"
                                                    </p>
                                                </div>
                                                <div class="w-px h-10 bg-white/20"></div>
                                                <div>
                                                    <p class="text-3xl font-bold" data-count=d.hadir.to_string()>
                                                        {d.hadir}
                                                    </p>
                                                    <p class="text-[10px] tracking-[0.15em] opacity-80 mt-0.5">
                                                        "SESI HADIR"
                                                    </p>
                                                </div>
                                            </div>
                                        </div>

                                        // Kartu izin/alpa ringkas
                                        <div class="grid grid-cols-2 gap-3">
                                            <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4 card-hover">
                                                <div class="w-10 h-10 rounded-xl bg-info/10 text-info flex items-center justify-center">
                                                    <span class="material-symbols-outlined">"event_busy"</span>
                                                </div>
                                                <div class="flex items-end justify-between mt-2">
                                                    <p class="text-[11px] font-bold tracking-[0.12em] text-on-surface-variant">
                                                        "IZIN"
                                                    </p>
                                                    <p class="text-2xl font-bold text-info" data-count=d.izin.to_string()>
                                                        {d.izin}
                                                    </p>
                                                </div>
                                            </div>
                                            <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4 card-hover">
                                                <div class="w-10 h-10 rounded-xl bg-error-container text-error flex items-center justify-center">
                                                    <span class="material-symbols-outlined">"cancel"</span>
                                                </div>
                                                <div class="flex items-end justify-between mt-2">
                                                    <p class="text-[11px] font-bold tracking-[0.12em] text-on-surface-variant">
                                                        "ALPA"
                                                    </p>
                                                    <p class="text-2xl font-bold text-error" data-count=d.alpa.to_string()>
                                                        {d.alpa}
                                                    </p>
                                                </div>
                                            </div>
                                        </div>

                                        // Daftar per bulan
                                        <RiwayatList items=d.items />
                                    }
                                })
                        }}
                    </Suspense>
                </div>

                <MobileNav items=NAV_ORTU active="/orang-tua/riwayat" />
            </div>
        </DeviceFrame>
    }
}

#[component]
fn RiwayatList(items: Vec<RiwayatItem>) -> impl IntoView {
    if items.is_empty() {
        return view! {
            <div class="bg-surface-container rounded-2xl p-8 text-center text-body-sm text-on-surface-variant">
                "Belum ada catatan kehadiran."
            </div>
        }
            .into_any();
    }
    let mut out: Vec<AnyView> = Vec::new();
    let mut last_month = String::new();
    for it in items {
        if it.month != last_month {
            last_month = it.month.clone();
            let m = it.month.clone();
            out.push(
                view! {
                    <div class="flex items-center gap-2 pt-2">
                        <span class="material-symbols-outlined text-on-surface-variant text-xl">
                            "calendar_month"
                        </span>
                        <h3 class="text-body-lg font-bold text-on-background">{m}</h3>
                    </div>
                }
                    .into_any(),
            );
        }
        let border = match it.kind.as_str() {
            "late" => "border-left:4px solid #f59e0b",
            "permit" => "border-left:4px solid #2563eb",
            "absent" => "border-left:4px solid #dc2626",
            _ => "border-left:4px solid #059669",
        };
        let badge = match it.kind.as_str() {
            "late" => "px-2.5 py-1 rounded-full text-[10px] font-bold tracking-wider bg-warning/10 text-warning",
            "permit" => "px-2.5 py-1 rounded-full text-[10px] font-bold tracking-wider bg-info/10 text-info",
            "absent" => "px-2.5 py-1 rounded-full text-[10px] font-bold tracking-wider bg-error-container text-error",
            _ => "px-2.5 py-1 rounded-full text-[10px] font-bold tracking-wider bg-success/10 text-success",
        };
        out.push(
            view! {
                <div
                    class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4 card-hover anim-in"
                    style=border
                >
                    <div class="flex items-center gap-2">
                        <p class="text-body-md font-bold text-on-background truncate flex-1">{it.title}</p>
                        <span class=badge>{it.status_label}</span>
                    </div>
                    <p class="text-body-sm text-on-surface-variant flex items-center gap-1 mt-1.5">
                        <span class="material-symbols-outlined text-[15px]">"schedule"</span>
                        {it.time_label}
                    </p>
                </div>
            }
                .into_any(),
        );
    }
    out.into_any()
}
