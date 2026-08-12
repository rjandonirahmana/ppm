//! web/pages/izin_staf.rs — Tinjau Izin (tahap 2, pamong/dewan guru/admin),
//! migrasi 17. Antrean izin yang SUDAH lolos konfirmasi orang tua
//! (parent_status='approved') menunggu keputusan pamong/dewan guru/admin.
//! Pola sama verifikasi_pamong.rs (Resource + Suspense + kartu Setujui/Tolak).

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::PermitReviewItem;
use crate::web::api::{decide_permit_action, permit_queue_data};
use crate::web::components::{
    kartu_grid, DeviceFrame, EmptyState, FetchError, FlashMsg, MobileHeader, SheetIzin,
};

#[component]
pub fn IzinStafPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { permit_queue_data().await });

    crate::web::components::guard_sesi(data);

    let busy_id = RwSignal::new(Option::<i64>::None);
    // Detail izin yang sedang dibuka — wali kelas bisa membaca (dan menyunting)
    // isinya sebelum memutuskan, bukan hanya melihat ringkasan di kartu.
    let detail = RwSignal::new(Option::<i64>::None);
    // Hasil keputusan terakhir. Dulu balasan server DIBUANG (`let _ = …`), dan
    // itu terlihat jelas di layar: kartu beranimasi keluar, daftar disegarkan,
    // kartunya kembali persis seperti semula — tanpa satu pun keterangan.
    // Pemutusnya tak punya cara membedakan "gagal" dari "tak terjadi apa-apa",
    // dan penolakan server yang sudah ditulis dengan kalimat Indonesia yang
    // jelas tak pernah sampai ke siapa pun.
    let msg = RwSignal::new(Option::<(bool, String)>::None);
    let decide = move |id: i64, approve: bool| {
        if busy_id.get_untracked().is_some() {
            return;
        }
        busy_id.set(Some(id));
        msg.set(None);
        leptos::task::spawn_local(async move {
            match decide_permit_action(id, approve).await {
                Ok(_) => msg.set(Some((
                    true,
                    if approve { "Izin disetujui." } else { "Izin ditolak." }.to_string(),
                ))),
                Err(e) => msg.set(Some((false, crate::web::components::pesan_galat(e)))),
            }
            busy_id.set(None);
            data.refetch();
        });
    };

    view! {
        <Title text="Tinjau Izin — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Tinjau Izin" subtitle="Antrean izin menunggu keputusan" back_href="/staf" />

                <div class="px-5 pt-5 space-y-5 stagger">
                    // Hasil keputusan, DI ATAS daftar — bukan di dalam kartu
                    // yang justru menghilang saat daftarnya disegarkan.
                    <FlashMsg pesan=msg />
                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="grid grid-cols-2 gap-3 md:max-w-lg">
                                    <div class="h-20 bg-surface-container rounded-2xl"></div>
                                    <div class="h-20 bg-surface-container rounded-2xl"></div>
                                </div>
                                <div class="grid gap-2 md:grid-cols-2">
                                    <div class="h-28 bg-surface-container rounded-2xl"></div>
                                    <div class="h-28 bg-surface-container rounded-2xl"></div>
                                </div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => {
                                        let stage = d.stage_label.clone();
                                        view! {
                                            <div class="ppm-chip bg-secondary-container text-primary inline-flex items-center gap-1">
                                                <span class="material-symbols-outlined text-[15px]">"how_to_reg"</span>
                                                {format!("{} · rute per-kelas (via pamong diatur di tiap kelas)", stage.clone())}
                                            </div>
                                            <div class="grid grid-cols-2 gap-3 md:max-w-lg">
                                                <div class="ppm-card p-4">
                                                    <div class="flex items-center gap-2 text-warning">
                                                        <span class="material-symbols-outlined pulse-dot">"pending_actions"</span>
                                                        <span
                                                            class="text-2xl font-bold text-on-background"
                                                            data-count=d.pending_count.to_string()
                                                        >
                                                            {d.pending_count}
                                                        </span>
                                                    </div>
                                                    <p class="text-body-sm text-on-surface-variant mt-1">
                                                        "Menunggu Tindakan"
                                                    </p>
                                                </div>
                                                <div class="ppm-card p-4">
                                                    <div class="flex items-center gap-2 text-success">
                                                        <span class="material-symbols-outlined">"done_all"</span>
                                                        <span
                                                            class="text-2xl font-bold text-on-background"
                                                            data-count=d.approved_today.to_string()
                                                        >
                                                            {d.approved_today}
                                                        </span>
                                                    </div>
                                                    <p class="text-body-sm text-on-surface-variant mt-1">
                                                        "Diputuskan Hari Ini"
                                                    </p>
                                                </div>
                                            </div>

                                            {if d.items.is_empty() {
                                                view! {
                                                    <EmptyState
                                                        icon="task_alt"
                                                        title="Tidak ada izin menunggu"
                                                        subtitle="Semua izin yang lolos konfirmasi orang tua sudah diputuskan."
                                                    />
                                                }
                                                    .into_any()
                                            } else {
                                                kartu_grid(
                                                        d.items
                                                            .into_iter()
                                                            .map(|p| {
                                                                view! { <PermitCard p=p busy_id=busy_id decide=decide buka=detail /> }
                                                                    .into_any()
                                                            })
                                                            .collect(),
                                                    )
                                                    .into_any()
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

            {move || {
                detail
                    .get()
                    .map(|pid| {
                        view! {
                            <SheetIzin
                                permit_id=pid
                                on_close=move || detail.set(None)
                                on_saved=move || data.refetch()
                            />
                        }
                    })
            }}
        </DeviceFrame>
    }
}

#[component]
fn PermitCard(
    p: PermitReviewItem,
    buka: RwSignal<Option<i64>>,
    busy_id: RwSignal<Option<i64>>,
    decide: impl Fn(i64, bool) + Copy + Send + 'static,
) -> impl IntoView {
    let id = p.id;
    // Segmen kosong TIDAK ikut tercetak. Santri yang NIS-nya belum terisi
    // muncul sebagai "NIS: - • kelas lambatan" — tanda hubung menggantung yang
    // terbaca seperti data rusak, padahal hanya belum diisi. Pola yang sama
    // sudah dipakai kartu sesi di kalender.
    let nis = p.nis.trim();
    let meta = match (nis, p.class_name.trim()) {
        ("" | "-", kelas) => kelas.to_string(),
        (n, "") => format!("NIS: {n}"),
        (n, kelas) => format!("NIS: {n} • {kelas}"),
    };
    let is_busy = move || busy_id.get() == Some(id);

    view! {
        <div class="ppm-card p-4 space-y-3 card-hover anim-in">
            <div class="flex items-start justify-between gap-2">
                <div class="min-w-0">
                    <button
                        class="text-body-md font-semibold text-on-background truncate text-left hover:underline"
                        on:click=move |_| buka.set(Some(id))
                    >
                        {p.student_name}
                    </button>
                    <p class="text-body-sm text-on-surface-variant truncate">{meta}</p>
                </div>
                <span class="ppm-chip bg-primary/10 text-primary shrink-0">{p.kind_label}</span>
            </div>

            // ── Rute persetujuan yang SEBENARNYA ───────────────────────────
            // Dulu tertulis "ORANG TUA → PENGURUS" dengan bar mati di 50%.
            // Orang tua berhenti jadi penyetuju sejak migrasi 46, dan
            // "PENGURUS" tak menyebut siapa pun secara khusus — jadi
            // indikatornya memberi kabar yang keliru sekaligus tak berguna.
            //
            // Sekarang: tahap yang benar-benar ada, dan bar-nya mengikuti
            // keadaan izin ini. Kelas satu langkah hanya menampilkan wali.
            {if p.dua_tahap {
                // KOSONG bila pamong belum meninjau, SETENGAH bila sudah.
                // Sebelumnya selalu setengah — bar yang tak pernah berubah
                // memberi kesan satu tahap sudah beres padahal belum ada yang
                // menyentuhnya. Tahap kedua (keputusan wali) baru mengisinya
                // penuh, dan itu terjadi setelah kartunya hilang dari antrean.
                let lebar = if p.pamong_ok {
                    "h-full bg-primary w-1/2 transition-all"
                } else {
                    "h-full bg-primary w-0 transition-all"
                };
                let cls_pamong = if p.pamong_ok {
                    "text-primary"
                } else {
                    "text-on-surface-variant"
                };
                view! {
                    <div class="w-full">
                        <div class="flex justify-between text-[10px] font-bold mb-1">
                            <span class=cls_pamong>
                                {if p.pamong_ok { "PAMONG KELAS ✓" } else { "PAMONG KELAS" }}
                            </span>
                            <span class="text-primary">"WALI KELAS"</span>
                        </div>
                        <div class="h-1.5 w-full bg-outline-variant rounded-full overflow-hidden">
                            <div class=lebar></div>
                        </div>
                        {(!p.pamong_ok)
                            .then(|| {
                                view! {
                                    <p class="text-[10px] text-on-surface-variant mt-1">
                                        "Pamong belum meninjau — Anda tetap boleh memutuskan."
                                    </p>
                                }
                            })}
                    </div>
                }
                    .into_any()
            } else {
                view! {
                    <div class="w-full">
                        <div class="flex justify-between text-[10px] font-bold text-primary mb-1">
                            <span>"WALI KELAS"</span>
                            <span>"KEPUTUSAN FINAL"</span>
                        </div>
                        <div class="h-1.5 w-full bg-outline-variant rounded-full overflow-hidden">
                            <div class="h-full bg-primary w-full"></div>
                        </div>
                    </div>
                }
                    .into_any()
            }}

            <p class="text-body-sm text-on-surface-variant flex items-center gap-1">
                <span class="material-symbols-outlined text-[15px]">"calendar_month"</span>
                {p.range_label}
                // Izin per jam (migrasi 66): tanpa jamnya, wali kelas mengira
                // santrinya absen sehari penuh padahal cuma dua jam.
                {(!p.jam_label.is_empty())
                    .then(|| view! { <span class="text-primary font-semibold">{p.jam_label.clone()}</span> })}
            </p>

            // Dampaknya, bukan cuma tanggalnya: kelas mana saja yang akan
            // kosong bila izin ini disetujui.
            {(!p.sesi_terlewat.is_empty())
                .then(|| {
                    let ringkas = format!("{} sesi terlewat", p.total_sesi);
                    view! {
                        <div class="rounded-xl bg-warning/5 border border-warning/30 px-3 py-2 space-y-1">
                            <p class="text-[11px] font-bold text-on-background flex items-center gap-1">
                                <span class="material-symbols-outlined text-[14px] text-warning">
                                    "event_busy"
                                </span>
                                {ringkas}
                            </p>
                            <p class="text-[11px] text-on-surface-variant">
                                {p.sesi_terlewat.join(" · ")}
                            </p>
                        </div>
                    }
                })}
            <p class="text-body-sm text-on-surface-variant italic">
                {format!("\u{201C}{}\u{201D}", p.reason)}
            </p>
            <p class="text-[10px] text-on-surface-variant">{p.when_label}</p>

            <div class="grid grid-cols-2 gap-3">
                <button
                    class="py-2.5 rounded-xl border border-error/40 text-error font-semibold text-body-sm hover:bg-error-container transition-colors disabled:opacity-50"
                    disabled=is_busy
                    on:click=move |_| decide(id, false)
                >
                    "Tolak"
                </button>
                <button
                    class="py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-sm hover:bg-primary-container transition-colors disabled:opacity-50"
                    disabled=is_busy
                    on:click=move |_| decide(id, true)
                >
                    "Setujui"
                </button>
            </div>
        </div>
    }
}
