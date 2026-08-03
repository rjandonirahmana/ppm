//! web/pages/kelas_detail — Detail & kelola satu kelas (/kelas/:id).
//!
//! Kerangka halaman + bilah tab; isi tiap tab ada di modulnya sendiri:
//!   • [`santri`]    — anggota kelas + tambah santri (cari + pilih jadwal)
//!   • [`jadwal`]    — daftar jadwal berulang, dan [`jadwal_form`] untuk
//!                     membuat/menyunting termasuk picker tanggal manual
//!   • [`sesi`]      — daftar sesi + buat sesi ad-hoc
//!   • [`kurikulum`] — materi kurikulum kelas (migrasi 17)
//!
//! Dipecah dari satu berkas 2.679 baris: menyunting satu tab dulu berarti
//! menggulir melewati empat tab lain yang tak ada hubungannya, dan setiap
//! perubahan kecil memaksa meng-compile ulang seluruhnya.

mod jadwal;
mod jadwal_form;
mod kurikulum;
mod santri;
mod sesi;

use jadwal::JadwalTab;
use kurikulum::KurikulumTab;
use santri::SantriTab;
use sesi::SesiTab;

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use crate::models::KelasDetail;
use crate::web::api::{
    kelas_detail,
    set_class_staff_action,
    update_class_action,
};
use crate::web::components::{DeviceFrame, FetchError, MobileHeader};

