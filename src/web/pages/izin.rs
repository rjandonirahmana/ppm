//! web/pages/izin.rs — Ajukan Perizinan (mockup stitch): kehadiran semester,
//! hadir/absen, banner "Kehadiran Terdeteksi", formulir izin (Sakit/Pulang,
//! tanggal, alasan) → server fn `submit_permit_action` (tersimpan ke DB).

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::IzinData;
use crate::web::api::{izin_data, submit_permit_action};
use crate::web::components::{DeviceFrame, NotifBell};

#[component]
pub fn IzinPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { izin_data().await });

    Effect::new(move |_| {
        if let Some(Err(e)) = data.get() {
            let msg = e.to_string();
            if msg.contains("unauth") || msg.contains("forbidden") {
                #[cfg(target_arch = "wasm32")]
                if let Some(w) = web_sys::window() {
                    let _ = w.location().replace("/login");
                }
            }
        }
    });

    // ── State formulir ──────────────────────────────────────────────────────
    let kind = RwSignal::new("sick".to_string());
    let start = RwSignal::new(String::new());
    let end = RwSignal::new(String::new());
    let reason = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);
    let success = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        error.set(None);
        success.set(false);
        let k = kind.get_untracked();
        let s = start.get_untracked();
        let e = end.get_untracked();
        let r = reason.get_untracked();
        leptos::task::spawn_local(async move {
            match submit_permit_action(k, s, e, r).await {
                Ok(_) => {
                    success.set(true);
                    start.set(String::new());
                    end.set(String::new());
                    reason.set(String::new());
                    data.refetch();
                }
                Err(e) => {
                    let msg = e.to_string();
                    let msg = msg.rsplit(": ").next().unwrap_or(&msg).to_string();
                    error.set(Some(msg));
                }
            }
            busy.set(false);
        });
    };

    view! {
        <Title text="Ajukan Perizinan — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                // ── Header dgn poin ────────────────────────────────────────
                <header class="sticky top-0 z-20 bg-surface/90 backdrop-blur border-b border-outline-variant/50 px-5 py-3 flex items-center gap-3">
                    <div class="flex-1">
                        <h1 class="text-headline-sm text-primary font-bold">"Ajukan Perizinan"</h1>
                        <Suspense fallback=|| ()>
                            {move || {
                                data.get()
                                    .and_then(|r| r.ok())
                                    .map(|d| {
                                        view! {
                                            <p class="text-body-sm text-on-surface-variant flex items-center gap-1">
                                                <span class="material-symbols-outlined text-[15px]">"stars"</span>
                                                {d.points} " Poin"
                                            </p>
                                        }
                                    })
                            }}
                        </Suspense>
                    </div>
                    <NotifBell />
                </header>

                <div class="px-5 pt-5 space-y-4 md:space-y-0 md:grid md:grid-cols-3 md:gap-5 md:items-start stagger">
                    // Sidebar (statistik+riwayat) DULUAN di DOM → mobile tetap
                    // urutan asli (status dulu, baru form). Di desktop didorong
                    // ke kanan via order-2 (form default order-0 = kiri).
                    <div class="space-y-4 md:order-2">
                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="h-36 bg-surface-container rounded-2xl"></div>
                                <div class="h-28 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .and_then(|r| r.ok())
                                .map(|d| view! { <IzinStats d=d /> })
                        }}
                    </Suspense>

                    <Suspense fallback=|| ()>
                        {move || {
                            data.get()
                                .and_then(|r| r.ok())
                                .filter(|d| !d.permits.is_empty())
                                .map(|d| {
                                    view! {
                                        <div class="space-y-3">
                                            <h2 class="text-headline-sm text-on-background pt-2">
                                                "Pengajuan Terakhir"
                                            </h2>
                                            {d.permits
                                                .into_iter()
                                                .map(|p| {
                                                    let badge = match p.status_kind.as_str() {
                                                        "approved" => "px-3 py-1.5 rounded-full text-label-md bg-success/10 text-success",
                                                        "rejected" => "px-3 py-1.5 rounded-full text-label-md bg-error-container text-error",
                                                        _ => "px-3 py-1.5 rounded-full text-label-md bg-warning/10 text-warning",
                                                    };
                                                    // Migrasi 46: satu ajuan bisa pecah jadi beberapa baris
                                                    // (satu per wali kelas). Tampilkan kelasnya supaya santri
                                                    // paham kenapa ada lebih dari satu baris untuk tanggal sama.
                                                    let kelas = (!p.class_label.is_empty())
                                                        .then(|| {
                                                            view! {
                                                                <p class="text-[11px] text-on-surface-variant flex items-center gap-1 mt-0.5">
                                                                    <span class="material-symbols-outlined text-[13px]">"school"</span>
                                                                    {p.class_label.clone()}
                                                                </p>
                                                            }
                                                        });
                                                    view! {
                                                        <div class="ppm-card p-4 flex items-center gap-3">
                                                            <div class="w-11 h-11 rounded-xl bg-info/10 text-info flex items-center justify-center shrink-0">
                                                                <span class="material-symbols-outlined">"medical_services"</span>
                                                            </div>
                                                            <div class="flex-1 min-w-0">
                                                                <p class="text-body-md font-semibold text-on-background">
                                                                    {p.kind_label}
                                                                </p>
                                                                <p class="text-body-sm text-on-surface-variant">
                                                                    {p.range_label}
                                                                </p>
                                                                {kelas}
                                                            </div>
                                                            <span class=badge>{p.status_label}</span>
                                                        </div>
                                                    }
                                                })
                                                .collect_view()}
                                        </div>
                                    }
                                })
                        }}
                    </Suspense>
                    </div>

                    <div class="md:col-span-2 space-y-4 md:order-1 md:row-start-1">
                    // ── Formulir Izin ──────────────────────────────────────
                    <div class="flex items-center gap-2 pt-2 md:pt-0">
                        <span class="material-symbols-outlined text-on-background">"edit_note"</span>
                        <h2 class="text-headline-sm text-on-background">"Formulir Izin"</h2>
                    </div>

                    <form
                        class="ppm-card p-5 space-y-5"
                        method="post"
                        on:submit=on_submit
                    >
                        {move || {
                            error
                                .get()
                                .map(|e| {
                                    view! {
                                        <div class="flex items-center gap-2 p-3 bg-error-container text-on-error-container rounded-xl text-body-sm anim-in" role="alert">
                                            <span class="material-symbols-outlined text-xl">"error"</span>
                                            <span>{e}</span>
                                        </div>
                                    }
                                })
                        }}
                        {move || {
                            success
                                .get()
                                .then(|| {
                                    view! {
                                        <div class="flex items-center gap-2 p-3 bg-secondary-container text-on-secondary-container rounded-xl text-body-sm anim-in" role="status">
                                            <span class="material-symbols-outlined text-xl">"task_alt"</span>
                                            <span>"Izin terkirim — menunggu konfirmasi orang tua, lalu persetujuan pengurus."</span>
                                        </div>
                                    }
                                })
                        }}

                        // Jenis izin
                        <div class="space-y-2">
                            <label class="text-label-md text-on-surface-variant">"Jenis Izin"</label>
                            <div class="grid grid-cols-3 gap-3">
                                <KindButton kind=kind value="sick" icon="medical_services" label="Sakit" />
                                <KindButton kind=kind value="leave" icon="home" label="Pulang" />
                                <KindButton kind=kind value="keperluan" icon="event_note" label="Keperluan" />
                            </div>
                        </div>

                        // Tanggal
                        <div class="space-y-2">
                            <label class="text-label-md text-on-surface-variant">"Mulai Tanggal"</label>
                            <input
                                type="date"
                                class="w-full bg-surface-container border-0 rounded-xl px-4 py-3.5 text-body-md text-on-surface"
                                prop:value=move || start.get()
                                on:input=move |ev| start.set(event_target_value(&ev))
                                required=true
                            />
                        </div>
                        <div class="space-y-2">
                            <label class="text-label-md text-on-surface-variant">"Sampai Tanggal"</label>
                            <input
                                type="date"
                                class="w-full bg-surface-container border-0 rounded-xl px-4 py-3.5 text-body-md text-on-surface"
                                prop:value=move || end.get()
                                on:input=move |ev| end.set(event_target_value(&ev))
                            />
                        </div>

                        // Alasan
                        <div class="space-y-2">
                            <label class="text-label-md text-on-surface-variant">"Alasan Perizinan"</label>
                            <textarea
                                rows="4"
                                class="w-full bg-surface-container border-0 rounded-xl px-4 py-3.5 text-body-md text-on-surface resize-none"
                                placeholder="Tuliskan alasan lengkap untuk pertimbangan pengurus…"
                                prop:value=move || reason.get()
                                on:input=move |ev| reason.set(event_target_value(&ev))
                            ></textarea>
                        </div>

                        // Lampiran (belum aktif — jujur diberi label)
                        <div class="space-y-2">
                            <label class="text-label-md text-on-surface-variant">"Lampiran (Opsional)"</label>
                            <div class="border-2 border-dashed border-outline-variant rounded-2xl p-6 text-center text-on-surface-variant">
                                <span class="material-symbols-outlined text-3xl">"cloud_upload"</span>
                                <p class="text-body-sm mt-2">"Unggah surat dokter atau bukti lainnya"</p>
                                <p class="text-[11px] opacity-70 mt-1">"(fitur unggah segera hadir)"</p>
                            </div>
                        </div>

                        <button
                            type="submit"
                            class="w-full py-4 bg-primary text-on-primary font-bold rounded-2xl shadow-lg shadow-primary/20 hover:bg-primary-container active:scale-[0.98] transition-all flex items-center justify-center gap-2 disabled:opacity-60"
                            disabled=move || busy.get()
                        >
                            <span class="material-symbols-outlined">"send"</span>
                            {move || if busy.get() { "Mengirim…" } else { "Ajukan Izin Sekarang" }}
                        </button>
                    </form>
                    </div>
                </div>

            </div>
        </DeviceFrame>
    }
}

