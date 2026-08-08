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

use crate::models::{HafalanItem, SessionAttRow, SessionChatItem, SessionDetailData};
use crate::web::api::{
    correct_attendance_bulk_action, decide_session_action, hafalan_of_class_action,
    log_hafalan_action,
    send_schedule_wa_action, session_detail_data, session_verify_data,
    set_session_actual_detail_action, set_session_book_action, set_session_live,
    set_session_pamong_action, set_session_target_action, set_session_teacher_action,
};
use crate::web::components::{DeviceFrame, FetchError, MobileHeader, SwipeArea};

#[component]
pub fn SesiDetailPage() -> impl IntoView {
    let params = use_params_map();
    let session_id =
        Memo::new(move |_| params.read().get("id").and_then(|s| s.parse::<i64>().ok()).unwrap_or(0));
    let data =
        Resource::new(move || session_id.get(), |id| async move { session_detail_data(id).await });

    view! {
        <Title text="Detail Sesi — AFM SMART" />
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
    // Tab aktif: "absensi" (default) | "kelola" | "hafalan" | "lainnya" (chat+rekaman).
    let tab = RwSignal::new("absensi".to_string());

    // ── Setoran Hafalan: hanya kelas kategori "Mengaji"/"Pengajian" ─────────
    let class_id = d.class_id;
    // Hafalan HANYA di kelas Bacaan Al-Quran. Dulu gerbangnya
    // `is_mengaji_category` atas kategori JADWAL — teks bebas, dan jadwal KBM
    // yang kebetulan berjudul "Pengajian KBM Malam" ikut memunculkan panel
    // setoran hafalan di kelas yang tak pernah menyetor hafalan.
    let show_hafalan = d.class_category == "bacaan";
    // Urutan tab yang tampil — sumber tunggal untuk gestur geser dan bilah tab.
    let urutan_tab = move || -> Vec<&'static str> {
        if show_hafalan {
            vec!["absensi", "kelola", "hafalan", "lainnya"]
        } else {
            vec!["absensi", "kelola", "lainnya"]
        }
    };
    let hafalan_students: Vec<(i64, String)> =
        d.attendance.iter().map(|a| (a.user_id, a.name.clone())).collect();

    // ── Kontrol kelola sesi (dewan guru = guru, admin, pamong) ──────────────
    let is_live = d.status_kind == "ongoing";
    let is_finished = d.status_kind == "finished";
    let cur_teacher = d.teacher_id.unwrap_or(0);
    let teacher_options = StoredValue::new(d.teacher_options.clone());
    let cur_pamong = d.pamong_id.unwrap_or(0);
    let pamong_options = StoredValue::new(d.pamong_options.clone());
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
                                ctl_err.set(Some(crate::web::components::pesan_galat(e)));
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
    let set_pamong = move |pid: i64| {
        if busy_ctl.get_untracked() {
            return;
        }
        busy_ctl.set(true);
        leptos::task::spawn_local(async move {
            let _ = set_session_pamong_action(session_id, pid).await;
            busy_ctl.set(false);
            refetch();
        });
    };

    // ── Materi buku (migrasi 20+41): TARGET (rencana) + AKTUAL (dibahas) ──────
    let book_options = StoredValue::new(d.book_options.clone());
    let book_meta = d
        .book_title
        .clone()
        .map(|t| if d.book_pages_label.is_empty() { t } else { format!("{t} · hal {}", d.book_pages_label) });

    // TARGET (rencana materi).
    let target_sel = RwSignal::new(d.target_book_id.unwrap_or(0));
    let target_pages_sel = RwSignal::new(d.target_pages_label.clone());
    let busy_target = RwSignal::new(false);
    let target_msg = RwSignal::new(Option::<(bool, String)>::None);
    let save_target = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy_target.get_untracked() {
            return;
        }
        busy_target.set(true);
        target_msg.set(None);
        let (bk, bp) = (target_sel.get_untracked(), target_pages_sel.get_untracked());
        leptos::task::spawn_local(async move {
            match set_session_target_action(session_id, bk, bp).await {
                Ok(_) => {
                    target_msg.set(Some((true, "Target materi tersimpan.".into())));
                    refetch();
                }
                Err(e) => {
                    target_msg.set(Some((false, crate::web::components::pesan_galat(e))));
                }
            }
            busy_target.set(false);
        });
    };

    // AKTUAL (materi + halaman + catatan ayat/hadith yang benar-benar dibahas).
    let book_sel = RwSignal::new(d.book_id.unwrap_or(0));
    let book_pages_sel = RwSignal::new(d.book_pages_label.clone());
    let actual_detail_sel = RwSignal::new(d.actual_detail.clone());
    let busy_book = RwSignal::new(false);
    let book_msg = RwSignal::new(Option::<(bool, String)>::None);
    let save_book = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy_book.get_untracked() {
            return;
        }
        busy_book.set(true);
        book_msg.set(None);
        let (bk, bp, det) =
            (book_sel.get_untracked(), book_pages_sel.get_untracked(), actual_detail_sel.get_untracked());
        leptos::task::spawn_local(async move {
            // Simpan buku+halaman lalu catatan ayat/hadith (dua kolom).
            let res = match set_session_book_action(session_id, bk, bp).await {
                Ok(_) => set_session_actual_detail_action(session_id, det).await,
                Err(e) => Err(e),
            };
            match res {
                Ok(_) => {
                    book_msg.set(Some((true, "Materi aktual tersimpan.".into())));
                    refetch();
                }
                Err(e) => {
                    book_msg.set(Some((false, crate::web::components::pesan_galat(e))));
                }
            }
            busy_book.set(false);
        });
    };

    // ── Kirim jadwal ke Google Calendar santri (via WhatsApp) ───────────────
    let busy_wa = RwSignal::new(false);
    let wa_msg = RwSignal::new(Option::<(bool, String)>::None);
    let send_wa = move |_| {
        if busy_wa.get_untracked() {
            return;
        }
        busy_wa.set(true);
        wa_msg.set(None);
        leptos::task::spawn_local(async move {
            match send_schedule_wa_action(session_id).await {
                Ok((sent, skipped)) => {
                    let mut m = format!("Tautan jadwal terkirim ke {sent} santri.");
                    if skipped > 0 {
                        m.push_str(&format!(" {skipped} dilewati (belum ada nomor HP)."));
                    }
                    wa_msg.set(Some((true, m)));
                }
                Err(e) => {
                    wa_msg.set(Some((false, crate::web::components::pesan_galat(e))));
                }
            }
            busy_wa.set(false);
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
            {book_meta
                .clone()
                .map(|m| {
                    view! {
                        <p class="text-body-sm opacity-85 flex items-center gap-1 mt-1">
                            <span class="material-symbols-outlined text-[15px]">"menu_book"</span>
                            {m}
                        </p>
                    }
                })}
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

        // Absensi (default) diisi full-width ppm-wide (daftar baris skala baik
        // di layar lebar); panel lain (form/chat) tetap dibatasi md:max-w-* per
        // arm — dulu SEMUA tab dibatasi md:max-w-2xl termasuk absensi, hasilnya
        // ruang kosong besar di kanan pada layar lebar (audit UI user).
        // Geser kiri/kanan = pindah tab, seperti kalender bulanan. Daftar
        // tabnya dihitung dari yang BENAR-BENAR tampil (Hafalan hanya ada di
        // kelas Bacaan), jadi geser tak pernah mendarat di tab yang tak ada.
        <SwipeArea
            on_prev=move || {
                let urut = urutan_tab();
                let i = urut.iter().position(|t| *t == tab.get_untracked()).unwrap_or(0);
                if i > 0 {
                    tab.set(urut[i - 1].to_string());
                }
            }
            on_next=move || {
                let urut = urutan_tab();
                let i = urut.iter().position(|t| *t == tab.get_untracked()).unwrap_or(0);
                if i + 1 < urut.len() {
                    tab.set(urut[i + 1].to_string());
                }
            }
        >
            {move || {
                match tab.get().as_str() {
                    "kelola" => {
                        view! {
                            // `.ppm-masonry`: satu kolom di ponsel, DUA kolom
                            // seimbang di desktop. Tab ini berisi empat kartu
                            // mandiri (pengajar/pamong, materi target, kitab,
                            // catatan) yang sebelumnya bertumpuk di kolom
                            // `md:max-w-md` dengan separuh layar kanan kosong.
                            <div class="ppm-masonry">
                            // ── Kelola sesi: mulai/akhiri + pengajar ──────
                            <div class="ppm-card p-4 anim-in">
                                <label class="text-[11px] font-bold tracking-wider uppercase text-on-surface-variant">
                                    "Pengajar (ustad bertugas)"
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
                                // ── Pamong bertugas verifikasi (migrasi 33) ──
                                <label class="mt-3 block text-[11px] font-bold tracking-wider uppercase text-on-surface-variant">
                                    "Pamong bertugas (verifikasi)"
                                </label>
                                <select
                                    class="mt-1 w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface disabled:opacity-60"
                                    disabled=move || busy_ctl.get()
                                    on:change=move |ev| {
                                        set_pamong(event_target_value(&ev).parse().unwrap_or(0))
                                    }
                                >
                                    <option value="0" selected=cur_pamong == 0>
                                        "— Pakai pamong kelas —"
                                    </option>
                                    {pamong_options
                                        .get_value()
                                        .into_iter()
                                        .map(|o| {
                                            let val = o.id.to_string();
                                            let sel = o.id == cur_pamong;
                                            view! { <option value=val selected=sel>{o.name}</option> }
                                        })
                                        .collect_view()}
                                </select>
                                <p class="mt-1 text-[11px] text-on-surface-variant">
                                    "Pamong kelas menugaskan ustad & pamong verifikator sesi ini (±1 jam sebelum mulai)."
                                </p>
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

                            // ── Materi TARGET / rencana (migrasi 41) ──────
                            <form class="ppm-card p-4 space-y-2 anim-in" method="post" on:submit=save_target>
                                <div class="flex items-center gap-2">
                                    <span class="material-symbols-outlined text-primary text-[18px]">"assignment"</span>
                                    <label class="text-[11px] font-bold tracking-wider uppercase text-on-surface-variant">
                                        "Materi Target (Rencana)"
                                    </label>
                                </div>
                                <select
                                    class="w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface disabled:opacity-60"
                                    disabled=move || busy_target.get()
                                    on:change=move |ev| {
                                        target_sel.set(event_target_value(&ev).parse().unwrap_or(0))
                                    }
                                >
                                    <option value="0" selected=move || target_sel.get() == 0>
                                        "— Tanpa target —"
                                    </option>
                                    {book_options
                                        .get_value()
                                        .into_iter()
                                        .map(|o| {
                                            let val = o.id.to_string();
                                            let sel = move || target_sel.get() == o.id;
                                            view! { <option value=val selected=sel>{o.title}</option> }
                                        })
                                        .collect_view()}
                                </select>
                                {move || {
                                    (target_sel.get() > 0)
                                        .then(|| {
                                            view! {
                                                <input
                                                    type="text"
                                                    class="w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface"
                                                    placeholder="Halaman/ayat rencana (mis. 11-20)"
                                                    prop:value=move || target_pages_sel.get()
                                                    on:input=move |ev| target_pages_sel.set(event_target_value(&ev))
                                                />
                                            }
                                        })
                                }}
                                {move || {
                                    target_msg
                                        .get()
                                        .map(|(ok, t)| {
                                            let cls = if ok {
                                                "p-2 bg-secondary-container text-on-secondary-container rounded-lg text-[11px]"
                                            } else {
                                                "p-2 bg-error-container text-on-error-container rounded-lg text-[11px]"
                                            };
                                            view! { <div class=cls>{t}</div> }
                                        })
                                }}
                                <button
                                    type="submit"
                                    class="w-full py-2.5 rounded-lg bg-secondary-container text-primary font-semibold text-body-sm disabled:opacity-60"
                                    disabled=move || busy_target.get()
                                >
                                    {move || if busy_target.get() { "Menyimpan…" } else { "Simpan Target" }}
                                </button>
                            </form>

                            // ── Materi AKTUAL / dibahas (migrasi 20+41) ────
                            <form class="ppm-card p-4 space-y-2 anim-in" method="post" on:submit=save_book>
                                <div class="flex items-center gap-2">
                                    <span class="material-symbols-outlined text-primary text-[18px]">"task_alt"</span>
                                    <label class="text-[11px] font-bold tracking-wider uppercase text-on-surface-variant">
                                        "Materi Aktual (Dibahas)"
                                    </label>
                                </div>
                                <select
                                    class="w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface disabled:opacity-60"
                                    disabled=move || busy_book.get()
                                    on:change=move |ev| {
                                        book_sel.set(event_target_value(&ev).parse().unwrap_or(0))
                                    }
                                >
                                    <option value="0" selected=move || book_sel.get() == 0>
                                        "— Tanpa materi —"
                                    </option>
                                    {book_options
                                        .get_value()
                                        .into_iter()
                                        .map(|o| {
                                            let val = o.id.to_string();
                                            let sel = move || book_sel.get() == o.id;
                                            view! { <option value=val selected=sel>{o.title}</option> }
                                        })
                                        .collect_view()}
                                </select>
                                {move || {
                                    (book_sel.get() > 0)
                                        .then(|| {
                                            view! {
                                                <input
                                                    type="text"
                                                    class="w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface"
                                                    placeholder="Halaman yang dibahas (mis. 11-20, 45-50)"
                                                    prop:value=move || book_pages_sel.get()
                                                    on:input=move |ev| book_pages_sel.set(event_target_value(&ev))
                                                />
                                            }
                                        })
                                }}
                                <textarea
                                    class="w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface"
                                    rows="2"
                                    placeholder="Ayat/hadith yang benar-benar dibahas (mis. An-Naba' 1-20 / Hadith no. 5-8)"
                                    prop:value=move || actual_detail_sel.get()
                                    on:input=move |ev| actual_detail_sel.set(event_target_value(&ev))
                                ></textarea>
                                {move || {
                                    book_msg
                                        .get()
                                        .map(|(ok, t)| {
                                            let cls = if ok {
                                                "p-2 bg-secondary-container text-on-secondary-container rounded-lg text-[11px]"
                                            } else {
                                                "p-2 bg-error-container text-on-error-container rounded-lg text-[11px]"
                                            };
                                            view! { <div class=cls>{t}</div> }
                                        })
                                }}
                                <button
                                    type="submit"
                                    class="w-full py-2.5 rounded-lg bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                                    disabled=move || busy_book.get()
                                >
                                    {move || if busy_book.get() { "Menyimpan…" } else { "Simpan Materi Aktual" }}
                                </button>
                            </form>

                            // ── Kirim jadwal ke Google Calendar santri ──────
                            <div class="ppm-card p-4 space-y-2 anim-in">
                                <div class="flex items-center gap-2">
                                    <span class="material-symbols-outlined text-primary">"event_available"</span>
                                    <h3 class="text-body-md font-bold text-on-background">
                                        "Kirim ke Google Calendar"
                                    </h3>
                                </div>
                                <p class="text-[11px] text-on-surface-variant">
                                    "Kirim tautan jadwal sesi ini ke WhatsApp semua santri kelas. "
                                    "Santri tap tautannya → langsung tersimpan di Google Calendar mereka."
                                </p>
                                {move || {
                                    wa_msg
                                        .get()
                                        .map(|(ok, t)| {
                                            let cls = if ok {
                                                "p-2 bg-secondary-container text-on-secondary-container rounded-lg text-[11px]"
                                            } else {
                                                "p-2 bg-error-container text-on-error-container rounded-lg text-[11px]"
                                            };
                                            view! { <div class=cls>{t}</div> }
                                        })
                                }}
                                <button
                                    class="w-full py-2.5 rounded-lg bg-primary text-on-primary font-semibold text-body-sm flex items-center justify-center gap-2 press disabled:opacity-60"
                                    disabled=move || busy_wa.get()
                                    on:click=send_wa
                                >
                                    <span class="material-symbols-outlined text-[18px]">"send"</span>
                                    {move || {
                                        if busy_wa.get() { "Mengirim…" } else { "Kirim jadwal ke WA santri" }
                                    }}
                                </button>
                            </div>
                            </div>
                        }
                            .into_any()
                    }
                    "hafalan" if show_hafalan => {
                        view! {
                            <div class="md:max-w-xl">
                                <HafalanPanel class_id=class_id students=hafalan_students.clone() />
                            </div>
                        }
                            .into_any()
                    }
                    "lainnya" => {
                        view! {
                            <div class="space-y-5 md:max-w-2xl">
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
                        // "absensi" (default) — panel tandai + panel verifikasi sesi.
                        view! {
                            <AbsensiVerifikasiPanel
                                attendance=d.attendance.clone()
                                session_id=session_id
                                can_correct=d.can_correct && !is_cancelled
                                refetch=refetch
                            />
                        }
                            .into_any()
                    }
                }
            }}
        </SwipeArea>
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

/// Panel absensi sesi dgn AKSI MASSAL: centang banyak santri (yang belum
/// tercatat) → "Tandai Hadir" atau "Alpa-kan" sekaligus. Tetap ada tombol

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
                    msg.set(Some((false, crate::web::components::pesan_galat(e))));
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

/// Panel verifikasi kehadiran PER-SESI (pamong tahap-1 / dewan guru final).
/// Centang = setujui (default); hilangkan centang = tolak. Satu klik memproses

/// Panel GABUNGAN absensi + verifikasi satu sesi.
///
/// Dua hal yang sengaja berbeda dari versi sebelumnya:
///
/// 1. **Tak ada request per klik.** Dulu menekan "Hadir"/"Terlambat"/… langsung
///    menembak server untuk SATU santri — layar berkedip tiap klik, dan
///    membetulkan lima santri berarti lima request yang bisa gagal separuh
///    jalan. Sekarang semua perubahan ditampung di klien dan dikirim SEKALI
///    lewat `correct_attendance_bulk_action`.
///
/// 2. **Absensi & verifikasi jadi satu kartu.** Keduanya membahas daftar orang
///    yang sama pada sesi yang sama; memisahkannya memaksa mata bolak-balik
///    dan membuat "sudah kuubah statusnya, kenapa masih menunggu?" jadi
///    pertanyaan wajar.
///
/// Jam masuk boleh diisi: server yang memutuskan hadir/terlambat dari
/// `limit_entery_time` jadwal — bukan tombol yang ditekan petugas.
#[component]
fn AbsensiVerifikasiPanel(
    attendance: Vec<SessionAttRow>,
    session_id: i64,
    can_correct: bool,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    use std::collections::HashMap;

    let verify = Resource::new(
        move || session_id,
        |id| async move { session_verify_data(id).await },
    );

    // Perubahan yang BELUM dikirim: user_id → (status, jam "HH:MM").
    let edits = RwSignal::new(HashMap::<i64, (String, String)>::new());
    let reject = RwSignal::new(Vec::<i64>::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);
    // Konfirmasi sebelum verifikasi — final tak bisa diulang.
    let konfirmasi = RwSignal::new(false);

    let awal = StoredValue::new(
        attendance
            .iter()
            .map(|r| {
                let jam = r.time_label.split_whitespace().next().unwrap_or("").to_string();
                (r.user_id, (r.status_kind.clone(), jam))
            })
            .collect::<HashMap<i64, (String, String)>>(),
    );

    let set_status = move |uid: i64, st: String| {
        edits.update(|m| {
            let jam = m
                .get(&uid)
                .map(|(_, j)| j.clone())
                .or_else(|| awal.get_value().get(&uid).map(|(_, j)| j.clone()))
                .unwrap_or_default();
            m.insert(uid, (st, jam));
        });
    };
    let set_jam = move |uid: i64, jam: String| {
        edits.update(|m| {
            let st = m
                .get(&uid)
                .map(|(s, _)| s.clone())
                .or_else(|| awal.get_value().get(&uid).map(|(s, _)| s.clone()))
                .unwrap_or_else(|| "present".to_string());
            m.insert(uid, (st, jam));
        });
    };

    let jumlah_ubah = move || edits.get().len();

    let simpan = move |_| {
        if busy.get_untracked() {
            return;
        }
        let items: Vec<crate::models::KoreksiAbsensi> = edits
            .get_untracked()
            .into_iter()
            .map(|(user_id, (status, jam))| crate::models::KoreksiAbsensi { user_id, status, jam })
            .collect();
        if items.is_empty() {
            msg.set(Some((false, "Belum ada perubahan.".into())));
            return;
        }
        busy.set(true);
        msg.set(None);
        leptos::task::spawn_local(async move {
            match correct_attendance_bulk_action(session_id, items).await {
                Ok(n) => {
                    msg.set(Some((true, format!("{n} kehadiran tersimpan. Poin lama ditarik."))));
                    edits.set(HashMap::new());
                    verify.refetch();
                    refetch();
                }
                Err(e) => {
                    msg.set(Some((false, crate::web::components::pesan_galat(e))));
                }
            }
            busy.set(false);
        });
    };

    let verifikasi = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        konfirmasi.set(false);
        msg.set(None);
        let rej = reject.get_untracked();
        leptos::task::spawn_local(async move {
            match decide_session_action(session_id, rej).await {
                Ok(n) => {
                    msg.set(Some((true, format!("{n} kehadiran diverifikasi."))));
                    reject.set(Vec::new());
                    verify.refetch();
                    refetch();
                }
                Err(e) => {
                    msg.set(Some((false, crate::web::components::pesan_galat(e))));
                }
            }
            busy.set(false);
        });
    };

    view! {
        <div class="ppm-card p-4 space-y-3">
            <div class="flex items-center justify-between gap-2">
                <div class="flex items-center gap-2">
                    <span class="material-symbols-outlined text-primary">"fact_check"</span>
                    <h3 class="text-body-md font-bold text-on-background">"Absensi & Verifikasi"</h3>
                </div>
                {move || {
                    let n = jumlah_ubah();
                    (n > 0)
                        .then(|| {
                            view! {
                                <span class="ppm-chip bg-warning/15 text-warning">
                                    {format!("{n} belum disimpan")}
                                </span>
                            }
                        })
                }}
            </div>

            {move || {
                msg.get()
                    .map(|(ok, t)| {
                        let cls = if ok {
                            "p-2.5 bg-secondary-container text-on-secondary-container rounded-lg text-body-sm"
                        } else {
                            "p-2.5 bg-error-container text-on-error-container rounded-lg text-body-sm"
                        };
                        view! { <div class=cls>{t}</div> }
                    })
            }}

            <div class="divide-y divide-outline-variant/40">
                {attendance
                    .into_iter()
                    .map(|r| {
                        let uid = r.user_id;
                        // Sudah pindah kelas, tapi PERNAH tercatat di sesi ini.
                        // Barisnya tetap ditampilkan supaya riwayat sesi lampau
                        // tak berlubang; koreksinya memang tak bisa lagi dari
                        // sini (pagar keanggotaan di repository).
                        let bukan_anggota = !r.masih_anggota;
                        let jam_awal = r.time_label.split_whitespace().next().unwrap_or("").to_string();
                        let inisial = r.name.chars().next().unwrap_or('S').to_string().to_uppercase();
                        let nis = if r.nis.is_empty() { "-".to_string() } else { r.nis.clone() };
                        let status_now = move || {
                            edits
                                .get()
                                .get(&uid)
                                .map(|(s, _)| s.clone())
                                .unwrap_or_else(|| {
                                    awal.get_value().get(&uid).map(|(s, _)| s.clone()).unwrap_or_default()
                                })
                        };
                        view! {
                            <div class="py-2.5 flex items-center gap-3 flex-wrap">
                                <span class="w-9 h-9 rounded-full bg-secondary-container text-primary flex items-center justify-center text-body-sm font-bold shrink-0">
                                    {inisial}
                                </span>
                                <div class="min-w-0 flex-1">
                                    <p class="text-body-sm font-semibold text-on-background truncate">{r.name}</p>
                                    <p class="text-[11px] text-on-surface-variant">{nis}</p>
                                    {bukan_anggota
                                        .then(|| {
                                            view! {
                                                <span class="mt-1 inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-surface-container-high text-on-surface-variant text-[10px] font-semibold">
                                                    <span class="material-symbols-outlined text-[13px]">
                                                        "swap_horiz"
                                                    </span>
                                                    "Sudah pindah kelas"
                                                </span>
                                            }
                                        })}
                                </div>
                                {if can_correct && !bukan_anggota {
                                    view! {
                                        <input
                                            type="time"
                                            class="bg-surface-container border-0 rounded-lg px-2 py-1.5 text-[12px] text-on-surface w-[92px]"
                                            prop:value=jam_awal.clone()
                                            on:input=move |ev| set_jam(uid, event_target_value(&ev))
                                            aria-label="Jam masuk"
                                        />
                                        <select
                                            class="bg-surface-container border-0 rounded-lg px-2 py-1.5 text-[12px] text-on-surface"
                                            on:change=move |ev| set_status(uid, event_target_value(&ev))
                                            aria-label="Status kehadiran"
                                        >
                                            {[
                                                ("present", "Hadir"),
                                                ("late", "Terlambat"),
                                                ("permit", "Izin"),
                                                ("sick", "Sakit"),
                                                ("absent", "Alpa"),
                                            ]
                                                .into_iter()
                                                .map(|(v, l)| {
                                                    view! {
                                                        <option value=v selected=move || status_now() == v>
                                                            {l}
                                                        </option>
                                                    }
                                                })
                                                .collect_view()}
                                        </select>
                                    }
                                        .into_any()
                                } else {
                                    view! { <span class=att_badge(&r.status_kind)>{r.status_label}</span> }
                                        .into_any()
                                }}
                            </div>
                        }
                    })
                    .collect_view()}
            </div>

            {(can_correct)
                .then(|| {
                    view! {
                        <button
                            class="w-full py-3 rounded-xl bg-primary text-on-primary font-bold text-body-sm press disabled:opacity-60"
                            prop:disabled=move || busy.get()
                            on:click=simpan
                        >
                            {move || {
                                if busy.get() {
                                    "Menyimpan…".to_string()
                                } else {
                                    format!("Simpan Perubahan ({})", jumlah_ubah())
                                }
                            }}
                        </button>
                        <p class="text-[10px] text-on-surface-variant text-center">
                            "Jam masuk menentukan hadir/terlambat otomatis dari batas jadwal."
                        </p>
                    }
                })}

            // ── Verifikasi (hanya untuk yang berwenang; server yang menentukan) ──
            <Suspense fallback=|| ()>
                {move || {
                    verify
                        .get()
                        .and_then(|r| r.ok())
                        .filter(|v| !v.items.is_empty())
                        .map(|v| {
                            let total = v.items.len();
                            view! {
                                <div class="pt-3 border-t border-outline-variant/50 space-y-2">
                                    <div class="flex items-center justify-between">
                                        <p class="text-body-sm font-bold text-on-background">{v.stage_label}</p>
                                        <span class="text-[11px] text-on-surface-variant">
                                            {format!("{total} menunggu")}
                                        </span>
                                    </div>
                                    <p class="text-[11px] text-on-surface-variant">
                                        "Centang = disetujui. Hilangkan centang untuk MENOLAK."
                                    </p>
                                    {v.items
                                        .into_iter()
                                        .map(|it| {
                                            let aid = it.att_id;
                                            view! {
                                                <label class="flex items-center gap-2 py-1">
                                                    <input
                                                        type="checkbox"
                                                        class="w-4 h-4"
                                                        prop:checked=move || !reject.get().contains(&aid)
                                                        on:change=move |ev| {
                                                            let setuju = event_target_checked(&ev);
                                                            reject
                                                                .update(|vv| {
                                                                    if setuju {
                                                                        vv.retain(|&x| x != aid);
                                                                    } else if !vv.contains(&aid) {
                                                                        vv.push(aid);
                                                                    }
                                                                });
                                                        }
                                                    />
                                                    <span class="text-body-sm text-on-background flex-1 truncate">
                                                        {it.name}
                                                    </span>
                                                    <span class="text-[11px] text-on-surface-variant">{it.status}</span>
                                                </label>
                                            }
                                        })
                                        .collect_view()}

                                    {move || {
                                        if konfirmasi.get() {
                                            let n_tolak = reject.get().len();
                                            view! {
                                                <div class="bg-warning/10 border border-warning/40 rounded-xl p-3 space-y-2">
                                                    <p class="text-body-sm text-on-background">
                                                        <b>"Verifikasi tidak bisa dibatalkan."</b>
                                                        " Poin akan diberikan dan baris yang sudah final tak bisa diubah lagi."
                                                    </p>
                                                    <p class="text-[11px] text-on-surface-variant">
                                                        {format!("{} disetujui · {n_tolak} ditolak", total - n_tolak)}
                                                    </p>
                                                    <div class="grid grid-cols-2 gap-2">
                                                        <button
                                                            class="py-2.5 rounded-lg border border-outline-variant text-on-surface font-semibold text-body-sm"
                                                            on:click=move |_| konfirmasi.set(false)
                                                        >
                                                            "Batal"
                                                        </button>
                                                        <button
                                                            class="py-2.5 rounded-lg bg-primary text-on-primary font-bold text-body-sm press disabled:opacity-60"
                                                            prop:disabled=move || busy.get()
                                                            on:click=verifikasi
                                                        >
                                                            "Ya, Verifikasi"
                                                        </button>
                                                    </div>
                                                </div>
                                            }
                                                .into_any()
                                        } else {
                                            view! {
                                                <button
                                                    class="w-full py-3 rounded-xl bg-primary text-on-primary font-bold text-body-sm press disabled:opacity-60"
                                                    prop:disabled=move || busy.get()
                                                    on:click=move |_| konfirmasi.set(true)
                                                >
                                                    {format!("Verifikasi Seluruh Sesi ({total})")}
                                                </button>
                                            }
                                                .into_any()
                                        }
                                    }}
                                </div>
                            }
                        })
                }}
            </Suspense>
        </div>
    }
}
