//! web/pages/sesi_detail.rs — Detail satu sesi (/sesi/:id, STAF).
//!
//! Tiga bagian: ABSENSI (anggota kelas + status di sesi ini + "Tandai Hadir"
//! manual → masuk antrean verifikasi normal), CHAT (transkrip), REKAMAN
//! (tombol unduh bila recording_path terisi; pipeline rekaman menyusul).

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use crate::models::{SessionAttRow, SessionChatItem, SessionDetailData};
use crate::web::api::{mark_session_present, session_detail_data};
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
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto">
                <MobileHeader title="Detail Sesi" back_href="/sesi" />

                <div class="px-5 pt-5 space-y-4">
                    <Suspense fallback=|| {
                        view! {
                            <div class="space-y-3 animate-pulse">
                                <div class="h-28 bg-surface-container rounded-2xl"></div>
                                <div class="h-40 bg-surface-container rounded-2xl"></div>
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
    let meta = format!("{} • {}", d.class_name, d.teacher);
    let when = format!("{} • {}", d.date_label, d.time_label);
    let hadir_label = format!("{}/{} hadir", d.hadir, d.total);
    let is_cancelled = d.status_kind == "cancelled";
    let busy_id = RwSignal::new(Option::<i64>::None);

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

        // ── Absensi ─────────────────────────────────────────────────────────
        <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4 anim-in">
            <div class="flex items-center gap-2 mb-3">
                <span class="material-symbols-outlined text-on-background">"fact_check"</span>
                <h2 class="text-body-lg font-bold text-on-background">"Absensi"</h2>
            </div>
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

        // ── Chat sesi ───────────────────────────────────────────────────────
        <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4 anim-in">
            <div class="flex items-center gap-2 mb-3">
                <span class="material-symbols-outlined text-on-background">"forum"</span>
                <h2 class="text-body-lg font-bold text-on-background">"Chat Sesi"</h2>
            </div>
            {if d.chats.is_empty() {
                view! {
                    <p class="text-body-sm text-on-surface-variant py-3">"Tidak ada chat di sesi ini."</p>
                }
                    .into_any()
            } else {
                view! {
                    <div class="space-y-2.5 max-h-80 overflow-y-auto">
                        {d.chats.iter().cloned().map(|c| view! { <ChatRow c=c /> }).collect_view()}
                    </div>
                }
                    .into_any()
            }}
        </div>

        // ── Rekaman ─────────────────────────────────────────────────────────
        <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-2xl p-4 anim-in">
            <div class="flex items-center gap-2 mb-2">
                <span class="material-symbols-outlined text-on-background">"play_circle"</span>
                <h2 class="text-body-lg font-bold text-on-background">"Rekaman"</h2>
            </div>
            <p class="text-body-sm text-on-surface-variant">{d.recording_label.clone()}</p>
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
                            <span class="material-symbols-outlined text-[18px]">"download"</span>
                            "Unduh Rekaman"
                        </a>
                    }
                })}
        </div>
    }
}

fn att_badge(kind: &str) -> &'static str {
    match kind {
        "present" => "px-2 py-0.5 rounded-full text-[10px] font-bold tracking-wider bg-success/10 text-success",
        "late" => "px-2 py-0.5 rounded-full text-[10px] font-bold tracking-wider bg-warning/10 text-warning",
        "absent" => "px-2 py-0.5 rounded-full text-[10px] font-bold tracking-wider bg-error-container text-error",
        "permit" | "sick" => "px-2 py-0.5 rounded-full text-[10px] font-bold tracking-wider bg-info/10 text-info",
        _ => "px-2 py-0.5 rounded-full text-[10px] font-bold tracking-wider bg-surface-container-highest text-on-surface-variant",
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
