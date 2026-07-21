//! web/pages/sesi_detail.rs — Detail satu sesi (/sesi/:id, STAF).
//!
//! Hero (ringkasan persisten) + TAB: Absensi, Kelola (mulai/akhiri + ganti
//! pengajar), Chat, Hafalan (kondisional — kategori mengaji), Rekaman. Dulu
//! semua bagian ditumpuk dalam satu scroll panjang (+ grid desktop 2/3-1/3) —
//! dipecah jadi tab krn terlalu padat dalam satu layar (audit UI: "cognitive
//! load tinggi, tak ada hierarchy"). Tab dipakai SERAGAM mobile+desktop (bukan
//! grid sidebar lagi) — lebih simpel & konsisten drpd layout ganda per-breakpoint.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use crate::models::{is_mengaji_category, HafalanItem, SessionAttRow, SessionChatItem, SessionDetailData};
use crate::web::api::{
    hafalan_of_class_action, log_hafalan_action, mark_session_present, session_detail_data,
    set_session_live, set_session_teacher_action,
};
use crate::web::components::{DeviceFrame, FetchError, MobileHeader};

#[component]
pub fn SesiDetailPage() -> impl IntoView {
    let params = use_params_map();
    let session_id =
        Memo::new(move |_| params.read().get("id").and_then(|s| s.parse::<i64>().ok()).unwrap_or(0));
    let data =
        Resource::new(move || session_id.get(), |id| async move { session_detail_data(id).await });

    view! {
        <Title text="Detail Sesi — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Detail Sesi" back_href="/sesi" />

                <div class="px-5 pt-5 space-y-4">
                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-3">
                                <div class="h-28 bg-surface-container rounded-2xl"></div>
                                <div class="h-12 bg-surface-container rounded-xl"></div>
                                <div class="h-64 bg-surface-container rounded-2xl md:max-w-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(d) => {
                                        view! { <DetailBody d=d refetch=move || data.refetch() /> }
                                            .into_any()
                                    }
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                })
                        }}
                    </Suspense>
                </div>
            </div>
        </DeviceFrame>
    }
}

