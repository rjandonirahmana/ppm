//! web/pages/register.rs — Registrasi mandiri dgn KODE REFERAL dari admin.
//! HANYA bisa dgn kode yang dibuat admin (User Control → create_invite_action):
//! disimpan di Redis dgn peran + TTL 24 JAM, dan SEKALI PAKAI (dihapus setelah
//! akun jadi — lihat service::registration::verify_register). Alur 3 langkah:
//!   1) masukkan kode referal (atau via link ?key=…) → validasi + tampil peran;
//!   2) isi nama+HP → OTP+password dikirim WhatsApp;
//!   3) masukkan OTP → akun dibuat + login.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_query_map;

use crate::web::api::{register_action, resend_otp_action, validate_invite_action, verify_register_action};

#[component]
pub fn RegisterPage() -> impl IntoView {
    let query = use_query_map();
    // Kode referal: prefill dari ?key= (link), tetap boleh diketik manual.
    let code = RwSignal::new(query.read_untracked().get("key").unwrap_or_default());
    // "code" (masukkan kode) | "form" (nama+HP) | "otp" (kode OTP)
    let stage = RwSignal::new("code".to_string());
    let role_label = RwSignal::new(String::new());
    // Peran undangan = santri? → form minta profil mahasiswa (migrasi 47).
    let need_student = RwSignal::new(false);
    let name = RwSignal::new(String::new());
    let phone = RwSignal::new(String::new());
    // Profil mahasiswa — hanya dipakai bila `need_student`.
    let gender = RwSignal::new(String::new());
    let campus = RwSignal::new(String::new());
    let major = RwSignal::new(String::new());
    let entry_year = RwSignal::new(String::new());
    let otp = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);
    let info = RwSignal::new(Option::<String>::None);

    let strip = |e: ServerFnError| {
        let s = e.to_string();
        s.rsplit(": ").next().unwrap_or(&s).to_string()
    };

    // Validasi kode referal → tampil peran, lanjut ke form.
    let do_validate = move || {
        if busy.get_untracked() {
            return;
        }
        let k = code.get_untracked().trim().to_string();
        if k.is_empty() {
            error.set(Some("Masukkan kode referal dari admin.".into()));
            return;
        }
        busy.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            match validate_invite_action(k).await {
                Ok(info) => {
                    role_label.set(info.role_label);
                    need_student.set(info.needs_student_profile);
                    stage.set("form".into());
                }
                Err(e) => error.set(Some(strip(e))),
            }
            busy.set(false);
        });
    };

    // Auto-validasi (klien saja) bila URL sudah membawa ?key=.
    Effect::new(move |prev: Option<()>| {
        if prev.is_none() && !code.get_untracked().trim().is_empty() {
            do_validate();
        }
    });

    let submit_code = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        do_validate();
    };

    let submit_form = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        let (k, n, p) = (code.get_untracked(), name.get_untracked(), phone.get_untracked());
        if n.trim().is_empty() || p.trim().is_empty() {
            error.set(Some("Nama & nomor WhatsApp wajib diisi.".into()));
            return;
        }
        let (g, c, m, y) = (
            gender.get_untracked(),
            campus.get_untracked(),
            major.get_untracked(),
            entry_year.get_untracked(),
        );
        // Cek di klien supaya salah isi ketahuan tanpa menunggu server; server
        // tetap memvalidasi ulang (jangan percaya klien).
        if need_student.get_untracked() {
            if g.is_empty() {
                error.set(Some("Pilih jenis kelamin.".into()));
                return;
            }
            if c.trim().is_empty() || m.trim().is_empty() || y.trim().is_empty() {
                error.set(Some("Kampus, jurusan & tahun masuk PPM wajib diisi.".into()));
                return;
            }
        }
        busy.set(true);
        error.set(None);
        info.set(None);
        leptos::task::spawn_local(async move {
            match register_action(k, n, p, g, c, m, y).await {
                Ok(_) => {
                    stage.set("otp".into());
                    info.set(Some("Kode OTP & password dikirim ke WhatsApp kamu.".into()));
                }
                Err(e) => error.set(Some(strip(e))),
            }
            busy.set(false);
        });
    };

    let submit_otp = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        let (k, p, o) = (code.get_untracked(), phone.get_untracked(), otp.get_untracked());
        if o.trim().is_empty() {
            error.set(Some("Masukkan kode OTP.".into()));
            return;
        }
        busy.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            match verify_register_action(k, p, o).await {
                Ok(path) => {
                    #[cfg(target_arch = "wasm32")]
                    if let Some(w) = web_sys::window() {
                        let _ = w.location().replace(&path);
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    let _ = path;
                }
                Err(e) => {
                    error.set(Some(strip(e)));
                    busy.set(false);
                }
            }
        });
    };

    let resend = move |_| {
        if busy.get_untracked() {
            return;
        }
        let (k, n, p) = (code.get_untracked(), name.get_untracked(), phone.get_untracked());
        let (g, c, m, y) = (
            gender.get_untracked(),
            campus.get_untracked(),
            major.get_untracked(),
            entry_year.get_untracked(),
        );
        busy.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            match resend_otp_action(k, n, p, g, c, m, y).await {
                Ok(_) => info.set(Some("OTP dikirim ulang ke WhatsApp.".into())),
                Err(e) => error.set(Some(strip(e))),
            }
            busy.set(false);
        });
    };

    let field = "w-full bg-surface-container border-0 rounded-xl px-4 py-3 text-body-md text-on-surface";
    view! {
        <Title text="Registrasi — PPM AFM" />
        <div class="min-h-screen flex items-center justify-center p-4 bg-surface">
            <div class="fixed inset-0 z-0"><div class="absolute inset-0 pattern-bg"></div></div>
            <main class="relative z-10 w-full max-w-md bg-surface-container-lowest rounded-2xl shadow-2xl overflow-hidden anim-in">
                <div class="spiritual-gradient p-6 text-on-primary">
                    <div class="flex items-center gap-3">
                        <div class="w-12 h-12 bg-primary-fixed rounded-xl flex items-center justify-center">
                            <span class="material-symbols-outlined text-primary text-3xl">"how_to_reg"</span>
                        </div>
                        <div>
                            <h1 class="text-headline-sm leading-tight">"Registrasi Akun"</h1>
                            <p class="text-body-sm opacity-85">"PPM Al-Faqih Mandiri"</p>
                        </div>
                    </div>
                </div>

                <div class="p-6 space-y-4">
                    // Peran (setelah kode valid).
                    {move || {
                        let l = role_label.get();
                        (!l.is_empty())
                            .then(|| {
                                view! {
                                    <div class="flex items-center gap-2 text-body-sm">
                                        <span class="text-on-surface-variant">"Mendaftar sebagai"</span>
                                        <span class="px-2.5 py-1 rounded-full bg-secondary-container text-primary font-bold text-[11px] tracking-wider uppercase">
                                            {l}
                                        </span>
                                    </div>
                                }
                            })
                    }}

                    {move || {
                        error
                            .get()
                            .map(|e| {
                                view! {
                                    <div class="p-2.5 bg-error-container text-on-error-container rounded-lg text-body-sm">
                                        {e}
                                    </div>
                                }
                            })
                    }}
                    {move || {
                        info.get()
                            .map(|t| {
                                view! {
                                    <div class="p-2.5 bg-secondary-container text-on-secondary-container rounded-lg text-body-sm">
                                        {t}
                                    </div>
                                }
                            })
                    }}

                    {move || {
                        match stage.get().as_str() {
                            // ── Langkah 1: kode referal ──────────────────────
                            "code" => {
                                view! {
                                    <form class="space-y-3" method="post" on:submit=submit_code>
                                        <label class="space-y-1 block">
                                            <span class="text-label-md text-on-surface-variant">
                                                "Kode referal dari admin"
                                            </span>
                                            <input
                                                type="text"
                                                class=field
                                                placeholder="Tempel kode di sini"
                                                prop:value=move || code.get()
                                                on:input=move |ev| code.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <p class="text-[11px] text-on-surface-variant">
                                            "Registrasi hanya bisa dengan kode dari admin (berlaku 24 jam, sekali pakai)."
                                        </p>
                                        <button
                                            type="submit"
                                            class="w-full py-3 rounded-xl bg-primary text-on-primary font-bold press disabled:opacity-60"
                                            disabled=move || busy.get()
                                        >
                                            {move || if busy.get() { "Memeriksa…" } else { "Lanjut" }}
                                        </button>
                                    </form>
                                }
                                    .into_any()
                            }
                            // ── Langkah 3: OTP ───────────────────────────────
                            "otp" => {
                                view! {
                                    <form class="space-y-3" method="post" on:submit=submit_otp>
                                        <label class="space-y-1 block">
                                            <span class="text-label-md text-on-surface-variant">
                                                "Kode OTP (6 digit dari WhatsApp)"
                                            </span>
                                            <input
                                                type="text"
                                                inputmode="numeric"
                                                class=field
                                                placeholder="123456"
                                                prop:value=move || otp.get()
                                                on:input=move |ev| otp.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <button
                                            type="submit"
                                            class="w-full py-3 rounded-xl bg-primary text-on-primary font-bold press disabled:opacity-60"
                                            disabled=move || busy.get()
                                        >
                                            {move || {
                                                if busy.get() { "Memproses…" } else { "Verifikasi & Buat Akun" }
                                            }}
                                        </button>
                                        <button
                                            type="button"
                                            class="w-full text-body-sm text-primary font-semibold disabled:opacity-50"
                                            disabled=move || busy.get()
                                            on:click=resend
                                        >
                                            "Kirim ulang OTP"
                                        </button>
                                    </form>
                                }
                                    .into_any()
                            }
                            // ── Langkah 2: nama + HP ─────────────────────────
                            _ => {
                                view! {
                                    <form class="space-y-3" method="post" on:submit=submit_form>
                                        <label class="space-y-1 block">
                                            <span class="text-label-md text-on-surface-variant">"Nama lengkap"</span>
                                            <input
                                                type="text"
                                                class=field
                                                placeholder="mis. Muhammad Rizky"
                                                prop:value=move || name.get()
                                                on:input=move |ev| name.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <label class="space-y-1 block">
                                            <span class="text-label-md text-on-surface-variant">
                                                "Nomor WhatsApp aktif (mis. 0858…)"
                                            </span>
                                            <input
                                                type="tel"
                                                class=field
                                                placeholder="08xxxxxxxxxx"
                                                prop:value=move || phone.get()
                                                on:input=move |ev| phone.set(event_target_value(&ev))
                                            />
                                        </label>
                                        // ── Profil mahasiswa — hanya peran santri ──
                                        <Show when=move || need_student.get() fallback=|| ()>
                                            <div class="pt-1 space-y-3">
                                                <p class="text-label-md text-on-surface-variant border-t border-outline-variant/50 pt-3">
                                                    "Data santri"
                                                </p>
                                                <label class="space-y-1 block">
                                                    <span class="text-label-md text-on-surface-variant">"Jenis kelamin"</span>
                                                    <select
                                                        class=field
                                                        prop:value=move || gender.get()
                                                        on:change=move |ev| gender.set(event_target_value(&ev))
                                                    >
                                                        <option value="">"Pilih…"</option>
                                                        <option value="L">"Laki-laki"</option>
                                                        <option value="P">"Perempuan"</option>
                                                    </select>
                                                </label>
                                                <label class="space-y-1 block">
                                                    <span class="text-label-md text-on-surface-variant">"Kampus"</span>
                                                    <input
                                                        type="text"
                                                        class=field
                                                        placeholder="mis. Universitas Indonesia"
                                                        prop:value=move || campus.get()
                                                        on:input=move |ev| campus.set(event_target_value(&ev))
                                                    />
                                                </label>
                                                <label class="space-y-1 block">
                                                    <span class="text-label-md text-on-surface-variant">"Jurusan"</span>
                                                    <input
                                                        type="text"
                                                        class=field
                                                        placeholder="mis. Teknik Informatika"
                                                        prop:value=move || major.get()
                                                        on:input=move |ev| major.set(event_target_value(&ev))
                                                    />
                                                </label>
                                                <label class="space-y-1 block">
                                                    <span class="text-label-md text-on-surface-variant">
                                                        "Tahun masuk PPM"
                                                    </span>
                                                    <input
                                                        type="text"
                                                        class=field
                                                        placeholder="mis. 2024"
                                                        inputmode="numeric"
                                                        maxlength="4"
                                                        prop:value=move || entry_year.get()
                                                        on:input=move |ev| entry_year.set(event_target_value(&ev))
                                                    />
                                                </label>
                                            </div>
                                        </Show>
                                        <p class="text-[11px] text-on-surface-variant">
                                            "OTP & password akun akan dikirim ke WhatsApp ini."
                                        </p>
                                        <button
                                            type="submit"
                                            class="w-full py-3 rounded-xl bg-primary text-on-primary font-bold press disabled:opacity-60"
                                            disabled=move || busy.get()
                                        >
                                            {move || if busy.get() { "Mengirim…" } else { "Lanjut — Kirim OTP" }}
                                        </button>
                                    </form>
                                }
                                    .into_any()
                            }
                        }
                    }}

                    <p class="text-center text-[11px] text-on-surface-variant pt-2">
                        "Sudah punya akun? " <a href="/login" class="text-primary font-semibold">"Masuk"</a>
                    </p>
                </div>
            </main>
        </div>
    }
}
