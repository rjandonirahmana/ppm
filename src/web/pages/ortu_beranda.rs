//! web/pages/ortu_beranda.rs — Pantauan Orang Tua (mockup stitch).
//!
//! Dua kondisi:
//! • BELUM terhubung → cari santri (nama/NIS) → kirim permintaan → MENUNGGU
//!   PERSETUJUAN SANTRI (kartu "Permintaan Menunggu" + Mode Preview terkunci).
//! • Sudah terhubung → chip pemilih anak (BISA BANYAK) + panel pantauan:
//!   status hari ini, ring persentase, hadir/terlambat/absen, izin terakhir.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{BookProgressItem, ChildMonitor, ParentHome, StudentSearchItem};
use crate::web::api::{
    parent_home, request_connection_action, search_students_action,
    student_book_progress_for_viewer,
};
use crate::web::components::{BookProgressDetail, DeviceFrame, FetchError, MobileHeader, Sheet};

#[component]
pub fn OrtuBerandaPage() -> impl IntoView {
    // Anak terpilih (None = anak pertama dari server).
    let selected = RwSignal::new(Option::<i64>::None);
    let data = Resource::new(move || selected.get(), |c| async move { parent_home(c).await });

    Effect::new(move |_| {
        if let Some(Err(e)) = data.get() {
            let msg = e.to_string();
            if crate::web::components::is_auth_error(&msg) {
                #[cfg(target_arch = "wasm32")]
                if let Some(w) = web_sys::window() {
                    let _ = w.location().replace("/login");
                }
            }
        }
    });

    // Tampilkan panel pencarian (dipaksa terbuka lewat tombol "+").
    let show_search = RwSignal::new(false);

    view! {
        <Title text="Pantauan Orang Tua — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Pantauan Orang Tua" settings=true />

                <div class="px-5 pt-5 space-y-4 stagger">
                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                                <div class="grid gap-3 md:grid-cols-3 md:items-start">
                                    <div class="md:col-span-2 space-y-3">
                                        <div class="h-28 bg-surface-container rounded-2xl"></div>
                                        <div class="h-40 bg-surface-container rounded-2xl"></div>
                                    </div>
                                    <div class="h-40 bg-surface-container rounded-2xl"></div>
                                </div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => {
                                        view! {
                                            <HomeBody
                                                d=d
                                                selected=selected
                                                show_search=show_search
                                                refetch=move || data.refetch()
                                            />
                                        }
                                            .into_any()
                                    }
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any()
                                })
                        }}
                    </Suspense>
                </div>

            </div>
        </DeviceFrame>
    }
}

