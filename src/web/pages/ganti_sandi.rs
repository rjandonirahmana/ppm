//! web/pages/ganti_sandi.rs — Ganti Kata Sandi (user yang sedang login).
//! Field: sandi lama + sandi baru (+ ulangi). Logic: cocokkan sandi lama di
//! server (bcrypt verify); bila cocok → simpan sandi baru.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::web::api::change_password_action;
use crate::web::components::{DeviceFrame, MobileHeader};

#[component]
pub fn GantiSandiPage() -> impl IntoView {
    let old_pw = RwSignal::new(String::new());
    let new_pw = RwSignal::new(String::new());
    let confirm = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        let (o, n, c) = (
            old_pw.get_untracked(),
            new_pw.get_untracked(),
            confirm.get_untracked(),
        );
        if o.is_empty() || n.is_empty() {
            msg.set(Some((false, "Sandi lama & baru wajib diisi.".into())));
            return;
        }
        if n.chars().count() < 6 {
            msg.set(Some((false, "Sandi baru minimal 6 karakter.".into())));
            return;
        }
        if n != c {
            msg.set(Some((false, "Ulangi sandi baru tidak cocok.".into())));
            return;
        }
        busy.set(true);
        msg.set(None);
        leptos::task::spawn_local(async move {
            match change_password_action(o, n).await {
                Ok(_) => {
                    msg.set(Some((true, "Kata sandi berhasil diganti.".into())));
                    old_pw.set(String::new());
                    new_pw.set(String::new());
                    confirm.set(String::new());
                }
                Err(e) => {
                    let m = e.to_string();
                    msg.set(Some((false, m.rsplit(": ").next().unwrap_or(&m).to_string())));
                }
            }
            busy.set(false);
        });
    };

    let field = "w-full bg-surface-container border-0 rounded-xl px-4 py-3 text-body-md text-on-surface";
    view! {
        <Title text="Ganti Kata Sandi — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Ganti Kata Sandi" subtitle="Perbarui sandi akun Anda" back_href="/profil" />
                <div class="px-5 pt-5">
                    <form class="ppm-card p-6 space-y-4" on:submit=submit>
                        {move || {
                            msg.get().map(|(ok, t)| {
                                let cls = if ok {
                                    "p-3 bg-secondary-container text-on-secondary-container rounded-xl text-body-sm"
                                } else {
                                    "p-3 bg-error-container text-on-error-container rounded-xl text-body-sm"
                                };
                                view! { <div class=cls>{t}</div> }
                            })
                        }}
                        <div class="space-y-1.5">
                            <label class="text-body-sm font-medium text-on-surface-variant">"Sandi Lama"</label>
                            <input type="password" class=field placeholder="••••••••" autocomplete="current-password"
                                prop:value=move || old_pw.get()
                                on:input=move |e| old_pw.set(event_target_value(&e)) />
                        </div>
                        <div class="space-y-1.5">
                            <label class="text-body-sm font-medium text-on-surface-variant">"Sandi Baru"</label>
                            <input type="password" class=field placeholder="Minimal 6 karakter" autocomplete="new-password"
                                prop:value=move || new_pw.get()
                                on:input=move |e| new_pw.set(event_target_value(&e)) />
                        </div>
                        <div class="space-y-1.5">
                            <label class="text-body-sm font-medium text-on-surface-variant">"Ulangi Sandi Baru"</label>
                            <input type="password" class=field placeholder="Ketik ulang sandi baru" autocomplete="new-password"
                                prop:value=move || confirm.get()
                                on:input=move |e| confirm.set(event_target_value(&e)) />
                        </div>
                        <button type="submit"
                            class="w-full py-3.5 spiritual-gradient text-on-primary rounded-xl font-bold text-body-md press disabled:opacity-60"
                            disabled=move || busy.get()>
                            {move || if busy.get() { "Menyimpan…" } else { "Simpan Sandi Baru" }}
                        </button>
                    </form>
                </div>
            </div>
        </DeviceFrame>
    }
}