#[component]
fn DetailBody(d: SessionDetailData, refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let session_id = d.id;
    let meta = format!("{} • {} • {}", d.class_name, d.category, d.teacher);
    let when = format!("{} • {}", d.date_label, d.time_label);
    let hadir_label = format!("{}/{} hadir", d.hadir, d.total);
    let is_cancelled = d.status_kind == "cancelled";
    let busy_id = RwSignal::new(Option::<i64>::None);
    // Tab aktif: "absensi" (default) | "kelola" | "hafalan" | "lainnya" (chat+rekaman).
    let tab = RwSignal::new("absensi".to_string());

    // ── Setoran Hafalan: hanya kelas kategori "Mengaji"/"Pengajian" ─────────
    let class_id = d.class_id;
    let show_hafalan = is_mengaji_category(&d.category);
    let hafalan_students: Vec<(i64, String)> =
        d.attendance.iter().map(|a| (a.user_id, a.name.clone())).collect();

    // ── Kontrol kelola sesi (dewan guru = guru, admin, pamong) ──────────────
    let is_live = d.status_kind == "ongoing";
    let is_finished = d.status_kind == "finished";
    let cur_teacher = d.teacher_id.unwrap_or(0);
    let teacher_options = StoredValue::new(d.teacher_options.clone());
    let busy_ctl = RwSignal::new(false);
    // Nonaktif HANYA untuk aksi MULAI (di luar jendela ±10 menit dari jadwal);
    // akhiri sesi selalu boleh. Alasan dari server (start_blocked_reason) —
    // WIB "now" hanya diketahui otoritatif di server.
    let blocked_reason = d.start_blocked_reason.clone();
    let start_disabled = !is_live && blocked_reason.is_some();
    let ctl_err = RwSignal::new(Option::<String>::None);
    let toggle_live = move |_| {
        if busy_ctl.get_untracked() {
            return;
        }
        busy_ctl.set(true);
        ctl_err.set(None);
        leptos::task::spawn_local(async move {
            if let Err(e) = set_session_live(session_id, !is_live).await {
                let s = e.to_string();
                ctl_err.set(Some(s.rsplit(": ").next().unwrap_or(&s).to_string()));
            }
            busy_ctl.set(false);
            refetch();
        });
    };
    let set_teacher = move |tid: i64| {
        if busy_ctl.get_untracked() {
            return;
        }
        busy_ctl.set(true);
        leptos::task::spawn_local(async move {
            let _ = set_session_teacher_action(session_id, tid).await;
            busy_ctl.set(false);
            refetch();
        });
    };

    view! {
        // ── Hero info sesi ──────────────────────────────────────────────────
        <div class="spiritual-gradient rounded-2xl p-5 text-on-primary shadow-lg shadow-primary/20 anim-in">
            <div class="flex items-center justify-between gap-2">
                <p class="text-body-lg font-bold">{d.title.clone()}</p>
                <span class="px-2.5 py-1 rounded-full bg-white/15 text-[10px] font-bold tracking-wider">
                    {d.status_label.clone()}
                </span>
            </div>
            <p class="text-body-sm opacity-85 mt-1">{meta}</p>
            <p class="text-body-sm opacity-85 flex items-center gap-1 mt-1">
                <span class="material-symbols-outlined text-[15px]">"schedule"</span>
                {when}
            </p>
            <div class="mt-3 flex items-center justify-between gap-2">
                <span class="px-3 py-1.5 rounded-lg bg-white/10 inline-flex items-center gap-1.5 text-body-sm font-semibold">
                    <span class="material-symbols-outlined text-[16px]">"how_to_reg"</span>
                    {hadir_label}
                </span>
                <a
                    href=format!("/sesi/{session_id}/live")
                    class="px-4 py-2 rounded-xl bg-primary-fixed text-primary font-bold text-body-sm flex items-center gap-1.5 press"
                >
                    <span class="material-symbols-outlined text-[18px]">"sensors"</span>
                    "Ruang Sesi"
                </a>
            </div>
        </div>

        // ── Tab bar ──────────────────────────────────────────────────────────
        <div
            class="grid gap-1 bg-surface-container rounded-xl p-1 anim-in"
            style=format!(
                "grid-template-columns:repeat({},minmax(0,1fr))",
                if show_hafalan { 4 } else { 3 },
            )
        >
            <TabBtn tab=tab value="absensi" icon="fact_check" label="Absensi" />
            <TabBtn tab=tab value="kelola" icon="tune" label="Kelola" />
            {show_hafalan
                .then(|| view! { <TabBtn tab=tab value="hafalan" icon="auto_stories" label="Hafalan" /> })}
            <TabBtn tab=tab value="lainnya" icon="more_horiz" label="Lainnya" />
        </div>

        <div class="md:max-w-2xl">
            {move || {
                match tab.get().as_str() {
                    "kelola" => {
                        view! {
                            // ── Kelola sesi: mulai/akhiri + pengajar ──────
                            <div class="ppm-card p-4 anim-in">
                                <label class="text-[11px] font-bold tracking-wider uppercase text-on-surface-variant">
                                    "Pengajar"
                                </label>
                                <select
                                    class="mt-1 w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface disabled:opacity-60"
                                    disabled=move || busy_ctl.get()
                                    on:change=move |ev| {
                                        set_teacher(event_target_value(&ev).parse().unwrap_or(0))
                                    }
                                >
                                    <option value="0" selected=cur_teacher == 0>
                                        "— Belum ditentukan —"
                                    </option>
                                    {teacher_options
                                        .get_value()
                                        .into_iter()
                                        .map(|o| {
                                            let val = o.id.to_string();
                                            let sel = o.id == cur_teacher;
                                            view! {
                                                <option value=val selected=sel>
                                                    {o.name}
                                                </option>
                                            }
                                        })
                                        .collect_view()}
                                </select>
                                {(!is_cancelled)
                                    .then(|| {
                                        let (label, icon, cls) = if is_live {
                                            (
                                                "Akhiri Sesi",
                                                "call_end",
                                                "mt-3 w-full py-3 rounded-xl bg-error text-on-error font-bold text-body-sm flex items-center justify-center gap-2 press disabled:opacity-50",
                                            )
                                        } else if is_finished {
                                            (
                                                "Mulai Ulang Sesi",
                                                "restart_alt",
                                                "mt-3 w-full py-3 rounded-xl bg-surface-container-highest text-on-background font-bold text-body-sm flex items-center justify-center gap-2 press disabled:opacity-50",
                                            )
                                        } else {
                                            (
                                                "Mulai Sesi",
                                                "play_circle",
                                                "mt-3 w-full py-3 rounded-xl bg-primary text-on-primary font-bold text-body-sm flex items-center justify-center gap-2 press disabled:opacity-50",
                                            )
                                        };
                                        view! {
                                            <button
                                                class=cls
                                                disabled=move || busy_ctl.get() || start_disabled
                                                on:click=toggle_live
                                            >
                                                <span class="material-symbols-outlined text-[18px]">{icon}</span>
                                                {label}
                                            </button>
                                        }
                                    })}
                                {
                                    // Dihitung SEKALI per render tab "kelola" (bukan di dalam
                                    // closure reaktif bersarang) — `blocked_reason` (String,
                                    // bukan Copy) tak bisa di-`move` dari closure luar yang
                                    // cuma pegang `&self` (Fn) ke closure dalam yang butuh
                                    // ownership sendiri; nilai lokal segar ini aman di-`move`.
                                    let reason_if_blocked =
                                        if start_disabled { blocked_reason.clone() } else { None };
                                    view! {
                                        {move || {
                                            ctl_err
                                                .get()
                                                .or_else(|| reason_if_blocked.clone())
                                                .map(|msg| {
                                                    view! {
                                                        <p class="mt-2 text-[11px] text-error bg-error-container/60 rounded-lg px-3 py-1.5">
                                                            {msg}
                                                        </p>
                                                    }
                                                })
                                        }}
                                    }
                                }
                            </div>
                        }
                            .into_any()
                    }
                    "hafalan" if show_hafalan => {
                        view! {
                            <HafalanPanel class_id=class_id students=hafalan_students.clone() />
                        }
                            .into_any()
                    }
                    "lainnya" => {
                        view! {
                            <div class="space-y-5">
                                // ── Chat sesi ──────────────────────────────
                                <div class="ppm-card p-4 anim-in">
                                    <div class="flex items-center gap-2 mb-3">
                                        <span class="material-symbols-outlined text-on-background">
                                            "forum"
                                        </span>
                                        <h2 class="text-body-lg font-bold text-on-background">
                                            "Chat Sesi"
                                        </h2>
                                    </div>
                                    {if d.chats.is_empty() {
                                        view! {
                                            <p class="text-body-sm text-on-surface-variant py-3">
                                                "Tidak ada chat di sesi ini."
                                            </p>
                                        }
                                            .into_any()
                                    } else {
                                        view! {
                                            <div class="space-y-2.5 max-h-80 overflow-y-auto">
                                                {d.chats
                                                    .iter()
                                                    .cloned()
                                                    .map(|c| view! { <ChatRow c=c /> })
                                                    .collect_view()}
                                            </div>
                                        }
                                            .into_any()
                                    }}
                                </div>

                                // ── Rekaman ────────────────────────────────
                                <div class="ppm-card p-4 anim-in">
                                    <div class="flex items-center gap-2 mb-2">
                                        <span class="material-symbols-outlined text-on-background">
                                            "play_circle"
                                        </span>
                                        <h2 class="text-body-lg font-bold text-on-background">
                                            "Rekaman"
                                        </h2>
                                    </div>
                                    <p class="text-body-sm text-on-surface-variant">
                                        {d.recording_label.clone()}
                                    </p>
                                    {d.recording_url
                                        .clone()
                                        .map(|url| {
                                            view! {
                                                <a
                                                    href=url
                                                    download=""
                                                    target="_blank"
                                                    rel="noopener"
                                                    class="mt-3 inline-flex items-center gap-2 px-4 py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-sm press"
                                                >
                                                    <span class="material-symbols-outlined text-[18px]">
                                                        "download"
                                                    </span>
                                                    "Unduh Rekaman"
                                                </a>
                                            }
                                        })}
                                </div>
                            </div>
                        }
                            .into_any()
                    }
                    _ => {
                        // "absensi" (default)
                        view! {
                            <div class="ppm-card p-4 anim-in">
                                {if d.attendance.is_empty() {
                                    view! {
                                        <p class="text-body-sm text-on-surface-variant py-3">
                                            "Belum ada anggota di kelas ini."
                                        </p>
                                    }
                                        .into_any()
                                } else {
                                    d.attendance
                                        .iter()
                                        .cloned()
                                        .map(|row| {
                                            view! {
                                                <AttRowView
                                                    row=row
                                                    session_id=session_id
                                                    can_mark=!is_cancelled
                                                    busy_id=busy_id
                                                    refetch=refetch
                                                />
                                            }
                                        })
                                        .collect_view()
                                        .into_any()
                                }}
                            </div>
                        }
                            .into_any()
                    }
                }
            }}
        </div>
    }
}