#[component]
fn HomeBody(
    d: ParentHome,
    selected: RwSignal<Option<i64>>,
    show_search: RwSignal<bool>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let has_children = !d.children.is_empty();
    let children = d.children.clone();
    let pending = d.pending.clone();
    // Id anak yang sedang dipantau (Copy → aman dipakai closure chip).
    let monitor_id = d.monitor.as_ref().map(|m| m.id);

    view! {
        // ── Chip pemilih anak (multi-anak) + tombol "+" tambah koneksi ──────
        {has_children
            .then(|| {
                let chips = children.clone();
                view! {
                    <div class="flex gap-2 overflow-x-auto pb-1">
                        {chips
                            .into_iter()
                            .map(|c| {
                                let id = c.id;
                                let cls = move || {
                                    if selected.get() == Some(id)
                                        || (selected.get().is_none() && Some(id) == monitor_id)
                                    {
                                        "px-4 py-2.5 rounded-full bg-secondary-container text-primary text-body-sm font-semibold whitespace-nowrap shrink-0 press"
                                    } else {
                                        "px-4 py-2.5 rounded-full bg-surface-container text-on-surface-variant text-body-sm font-medium whitespace-nowrap shrink-0 press"
                                    }
                                };
                                view! {
                                    <button class=cls on:click=move |_| selected.set(Some(id))>
                                        {c.name}
                                    </button>
                                }
                            })
                            .collect_view()}
                        <button
                            class="w-10 h-10 rounded-full bg-surface-container text-on-surface-variant flex items-center justify-center shrink-0 press"
                            on:click=move |_| show_search.update(|s| *s = !*s)
                            aria-label="Tambah koneksi santri"
                        >
                            <span class="material-symbols-outlined">"add"</span>
                        </button>
                    </div>
                }
            })}

        // Panel pencarian & kartu "belum terhubung" dibatasi lebar di desktop
        // (md:max-w-md) — sebelum ada anak terhubung tak ada grid lain di
        // halaman ini utk mengisi kanvas 72rem, jadi biarkan tetap selebar
        // kolom mobile drpd melebar penuh (lihat tailwind.css .ppm-wide).
        <div class="space-y-4 md:max-w-md">
        // ── Pencarian & kirim permintaan (selalu utk yg belum punya anak) ────
        // WAJIB `move ||`: tanpa ini panel dievaluasi SEKALI (tak reaktif) →
        // klik "+" mengubah show_search tapi DOM tak update (bug "tambah anak").
        {move || {
            (!has_children || show_search.get())
                .then(|| view! { <SearchPanel refetch=refetch /> })
        }}

        // ── Belum terhubung: kartu edukasi + Mode Preview ────────────────────
        {(!has_children)
            .then(|| {
                view! {
                    <div class="border-2 border-dashed border-outline-variant rounded-2xl p-8 text-center bg-surface-container-low/50">
                        <div class="w-16 h-16 mx-auto rounded-full bg-secondary-container flex items-center justify-center text-primary">
                            <span class="material-symbols-outlined text-3xl">"person_search"</span>
                        </div>
                        <h3 class="text-headline-sm text-on-background mt-4">"Belum Terhubung?"</h3>
                        <p class="text-body-sm text-on-surface-variant mt-2 leading-relaxed">
                            "Gunakan fitur pencarian di atas untuk menemukan data anak Anda dan kirimkan permintaan koneksi. "
                            "Koneksi aktif setelah DISETUJUI oleh santri."
                        </p>
                    </div>

                    // Mode Preview (terkunci)
                    <div class="relative rounded-2xl overflow-hidden">
                        <div class="space-y-3 p-4 opacity-40 blur-[2px] select-none pointer-events-none">
                            <div class="h-20 bg-surface-container rounded-2xl"></div>
                            <div class="h-28 bg-surface-container rounded-2xl"></div>
                        </div>
                        <div class="absolute inset-0 flex items-center justify-center p-6">
                            <div class="spiritual-gradient rounded-2xl p-6 text-on-primary text-center max-w-xs shadow-xl">
                                <span class="material-symbols-outlined text-3xl">"lock"</span>
                                <p class="text-body-lg font-bold mt-2">"Mode Preview"</p>
                                <p class="text-body-sm opacity-85 mt-1">
                                    "Data absensi & laporan dapat diakses setelah terhubung dengan santri."
                                </p>
                            </div>
                        </div>
                    </div>
                }
            })}
        </div>

        // ── Permintaan menunggu persetujuan santri (dibatasi md:max-w-md,
        // lihat catatan di panel pencarian di atas) ─────────────────────────
        {(!pending.is_empty())
            .then(|| {
                view! {
                    <div class="md:max-w-md bg-secondary-container/50 border border-secondary-container rounded-2xl overflow-hidden">
                        <div class="px-4 py-3 flex items-center gap-2 text-primary">
                            <span class="material-symbols-outlined text-lg pulse-dot">"schedule"</span>
                            <span class="text-[11px] font-bold tracking-[0.15em]">
                                "PERMINTAAN MENUNGGU"
                            </span>
                        </div>
                        <div class="bg-surface-container-lowest p-4 space-y-3">
                            {pending
                                .into_iter()
                                .map(|p| {
                                    let initial = p.student_name.chars().next().unwrap_or('S').to_string();
                                    view! {
                                        <div class="flex items-center gap-3">
                                            <div class="w-11 h-11 rounded-full bg-secondary-container flex items-center justify-center text-primary font-bold shrink-0">
                                                {initial}
                                            </div>
                                            <div class="flex-1 min-w-0">
                                                <p class="text-body-md font-semibold text-on-background">
                                                    {p.student_name}
                                                </p>
                                                <p class="text-body-sm text-on-surface-variant">{p.since_label}</p>
                                            </div>
                                            <span class="px-3 py-1.5 rounded-full text-[10px] font-bold tracking-wider bg-warning/10 text-warning">
                                                "PROSES"
                                            </span>
                                        </div>
                                    }
                                })
                                .collect_view()}
                            <p class="text-body-sm italic text-on-surface-variant text-center pt-1">
                                "Menunggu persetujuan santri. Anda akan bisa memantau setelah disetujui."
                            </p>
                        </div>
                    </div>
                }
            })}

        // ── Panel pantauan anak terpilih ─────────────────────────────────────
        {d.monitor.map(|m| view! { <MonitorPanel m=m /> })}

        // ── Butuh bantuan (md:max-w-md — kartu CTA tunggal, bukan grid) ──────
        <div class="md:max-w-md spiritual-gradient rounded-2xl p-5 text-on-primary">
            <p class="text-body-lg font-bold">"Butuh Bantuan?"</p>
            <p class="text-body-sm opacity-85 mt-1">
                "Jika Anda kesulitan menemukan NIS santri atau permintaan tidak kunjung dikonfirmasi, silakan hubungi bagian administrasi."
            </p>
            <button class="w-full mt-4 py-3 bg-white/10 border border-white/20 rounded-xl text-body-sm font-semibold flex items-center justify-center gap-2">
                <span class="material-symbols-outlined text-lg">"forum"</span>
                "Hubungi Admin"
            </button>
        </div>
    }
}


