//! Tab SESI — daftar sesi kelas + buat sesi ad-hoc (di luar jadwal).

use leptos::prelude::*;

use crate::models::{
    KelasDetail, ScheduleOption,
    TeacherOption,
};
use crate::web::api::{
    create_session_action, set_session_libur_action, set_session_teacher_action,
};
use crate::web::components::AdminOnly;
use crate::web::components::{
    EmptyState, FlashMsg, kartu_grid,
};

// ── Tab SESI ──────────────────────────────────────────────────────────────────

#[component]
pub(super) fn SesiTab(
    class_id: i64,
    d: KelasDetail,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let can_manage = d.can_manage;
    let sessions = d.sessions.clone();
    let sched_opts = StoredValue::new(d.schedule_options.clone());
    let teacher_opts = StoredValue::new(d.teacher_options.clone());
    let book_opts = StoredValue::new(d.book_options.clone());

    view! {
        <div class="space-y-3 stagger">
            <div class="md:max-w-md">
                // Membuat sesi ad-hoc = wewenang admin. Sesi rutin lahir
                // otomatis dari jadwal, jadi peran lain tak kehilangan apa pun.
                <AdminOnly can_manage=can_manage apa="membuat sesi baru">
                    <BuatSesiForm
                        class_id=class_id
                        schedule_options=sched_opts
                        teacher_options=teacher_opts
                        book_options=book_opts
                        refetch=refetch
                    />
                </AdminOnly>
            </div>

            <p class="text-body-sm text-on-surface-variant">
                "Sesi mendatang dibuat otomatis dari jadwal. Isi pengajar atau tandai libur di bawah."
            </p>

            {if sessions.is_empty() {
                view! {
                    <EmptyState
                        icon="event"
                        title="Belum ada sesi"
                        subtitle="Buat jadwal dulu agar sesi ter-generate otomatis."
                    />
                }
                    .into_any()
            } else {
                kartu_grid(
                        sessions
                            .into_iter()
                            .map(|s| {
                                view! { <SesiCard s=s teacher_options=teacher_opts refetch=refetch /> }
                                    .into_any()
                            })
                            .collect(),
                    )
                    .into_any()
            }}
        </div>
    }
}

#[component]
fn SesiCard(
    s: crate::models::SessionItem,
    teacher_options: StoredValue<Vec<TeacherOption>>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let sid = s.id;
    let is_libur = s.status_kind == "cancelled";
    let cur_teacher = s.teacher_id.unwrap_or(0);
    let busy = RwSignal::new(false);

    let badge = if is_libur {
        "ppm-chip bg-error-container text-error"
    } else {
        match s.status_kind.as_str() {
            "ongoing" => "ppm-chip bg-success/10 text-success",
            "finished" => "ppm-chip bg-surface-container-highest text-on-surface-variant",
            _ => "ppm-chip bg-info/10 text-info",
        }
    };
    let status_text = if is_libur {
        "Libur".to_string()
    } else {
        s.status_label.clone()
    };
    let when = format!("{} • {}", s.date_label, s.time_label);

    let set_teacher = move |tid: i64| {
        busy.set(true);
        leptos::task::spawn_local(async move {
            let _ = set_session_teacher_action(sid, tid).await;
            busy.set(false);
            refetch();
        });
    };
    let toggle_libur = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            let _ = set_session_libur_action(sid, !is_libur).await;
            busy.set(false);
            refetch();
        });
    };

    let border = if is_libur { "ppm-accent-error" } else { "ppm-accent" };
    let btn_cls = if is_libur {
        "mt-3 w-full py-2 rounded-lg text-body-sm font-semibold press disabled:opacity-60 flex items-center justify-center gap-1.5 bg-secondary-container text-primary"
    } else {
        "mt-3 w-full py-2 rounded-lg text-body-sm font-semibold press disabled:opacity-60 flex items-center justify-center gap-1.5 bg-error-container/60 text-error"
    };

    view! {
        <div class=format!("ppm-card p-4 card-hover anim-in {border}")>
            <div class="flex items-center justify-between gap-2">
                <p class="text-body-md font-bold text-on-background truncate flex-1">{s.title}</p>
                <span class=badge>{status_text}</span>
            </div>
            <p class="text-body-sm text-on-surface-variant flex items-center gap-1 mt-1">
                <span class="material-symbols-outlined text-[15px]">"schedule"</span>
                {when}
            </p>

            {(!is_libur)
                .then(|| {
                    view! {
                        <div class="mt-3 space-y-1">
                            <label class="text-label-md text-on-surface-variant flex items-center gap-1">
                                <span class="material-symbols-outlined text-[15px]">"person"</span>
                                "Pengajar"
                            </label>
                            <select
                                class="w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface disabled:opacity-60"
                                disabled=move || busy.get()
                                on:change=move |ev| set_teacher(
                                    event_target_value(&ev).parse().unwrap_or(0),
                                )
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
                            // Guru pengisi sesi inilah yang mengesahkan
                            // absensinya. Bila dibiarkan kosong, yang menutup
                            // adalah WALI KELAS — jadi tak pernah ada sesi tanpa
                            // penanggung jawab absensi.
                            <p class="text-[11px] text-on-surface-variant">
                                "Kosongkan bila diisi wali kelas sendiri."
                            </p>
                        </div>
                    }
                })}

            <button
                class=btn_cls
                disabled=move || busy.get()
                on:click=toggle_libur
            >
                <span class="material-symbols-outlined text-[18px]">
                    {if is_libur { "event_available" } else { "event_busy" }}
                </span>
                {if is_libur { "Batalkan Libur (aktifkan)" } else { "Tandai Libur" }}
            </button>
        </div>
    }
}

