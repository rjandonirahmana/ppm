//! web/pages/izin_aktif.rs — Sedang Izin (/izin-aktif): santri yang izin atau
//! sakitnya BERLAKU HARI INI.
//!
//! Halaman TERSENDIRI, bukan tab di `/izin-staf`, karena penontonnya berbeda:
//! `/izin-staf` adalah antrean KEPUTUSAN dan sengaja tertutup untuk ketua &
//! admin (izin diputuskan orang yang mengenal santrinya — lihat gerbang
//! `permit_queue_data`). Yang ini BACAAN: siapa saja yang hari ini tak masuk,
//! sampai kapan, dan dengan alasan apa. Justru ketua & admin yang paling
//! membutuhkannya.
//!
//! Yang dihitung hanya izin yang SUDAH DISETUJUI. Pengajuan yang masih
//! menunggu keputusan tak ikut: selama belum diputus, santrinya belum berizin —
//! dan daftar yang mencampur keduanya membuat pengurus mengira seseorang sudah
//! dibolehkan padahal belum. Antreannya ada di `/izin-staf`.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::SedangIzinItem;
use crate::web::api::sedang_izin_data;
use crate::web::components::{
    kartu_grid, DeviceFrame, EmptyState, FetchError, MobileHeader, Skeleton,
};

#[component]
pub fn IzinAktifPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { sedang_izin_data().await });

    crate::web::components::guard_sesi(data);

    // Pencarian DI KLIEN: daftarnya sudah ada di memori (maks 300 baris) dan
    // pondok ini tak pernah punya ratusan santri berizin sekaligus — satu
    // perjalanan ke server untuk tiap huruf yang diketik tak menambah apa pun.
    let cari = RwSignal::new(String::new());

    view! {
        <Title text="Sedang Izin — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader
                    title="Sedang Izin"
                    subtitle="Santri yang izin/sakitnya berlaku hari ini"
                    back_href="/staf"
                />

                <div class="px-5 pt-5 space-y-4 stagger">
                    <input
                        type="search"
                        class="w-full bg-surface-container border-0 rounded-xl px-4 py-3 text-body-sm text-on-surface md:max-w-md"
                        placeholder="Cari nama, NIS, atau kelas…"
                        prop:value=move || cari.get()
                        on:input=move |ev| cari.set(event_target_value(&ev))
                    />

                    <Suspense fallback=|| view! { <Skeleton baris=3 tinggi="h-24" /> }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                    Ok(semua) if semua.is_empty() => {
                                        view! {
                                            <EmptyState
                                                icon="task_alt"
                                                title="Tidak ada yang izin hari ini"
                                                subtitle="Semua santri terjadwal masuk. Pengajuan yang masih menunggu keputusan ada di Tinjau Izin."
                                            />
                                        }
                                            .into_any()
                                    }
                                    Ok(semua) => {
                                        let total = semua.len();
                                        let sakit =
                                            semua.iter().filter(|i| i.kind == "sick").count();
                                        let daftar = StoredValue::new(semua);
                                        view! {
                                            <div class="grid grid-cols-2 gap-3 md:max-w-lg">
                                                <div class="ppm-card p-4">
                                                    <p class="text-body-sm text-on-surface-variant">"Sedang Izin"</p>
                                                    <p class="text-2xl font-bold text-on-background mt-1">{total}</p>
                                                </div>
                                                <div class="ppm-card p-4">
                                                    <p class="text-body-sm text-on-surface-variant">"Di antaranya Sakit"</p>
                                                    <p class="text-2xl font-bold text-warning mt-1">{sakit}</p>
                                                </div>
                                            </div>

                                            {move || {
                                                let q = cari.get().trim().to_lowercase();
                                                let hasil: Vec<SedangIzinItem> = daftar
                                                    .get_value()
                                                    .into_iter()
                                                    .filter(|i| {
                                                        q.is_empty()
                                                            || i.name.to_lowercase().contains(&q)
                                                            || i.nis.to_lowercase().contains(&q)
                                                            || i.class_name.to_lowercase().contains(&q)
                                                    })
                                                    .collect();
                                                if hasil.is_empty() {
                                                    return view! {
                                                        <p class="ppm-empty">"Tak ada yang cocok dengan pencarian itu."</p>
                                                    }
                                                        .into_any();
                                                }
                                                kartu_grid(
                                                        hasil
                                                            .into_iter()
                                                            .map(|i| view! { <KartuIzin i=i /> }.into_any())
                                                            .collect(),
                                                    )
                                                    .into_any()
                                            }}
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
fn KartuIzin(i: SedangIzinItem) -> impl IntoView {
    // Sakit dibedakan warnanya: itu satu-satunya jenis yang mungkin
    // DIPERPANJANG, dan pengurus yang menyapu daftar ini mencarinya lebih dulu.
    let chip = if i.kind == "sick" {
        "ppm-chip bg-warning/15 text-warning shrink-0"
    } else {
        "ppm-chip bg-info/10 text-info shrink-0"
    };
    let hari = if i.sisa_hari <= 1 {
        "Hari terakhir".to_string()
    } else {
        format!("{} hari lagi", i.sisa_hari)
    };
    let meta = format!("{} · {}", i.nis, i.class_name);

    view! {
        <div class="ppm-card p-4 space-y-2 anim-in">
            <div class="flex items-start justify-between gap-2">
                <div class="min-w-0">
                    <a
                        href=format!("/poin/{}", i.user_id)
                        class="text-body-md font-bold text-on-background truncate hover:underline"
                    >
                        {i.name}
                    </a>
                    <p class="text-[11px] text-on-surface-variant truncate">{meta}</p>
                </div>
                <span class=chip>{i.kind_label}</span>
            </div>

            <div class="bg-surface-container rounded-xl px-3 py-2 space-y-0.5">
                <p class="text-body-sm text-on-background flex items-center gap-1.5">
                    <span class="material-symbols-outlined text-[15px]">"calendar_month"</span>
                    {i.range_label}
                </p>
                {(!i.jam_label.is_empty())
                    .then(|| {
                        view! {
                            <p class="text-[11px] text-on-surface-variant flex items-center gap-1.5">
                                <span class="material-symbols-outlined text-[14px]">"schedule"</span>
                                {i.jam_label}
                            </p>
                        }
                    })}
                <p class="text-[11px] font-semibold text-primary">
                    {i.sampai_label} " · " {hari}
                </p>
            </div>

            {(!i.reason.trim().is_empty())
                .then(|| {
                    view! {
                        <p class="text-[11px] text-on-surface-variant break-words">{i.reason}</p>
                    }
                })}
        </div>
    }
}
