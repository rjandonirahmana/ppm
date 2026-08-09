//! web/pages/tagihan.rs — Pembayaran santri (migrasi 37).
//!   • FinancePage (/tagihan): admin/ketua/santri_finance — periode berjalan,
//!     tandai lunas + verifikasi; admin/ketua juga catat/hapus pembayaran.
//!   • MyBillsPage (/tagihan-saya): santri lihat pembayaran mereka + unggah bukti bayar.
//!
//! BAHASA HALAMAN INI. Kata "belum bayar" dan "tagihan" sengaja tidak dipakai:
//! yang dilihat pengurus sebagian besar adalah PERIODE yang catatannya belum
//! masuk — bukan tuduhan menunggak — dan halaman yang berbicara seperti debt
//! collector terbaca kasar di lingkungan pesantren, apalagi karena santri
//! sendiri melihat layar yang sama di /tagihan-saya. Karena itu:
//!   "Belum Bayar"   → "Periode Berjalan"
//!   "Jatuh tempo"   → "Periode terlewat"
//!   "Buat Tagihan"  → "Buat History Bayar" (bisa langsung tersimpan lunas)

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{fmt_rupiah, BillItem, SessionUser, StudentSearchItem, TunggakanItem};
use crate::web::api::{
    create_bill_action, delete_bill_action, finance_student_search, kirim_pengingat_bayar_action,
    mark_bill_paid_action, paid_bills_data, pending_bills_data, reject_bill_action,
    student_bills_data, tunggakan_data, unpaid_bills_data, verify_bill_action,
};
use crate::web::components::{DeviceFrame, EmptyState, FetchError, MobileHeader};

// ═══════════════════════════════════════════════════════════════════════════
// FINANCE — daftar belum bayar + kelola
// ═══════════════════════════════════════════════════════════════════════════
#[component]
pub fn FinancePage() -> impl IntoView {
    let session = use_context::<Resource<Option<SessionUser>>>();
    let can_manage = RwSignal::new(false); // admin/ketua (buat/hapus)
    Effect::new(move |_| {
        let m = session
            .and_then(|s| s.get())
            .flatten()
            .map(|u| matches!(u.role.as_str(), "admin" | "ketua"))
            .unwrap_or(false);
        can_manage.set(m);
    });

    let data = Resource::new(|| (), |_| async move { unpaid_bills_data().await });
    let paid = Resource::new(|| (), |_| async move { paid_bills_data().await });
    let menunggu = Resource::new(|| (), |_| async move { pending_bills_data().await });
    let tunggakan = Resource::new(|| (), |_| async move { tunggakan_data().await });
    // SEMUA daftar disegarkan bersama. Satu keputusan memindahkan baris antar
    // tab — menyetujui pengajuan mengeluarkannya dari antrean, memasukkannya ke
    // riwayat, DAN mengubah siapa yang periodenya terlewat — jadi menyegarkan
    // satu saja meninggalkan tab sebelah memajang data yang sudah tidak benar.
    let refetch = move || {
        data.refetch();
        paid.refetch();
        menunggu.refetch();
        tunggakan.refetch();
    };
    // Tab: menunggu | unpaid (periode berjalan) | terlewat | history.
    // Bawaannya "menunggu": itu satu-satunya tab yang berisi PEKERJAAN —
    // keluarga sedang menunggu kirimannya diperiksa.
    let tab = RwSignal::new("menunggu".to_string());

    view! {
        <Title text="Pembayaran Santri — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Pembayaran Santri" subtitle="Catat & verifikasi pembayaran" />
                <div class="px-5 pt-5 space-y-4 stagger">
                    // ── Tab bar ─────────────────────────────────────────────
                    // DUA BARIS di ponsel, satu baris di desktop.
                    //
                    // Sempat memakai `flex overflow-x-auto`, dan itu KELIRU:
                    // empat label berbahasa Indonesia berjumlah ±503px di dalam
                    // kolom selebar ±400px, jadi tab terakhir ("Riwayat")
                    // terdorong sepenuhnya ke luar batas. Di ponsel masih bisa
                    // digeser dengan jari — tapi di desktop tak ada cara
                    // menggulir bilah mendatar dengan tetikus, sehingga tabnya
                    // benar-benar tak bisa dijangkau siapa pun. Tab yang
                    // tersembunyi = fitur yang tidak ada.
                    //
                    // Grid membungkus, jadi berapa pun panjang labelnya semua
                    // tetap terlihat dan bisa ditekan. JANGAN kembalikan ke
                    // overflow-x tanpa memendekkan labelnya lebih dulu.
                    <div class="grid grid-cols-2 md:grid-cols-4 gap-1 bg-surface-container rounded-xl p-1">
                        <FinTab
                            tab=tab
                            value="menunggu"
                            label="Menunggu"
                            badge=Signal::derive(move || {
                                menunggu.get().and_then(|r| r.ok()).map(|v| v.len()).unwrap_or(0)
                            })
                        />
                        <FinTab tab=tab value="unpaid" label="Periode Berjalan" badge=Signal::derive(|| 0) />
                        <FinTab
                            tab=tab
                            value="terlewat"
                            label="Periode Terlewat"
                            badge=Signal::derive(move || {
                                tunggakan
                                    .get()
                                    .and_then(|r| r.ok())
                                    .map(|d| d.terlewat.len())
                                    .unwrap_or(0)
                            })
                        />
                        <FinTab tab=tab value="history" label="Riwayat" badge=Signal::derive(|| 0) />
                    </div>

                    // ── Menunggu verifikasi ─────────────────────────────────
                    <Show when=move || tab.get() == "menunggu" fallback=|| ()>
                        <Suspense fallback=|| {
                            view! { <div class="h-20 bg-surface-container rounded-2xl animate-pulse"></div> }
                        }>
                            {move || {
                                menunggu.get().map(|res| match res {
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                    Ok(list) if list.is_empty() => view! {
                                        <EmptyState icon="inbox" title="Tak ada yang menunggu"
                                            subtitle="Pengajuan pembayaran dari santri & orang tua muncul di sini." />
                                    }.into_any(),
                                    Ok(list) => view! {
                                        <div class="space-y-2">
                                            {list.into_iter()
                                                .map(|b| view! { <PengajuanRow b=b refetch=refetch /> })
                                                .collect_view()}
                                        </div>
                                    }.into_any(),
                                })
                            }}
                        </Suspense>
                    </Show>

                    // ── Periode terlewat ────────────────────────────────────
                    <Show when=move || tab.get() == "terlewat" fallback=|| ()>
                        <Suspense fallback=|| {
                            view! { <div class="h-20 bg-surface-container rounded-2xl animate-pulse"></div> }
                        }>
                            {move || {
                                tunggakan.get().map(|res| match res {
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                    Ok(d) => view! { <PanelTerlewat d=d refetch=refetch /> }.into_any(),
                                })
                            }}
                        </Suspense>
                    </Show>

                    // ── Periode Berjalan ────────────────────────────────────
                    <Show when=move || tab.get() == "unpaid" fallback=|| ()>
                        <Show when=move || can_manage.get() fallback=|| ()>
                            <CreateBillForm refetch=refetch />
                        </Show>
                        <Suspense fallback=|| {
                            view! { <div class="h-20 bg-surface-container rounded-2xl animate-pulse"></div> }
                        }>
                            {move || {
                                data.get().map(|res| match res {
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                    Ok(list) => {
                                        if list.is_empty() {
                                            view! {
                                                <EmptyState icon="task_alt" title="Semua tercatat 🎉"
                                                    subtitle="Tak ada periode yang menunggu pencatatan." />
                                            }.into_any()
                                        } else {
                                            let total: i64 = list.iter().map(|b| b.price).sum();
                                            view! {
                                                <div class="ppm-card p-4 flex items-center justify-between">
                                                    <span class="text-body-sm text-on-surface-variant">
                                                        {format!("{} periode belum tercatat", list.len())}
                                                    </span>
                                                    <span class="text-body-md font-bold text-error">{fmt_rupiah(total)}</span>
                                                </div>
                                                <div class="space-y-2">
                                                    {list.into_iter().map(|b| {
                                                        view! { <UnpaidRow b=b can_manage=can_manage refetch=refetch /> }
                                                    }).collect_view()}
                                                </div>
                                            }.into_any()
                                        }
                                    }
                                })
                            }}
                        </Suspense>
                    </Show>

                    // ── Riwayat Pembayaran (lunas) ──────────────────────────
                    <Show when=move || tab.get() == "history" fallback=|| ()>
                        <Suspense fallback=|| {
                            view! { <div class="h-20 bg-surface-container rounded-2xl animate-pulse"></div> }
                        }>
                            {move || {
                                paid.get().map(|res| match res {
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                    Ok(list) => {
                                        if list.is_empty() {
                                            view! {
                                                <EmptyState icon="history" title="Belum ada pembayaran"
                                                    subtitle="Riwayat pembayaran santri akan muncul di sini." />
                                            }.into_any()
                                        } else {
                                            let total: i64 =
                                                list.iter().map(|b| b.paid_amount.unwrap_or(b.price)).sum();
                                            view! {
                                                <div class="ppm-card p-4 flex items-center justify-between">
                                                    <span class="text-body-sm text-on-surface-variant">
                                                        {format!("{} pembayaran diterima", list.len())}
                                                    </span>
                                                    <span class="text-body-md font-bold text-success">{fmt_rupiah(total)}</span>
                                                </div>
                                                <div class="space-y-2">
                                                    {list.into_iter().map(|b| view! { <PaidRow b=b /> }).collect_view()}
                                                </div>
                                            }.into_any()
                                        }
                                    }
                                })
                            }}
                        </Suspense>
                    </Show>
                </div>
            </div>
        </DeviceFrame>
    }
}

