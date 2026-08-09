//! web/pages/materi.rs — "Materials Library" (migrasi 17): widget dashboard
//! (staf/dewan guru) + halaman lengkap /materi (tanpa item navbar — hanya
//! dijangkau lewat link "Lihat Semua" pada widget).

use leptos::prelude::*;
use leptos_meta::Title;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

use crate::models::MaterialItem;
use crate::web::api::{add_material_link_action, delete_material_action, materials_list};
use crate::web::components::{DeviceFrame, EmptyState, FetchError, MobileHeader};

fn kind_icon(kind: &str) -> &'static str {
    match kind {
        "audio" => "audio_file",
        "document" => "menu_book",
        "video" => "video_library",
        _ => "open_in_new",
    }
}

/// Kartu "Materials Library" dipasang di dashboard staf/dewan guru.
/// `manage`=true (admin/dewan_guru) → tampil form unggah + tombol hapus.
#[component]
pub fn MaterialsWidget(manage: bool) -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { materials_list(5).await });
    let show_form = RwSignal::new(false);

    view! {
        <div class="ppm-card p-4 space-y-3">
            <div class="flex items-center justify-between">
                <h3 class="text-body-md font-bold text-on-background flex items-center gap-2">
                    <span class="material-symbols-outlined text-primary">"library_books"</span>
                    "Materials Library"
                </h3>
                <a href="/materi" class="text-primary font-semibold text-body-sm">"Lihat Semua"</a>
            </div>

            <Suspense fallback=|| {
                view! { <div class="h-16 bg-surface-container rounded-xl animate-pulse"></div> }
            }>
                {move || {
                    data.get()
                        .map(|res| match res {
                            Ok(items) => {
                                if items.is_empty() {
                                    view! {
                                        <p class="text-body-sm text-on-surface-variant">
                                            "Belum ada materi dibagikan."
                                        </p>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <div class="space-y-1">
                                            {items
                                                .into_iter()
                                                .map(|m| view! { <MaterialRow m=m /> })
                                                .collect_view()}
                                        </div>
                                    }
                                        .into_any()
                                }
                            }
                            Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                        })
                }}
            </Suspense>

            {manage
                .then(|| {
                    view! {
                        <div class="pt-2 border-t border-outline-variant/40">
                            <button
                                class="text-primary font-semibold text-body-sm flex items-center gap-1"
                                on:click=move |_| show_form.update(|s| *s = !*s)
                            >
                                <span class="material-symbols-outlined text-[18px]">"add_link"</span>
                                "Tambah Tautan"
                            </button>
                            {move || {
                                show_form
                                    .get()
                                    .then(|| view! { <AddLinkForm refetch=move || data.refetch() /> })
                            }}
                            <p class="text-[11px] text-on-surface-variant mt-2">
                                "Untuk unggah file (MP3/PDF/MP4), buka halaman "
                                <a href="/materi" class="text-primary font-semibold">"Materials Library"</a>
                                "."
                            </p>
                        </div>
                    }
                })}
        </div>
    }
}

#[component]
fn AddLinkForm(refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let title = RwSignal::new(String::new());
    let url = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let (t, u) = (title.get_untracked(), url.get_untracked());
        leptos::task::spawn_local(async move {
            match add_material_link_action(t, u).await {
                Ok(_) => {
                    title.set(String::new());
                    url.set(String::new());
                    refetch();
                }
                Err(e) => {
                    msg.set(Some((false, crate::web::components::pesan_galat(e))));
                }
            }
            busy.set(false);
        });
    };

    let field =
        "w-full bg-surface-container border-0 rounded-lg px-3 py-2 text-body-sm text-on-surface";
    view! {
        <form class="mt-2 space-y-2 anim-in" method="post" on:submit=submit>
            {move || {
                msg.get()
                    .map(|(_, t)| {
                        view! {
                            <div class="p-2 bg-error-container text-on-error-container rounded-lg text-body-sm">
                                {t}
                            </div>
                        }
                    })
            }}
            <input
                type="text"
                class=field
                placeholder="Judul (mis. Sejarah Islam: Khulafaur Rasyidin)"
                prop:value=move || title.get()
                on:input=move |ev| title.set(event_target_value(&ev))
            />
            <input
                type="url"
                class=field
                placeholder="https://youtube.com/…"
                prop:value=move || url.get()
                on:input=move |ev| url.set(event_target_value(&ev))
            />
            <button
                type="submit"
                class="w-full py-2 rounded-lg bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                disabled=move || busy.get()
            >
                {move || if busy.get() { "Menyimpan…" } else { "Simpan Tautan" }}
            </button>
        </form>
    }
}

