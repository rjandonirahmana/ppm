//! web/pages/ortu_pembayaran.rs — Pembayaran sisi ORANG TUA (/orang-tua/pembayaran).
//!
//! Yang dikerjakan orang tua di sini cuma satu: mengirim jumlah yang ditransfer
//! + fotonya. Ia TIDAK menentukan periode berlakunya — itu ditetapkan pengurus
//! keuangan setelah bukti dicocokkan dengan mutasi rekening pondok. Membiarkan
//! penyetor mengisi sendiri "berlaku sampai kapan" berarti angka yang tak
//! pernah diperiksa siapa pun ikut menentukan siapa yang dianggap menunggak.
//!
//! Form pengajuannya SATU komponen dengan layar santri (`FormAjukanBayar`) —
//! isinya memang sama, dan menyalinnya ke dua halaman membuat keduanya
//! menyimpang begitu ada satu isian ditambahkan.
//!
//! DUA PERAN CHIP ANAK. Satu transfer sering menutup BEBERAPA anak sekaligus,
//! jadi pemilihan "siapa yang dibayarkan" ada di dalam form (daftar centang +
//! nominal per anak). Chip di halaman ini tinggal mengatur satu hal lain:
//! riwayat siapa yang sedang ditampilkan di bawah. Keduanya sengaja dipisah —
//! menggabungkannya berarti melihat riwayat anak kedua diam-diam mengubah
//! kepada siapa bukti transfer yang sudah diisi akan dikirim.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::web::api::{parent_home, student_bills_data};
use crate::web::components::{DeviceFrame, EmptyState, FetchError, MobileHeader};
use crate::web::pages::tagihan::{FormAjukanBayar, RiwayatBayarList};

#[component]
pub fn OrtuPembayaranPage() -> impl IntoView {
    let home = Resource::new(|| (), |_| async move { parent_home(None).await });
    // Anak yang sedang dilihat. None sampai daftar anak termuat — Effect di
    // bawah memilih anak pertama begitu datanya ada.
    let anak = RwSignal::new(Option::<i64>::None);

    Effect::new(move |_| {
        if let Some(Ok(h)) = home.get() {
            if anak.get_untracked().is_none() {
                if let Some(first) = h.children.first() {
                    anak.set(Some(first.id));
                }
            }
        }
    });

    crate::web::components::guard_sesi(home);

    // Kunci resource-nya id anak: berganti anak = daftar yang lain sama sekali,
    // bukan penyaringan atas daftar yang sudah ada.
    let bills = Resource::new(
        move || anak.get().unwrap_or(0),
        |id| async move {
            if id == 0 {
                Ok(Vec::new())
            } else {
                student_bills_data(id).await
            }
        },
    );
    let refetch = move || bills.refetch();

    view! {
        <Title text="Pembayaran — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide ppm-content">
                <MobileHeader
                    title="Pembayaran"
                    subtitle="Kirim bukti transfer & pantau statusnya"
                    back_href="/orang-tua"
                />
                <div class="px-5 pt-5 space-y-4">
                    // ── Pilih anak ───────────────────────────────────────
                    <Suspense fallback=|| {
                        view! { <div class="h-10 bg-surface-container rounded-xl animate-pulse"></div> }
                    }>
                        {move || {
                            home.get()
                                .map(|res| match res {
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                    Ok(h) if h.children.is_empty() => {
                                        view! {
                                            <EmptyState
                                                icon="link_off"
                                                title="Belum terhubung dengan santri"
                                                subtitle="Hubungkan akun Anda dengan putra/putri di halaman Beranda lebih dulu."
                                            />
                                        }
                                            .into_any()
                                    }
                                    Ok(h) => {
                                        // Chip disembunyikan bila anaknya cuma satu:
                                        // satu tombol yang tak bisa tidak terpilih
                                        // hanya menambah baris tanpa menawarkan pilihan.
                                        let banyak = h.children.len() > 1;
                                        view! {
                                            {banyak
                                                .then(|| {
                                                    view! {
                                                        <p class="text-[11px] text-on-surface-variant">
                                                            "Riwayat pembayaran yang ditampilkan:"
                                                        </p>
                                                        <div class="flex gap-2 overflow-x-auto pb-1">
                                                            {h
                                                                .children
                                                                .iter()
                                                                .map(|c| {
                                                                    let id = c.id;
                                                                    let nama = c.name.clone();
                                                                    view! {
                                                                        <button
                                                                            class=move || {
                                                                                if anak.get() == Some(id) {
                                                                                    "shrink-0 px-3.5 py-2 rounded-full bg-primary text-on-primary text-body-sm font-semibold press cursor-pointer"
                                                                                } else {
                                                                                    "shrink-0 px-3.5 py-2 rounded-full bg-surface-container text-on-surface-variant text-body-sm font-semibold press cursor-pointer"
                                                                                }
                                                                            }
                                                                            aria-pressed=move || (anak.get() == Some(id)).to_string()
                                                                            on:click=move |_| anak.set(Some(id))
                                                                        >
                                                                            {nama.clone()}
                                                                        </button>
                                                                    }
                                                                })
                                                                .collect_view()}
                                                        </div>
                                                    }
                                                })}
                                        }
                                            .into_any()
                                    }
                                })
                        }}
                    </Suspense>

                    // Form menerima SELURUH daftar anak, bukan yang sedang
                    // dilihat riwayatnya: satu transfer boleh menutup beberapa
                    // anak, dan form-lah yang menawarkan pilihannya.
                    <Show
                        when=move || {
                            home.get()
                                .and_then(|r| r.ok())
                                .map(|h| !h.children.is_empty())
                                .unwrap_or(false)
                        }
                        fallback=|| ()
                    >
                        <FormAjukanBayar
                            anak=Signal::derive(move || {
                                home.get()
                                    .and_then(|r| r.ok())
                                    .map(|h| h.children)
                                    .unwrap_or_default()
                            })
                            refetch=refetch
                        />
                    </Show>

                    <Suspense fallback=|| {
                        view! { <div class="h-20 bg-surface-container rounded-2xl animate-pulse"></div> }
                    }>
                        {move || {
                            bills
                                .get()
                                .map(|res| match res {
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                    Ok(list) => view! { <RiwayatBayarList list=list /> }.into_any(),
                                })
                        }}
                    </Suspense>
                </div>
            </div>
        </DeviceFrame>
    }
}