#[component]
fn FinTab(
    tab: RwSignal<String>,
    value: &'static str,
    label: &'static str,
    /// Angka pada lencana; 0 = tak ada lencana.
    badge: Signal<usize>,
) -> impl IntoView {
    // `min-w-0` + `truncate` pada labelnya: di sel grid selebar setengah layar,
    // label yang tak mau menyusut mendorong lencananya keluar sel.
    let cls = move || {
        if tab.get() == value {
            "w-full min-w-0 px-2 py-2 rounded-lg bg-surface-container-lowest text-primary font-bold text-body-sm shadow-sm press flex items-center justify-center gap-1.5 cursor-pointer"
        } else {
            "w-full min-w-0 px-2 py-2 rounded-lg text-on-surface-variant font-semibold text-body-sm press flex items-center justify-center gap-1.5 cursor-pointer"
        }
    };
    view! {
        <button
            class=cls
            aria-pressed=move || (tab.get() == value).to_string()
            on:click=move |_| tab.set(value.to_string())
        >
            <span class="truncate">{label}</span>
            {move || {
                let n = badge.get();
                (n > 0)
                    .then(|| {
                        view! {
                            <span class="shrink-0 px-1.5 min-w-5 h-5 rounded-full bg-error text-on-error text-[10px] font-bold flex items-center justify-center">
                                {n}
                            </span>
                        }
                    })
            }}
        </button>
    }
}

/// Satu pengajuan yang menunggu diperiksa: bukti transfer + form penetapan
/// periode. Panelnya TERTUTUP sampai ditekan — verifikator memeriksa bukti
/// lebih dulu, dan empat isian yang terbuka semua di sepuluh baris sekaligus
/// membuat halaman ini mustahil dibaca.
#[component]
fn PengajuanRow(b: BillItem, refetch: impl Fn() + Copy + Send + Sync + 'static) -> impl IntoView {
    let id = b.id;
    let buka = RwSignal::new(false);
    let judul = RwSignal::new(String::new());
    let nominal = RwSignal::new(b.price.to_string());
    let mulai = RwSignal::new(String::new());
    let sampai = RwSignal::new(String::new());
    let metode = RwSignal::new("transfer".to_string());
    let alasan = RwSignal::new(String::new());
    let tolak_mode = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let err = RwSignal::new(String::new());

    let setujui = move |_| {
        if busy.get_untracked() {
            return;
        }
        let n: i64 = nominal
            .get_untracked()
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(0);
        busy.set(true);
        err.set(String::new());
        let (j, m, s, mt) = (
            judul.get_untracked(),
            mulai.get_untracked(),
            sampai.get_untracked(),
            metode.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            match verify_bill_action(id, j, m, s, n, mt).await {
                Ok(()) => refetch(),
                Err(e) => {
                    let t = e.to_string();
                    err.set(t.rsplit(": ").next().unwrap_or(&t).to_string());
                }
            }
            busy.set(false);
        });
    };

    let tolak = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        err.set(String::new());
        let a = alasan.get_untracked();
        leptos::task::spawn_local(async move {
            match reject_bill_action(id, a).await {
                Ok(()) => refetch(),
                Err(e) => {
                    let t = e.to_string();
                    err.set(t.rsplit(": ").next().unwrap_or(&t).to_string());
                }
            }
            busy.set(false);
        });
    };

    let field = "w-full bg-surface-container border-0 rounded-lg px-2.5 py-2 text-body-sm text-on-surface";
    let pengaju = if b.submitted_by_name.is_empty() {
        String::new()
    } else {
        format!("Diajukan {} • {}", b.submitted_by_name, b.submitted_at)
    };
    let ada_bukti = !b.proof_url.is_empty();
    let bukti = b.proof_url.clone();

    view! {
        <div class="ppm-card p-4 space-y-2">
            <div class="flex items-start justify-between gap-2">
                <div class="min-w-0">
                    <p class="text-body-md font-semibold text-on-background truncate">{b.student_name}</p>
                    <p class="text-[11px] text-on-surface-variant">{format!("{} • {}", b.nis, b.class_name)}</p>
                    {(!pengaju.is_empty()).then(|| view! {
                        <p class="text-[11px] text-on-surface-variant mt-0.5">{pengaju.clone()}</p>
                    })}
                </div>
                <div class="text-right shrink-0">
                    <p class="text-body-md font-bold text-on-background">{fmt_rupiah(b.price)}</p>
                    <span class="text-[10px] font-bold text-warning bg-warning/10 px-2 py-0.5 rounded-full">
                        "Menunggu"
                    </span>
                </div>
            </div>
            {(!b.note.is_empty()).then({ let n = b.note.clone(); move || view! {
                <p class="text-body-sm text-on-surface-variant">{n.clone()}</p>
            }})}

            // Bukti transfer paling atas: itu yang diperiksa lebih dulu, dan
            // tanpa bukti verifikator tak punya dasar apa pun untuk menyetujui.
            {if ada_bukti {
                view! {
                    <a href=bukti.clone() target="_blank"
                        class="inline-flex items-center gap-1 text-body-sm text-primary font-semibold">
                        <span class="material-symbols-outlined text-[16px]">"fact_check"</span>
                        "Lihat bukti transfer"
                    </a>
                }.into_any()
            } else {
                view! {
                    <p class="text-body-sm text-error">"Tanpa bukti transfer — sebaiknya ditolak."</p>
                }.into_any()
            }}

            <Show when=move || !buka.get() fallback=|| ()>
                <button class="w-full py-2 rounded-lg bg-primary text-on-primary text-body-sm font-bold press cursor-pointer"
                    on:click=move |_| buka.set(true)>
                    "Periksa & Tetapkan Periode"
                </button>
            </Show>

            <Show when=move || buka.get() fallback=|| ()>
                <div class="space-y-2 pt-1">
                    <Show when=move || !tolak_mode.get() fallback=|| ()>
                        <input class=field placeholder="Judul (mis. SPP Agustus 2026)"
                            prop:value=move || judul.get()
                            on:input=move |e| judul.set(event_target_value(&e)) />
                        <input class=field r#type="number" placeholder="Nominal diterima (rupiah)"
                            prop:value=move || nominal.get()
                            on:input=move |e| nominal.set(event_target_value(&e)) />
                        <div class="grid grid-cols-2 gap-2">
                            <div>
                                <label class="text-[11px] text-on-surface-variant ml-1">"Berlaku dari"</label>
                                <input class=field r#type="date" prop:value=move || mulai.get()
                                    on:input=move |e| mulai.set(event_target_value(&e)) />
                            </div>
                            <div>
                                <label class="text-[11px] text-on-surface-variant ml-1">"Sampai"</label>
                                <input class=field r#type="date" prop:value=move || sampai.get()
                                    on:input=move |e| sampai.set(event_target_value(&e)) />
                            </div>
                        </div>
                        <select class=field prop:value=move || metode.get()
                            on:change=move |e| metode.set(event_target_value(&e))>
                            <option value="transfer">"Transfer"</option>
                            <option value="tunai">"Tunai"</option>
                        </select>
                        <div class="flex gap-2">
                            <button class="flex-1 py-2 rounded-lg bg-primary text-on-primary text-body-sm font-bold press cursor-pointer disabled:opacity-60"
                                prop:disabled=move || busy.get() on:click=setujui>
                                {move || if busy.get() { "Menyimpan…" } else { "Setujui" }}
                            </button>
                            <button class="px-3 py-2 rounded-lg border border-error/40 text-error text-body-sm font-semibold cursor-pointer"
                                on:click=move |_| tolak_mode.set(true)>
                                "Tolak"
                            </button>
                        </div>
                    </Show>

                    <Show when=move || tolak_mode.get() fallback=|| ()>
                        <textarea class=field rows="2"
                            placeholder="Alasan penolakan — dibaca santri (mis. tidak ada mutasi masuk sejumlah itu)"
                            prop:value=move || alasan.get()
                            on:input=move |e| alasan.set(event_target_value(&e))></textarea>
                        <div class="flex gap-2">
                            <button class="flex-1 py-2 rounded-lg bg-error text-on-error text-body-sm font-bold press cursor-pointer disabled:opacity-60"
                                prop:disabled=move || busy.get() on:click=tolak>
                                {move || if busy.get() { "Menyimpan…" } else { "Kirim Penolakan" }}
                            </button>
                            <button class="px-3 py-2 rounded-lg border border-outline-variant text-body-sm font-semibold text-on-surface cursor-pointer"
                                on:click=move |_| tolak_mode.set(false)>
                                "Batal"
                            </button>
                        </div>
                    </Show>

                    <Show when=move || !err.get().is_empty() fallback=|| ()>
                        <div class="p-2 bg-error-container text-on-error-container rounded-lg text-[11px]">
                            {move || err.get()}
                        </div>
                    </Show>
                </div>
            </Show>
        </div>
    }
}

