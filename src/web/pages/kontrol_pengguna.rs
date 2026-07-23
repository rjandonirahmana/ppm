//! web/pages/kontrol_pengguna.rs — "User Control" (/kontrol-pengguna, migrasi
//! 17: activity_logs). Nav item ini tampil di SEMUA peran staf (uniform per
//! components::nav_for), tapi halaman + server fn tetap admin-only — non-admin
//! yang mengklik dari nav melihat kartu "Khusus Admin", BUKAN dilempar ke
//! /login (mereka toh sudah login sah, cuma beda peran).

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{ActivityLogItem, UserControlData, UserRow};
use crate::web::api::{
    activity_log_data, change_user_role_action, toggle_user_active_action, user_control_data,
};
use crate::web::components::{DeviceFrame, MobileHeader};

const ROLE_OPTIONS: &[(&str, &str)] = &[
    ("admin", "Admin"),
    ("teacher", "Guru"),
    ("dewan_guru", "Dewan Guru"),
    ("supervisor", "Pamong"),
    ("santri", "Santri"),
    ("parent", "Orang Tua"),
];

#[component]
pub fn KontrolPenggunaPage() -> impl IntoView {
    let role_filter = RwSignal::new(String::new());
    let data = Resource::new(
        move || role_filter.get(),
        |f| async move { user_control_data(f).await },
    );
    let logs = Resource::new(|| (), |_| async move { activity_log_data().await });

    Effect::new(move |_| {
        if let Some(Err(e)) = data.get() {
            if e.to_string().contains("unauth") {
                #[cfg(target_arch = "wasm32")]
                if let Some(w) = web_sys::window() {
                    let _ = w.location().replace("/login");
                }
            }
        }
    });

    view! {
        <Title text="User Control — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="User Control" subtitle="Administrasi akun & keamanan" back_href="/staf" />

                <div class="px-5 pt-5 space-y-4 stagger">
                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
                                    <div class="h-20 bg-surface-container rounded-2xl"></div>
                                    <div class="h-20 bg-surface-container rounded-2xl"></div>
                                    <div class="h-20 bg-surface-container rounded-2xl hidden md:block"></div>
                                    <div class="h-20 bg-surface-container rounded-2xl hidden md:block"></div>
                                </div>
                                <div class="h-64 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => {
                                        view! {
                                            <Body
                                                d=d
                                                role_filter=role_filter
                                                refetch=move || data.refetch()
                                            />
                                        }
                                            .into_any()
                                    }
                                    Err(e) => {
                                        let msg = e.to_string();
                                        if msg.contains("forbidden") {
                                            view! { <AdminOnlyCard /> }.into_any()
                                        } else {
                                            view! {
                                                <crate::web::components::FetchError err=msg />
                                            }
                                                .into_any()
                                        }
                                    }
                                })
                        }}
                    </Suspense>

                    // ── Activity Logs ────────────────────────────────────────
                    <Suspense fallback=|| ()>
                        {move || {
                            logs.get()
                                .and_then(|r| r.ok())
                                .map(|items| view! { <ActivityPanel items=items /> })
                        }}
                    </Suspense>
                </div>
            </div>
        </DeviceFrame>
    }
}

/// Non-admin (guru/pamong/dewan guru) yang membuka lewat item nav "User
/// Control" — bukan sesi berakhir, cuma bukan peran yang tepat.
#[component]
fn AdminOnlyCard() -> impl IntoView {
    view! {
        <div class="ppm-card p-8 text-center space-y-2 anim-in">
            <span class="material-symbols-outlined text-4xl text-on-surface-variant/60">"lock"</span>
            <p class="text-body-md font-semibold text-on-background">"Halaman ini khusus Admin."</p>
            <p class="text-body-sm text-on-surface-variant">
                "Hubungi administrator bila Anda perlu mengelola akun pengguna."
            </p>
        </div>
    }
}

#[component]
fn Body(
    d: UserControlData,
    role_filter: RwSignal<String>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let users = d.users;
    view! {
        <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
            <div class="ppm-card p-4">
                <span class="material-symbols-outlined text-primary">"group"</span>
                <p class="text-2xl font-bold text-on-background mt-1" data-count=d.total.to_string()>
                    {d.total}
                </p>
                <p class="text-[11px] font-bold tracking-wider text-on-surface-variant uppercase">"Total User"</p>
            </div>
            <div class="ppm-card p-4">
                <span class="material-symbols-outlined text-primary">"school"</span>
                <p class="text-2xl font-bold text-on-background mt-1" data-count=d.santri_count.to_string()>
                    {d.santri_count}
                </p>
                <p class="text-[11px] font-bold tracking-wider text-on-surface-variant uppercase">"Santri"</p>
            </div>
            <div class="ppm-card p-4">
                <span class="material-symbols-outlined text-primary">"supervisor_account"</span>
                <p class="text-2xl font-bold text-on-background mt-1" data-count=d.staff_count.to_string()>
                    {d.staff_count}
                </p>
                <p class="text-[11px] font-bold tracking-wider text-on-surface-variant uppercase">"Guru & Pamong"</p>
            </div>
            <div class="ppm-card p-4">
                <span class="material-symbols-outlined text-error">"person_off"</span>
                <p class="text-2xl font-bold text-on-background mt-1" data-count=d.inactive_count.to_string()>
                    {d.inactive_count}
                </p>
                <p class="text-[11px] font-bold tracking-wider text-on-surface-variant uppercase">"Nonaktif"</p>
            </div>
        </div>

        // ── Filter peran ─────────────────────────────────────────────────────
        <div class="flex gap-2 overflow-x-auto pb-1">
            <RoleChip filter=role_filter value="" label="Semua Peran" />
            {ROLE_OPTIONS
                .iter()
                .map(|(v, l)| view! { <RoleChip filter=role_filter value=*v label=*l /> })
                .collect_view()}
        </div>

        // ── Tabel user ───────────────────────────────────────────────────────
        {if users.is_empty() {
            view! {
                <div class="ppm-empty space-y-1.5">
                    <span class="material-symbols-outlined text-4xl text-on-surface-variant/60">"group_off"</span>
                    <p class="text-body-md font-semibold text-on-background">"Tidak ada pengguna"</p>
                </div>
            }
                .into_any()
        } else {
            view! {
                <div class="ppm-card divide-y divide-outline-variant/40">
                    {users
                        .into_iter()
                        .map(|u| view! { <UserRowView u=u refetch=refetch /> })
                        .collect_view()}
                </div>
            }
                .into_any()
        }}
    }
}

