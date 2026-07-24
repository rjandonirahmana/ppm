//! web/pages/kontrol_pengguna.rs — "User Control" (/kontrol-pengguna, migrasi
//! 17: activity_logs). Nav item ini tampil di SEMUA peran staf (uniform per
//! components::nav_for), tapi halaman + server fn tetap admin-only — non-admin
//! yang mengklik dari nav melihat kartu "Khusus Admin", BUKAN dilempar ke
//! /login (mereka toh sudah login sah, cuma beda peran).

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{ActivityLogItem, RfidDeviceItem, UserControlData, UserRow};
use crate::web::api::{
    activity_log_data, change_user_role_action, create_rfid_device_action, delete_rfid_device_action,
    regenerate_rfid_key_action, rfid_devices_list, toggle_user_active_action,
    update_rfid_device_action, user_control_data,
};
use crate::web::components::{DeviceFrame, EmptyState, MobileHeader};

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

                    // ── Perangkat RFID / Ruang ───────────────────────────────
                    <RfidPanel />

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

/// Manajemen perangkat RFID (= "ruang"). Buat perangkat DULU sebelum dipakai
/// sebagai ruang di jadwal. api_key dipakai firmware ESP8266 (POST /api/rfid/scan).
#[component]
fn RfidPanel() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { rfid_devices_list().await });
    let show_form = RwSignal::new(false);
    let name = RwSignal::new(String::new());
    let serial = RwSignal::new(String::new());
    let location = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let (n, s, l) = (name.get_untracked(), serial.get_untracked(), location.get_untracked());
        leptos::task::spawn_local(async move {
            // api_key dikosongkan → server generate otomatis.
            match create_rfid_device_action(n, s, l, String::new()).await {
                Ok(_) => {
                    name.set(String::new());
                    serial.set(String::new());
                    location.set(String::new());
                    show_form.set(false);
                    data.refetch();
                }
                Err(e) => {
                    let m = e.to_string();
                    msg.set(Some((false, m.rsplit(": ").next().unwrap_or(&m).to_string())));
                }
            }
            busy.set(false);
        });
    };

    let field = "w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface";
    view! {
        <div class="ppm-card p-4 space-y-3">
            <div class="flex items-center justify-between gap-2">
                <h3 class="text-body-md font-bold text-on-background flex items-center gap-2">
                    <span class="material-symbols-outlined text-primary">"sensors"</span>
                    "Perangkat RFID / Ruang"
                </h3>
                <button
                    class="px-3 py-1.5 rounded-lg bg-secondary-container text-primary text-body-sm font-semibold press"
                    on:click=move |_| show_form.update(|s| *s = !*s)
                >
                    {move || if show_form.get() { "Tutup" } else { "+ Tambah" }}
                </button>
            </div>
            <p class="text-[11px] text-on-surface-variant">
                "Daftarkan perangkat/ruang RFID di sini DULU — nanti dipilih sebagai ruang saat buat jadwal. api_key dipakai firmware perangkat."
            </p>

            {move || {
                msg.get()
                    .map(|(_, t)| {
                        view! {
                            <div class="p-2 bg-error-container text-on-error-container rounded-lg text-[11px]">
                                {t}
                            </div>
                        }
                    })
            }}

            {move || {
                show_form
                    .get()
                    .then(|| {
                        view! {
                            <form class="space-y-2 anim-in" method="post" on:submit=submit>
                                <input
                                    type="text"
                                    class=field
                                    placeholder="Nama perangkat/ruang (mis. Aula Utama)"
                                    prop:value=move || name.get()
                                    on:input=move |ev| name.set(event_target_value(&ev))
                                    required=true
                                />
                                <div class="grid grid-cols-2 gap-2">
                                    <input
                                        type="text"
                                        class=field
                                        placeholder="Serial (opsional)"
                                        prop:value=move || serial.get()
                                        on:input=move |ev| serial.set(event_target_value(&ev))
                                    />
                                    <input
                                        type="text"
                                        class=field
                                        placeholder="Lokasi (opsional)"
                                        prop:value=move || location.get()
                                        on:input=move |ev| location.set(event_target_value(&ev))
                                    />
                                </div>
                                <button
                                    type="submit"
                                    class="w-full py-2.5 rounded-lg bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                                    disabled=move || busy.get()
                                >
                                    {move || if busy.get() { "Menyimpan…" } else { "Simpan Perangkat (api_key otomatis)" }}
                                </button>
                            </form>
                        }
                    })
            }}

            <Suspense fallback=|| ()>
                {move || {
                    data.get()
                        .map(|res| match res {
                            Ok(items) if items.is_empty() => {
                                view! {
                                    <EmptyState
                                        icon="sensors_off"
                                        title="Belum ada perangkat RFID"
                                        subtitle="Tambahkan perangkat/ruang lewat tombol di atas."
                                    />
                                }
                                    .into_any()
                            }
                            Ok(items) => {
                                view! {
                                    <div class="ppm-card divide-y divide-outline-variant/40">
                                        {items
                                            .into_iter()
                                            .map(|d| {
                                                view! { <RfidRow d=d refetch=move || data.refetch() /> }
                                            })
                                            .collect_view()}
                                    </div>
                                }
                                    .into_any()
                            }
                            Err(e) => {
                                let m = e.to_string();
                                view! {
                                    <p class="text-body-sm text-on-surface-variant py-2">
                                        {m.rsplit(": ").next().unwrap_or(&m).to_string()}
                                    </p>
                                }
                                    .into_any()
                            }
                        })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn RfidRow(d: RfidDeviceItem, refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let id = d.id;
    let busy = RwSignal::new(false);
    let editing = RwSignal::new(false);
    let key_shown = RwSignal::new(d.api_key.clone());
    let e_name = RwSignal::new(d.device_name.clone());
    let e_serial = RwSignal::new(d.serial_number.clone());
    let e_loc = RwSignal::new(d.location.clone());

    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        let (n, s, l) = (e_name.get_untracked(), e_serial.get_untracked(), e_loc.get_untracked());
        leptos::task::spawn_local(async move {
            if update_rfid_device_action(id, n, s, l).await.is_ok() {
                editing.set(false);
                refetch();
            }
            busy.set(false);
        });
    };
    let regen = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            if let Ok(k) = regenerate_rfid_key_action(id).await {
                key_shown.set(k);
            }
            busy.set(false);
        });
    };
    let del = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            let _ = delete_rfid_device_action(id).await;
            busy.set(false);
            refetch();
        });
    };

    let name_ro = d.device_name.clone();
    let meta = {
        let mut parts = Vec::new();
        if !d.location.is_empty() {
            parts.push(d.location.clone());
        }
        if !d.serial_number.is_empty() {
            parts.push(format!("SN: {}", d.serial_number));
        }
        parts.join(" • ")
    };
    let field = "w-full bg-surface-container border-0 rounded-lg px-3 py-2 text-body-sm text-on-surface";
    view! {
        <div class="p-3 space-y-2">
            {move || {
                if editing.get() {
                    view! {
                        <form class="space-y-2 anim-in" method="post" on:submit=save>
                            <input
                                type="text"
                                class=field
                                prop:value=move || e_name.get()
                                on:input=move |ev| e_name.set(event_target_value(&ev))
                                required=true
                            />
                            <div class="grid grid-cols-2 gap-2">
                                <input
                                    type="text"
                                    class=field
                                    placeholder="Serial"
                                    prop:value=move || e_serial.get()
                                    on:input=move |ev| e_serial.set(event_target_value(&ev))
                                />
                                <input
                                    type="text"
                                    class=field
                                    placeholder="Lokasi"
                                    prop:value=move || e_loc.get()
                                    on:input=move |ev| e_loc.set(event_target_value(&ev))
                                />
                            </div>
                            <div class="grid grid-cols-2 gap-2">
                                <button
                                    type="button"
                                    class="py-2 rounded-lg border border-outline-variant text-on-surface text-body-sm font-semibold"
                                    on:click=move |_| editing.set(false)
                                >
                                    "Batal"
                                </button>
                                <button
                                    type="submit"
                                    class="py-2 rounded-lg bg-primary text-on-primary text-body-sm font-semibold disabled:opacity-60"
                                    disabled=move || busy.get()
                                >
                                    "Simpan"
                                </button>
                            </div>
                        </form>
                    }
                        .into_any()
                } else {
                    let name = name_ro.clone();
                    let meta = meta.clone();
                    view! {
                        <div class="flex items-center gap-3">
                            <span class="w-9 h-9 rounded-lg bg-secondary-container text-primary flex items-center justify-center shrink-0">
                                <span class="material-symbols-outlined text-[18px]">"meeting_room"</span>
                            </span>
                            <div class="flex-1 min-w-0">
                                <p class="text-body-sm font-semibold text-on-background truncate">{name}</p>
                                {(!meta.is_empty())
                                    .then(|| {
                                        view! {
                                            <p class="text-[11px] text-on-surface-variant truncate">{meta}</p>
                                        }
                                    })}
                            </div>
                            <button
                                class="w-8 h-8 rounded-lg bg-surface-container text-primary flex items-center justify-center press"
                                on:click=move |_| editing.set(true)
                                aria-label="Edit perangkat"
                            >
                                <span class="material-symbols-outlined text-[16px]">"edit"</span>
                            </button>
                            <button
                                class="w-8 h-8 rounded-lg bg-error-container/60 text-error flex items-center justify-center press disabled:opacity-50"
                                disabled=move || busy.get()
                                on:click=del
                                aria-label="Hapus perangkat"
                            >
                                <span class="material-symbols-outlined text-[16px]">"delete"</span>
                            </button>
                        </div>
                    }
                        .into_any()
                }
            }}
            // api_key (untuk konfigurasi firmware) + tombol ganti.
            <div class="flex items-center gap-2 rounded-lg bg-surface-container px-2.5 py-1.5">
                <span class="material-symbols-outlined text-[15px] text-on-surface-variant">"key"</span>
                <code class="flex-1 min-w-0 text-[11px] text-on-surface-variant truncate">
                    {move || key_shown.get()}
                </code>
                <button
                    class="text-[11px] font-semibold text-primary shrink-0 disabled:opacity-50"
                    disabled=move || busy.get()
                    on:click=regen
                >
                    "Ganti key"
                </button>
            </div>
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