#[component]
fn BuatSesiForm(
    class_id: i64,
    schedule_options: StoredValue<Vec<ScheduleOption>>,
    teacher_options: StoredValue<Vec<TeacherOption>>,
    book_options: StoredValue<Vec<crate::models::BookItem>>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let title = RwSignal::new(String::new());
    let date = RwSignal::new(String::new());
    let sched = RwSignal::new(0i64);
    let teacher = RwSignal::new(0i64);
    let book = RwSignal::new(0i64);
    let book_pages = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let (t, dt, sc, tc, bk, bp) = (
            title.get_untracked(),
            date.get_untracked(),
            sched.get_untracked(),
            teacher.get_untracked(),
            book.get_untracked(),
            book_pages.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            match create_session_action(class_id, sc, tc, t, dt, bk, bp).await {
                Ok(_) => {
                    msg.set(Some((true, "Sesi dibuat.".into())));
                    title.set(String::new());
                    date.set(String::new());
                    book.set(0);
                    book_pages.set(String::new());
                    refetch();
                }
                Err(e) => {
                    let m = e.to_string();
                    msg.set(Some((
                        false,
                        crate::web::components::pesan_galat(&m),
                    )));
                }
            }
            busy.set(false);
        });
    };

    let field =
        "w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface";
    view! {
        {move || {
            if !open.get() {
                return view! {
                    <button
                        class="w-full py-3.5 rounded-2xl bg-primary text-on-primary font-bold flex items-center justify-center gap-2 press"
                        on:click=move |_| open.set(true)
                    >
                        <span class="material-symbols-outlined">"add"</span>
                        "Buat Sesi Baru"
                    </button>
                }
                    .into_any();
            }
            let field = field;
            view! {
                <form
                    class="ppm-card p-4 space-y-3 anim-in"
                    method="post"
                    on:submit=submit
                >
                    <h3 class="text-body-md font-bold text-on-background">"Sesi Baru"</h3>
                    <FlashMsg pesan=msg />
                    <input
                        type="text"
                        class=field
                        placeholder="Judul sesi (mis. Tadarus Juz 5)"
                        prop:value=move || title.get()
                        on:input=move |ev| title.set(event_target_value(&ev))
                    />
                    <label class="space-y-1 block">
                        <span class="text-label-md text-on-surface-variant">"Tanggal sesi"</span>
                        <input
                            type="date"
                            class=field
                            prop:value=move || date.get()
                            on:input=move |ev| date.set(event_target_value(&ev))
                            required=true
                        />
                    </label>
                    <label class="space-y-1 block">
                        <span class="text-label-md text-on-surface-variant">"Jadwal (opsional)"</span>
                        <select class=field on:change=move |ev| sched.set(event_target_value(&ev).parse().unwrap_or(0))>
                            <option value="0">"— Tanpa jadwal —"</option>
                            {schedule_options
                                .get_value()
                                .into_iter()
                                .map(|o| {
                                    let val = o.id.to_string();
                                    view! { <option value=val>{o.label}</option> }
                                })
                                .collect_view()}
                        </select>
                    </label>
                    <label class="space-y-1 block">
                        <span class="text-label-md text-on-surface-variant">"Pengajar (opsional)"</span>
                        <select class=field on:change=move |ev| teacher.set(event_target_value(&ev).parse().unwrap_or(0))>
                            <option value="0">"— Belum ditentukan —"</option>
                            {teacher_options
                                .get_value()
                                .into_iter()
                                .map(|o| {
                                    let val = o.id.to_string();
                                    view! { <option value=val>{o.name}</option> }
                                })
                                .collect_view()}
                        </select>
                    </label>
                    <label class="space-y-1 block">
                        <span class="text-label-md text-on-surface-variant">"Materi (opsional)"</span>
                        <select
                            class=field
                            on:change=move |ev| book.set(event_target_value(&ev).parse().unwrap_or(0))
                        >
                            <option value="0">"— Tanpa materi —"</option>
                            {book_options
                                .get_value()
                                .into_iter()
                                .map(|o| {
                                    let val = o.id.to_string();
                                    view! { <option value=val>{o.title}</option> }
                                })
                                .collect_view()}
                        </select>
                    </label>
                    {move || {
                        (book.get() > 0)
                            .then(|| {
                                view! {
                                    <label class="space-y-1 block anim-in">
                                        <span class="text-label-md text-on-surface-variant">
                                            "Halaman yang dibahas (mis. 11-20, 45-50)"
                                        </span>
                                        <input
                                            type="text"
                                            class=field
                                            placeholder="11-20, 45-50"
                                            prop:value=move || book_pages.get()
                                            on:input=move |ev| book_pages.set(event_target_value(&ev))
                                        />
                                    </label>
                                }
                            })
                    }}
                    <div class="grid grid-cols-2 gap-3">
                        <button
                            type="button"
                            class="py-3 rounded-xl border border-outline-variant text-on-surface font-semibold text-body-sm"
                            on:click=move |_| open.set(false)
                        >
                            "Batal"
                        </button>
                        <button
                            type="submit"
                            class="py-3 rounded-xl bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                            disabled=move || busy.get()
                        >
                            {move || if busy.get() { "Menyimpan…" } else { "Simpan Sesi" }}
                        </button>
                    </div>
                </form>
            }
                .into_any()
        }}
    }
}