#[component]
fn RoleChip(filter: RwSignal<String>, value: &'static str, label: &'static str) -> impl IntoView {
    let cls = move || {
        if filter.get() == value {
            "px-4 py-2 rounded-full bg-secondary-container text-primary text-body-sm font-semibold whitespace-nowrap shrink-0 press"
        } else {
            "px-4 py-2 rounded-full bg-surface-container-lowest border border-outline-variant/60 text-on-surface-variant text-body-sm whitespace-nowrap shrink-0 press"
        }
    };
    view! {
        <button class=cls on:click=move |_| filter.set(value.to_string())>
            {label}
        </button>
    }
}

#[component]
fn UserRowView(u: UserRow, refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let id = u.id;
    let busy = RwSignal::new(false);
    let is_active = u.is_active;
    let initial = u.name.chars().next().unwrap_or('U').to_string();

    let toggle = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            let _ = toggle_user_active_action(id, !is_active).await;
            busy.set(false);
            refetch();
        });
    };
    let change_role = move |ev: leptos::ev::Event| {
        let new_role = event_target_value(&ev);
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            let _ = change_user_role_action(id, new_role).await;
            busy.set(false);
            refetch();
        });
    };

    let status_dot = if is_active { "bg-success" } else { "bg-error" };
    let status_text = if is_active {
        "text-success"
    } else {
        "text-error opacity-70"
    };
    let toggle_icon = if is_active { "block" } else { "check_circle" };
    let toggle_label = if is_active { "Nonaktifkan" } else { "Aktifkan" };
    let cur_role = u.role.clone();

    view! {
        <div class="p-3.5 flex items-center gap-3">
            <div class="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center font-bold text-primary shrink-0">
                {initial}
            </div>
            <div class="flex-1 min-w-0">
                <p class="text-body-md font-semibold text-on-background truncate">{u.name}</p>
                <p class="text-body-sm text-on-surface-variant truncate">{u.contact}</p>
                <div class="flex items-center gap-1.5 mt-0.5">
                    <span class=format!("w-1.5 h-1.5 rounded-full {status_dot}")></span>
                    <span class=format!("text-[11px] font-medium {status_text}")>
                        {if is_active { "Aktif" } else { "Nonaktif" }}
                    </span>
                </div>
            </div>
            <select
                class="bg-surface-container border-0 rounded-lg px-2 py-1.5 text-[11px] font-semibold text-on-surface shrink-0 disabled:opacity-50"
                disabled=move || busy.get()
                on:change=change_role
            >
                {ROLE_OPTIONS
                    .iter()
                    .map(|(v, l)| {
                        let sel = *v == cur_role;
                        view! { <option value=*v selected=sel>{*l}</option> }
                    })
                    .collect_view()}
            </select>
            <button
                class="w-9 h-9 rounded-lg bg-surface-container text-on-surface-variant flex items-center justify-center shrink-0 press disabled:opacity-50"
                disabled=move || busy.get()
                on:click=toggle
                aria-label=toggle_label
                title=toggle_label
            >
                <span class="material-symbols-outlined text-[20px]">{toggle_icon}</span>
            </button>
        </div>
    }
}

#[component]
fn ActivityPanel(items: Vec<ActivityLogItem>) -> impl IntoView {
    view! {
        <div class="ppm-card p-4 space-y-1">
            <h3 class="text-body-md font-bold text-on-background flex items-center gap-2 mb-2">
                <span class="material-symbols-outlined text-primary">"history"</span>
                "Activity Logs"
            </h3>
            {if items.is_empty() {
                view! {
                    <p class="text-body-sm text-on-surface-variant">"Belum ada aktivitas tercatat."</p>
                }
                    .into_any()
            } else {
                items
                    .into_iter()
                    .map(|l| {
                        let target = l.target_name.map(|t| format!(" → {t}")).unwrap_or_default();
                        let detail = l.detail.map(|d| format!(" ({d})")).unwrap_or_default();
                        view! {
                            <div class="py-2 border-b border-outline-variant/30 last:border-0">
                                <p class="text-body-sm text-on-background">
                                    <b>{l.actor_name}</b>
                                    " — "
                                    {l.action_label}
                                    {target}
                                    <span class="text-on-surface-variant">{detail}</span>
                                </p>
                                <p class="text-[10px] text-on-surface-variant">{l.when_label}</p>
                            </div>
                        }
                    })
                    .collect_view()
                    .into_any()
            }}
        </div>
    }
}
