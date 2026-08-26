//! web/pages/tamu.rs — Buku Tamu publik (/tamu, migrasi 35).
//!
//! Alur: isi nama/HP/keperluan → dapat KODE 6-digit → ketik kode di mesin IoT
//! (mesin ambil wajah) → halaman ini polling status → tampil ✅ saat berhasil.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::GuestCheckin;
use crate::web::api::{guest_status_action, register_guest_action};

#[component]
pub fn TamuPage() -> impl IntoView {
    let f_name = RwSignal::new(String::new());
    let f_phone = RwSignal::new(String::new());
    let f_purpose = RwSignal::new(String::new());
    // TIKET polling, bukan kode check-in — kodenya tak pernah sampai ke browser
    // ini, dan itu yang membuat nomor HP-nya terbukti (lihat
    // `models::GuestTicket`).
    let tiket = RwSignal::new(String::new());
    // Nomor tujuan yang sudah disamarkan, untuk ditampilkan.
    let tujuan = RwSignal::new(String::new());
    // false = yang dikirim kode LAMA yang masih berlaku (mendaftar dua kali
    // dengan nomor yang sama).
    let kode_baru = RwSignal::new(true);
    let tick = RwSignal::new(0u32);
    let submitting = RwSignal::new(false);
    let err = RwSignal::new(String::new());

    // Polling status check-in: fetch saat tiket terisi, ulang tiap `tick`.
    let status = Resource::new(
        move || (tiket.get(), tick.get()),
        |(t, _)| async move {
            if t.is_empty() {
                Ok::<Option<GuestCheckin>, ServerFnError>(None)
            } else {
                guest_status_action(t).await
            }
        },
    );

    // HASILNYA DISALIN KE SIGNAL LEWAT EFFECT, tidak dibaca langsung di view.
    //
    // Membaca resource di luar <Suspense/> membuat Leptos memperingatkan
    // hydration mismatch — dan peringatannya benar: bila server dan klien
    // menyimpulkan keadaan yang berbeda, DOM hasil hidrasi tak lagi sepadan
    // dengan yang di-render server, dan halaman ini justru yang paling rugi
    // (tombolnya berhenti bekerja di tengah antrean gerbang).
    //
    // Membungkusnya dengan <Suspense/> bukan jawaban yang tepat di sini:
    // halaman ini mesin tiga keadaan yang DUA di antaranya (formulir dan
    // "menunggu mesin") tak bergantung pada resource sama sekali, dan
    // fallback-nya hanya akan mengedipkan seluruh halaman tiap polling.
    //
    // Effect hanya jalan di KLIEN pasca-hidrasi, jadi server selalu merender
    // keadaan 1 (formulir) — yang memang satu-satunya keadaan benar di sana,
    // karena saat SSR belum ada tiket apa pun. Pola sama `pages/galeri.rs`.
    let checkin_data = RwSignal::new(None::<GuestCheckin>);
    Effect::new(move |_| {
        if let Some(Ok(Some(c))) = status.get() {
            checkin_data.set(Some(c));
        }
    });
    let checkin = move || checkin_data.get();

    // Interval polling (WASM) — mulai saat kode ada, berhenti saat sudah ✅.
    // Hanya dibaca oleh Effect di bawah yang khusus wasm; di build SSR memang
    // tak terpakai (lihat catatan serupa di pages/tagihan.rs).
    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
    let interval_id = StoredValue::new(None::<i32>);
    // Pemegang closure interval. Tanpa ini closure harus di-`forget()` (bocor
    // permanen); SendWrapper dipakai karena Closure bukan Send sementara
    // StoredValue menuntutnya — aman, semuanya di thread browser yang sama.
    // Hanya ada di build WASM: `send_wrapper` memang dependensi khusus wasm.
    #[cfg(target_arch = "wasm32")]
    let held = StoredValue::new(
        None::<send_wrapper::SendWrapper<wasm_bindgen::closure::Closure<dyn FnMut()>>>,
    );
    #[cfg(target_arch = "wasm32")]
    Effect::new(move |_| {
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;
        let has_code = !tiket.get().is_empty();
        let done = checkin().is_some();
        let win = web_sys::window();
        if has_code && !done && interval_id.get_value().is_none() {
            if let Some(w) = win.clone() {
                let cb = Closure::<dyn FnMut()>::new(move || tick.update(|t| *t += 1));
                if let Ok(id) = w.set_interval_with_callback_and_timeout_and_arguments_0(
                    cb.as_ref().unchecked_ref(),
                    2500,
                ) {
                    interval_id.set_value(Some(id));
                }
                // Closure DI-HOLD, bukan di-forget: `forget()` melepasnya ke
                // heap selamanya, dan Effect ini bisa jalan berkali-kali
                // (tick berubah tiap 2,5 detik) sehingga satu closure bocor
                // tiap kali. Disimpan lalu dibuang bersama interval-nya.
                held.set_value(Some(send_wrapper::SendWrapper::new(cb)));
            }
        }
        if done {
            if let (Some(id), Some(w)) = (interval_id.get_value(), win) {
                w.clear_interval_with_handle(id);
                interval_id.set_value(None);
                // Interval dihentikan → closure-nya ikut dibuang.
                held.set_value(None);
            }
        }
    });

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get_untracked() {
            return;
        }
        let (n, p, k) = (
            f_name.get_untracked(),
            f_phone.get_untracked(),
            f_purpose.get_untracked(),
        );
        if n.trim().is_empty() || p.trim().len() < 6 {
            err.set("Isi nama dan nomor HP yang benar.".into());
            return;
        }
        submitting.set(true);
        err.set(String::new());
        leptos::task::spawn_local(async move {
            match register_guest_action(n, p, k).await {
                Ok(hasil) => {
                    tujuan.set(hasil.tujuan);
                    kode_baru.set(hasil.kode_baru);
                    tiket.set(hasil.ticket);
                }
                // Galatnya DITAMPILKAN, termasuk "kode gagal dikirim ke
                // WhatsApp": di alur ini kegagalan kirim berarti tamu tak punya
                // kode sama sekali, jadi ia tak boleh berakhir sebagai layar
                // "menunggu mesin" yang tak akan pernah berubah.
                Err(e) => err.set(crate::web::components::pesan_galat(&e.to_string())),
            }
            submitting.set(false);
        });
    };

    let field = "w-full bg-surface-container border-0 rounded-xl px-4 py-3 text-body-md text-on-surface";

    view! {
        <Title text="Buku Tamu — AFM SMART" />
        <div class="min-h-screen bg-surface text-on-surface flex flex-col items-center px-5 py-10">
            <div class="w-full max-w-md">
                <header class="flex items-center gap-3 mb-8">
                    <div class="w-12 h-12 spiritual-gradient rounded-xl flex items-center justify-center">
                        <span class="material-symbols-outlined text-on-primary text-3xl">"mosque"</span>
                    </div>
                    <div>
                        <h1 class="text-display-md text-primary">"Buku Tamu"</h1>
                        <p class="text-body-sm text-on-surface-variant">"PPM Al-Faqih Mandiri"</p>
                    </div>
                </header>

                {move || {
                    // ── STATE 3: sudah check-in (✅) ───────────────────────────
                    if let Some(c) = checkin() {
                        view! {
                            <div class="ppm-card p-6 text-center space-y-4 anim-in">
                                <span class="material-symbols-outlined text-success text-6xl">"check_circle"</span>
                                <h2 class="text-headline-sm font-bold text-on-background">"Berhasil!"</h2>
                                <p class="text-body-md text-on-surface-variant">
                                    "Selamat datang, " <span class="font-semibold text-on-background">{c.name.clone()}</span>". Kehadiran Anda tercatat."
                                </p>
                                {(!c.face_url.is_empty())
                                    .then(|| view! {
                                        <img
                                            src=c.face_url.clone()
                                            alt="Foto kehadiran"
                                            class="w-32 h-32 object-cover rounded-2xl mx-auto border border-outline-variant"
                                        />
                                    })}
                            </div>
                        }
                            .into_any()
                    } else if !tiket.get().is_empty() {
                        // ── STATE 2: kode ada di WhatsApp, tunggu mesin ───────
                        //
                        // Kodenya SENGAJA tak ditampilkan di sini. Halaman ini
                        // bisa terbuka di HP siapa saja; yang membuktikan tamu
                        // memegang nomor yang ia tulis justru bahwa kodenya
                        // hanya ada di WhatsApp nomor itu.
                        view! {
                            <div class="ppm-card p-6 text-center space-y-4 anim-in">
                                <span class="material-symbols-outlined text-primary text-5xl">"forum"</span>
                                <h2 class="text-headline-sm font-bold text-on-background">
                                    {move || {
                                        if kode_baru.get() {
                                            "Kode dikirim ke WhatsApp"
                                        } else {
                                            "Kode sebelumnya masih berlaku"
                                        }
                                    }}
                                </h2>
                                <p class="text-body-md text-on-surface-variant">
                                    "Buka WhatsApp "
                                    <span class="font-semibold text-on-background">
                                        {move || tujuan.get()}
                                    </span>
                                    ", lalu ketik kodenya di mesin buku tamu di gerbang dan tatap kamera."
                                </p>
                                <div class="flex items-center justify-center gap-2 text-on-surface-variant">
                                    <span class="material-symbols-outlined animate-spin text-[18px]">"autorenew"</span>
                                    <span class="text-body-sm">"Menunggu konfirmasi mesin…"</span>
                                </div>
                                <p class="text-[11px] text-on-surface-variant/70">
                                    "Kode berlaku hari ini. Belum menerima pesan? Pastikan nomornya benar dan aktif di WhatsApp."
                                </p>
                            </div>
                        }
                            .into_any()
                    } else {
                        // ── STATE 1: form data diri ───────────────────────────
                        view! {
                            <form class="ppm-card p-6 space-y-4" on:submit=submit>
                                {move || {
                                    (!err.get().is_empty())
                                        .then(|| view! {
                                            <div class="p-3 bg-error-container text-on-error-container rounded-xl text-body-sm">
                                                {move || err.get()}
                                            </div>
                                        })
                                }}
                                <div class="space-y-1.5">
                                    <label class="text-body-sm font-medium text-on-surface-variant">"Nama lengkap"</label>
                                    <input type="text" class=field placeholder="Nama Anda"
                                        prop:value=move || f_name.get()
                                        on:input=move |e| f_name.set(event_target_value(&e)) />
                                </div>
                                <div class="space-y-1.5">
                                    <label class="text-body-sm font-medium text-on-surface-variant">"Nomor HP"</label>
                                    <input type="tel" class=field placeholder="08xxxxxxxxxx"
                                        prop:value=move || f_phone.get()
                                        on:input=move |e| f_phone.set(event_target_value(&e)) />
                                </div>
                                <div class="space-y-1.5">
                                    <label class="text-body-sm font-medium text-on-surface-variant">"Keperluan"</label>
                                    <textarea class=field placeholder="cth. Bertemu pengurus, mengantar barang…"
                                        prop:value=move || f_purpose.get()
                                        on:input=move |e| f_purpose.set(event_target_value(&e)) />
                                </div>
                                <button type="submit"
                                    class="w-full py-3.5 spiritual-gradient text-on-primary rounded-xl font-bold text-body-md press disabled:opacity-60"
                                    disabled=move || submitting.get()>
                                    {move || if submitting.get() { "Memproses…" } else { "Dapatkan Kode" }}
                                </button>
                            </form>
                        }
                            .into_any()
                    }
                }}
            </div>
        </div>
    }
}
