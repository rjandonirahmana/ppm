//! web/pages/setelan.rs — Setelan admin (global). Saat ini: MODE PERSETUJUAN
//! IZIN — pilih 2 tahap (Pamong → Guru) atau langsung Guru. Nilai disimpan di
//! tabel app_settings (kunci permit_approval_mode) via server fn admin-only.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::web::api::{permit_mode_get, permit_mode_set, reset_semester_points_action};
use crate::web::components::{DeviceFrame, FetchError, MobileHeader};

#[component]
pub fn SetelanPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { permit_mode_get().await });

    Effect::new(move |_| {
        if let Some(Err(e)) = data.get() {
            if crate::web::components::is_auth_error(&e.to_string()) || e.to_string().contains("forbidden") {
                #[cfg(target_arch = "wasm32")]
                if let Some(w) = web_sys::window() {
                    let _ = w.location().replace("/login");
                }
            }
        }
    });

    // Pilihan lokal (diinisialisasi dari resource), status simpan.
    let mode = RwSignal::new(String::new());
    let saving = RwSignal::new(false);
    let saved = RwSignal::new(false);

    Effect::new(move |_| {
        if let Some(Ok(m)) = data.get() {
            mode.set(m);
        }
    });

    // Reset saldo poin semester (PRD: kembalikan semua santri ke 300).
    let reset_busy = RwSignal::new(false);
    let reset_msg = RwSignal::new(String::new());
    let do_reset = move |_| {
        if reset_busy.get_untracked() {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let ok = web_sys::window()
                .and_then(|w| {
                    w.confirm_with_message(
                        "Reset saldo poin SEMUA santri ke 300? Tindakan ini untuk awal semester baru.",
                    )
                    .ok()
                })
                .unwrap_or(false);
            if !ok {
                return;
            }
        }
        reset_busy.set(true);
        reset_msg.set(String::new());
        leptos::task::spawn_local(async move {
            match reset_semester_points_action().await {
                Ok(n) => reset_msg.set(format!("Saldo {n} santri direset ke 300.")),
                Err(e) => reset_msg.set(e.to_string()),
            }
            reset_busy.set(false);
        });
    };

    let choose = move |m: &'static str| {
        if mode.get_untracked() == m || saving.get_untracked() {
            return;
        }
        mode.set(m.to_string());
        saving.set(true);
        saved.set(false);
        leptos::task::spawn_local(async move {
            let _ = permit_mode_set(m.to_string()).await;
            saving.set(false);
            saved.set(true);
        });
    };

    view! {
        <Title text="Setelan — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Setelan" subtitle="Konfigurasi global aplikasi" back_href="/staf" />

                <div class="px-5 pt-5 md:px-8 md:pt-7 space-y-5 md:space-y-0 md:grid md:grid-cols-2 md:gap-5 md:items-start stagger">
                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="h-8 w-48 bg-surface-container rounded-lg"></div>
                                <div class="h-28 bg-surface-container rounded-2xl"></div>
                                <div class="h-28 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(_) => {
                                        view! {
                                            <div class="ppm-card p-5 space-y-4">
                                                <div class="flex items-center gap-2">
                                                    <span class="material-symbols-outlined text-primary">"fact_check"</span>
                                                    <h2 class="text-body-lg font-bold text-on-background">
                                                        "Alur Persetujuan Izin"
                                                    </h2>
                                                </div>
                                                <p class="text-body-sm text-on-surface-variant">
                                                    "Rute izin utama kini diatur PER-KELAS (wali kelas + perlu pamong) di halaman detail kelas. Setelan di bawah hanya DEFAULT untuk santri yang belum punya kelas utama."
                                                </p>

                                                <ModeOption
                                                    active=Signal::derive(move || mode.get() == "two_stage")
                                                    on_pick=move |_| choose("two_stage")
                                                    icon="account_tree"
                                                    title="Dua tahap (Pamong → Guru)"
                                                    desc="Pamong menyetujui lebih dulu, lalu Guru memberi keputusan final. Butuh 2 persetujuan."
                                                />
                                                <ModeOption
                                                    active=Signal::derive(move || mode.get() == "direct_guru")
                                                    on_pick=move |_| choose("direct_guru")
                                                    icon="bolt"
                                                    title="Langsung Guru"
                                                    desc="Izin langsung ke Guru untuk keputusan final (pamong dilewati). Cukup 1 persetujuan."
                                                />

                                                <Show when=move || saving.get()>
                                                    <p class="text-body-sm text-on-surface-variant flex items-center gap-1">
                                                        <span class="material-symbols-outlined text-[16px] pulse-dot">"sync"</span>
                                                        "Menyimpan…"
                                                    </p>
                                                </Show>
                                                <Show when=move || saved.get() && !saving.get()>
                                                    <p class="text-body-sm text-success flex items-center gap-1">
                                                        <span class="material-symbols-outlined text-[16px]">"check_circle"</span>
                                                        "Tersimpan."
                                                    </p>
                                                </Show>
                                            </div>
                                        }
                                            .into_any()
                                    }
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                })
                        }}
                    </Suspense>

                    // ── Reset saldo poin semester ──────────────────────
                    <div class="ppm-card p-5 space-y-3">
                        <div class="flex items-center gap-2">
                            <span class="material-symbols-outlined text-primary">"restart_alt"</span>
                            <h2 class="text-body-lg font-bold text-on-background">"Awal Semester Baru"</h2>
                        </div>
                        <p class="text-body-sm text-on-surface-variant">
                            "Kembalikan saldo poin SEMUA santri ke 300 (PRD: saldo direset tiap awal semester). Riwayat poin lama tetap tersimpan."
                        </p>
                        <button
                            class="px-5 py-2.5 rounded-xl border border-error/40 text-error font-semibold text-body-md cursor-pointer press disabled:opacity-60"
                            prop:disabled=move || reset_busy.get()
                            on:click=do_reset
                        >
                            {move || if reset_busy.get() { "Memproses…" } else { "Reset Saldo → 300" }}
                        </button>
                        <Show when=move || !reset_msg.get().is_empty()>
                            <p class="text-body-sm text-on-surface-variant">{move || reset_msg.get()}</p>
                        </Show>
                    </div>
                </div>
            </div>
        </DeviceFrame>
    }
}

#[component]
fn ModeOption(
    active: Signal<bool>,
    on_pick: impl Fn(()) + Copy + Send + 'static,
    icon: &'static str,
    title: &'static str,
    desc: &'static str,
) -> impl IntoView {
    let cls = move || {
        if active.get() {
            "w-full text-left rounded-2xl border-2 border-primary bg-primary/5 p-4 flex items-start gap-3 press cursor-pointer"
        } else {
            "w-full text-left rounded-2xl border border-outline-variant bg-surface p-4 flex items-start gap-3 press cursor-pointer hover:bg-surface-container transition-colors"
        }
    };
    view! {
        <button class=cls on:click=move |_| on_pick(())>
            <span class="w-11 h-11 rounded-xl bg-secondary-container flex items-center justify-center shrink-0">
                <span class="material-symbols-outlined text-primary">{icon}</span>
            </span>
            <span class="flex-1 min-w-0">
                <span class="flex items-center gap-2">
                    <span class="text-body-md font-bold text-on-background">{title}</span>
                    <Show when=move || active.get()>
                        <span class="material-symbols-outlined text-primary text-[18px]">"check_circle"</span>
                    </Show>
                </span>
                <span class="block text-body-sm text-on-surface-variant mt-0.5">{desc}</span>
            </span>
        </button>
    }
}