/// Panel cari santri + kirim permintaan koneksi.
#[component]
fn SearchPanel(refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let q = RwSignal::new(String::new());
    let results = RwSignal::new(Vec::<StudentSearchItem>::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None); // (sukses?, teks)

    let do_search = move || {
        let query = q.get_untracked();
        if query.trim().chars().count() < 2 {
            results.set(Vec::new());
            return;
        }
        leptos::task::spawn_local(async move {
            if let Ok(r) = search_students_action(query).await {
                results.set(r);
            }
        });
    };

    let send_request = move |student_id: i64| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        leptos::task::spawn_local(async move {
            match request_connection_action(student_id).await {
                Ok(_) => {
                    msg.set(Some((true, "Permintaan terkirim — menunggu persetujuan santri.".into())));
                    results.set(Vec::new());
                    q.set(String::new());
                    refetch();
                }
                Err(e) => {
                    let m = e.to_string();
                    let m = m.rsplit(": ").next().unwrap_or(&m).to_string();
                    msg.set(Some((false, m)));
                }
            }
            busy.set(false);
        });
    };

    view! {
        <div class="ppm-card p-4">
            <p class="text-[11px] font-bold tracking-[0.15em] text-on-surface-variant">
                "CARI SANTRI"
            </p>
            <div class="relative mt-2">
                <span class="material-symbols-outlined absolute left-3.5 top-1/2 -translate-y-1/2 text-outline">
                    "search"
                </span>
                <input
                    type="text"
                    class="w-full pl-11 pr-4 py-3.5 bg-surface-container border-0 rounded-xl text-body-md text-on-surface"
                    placeholder="Cari Nama atau NIS…"
                    prop:value=move || q.get()
                    on:input=move |ev| {
                        q.set(event_target_value(&ev));
                        do_search();
                    }
                />
            </div>
            <p class="text-body-sm italic text-on-surface-variant mt-2">
                "Contoh: \"Ahmad Zaki\" atau \"2024001\""
            </p>

            {move || {
                msg.get()
                    .map(|(ok, text)| {
                        let cls = if ok {
                            "mt-3 flex items-center gap-2 p-3 bg-secondary-container text-on-secondary-container rounded-xl text-body-sm anim-in"
                        } else {
                            "mt-3 flex items-center gap-2 p-3 bg-error-container text-on-error-container rounded-xl text-body-sm anim-in"
                        };
                        let icon = if ok { "task_alt" } else { "error" };
                        view! {
                            <div class=cls>
                                <span class="material-symbols-outlined text-xl">{icon}</span>
                                <span>{text}</span>
                            </div>
                        }
                    })
            }}

            {move || {
                let list = results.get();
                (!list.is_empty())
                    .then(|| {
                        view! {
                            <div class="mt-3 space-y-2">
                                {list
                                    .into_iter()
                                    .map(|s| {
                                        let id = s.id;
                                        let meta = format!("NIS: {} • {}", s.nis, s.class_name);
                                        let initial = s.name.chars().next().unwrap_or('S').to_string();
                                        view! {
                                            <div class="flex items-center gap-3 p-3 bg-surface-container rounded-xl anim-in">
                                                <div class="w-10 h-10 rounded-full bg-secondary-container flex items-center justify-center text-primary font-bold shrink-0">
                                                    {initial}
                                                </div>
                                                <div class="flex-1 min-w-0">
                                                    <p class="text-body-md font-semibold text-on-background truncate">
                                                        {s.name}
                                                    </p>
                                                    <p class="text-body-sm text-on-surface-variant truncate">{meta}</p>
                                                </div>
                                                <button
                                                    class="px-3.5 py-2 bg-primary text-on-primary rounded-xl text-body-sm font-semibold disabled:opacity-60"
                                                    disabled=move || busy.get()
                                                    on:click=move |_| send_request(id)
                                                >
                                                    "Hubungkan"
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

/// Panel pantauan anak terpilih (kartu anak + status + ring + izin +
/// tombol detail progres materi).
#[component]
fn MonitorPanel(m: ChildMonitor) -> impl IntoView {
    let initial = m.name.chars().next().unwrap_or('S').to_string();
    let meta = format!("{} • NIS: {}", m.class_name, m.nis);
    let dash = format!("{},100", m.pct.clamp(0, 100));

    // Sheet detail progres materi (orang tua melihat seperti tampilan santri).
    let show_detail = RwSignal::new(false);
    let student_id = m.id;
    let student_name = RwSignal::new(m.name.clone());
    let detail_data = Resource::new(
        move || show_detail.get(),
        move |show| async move {
            if show {
                student_book_progress_for_viewer(student_id).await.ok()
            } else {
                None
            }
        },
    );

    view! {
        // Desktop: profil+status+ring col-span-2 (kiri), izin jadi sidebar
        // kanan — konten sama, disusun 2 kolom di layar lebar.
        <div class="space-y-5 md:space-y-0 md:grid md:grid-cols-3 md:gap-5 md:items-start">
        <div class="md:col-span-2 space-y-5">
        // Kartu anak
        <div class="spiritual-gradient rounded-2xl p-5 text-on-primary flex items-center gap-4 shadow-lg shadow-primary/20">
            <div class="w-16 h-16 rounded-xl bg-primary-fixed text-primary flex items-center justify-center text-2xl font-bold shrink-0">
                {initial}
            </div>
            <div class="min-w-0">
                <p class="text-body-lg font-bold leading-tight">{m.name.clone()}</p>
                <p class="text-body-sm opacity-85 mt-0.5">{meta}</p>
                <span class="inline-flex items-center gap-1.5 bg-white/15 px-2.5 py-1 rounded-full text-[11px] font-semibold mt-2">
                    <span class="material-symbols-outlined text-[14px]">"verified_user"</span>
                    "Akademik Aktif"
                </span>
            </div>
        </div>

        // Status kehadiran hari ini
        <div class="ppm-card p-4">
            <div class="flex items-center justify-between">
                <h3 class="text-body-lg font-bold text-primary">"Status Kehadiran"</h3>
                <span class="text-[10px] font-bold tracking-[0.15em] text-on-surface-variant">
                    "HARI INI"
                </span>
            </div>
            {match m.today {
                Some(t) => {
                    view! {
                        <div class="flex items-center gap-3 mt-3">
                            <div class="w-13 h-13 p-3 rounded-full bg-secondary-container text-primary">
                                <span class="material-symbols-outlined">"how_to_reg"</span>
                            </div>
                            <div>
                                <p class="text-headline-sm text-on-background">{t.label}</p>
                                <p class="text-body-sm text-on-surface-variant flex items-center gap-1 mt-0.5">
                                    <span class="material-symbols-outlined text-[15px]">"schedule"</span>
                                    {t.time}
                                </p>
                                <p class="text-body-sm text-on-surface-variant flex items-center gap-1">
                                    <span class="material-symbols-outlined text-[15px]">"location_on"</span>
                                    {t.gate}
                                </p>
                            </div>
                        </div>
                    }
                        .into_any()
                }
                None => {
                    view! {
                        <p class="text-body-sm text-on-surface-variant mt-3">
                            "Belum ada catatan kehadiran hari ini."
                        </p>
                    }
                        .into_any()
                }
            }}
        </div>

        // Ring persentase + hitungan + tombol detail progres
        <div class="space-y-3">
            <div class="grid grid-cols-2 gap-3">
                <div class="ppm-card p-4 flex flex-col items-center">
                    <p class="text-[11px] font-bold tracking-[0.12em] text-on-surface-variant text-center">
                        "PERSENTASE KEHADIRAN"
                    </p>
                    <div class="relative w-28 h-28 mt-3">
                        <svg viewBox="0 0 36 36" class="w-full h-full -rotate-90">
                            <circle cx="18" cy="18" r="15.9" fill="none" stroke="#dce2f3" stroke-width="3.6"></circle>
                            <circle
                                cx="18" cy="18" r="15.9" fill="none" stroke="#064e3b"
                                stroke-width="3.6" stroke-linecap="round"
                                pathLength="100" stroke-dasharray=dash
                            ></circle>
                        </svg>
                        <div class="absolute inset-0 flex items-center justify-center">
                            <span class="text-2xl font-bold text-primary" data-count=m.pct.to_string()>
                                {m.pct}
                            </span>
                            <span class="text-body-sm font-bold text-primary">"%"</span>
                        </div>
                    </div>
                </div>
                <div class="ppm-card p-4 flex flex-col justify-center divide-y divide-outline-variant/40">
                    <div class="flex items-center justify-between py-2">
                        <span class="text-body-md text-on-surface-variant">"Hadir"</span>
                        <span class="text-body-lg font-bold text-success" data-count=m.hadir.to_string()>
                            {m.hadir}
                        </span>
                    </div>
                    <div class="flex items-center justify-between py-2">
                        <span class="text-body-md text-on-surface-variant">"Terlambat"</span>
                        <span class="text-body-lg font-bold text-warning" data-count=m.terlambat.to_string()>
                            {m.terlambat}
                        </span>
                    </div>
                    <div class="flex items-center justify-between py-2">
                        <span class="text-body-md text-on-surface-variant">"Absen"</span>
                        <span class="text-body-lg font-bold text-error" data-count=m.absen.to_string()>
                            {m.absen}
                        </span>
                    </div>
                </div>
            </div>

            <button
                class="w-full py-3 bg-secondary-container text-primary rounded-xl text-body-md font-semibold press flex items-center justify-center gap-2"
                on:click=move |_| show_detail.set(true)
            >
                <span class="material-symbols-outlined text-xl">"auto_stories"</span>
                "Lihat Detail Progres Materi"
            </button>
        </div>
        </div>

        // Pembayaran — pintunya di beranda, bukan di navbar: navbar orang tua
        // sudah lima item, dan label "Pembayaran" membuatnya membungkus dua
        // baris di layar 360px. Pola yang sama dipakai dashboard santri.
        <a
            href="/orang-tua/pembayaran"
            class="ppm-card p-4 flex items-center gap-3 press hover:border-primary transition-colors"
        >
            <span class="w-11 h-11 ppm-tile shrink-0">
                <span class="material-symbols-outlined">"payments"</span>
            </span>
            <div class="flex-1 min-w-0">
                <p class="text-body-md font-semibold text-on-background">"Pembayaran"</p>
                <p class="text-[11px] text-on-surface-variant">
                    "Kirim bukti transfer & lihat masa berlakunya"
                </p>
            </div>
            <span class="material-symbols-outlined text-on-surface-variant">"chevron_right"</span>
        </a>

        // Permohonan izin
        <div>
            <div class="flex items-center justify-between mb-3">
                <h3 class="text-headline-sm text-on-background">"Permohonan Izin"</h3>
                <a
                    href="/orang-tua/izin"
                    class="flex items-center gap-1.5 px-4 py-2 bg-primary text-on-primary rounded-xl text-body-sm font-semibold press"
                >
                    <span class="material-symbols-outlined text-lg">"add"</span>
                    "Izin Baru"
                </a>
            </div>
            {if m.permits.is_empty() {
                view! {
                    <div class="bg-surface-container rounded-2xl p-5 text-center text-body-sm text-on-surface-variant">
                        "Belum ada permohonan izin."
                    </div>
                }
                    .into_any()
            } else {
                m.permits
                    .into_iter()
                    .map(|p| {
                        let badge = match p.status_kind.as_str() {
                            "approved" => "px-3 py-1.5 rounded-full text-label-md bg-success/10 text-success",
                            "rejected" => "px-3 py-1.5 rounded-full text-label-md bg-error-container text-error",
                            _ => "px-3 py-1.5 rounded-full text-label-md bg-warning/10 text-warning",
                        };
                        view! {
                            <div class="ppm-card p-4 flex items-center gap-3 mb-2 card-hover">
                                <div class="w-11 h-11 rounded-xl bg-info/10 text-info flex items-center justify-center shrink-0">
                                    <span class="material-symbols-outlined">"medical_services"</span>
                                </div>
                                <div class="flex-1 min-w-0">
                                    <p class="text-body-md font-semibold text-on-background">{p.kind_label}</p>
                                    <p class="text-body-sm text-on-surface-variant">{p.range_label}</p>
                                </div>
                                <span class=badge>{p.status_label}</span>
                            </div>
                        }
                    })
                    .collect_view()
                    .into_any()
            }}
        </div>
        </div>

        // ── Bottom-sheet detail progres materi ─────────────────────────────────
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
                                                    student_name=student_name.get()
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
    }
}