/// Baris riwayat pembayaran (bill lunas): santri, periode, nominal dibayar,
/// metode, waktu bayar, verifikator, + bukti transfer bila ada.
#[component]
fn PaidRow(b: BillItem) -> impl IntoView {
    let amount = b.paid_amount.unwrap_or(b.price);
    let method = if b.method.is_empty() { "-".to_string() } else { b.method.clone() };
    view! {
        <div class="ppm-card p-4 space-y-2">
            <div class="flex items-start justify-between gap-2">
                <div class="min-w-0">
                    <p class="text-body-md font-semibold text-on-background truncate">{b.student_name}</p>
                    <p class="text-[11px] text-on-surface-variant">{format!("{} • {}", b.nis, b.class_name)}</p>
                    <p class="text-body-sm text-on-surface-variant mt-0.5">{b.title.clone()}</p>
                </div>
                <div class="text-right shrink-0">
                    <p class="text-body-md font-bold text-success">{fmt_rupiah(amount)}</p>
                    <span class="text-[10px] font-bold text-success bg-success/10 px-2 py-0.5 rounded-full">"Lunas"</span>
                </div>
            </div>
            <div class="flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-on-surface-variant">
                <span class="inline-flex items-center gap-1">
                    <span class="material-symbols-outlined text-[14px]">"event"</span>
                    {format!("{} → {}", b.started_date, b.expired_date)}
                </span>
                {(!b.paid_at.is_empty())
                    .then({
                        let pa = b.paid_at.clone();
                        move || view! {
                            <span class="inline-flex items-center gap-1">
                                <span class="material-symbols-outlined text-[14px]">"schedule"</span>
                                {pa.clone()}
                            </span>
                        }
                    })}
                <span class="inline-flex items-center gap-1">
                    <span class="material-symbols-outlined text-[14px]">"payments"</span>
                    {method}
                </span>
            </div>
            {(!b.verified_by_name.is_empty())
                .then({
                    let v = b.verified_by_name.clone();
                    move || view! {
                        <p class="text-[11px] text-on-surface-variant">
                            {format!("Diverifikasi: {}", v)}
                        </p>
                    }
                })}
            {(!b.proof_url.is_empty())
                .then({
                    let url = b.proof_url.clone();
                    move || view! {
                        <a href=url.clone() target="_blank"
                            class="inline-flex items-center gap-1 text-body-sm text-primary font-semibold">
                            <span class="material-symbols-outlined text-[16px]">"fact_check"</span>
                            "Lihat bukti transfer"
                        </a>
                    }
                })}
        </div>
    }
}

#[component]
fn UnpaidRow(
    b: BillItem,
    can_manage: RwSignal<bool>,
    refetch: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let id = b.id;
    let method = RwSignal::new("transfer".to_string());
    let busy = RwSignal::new(false);

    let pay = move |_| {
        if busy.get_untracked() { return; }
        busy.set(true);
        let m = method.get_untracked();
        leptos::task::spawn_local(async move {
            let _ = mark_bill_paid_action(id, None, m).await;
            refetch();
            busy.set(false);
        });
    };
    let del = move |_| {
        leptos::task::spawn_local(async move {
            let _ = delete_bill_action(id).await;
            refetch();
        });
    };

    let field = "bg-surface-container border-0 rounded-lg px-2 py-1.5 text-body-sm text-on-surface";
    view! {
        <div class="ppm-card p-4 space-y-2">
            <div class="flex items-start justify-between gap-2">
                <div class="min-w-0">
                    <p class="text-body-md font-semibold text-on-background truncate">{b.student_name}</p>
                    <p class="text-[11px] text-on-surface-variant">{format!("{} • {}", b.nis, b.class_name)}</p>
                    <p class="text-body-sm text-on-surface-variant mt-0.5">{b.title.clone()}</p>
                </div>
                <div class="text-right shrink-0">
                    <p class="text-body-md font-bold text-on-background">{fmt_rupiah(b.price)}</p>
                    {b.overdue.then(|| view! {
                        <span class="text-[10px] font-bold text-error bg-error-container px-2 py-0.5 rounded-full">"Periode terlewat"</span>
                    })}
                </div>
            </div>
            <p class="text-[11px] text-on-surface-variant">{format!("Periode {} → {}", b.started_date, b.expired_date)}</p>
            {(!b.proof_url.is_empty()).then({ let url = b.proof_url.clone(); move || view! {
                <a href=url.clone() target="_blank" class="inline-flex items-center gap-1 text-body-sm text-primary font-semibold">
                    <span class="material-symbols-outlined text-[16px]">"fact_check"</span> "Lihat bukti bayar"
                </a>
            }})}
            <div class="flex items-center gap-2 pt-1">
                <select class=field prop:value=move || method.get()
                    on:change=move |e| method.set(event_target_value(&e))>
                    <option value="transfer">"Transfer"</option>
                    <option value="tunai">"Tunai"</option>
                </select>
                <button class="flex-1 py-2 bg-primary text-on-primary rounded-lg text-body-sm font-bold press disabled:opacity-60"
                    disabled=move || busy.get() on:click=pay>
                    "Tandai Lunas"
                </button>
                <Show when=move || can_manage.get() fallback=|| ()>
                    <button class="w-9 h-9 rounded-lg bg-error-container/60 text-error flex items-center justify-center shrink-0"
                        on:click=del aria-label="Hapus catatan pembayaran">
                        <span class="material-symbols-outlined text-[18px]">"delete"</span>
                    </button>
                </Show>
            </div>
        </div>
    }
}

