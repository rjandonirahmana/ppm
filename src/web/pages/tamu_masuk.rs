//! web/pages/tamu_masuk.rs — Tinjau Tamu (/tamu-masuk), peran PENJAGA.
//!
//! Buku tamu sudah berjalan sejak migrasi 35: tamu mengisi /tamu, dapat kode 6
//! digit, lalu mengetiknya di mesin gerbang yang memotret wajahnya. Baris yang
//! lahir dari situ TAK PERNAH DIBACA siapa pun — datanya terkumpul rapi selama
//! ini dan tak ada satu layar pun yang menampilkannya.
//!
//! Halaman ini pembacanya. Pekerjaan penjaga cuma satu: melihat wajah yang
//! terpotret di sebelah nama dan keperluan yang diketik, lalu menyatakan cocok
//! atau menuliskan apa yang janggal.
//!
//! KENAPA TAK ADA TOMBOL "TOLAK". Tamunya sudah masuk — fotonya bukti ia sudah
//! berdiri di gerbang. Yang bisa dilakukan penjaga bukan membatalkan kejadian,
//! melainkan MENCATAT bahwa datanya tak cocok supaya pengurus menindaklanjuti.
//! Tombol yang menjanjikan pembatalan akan berbohong.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::TamuMasukItem;
use crate::web::api::{periksa_tamu_action, tamu_masuk_data};
use crate::web::components::{
    kartu_grid, DeviceFrame, EmptyState, FetchError, FlashMsg, MediaFrame, MobileHeader,
};

