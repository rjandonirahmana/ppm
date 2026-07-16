//! web/pages/profil.rs — Profil pengguna (mockup Halaqah Manager): kartu hero
//! gradient, informasi kontak, status akademik, pengaturan akun + Logout ASLI.
//! Data via server fn `profil_data` (semua peran).

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::ProfilData;
use crate::web::api::{logout_action, profil_data};
use crate::web::components::{DeviceFrame, MobileHeader, MobileNav, NAV_SANTRI};

#[component]
pub fn ProfilPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { profil_data().await });

    Effect::new(move |_| {
        if let Some(Err(e)) = data.get() {
            let msg = e.to_string();
            if msg.contains("unauth") || msg.contains("forbidden") {
                #[cfg(target_arch = "wasm32")]
                if let Some(w) = web_sys::window() {
                    let _ = w.location().replace("/login");
                }
            }
        }
    });

    view! {
        <Title text="Profil — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto">
                <MobileHeader title="Profil Pengguna" />

                <div class="px-5 pt-5 space-y-5 stagger">
                    <Suspense fallback=|| {
                        view! {
                            <div class="space-y-3 animate-pulse">
                                <div class="h-56 bg-surface-container rounded-2xl"></div>
                                <div class="h-44 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .and_then(|r| r.ok())
                                .map(|p| view! { <ProfilContent p=p /> })
                        }}
                    </Suspense>

                    // ── Pengaturan Akun ────────────────────────────────────
                    <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-5">
                        <div class="flex items-center gap-2 mb-4">
                            <span class="material-symbols-outlined text-on-background">"settings"</span>
                            <h2 class="text-body-lg font-bold text-on-background">"Pengaturan Akun"</h2>
                        </div>
                        <div class="space-y-1">
                            <SettingRow icon="lock" label="Ganti Kata Sandi" />
                            <SettingRow icon="language" label="Bahasa Aplikasi" />
                            <SettingRow icon="shield" label="Privasi & Keamanan" />
                            <SettingRow icon="info" label="Tentang Aplikasi" />
                            <SettingRow icon="help" label="Bantuan & Dukungan" />

                            // Logout ASLI: hapus cookie sesi → /login.
                            <button
                                class="w-full flex items-center gap-4 py-3.5 text-error font-semibold"
                                on:click=move |_| {
                                    leptos::task::spawn_local(async move {
                                        let _ = logout_action().await;
                                        #[cfg(target_arch = "wasm32")]
                                        if let Some(w) = web_sys::window() {
                                            let _ = w.location().replace("/login");
                                        }
                                    });
                                }
                            >
                                <span class="w-11 h-11 rounded-full bg-error-container flex items-center justify-center">
                                    <span class="material-symbols-outlined text-error">"logout"</span>
                                </span>
                                "Keluar"
                            </button>
                        </div>
                    </div>

                    // ── Versi aplikasi ─────────────────────────────────────
                    <div class="bg-surface-container rounded-2xl p-5 text-center">
                        <p class="text-body-md font-bold text-on-background">"Portal PPM AFM v0.1.0"</p>
                        <p class="text-body-sm text-on-surface-variant mt-1">
                            "Absensi & pembinaan santri — dibuat dengan ♥ untuk keluarga PPM AFM."
                        </p>
                    </div>
                </div>

                <MobileNav items=NAV_SANTRI active="/profil" />
            </div>
        </DeviceFrame>
    }
}

#[component]
fn ProfilContent(p: ProfilData) -> impl IntoView {
    let initial = p.name.chars().next().unwrap_or('P').to_string();
    view! {
        // ── Kartu hero ──────────────────────────────────────────────────────
        <div class="spiritual-gradient rounded-2xl p-6 text-on-primary text-center shadow-lg shadow-primary/20">
            <div class="w-24 h-24 mx-auto rounded-full bg-primary-fixed text-primary flex items-center justify-center text-4xl font-bold ring-4 ring-white/20">
                {initial}
            </div>
            <h2 class="text-display-md mt-4">{p.name.clone()}</h2>
            <div class="flex items-center justify-center gap-2 mt-3">
                <span class="px-3 py-1 rounded-full bg-primary-fixed text-primary text-[11px] font-bold tracking-wider">
                    {p.role_label.clone()}
                </span>
                <span class="px-3 py-1 rounded-full bg-white/15 text-[11px] font-bold tracking-wider">
                    "PPM AFM"
                </span>
            </div>
            {(!p.username.is_empty())
                .then(|| {
                    view! {
                        <p class="text-body-sm opacity-80 mt-3">"@" {p.username.clone()}</p>
                    }
                })}
        </div>

        // ── Informasi kontak ────────────────────────────────────────────────
        <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-5">
            <div class="flex items-center gap-2 mb-4">
                <span class="material-symbols-outlined text-on-background">"contact_mail"</span>
                <h2 class="text-body-lg font-bold text-on-background">"Informasi Kontak"</h2>
            </div>
            <div class="space-y-3">
                <ContactRow icon="mail" label="Email" value=p.email.unwrap_or_else(|| "—".into()) />
                <ContactRow icon="call" label="Nomor Telepon" value=p.phone.unwrap_or_else(|| "—".into()) />
                <ContactRow icon="location_on" label="Alamat" value=p.address.unwrap_or_else(|| "—".into()) />
            </div>
        </div>

        // ── Status akademik ─────────────────────────────────────────────────
        <div class="spiritual-gradient rounded-2xl p-6 text-on-primary shadow-lg shadow-primary/20">
            <h2 class="text-body-lg font-bold mb-4">"Status Akademik"</h2>
            <div class="space-y-3">
                <div class="flex items-center justify-between border-b border-white/10 pb-3">
                    <span class="text-[11px] font-bold tracking-[0.15em] opacity-80">"NIS"</span>
                    <span class="text-body-lg font-bold">{p.nis.unwrap_or_else(|| "—".into())}</span>
                </div>
                <div class="flex items-center justify-between">
                    <span class="text-[11px] font-bold tracking-[0.15em] opacity-80">"TOTAL POIN"</span>
                    <span class="text-body-lg font-bold" data-count=p.points.to_string()>{p.points}</span>
                </div>
            </div>
        </div>
    }
}

#[component]
fn ContactRow(icon: &'static str, label: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="bg-surface-container rounded-xl p-3.5 flex items-center gap-3">
            <span class="material-symbols-outlined text-on-surface-variant">{icon}</span>
            <div class="flex-1 min-w-0">
                <p class="text-[11px] font-bold tracking-wider text-on-surface-variant uppercase">
                    {label}
                </p>
                <p class="text-body-md text-on-background truncate">{value}</p>
            </div>
        </div>
    }
}

#[component]
fn SettingRow(icon: &'static str, label: &'static str) -> impl IntoView {
    view! {
        <a href="#" class="flex items-center gap-4 py-3.5 px-2 -mx-2 rounded-xl text-on-background hover:bg-surface-container transition-colors press">
            <span class="w-11 h-11 rounded-full bg-secondary-container flex items-center justify-center">
                <span class="material-symbols-outlined text-primary">{icon}</span>
            </span>
            <span class="flex-1 text-body-md font-medium">{label}</span>
            <span class="material-symbols-outlined text-on-surface-variant">"arrow_forward"</span>
        </a>
    }
}