#[component]
fn TabBtn(
    tab: RwSignal<String>,
    value: &'static str,
    icon: &'static str,
    label: &'static str,
) -> impl IntoView {
    let cls = move || {
        if tab.get() == value {
            "py-2.5 rounded-lg bg-surface-container-lowest text-primary font-bold text-[11px] shadow-sm press flex flex-col items-center gap-0.5"
        } else {
            "py-2.5 rounded-lg text-on-surface-variant font-semibold text-[11px] press flex flex-col items-center gap-0.5"
        }
    };
    view! {
        <button class=cls on:click=move |_| tab.set(value.to_string())>
            <span class="material-symbols-outlined text-[18px]">{icon}</span>
            {label}
        </button>
    }
}

fn att_badge(kind: &str) -> &'static str {
    match kind {
        "present" => "ppm-chip-sm bg-success/10 text-success",
        "late" => "ppm-chip-sm bg-warning/10 text-warning",
        "absent" => "ppm-chip-sm bg-error-container text-error",
        "permit" | "sick" => "ppm-chip-sm bg-info/10 text-info",
        _ => "ppm-chip-sm bg-surface-container-highest text-on-surface-variant",
    }
}

#[component]
fn AttRowView(
    row: SessionAttRow,
    session_id: i64,
    can_mark: bool,
    busy_id: RwSignal<Option<i64>>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let badge = att_badge(&row.status_kind);
    let unrecorded = row.status_kind == "none";
    let sid = row.user_id;
    let initial: String = row.name.chars().next().map(|c| c.to_string()).unwrap_or_default();
    let mark = move |_| {
        if busy_id.get_untracked().is_some() {
            return;
        }
        busy_id.set(Some(sid));
        leptos::task::spawn_local(async move {
            let _ = mark_session_present(session_id, sid).await;
            busy_id.set(None);
            refetch();
        });
    };
    view! {
        <div class="flex items-center gap-3 py-2.5 border-b border-outline-variant/40 last:border-0">
            <div class="w-9 h-9 rounded-full bg-secondary-container text-primary flex items-center justify-center text-body-sm font-bold shrink-0">
                {initial}
            </div>
            <div class="flex-1 min-w-0">
                <p class="text-body-sm font-semibold text-on-background truncate">{row.name.clone()}</p>
                <p class="text-[11px] text-on-surface-variant">
                    {row.nis.clone()}
                    {(!row.time_label.is_empty()).then(|| format!(" • {}", row.time_label))}
                </p>
            </div>
            <span class=badge>{row.status_label.clone()}</span>
            {(unrecorded && can_mark)
                .then(|| {
                    view! {
                        <button
                            class="px-2.5 py-1.5 rounded-lg bg-primary text-on-primary text-[11px] font-bold press disabled:opacity-50"
                            disabled=move || busy_id.get() == Some(sid)
                            on:click=mark
                        >
                            "Tandai Hadir"
                        </button>
                    }
                })}
        </div>
    }
}