#[component]
pub fn KelasDetailPage() -> impl IntoView {
    let params = use_params_map();
    let class_id = Memo::new(move |_| {
        params
            .read()
            .get("id")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    });
    let data = Resource::new(
        move || class_id.get(),
        |id| async move { kelas_detail(id).await },
    );

    Effect::new(move |_| {
        if let Some(Err(e)) = data.get() {
            if crate::web::components::is_auth_error(&e.to_string()) {
                #[cfg(target_arch = "wasm32")]
                if let Some(w) = web_sys::window() {
                    let _ = w.location().replace("/login");
                }
            }
        }
    });

    // Tab aktif: "santri" | "jadwal" | "sesi".
    let tab = RwSignal::new("santri".to_string());

    view! {
        <Title text="Detail Kelas — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Detail Kelas" back_href="/kelas" />

                <div class="px-5 pt-5 space-y-4">
                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                                <div class="grid gap-3 md:grid-cols-2">
                                    <div class="h-32 bg-surface-container rounded-2xl"></div>
                                    <div class="h-32 bg-surface-container rounded-2xl hidden md:block"></div>
                                </div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => {
                                        view! { <DetailBody d=d tab=tab refetch=move || data.refetch() /> }
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
fn DetailBody(
    d: KelasDetail,
    tab: RwSignal<String>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let class_id = d.id;
    let member_count = d.members.len();
    let sched_count = d.schedules.len();
    let sesi_count = d.sessions.len();
    let category = d.category.clone();
    let cat_opts = StoredValue::new(d.category_options.clone());
    let golongan = d.golongan.clone();
    let gol_opts = StoredValue::new(d.golongan_options.clone());

    // Mode edit hero (Edit Detail Kelas: nama + kategori + golongan).
    let editing = RwSignal::new(false);
    let name_v = RwSignal::new(d.name.clone());
    let cat_v = RwSignal::new(d.category.clone());
    let gol_v = RwSignal::new(d.golongan.clone());
    let busy = RwSignal::new(false);
    let err = RwSignal::new(Option::<String>::None);
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        err.set(None);
        let (n, c, g) = (name_v.get_untracked(), cat_v.get_untracked(), gol_v.get_untracked());
        leptos::task::spawn_local(async move {
            match update_class_action(class_id, n, c, g).await {
                Ok(_) => {
                    editing.set(false);
                    refetch();
                }
                Err(e) => {
                    let m = e.to_string();
                    err.set(Some(m.rsplit(": ").next().unwrap_or(&m).to_string()));
                }
            }
            busy.set(false);
        });
    };

    view! {
        // ── Hero kelas (dgn Edit Detail: nama + kategori) ───────────────────
        <div class="spiritual-gradient rounded-2xl p-5 text-on-primary shadow-lg shadow-primary/20 anim-in">
            {move || {
                if editing.get() {
                    view! {
                        <form class="space-y-3" method="post" on:submit=save>
                            <p class="text-body-lg font-bold flex items-center gap-2">
                                <span class="material-symbols-outlined">"edit"</span>
                                "Edit Detail Kelas"
                            </p>
                            {move || {
                                err.get()
                                    .map(|e| {
                                        view! {
                                            <div class="p-2.5 bg-white/15 rounded-lg text-body-sm">{e}</div>
                                        }
                                    })
                            }}
                            <div class="space-y-1">
                                <label class="text-[11px] font-bold tracking-wider opacity-80">"NAMA KELAS"</label>
                                <input
                                    type="text"
                                    class="w-full bg-white/15 border-0 rounded-lg px-3 py-2.5 text-body-md text-on-primary placeholder-white/50"
                                    prop:value=move || name_v.get()
                                    on:input=move |ev| name_v.set(event_target_value(&ev))
                                    required=true
                                />
                            </div>
                            <div class="space-y-1">
                                <label class="text-[11px] font-bold tracking-wider opacity-80">"KATEGORI KELAS"</label>
                                <input
                                    type="text"
                                    list="kategori-detail"
                                    class="w-full bg-white/15 border-0 rounded-lg px-3 py-2.5 text-body-md text-on-primary placeholder-white/50"
                                    placeholder="mis. Lambatan — ketik baru bila belum ada"
                                    prop:value=move || cat_v.get()
                                    on:input=move |ev| cat_v.set(event_target_value(&ev))
                                />
                                <datalist id="kategori-detail">
                                    {cat_opts
                                        .get_value()
                                        .into_iter()
                                        .map(|c| view! { <option value=c></option> })
                                        .collect_view()}
                                </datalist>
                            </div>
                            <div class="space-y-1">
                                <label class="text-[11px] font-bold tracking-wider opacity-80">"GOLONGAN"</label>
                                <input
                                    type="text"
                                    list="golongan-detail"
                                    class="w-full bg-white/15 border-0 rounded-lg px-3 py-2.5 text-body-md text-on-primary placeholder-white/50"
                                    placeholder="mis. Bacaan/Makna — ketik baru bila belum ada"
                                    prop:value=move || gol_v.get()
                                    on:input=move |ev| gol_v.set(event_target_value(&ev))
                                />
                                <datalist id="golongan-detail">
                                    {gol_opts
                                        .get_value()
                                        .into_iter()
                                        .map(|g| view! { <option value=g></option> })
                                        .collect_view()}
                                </datalist>
                            </div>
                            <div class="grid grid-cols-2 gap-2 pt-1">
                                <button
                                    type="button"
                                    class="py-2.5 rounded-lg bg-white/10 border border-white/20 font-semibold text-body-sm"
                                    on:click=move |_| editing.set(false)
                                >
                                    "Batal"
                                </button>
                                <button
                                    type="submit"
                                    class="py-2.5 rounded-lg bg-primary-fixed text-primary font-bold text-body-sm disabled:opacity-60"
                                    disabled=move || busy.get()
                                >
                                    {move || if busy.get() { "Menyimpan…" } else { "Simpan Perubahan" }}
                                </button>
                            </div>
                        </form>
                    }
                        .into_any()
                } else {
                    let category = category.clone();
                    let golongan = golongan.clone();
                    view! {
                        <div class="flex items-start justify-between gap-3">
                            <div class="min-w-0">
                                <div class="flex flex-wrap gap-1.5 mb-1">
                                    {(!golongan.is_empty())
                                        .then(|| {
                                            view! {
                                                <span class="inline-block px-2.5 py-0.5 rounded-full bg-primary-fixed text-primary text-[10px] font-bold tracking-wider uppercase">
                                                    {golongan}
                                                </span>
                                            }
                                        })}
                                    {(!category.is_empty())
                                        .then(|| {
                                            view! {
                                                <span class="inline-block px-2.5 py-0.5 rounded-full bg-white/20 text-[10px] font-bold tracking-wider uppercase">
                                                    {category}
                                                </span>
                                            }
                                        })}
                                </div>
                                <h2 class="text-headline-sm font-bold">{name_v.get()}</h2>
                            </div>
                            <button
                                class="w-9 h-9 rounded-full bg-white/15 flex items-center justify-center shrink-0 press"
                                on:click=move |_| editing.set(true)
                                aria-label="Edit detail kelas"
                            >
                                <span class="material-symbols-outlined text-[20px]">"edit"</span>
                            </button>
                        </div>
                        <div class="flex items-center gap-4 mt-3 text-body-sm">
                            <span class="flex items-center gap-1">
                                <span class="material-symbols-outlined text-[16px]">"groups"</span>
                                {member_count}
                                " Santri"
                            </span>
                            <span class="flex items-center gap-1">
                                <span class="material-symbols-outlined text-[16px]">"event"</span>
                                {sched_count}
                                " Jadwal"
                            </span>
                            <span class="flex items-center gap-1">
                                <span class="material-symbols-outlined text-[16px]">"cast_for_education"</span>
                                {sesi_count}
                                " Sesi"
                            </span>
                        </div>
                    }
                        .into_any()
                }
            }}
        </div>

        // ── Wali kelas & rute perizinan (migrasi 29) ────────────────────────
        <WaliKelasCard class_id=class_id d=d.clone() refetch=refetch />

        // ── Tab ─────────────────────────────────────────────────────────────
        <div class="flex gap-1 bg-surface-container rounded-xl p-1">
            <TabBtn tab=tab value="santri" label="Santri" />
            <TabBtn tab=tab value="jadwal" label="Jadwal" />
            <TabBtn tab=tab value="sesi" label="Sesi" />
            <TabBtn tab=tab value="kurikulum" label="Kurikulum" />
        </div>

        // ── Konten tab ──────────────────────────────────────────────────────
        {move || {
            match tab.get().as_str() {
                "jadwal" => {
                    view! { <JadwalTab class_id=class_id d=d.clone() refetch=refetch /> }.into_any()
                }
                "sesi" => {
                    view! { <SesiTab class_id=class_id d=d.clone() refetch=refetch /> }.into_any()
                }
                "kurikulum" => {
                    view! { <KurikulumTab class_id=class_id d=d.clone() refetch=refetch /> }
                        .into_any()
                }
                _ => {
                    view! { <SantriTab class_id=class_id d=d.clone() refetch=refetch /> }.into_any()
                }
            }
        }}
    }
}

#[component]
fn TabBtn(tab: RwSignal<String>, value: &'static str, label: &'static str) -> impl IntoView {
    let cls = move || {
        if tab.get() == value {
            "flex-1 py-2.5 rounded-lg bg-surface-container-lowest text-primary font-semibold text-body-sm shadow-sm press"
        } else {
            "flex-1 py-2.5 rounded-lg text-on-surface-variant font-medium text-body-sm press"
        }
    };
    view! {
        <button class=cls on:click=move |_| tab.set(value.to_string())>
            {label}
        </button>
    }
}

// ── Wali kelas & rute perizinan ───────────────────────────────────────────────

#[component]
fn WaliKelasCard(
    class_id: i64,
    d: KelasDetail,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let teacher_opts = StoredValue::new(d.teacher_options.clone());
    let pamong_opts = StoredValue::new(d.pamong_options.clone());
    let wali = RwSignal::new(d.wali_kelas_id);
    let pamong = RwSignal::new(d.pamong_id);
    let req_pamong = RwSignal::new(d.require_pamong);
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let save = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let (w, pm, rp) = (wali.get_untracked(), pamong.get_untracked(), req_pamong.get_untracked());
        leptos::task::spawn_local(async move {
            match set_class_staff_action(class_id, w, pm, rp).await {
                Ok(_) => {
                    msg.set(Some((true, "Tersimpan.".into())));
                    refetch();
                }
                Err(e) => {
                    let m = e.to_string();
                    msg.set(Some((false, m.rsplit(": ").next().unwrap_or(&m).to_string())));
                }
            }
            busy.set(false);
        });
    };

    view! {
        <div class="ppm-card p-4 space-y-3 anim-in">
            <div class="flex items-center gap-2">
                <span class="material-symbols-outlined text-primary">"badge"</span>
                <h3 class="text-body-lg font-bold text-on-background">"Wali Kelas & Perizinan"</h3>
            </div>
            <p class="text-body-sm text-on-surface-variant">
                "Wali kelas menyetujui izin/sakit/keluar santri kelas ini. Atur juga apakah izin lewat Pamong dulu atau langsung ke wali kelas."
            </p>

            <label class="space-y-1 block">
                <span class="text-[11px] font-bold tracking-wider text-on-surface-variant">"WALI KELAS"</span>
                <select
                    class="w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface cursor-pointer"
                    on:change=move |ev| wali.set(event_target_value(&ev).parse().unwrap_or(0))
                >
                    <option value="0" selected=move || wali.get() == 0>"— Belum ada wali kelas"</option>
                    {teacher_opts
                        .get_value()
                        .into_iter()
                        .map(|t| {
                            let tid = t.id;
                            view! {
                                <option value=t.id.to_string() selected=move || wali.get() == tid>
                                    {t.name}
                                </option>
                            }
                        })
                        .collect_view()}
                </select>
            </label>

            <label class="space-y-1 block">
                <span class="text-[11px] font-bold tracking-wider text-on-surface-variant">"PAMONG KELAS"</span>
                <select
                    class="w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface cursor-pointer"
                    on:change=move |ev| pamong.set(event_target_value(&ev).parse().unwrap_or(0))
                >
                    <option value="0" selected=move || pamong.get() == 0>"— Belum ada pamong"</option>
                    {pamong_opts
                        .get_value()
                        .into_iter()
                        .map(|t| {
                            let pid = t.id;
                            view! {
                                <option value=t.id.to_string() selected=move || pamong.get() == pid>
                                    {t.name}
                                </option>
                            }
                        })
                        .collect_view()}
                </select>
            </label>
            <p class="text-[11px] text-on-surface-variant">
                "Pamong kelas memverifikasi kehadiran gate santri kelas ini, jadi tahap-1 persetujuan izin, & menerima WA ~1 jam sebelum sesi untuk mengatur dewan guru pengisi."
            </p>

            <label class="flex items-center gap-3 cursor-pointer py-1">
                <input
                    type="checkbox"
                    class="w-5 h-5 accent-primary cursor-pointer"
                    prop:checked=move || req_pamong.get()
                    on:change=move |ev| req_pamong.set(event_target_checked(&ev))
                />
                <span class="text-body-sm text-on-background">
                    "Izin harus lewat Pamong dulu (2 tahap). Nonaktifkan = langsung ke wali kelas."
                </span>
            </label>

            <div class="flex items-center gap-3">
                <button
                    class="px-5 py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-sm cursor-pointer press disabled:opacity-60"
                    prop:disabled=move || busy.get()
                    on:click=save
                >
                    {move || if busy.get() { "Menyimpan…" } else { "Simpan" }}
                </button>
                {move || {
                    msg.get()
                        .map(|(ok, m)| {
                            let cls = if ok { "text-success" } else { "text-error" };
                            view! { <span class=format!("text-body-sm {cls}")>{m}</span> }
                        })
                }}
            </div>
        </div>
    }
}
