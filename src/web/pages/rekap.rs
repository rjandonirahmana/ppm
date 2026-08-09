//! web/pages/rekap.rs — Rekap kehadiran mingguan per-santri (kontrol staf).
//! Filter per kelas & angkatan (klien), navigasi antar-pekan (server offset).

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{
    prestasi_label, PemanggilanItem, SessionUser, SpItem, WeeklyRecapRow, WeeklyRewardRow,
};
use crate::web::api::{credit_weekly_rewards_action, weekly_recap_data};
use crate::web::components::{
    DeviceFrame, EmptyState, FetchError, MobileHeader, SwipeArea, SwipeHint,
};

#[component]
pub fn RekapMingguanPage() -> impl IntoView {
    // Offset pekan (0 = pekan ini) → memicu refetch resource.
    let offset = RwSignal::new(0_i32);
    let data = Resource::new(move || offset.get(), |o| async move { weekly_recap_data(o).await });

    // Peran (dari sesi global) → tombol kredit reward hanya untuk admin.
    let session = use_context::<Resource<Option<SessionUser>>>();
    let is_admin = move || {
        session
            .and_then(|s| s.get())
            .flatten()
            .map(|u| matches!(u.role.as_str(), "admin" | "ketua")) // ketua = admin
            .unwrap_or(false)
    };

    // Kredit reward mingguan (admin).
    let crediting = RwSignal::new(false);
    let credit_msg = RwSignal::new(String::new());
    let do_credit = move |_| {
        if crediting.get_untracked() {
            return;
        }
        crediting.set(true);
        credit_msg.set(String::new());
        let o = offset.get_untracked();
        leptos::task::spawn_local(async move {
            match credit_weekly_rewards_action(o).await {
                Ok((n, total)) => {
                    credit_msg.set(format!("{n} santri dikreditkan (+{total} poin)."));
                    data.refetch();
                }
                Err(e) => credit_msg.set(e.to_string()),
            }
            crediting.set(false);
        });
    };

    crate::web::components::guard_sesi(data);

    // Filter klien.
    let class_filter = RwSignal::new(String::new());
    let angkatan_filter = RwSignal::new(String::new());

    view! {
        <Title text="Rekap Mingguan — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader
                    title="Rekap Kehadiran Mingguan"
                    subtitle="Ringkasan kehadiran santri per pekan"
                    back_href="/staf"
                />

                <div class="px-5 pt-5 md:px-8 md:pt-7 space-y-5 stagger">
                    // ── Navigasi pekan ─────────────────────────────────
                    // Geser kiri/kanan = pindah pekan, sama seperti kalender.
                    // Tombol panah TETAP ada dan bukan pelengkap: gestur tak
                    // bisa dijangkau papan ketik maupun pembaca layar, dan di
                    // desktop tak ada yang menggeser apa pun (WCAG 2.5.1).
                    //
                    // Arah geseran mengikuti waktu, bukan daftar: geser ke
                    // KANAN membawa mundur ke pekan sebelumnya — arah yang sama
                    // dengan tombol panah kiri di sebelahnya.
                    <SwipeArea
                        on_prev=move || offset.update(|o| *o += 1)
                        on_next=move || offset.update(|o| *o = (*o - 1).max(0))
                        hint_key="rekap"
                        // Yang bisa digeser adalah BILAH PERIODE ini, bukan
                        // seluruh badan halaman. Dua alasan: `.ppm-swipe-area`
                        // mematikan seleksi teks (perlu, supaya menyeret tak
                        // menyorot tulisan) — dan halaman ini penuh nama serta
                        // angka yang wajar disalin orang; lalu daftarnya
                        // panjang, jadi geseran tak sengaja di tengah daftar
                        // memindahkan pekan tanpa pengguna melihat kepala
                        // periodenya berubah. `py-2` membuat bilahnya ±56px,
                        // di atas ambang target sentuh 44px.
                        class="flex items-center justify-between gap-2 py-2"
                    >
                        <button
                            class="ppm-nav-btn press"
                            aria-label="Pekan sebelumnya"
                            on:click=move |_| offset.update(|o| *o += 1)
                        >
                            <span class="material-symbols-outlined">"chevron_left"</span>
                        </button>
                        <div class="text-center">
                            <Suspense fallback=|| {
                                view! { <span class="text-body-md font-bold text-on-background">"…"</span> }
                            }>
                                {move || {
                                    data.get()
                                        .and_then(|r| r.ok())
                                        .map(|d| {
                                            view! {
                                                <p class="text-body-md font-bold text-on-background">{d.week_label}</p>
                                                <p class="text-[11px] text-on-surface-variant">
                                                    {if d.offset == 0 {
                                                        "Pekan ini".to_string()
                                                    } else {
                                                        format!("{} pekan lalu", d.offset)
                                                    }}
                                                </p>
                                            }
                                        })
                                }}
                            </Suspense>
                        </div>
                        <button
                            class="ppm-nav-btn press"
                            aria-label="Pekan berikutnya"
                            prop:disabled=move || offset.get() == 0
                            on:click=move |_| offset.update(|o| *o = (*o - 1).max(0))
                        >
                            <span class="material-symbols-outlined">"chevron_right"</span>
                        </button>
                    </SwipeArea>
                    <SwipeHint key="rekap" teks="Geser untuk ganti pekan" />

                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="grid grid-cols-2 gap-3 md:max-w-lg">
                                    <div class="h-20 bg-surface-container rounded-2xl"></div>
                                    <div class="h-20 bg-surface-container rounded-2xl"></div>
                                </div>
                                <div class="h-64 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                    Ok(d) => {
                                        let cf = class_filter.get();
                                        let af = angkatan_filter.get();
                                        let filtered: Vec<WeeklyRecapRow> = d
                                            .rows
                                            .iter()
                                            .filter(|r| {
                                                (cf.is_empty() || r.class_name == cf)
                                                    && (af.is_empty() || r.angkatan == af)
                                            })
                                            .cloned()
                                            .collect();
                                        let shown = filtered.len();
                                        let rewards = d.rewards.clone();
                                        let rewards_total = d.rewards_total;
                                        let rewards_pending = d.rewards_pending;
                                        let has_rewards = !rewards.is_empty();
                                        let pemanggilan = d.pemanggilan.clone();
                                        let has_pemanggilan = !pemanggilan.is_empty();
                                        let sp_list = d.sp_list.clone();
                                        let has_sp = !sp_list.is_empty();
                                        view! {
                                            // Ringkasan
                                            <div class="grid grid-cols-2 gap-3 md:max-w-lg">
                                                <div class="ppm-card p-4">
                                                    <div class="flex items-center gap-2 text-primary">
                                                        <span class="material-symbols-outlined">"groups"</span>
                                                        <span
                                                            class="text-2xl font-bold text-on-background"
                                                            data-count=d.total_santri.to_string()
                                                        >
                                                            {d.total_santri}
                                                        </span>
                                                    </div>
                                                    <p class="text-body-sm text-on-surface-variant mt-1">"Total Santri"</p>
                                                </div>
                                                <div class="ppm-card p-4">
                                                    <div class="flex items-center gap-2 text-success">
                                                        <span class="material-symbols-outlined">"trending_up"</span>
                                                        <span
                                                            class="text-2xl font-bold text-on-background"
                                                            data-count=d.avg_pct.to_string()
                                                        >
                                                            {d.avg_pct}"%"
                                                        </span>
                                                    </div>
                                                    <p class="text-body-sm text-on-surface-variant mt-1">"Rata-rata Hadir"</p>
                                                </div>
                                            </div>

                                            // Filter
                                            <div class="flex flex-col sm:flex-row gap-2">
                                                <select
                                                    class="flex-1 rounded-xl border border-outline-variant bg-surface px-3 py-2.5 text-body-md text-on-background focus:outline-none focus:ring-2 focus:ring-primary cursor-pointer"
                                                    on:change=move |e| class_filter.set(event_target_value(&e))
                                                >
                                                    <option value="">"Semua Kelas"</option>
                                                    {d
                                                        .classes
                                                        .iter()
                                                        .map(|c| {
                                                            let c2 = c.clone();
                                                            view! { <option value=c.clone()>{c2}</option> }
                                                        })
                                                        .collect_view()}
                                                </select>
                                                <select
                                                    class="flex-1 rounded-xl border border-outline-variant bg-surface px-3 py-2.5 text-body-md text-on-background focus:outline-none focus:ring-2 focus:ring-primary cursor-pointer"
                                                    on:change=move |e| angkatan_filter.set(event_target_value(&e))
                                                >
                                                    <option value="">"Semua Angkatan"</option>
                                                    {d
                                                        .angkatans
                                                        .iter()
                                                        .map(|a| {
                                                            let a2 = a.clone();
                                                            view! { <option value=a.clone()>{a2}</option> }
                                                        })
                                                        .collect_view()}
                                                </select>
                                            </div>

                                            {if shown == 0 {
                                                view! {
                                                    <EmptyState
                                                        icon="event_busy"
                                                        title="Tidak ada data"
                                                        subtitle="Tidak ada santri untuk filter/pekan ini."
                                                    />
                                                }
                                                    .into_any()
                                            } else {
                                                view! {
                                                    // Tabel rekap (satu kartu, baris divide-y)
                                                    <div class="ppm-card overflow-hidden">
                                                        <div class="hidden md:grid grid-cols-[2fr_repeat(5,1fr)] gap-2 px-4 py-2.5 bg-surface-container-low text-[11px] font-bold tracking-wider text-on-surface-variant uppercase">
                                                            <span>"Santri"</span>
                                                            <span class="text-center">"Hadir"</span>
                                                            <span class="text-center">"Telat"</span>
                                                            <span class="text-center">"Izin"</span>
                                                            <span class="text-center">"Alpa"</span>
                                                            <span class="text-center">"%"</span>
                                                        </div>
                                                        <div class="divide-y divide-outline-variant/60">
                                                            {filtered
                                                                .into_iter()
                                                                .map(|r| view! { <RecapRow r=r /> })
                                                                .collect_view()}
                                                        </div>
                                                    </div>
                                                }
                                                    .into_any()
                                            }}

                                            // Panel kontrol mingguan: 3 kolom sejajar di desktop.
                                            <div class="space-y-5 md:space-y-0 md:grid md:grid-cols-3 md:gap-5 md:items-start">
                                            // ── Reward Mingguan (PRD) ──────────────────
                                            <div class="ppm-card p-4 space-y-3">
                                                <div class="flex items-center justify-between gap-2">
                                                    <div class="flex items-center gap-2">
                                                        <span class="material-symbols-outlined text-primary">"redeem"</span>
                                                        <h3 class="text-body-lg font-bold text-on-background">"Reward Mingguan"</h3>
                                                    </div>
                                                    <span class="ppm-chip bg-success/10 text-success">
                                                        {format!("Total +{rewards_total}")}
                                                    </span>
                                                </div>
                                                <p class="text-[11px] text-on-surface-variant">
                                                    "No-Alfa / No-Telat / Full-Hadir per kegiatan (KBM/Non-KBM/Piket). Kreditkan tiap Senin untuk pekan sebelumnya."
                                                </p>

                                                // Suspense: is_admin membaca resource sesi.
                                                // Fallback kosong = tombol tak muncul selama sesi
                                                // belum termuat — aman, karena server tetap
                                                // memeriksa peran saat tombolnya ditekan.
                                                <Suspense fallback=|| ()>
                                                <Show
                                                    when=move || { is_admin() && rewards_pending > 0 }
                                                    fallback=|| ().into_any()
                                                >
                                                    <button
                                                        class="px-5 py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-sm cursor-pointer press disabled:opacity-60"
                                                        prop:disabled=move || crediting.get()
                                                        on:click=do_credit
                                                    >
                                                        {move || {
                                                            if crediting.get() {
                                                                "Memproses…".to_string()
                                                            } else {
                                                                format!("Kreditkan {rewards_pending} santri ke saldo")
                                                            }
                                                        }}
                                                    </button>
                                                </Show>
                                                </Suspense>
                                                <Show when=move || !credit_msg.get().is_empty() fallback=|| ().into_any()>
                                                    <p class="text-body-sm text-on-surface-variant">{move || credit_msg.get()}</p>
                                                </Show>

                                                {if has_rewards {
                                                    view! {
                                                        <div class="space-y-2">
                                                            {rewards
                                                                .into_iter()
                                                                .map(|r| view! { <RewardRow r=r /> })
                                                                .collect_view()}
                                                        </div>
                                                    }
                                                        .into_any()
                                                } else {
                                                    view! {
                                                        <p class="text-body-sm text-on-surface-variant py-1">
                                                            "Belum ada santri yang memenuhi syarat reward pekan ini."
                                                        </p>
                                                    }
                                                        .into_any()
                                                }}
                                            </div>

                                            // ── Pemanggilan Mingguan (PRD hal. 12) ─────
                                            <div class="ppm-card p-4 space-y-3">
                                                <div class="flex items-center justify-between gap-2">
                                                    <div class="flex items-center gap-2">
                                                        <span class="material-symbols-outlined text-error">"campaign"</span>
                                                        <h3 class="text-body-lg font-bold text-on-background">"Pemanggilan Mingguan"</h3>
                                                    </div>
                                                    <span class="ppm-chip bg-error/10 text-error">
                                                        {format!("{} santri", pemanggilan.len())}
                                                    </span>
                                                </div>
                                                <p class="text-[11px] text-on-surface-variant">
                                                    "Net poin ≤ -9 pekan ini. Pemanggil: KoorSantri (≤-9), Pamong (≤-12), Wali Kelas (≤-18)."
                                                </p>
                                                {if has_pemanggilan {
                                                    view! {
                                                        <div class="space-y-2">
                                                            {pemanggilan
                                                                .into_iter()
                                                                .map(|p| view! { <PemanggilanRow p=p /> })
                                                                .collect_view()}
                                                        </div>
                                                    }
                                                        .into_any()
                                                } else {
                                                    view! {
                                                        <p class="text-body-sm text-on-surface-variant py-1">
                                                            "Tidak ada santri yang perlu dipanggil pekan ini. 🎉"
                                                        </p>
                                                    }
                                                        .into_any()
                                                }}
                                            </div>

                                            // ── Santri SP (PRD hal. 13-14) ─────────────
                                            <div class="ppm-card p-4 space-y-3">
                                                <div class="flex items-center justify-between gap-2">
                                                    <div class="flex items-center gap-2">
                                                        <span class="material-symbols-outlined text-error">"gavel"</span>
                                                        <h3 class="text-body-lg font-bold text-on-background">"Santri SP"</h3>
                                                    </div>
                                                    <span class="ppm-chip bg-error/10 text-error">
                                                        {format!("{} santri", sp_list.len())}
                                                    </span>
                                                </div>
                                                <p class="text-[11px] text-on-surface-variant">
                                                    "Berdasar sisa saldo: SP1 ≤150, SP2 ≤100, SP3 ≤50. Pemanggilan SP tiap Rabu malam, dipantau 1 bulan."
                                                </p>
                                                {if has_sp {
                                                    view! {
                                                        <div class="space-y-2">
                                                            {sp_list
                                                                .into_iter()
                                                                .map(|s| view! { <SpRow s=s /> })
                                                                .collect_view()}
                                                        </div>
                                                    }
                                                        .into_any()
                                                } else {
                                                    view! {
                                                        <p class="text-body-sm text-on-surface-variant py-1">
                                                            "Tidak ada santri berstatus SP. 🎉"
                                                        </p>
                                                    }
                                                        .into_any()
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
fn RecapRow(r: WeeklyRecapRow) -> impl IntoView {
    let pct = r.pct;
    let pct_cls = if pct >= 80 {
        "text-success"
    } else if pct >= 50 {
        "text-warning"
    } else {
        "text-error"
    };
    let meta = if r.nis.is_empty() {
        r.class_name.clone()
    } else {
        format!("{} • {}", r.nis, r.class_name)
    };
    let (prestasi, pk) = prestasi_label(r.points);
    // Kelas literal (Tailwind tak bisa deteksi nama kelas dinamis).
    let prestasi_cls = match pk {
        "success" => "ppm-chip-sm bg-success/10 text-success shrink-0",
        "info" => "ppm-chip-sm bg-info/10 text-info shrink-0",
        "primary" => "ppm-chip-sm bg-primary/10 text-primary shrink-0",
        "warning" => "ppm-chip-sm bg-warning/10 text-warning shrink-0",
        _ => "ppm-chip-sm bg-error/10 text-error shrink-0",
    };
    view! {
        <div class="px-4 py-3 hover:bg-surface-container-low transition-colors">
            // Desktop: grid selaras header; Mobile: nama + baris statistik chip.
            <div class="md:grid md:grid-cols-[2fr_repeat(5,1fr)] md:gap-2 md:items-center">
                <div class="min-w-0">
                    <div class="flex items-center gap-2">
                        <p class="text-body-md font-semibold text-on-background truncate">{r.name}</p>
                        <span class=prestasi_cls title=format!("Saldo {} poin", r.points)>{prestasi}</span>
                    </div>
                    <p class="text-[11px] text-on-surface-variant truncate">{meta}</p>
                </div>
                // Angka (desktop kolom; mobile chip row)
                <div class="flex md:contents gap-2 mt-2 md:mt-0">
                    <Stat label="Hadir" value=r.hadir cls="text-success" />
                    <Stat label="Telat" value=r.telat cls="text-warning" />
                    <Stat label="Izin" value=r.izin cls="text-info" />
                    <Stat label="Alpa" value=r.alpa cls="text-error" />
                    <div class="flex-1 md:flex-none text-center">
                        <span class=format!("text-body-md font-bold {pct_cls}")>{pct}"%"</span>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn SpRow(s: SpItem) -> impl IntoView {
    let badge = if s.level_kind == "warning" {
        "ppm-chip-sm bg-warning/10 text-warning shrink-0"
    } else {
        "ppm-chip-sm bg-error/10 text-error shrink-0"
    };
    let meta = if s.nis.is_empty() {
        s.class_name.clone()
    } else {
        format!("{} • {}", s.nis, s.class_name)
    };
    view! {
        <div class="flex items-center gap-3 bg-surface-container-low rounded-xl px-3.5 py-2.5">
            <div class="flex-1 min-w-0">
                <p class="text-body-md font-semibold text-on-background truncate">{s.name}</p>
                <p class="text-[11px] text-on-surface-variant truncate">{meta}</p>
                <p class="text-[10px] text-on-surface-variant truncate">{s.treatment}</p>
            </div>
            <span class="text-body-md font-bold text-on-surface-variant tabular-nums shrink-0">
                {format!("{} poin", s.points)}
            </span>
            <span class=badge>{s.level}</span>
        </div>
    }
}

#[component]
fn PemanggilanRow(p: PemanggilanItem) -> impl IntoView {
    let badge = match p.tier_kind.as_str() {
        "wali" => "ppm-chip-sm bg-error/10 text-error shrink-0",
        "pamong" => "ppm-chip-sm bg-warning/10 text-warning shrink-0",
        _ => "ppm-chip-sm bg-info/10 text-info shrink-0",
    };
    let meta = if p.nis.is_empty() {
        p.class_name.clone()
    } else {
        format!("{} • {}", p.nis, p.class_name)
    };
    view! {
        <div class="flex items-center gap-3 bg-surface-container-low rounded-xl px-3.5 py-2.5">
            <div class="flex-1 min-w-0">
                <p class="text-body-md font-semibold text-on-background truncate">{p.name}</p>
                <p class="text-[11px] text-on-surface-variant truncate">{meta}</p>
            </div>
            <span class="text-title-md font-bold text-error tabular-nums shrink-0">{p.net}</span>
            <span class=badge>{p.tier}</span>
        </div>
    }
}

#[component]
fn RewardRow(r: WeeklyRewardRow) -> impl IntoView {
    let meta = if r.nis.is_empty() { String::new() } else { r.nis.clone() };
    view! {
        <div class="flex items-center gap-3 bg-surface-container-low rounded-xl px-3.5 py-2.5">
            <div class="flex-1 min-w-0">
                <p class="text-body-md font-semibold text-on-background truncate">{r.name}</p>
                <p class="text-[11px] text-on-surface-variant truncate">{r.detail}</p>
                {(!meta.is_empty()).then(|| view! { <p class="text-[10px] text-on-surface-variant">{meta}</p> })}
            </div>
            <span class="text-title-md font-bold text-success tabular-nums shrink-0">{format!("+{}", r.points)}</span>
            {if r.credited {
                view! {
                    <span class="ppm-chip-sm bg-success/10 text-success shrink-0">"Dikredit"</span>
                }
                    .into_any()
            } else {
                view! {
                    <span class="ppm-chip-sm bg-warning/10 text-warning shrink-0">"Menunggu"</span>
                }
                    .into_any()
            }}
        </div>
    }
}

#[component]
fn Stat(label: &'static str, value: i64, cls: &'static str) -> impl IntoView {
    view! {
        <div class="flex-1 md:flex-none text-center">
            <span class=format!("text-body-md font-bold {cls}")>{value}</span>
            <span class="block md:hidden text-[10px] text-on-surface-variant">{label}</span>
        </div>
    }
}