#[component]
fn MaterialRow(m: MaterialItem) -> impl IntoView {
    let icon = kind_icon(&m.kind);
    view! {
        <a
            href=m.file_url.clone()
            target="_blank"
            rel="noopener noreferrer"
            class="flex items-center gap-3 p-2 rounded-lg hover:bg-surface-container transition-colors"
        >
            <div class="w-10 h-10 rounded-lg bg-secondary-container flex items-center justify-center text-primary shrink-0">
                <span class="material-symbols-outlined text-[20px]">{icon}</span>
            </div>
            <div class="flex-1 min-w-0">
                <p class="text-body-sm font-semibold text-on-background truncate">{m.title}</p>
                <p class="text-[11px] text-on-surface-variant">{m.meta_label}</p>
            </div>
            <span class="material-symbols-outlined text-on-surface-variant text-[18px]">
                {if m.kind == "link" { "open_in_new" } else { "download" }}
            </span>
        </a>
    }
}

/// Halaman penuh /materi (admin/dewan_guru) — daftar lengkap + unggah file.
#[component]
pub fn MateriPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { materials_list(200).await });

    crate::web::components::guard_sesi(data);

    view! {
        <Title text="Materials Library — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader title="Materials Library" subtitle="File bersama untuk santri & pengajar" back_href="/staf" />

                <div class="px-5 pt-5 space-y-4 stagger">
                    <div class="md:max-w-md">
                        <UploadForm refetch=move || data.refetch() />
                    </div>

                    <Suspense fallback=|| {
                        view! {
                            <div class="animate-pulse space-y-2">
                                <div class="h-16 bg-surface-container rounded-2xl"></div>
                                <div class="h-16 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Ok(items) => {
                                        if items.is_empty() {
                                            view! {
                                                <EmptyState
                                                    icon="library_books"
                                                    title="Belum ada materi"
                                                    subtitle="Unggah file atau tambahkan tautan lewat form di atas."
                                                />
                                            }
                                                .into_any()
                                        } else {
                                            // Desktop: baris materi (kompak) 2 kolom agar tak
                                            // melar penuh kanvas; mobile tetap bertumpuk.
                                            view! {
                                                <div class="ppm-card-grid">
                                                    {items
                                                        .into_iter()
                                                        .map(|m| {
                                                            view! {
                                                                <MaterialRowManage m=m refetch=move || data.refetch() />
                                                            }
                                                        })
                                                        .collect_view()}
                                                </div>
                                            }
                                                .into_any()
                                        }
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
fn MaterialRowManage(m: MaterialItem, refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let id = m.id;
    let icon = kind_icon(&m.kind);
    let busy = RwSignal::new(false);
    let del = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            let _ = delete_material_action(id).await;
            busy.set(false);
            refetch();
        });
    };
    view! {
        <div class="ppm-card p-3 flex items-center gap-3">
            <div class="w-10 h-10 rounded-lg bg-secondary-container flex items-center justify-center text-primary shrink-0">
                <span class="material-symbols-outlined text-[20px]">{icon}</span>
            </div>
            <div class="flex-1 min-w-0">
                <a
                    href=m.file_url.clone()
                    target="_blank"
                    rel="noopener noreferrer"
                    class="text-body-sm font-semibold text-on-background truncate hover:underline block"
                >
                    {m.title}
                </a>
                <p class="text-[11px] text-on-surface-variant">{m.meta_label}</p>
            </div>
            <button
                class="w-8 h-8 rounded-lg bg-error-container/60 text-error flex items-center justify-center shrink-0 disabled:opacity-50"
                disabled=move || busy.get()
                on:click=del
                aria-label="Hapus materi"
            >
                <span class="material-symbols-outlined text-[18px]">"delete"</span>
            </button>
        </div>
    }
}

