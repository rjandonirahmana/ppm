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
    let code = RwSignal::new(String::new());
    let tick = RwSignal::new(0u32);
    let submitting = RwSignal::new(false);
    let err = RwSignal::new(String::new());

    // Polling status check-in: fetch saat `code` terisi, ulang tiap `tick`.
    let status = Resource::new(
        move || (code.get(), tick.get()),
        |(c, _)| async move {
            if c.is_empty() {
                Ok::<Option<GuestCheckin>, ServerFnError>(None)
            } else {
                guest_status_action(c).await
            }
        },
    );
    let checkin = move || status.get().and_then(|r| r.ok()).flatten();

    // Interval polling (WASM) — mulai saat kode ada, berhenti saat sudah ✅.
    let interval_id = StoredValue::new(None::<i32>);
    #[cfg(target_arch = "wasm32")]
    Effect::new(move |_| {
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;
        let has_code = !code.get().is_empty();
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
                cb.forget();
            }
        }
        if done {
            if let (Some(id), Some(w)) = (interval_id.get_value(), win) {
                w.clear_interval_with_handle(id);
                interval_id.set_value(None);
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
                Ok(c) => code.set(c),
                Err(e) => err.set(e.to_string()),
            }
            submitting.set(false);
        });
    };

    let field = "w-full bg-surface-container border-0 rounded-xl px-4 py-3 text-body-md text-on-surface";

    view! {
        <Title text="Buku Tamu — PPM AFM" />
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
                    } else if !code.get().is_empty() {
                        // ── STATE 2: tampilkan kode + tunggu mesin ────────────
                        view! {
                            <div class="ppm-card p-6 text-center space-y-4 anim-in">
                                <p class="text-body-sm text-on-surface-variant">"Ketik kode ini di mesin, lalu tatap kamera:"</p>
                                <p class="text-[44px] leading-none font-bold tracking-[0.3em] text-primary">
                                    {move || code.get()}
                                </p>
                                <div class="flex items-center justify-center gap-2 text-on-surface-variant">
                                    <span class="material-symbols-outlined animate-spin text-[18px]">"autorenew"</span>
                                    <span class="text-body-sm">"Menunggu konfirmasi mesin…"</span>
                                </div>
                                <p class="text-[11px] text-on-surface-variant/70">"Kode berlaku hari ini. Jangan tutup halaman ini."</p>
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
