//! web/pages/tagihan.rs — Pembayaran santri (migrasi 37).
//!   • FinancePage (/tagihan): admin/ketua/santri_finance — daftar belum bayar,
//!     tandai lunas + verifikasi; admin/ketua juga buat/hapus pembayaran.
//!   • MyBillsPage (/tagihan-saya): santri lihat pembayaran mereka + unggah bukti bayar.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{fmt_rupiah, BillItem, SessionUser, StudentSearchItem};
use crate::web::api::{
    create_bill_action, delete_bill_action, finance_student_search, mark_bill_paid_action,
    my_bills_data, paid_bills_data, unpaid_bills_data,
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
    let refetch = move || data.refetch();
    // Tab: "unpaid" (belum bayar + kelola) | "history" (riwayat pembayaran lunas).
    let tab = RwSignal::new("unpaid".to_string());
    let paid = Resource::new(|| (), |_| async move { paid_bills_data().await });

    view! {
        <Title text="Pembayaran Santri — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Pembayaran Santri" subtitle="Kelola & verifikasi pembayaran" />
                <div class="px-5 pt-5 space-y-4 stagger">
                    // ── Tab bar ─────────────────────────────────────────────
                    <div class="grid grid-cols-2 gap-1 bg-surface-container rounded-xl p-1">
                        <FinTab tab=tab value="unpaid" label="Belum Bayar" />
                        <FinTab tab=tab value="history" label="Riwayat Pembayaran" />
                    </div>

                    // ── Belum Bayar ─────────────────────────────────────────
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
                                                <EmptyState icon="task_alt" title="Semua lunas 🎉"
                                                    subtitle="Tak ada tagihan yang belum dibayar." />
                                            }.into_any()
                                        } else {
                                            let total: i64 = list.iter().map(|b| b.price).sum();
                                            view! {
                                                <div class="ppm-card p-4 flex items-center justify-between">
                                                    <span class="text-body-sm text-on-surface-variant">
                                                        {format!("{} tagihan belum bayar", list.len())}
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
fn FinTab(tab: RwSignal<String>, value: &'static str, label: &'static str) -> impl IntoView {
    let cls = move || {
        if tab.get() == value {
            "py-2 rounded-lg bg-surface-container-lowest text-primary font-bold text-body-sm shadow-sm press"
        } else {
            "py-2 rounded-lg text-on-surface-variant font-semibold text-body-sm press"
        }
    };
    view! { <button class=cls on:click=move |_| tab.set(value.to_string())>{label}</button> }
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
                        <span class="text-[10px] font-bold text-error bg-error-container px-2 py-0.5 rounded-full">"Jatuh tempo"</span>
                    })}
                </div>
            </div>
            <p class="text-[11px] text-on-surface-variant">{format!("Berlaku {} → {}", b.started_date, b.expired_date)}</p>
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
                        on:click=del aria-label="Hapus tagihan">
                        <span class="material-symbols-outlined text-[18px]">"delete"</span>
                    </button>
                </Show>
            </div>
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
        busy.set(true); msg.set(None);
        leptos::task::spawn_local(async move {
            match create_bill_action(uid, t, pr, sd, ed, String::new()).await {
                Ok(_) => {
                    msg.set(Some((true, "Tagihan dibuat.".into())));
                    title.set(String::new()); price.set(String::new());
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
                    <span class="material-symbols-outlined text-primary">"add_circle"</span> "Buat Tagihan"
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
                            <label class="text-[11px] text-on-surface-variant ml-1">"Mulai"</label>
                            <input class=field r#type="date" prop:value=move || started.get()
                                on:input=move |e| started.set(event_target_value(&e)) />
                        </div>
                        <div>
                            <label class="text-[11px] text-on-surface-variant ml-1">"Jatuh tempo"</label>
                            <input class=field r#type="date" prop:value=move || expired.get()
                                on:input=move |e| expired.set(event_target_value(&e)) />
                        </div>
                    </div>
                    <button class="w-full py-2.5 bg-primary text-on-primary rounded-xl font-bold text-body-sm press disabled:opacity-60"
                        disabled=move || busy.get() on:click=submit>
                        {move || if busy.get() { "Menyimpan…" } else { "Simpan Tagihan" }}
                    </button>
                </div>
            })}
        </div>
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SANTRI — tagihan saya + unggah bukti
// ═══════════════════════════════════════════════════════════════════════════
#[component]
pub fn MyBillsPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { my_bills_data().await });
    let refetch = move || data.refetch();

    view! {
        <Title text="Pembayaran Saya — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Pembayaran Saya" subtitle="Pembayaran & bukti transfer" back_href="/santri" />
                <div class="px-5 pt-5 space-y-3 stagger">
                    <Suspense fallback=|| {
                        view! { <div class="h-20 bg-surface-container rounded-2xl animate-pulse"></div> }
                    }>
                        {move || {
                            data.get().map(|res| match res {
                                Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                Ok(list) => {
                                    if list.is_empty() {
                                        view! { <EmptyState icon="fact_check" title="Belum ada pembayaran"
                                            subtitle="Pembayaran dari pengurus akan muncul di sini." /> }.into_any()
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

#[component]
// `refetch` & `id` hanya dipakai di blok `cfg(target_arch = "wasm32")` (unggah
// bukti bayar lewat fetch browser). Di build SSR keduanya memang menganggur —
// itu benar, bukan sisa kode mati, jadi peringatannya dibungkam KHUSUS untuk
// target non-wasm alih-alih diberi awalan garis bawah (yang akan menyesatkan
// pembaca sisi klien).
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
fn MyBillRow(b: BillItem, refetch: impl Fn() + Copy + Send + Sync + 'static) -> impl IntoView {
    let lunas = b.status == "lunas";
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

    let badge = if lunas { ("Lunas", "text-primary bg-secondary-container") }
        else if b.overdue { ("Jatuh tempo", "text-error bg-error-container") }
        else { ("Belum bayar", "text-on-surface-variant bg-surface-container-high") };

    view! {
        <div class="ppm-card p-4 space-y-2">
            <div class="flex items-start justify-between gap-2">
                <div>
                    <p class="text-body-md font-semibold text-on-background">{b.title.clone()}</p>
                    <p class="text-[11px] text-on-surface-variant">{format!("Jatuh tempo {}", b.expired_date)}</p>
                </div>
                <div class="text-right shrink-0">
                    <p class="text-body-md font-bold text-on-background">{fmt_rupiah(b.price)}</p>
                    <span class=format!("text-[10px] font-bold px-2 py-0.5 rounded-full {}", badge.1)>{badge.0}</span>
                </div>
            </div>
            {(!lunas).then(|| view! {
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