/// Form unggah file (mp3/wav/ogg, pdf, mp4/webm) via `POST /api/materials/upload`
/// (multipart, di luar server-fn) + tautan lewat server fn biasa.
#[component]
fn UploadForm(refetch: impl Fn() + Copy + Send + 'static) -> impl IntoView {
    let mode = RwSignal::new("file".to_string());
    let title = RwSignal::new(String::new());
    let url = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);
    let file_input: NodeRef<leptos::html::Input> = NodeRef::new();

    let submit_link = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let (t, u) = (title.get_untracked(), url.get_untracked());
        leptos::task::spawn_local(async move {
            match add_material_link_action(t, u).await {
                Ok(_) => {
                    title.set(String::new());
                    url.set(String::new());
                    refetch();
                }
                Err(e) => {
                    msg.set(Some((false, crate::web::components::pesan_galat(e))));
                }
            }
            busy.set(false);
        });
    };

    let submit_file = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        #[cfg(target_arch = "wasm32")]
        {
            if busy.get_untracked() {
                return;
            }
            let Some(input) = file_input.get() else { return };
            let Some(files) = input.files() else { return };
            let Some(file) = files.get(0) else {
                msg.set(Some((false, "Pilih file terlebih dahulu.".into())));
                return;
            };
            busy.set(true);
            msg.set(None);
            let t = title.get_untracked();
            leptos::task::spawn_local(async move {
                let form = web_sys::FormData::new().unwrap();
                let _ = form.append_with_str("title", &t);
                let _ = form.append_with_blob("file", &file);

                let window = web_sys::window().unwrap();
                let opts = web_sys::RequestInit::new();
                opts.set_method("POST");
                opts.set_body(form.as_ref());
                let req = web_sys::Request::new_with_str_and_init("/api/materials/upload", &opts)
                    .unwrap();
                match wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&req)).await {
                    Ok(resp) => {
                        let resp: web_sys::Response = resp.dyn_into().unwrap();
                        if resp.ok() {
                            title.set(String::new());
                            refetch();
                        } else {
                            msg.set(Some((
                                false,
                                format!("Upload gagal (HTTP {}).", resp.status()),
                            )));
                        }
                    }
                    Err(_) => {
                        msg.set(Some((false, "Upload gagal — periksa koneksi.".into())));
                    }
                }
                busy.set(false);
            });
        }
    };

    let field =
        "w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface";
    view! {
        <div class="ppm-card p-4 space-y-3">
            <h3 class="text-body-md font-bold text-on-background">"Tambah Materi"</h3>
            <div class="flex gap-1 bg-surface-container rounded-xl p-1">
                <button
                    type="button"
                    class=move || {
                        if mode.get() == "file" {
                            "flex-1 py-2 rounded-lg bg-surface-container-lowest text-primary font-semibold text-body-sm shadow-sm"
                        } else {
                            "flex-1 py-2 rounded-lg text-on-surface-variant font-medium text-body-sm"
                        }
                    }
                    on:click=move |_| mode.set("file".into())
                >
                    "Unggah File"
                </button>
                <button
                    type="button"
                    class=move || {
                        if mode.get() == "link" {
                            "flex-1 py-2 rounded-lg bg-surface-container-lowest text-primary font-semibold text-body-sm shadow-sm"
                        } else {
                            "flex-1 py-2 rounded-lg text-on-surface-variant font-medium text-body-sm"
                        }
                    }
                    on:click=move |_| mode.set("link".into())
                >
                    "Tautan"
                </button>
            </div>

            {move || {
                msg.get()
                    .map(|(_, t)| {
                        view! {
                            <div class="p-2.5 bg-error-container text-on-error-container rounded-lg text-body-sm">
                                {t}
                            </div>
                        }
                    })
            }}

            <input
                type="text"
                class=field
                placeholder="Judul materi"
                prop:value=move || title.get()
                on:input=move |ev| title.set(event_target_value(&ev))
            />

            {move || {
                if mode.get() == "file" {
                    view! {
                        <form class="space-y-2" method="post" on:submit=submit_file>
                            <input
                                type="file"
                                node_ref=file_input
                                accept=".mp3,.wav,.ogg,.pdf,.mp4,.webm"
                                class=field
                            />
                            <button
                                type="submit"
                                class="w-full py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                                disabled=move || busy.get()
                            >
                                {move || if busy.get() { "Mengunggah…" } else { "Unggah" }}
                            </button>
                        </form>
                    }
                        .into_any()
                } else {
                    view! {
                        <form class="space-y-2" method="post" on:submit=submit_link>
                            <input
                                type="url"
                                class=field
                                placeholder="https://youtube.com/…"
                                prop:value=move || url.get()
                                on:input=move |ev| url.set(event_target_value(&ev))
                            />
                            <button
                                type="submit"
                                class="w-full py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                                disabled=move || busy.get()
                            >
                                {move || if busy.get() { "Menyimpan…" } else { "Simpan Tautan" }}
                            </button>
                        </form>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}