/// Statistik atas halaman izin.
#[component]
fn IzinStats(d: IzinData) -> impl IntoView {
    let width = format!("width:{}%", d.pct.clamp(0, 100));
    view! {
        // Kartu kehadiran semester
        <div class="spiritual-gradient rounded-2xl p-6 text-on-primary shadow-lg shadow-primary/20">
            <p class="text-label-md opacity-80 tracking-[0.15em]">"KEHADIRAN SEMESTER INI"</p>
            <p class="text-5xl font-bold mt-2">
                <span data-count=d.pct.to_string()>{d.pct}</span>
                "%"
            </p>
            <div class="w-full h-2.5 bg-white/20 rounded-full mt-5 overflow-hidden">
                <div class="h-full bg-primary-fixed rounded-full bar-grow" style=width></div>
            </div>
        </div>

        // Hadir / Absen
        <div class="grid grid-cols-2 gap-3">
            <div class="ppm-card p-5 text-center card-hover">
                <span class="material-symbols-outlined text-success text-3xl">"check_circle"</span>
                <p class="text-3xl font-bold text-on-background mt-1" data-count=d.hadir.to_string()>
                    {d.hadir}
                </p>
                <p class="text-body-sm text-on-surface-variant">"Hadir"</p>
            </div>
            <div class="ppm-card p-5 text-center card-hover">
                <span class="material-symbols-outlined text-error text-3xl">"warning"</span>
                <p class="text-3xl font-bold text-on-background mt-1" data-count=d.absen.to_string()>
                    {d.absen}
                </p>
                <p class="text-body-sm text-on-surface-variant">"Absen"</p>
            </div>
        </div>

        // Banner kehadiran terdeteksi hari ini
        {d.detected
            .map(|txt| {
                view! {
                    <div class="bg-secondary-container/60 border border-secondary-container rounded-2xl p-4 flex items-center gap-3">
                        <div class="w-12 h-12 rounded-full bg-secondary-container shrink-0 flex items-center justify-center text-primary">
                            <span class="material-symbols-outlined pulse-dot">"sensors"</span>
                        </div>
                        <div class="flex-1 min-w-0">
                            <p class="text-body-md font-semibold text-on-background">
                                "Kehadiran Terdeteksi"
                            </p>
                            <p class="text-body-sm text-on-surface-variant truncate">{txt}</p>
                        </div>
                        <span class="material-symbols-outlined text-on-surface-variant">"chevron_right"</span>
                    </div>
                }
            })}
    }
}

/// Tombol pilihan jenis izin (Sakit/Pulang).
#[component]
fn KindButton(
    kind: RwSignal<String>,
    value: &'static str,
    icon: &'static str,
    label: &'static str,
) -> impl IntoView {
    let cls = move || {
        if kind.get() == value {
            "flex items-center justify-center gap-2 py-4 rounded-2xl border-2 border-primary bg-secondary-container/40 text-primary font-semibold transition-all"
        } else {
            "flex items-center justify-center gap-2 py-4 rounded-2xl border border-outline-variant bg-surface-container-lowest text-on-surface font-semibold transition-all"
        }
    };
    view! {
        <button type="button" class=cls on:click=move |_| kind.set(value.to_string())>
            <span class="material-symbols-outlined">{icon}</span>
            {label}
        </button>
    }
}
