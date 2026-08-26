//! web/pages/login.rs — Portal AFM SMART (dua kolom: branding + form).
//!
//! Diporting dari desain `login_portal_ppm_afm_polished`. Toggle password lewat
//! inline JS (progressive enhancement). Submit → server fn `login_action`
//! (bcrypt + JWT cookie) → redirect sesuai peran.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{role_home, SessionUser};
use crate::web::api::login_action;

#[component]
pub fn LoginPage() -> impl IntoView {
    let login = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);
    // Pesan info (mis. forgot-password terkirim).
    let info = RwSignal::new(Option::<String>::None);

    // Lupa password: pakai nomor HP yang sudah diketik → kirim password baru via WA.
    //
    // HASILNYA DIBACA. Sebelum ini balasannya dibuang (`let _ = …`) dan layar
    // SELALU menampilkan "password baru dikirim, cek WA" — apa pun yang
    // sebenarnya terjadi. Nomor salah ketik, akun nonaktif, WAHA mati, bahkan
    // 500 dari server, semuanya tampil sebagai keberhasilan, dan orang menunggu
    // WhatsApp yang tak akan pernah datang. Itulah kenapa "registrasi WA masuk,
    // lupa sandi tidak" terasa seperti tak ada sebabnya: sebabnya ada, cuma tak
    // pernah sampai ke layar.
    let on_forgot = move |ev: leptos::ev::MouseEvent| {
        ev.prevent_default();
        let l = login.get_untracked();
        if l.trim().len() < 6 {
            error.set(Some("Isi nomor HP dulu untuk reset password.".into()));
            return;
        }
        if busy.get_untracked() {
            return;
        }
        info.set(None);
        error.set(None);
        busy.set(true);
        leptos::task::spawn_local(async move {
            let hasil = crate::web::api::forgot_password_action(l).await;
            busy.set(false);
            match hasil {
                Ok(pesan) => info.set(Some(pesan)),
                Err(e) => {
                    let msg = e.to_string();
                    error.set(Some(crate::web::components::pesan_galat(&msg)));
                }
            }
        });
    };

    // Sudah punya sesi (JWT cookie masih hidup)? Langsung masuk — tak perlu
    // login ulang. Pakai CONTEXT sesi global dari App (audit poin 4); sliding
    // refresh sudah terjadi saat resource global fetch di awal load.
    let session = use_context::<Resource<Option<SessionUser>>>();
    Effect::new(move |_| {
        if let Some(Some(user)) = session.and_then(|s| s.get()) {
            crate::web::components::masuk_ke(role_home(&user.role));
        }
    });

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let l = login.get_untracked();
        let p = password.get_untracked();
        if l.trim().is_empty() || p.is_empty() {
            error.set(Some("Nomor HP / username dan kata sandi wajib diisi.".into()));
            return;
        }
        busy.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            match login_action(l, p).await {
                Ok(path) => {
                    // Full reload → cookie sesi terpasang, SSR render sesuai peran.
                    crate::web::components::masuk_ke(&path);
                }
                Err(e) => {
                    // Ambil pesan inti tanpa prefix ServerFnError.
                    let msg = e.to_string();
                    let msg = crate::web::components::pesan_galat(&msg);
                    error.set(Some(msg));
                    busy.set(false);
                }
            }
        });
    };

    view! {
        <Title text="Masuk — AFM SMART" />
        <PasswordToggleScript />
        <div class="min-h-screen flex items-center justify-center p-4">
            <div class="fixed inset-0 z-0">
                <div class="absolute inset-0 pattern-bg"></div>
            </div>

            <main class="relative z-10 w-full max-w-5xl grid md:grid-cols-2 gap-0 overflow-hidden rounded-2xl shadow-2xl bg-surface-container-lowest anim-in">

                // ── Kiri: branding (desktop) ────────────────────────────────
                <div class="spiritual-gradient hidden md:flex flex-col justify-between p-12 text-on-primary">
                    <div class="space-y-8">
                        <div class="flex items-center gap-4">
                            // Logo asli, bukan glyph masjid generik: ini
                            // penanda lembaganya, dan ikon bawaan font tak
                            // mewakili apa pun yang khas.
                            <div class="w-14 h-14 bg-white rounded-xl flex items-center justify-center shadow-inner overflow-hidden">
                                <img src="/icons/logo.png" alt="Logo AFM SMART" class="w-full h-full object-contain p-1.5" />
                            </div>
                            <div>
                                <h1 class="text-display-md tracking-tight leading-none">"AFM SMART"</h1>
                                <p class="text-primary-fixed/80 text-sm font-medium mt-1 uppercase tracking-widest">
                                    "Pusat Pendidikan Modern"
                                </p>
                            </div>
                        </div>
                        <div class="space-y-4">
                            <h2 class="text-display-lg leading-tight">
                                "Profesionalisme Spiritual dalam Pendidikan"
                            </h2>
                            <p class="text-body-md opacity-80 max-w-sm">
                                "Membangun generasi unggul melalui integrasi nilai-nilai keislaman dan manajemen profesional modern."
                            </p>
                        </div>
                    </div>
                    <div class="flex items-center gap-4 pt-8 border-t border-white/10">
                        <div class="flex -space-x-3">
                            <div class="w-10 h-10 rounded-full border-2 border-primary bg-primary-fixed-dim flex items-center justify-center text-[10px] text-primary font-bold">"AF"</div>
                            <div class="w-10 h-10 rounded-full border-2 border-primary bg-secondary-container flex items-center justify-center text-[10px] text-primary font-bold">"PM"</div>
                            <div class="w-10 h-10 rounded-full border-2 border-primary bg-surface-variant flex items-center justify-center text-[10px] text-primary font-bold">"+"</div>
                        </div>
                        <p class="text-label-md">"Bergabung dengan 500+ Asatidzah & Staff"</p>
                    </div>
                </div>

                // ── Kanan: form login ───────────────────────────────────────
                <div class="p-8 md:p-16 flex flex-col justify-center bg-surface relative">
                    // Header khusus mobile
                    <div class="md:hidden flex flex-col items-center mb-10 text-center">
                        <div class="w-20 h-20 bg-white rounded-2xl flex items-center justify-center mb-4 shadow-xl overflow-hidden">
                            <img src="/icons/logo.png" alt="Logo AFM SMART" class="w-full h-full object-contain p-2" />
                        </div>
                        <h1 class="text-display-md text-primary">"AFM SMART"</h1>
                        <p class="text-primary font-medium text-xs uppercase tracking-widest opacity-70 mt-1">
                            "Pusat Pendidikan Modern"
                        </p>
                    </div>

                    <div class="max-w-sm mx-auto w-full">
                        <header class="mb-10">
                            <h3 class="text-headline-sm text-on-background mb-2">"Selamat Datang Kembali"</h3>
                            <p class="text-body-md text-on-surface-variant">
                                "Silakan masuk ke akun Anda untuk melanjutkan akses portal."
                            </p>
                        </header>

                        // method="post" + action WAJIB eksplisit — pertahanan
                        // berlapis: `on:submit` (prevent_default+fetch WASM)
                        // hanya aktif SETELAH hydration selesai. Kalau form
                        // ini ter-submit SEBELUM hydration (race saat load
                        // lambat/WASM belum siap), browser jatuh ke perilaku
                        // NATIF — TANPA method eksplisit itu defaultnya GET,
                        // yang membocorkan password ke query string URL
                        // (riwayat browser, log akses server). method="post"
                        // membuat fallback nativenya aman (POST tanpa handler
                        // nyata → cuma render ulang halaman, TAK ada kebocoran)
                        // walau tetap tak login (itu wajar: harus tunggu JS).
                        <form
                            class="space-y-6"
                            method="post"
                            action="/login"
                            on:submit=on_submit
                        >
                            {move || {
                                error
                                    .get()
                                    .map(|e| {
                                        view! {
                                            <div class="flex items-center gap-2 p-3 bg-error-container text-on-error-container rounded-xl text-body-sm" role="alert">
                                                <span class="material-symbols-outlined text-xl">"error"</span>
                                                <span>{e}</span>
                                            </div>
                                        }
                                    })
                            }}
                            {move || {
                                info.get().map(|m| view! {
                                    <div class="flex items-center gap-2 p-3 bg-secondary-container text-on-secondary-container rounded-xl text-body-sm" role="status">
                                        <span class="material-symbols-outlined text-xl">"chat"</span>
                                        <span>{m}</span>
                                    </div>
                                })
                            }}
                            <div class="space-y-2">
                                <label class="text-label-md text-on-surface-variant ml-1" for="username">
                                    "Nomor HP atau Username"
                                </label>
                                <div class="relative group">
                                    <span class="material-symbols-outlined absolute left-4 top-1/2 -translate-y-1/2 text-outline group-focus-within:text-primary transition-colors">"person"</span>
                                    <input
                                        class="w-full pl-12 pr-4 py-4 bg-surface-container-lowest border border-outline-variant rounded-xl focus:border-primary input-focus-ring outline-none transition-all text-body-md"
                                        id="username"
                                        name="username"
                                        placeholder="08xxxxxxxxxx atau username"
                                        type="text"
                                        autocomplete="username"
                                        required=true
                                        prop:value=move || login.get()
                                        on:input=move |ev| login.set(event_target_value(&ev))
                                    />
                                </div>
                            </div>

                            <div class="space-y-2">
                                <div class="flex justify-between items-center px-1">
                                    <label class="text-label-md text-on-surface-variant" for="password">"Kata Sandi"</label>
                                    // Berlabel "Mengirim…" selama permintaannya jalan. Tanpa penanda
                                    // apa pun, tautan yang diklik lalu diam beberapa detik
                                    // (bcrypt + panggilan WAHA) terbaca sebagai tak berfungsi,
                                    // dan orang menekannya berkali-kali — yang justru menabrak
                                    // batas laju 10 menit.
                                    <a
                                        class="text-label-md text-primary font-bold hover:underline cursor-pointer"
                                        href="#"
                                        on:click=on_forgot
                                    >
                                        {move || if busy.get() { "Mengirim…" } else { "Lupa sandi?" }}
                                    </a>
                                </div>
                                <div class="relative group">
                                    <span class="material-symbols-outlined absolute left-4 top-1/2 -translate-y-1/2 text-outline group-focus-within:text-primary transition-colors">"lock"</span>
                                    <input
                                        class="w-full pl-12 pr-12 py-4 bg-surface-container-lowest border border-outline-variant rounded-xl focus:border-primary input-focus-ring outline-none transition-all text-body-md"
                                        id="password"
                                        name="password"
                                        placeholder="••••••••"
                                        type="password"
                                        required=true
                                        prop:value=move || password.get()
                                        on:input=move |ev| password.set(event_target_value(&ev))
                                    />
                                    <button
                                        class="absolute right-4 top-1/2 -translate-y-1/2 text-outline hover:text-primary transition-colors"
                                        type="button"
                                        id="toggle-password"
                                    >
                                        <span class="material-symbols-outlined">"visibility"</span>
                                    </button>
                                </div>
                            </div>

                            <button
                                class="w-full py-4 bg-primary text-on-primary font-bold text-lg rounded-xl shadow-lg shadow-primary/20 hover:bg-primary-container active:scale-[0.98] transition-all flex items-center justify-center gap-3 mt-4 disabled:opacity-60"
                                type="submit"
                                disabled=move || busy.get()
                            >
                                {move || if busy.get() { "Memproses…" } else { "Masuk" }}
                                <span class="material-symbols-outlined text-xl">"arrow_forward"</span>
                            </button>
                        </form>

                        // (Login sosial Google/SSO placeholder DIHAPUS — belum berfungsi.)

                        <footer class="mt-12 text-center">
                            <p class="text-body-sm text-on-surface-variant">
                                "Punya kode referal dari admin? "
                                <a class="text-primary font-bold hover:underline" href="/register">"Daftar di sini"</a>
                            </p>
                            <div class="mt-6">
                                <a class="inline-flex items-center gap-1.5 text-label-md text-primary font-semibold hover:underline" href="/">
                                    <span class="material-symbols-outlined text-lg">"home"</span>
                                    "Kembali ke Beranda"
                                </a>
                            </div>
                        </footer>
                    </div>
                </div>
            </main>
        </div>
    }
}

/// Inline JS: toggle lihat/sembunyi password (progressive enhancement).
#[component]
fn PasswordToggleScript() -> impl IntoView {
    let js = r#"
(function(){
  if(window.__ppmLogin) return; window.__ppmLogin=true;
  document.addEventListener('click', function(e){
    var b=e.target.closest('#toggle-password'); if(!b) return;
    var inp=document.getElementById('password'); var ic=b.querySelector('span');
    if(!inp) return;
    if(inp.type==='password'){ inp.type='text'; if(ic) ic.textContent='visibility_off'; }
    else { inp.type='password'; if(ic) ic.textContent='visibility'; }
  });
})();
"#;
    view! { <script inner_html=js></script> }
}
