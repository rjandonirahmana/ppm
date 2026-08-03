//! web/pages/kontrol_pengguna.rs — "User Control" (/kontrol-pengguna, migrasi
//! 17: activity_logs). Nav item ini tampil di SEMUA peran staf (uniform per
//! components::nav_for), tapi halaman + server fn tetap admin-only — non-admin
//! yang mengklik dari nav melihat kartu "Khusus Admin", BUKAN dilempar ke
//! /login (mereka toh sudah login sah, cuma beda peran).

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{ActivityLogItem, RfidDeviceItem, SessionUser, UserControlData, UserRow};
use crate::web::api::{
    activity_log_data, assign_card_action, change_user_role_action, create_invite_action,
    create_rfid_device_action, pending_cards_data, search_users_for_card,
    delete_rfid_device_action, regenerate_rfid_key_action, rfid_devices_list,
    toggle_user_active_action, update_rfid_device_action, user_control_data,
};
use crate::web::components::{DeviceFrame, EmptyState, FetchError, MobileHeader};

const ROLE_OPTIONS: &[(&str, &str)] = &[
    ("admin", "Admin"),
    ("ketua", "Ketua"),
    ("dewan_guru", "Dewan Guru"),
    ("supervisor", "Pamong"),
    ("santri", "Santri"),
    ("santri_finance", "Santri (Finance)"),
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
            if crate::web::components::is_auth_error(&e.to_string()) {
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

                    // ── Buat link registrasi (undangan admin) ────────────────
                    <InvitePanel />

                    // ── Perangkat RFID / Ruang ───────────────────────────────
                    <RfidPanel />

                    // ── Pasang kartu RFID ke pengguna ────────────────────────
                    <KartuPanel />

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

/// Buat LINK REGISTRASI: hanya orang dgn link ini (key dari admin) yang boleh
/// mendaftar. Pilih peran → generate → salin link `/register?key=…` (TTL 24 jam),
/// bagikan ke calon pengguna. Mereka daftar (nama+HP) → OTP+password via WhatsApp.
#[component]
fn InvitePanel() -> impl IntoView {
    // Peran yang boleh diundang (admin TIDAK termasuk — dibuat manual).
    // Peran STAF hanya untuk admin: mengundang dewan guru/pamong = memberi
    // wewenang setara, jadi pamong & dewan guru tak boleh mencetaknya (server
    // menolaknya lewat service::registration::can_invite — dropdown ini hanya
    // menyembunyikan pilihan yang pasti ditolak).
    const ROLES: &[(&str, &str)] = &[
        ("santri", "Santri"),
        ("parent", "Orang Tua"),
        ("dewan_guru", "Dewan Guru"),
        ("supervisor", "Pamong"),
    ];
    let session = use_context::<Resource<Option<SessionUser>>>();
    let is_admin = move || {
        session
            .and_then(|s| s.get())
            .flatten()
            .map(|u| crate::models::role_satisfies(&u.role, &["admin"]))
            .unwrap_or(false)
    };
    let role = RwSignal::new("santri".to_string());
    // Kuota: berapa orang boleh pakai token SAMA (1 = sekali pakai; mis. 100 utk
    // intake santri). Masa berlaku dalam hari.
    let kuota = RwSignal::new(1_i64);
    let hari = RwSignal::new(7_i64);
    let code = RwSignal::new(String::new());
    let link = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<String>::None);
    // 0 = belum, 1 = kode tersalin, 2 = link tersalin.
    let copied = RwSignal::new(0u8);

    let generate = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        copied.set(0);
        let r = role.get_untracked();
        let (mu, td) = (kuota.get_untracked().max(1), hari.get_untracked().max(1));
        leptos::task::spawn_local(async move {
            match create_invite_action(r, mu, td).await {
                Ok(token) => {
                    // Rangkai URL penuh dari origin browser (klien).
                    #[cfg(target_arch = "wasm32")]
                    {
                        let origin = web_sys::window()
                            .and_then(|w| w.location().origin().ok())
                            .unwrap_or_default();
                        link.set(format!("{origin}/register?key={token}"));
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    link.set(format!("/register?key={token}"));
                    code.set(token);
                }
                Err(e) => {
                    let s = e.to_string();
                    msg.set(Some(s.rsplit(": ").next().unwrap_or(&s).to_string()));
                }
            }
            busy.set(false);
        });
    };

    let copy_to_clipboard = |text: String| {
        if text.is_empty() {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(w) = web_sys::window() {
            let _ = w.navigator().clipboard().write_text(&text);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = text;
    };
    let copy_code = move |_| {
        copy_to_clipboard(code.get_untracked());
        copied.set(1);
    };
    let copy_link = move |_| {
        copy_to_clipboard(link.get_untracked());
        copied.set(2);
    };

    let field = "w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface";
    view! {
        <div class="ppm-card p-4 space-y-3">
            <h3 class="text-body-md font-bold text-on-background flex items-center gap-2">
                <span class="material-symbols-outlined text-primary">"person_add"</span>
                "Buat Link Registrasi"
            </h3>
            <p class="text-[11px] text-on-surface-variant">
                "Hanya yang punya link ini yang bisa mendaftar. Atur peran + KUOTA (berapa orang boleh pakai token yang sama) + masa berlaku. Mis. kuota 100 utk intake santri → satu link untuk semua."
            </p>
            {move || {
                msg.get()
                    .map(|t| {
                        view! {
                            <div class="p-2 bg-error-container text-on-error-container rounded-lg text-[11px]">
                                {t}
                            </div>
                        }
                    })
            }}
            <select
                class=field
                prop:value=move || role.get()
                on:change=move |ev| role.set(event_target_value(&ev))
            >
                // Suspense WAJIB: is_admin membaca resource sesi, dan membacanya
                // di luar sini memicu peringatan hydration mismatch Leptos.
                // Fallback = daftar tanpa peran staf; itu pilihan yang AMAN bila
                // sesi belum termuat (server tetap menolak lewat can_invite).
                <Suspense fallback=move || {
                    ROLES
                        .iter()
                        .filter(|(v, _)| !crate::models::is_staff_invite(v))
                        .map(|(v, l)| {
                            let val = v.to_string();
                            view! { <option value=val>{*l}</option> }
                        })
                        .collect_view()
                }>
                    {move || {
                        let admin = is_admin();
                        ROLES
                            .iter()
                            .filter(|(v, _)| admin || !crate::models::is_staff_invite(v))
                            .map(|(v, l)| {
                                let val = v.to_string();
                                view! { <option value=val>{*l}</option> }
                            })
                            .collect_view()
                    }}
                </Suspense>
            </select>
            <div class="flex gap-2">
                <label class="flex-1 space-y-1">
                    <span class="text-[11px] text-on-surface-variant">"Kuota (orang)"</span>
                    <input
                        type="number"
                        min="1"
                        max="1000"
                        class=field
                        prop:value=move || kuota.get().to_string()
                        on:input=move |ev| {
                            kuota.set(event_target_value(&ev).parse().unwrap_or(1).clamp(1, 1000))
                        }
                    />
                </label>
                <label class="flex-1 space-y-1">
                    <span class="text-[11px] text-on-surface-variant">"Berlaku (hari)"</span>
                    <input
                        type="number"
                        min="1"
                        max="30"
                        class=field
                        prop:value=move || hari.get().to_string()
                        on:input=move |ev| {
                            hari.set(event_target_value(&ev).parse().unwrap_or(7).clamp(1, 30))
                        }
                    />
                </label>
            </div>
            <button
                class="w-full py-2.5 rounded-lg bg-primary text-on-primary font-semibold text-body-sm press disabled:opacity-60"
                disabled=move || busy.get()
                on:click=generate
            >
                {move || {
                    if busy.get() {
                        "…".to_string()
                    } else {
                        format!("Buat Link (kuota {})", kuota.get())
                    }
                }}
            </button>
            {move || {
                let c = code.get();
                (!c.is_empty())
                    .then(|| {
                        view! {
                            // Kode referal (utama — bisa dibagikan langsung, calon
                            // pengguna tempel di halaman /register).
                            <div class="rounded-lg bg-secondary-container/50 p-2.5 space-y-1.5">
                                <p class="text-[11px] font-bold tracking-wider text-on-surface-variant uppercase">
                                    {move || format!("Kode Referal (kuota {} · {} hari)", kuota.get(), hari.get())}
                                </p>
                                <div class="flex items-center gap-2">
                                    <code class="flex-1 min-w-0 text-body-sm font-bold text-primary truncate">
                                        {c}
                                    </code>
                                    <button
                                        class="px-2.5 py-1 rounded-lg bg-primary text-on-primary text-[11px] font-semibold shrink-0 press"
                                        on:click=copy_code
                                    >
                                        {move || if copied.get() == 1 { "Tersalin ✓" } else { "Salin kode" }}
                                    </button>
                                </div>
                                // Alternatif: link langsung (kode sudah di dalamnya).
                                <div class="flex items-center gap-2 pt-1">
                                    <span class="material-symbols-outlined text-[14px] text-on-surface-variant">
                                        "link"
                                    </span>
                                    <code class="flex-1 min-w-0 text-[10px] text-on-surface-variant truncate">
                                        {move || link.get()}
                                    </code>
                                    <button
                                        class="text-[11px] font-semibold text-primary shrink-0"
                                        on:click=copy_link
                                    >
                                        {move || if copied.get() == 2 { "Tersalin ✓" } else { "Salin link" }}
                                    </button>
                                </div>
                            </div>
                        }
                    })
            }}
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
    // Kategori menentukan PERILAKU tap (migrasi 49) — gate_utama = keluar/masuk
    // area pondok, selainnya = absensi kelas. Default "custom" (absensi).
    let category = RwSignal::new("custom".to_string());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let (n, s, l, cat) = (
            name.get_untracked(),
            serial.get_untracked(),
            location.get_untracked(),
            category.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            // api_key dikosongkan → server generate otomatis (16 digit).
            match create_rfid_device_action(n, s, l, String::new(), cat).await {
                Ok(key) => {
                    // Tampilkan kuncinya SEKARANG — setelah ini hanya hash-nya
                    // yang tersimpan, jadi tak ada cara membacanya lagi.
                    msg.set(Some((
                        true,
                        format!("Perangkat dibuat. CATAT kuncinya: {key}"),
                    )));
                    name.set(String::new());
                    serial.set(String::new());
                    location.set(String::new());
                    category.set("custom".to_string());
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
                    .map(|(ok, t)| {
                        // Pesan sukses kini membawa KUNCI perangkat baru — harus
                        // menonjol, bukan disamarkan sebagai galat merah.
                        let cls = if ok {
                            "p-2.5 bg-success/10 text-success rounded-lg text-[11px] font-semibold break-all"
                        } else {
                            "p-2 bg-error-container text-on-error-container rounded-lg text-[11px]"
                        };
                        view! { <div class=cls>{t}</div> }
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
                                <select
                                    class=field
                                    prop:value=move || category.get()
                                    on:change=move |ev| category.set(event_target_value(&ev))
                                >
                                    {crate::models::DEVICE_CATEGORIES
                                        .iter()
                                        .map(|(v, l)| view! { <option value=*v>{*l}</option> })
                                        .collect_view()}
                                </select>
                                <p class="text-[11px] text-on-surface-variant">
                                    {move || {
                                        if category.get() == "gate_utama" {
                                            "Tap di perangkat ini menandai santri KELUAR/MASUK area pondok — bukan absensi kelas."
                                        } else {
                                            "Tap di perangkat ini dicatat sebagai absensi kelas sesuai jadwal santri saat itu."
                                        }
                                    }}
                                </p>
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
    let e_cat = RwSignal::new(d.category.clone());

    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        let (n, s, l, cat) = (
            e_name.get_untracked(),
            e_serial.get_untracked(),
            e_loc.get_untracked(),
            e_cat.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            if update_rfid_device_action(id, n, s, l, cat).await.is_ok() {
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
    let cat_ro = d.category.clone();
    let cat_label = crate::models::device_category_label(&d.category);
    // Gerbang utama diberi warna beda: perilakunya menyimpang dari perangkat
    // lain (keluar/masuk area, bukan absensi) — jangan sampai tertukar saat
    // admin menyapu daftar.
    let cat_cls = if crate::models::is_main_gate(&cat_ro) {
        "text-[10px] font-bold px-2 py-0.5 rounded-full bg-warning/15 text-warning shrink-0"
    } else {
        "text-[10px] font-bold px-2 py-0.5 rounded-full bg-surface-container-high text-on-surface-variant shrink-0"
    };
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
                            <select
                                class=field
                                prop:value=move || e_cat.get()
                                on:change=move |ev| e_cat.set(event_target_value(&ev))
                            >
                                {crate::models::DEVICE_CATEGORIES
                                    .iter()
                                    .map(|(v, l)| view! { <option value=*v>{*l}</option> })
                                    .collect_view()}
                            </select>
                            {move || {
                                (e_cat.get() == "gate_utama")
                                    .then(|| {
                                        view! {
                                            <p class="text-[11px] text-warning">
                                                "Tap di sini menandai KELUAR/MASUK area pondok — tidak dicatat sebagai absensi kelas."
                                            </p>
                                        }
                                    })
                            }}
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
                                <span class="material-symbols-outlined text-[18px]">
                                    {if crate::models::is_main_gate(&cat_ro) { "door_open" } else { "meeting_room" }}
                                </span>
                            </span>
                            <div class="flex-1 min-w-0">
                                <div class="flex items-center gap-1.5 min-w-0">
                                    <p class="text-body-sm font-semibold text-on-background truncate">{name}</p>
                                    <span class=cat_cls>{cat_label}</span>
                                </div>
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
            // api_key. Kini DISIMPAN SEBAGAI HASH (migrasi 53), jadi kunci asli
            // TIDAK bisa dibaca balik dari database — hanya tampil sekali,
            // tepat setelah dibuat/diganti. Yang lupa mencatatnya harus
            // menggantinya, bukan mengintipnya.
            <div class="flex items-center gap-2 rounded-lg bg-surface-container px-2.5 py-1.5">
                <span class="material-symbols-outlined text-[15px] text-on-surface-variant">"key"</span>
                <code class="flex-1 min-w-0 text-[11px] truncate"
                      class:text-primary=move || !key_shown.get().is_empty()
                      class:font-bold=move || !key_shown.get().is_empty()
                      class:text-on-surface-variant=move || key_shown.get().is_empty()>
                    {move || {
                        let k = key_shown.get();
                        if k.is_empty() { "tersimpan sebagai hash — tak bisa dilihat lagi".to_string() } else { k }
                    }}
                </code>
                <button
                    class="text-[11px] font-semibold text-primary shrink-0 disabled:opacity-50"
                    disabled=move || busy.get()
                    on:click=regen
                >
                    "Ganti key"
                </button>
            </div>
            {move || {
                (!key_shown.get().is_empty())
                    .then(|| {
                        view! {
                            <p class="text-[10px] text-warning flex items-start gap-1">
                                <span class="material-symbols-outlined text-[13px] shrink-0">"warning"</span>
                                "Catat sekarang — kunci ini tak bisa ditampilkan lagi setelah halaman ditutup."
                            </p>
                        }
                    })
            }}
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


/// Panel pemasangan kartu RFID (admin).
///
/// Nomor kartu TIDAK diketik — 10 digit terlalu rawan salah. Santri menempel
/// kartunya di mesin mana pun; kartu yang belum terdaftar muncul di sini
/// (titipan Redis, hidup 1 jam), lalu admin memilih pemiliknya.
#[component]
fn KartuPanel() -> impl IntoView {
    let pending = Resource::new(|| (), |_| async move { pending_cards_data().await });
    // Kartu yang sedang dipasangkan (None = belum ada yang dipilih).
    let picked = RwSignal::new(Option::<i64>::None);
    let q = RwSignal::new(String::new());
    let hits = RwSignal::new(Vec::<crate::models::UserPickItem>::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let cari = move |_| {
        let term = q.get_untracked();
        if term.trim().chars().count() < 2 {
            hits.set(Vec::new());
            return;
        }
        leptos::task::spawn_local(async move {
            hits.set(search_users_for_card(term).await.unwrap_or_default());
        });
    };

    let pasang = move |user_id: i64| {
        let Some(card) = picked.get_untracked() else { return };
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        leptos::task::spawn_local(async move {
            match assign_card_action(user_id, card).await {
                Ok(_) => {
                    msg.set(Some((true, "Kartu terpasang.".into())));
                    picked.set(None);
                    q.set(String::new());
                    hits.set(Vec::new());
                    pending.refetch();
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
                <span class="text-body-md font-bold text-on-background flex items-center gap-2">
                    <span class="material-symbols-outlined text-primary">"badge"</span>
                    "Pasang Kartu RFID"
                </span>
                <button
                    class="w-8 h-8 rounded-lg bg-surface-container text-primary flex items-center justify-center press"
                    on:click=move |_| pending.refetch()
                    aria-label="Muat ulang daftar kartu"
                >
                    <span class="material-symbols-outlined text-[18px]">"sync"</span>
                </button>
            </div>
            <p class="text-[11px] text-on-surface-variant">
                "Minta pemiliknya menempelkan kartu di mesin mana pun, lalu pilih di bawah. \
                 Kartu yang belum dipasang hilang sendiri setelah 1 jam."
            </p>

            {move || {
                msg.get()
                    .map(|(ok, t)| {
                        let cls = if ok {
                            "p-2 bg-success/10 text-success rounded-lg text-[11px]"
                        } else {
                            "p-2 bg-error-container text-on-error-container rounded-lg text-[11px]"
                        };
                        view! { <div class=cls>{t}</div> }
                    })
            }}

            <Suspense fallback=|| ()>
                {move || {
                    pending
                        .get()
                        .map(|res| match res {
                            Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                            Ok(list) if list.is_empty() => {
                                view! {
                                    <p class="text-body-sm text-on-surface-variant py-2 text-center">
                                        "Belum ada kartu baru ditempel."
                                    </p>
                                }
                                    .into_any()
                            }
                            Ok(list) => {
                                view! {
                                    <div class="space-y-1.5">
                                        {list
                                            .into_iter()
                                            .map(|c| {
                                                let card = c.card;
                                                let is_picked = move || picked.get() == Some(card);
                                                let cls = move || {
                                                    if is_picked() {
                                                        "w-full flex items-center gap-2 rounded-lg px-3 py-2 bg-primary text-on-primary press"
                                                    } else {
                                                        "w-full flex items-center gap-2 rounded-lg px-3 py-2 bg-surface-container text-on-surface press"
                                                    }
                                                };
                                                view! {
                                                    <button
                                                        class=cls
                                                        on:click=move |_| {
                                                            picked.set(if is_picked() { None } else { Some(card) });
                                                        }
                                                    >
                                                        <span class="material-symbols-outlined text-[18px]">"contactless"</span>
                                                        <span class="flex-1 min-w-0 text-left">
                                                            <span class="block text-body-sm font-semibold tabular-nums">
                                                                {card.to_string()}
                                                            </span>
                                                            <span class="block text-[10px] opacity-75 truncate">
                                                                {format!("{} • {}", c.device, c.when_label)}
                                                            </span>
                                                        </span>
                                                    </button>
                                                }
                                            })
                                            .collect_view()}
                                    </div>
                                }
                                    .into_any()
                            }
                        })
                }}
            </Suspense>

            // ── Pemilih pengguna: hanya muncul setelah kartu dipilih ─────────
            {move || {
                picked
                    .get()
                    .map(|card| {
                        view! {
                            <div class="space-y-2 pt-2 border-t border-outline-variant/40">
                                <p class="text-[11px] text-on-surface-variant">
                                    {format!("Pasang kartu {card} ke:")}
                                </p>
                                <input
                                    type="text"
                                    class=field
                                    placeholder="Cari nama, NIS, atau nomor HP…"
                                    prop:value=move || q.get()
                                    on:input=move |ev| {
                                        q.set(event_target_value(&ev));
                                        cari(());
                                    }
                                />
                                {move || {
                                    let list = hits.get();
                                    if list.is_empty() {
                                        return ().into_any();
                                    }
                                    view! {
                                        <div class="space-y-1">
                                            {list
                                                .into_iter()
                                                .map(|u| {
                                                    let uid = u.id;
                                                    // Peringatkan bila orang ini SUDAH punya kartu —
                                                    // memasang yang baru membuat kartu lamanya mati.
                                                    let punya = u.current_card > 0;
                                                    view! {
                                                        <button
                                                            class="w-full flex items-center gap-2 rounded-lg px-3 py-2 bg-surface-container text-left press disabled:opacity-50"
                                                            disabled=move || busy.get()
                                                            on:click=move |_| pasang(uid)
                                                        >
                                                            <div class="flex-1 min-w-0">
                                                                <p class="text-body-sm font-semibold text-on-background truncate">
                                                                    {u.full_name}
                                                                </p>
                                                                <p class="text-[10px] text-on-surface-variant truncate">
                                                                    {format!("{} • {}", u.role_label, u.nis)}
                                                                </p>
                                                            </div>
                                                            {punya
                                                                .then(|| {
                                                                    view! {
                                                                        <span class="text-[10px] font-bold text-warning bg-warning/15 px-2 py-0.5 rounded-full shrink-0">
                                                                            "ganti kartu"
                                                                        </span>
                                                                    }
                                                                })}
                                                        </button>
                                                    }
                                                })
                                                .collect_view()}
                                        </div>
                                    }
                                        .into_any()
                                }}
                            </div>
                        }
                    })
            }}
        </div>
    }
}
