//! Halaman "Kelas Saya" — dipakai TIGA peran dengan isi yang sama:
//!   • santri            → kelas yang ia IKUTI
//!   • wali kelas (guru) → kelas yang ia PEGANG
//!   • pamong            → kelas yang ia DAMPINGI
//!
//! Yang berbeda hanya kelas mana yang diambil (ditentukan server dari peran di
//! sesi) dan beberapa kalimatnya. Isi kartunya identik — kurikulum, materi yang
//! sedang dibahas, daftar santri, wali kelas & pamong — karena pertanyaan yang
//! ingin dijawab ketiganya memang sama: "kelas ini sedang di mana?".
//!
//! Semuanya BACA SAJA: tak ada tombol ubah/hapus. Pengelolaan tetap di
//! /kelas/:id. id pemirsa diambil dari sesi di server, tak pernah dari URL.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{KelasSayaItem, SessionUser};
use crate::web::api::kelas_saya_data;
use crate::web::components::{DeviceFrame, EmptyState, FetchError, MobileHeader};

#[component]
pub fn KelasSayaPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { kelas_saya_data().await });

    // Galat auth → /login, sama seperti halaman santri lain.
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

    view! {
        <Title text="Kelas Saya — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Kelas Saya" subtitle="Kurikulum & materi berjalan" />
                <div class="px-5 pt-5 space-y-4 stagger">
                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="h-40 bg-surface-container rounded-2xl"></div>
                                <div class="h-40 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Err(e) => {
                                        view! { <FetchError err=e.to_string() /> }.into_any()
                                    }
                                    Ok(d) if d.items.is_empty() => {
                                        let (judul, sub) = if d.sebagai_staf {
                                            (
                                                "Belum ditugaskan di kelas mana pun",
                                                "Kelas akan muncul di sini setelah kamu ditunjuk sebagai wali kelas atau pamong.",
                                            )
                                        } else {
                                            (
                                                "Belum terdaftar di kelas mana pun",
                                                "Hubungi pengelola bila menurutmu ini keliru.",
                                            )
                                        };
                                        view! { <EmptyState icon="school" title=judul subtitle=sub /> }
                                            .into_any()
                                    }
                                    Ok(d) => {
                                        view! {
                                            <div class="space-y-4 md:space-y-0 md:grid md:grid-cols-2 md:gap-4">
                                                {d.items
                                                    .into_iter()
                                                    .map(|k| view! { <KelasCard k=k /> })
                                                    .collect_view()}
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

/// Satu kelas: identitas + petugas + materi berjalan + kurikulum + teman.
#[component]
fn KelasCard(k: KelasSayaItem) -> impl IntoView {
    let session = use_context::<Resource<Option<SessionUser>>>();
    // Daftar santri bisa panjang; dilipat agar kartu tetap terbaca — yang
    // dicari lebih sering "materi sekarang", bukan daftar nama.
    let buka_teman = RwSignal::new(false);

    let jumlah_teman = k.members.len();
    let members = StoredValue::new(k.members);
    let golongan = k.golongan.clone();
    let category = k.category.clone();
    let peran = k.peran_saya.clone();
    let wali = k.wali_kelas.clone();
    let pamong = k.pamong.clone();

    view! {
        <div class="ppm-card p-4 anim-in space-y-3" style="border-left:4px solid #064e3b">
            // ── Identitas kelas ────────────────────────────────────────────
            <div>
                <div class="flex items-center gap-1.5 flex-wrap">
                    // Peran pemirsa di kelas ini — hanya untuk staf; santri
                    // adalah peserta, bukan petugas, jadi tak berlencana.
                    {(!peran.is_empty())
                        .then(|| view! { <span class="ppm-chip bg-primary text-on-primary">{peran.clone()}</span> })}
                    {(!golongan.is_empty())
                        .then(|| view! { <span class="ppm-chip bg-secondary-container text-primary">{golongan.clone()}</span> })}
                    {(!category.is_empty())
                        .then(|| view! { <span class="ppm-chip bg-surface-container-highest text-on-surface-variant">{category.clone()}</span> })}
                </div>
                <p class="text-body-lg font-bold text-on-background mt-1.5">{k.name}</p>
            </div>

            // ── Petugas ────────────────────────────────────────────────────
            <div class="grid grid-cols-2 gap-2">
                <Petugas peran="Wali Kelas" nama=wali icon="badge" />
                <Petugas peran="Pamong" nama=pamong icon="supervisor_account" />
            </div>

            // ── Materi yang sedang dibahas per jadwal ──────────────────────
            {(!k.schedules.is_empty())
                .then(|| {
                    view! {
                        <div class="space-y-1.5">
                            <p class="text-[10px] font-bold tracking-wide text-on-surface-variant">
                                "JADWAL & MATERI SEKARANG"
                            </p>
                            {k.schedules
                                .into_iter()
                                .map(|j| {
                                    let ada_materi = !j.current_book_title.is_empty();
                                    view! {
                                        <div class="bg-surface-container rounded-xl px-3 py-2">
                                            <div class="flex items-center justify-between gap-2">
                                                <p class="text-body-sm font-semibold text-on-background truncate">
                                                    {j.title}
                                                </p>
                                                <span class="text-[10px] text-on-surface-variant shrink-0">
                                                    {j.recurrence_label}
                                                </span>
                                            </div>
                                            <p class="text-[11px] text-on-surface-variant">{j.time_label}</p>
                                            {if ada_materi {
                                                view! {
                                                    <p class="text-[11px] text-primary font-semibold mt-0.5">
                                                        {j.current_book_title}
                                                        {(!j.current_label.is_empty())
                                                            .then(|| format!(" · {}", j.current_label))}
                                                    </p>
                                                }
                                                    .into_any()
                                            } else {
                                                view! {
                                                    <p class="text-[11px] text-on-surface-variant mt-0.5">
                                                        "Materi belum ditentukan"
                                                    </p>
                                                }
                                                    .into_any()
                                            }}
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                })}

            // ── Kurikulum kelas ────────────────────────────────────────────
            {(!k.curriculum.is_empty())
                .then(|| {
                    view! {
                        <div class="space-y-1.5">
                            <p class="text-[10px] font-bold tracking-wide text-on-surface-variant">
                                "KURIKULUM KELAS"
                            </p>
                            {k.curriculum
                                .into_iter()
                                .map(|c| {
                                    let pct = c.progress_pct;
                                    let bar = if pct >= 100 {
                                        "h-full bg-success bar-grow"
                                    } else {
                                        "h-full bg-primary bar-grow"
                                    };
                                    let badge = match c.status.as_str() {
                                        "completed" => "ppm-chip bg-success/10 text-success",
                                        "upcoming" => {
                                            "ppm-chip bg-surface-container-highest text-on-surface-variant"
                                        }
                                        _ => "ppm-chip bg-primary/10 text-primary",
                                    };
                                    view! {
                                        <div class="bg-surface-container rounded-xl px-3 py-2">
                                            <div class="flex items-center justify-between gap-2">
                                                <p class="text-body-sm font-semibold text-on-background truncate">
                                                    {c.title}
                                                </p>
                                                <span class=badge>{c.status_label}</span>
                                            </div>
                                            {(!c.range_label.is_empty())
                                                .then(|| {
                                                    view! {
                                                        <p class="text-[11px] text-on-surface-variant">{c.range_label}</p>
                                                    }
                                                })}
                                            {(!c.current_label.is_empty())
                                                .then(|| {
                                                    view! {
                                                        <p class="text-[11px] text-primary font-semibold">
                                                            "Sudah sampai " {c.current_label}
                                                        </p>
                                                    }
                                                })}
                                            <div class="flex items-center justify-between text-[10px] font-semibold mt-1.5">
                                                <span class="text-on-surface-variant">"Progres"</span>
                                                <span class="text-on-background">{format!("{pct}%")}</span>
                                            </div>
                                            <div class="h-1.5 bg-surface-container-highest rounded-full overflow-hidden mt-1">
                                                <div class=bar style=format!("width: {pct}%")></div>
                                            </div>
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                })}

            // ── Teman sekelas ──────────────────────────────────────────────
            <div>
                <button
                    class="w-full flex items-center justify-between py-1.5 press"
                    on:click=move |_| buka_teman.update(|b| *b = !*b)
                >
                    <span class="text-[10px] font-bold tracking-wide text-on-surface-variant">
                        {format!("SANTRI DI KELAS INI ({jumlah_teman})")}
                    </span>
                    <span
                        class="material-symbols-outlined text-on-surface-variant transition-transform text-[20px]"
                        class:rotate-180=move || buka_teman.get()
                    >
                        "expand_more"
                    </span>
                </button>
                {move || {
                    buka_teman
                        .get()
                        .then(|| {
                            // Santri yang sedang masuk ditandai supaya mudah
                            // menemukan dirinya di daftar yang panjang.
                            let saya = session.and_then(|s| s.get()).flatten().map(|u| u.id);
                            view! {
                                <div class="space-y-1 anim-in">
                                    {members
                                        .get_value()
                                        .into_iter()
                                        .map(|m| {
                                            let ini_saya = saya == Some(m.id);
                                            let cls = if ini_saya {
                                                "flex items-center gap-2 px-3 py-1.5 rounded-lg bg-secondary-container"
                                            } else {
                                                "flex items-center gap-2 px-3 py-1.5 rounded-lg"
                                            };
                                            view! {
                                                <div class=cls>
                                                    <span class="w-7 h-7 rounded-full bg-surface-container-highest text-on-surface-variant flex items-center justify-center text-[11px] font-bold shrink-0">
                                                        {m.name.chars().next().unwrap_or('S').to_string().to_uppercase()}
                                                    </span>
                                                    <div class="min-w-0 flex-1">
                                                        <p class="text-body-sm text-on-background truncate">
                                                            {m.name}
                                                            {ini_saya.then(|| " (kamu)".to_string())}
                                                        </p>
                                                    </div>
                                                    {(!m.angkatan.is_empty())
                                                        .then(|| {
                                                            view! {
                                                                <span class="text-[10px] text-on-surface-variant shrink-0">
                                                                    {m.angkatan}
                                                                </span>
                                                            }
                                                        })}
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            }
                        })
                }}
            </div>
        </div>
    }
}

/// Satu petugas kelas. Kosong → "Belum ditunjuk", bukan bidang kosong yang
/// menyisakan tanya.
#[component]
fn Petugas(peran: &'static str, nama: String, icon: &'static str) -> impl IntoView {
    let kosong = nama.is_empty();
    view! {
        <div class="bg-surface-container rounded-xl px-3 py-2">
            <p class="text-[10px] text-on-surface-variant flex items-center gap-1">
                <span class="material-symbols-outlined text-[14px]">{icon}</span>
                {peran}
            </p>
            {if kosong {
                view! {
                    <p class="text-body-sm text-on-surface-variant italic">"Belum ditunjuk"</p>
                }
                    .into_any()
            } else {
                view! {
                    <p class="text-body-sm font-semibold text-on-background truncate">{nama}</p>
                }
                    .into_any()
            }}
        </div>
    }
}