/// Batas kirim sekali tekan — CERMIN dari `service::finance::MAX_PENGINGAT_SEKALI`.
/// Layar memakainya hanya untuk menjelaskan lebih awal; yang menegakkan tetap server.
const MAX_KIRIM: usize = 30;

/// Daftar santri yang masa berlaku pembayarannya habis, plus pengiriman
/// pengingat WhatsApp.
///
/// Dua kelompok terpisah atas permintaan pengurus: yang PERNAH membayar lalu
/// habis masa berlakunya adalah tagihan nyata, sedangkan yang belum pernah
/// tercatat sebagian besar adalah santri hasil impor daftar induk. Mencampur
/// keduanya membuat tujuh nama yang benar-benar perlu ditagih tenggelam di
/// antara lima ratus yang datanya memang belum pernah dimasukkan.
#[component]
fn PanelTerlewat(
    d: crate::models::TunggakanData,
    refetch: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let dipilih = RwSignal::new(Vec::<i64>::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);
    let buka_belum = RwSignal::new(false);

    let toggle = move |id: i64| {
        dipilih.update(|v| {
            if let Some(i) = v.iter().position(|&x| x == id) {
                v.remove(i);
            } else {
                v.push(id);
            }
        });
    };

    // Pengiriman SELALU lewat konfirmasi yang menyebut berapa orang akan
    // menerima. WhatsApp yang sudah terkirim tak bisa ditarik, dan "N terpilih"
    // di layar tak sama dengan "N nomor" — tiap santri bisa punya dua orang tua.
    let kirim = move |ids: Vec<i64>, keterangan: String| {
        if busy.get_untracked() || ids.is_empty() {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let ok = web_sys::window()
                .and_then(|w| w.confirm_with_message(&keterangan).ok())
                .unwrap_or(false);
            if !ok {
                return;
            }
        }
        let _ = &keterangan;
        busy.set(true);
        msg.set(None);
        leptos::task::spawn_local(async move {
            match kirim_pengingat_bayar_action(ids).await {
                Ok(t) => {
                    msg.set(Some((true, t)));
                    dipilih.set(Vec::new());
                    refetch();
                }
                Err(e) => {
                    let t = e.to_string();
                    msg.set(Some((false, t.rsplit(": ").next().unwrap_or(&t).to_string())));
                }
            }
            busy.set(false);
        });
    };

    let terlewat = StoredValue::new(d.terlewat.clone());
    let belum = StoredValue::new(d.belum_pernah.clone());
    let n_terlewat = d.terlewat.len();
    let n_belum = d.belum_pernah.len();

    view! {
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

        // ── Bilah aksi massal ────────────────────────────────────────────
        <Show when=move || !dipilih.get().is_empty() fallback=|| ()>
            <div class="ppm-card p-3 flex flex-wrap items-center gap-2 anim-in">
                <span class="text-body-sm font-semibold text-on-background flex-1 min-w-0">
                    {move || format!("{} dipilih", dipilih.get().len())}
                </span>
                <button
                    class="px-3 py-1.5 rounded-lg bg-primary text-on-primary text-body-sm font-semibold press cursor-pointer disabled:opacity-50"
                    prop:disabled=move || busy.get()
                    on:click=move |_| {
                        let ids = dipilih.get_untracked();
                        let n = ids.len();
                        // Jumlah NOMOR, bukan jumlah santri: satu santri bisa
                        // punya dua orang tua terhubung, dan konfirmasi yang
                        // menyebut angka terlalu kecil membuat pengurus mengira
                        // kirimannya lebih sempit dari kenyataannya.
                        let nomor: i64 = terlewat
                            .get_value()
                            .iter()
                            .chain(belum.get_value().iter())
                            .filter(|t| ids.contains(&t.user_id))
                            .map(|t| t.jumlah_ortu + i64::from(t.punya_hp))
                            .sum();
                        kirim(
                            ids,
                            format!(
                                "Kirim pengingat WhatsApp ke {n} santri ({nomor} nomor, termasuk orang tua)?\n\nPesan terkirim tidak bisa ditarik kembali.",
                            ),
                        );
                    }
                >
                    {move || format!("Kirim WA ke {} terpilih", dipilih.get().len())}
                </button>
                <button
                    class="px-3 py-1.5 rounded-lg text-on-surface-variant text-body-sm font-semibold cursor-pointer"
                    on:click=move |_| dipilih.set(Vec::new())
                >
                    "Batal"
                </button>
            </div>
        </Show>

        <p class="text-body-sm text-on-surface-variant">
            {format!(
                "{n_terlewat} santri masa berlakunya habis. Maksimal {MAX_KIRIM} penerima sekali kirim.",
            )}
        </p>

        {if n_terlewat == 0 {
            view! {
                <EmptyState
                    icon="verified"
                    title="Semua masih berlaku"
                    subtitle="Tak ada santri yang periode bayarnya terlewat."
                />
            }
                .into_any()
        } else {
            view! {
                <div class="space-y-2">
                    {terlewat
                        .get_value()
                        .into_iter()
                        .map(|t| {
                            let id = t.user_id;
                            view! {
                                <BarisTunggakan
                                    t=t
                                    busy=busy
                                    tercentang=Signal::derive(move || dipilih.get().contains(&id))
                                    on_centang=move || toggle(id)
                                    on_kirim=move || {
                                        kirim(
                                            vec![id],
                                            "Kirim pengingat WhatsApp ke santri ini dan orang tuanya?".to_string(),
                                        );
                                    }
                                />
                            }
                        })
                        .collect_view()}
                </div>
            }
                .into_any()
        }}

        // ── Belum pernah tercatat (dilipat) ──────────────────────────────
        {(n_belum > 0)
            .then(|| {
                view! {
                    <div class="ppm-card p-4">
                        <button
                            class="w-full flex items-center justify-between cursor-pointer"
                            on:click=move |_| buka_belum.update(|o| *o = !*o)
                        >
                            <span class="text-body-sm font-semibold text-on-background text-left">
                                {format!("Belum ada catatan pembayaran ({n_belum})")}
                            </span>
                            <span
                                class="material-symbols-outlined text-on-surface-variant transition-transform"
                                class:rotate-180=move || buka_belum.get()
                            >
                                "expand_more"
                            </span>
                        </button>
                        <p class="text-[11px] text-on-surface-variant mt-1">
                            "Sebagian besar berasal dari impor daftar induk — periksa dulu sebelum menagih."
                        </p>
                        <Show when=move || buka_belum.get() fallback=|| ()>
                            <div class="space-y-2 mt-3">
                                {belum
                                    .get_value()
                                    .into_iter()
                                    .map(|t| {
                                        let id = t.user_id;
                                        view! {
                                            <BarisTunggakan
                                                t=t
                                                busy=busy
                                                tercentang=Signal::derive(move || dipilih.get().contains(&id))
                                                on_centang=move || toggle(id)
                                                on_kirim=move || {
                                                    kirim(
                                                        vec![id],
                                                        "Kirim pengingat WhatsApp ke santri ini dan orang tuanya?"
                                                            .to_string(),
                                                    );
                                                }
                                            />
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        </Show>
                    </div>
                }
            })}
    }
}

/// Satu baris santri di daftar periode terlewat.
#[component]
fn BarisTunggakan(
    t: TunggakanItem,
    busy: RwSignal<bool>,
    tercentang: Signal<bool>,
    on_centang: impl Fn() + Copy + Send + 'static,
    on_kirim: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    // Tanpa satu pun nomor, tombol WA tak akan mengirim apa-apa. Menampilkannya
    // tetap aktif berarti pengurus menekannya, melihat "0 terkirim", lalu
    // menebak-nebak apa yang salah.
    let punya_tujuan = t.punya_hp || t.jumlah_ortu > 0;
    let keterangan = if t.belum_pernah {
        "Belum pernah ada catatan".to_string()
    } else {
        format!("Habis {} · {} hari lalu", t.habis_tanggal, t.hari_lewat)
    };
    let tujuan = if punya_tujuan {
        format!(
            "{} nomor tujuan",
            t.jumlah_ortu + i64::from(t.punya_hp)
        )
    } else {
        "Tak ada nomor HP".to_string()
    };

    view! {
        <div class="ppm-card p-3 flex gap-2.5 items-start">
            <input
                type="checkbox"
                class="w-5 h-5 accent-primary cursor-pointer shrink-0 mt-0.5"
                prop:checked=move || tercentang.get()
                on:change=move |_| on_centang()
                aria-label="Pilih untuk kirim WA massal"
            />
            <div class="flex-1 min-w-0">
                <p class="text-body-md font-semibold text-on-background truncate">{t.name}</p>
                <p class="text-[11px] text-on-surface-variant truncate">
                    {format!("{} • {}", t.nis, t.class_name)}
                </p>
                <p class=if t.belum_pernah {
                    "text-[11px] text-on-surface-variant mt-0.5"
                } else {
                    "text-[11px] text-error mt-0.5"
                }>{keterangan}</p>
                <p class=if punya_tujuan {
                    "text-[10px] text-on-surface-variant mt-0.5"
                } else {
                    "text-[10px] text-warning mt-0.5"
                }>
                    {tujuan}
                    {(!t.diingatkan.is_empty())
                        .then(|| format!(" · diingatkan {}", t.diingatkan))}
                </p>
            </div>
            <button
                class="px-2.5 py-1.5 rounded-lg border border-outline-variant text-[11px] font-semibold text-on-surface hover:border-primary hover:text-primary transition-colors cursor-pointer shrink-0 disabled:opacity-40"
                prop:disabled=move || busy.get() || !punya_tujuan
                on:click=move |_| on_kirim()
            >
                "Kirim WA"
            </button>
        </div>
    }
}

#[component]
fn CreateBillForm(refetch: impl Fn() + Copy + Send + Sync + 'static) -> impl IntoView {
    let q = RwSignal::new(String::new());
    let results = RwSignal::new(Vec::<StudentSearchItem>::new());
    let picked = RwSignal::new(Option::<(i64, String)>::None);
    let title = RwSignal::new(String::new());
    let price = RwSignal::new(String::new());
    let started = RwSignal::new(String::new());
    let expired = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);
    let open = RwSignal::new(false);
    // Uang SUDAH diterima? Bawaannya ya: itu alasan halaman ini dibuka —
    // pengurus mencatat setoran yang sudah masuk, bukan menerbitkan tagihan.
    let sudah = RwSignal::new(true);
    let metode = RwSignal::new("transfer".to_string());
    // Kosong = hari ini (diisi server). Tak dipranilai di klien supaya jam
    // browser yang meleset tak diam-diam menetapkan tanggal setoran.
    let tgl_bayar = RwSignal::new(String::new());

    let do_search = move || {
        let query = q.get_untracked();
        leptos::task::spawn_local(async move {
            if let Ok(r) = finance_student_search(query).await { results.set(r); }
        });
    };

    let submit = move |_| {
        if busy.get_untracked() { return; }
        let Some((uid, _)) = picked.get_untracked() else {
            msg.set(Some((false, "Pilih santri dulu.".into()))); return;
        };
        let (t, p, sd, ed) = (title.get_untracked(), price.get_untracked(),
            started.get_untracked(), expired.get_untracked());
        let Ok(pr) = p.trim().parse::<i64>() else {
            msg.set(Some((false, "Nominal harus angka (rupiah).".into()))); return;
        };
        let (lunas, m, tb) =
            (sudah.get_untracked(), metode.get_untracked(), tgl_bayar.get_untracked());
        busy.set(true); msg.set(None);
        leptos::task::spawn_local(async move {
            match create_bill_action(uid, t, pr, sd, ed, String::new(), lunas, m, tb).await {
                Ok(_) => {
                    // Pesan menyebutkan KE MANA catatannya pergi: tersimpan
                    // lunas berarti ia tak muncul di daftar yang sedang dilihat
                    // pengurus, dan "tersimpan" tanpa keterangan terbaca gagal.
                    msg.set(Some((true, if lunas {
                        "History bayar tersimpan — lihat di tab Riwayat Pembayaran.".to_string()
                    } else {
                        "Periode tersimpan di daftar Periode Berjalan.".to_string()
                    })));
                    title.set(String::new()); price.set(String::new());
                    tgl_bayar.set(String::new());
                    picked.set(None); q.set(String::new()); results.set(Vec::new());
                    refetch();
                }
                Err(e) => { let m = e.to_string(); msg.set(Some((false, m.rsplit(": ").next().unwrap_or(&m).to_string()))); }
            }
            busy.set(false);
        });
    };

    let field = "w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface";
    view! {
        <div class="ppm-card p-4 space-y-3">
            <button class="w-full flex items-center justify-between" on:click=move |_| open.update(|o| *o = !*o)>
                <span class="text-body-md font-bold text-on-background flex items-center gap-2">
                    <span class="material-symbols-outlined text-primary">"add_circle"</span> "Buat History Bayar"
                </span>
                <span class="material-symbols-outlined text-on-surface-variant transition-transform"
                    class:rotate-180=move || open.get()>"expand_more"</span>
            </button>
            {move || open.get().then(|| view! {
                <div class="space-y-2.5 pt-1">
                    {move || msg.get().map(|(ok, t)| {
                        let cls = if ok { "p-2.5 bg-secondary-container text-on-secondary-container rounded-lg text-body-sm" }
                                  else { "p-2.5 bg-error-container text-on-error-container rounded-lg text-body-sm" };
                        view! { <div class=cls>{t}</div> }
                    })}
                    {move || match picked.get() {
                        Some((_, name)) => view! {
                            <div class="flex items-center justify-between bg-secondary-container/50 rounded-xl px-3 py-2">
                                <span class="text-body-sm font-semibold text-primary">{name}</span>
                                <button class="text-body-sm text-error" on:click=move |_| picked.set(None)>"Ganti"</button>
                            </div>
                        }.into_any(),
                        None => view! {
                            <div class="space-y-1.5">
                                <div class="flex gap-2">
                                    <input class=field placeholder="Cari nama/NIS santri"
                                        prop:value=move || q.get()
                                        on:input=move |e| q.set(event_target_value(&e)) />
                                    <button class="px-3 bg-surface-container rounded-xl text-primary" on:click=move |_| do_search()>
                                        <span class="material-symbols-outlined">"search"</span>
                                    </button>
                                </div>
                                {move || {
                                    let r = results.get();
                                    (!r.is_empty()).then(|| view! {
                                        <div class="max-h-40 overflow-y-auto space-y-1">
                                            {r.into_iter().map(|s| {
                                                let (id, name) = (s.id, s.name.clone());
                                                view! {
                                                    <button class="w-full text-left px-3 py-2 bg-surface-container rounded-lg hover:bg-secondary-container/50"
                                                        on:click=move |_| picked.set(Some((id, name.clone())))>
                                                        <span class="text-body-sm text-on-surface">{s.name}" "</span>
                                                        <span class="text-[11px] text-on-surface-variant">{s.nis}</span>
                                                    </button>
                                                }
                                            }).collect_view()}
                                        </div>
                                    })
                                }}
                            </div>
                        }.into_any(),
                    }}
                    <input class=field placeholder="Judul (mis. SPP Juli 2026)"
                        prop:value=move || title.get() on:input=move |e| title.set(event_target_value(&e)) />
                    <input class=field r#type="number" placeholder="Nominal (rupiah)"
                        prop:value=move || price.get() on:input=move |e| price.set(event_target_value(&e)) />
                    <div class="grid grid-cols-2 gap-2">
                        <div>
                            <label class="text-[11px] text-on-surface-variant ml-1">"Periode mulai"</label>
                            <input class=field r#type="date" prop:value=move || started.get()
                                on:input=move |e| started.set(event_target_value(&e)) />
                        </div>
                        <div>
                            <label class="text-[11px] text-on-surface-variant ml-1">"Periode sampai"</label>
                            <input class=field r#type="date" prop:value=move || expired.get()
                                on:input=move |e| expired.set(event_target_value(&e)) />
                        </div>
                    </div>

                    // ── Sudah dibayar? ──────────────────────────────────────
                    // Satu sakelar, bukan dua tombol berbeda: yang membedakan
                    // "catat setoran" dan "buka periode baru" cuma satu fakta —
                    // uangnya sudah diterima atau belum.
                    <label class="flex items-center gap-2.5 py-1 cursor-pointer">
                        <input type="checkbox" class="w-5 h-5 accent-primary shrink-0"
                            prop:checked=move || sudah.get()
                            on:change=move |e| sudah.set(event_target_checked(&e)) />
                        <span class="text-body-sm font-semibold text-on-background">
                            "Sudah dibayar"
                        </span>
                    </label>
                    <Show when=move || sudah.get() fallback=|| view! {
                        <p class="text-[11px] text-on-surface-variant">
                            "Tersimpan sebagai periode berjalan — tandai lunas nanti setelah setoran diterima."
                        </p>
                    }>
                        <div class="grid grid-cols-2 gap-2">
                            <div>
                                <label class="text-[11px] text-on-surface-variant ml-1">"Metode"</label>
                                <select class=field prop:value=move || metode.get()
                                    on:change=move |e| metode.set(event_target_value(&e))>
                                    <option value="transfer">"Transfer"</option>
                                    <option value="tunai">"Tunai"</option>
                                </select>
                            </div>
                            <div>
                                <label class="text-[11px] text-on-surface-variant ml-1">"Tanggal bayar"</label>
                                <input class=field r#type="date" prop:value=move || tgl_bayar.get()
                                    on:input=move |e| tgl_bayar.set(event_target_value(&e)) />
                            </div>
                        </div>
                        <p class="text-[11px] text-on-surface-variant">
                            "Kosongkan tanggal bila setorannya diterima hari ini."
                        </p>
                    </Show>

                    <button class="w-full py-2.5 bg-primary text-on-primary rounded-xl font-bold text-body-sm press disabled:opacity-60"
                        disabled=move || busy.get() on:click=submit>
                        {move || if busy.get() {
                            "Menyimpan…"
                        } else if sudah.get() {
                            "Simpan History Bayar"
                        } else {
                            "Simpan Periode"
                        }}
                    </button>
                </div>
            })}
        </div>
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SANTRI — pembayaran saya + AJUKAN pembayaran
// ═══════════════════════════════════════════════════════════════════════════
#[component]
pub fn MyBillsPage() -> impl IntoView {
    // `student_bills_data(0)` = milik sendiri. Beda dari `my_bills_data` lama:
    // yang ini ikut membawa pengajuan berstatus menunggu & ditolak — dan justru
    // itu yang paling ingin dilihat keluarga setelah mengirim bukti transfer.
    let data = Resource::new(|| (), |_| async move { student_bills_data(0).await });
    let refetch = move || data.refetch();

    view! {
        <Title text="Pembayaran Saya — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Pembayaran Saya" subtitle="Ajukan & pantau pembayaran" back_href="/santri" />
                <div class="px-5 pt-5 space-y-3 stagger">
                    <FormAjukanBayar student_id=0 refetch=refetch />
                    <Suspense fallback=|| {
                        view! { <div class="h-20 bg-surface-container rounded-2xl animate-pulse"></div> }
                    }>
                        {move || {
                            data.get().map(|res| match res {
                                Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                Ok(list) => {
                                    if list.is_empty() {
                                        view! { <EmptyState icon="fact_check" title="Belum ada pembayaran"
                                            subtitle="Kirim bukti transfer di atas — statusnya muncul di sini." /> }.into_any()
                                    } else {
                                        view! {
                                            <div class="space-y-2">
                                                {list.into_iter().map(|b| view! { <MyBillRow b=b refetch=refetch /> }).collect_view()}
                                            </div>
                                        }.into_any()
                                    }
                                }
                            })
                        }}
                    </Suspense>
                </div>
            </div>
        </DeviceFrame>
    }
}

/// Form pengajuan pembayaran — dipakai layar SANTRI dan layar ORANG TUA.
///
/// `student_id = 0` berarti "diri sendiri" (santri); orang tua meneruskan id
/// anak yang sedang dipilih. Satu komponen untuk keduanya karena isinya memang
/// sama persis: nominal yang disetor + foto bukti. Menyalinnya ke dua halaman
/// akan membuat keduanya menyimpang begitu ada satu isian ditambahkan.
///
/// Nominal DAN berkas dikirim dalam SATU multipart ke `/api/bills/request` —
/// bukan "buat baris dulu, unggah bukti kemudian". Yang kedua meninggalkan
/// pengajuan tanpa bukti setiap kali unggahannya putus di tengah, dan baris
/// seperti itu tak bisa diapa-apakan verifikator sementara keluarga mengira
/// urusannya sudah selesai.
/// Daftar pembayaran satu santri, HANYA BACA — dipakai layar orang tua.
///
/// Berbeda dari [`MyBillRow`] yang masih menawarkan unggah bukti untuk tagihan
/// lama: jalur unggah itu dijaga `user_id = pengunggah` di query, jadi bagi
/// orang tua tombolnya pasti gagal. Menampilkan tombol yang pasti gagal lebih
/// buruk daripada tak menampilkannya.
#[component]
pub fn RiwayatBayarList(list: Vec<BillItem>) -> impl IntoView {
    if list.is_empty() {
        return view! {
            <EmptyState
                icon="fact_check"
                title="Belum ada pembayaran"
                subtitle="Kirim bukti transfer di atas — statusnya muncul di sini."
            />
        }
        .into_any();
    }
    view! {
        <div class="space-y-2">
            {list
                .into_iter()
                .map(|b| {
                    let badge = match b.status.as_str() {
                        "lunas" => ("Lunas", "text-primary bg-secondary-container"),
                        "menunggu" => ("Menunggu diperiksa", "text-warning bg-warning/10"),
                        "ditolak" => ("Ditolak", "text-error bg-error-container"),
                        _ if b.overdue => ("Periode terlewat", "text-error bg-error-container"),
                        _ => {
                            ("Periode berjalan", "text-on-surface-variant bg-surface-container-high")
                        }
                    };
                    let periode = if b.started_date.is_empty() || b.expired_date.is_empty() {
                        if b.submitted_at.is_empty() {
                            String::new()
                        } else {
                            format!("Dikirim {}", b.submitted_at)
                        }
                    } else {
                        format!("Berlaku {} → {}", b.started_date, b.expired_date)
                    };
                    let alasan = b.reject_reason.clone();
                    let bukti = b.proof_url.clone();
                    view! {
                        <div class="ppm-card p-4 space-y-2">
                            <div class="flex items-start justify-between gap-2">
                                <div class="min-w-0">
                                    <p class="text-body-md font-semibold text-on-background">
                                        {b.title.clone()}
                                    </p>
                                    {(!periode.is_empty())
                                        .then(|| {
                                            view! {
                                                <p class="text-[11px] text-on-surface-variant">
                                                    {periode.clone()}
                                                </p>
                                            }
                                        })}
                                </div>
                                <div class="text-right shrink-0">
                                    <p class="text-body-md font-bold text-on-background">
                                        {fmt_rupiah(b.paid_amount.unwrap_or(b.price))}
                                    </p>
                                    <span class=format!(
                                        "text-[10px] font-bold px-2 py-0.5 rounded-full {}",
                                        badge.1,
                                    )>{badge.0}</span>
                                </div>
                            </div>
                            {(!alasan.is_empty())
                                .then(|| {
                                    view! {
                                        <p class="text-body-sm text-error bg-error-container/40 rounded-lg px-2.5 py-2">
                                            {alasan.clone()}
                                        </p>
                                    }
                                })}
                            {(!bukti.is_empty())
                                .then(|| {
                                    view! {
                                        <a
                                            href=bukti.clone()
                                            target="_blank"
                                            class="inline-flex items-center gap-1 text-body-sm text-on-surface-variant"
                                        >
                                            <span class="material-symbols-outlined text-[16px]">
                                                "fact_check"
                                            </span>
                                            "Bukti terunggah"
                                        </a>
                                    }
                                })}
                        </div>
                    }
                })
                .collect_view()}
        </div>
    }
    .into_any()
}

#[component]
// Pengirimannya seluruhnya hidup di blok `cfg(target_arch = "wasm32")` (fetch
// multipart dari browser), jadi di build SSR `refetch` memang menganggur — itu
// benar, bukan sisa kode mati.
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
pub fn FormAjukanBayar(
    /// 0 = santri yang sedang login. Selain itu: id anak (dipakai orang tua).
    #[prop(into)]
    student_id: Signal<i64>,
    refetch: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let buka = RwSignal::new(false);
    let nominal = RwSignal::new(String::new());
    let catatan = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);
    let nama_berkas = RwSignal::new(String::new());
    let file_input: NodeRef<leptos::html::Input> = NodeRef::new();

    let on_pick = move |_ev: leptos::ev::Event| {
        #[cfg(target_arch = "wasm32")]
        {
            let nama = file_input
                .get_untracked()
                .and_then(|i| i.files())
                .and_then(|f| f.get(0))
                .map(|f| f.name())
                .unwrap_or_default();
            nama_berkas.set(nama);
        }
        let _ = (&file_input, &nama_berkas);
    };

    let kirim = move |_| {
        if busy.get_untracked() {
            return;
        }
        // Divalidasi di klien SEBELUM mengunggah foto: menyuruh orang menunggu
        // unggahan 3 MB selesai hanya untuk dibalas "nominal kosong" adalah
        // pemborosan kuota yang bisa dicegah satu baris.
        let angka: String =
            nominal.get_untracked().chars().filter(|c| c.is_ascii_digit()).collect();
        if angka.is_empty() || angka.parse::<i64>().unwrap_or(0) <= 0 {
            msg.set(Some((false, "Isi dulu jumlah yang ditransfer.".into())));
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            let file = file_input.get_untracked().and_then(|i| i.files()).and_then(|f| f.get(0));
            let Some(file) = file else {
                msg.set(Some((false, "Pilih dulu foto bukti transfernya.".into())));
                return;
            };
            busy.set(true);
            msg.set(None);
            let sid = student_id.get_untracked();
            let cat = catatan.get_untracked();
            leptos::task::spawn_local(async move {
                let form = web_sys::FormData::new().unwrap();
                let _ = form.append_with_str("amount", &angka);
                let _ = form.append_with_str("student_id", &sid.to_string());
                let _ = form.append_with_str("note", &cat);
                let _ = form.append_with_blob("file", &file);

                let win = web_sys::window().unwrap();
                let opts = web_sys::RequestInit::new();
                opts.set_method("POST");
                opts.set_body(form.as_ref());
                let mut ok = false;
                let mut gagal = String::new();
                if let Ok(req) =
                    web_sys::Request::new_with_str_and_init("/api/bills/request", &opts)
                {
                    match wasm_bindgen_futures::JsFuture::from(win.fetch_with_request(&req)).await {
                        Ok(resp) => {
                            let resp: web_sys::Response = resp.dyn_into().unwrap();
                            if resp.ok() {
                                ok = true;
                            } else {
                                gagal = wasm_bindgen_futures::JsFuture::from(resp.text().unwrap())
                                    .await
                                    .ok()
                                    .and_then(|t| t.as_string())
                                    .unwrap_or_default();
                            }
                        }
                        Err(_) => {
                            gagal = "Koneksi terputus saat mengirim. Coba lagi setelah \
                                     sinyalnya stabil."
                                .to_string();
                        }
                    }
                }
                busy.set(false);
                if ok {
                    msg.set(Some((
                        true,
                        "Terkirim. Pengurus akan memeriksa bukti transfermu dan menetapkan \
                         masa berlakunya."
                            .into(),
                    )));
                    nominal.set(String::new());
                    catatan.set(String::new());
                    nama_berkas.set(String::new());
                    if let Some(inp) = file_input.get_untracked() {
                        inp.set_value("");
                    }
                    buka.set(false);
                    refetch();
                } else {
                    msg.set(Some((
                        false,
                        if gagal.trim().is_empty() {
                            "Gagal mengirim — periksa koneksi, lalu coba lagi.".to_string()
                        } else {
                            gagal
                        },
                    )));
                }
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (&busy, &msg, &student_id, &catatan);
        }
    };

    let field =
        "w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-md text-on-surface";
    view! {
        <div class="ppm-card p-4 space-y-3">
            <button
                class="w-full flex items-center justify-between cursor-pointer"
                on:click=move |_| buka.update(|o| *o = !*o)
            >
                <span class="text-body-md font-bold text-on-background flex items-center gap-2">
                    <span class="material-symbols-outlined text-primary">"upload_file"</span>
                    "Kirim Bukti Pembayaran"
                </span>
                <span
                    class="material-symbols-outlined text-on-surface-variant transition-transform"
                    class:rotate-180=move || buka.get()
                >
                    "expand_more"
                </span>
            </button>

            {move || {
                msg.get()
                    .map(|(ok, t)| {
                        let cls = if ok {
                            "p-2.5 bg-secondary-container text-on-secondary-container rounded-lg text-body-sm"
                        } else {
                            "p-2.5 bg-error-container text-on-error-container rounded-lg text-body-sm"
                        };
                        view! { <div class=cls role="alert">{t}</div> }
                    })
            }}

            <Show when=move || buka.get() fallback=|| ()>
                <div class="space-y-2.5">
                    <p class="text-[11px] text-on-surface-variant">
                        "Cukup isi jumlah yang ditransfer dan lampirkan fotonya. Masa berlaku \
                         (dari tanggal berapa sampai kapan) ditetapkan pengurus setelah \
                         bukti dicocokkan dengan rekening pondok."
                    </p>
                    <div>
                        <label class="text-[11px] text-on-surface-variant ml-1">
                            "Jumlah ditransfer (rupiah)"
                        </label>
                        <input
                            class=field
                            r#type="text"
                            inputmode="numeric"
                            placeholder="mis. 500000"
                            prop:value=move || nominal.get()
                            on:input=move |e| nominal.set(event_target_value(&e))
                        />
                    </div>
                    <label class="flex items-center gap-2 py-2.5 px-3 rounded-xl border-2 border-dashed border-outline-variant text-body-sm text-on-surface-variant cursor-pointer">
                        <span class="material-symbols-outlined text-[20px] text-primary">
                            "photo_camera"
                        </span>
                        <span class="flex-1 min-w-0 truncate">
                            {move || {
                                let n = nama_berkas.get();
                                if n.is_empty() { "Pilih foto bukti transfer".to_string() } else { n }
                            }}
                        </span>
                        <input
                            type="file"
                            node_ref=file_input
                            accept="image/*"
                            class="hidden"
                            on:change=on_pick
                        />
                    </label>
                    <input
                        class=field
                        r#type="text"
                        placeholder="Catatan (opsional) — mis. transfer BSI a.n. Bapak Sulaiman"
                        prop:value=move || catatan.get()
                        on:input=move |e| catatan.set(event_target_value(&e))
                    />
                    <button
                        class="w-full py-3 rounded-xl bg-primary text-on-primary font-bold text-body-md press cursor-pointer disabled:opacity-60"
                        prop:disabled=move || busy.get()
                        on:click=kirim
                    >
                        {move || if busy.get() { "Mengirim…" } else { "Kirim ke Pengurus" }}
                    </button>
                </div>
            </Show>
        </div>
    }
}

#[component]
// `refetch` & `id` hanya dipakai di blok `cfg(target_arch = "wasm32")` (unggah
// bukti bayar lewat fetch browser). Di build SSR keduanya memang menganggur —
// itu benar, bukan sisa kode mati, jadi peringatannya dibungkam KHUSUS untuk
// target non-wasm alih-alih diberi awalan garis bawah (yang akan menyesatkan
// pembaca sisi klien).
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
fn MyBillRow(b: BillItem, refetch: impl Fn() + Copy + Send + Sync + 'static) -> impl IntoView {
    let id = b.id;
    // Precompute agar `b.proof_url` tak di-move ke >1 closure.
    let proof_url = b.proof_url.clone();
    let has_proof = !proof_url.is_empty();
    let uploading = RwSignal::new(false);
    let file_input: NodeRef<leptos::html::Input> = NodeRef::new();

    let on_pick = move |_ev: leptos::ev::Event| {
        #[cfg(target_arch = "wasm32")]
        {
            let Some(input) = file_input.get() else { return };
            let Some(files) = input.files() else { return };
            let Some(file) = files.get(0) else { return };
            uploading.set(true);
            leptos::task::spawn_local(async move {
                let form = web_sys::FormData::new().unwrap();
                let _ = form.append_with_str("bill_id", &id.to_string());
                let _ = form.append_with_blob("file", &file);
                let win = web_sys::window().unwrap();
                let opts = web_sys::RequestInit::new();
                opts.set_method("POST");
                opts.set_body(form.as_ref());
                if let Ok(req) = web_sys::Request::new_with_str_and_init("/api/bills/proof", &opts) {
                    let _ = wasm_bindgen_futures::JsFuture::from(win.fetch_with_request(&req)).await;
                }
                uploading.set(false);
                refetch();
            });
        }
        let _ = &file_input;
    };

    // Santri melihat layar ini juga — jadi kata yang dipakai bukan tuduhan.
    let badge = match b.status.as_str() {
        "lunas" => ("Lunas", "text-primary bg-secondary-container"),
        "menunggu" => ("Menunggu diperiksa", "text-warning bg-warning/10"),
        "ditolak" => ("Ditolak", "text-error bg-error-container"),
        _ if b.overdue => ("Periode terlewat", "text-error bg-error-container"),
        _ => ("Periode berjalan", "text-on-surface-variant bg-surface-container-high"),
    };
    let menunggu = b.status == "menunggu";
    let ditolak = b.status == "ditolak";
    let alasan = b.reject_reason.clone();
    // Periode baru ada setelah diverifikasi — pada baris pengajuan kolomnya
    // kosong, dan "Periode s/d " tanpa tanggal terbaca seperti data rusak.
    let periode = if b.expired_date.is_empty() {
        if b.submitted_at.is_empty() {
            String::new()
        } else {
            format!("Dikirim {}", b.submitted_at)
        }
    } else {
        format!("Periode s/d {}", b.expired_date)
    };

    view! {
        <div class="ppm-card p-4 space-y-2">
            <div class="flex items-start justify-between gap-2">
                <div class="min-w-0">
                    <p class="text-body-md font-semibold text-on-background">{b.title.clone()}</p>
                    {(!periode.is_empty())
                        .then(|| view! {
                            <p class="text-[11px] text-on-surface-variant">{periode.clone()}</p>
                        })}
                </div>
                <div class="text-right shrink-0">
                    <p class="text-body-md font-bold text-on-background">{fmt_rupiah(b.price)}</p>
                    <span class=format!("text-[10px] font-bold px-2 py-0.5 rounded-full {}", badge.1)>{badge.0}</span>
                </div>
            </div>
            // Alasan penolakan ditampilkan UTUH, bukan diringkas: inilah satu-
            // satunya cara keluarga tahu apa yang harus diperbaiki sebelum
            // mengirim ulang.
            {(ditolak && !alasan.is_empty())
                .then(|| view! {
                    <p class="text-body-sm text-error bg-error-container/40 rounded-lg px-2.5 py-2">
                        {alasan.clone()}
                    </p>
                })}
            {menunggu
                .then(|| view! {
                    <p class="text-[11px] text-on-surface-variant">
                        "Sedang diperiksa pengurus. Masa berlakunya ditetapkan setelah bukti dicocokkan."
                    </p>
                })}
            // Tombol unggah bukti hanya untuk tagihan LAMA yang dibuat pengurus
            // (status "belum"). Pengajuan sudah membawa buktinya sendiri, dan
            // yang ditolak harus dikirim ulang lewat form di atas — bukan
            // ditimpa diam-diam, supaya jejak penolakannya tetap terbaca.
            {(b.status == "belum").then(|| view! {
                <div>
                    <label class="inline-flex items-center gap-2 text-body-sm text-primary font-semibold cursor-pointer">
                        <span class="material-symbols-outlined text-[18px]">"cloud_upload"</span>
                        {move || if uploading.get() { "Mengunggah…" } else if has_proof { "Ganti bukti" } else { "Unggah bukti bayar" }}
                        <input type="file" node_ref=file_input accept="image/*" class="hidden" on:change=on_pick />
                    </label>
                </div>
            })}
            {has_proof.then({ let url = proof_url.clone(); move || view! {
                <a href=url.clone() target="_blank" class="inline-flex items-center gap-1 text-body-sm text-on-surface-variant">
                    <span class="material-symbols-outlined text-[16px]">"fact_check"</span> "Bukti terunggah"
                </a>
            }})}
        </div>
    }
}