#[component]
pub fn TamuMasukPage() -> impl IntoView {
    // Bawaannya HANYA yang belum diperiksa: itulah pekerjaan yang menunggu.
    // Riwayat lengkap ada di balik satu ketukan, bukan sebaliknya.
    let hanya_belum = RwSignal::new(true);
    let data = Resource::new(
        move || hanya_belum.get(),
        |belum| async move { tamu_masuk_data(belum).await },
    );

    crate::web::components::guard_sesi(data);

    let msg = RwSignal::new(Option::<(bool, String)>::None);

    view! {
        <Title text="Tinjau Tamu — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Tinjau Tamu" subtitle="Cocokkan data tamu dengan wajahnya" />

                <div class="px-5 pt-5 space-y-4 stagger">
                    <FlashMsg pesan=msg />

                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="h-20 bg-surface-container rounded-2xl"></div>
                                <div class="grid gap-3 md:grid-cols-2">
                                    <div class="h-64 bg-surface-container rounded-2xl"></div>
                                    <div class="h-64 bg-surface-container rounded-2xl hidden md:block"></div>
                                </div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                    Ok(d) => {
                                        let kosong = d.items.is_empty();
                                        view! {
                                            <div class="ppm-card p-4 flex items-center justify-between gap-3">
                                                <div>
                                                    <div class="flex items-center gap-2 text-warning">
                                                        <span class="material-symbols-outlined">
                                                            "pending_actions"
                                                        </span>
                                                        <span class="text-2xl font-bold text-on-background">
                                                            {d.belum_diperiksa}
                                                        </span>
                                                    </div>
                                                    <p class="text-body-sm text-on-surface-variant mt-1">
                                                        "Menunggu diperiksa"
                                                    </p>
                                                </div>
                                                // Tombol, bukan tab: hanya ada dua keadaan, dan
                                                // yang satu jelas lebih sering dipakai.
                                                <button
                                                    class="px-4 py-2.5 rounded-xl border border-outline-variant text-body-sm font-semibold text-on-surface hover:border-primary hover:text-primary transition-colors cursor-pointer"
                                                    on:click=move |_| hanya_belum.update(|b| *b = !*b)
                                                >
                                                    {move || {
                                                        if hanya_belum.get() {
                                                            "Lihat semua"
                                                        } else {
                                                            "Yang belum saja"
                                                        }
                                                    }}
                                                </button>
                                            </div>

                                            {if kosong {
                                                view! {
                                                    <EmptyState
                                                        icon="task_alt"
                                                        title="Tidak ada tamu menunggu"
                                                        subtitle="Semua kunjungan yang tercatat sudah diperiksa."
                                                    />
                                                }
                                                    .into_any()
                                            } else {
                                                kartu_grid(
                                                        d.items
                                                            .into_iter()
                                                            .map(|t| {
                                                                view! {
                                                                    <KartuTamu t=t msg=msg refetch=move || data.refetch() />
                                                                }
                                                                    .into_any()
                                                            })
                                                            .collect::<Vec<_>>(),
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
fn KartuTamu(
    t: TamuMasukItem,
    msg: RwSignal<Option<(bool, String)>>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let id = t.id;
    let busy = RwSignal::new(false);
    let catatan = RwSignal::new(String::new());
    let sudah = t.diperiksa;
    let ada_foto = !t.face_url.is_empty();

    let kirim = move |c: String| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        leptos::task::spawn_local(async move {
            match periksa_tamu_action(id, c).await {
                Ok(_) => msg.set(Some((true, "Tercatat. Terima kasih.".into()))),
                Err(e) => msg.set(Some((false, crate::web::components::pesan_galat(e)))),
            }
            busy.set(false);
            refetch();
        });
    };

    view! {
        <div class="ppm-card p-4 space-y-3 anim-in">
            // Wajah lebih dulu, besar. Inilah yang dibandingkan penjaga dengan
            // orang di depannya; menaruhnya sebagai lampiran kecil di bawah
            // membalik urutan pekerjaannya.
            {if ada_foto {
                view! {
                    <MediaFrame
                        src=t.face_url.clone()
                        style="width:100%;height:100%;object-fit:cover".to_string()
                        video=false
                        backdrop=false
                        alt=format!("Wajah tamu {}", t.name)
                        class="rounded-xl aspect-[4/3] bg-surface-container"
                        lazy=true
                    />
                }
                    .into_any()
            } else {
                // Tanpa foto justru baris yang PALING perlu diperiksa — mesin
                // tak sempat memotret, jadi tak ada bukti siapa yang masuk.
                view! {
                    <div class="rounded-xl aspect-[4/3] bg-warning/10 flex flex-col items-center justify-center gap-1 text-warning">
                        <span class="material-symbols-outlined text-3xl">"no_photography"</span>
                        <p class="text-body-sm font-semibold">"Tanpa foto wajah"</p>
                        <p class="text-[11px] text-on-surface-variant px-4 text-center">
                            "Mesin gerbang tak sempat memotret. Pastikan langsung ke orangnya."
                        </p>
                    </div>
                }
                    .into_any()
            }}

            <div class="min-w-0">
                <p class="text-body-lg font-bold text-on-background truncate">{t.name}</p>
                <p class="text-body-sm text-on-surface-variant">{t.phone}</p>
                {(!t.purpose.trim().is_empty())
                    .then(|| {
                        view! {
                            <p class="text-body-sm text-on-surface mt-1">
                                "Keperluan: "
                                {t.purpose.clone()}
                            </p>
                        }
                    })}
                <p class="text-[11px] text-on-surface-variant mt-1">{t.waktu_label}</p>
            </div>

            {if sudah {
                let oleh = t.diperiksa_oleh.clone();
                let cat = t.catatan.clone();
                view! {
                    <div class="rounded-xl p-3 bg-surface-container">
                        <p class="text-body-sm font-semibold text-on-background">
                            {if cat.trim().is_empty() { "Data cocok" } else { "Ada catatan" }}
                        </p>
                        {(!cat.trim().is_empty())
                            .then(|| {
                                view! { <p class="text-body-sm text-error mt-1">{cat.clone()}</p> }
                            })}
                        {(!oleh.trim().is_empty())
                            .then(|| {
                                view! {
                                    <p class="text-[11px] text-on-surface-variant mt-1">
                                        "Diperiksa " {oleh.clone()}
                                    </p>
                                }
                            })}
                    </div>
                }
                    .into_any()
            } else {
                view! {
                    <div class="space-y-2">
                        <textarea
                            class="w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface"
                            rows="2"
                            placeholder="Kosongkan bila data cocok. Isi bila ada yang janggal."
                            prop:value=move || catatan.get()
                            on:input=move |ev| catatan.set(event_target_value(&ev))
                            aria-label="Catatan kejanggalan"
                        ></textarea>
                        <div class="grid grid-cols-2 gap-2">
                            <button
                                class="py-2.5 rounded-xl border border-outline-variant text-body-sm font-semibold text-error hover:border-error transition-colors cursor-pointer press disabled:opacity-60"
                                prop:disabled=move || busy.get() || catatan.get().trim().is_empty()
                                on:click=move |_| kirim(catatan.get_untracked())
                            >
                                {move || if busy.get() { "Menyimpan…" } else { "Ada yang janggal" }}
                            </button>
                            <button
                                class="py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-sm cursor-pointer press disabled:opacity-60"
                                prop:disabled=move || busy.get()
                                on:click=move |_| kirim(String::new())
                            >
                                {move || if busy.get() { "Menyimpan…" } else { "Data cocok" }}
                            </button>
                        </div>
                        <p class="text-[11px] text-on-surface-variant">
                            "Tombol kiri aktif setelah catatan diisi — catatan kosong berarti data cocok."
                        </p>
                    </div>
                }
                    .into_any()
            }}
        </div>
    }
}
