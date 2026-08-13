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

pub use jadwal::PosisiBerjalan;
use jadwal::JadwalTab;
pub use kurikulum::KurikulumTab;
use santri::SantriTab;
use sesi::SesiTab;

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use crate::models::KelasDetail;
use crate::web::api::{
    kelas_detail,
    set_class_wali_action,
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

    crate::web::components::guard_sesi(data);

    // Tab aktif: "santri" | "jadwal" | "sesi".
    let tab = RwSignal::new("santri".to_string());

    view! {
        <Title text="Detail Kelas — AFM SMART" />
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
    let hero_can_manage = d.can_manage;
    let member_count = d.members.len();
    let sched_count = d.schedules.len();
    let sesi_count = d.sessions.len();
    let category = d.category.clone();
    let jenjang = d.jenjang.clone();
    // Hanya kelas KBM yang punya kurikulum berjenjang (migrasi 65).
    let ada_kurikulum = category == "kbm";
    // `tab` bertahan saat berpindah antar kelas. Kalau ia tertinggal di
    // "kurikulum" lalu kelas non-KBM dibuka, isinya memang jatuh ke Santri
    // (guard di bawah) — tapi tak ada satu pun tab yang tersorot, dan halaman
    // tampak seperti gagal memuat. Dikembalikan ke Santri.
    Effect::new(move |_| {
        if !ada_kurikulum && tab.get() == "kurikulum" {
            tab.set("santri".to_string());
        }
    });

    // Mode edit hero (Edit Detail Kelas: nama + kategori + jenjang).
    let editing = RwSignal::new(false);
    let name_v = RwSignal::new(d.name.clone());
    let cat_v = RwSignal::new(d.category.clone());
    let gol_v = RwSignal::new(d.jenjang.clone());
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
                                        err.set(Some(crate::web::components::pesan_galat(e)));
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
                                <label class="text-[11px] font-bold tracking-wider opacity-80">"JENIS KELAS"</label>
                                // Dropdown tertutup — lihat alasannya di
                                // pages/kelas.rs (form buat kelas).
                                <select
                                    class="w-full bg-white/15 border-0 rounded-lg px-3 py-2.5 text-body-md text-on-primary"
                                    prop:value=move || cat_v.get()
                                    on:change=move |ev| cat_v.set(event_target_value(&ev))
                                >
                                    {crate::models::KATEGORI_KELAS
                                        .iter()
                                        .map(|(k, l)| view! { <option value=*k>{*l}</option> })
                                        .collect_view()}
                                </select>
                            </div>
                            <Show when=move || cat_v.get() == "kbm">
                                <div class="space-y-1">
                                    <label class="text-[11px] font-bold tracking-wider opacity-80">
                                        "JENJANG"
                                    </label>
                                    <select
                                        class="w-full bg-white/15 border-0 rounded-lg px-3 py-2.5 text-body-md text-on-primary"
                                        prop:value=move || gol_v.get()
                                        on:change=move |ev| gol_v.set(event_target_value(&ev))
                                    >
                                        <option value="">"— pilih jenjang —"</option>
                                        {crate::models::JENJANG
                                            .iter()
                                            .map(|(k, l)| view! { <option value=*k>{*l}</option> })
                                            .collect_view()}
                                    </select>
                                    <p class="text-[11px] opacity-70">
                                        "Santri naik jenjang setelah kurikulumnya tuntas."
                                    </p>
                                </div>
                            </Show>
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
                    let jenjang = jenjang.clone();
                    view! {
                        <div class="flex items-start justify-between gap-3">
                            <div class="min-w-0">
                                <div class="flex flex-wrap gap-1.5 mb-1">
                                    {(!jenjang.is_empty())
                                        .then(|| {
                                            view! {
                                                <span class="inline-block px-2.5 py-0.5 rounded-full bg-primary-fixed text-primary text-[10px] font-bold tracking-wider uppercase">
                                                    {crate::models::jenjang_label(&jenjang)}
                                                </span>
                                            }
                                        })}
                                    <span class="inline-block px-2.5 py-0.5 rounded-full bg-white/20 text-[10px] font-bold tracking-wider uppercase">
                                        {crate::models::kategori_label(&category)}
                                    </span>
                                </div>
                                <h2 class="text-headline-sm font-bold">{name_v.get()}</h2>
                            </div>
                            // Mengubah identitas kelas = wewenang admin.
                            {hero_can_manage
                                .then(|| {
                                    view! {
                                        <button
                                            class="w-9 h-9 rounded-full bg-white/15 flex items-center justify-center shrink-0 press"
                                            on:click=move |_| editing.set(true)
                                            aria-label="Edit detail kelas"
                                        >
                                            <span class="material-symbols-outlined text-[20px]">"edit"</span>
                                        </button>
                                    }
                                })}
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
        // KURIKULUM hanya untuk kelas KBM. Kurikulum itu daftar kitab berjenjang
        // yang harus dituntaskan sebelum santri naik jenjang — dan hanya kelas
        // KBM yang berjenjang (migrasi 65). Pada kelas piket, apel, atau sholat
        // tabnya selalu kosong, dan tab kosong yang permanen mengajari orang
        // untuk berhenti membuka tab.
        <div class="flex gap-1 bg-surface-container rounded-xl p-1">
            <TabBtn tab=tab value="santri" label="Santri" />
            <TabBtn tab=tab value="jadwal" label="Jadwal" />
            <TabBtn tab=tab value="sesi" label="Sesi" />
            {ada_kurikulum.then(|| view! { <TabBtn tab=tab value="kurikulum" label="Kurikulum" /> })}
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
                // Dijaga di sini juga, bukan cuma di bilah tab: `tab` bisa
                // tertinggal di "kurikulum" saat pengguna berpindah dari kelas
                // KBM ke kelas non-KBM — tabnya lenyap, tapi isinya tetap
                // terpasang.
                "kurikulum" if ada_kurikulum => {
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
    // Kelas KBM ditandai untuk KALIMATNYA saja (izin lewat wali KBM); bidang
    // walinya sendiri kini tampil di semua kategori.
    let kbm = d.category == "kbm";
    let wali = RwSignal::new(d.wali_kelas_id);
    let can_manage = d.can_manage;
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let save = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let w = wali.get_untracked();
        leptos::task::spawn_local(async move {
            match set_class_wali_action(class_id, w).await {
                Ok(_) => {
                    msg.set(Some((true, "Tersimpan.".into())));
                    refetch();
                }
                Err(e) => {
                    msg.set(Some((false, crate::web::components::pesan_galat(e))));
                }
            }
            busy.set(false);
        });
    };

    view! {
        <div class="ppm-card p-4 space-y-3 anim-in">
            <div class="flex items-center gap-2">
                <span class="material-symbols-outlined text-primary">"badge"</span>
                <h3 class="text-body-lg font-bold text-on-background">"Wali Kelas & Verifikasi"</h3>
                // Penanda TERKUNCI, bukan tombol yang diam-diam gagal: pemirsa
                // langsung tahu ini bukan wewenangnya, bukan aplikasi yang rusak.
                {(!can_manage)
                    .then(|| {
                        view! {
                            <span class="ppm-chip bg-surface-container-highest text-on-surface-variant flex items-center gap-1">
                                <span class="material-symbols-outlined text-[14px]">"lock"</span>
                                "Hanya admin"
                            </span>
                        }
                    })}
            </div>
            <p class="text-body-sm text-on-surface-variant">
                {if !can_manage {
                    "Hanya admin/ketua yang boleh mengubah bagian ini. Ditampilkan agar Anda tahu siapa wali kelasnya dan bagaimana absensinya diverifikasi."
                } else if kbm {
                    "Wali kelas memegang kelas ini: menunjuk guru pengisi tiap sesi, mengesahkan absensinya, dan MENYETUJUI IZIN santrinya — satu santri satu kelas KBM, jadi satu izin cukup."
                } else {
                    "Wali kelas memegang kelas ini: menunjuk guru pengisi tiap sesi dan mengesahkan absensinya. Perizinan santri TIDAK lewat sini — itu selalu wali kelas KBM-nya."
                }}
            </p>
            // Wali kelas kini ada di SEMUA kategori (migrasi 84). Dulu bidang
            // ini disembunyikan di kelas non-KBM karena petugasnya pamong;
            // setelah peran itu hilang, kelas sholat/apel/piket justru tak
            // punya penanggung jawab sama sekali kalau bidang ini tak ada.
            {
                {
                    view! {
                        <label class="space-y-1 block">
                            <span class="text-[11px] font-bold tracking-wider text-on-surface-variant">
                                "WALI KELAS"
                            </span>
                            <select
                                class="w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface cursor-pointer"
                                prop:disabled=!can_manage
                                on:change=move |ev| {
                                    wali.set(event_target_value(&ev).parse().unwrap_or(0))
                                }
                            >
                                <option value="0" selected=move || wali.get() == 0>
                                    "— Belum ada wali kelas"
                                </option>
                                {teacher_opts
                                    .get_value()
                                    .into_iter()
                                    .map(|t| {
                                        let tid = t.id;
                                        view! {
                                            <option
                                                value=t.id.to_string()
                                                selected=move || wali.get() == tid
                                            >
                                                {t.name}
                                            </option>
                                        }
                                    })
                                    .collect_view()}
                            </select>
                        </label>
                    }
                }
            }

            // Mode verifikasi absensi TAK LAGI BISA DIPILIH: peran pamong
            // dihapus (migrasi 84) dan semua kelas dipindah ke satu tahap.
            // Dropdown-nya dibuang, bukan disisakan dengan satu pilihan — pilihan
            // tunggal cuma menyuruh orang menebak-nebak apa yang hilang.
            <p class="text-[11px] text-on-surface-variant flex items-start gap-1.5">
                <span class="material-symbols-outlined text-[15px] shrink-0">"verified"</span>
                "Absensi kelas ini disahkan SATU tahap oleh guru pengisi sesi (atau wali kelas bila sesi belum menetapkan pengisi)."
            </p>

            <div class="flex items-center gap-3">
                <button
                    class="px-5 py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-sm cursor-pointer press disabled:opacity-60"
                    prop:disabled=move || busy.get() || !can_manage
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
