//! Tab KURIKULUM (migrasi 17) — materi yang direncanakan untuk kelas ini.

use leptos::prelude::*;

use crate::models::{
    CurriculumItem, KelasDetail,
};
use crate::web::api::{
    create_curriculum_action,
    delete_curriculum_action, update_curriculum_action,
};
use crate::web::components::EmptyState;

// ── Tab KURIKULUM (migrasi 17) ───────────────────────────────────────────────

#[component]
pub(super) fn KurikulumTab(
    class_id: i64,
    d: KelasDetail,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let items = d.curriculum.clone();
    let book_opts = StoredValue::new(d.book_options.clone());

    view! {
        <div class="space-y-3 stagger">
            <div class="md:max-w-md">
                <BuatKurikulumForm class_id=class_id book_options=book_opts refetch=refetch />
            </div>

            {if items.is_empty() {
                view! {
                    <EmptyState
                        icon="menu_book"
                        title="Belum ada materi/kitab"
                        subtitle="Tambahkan cakupan materi kelas ini lewat form di atas."
                    />
                }
                    .into_any()
            } else {
                view! {
                    <div class="space-y-3 md:space-y-0 md:grid md:grid-cols-2 md:gap-3">
                        {items
                            .into_iter()
                            .map(|c| view! { <KurikulumCard c=c book_options=book_opts refetch=refetch /> })
                            .collect_view()}
                    </div>
                }
                    .into_any()
            }}
        </div>
    }
}