#[component]
fn ChatRow(c: SessionChatItem) -> impl IntoView {
    view! {
        <div class="bg-surface-container rounded-xl px-3 py-2">
            <div class="flex items-center justify-between gap-2">
                <p class="text-[12px] font-bold text-primary truncate">{c.name.clone()}</p>
                <p class="text-[10px] text-on-surface-variant shrink-0">{c.time_label.clone()}</p>
            </div>
            <p class="text-body-sm text-on-background break-words">{c.message.clone()}</p>
        </div>
    }
}

/// Panel Setoran Hafalan — kerangka laporan akademik kategori "Mengaji": staf
/// pilih santri (dari daftar absensi sesi ini) + catat surah/ayat/juz/kualitas.
/// Dipasang HANYA saat kategori efektif sesi (jadwal → kelas) cocok
/// `is_mengaji_category` (lihat models::hafalan). Muncul juga di rapor santri &
/// laporan ortu (riwayat) serta ranking "Santri Teladan" (laporan dewan guru).
#[component]
fn HafalanPanel(class_id: i64, students: Vec<(i64, String)>) -> impl IntoView {
    let entries = Resource::new(|| (), move |_| async move { hafalan_of_class_action(class_id).await });
    let students = StoredValue::new(students);
    let student_id = RwSignal::new(students.get_value().first().map(|s| s.0).unwrap_or(0));
    let surah = RwSignal::new(String::new());
    let ayat = RwSignal::new(String::new());
    let juz = RwSignal::new(String::new());
    let quality = RwSignal::new("lancar".to_string());
    let note = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() || student_id.get_untracked() == 0 {
            return;
        }
        let s = surah.get_untracked();
        if s.trim().is_empty() {
            msg.set(Some((false, "Nama surah wajib diisi.".into())));
            return;
        }
        busy.set(true);
        msg.set(None);
        let juz_n = juz.get_untracked().trim().parse::<i16>().ok();
        leptos::task::spawn_local(async move {
            match log_hafalan_action(
                student_id.get_untracked(),
                Some(class_id),
                s,
                ayat.get_untracked(),
                juz_n,
                quality.get_untracked(),
                note.get_untracked(),
            )
            .await
            {
                Ok(_) => {
                    surah.set(String::new());
                    ayat.set(String::new());
                    juz.set(String::new());
                    note.set(String::new());
                    msg.set(Some((true, "Setoran tersimpan.".into())));
                    entries.refetch();
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
        <div class="ppm-card p-4 anim-in">
            <div class="flex items-center gap-2 mb-3">
                <span class="material-symbols-outlined text-primary">"auto_stories"</span>
                <h2 class="text-body-lg font-bold text-on-background">"Setoran Hafalan"</h2>
            </div>

            <form class="space-y-2" method="post" on:submit=submit>
                {move || {
                    msg.get()
                        .map(|(ok, t)| {
                            let cls = if ok {
                                "p-2 bg-secondary-container text-on-secondary-container rounded-lg text-body-sm"
                            } else {
                                "p-2 bg-error-container text-on-error-container rounded-lg text-body-sm"
                            };
                            view! { <div class=cls>{t}</div> }
                        })
                }}
                <select
                    class=field
                    on:change=move |ev| student_id.set(event_target_value(&ev).parse().unwrap_or(0))
                >
                    {students
                        .get_value()
                        .into_iter()
                        .map(|(id, name)| view! { <option value=id.to_string()>{name}</option> })
                        .collect_view()}
                </select>
                <div class="grid grid-cols-2 gap-2">
                    <input
                        type="text"
                        class=field
                        placeholder="Surah (mis. An-Naba')"
                        prop:value=move || surah.get()
                        on:input=move |ev| surah.set(event_target_value(&ev))
                    />
                    <input
                        type="text"
                        class=field
                        placeholder="Ayat (mis. 1-40)"
                        prop:value=move || ayat.get()
                        on:input=move |ev| ayat.set(event_target_value(&ev))
                    />
                </div>
                <div class="grid grid-cols-2 gap-2">
                    <input
                        type="number"
                        min="1"
                        max="30"
                        class=field
                        placeholder="Juz (opsional)"
                        prop:value=move || juz.get()
                        on:input=move |ev| juz.set(event_target_value(&ev))
                    />
                    <select class=field on:change=move |ev| quality.set(event_target_value(&ev))>
                        <option value="lancar">"Lancar"</option>
                        <option value="perlu_perbaikan">"Perlu Perbaikan"</option>
                        <option value="mengulang">"Mengulang"</option>
                    </select>
                </div>
                <input
                    type="text"
                    class=field
                    placeholder="Catatan (opsional)"
                    prop:value=move || note.get()
                    on:input=move |ev| note.set(event_target_value(&ev))
                />
                <button
                    type="submit"
                    class="w-full py-2.5 rounded-lg bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                    disabled=move || busy.get()
                >
                    {move || if busy.get() { "Menyimpan…" } else { "Simpan Setoran" }}
                </button>
            </form>

            <Suspense fallback=|| ()>
                {move || {
                    entries
                        .get()
                        .map(|res| match res {
                            Ok(rows) if rows.is_empty() => ().into_any(),
                            Ok(rows) => {
                                view! {
                                    <div class="mt-3 pt-3 border-t border-outline-variant/40 space-y-1.5">
                                        {rows
                                            .into_iter()
                                            .map(|(name, h): (String, HafalanItem)| {
                                                let range = if h.ayat_range.is_empty() {
                                                    h.surah.clone()
                                                } else {
                                                    format!("{} ({})", h.surah, h.ayat_range)
                                                };
                                                view! {
                                                    <div class="flex items-center justify-between gap-2 text-body-sm">
                                                        <span class="text-on-background truncate">
                                                            {name} " — " {range}
                                                        </span>
                                                        <span class="text-[11px] text-on-surface-variant shrink-0">
                                                            {h.date_label}
                                                        </span>
                                                    </div>
                                                }
                                            })
                                            .collect_view()}
                                    </div>
                                }
                                    .into_any()
                            }
                            Err(_) => ().into_any(),
                        })
                }}
            </Suspense>
        </div>
    }
}