#[component]
fn KurikulumCard(
    c: CurriculumItem,
    book_options: StoredValue<Vec<crate::models::BookItem>>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let cid = c.id;
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let editing = RwSignal::new(false);
    let e_title = RwSignal::new(c.title.clone());
    let e_desc = RwSignal::new(c.description.clone());
    let e_start = RwSignal::new(c.scope_start.clone());
    let e_end = RwSignal::new(c.scope_end.clone());
    let e_pct = RwSignal::new(c.progress_pct.to_string());
    let e_status = RwSignal::new(c.status.clone());
    let e_book = RwSignal::new(c.book_id);

    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let a = (
            e_title.get_untracked(),
            e_desc.get_untracked(),
            e_start.get_untracked(),
            e_end.get_untracked(),
            e_pct.get_untracked(),
            e_status.get_untracked(),
            e_book.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            match update_curriculum_action(cid, a.0, a.1, a.2, a.3, a.4, a.5, a.6).await {
                Ok(_) => {
                    editing.set(false);
                    refetch();
                }
                Err(e) => {
                    let s = e.to_string();
                    msg.set(Some((false, s.rsplit(": ").next().unwrap_or(&s).to_string())));
                }
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
            let _ = delete_curriculum_action(cid).await;
            busy.set(false);
            refetch();
        });
    };

    let title_ro = c.title.clone();
    let status_label = c.status_label.clone();
    let pct = c.progress_pct;
    let book_ro = c.book_title.clone();
    let scope_label = if c.scope_start.is_empty() && c.scope_end.is_empty() {
        String::new()
    } else {
        format!("{} → {}", c.scope_start, c.scope_end)
    };
    let badge = match c.status.as_str() {
        "completed" => "ppm-chip bg-success/10 text-success",
        "upcoming" => "ppm-chip bg-surface-container-highest text-on-surface-variant",
        _ => "ppm-chip bg-primary/10 text-primary",
    };
    let field =
        "w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface";

    view! {
        <div class="ppm-card p-4 card-hover anim-in" style="border-left:4px solid #064e3b">
            <div class="flex items-center justify-between gap-2">
                <p class="text-body-md font-bold text-on-background truncate">{title_ro}</p>
                <div class="flex items-center gap-2 shrink-0">
                    <span class=badge>{status_label}</span>
                    <button
                        class="w-8 h-8 rounded-lg bg-surface-container text-primary flex items-center justify-center press"
                        on:click=move |_| editing.update(|e| *e = !*e)
                        aria-label="Edit materi"
                    >
                        <span class="material-symbols-outlined text-[18px]">"edit"</span>
                    </button>
                </div>
            </div>

            {move || {
                msg.get()
                    .map(|(_, t)| {
                        view! {
                            <div class="mt-2 p-2 bg-error-container text-on-error-container rounded-lg text-body-sm anim-in">
                                {t}
                            </div>
                        }
                    })
            }}

            {move || {
                if editing.get() {
                    view! {
                        <form class="mt-3 space-y-2 anim-in" method="post" on:submit=save>
                            <input
                                type="text"
                                class=field
                                placeholder="Judul materi/kitab"
                                prop:value=move || e_title.get()
                                on:input=move |ev| e_title.set(event_target_value(&ev))
                            />
                            <input
                                type="text"
                                class=field
                                placeholder="Sub-judul/topik (opsional)"
                                prop:value=move || e_desc.get()
                                on:input=move |ev| e_desc.set(event_target_value(&ev))
                            />
                            <div class="grid grid-cols-2 gap-2">
                                <input
                                    type="text"
                                    class=field
                                    placeholder="Dari (mis. Al-Fatihah)"
                                    prop:value=move || e_start.get()
                                    on:input=move |ev| e_start.set(event_target_value(&ev))
                                />
                                <input
                                    type="text"
                                    class=field
                                    placeholder="Sampai (mis. Juz 15)"
                                    prop:value=move || e_end.get()
                                    on:input=move |ev| e_end.set(event_target_value(&ev))
                                />
                            </div>
                            <div class="grid grid-cols-2 gap-2">
                                <label class="space-y-1">
                                    <span class="text-[11px] text-on-surface-variant">"Progres (%)"</span>
                                    <input
                                        type="number"
                                        min="0"
                                        max="100"
                                        class=field
                                        prop:value=move || e_pct.get()
                                        on:input=move |ev| e_pct.set(event_target_value(&ev))
                                    />
                                </label>
                                <label class="space-y-1">
                                    <span class="text-[11px] text-on-surface-variant">"Status"</span>
                                    <select class=field on:change=move |ev| e_status.set(event_target_value(&ev))>
                                        <option value="active" selected=move || e_status.get() == "active">"Berjalan"</option>
                                        <option value="completed" selected=move || e_status.get() == "completed">"Selesai"</option>
                                        <option value="upcoming" selected=move || e_status.get() == "upcoming">"Akan Datang"</option>
                                    </select>
                                </label>
                            </div>
                            <label class="space-y-1 block">
                                <span class="text-[11px] text-on-surface-variant">
                                    "Tautkan ke materi terdaftar (opsional)"
                                </span>
                                <select
                                    class=field
                                    on:change=move |ev| e_book.set(event_target_value(&ev).parse().unwrap_or(0))
                                >
                                    <option value="0" selected=move || e_book.get() == 0>
                                        "— Tanpa tautan —"
                                    </option>
                                    {book_options
                                        .get_value()
                                        .into_iter()
                                        .map(|b| {
                                            let val = b.id.to_string();
                                            let sel = move || e_book.get() == b.id;
                                            view! {
                                                <option value=val selected=sel>
                                                    {b.title}
                                                </option>
                                            }
                                        })
                                        .collect_view()}
                                </select>
                            </label>
                            <div class="grid grid-cols-2 gap-2 pt-1">
                                <button
                                    type="button"
                                    class="py-2.5 rounded-lg border border-outline-variant text-on-surface font-semibold text-body-sm"
                                    on:click=move |_| editing.set(false)
                                >
                                    "Batal"
                                </button>
                                <button
                                    type="submit"
                                    class="py-2.5 rounded-lg bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                                    disabled=move || busy.get()
                                >
                                    {move || if busy.get() { "Menyimpan…" } else { "Simpan" }}
                                </button>
                            </div>
                            <button
                                type="button"
                                class="w-full flex items-center justify-center gap-1.5 py-2 rounded-lg bg-error-container/60 text-error text-body-sm font-semibold press disabled:opacity-60"
                                disabled=move || busy.get()
                                on:click=del
                            >
                                <span class="material-symbols-outlined text-[18px]">"delete"</span>
                                "Hapus Materi"
                            </button>
                        </form>
                    }
                        .into_any()
                } else {
                    view! {
                        <div class="mt-2">
                            {(!book_ro.is_empty())
                                .then(|| {
                                    view! {
                                        <p class="text-body-sm text-primary flex items-center gap-1">
                                            <span class="material-symbols-outlined text-[15px]">"menu_book"</span>
                                            "Materi: " {book_ro.clone()}
                                        </p>
                                    }
                                })}
                            {(!scope_label.is_empty())
                                .then(|| {
                                    view! {
                                        <p class="text-body-sm text-on-surface-variant">{scope_label.clone()}</p>
                                    }
                                })}
                            <div class="flex items-center justify-between text-xs font-semibold mt-2">
                                <span class="text-on-surface-variant">"Progres"</span>
                                <span class="text-on-background">{format!("{pct}%")}</span>
                            </div>
                            <div class="h-2 bg-surface-container rounded-full overflow-hidden mt-1">
                                <div class="h-full bg-primary bar-grow" style=format!("width: {pct}%")></div>
                            </div>
                        </div>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}

#[component]
fn BuatKurikulumForm(
    class_id: i64,
    book_options: StoredValue<Vec<crate::models::BookItem>>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let title = RwSignal::new(String::new());
    let desc = RwSignal::new(String::new());
    let scope_start = RwSignal::new(String::new());
    let scope_end = RwSignal::new(String::new());
    let pct = RwSignal::new(String::new());
    let status = RwSignal::new("active".to_string());
    let book = RwSignal::new(0i64);
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let a = (
            title.get_untracked(),
            desc.get_untracked(),
            scope_start.get_untracked(),
            scope_end.get_untracked(),
            pct.get_untracked(),
            status.get_untracked(),
            book.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            match create_curriculum_action(class_id, a.0, a.1, a.2, a.3, a.4, a.5, a.6).await {
                Ok(_) => {
                    msg.set(Some((true, "Materi ditambahkan.".into())));
                    title.set(String::new());
                    desc.set(String::new());
                    scope_start.set(String::new());
                    scope_end.set(String::new());
                    pct.set(String::new());
                    book.set(0);
                    refetch();
                }
                Err(e) => {
                    let m = e.to_string();
                    msg.set(Some((false, m.rsplit(": ").next().unwrap_or(&m).to_string())));
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
                        "Tambah Materi/Kitab"
                    </button>
                }
                    .into_any();
            }
            let field = field;
            view! {
                <form class="ppm-card p-4 space-y-3 anim-in" method="post" on:submit=submit>
                    <h3 class="text-body-md font-bold text-on-background">"Materi/Kitab Baru"</h3>
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
                    <input
                        type="text"
                        class=field
                        placeholder="Judul (mis. Al-Qur'an, Sahih Bukhari)"
                        prop:value=move || title.get()
                        on:input=move |ev| title.set(event_target_value(&ev))
                    />
                    <input
                        type="text"
                        class=field
                        placeholder="Sub-judul/topik (opsional)"
                        prop:value=move || desc.get()
                        on:input=move |ev| desc.set(event_target_value(&ev))
                    />
                    // Tautkan ke materi terdaftar (opsional) — konsisten dgn daftar materi.
                    <label class="space-y-1 block">
                        <span class="text-label-md text-on-surface-variant">
                            "Tautkan ke materi terdaftar (opsional)"
                        </span>
                        <select
                            class=field
                            on:change=move |ev| book.set(event_target_value(&ev).parse().unwrap_or(0))
                        >
                            <option value="0">"— Tanpa tautan (judul manual) —"</option>
                            {book_options
                                .get_value()
                                .into_iter()
                                .map(|b| {
                                    let val = b.id.to_string();
                                    view! { <option value=val>{b.title}</option> }
                                })
                                .collect_view()}
                        </select>
                    </label>
                    <div class="grid grid-cols-2 gap-2">
                        <input
                            type="text"
                            class=field
                            placeholder="Dari (mis. Al-Fatihah)"
                            prop:value=move || scope_start.get()
                            on:input=move |ev| scope_start.set(event_target_value(&ev))
                        />
                        <input
                            type="text"
                            class=field
                            placeholder="Sampai (mis. Juz 15)"
                            prop:value=move || scope_end.get()
                            on:input=move |ev| scope_end.set(event_target_value(&ev))
                        />
                    </div>
                    <label class="space-y-1 block">
                        <span class="text-label-md text-on-surface-variant">"Progres awal (%)"</span>
                        <input
                            type="number"
                            min="0"
                            max="100"
                            class=field
                            placeholder="0"
                            prop:value=move || pct.get()
                            on:input=move |ev| pct.set(event_target_value(&ev))
                        />
                    </label>
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
                            {move || if busy.get() { "Menyimpan…" } else { "Simpan" }}
                        </button>
                    </div>
                </form>
            }
                .into_any()
        }}
    }
}
