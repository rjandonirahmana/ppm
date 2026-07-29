//! web/pages/galeri.rs — Galeri "Foto Kegiatan" (migrasi 34). Kelola foto yang
//! tampil di beranda publik: unggah banyak sekaligus, geser (drag-and-drop asli)
//! untuk mengubah urutan, dan hapus. Hanya admin/dewan_guru yang bisa mengelola.
//!
//! Upload lewat POST /api/activity-photos/upload (multipart, di luar server-fn,
//! sama pola materi.rs). Urutan disimpan via `reorder_activity_photos_action`.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{ActivityPhoto, SessionUser};
use crate::web::api::{
    activity_photos_data, delete_activity_photo_action, reorder_activity_photos_action,
};
use crate::web::components::{DeviceFrame, EmptyState, FetchError, MobileHeader};

#[component]
pub fn GaleriPage() -> impl IntoView {
    let session = use_context::<Resource<Option<SessionUser>>>();
    // `manage` di-set lewat Effect (hanya jalan di KLIEN pasca-hydration). Jadi
    // SSR & render awal klien sama-sama `false` → tak ada hydration-mismatch;
    // kontrol kelola muncul setelah sesi ter-resolve.
    let manage = RwSignal::new(false);
    Effect::new(move |_| {
        let m = session
            .and_then(|s| s.get())
            .flatten()
            .map(|u| matches!(u.role.as_str(), "admin" | "ketua" | "dewan_guru"))
            .unwrap_or(false);
        manage.set(m);
    });

    let data = Resource::new(|| (), |_| async move { activity_photos_data().await });

    // Mirror lokal: reorder optimistik + persist tanpa refetch tiap drag.
    let items: RwSignal<Vec<ActivityPhoto>> = RwSignal::new(vec![]);
    let initialized = RwSignal::new(false);
    Effect::new(move |_| {
        if let Some(Ok(list)) = data.get() {
            if !initialized.get_untracked() {
                items.set(list);
                initialized.set(true);
            }
        }
    });

    let drag_from: RwSignal<Option<usize>> = RwSignal::new(None);
    let uploading = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    // Simpan urutan sekarang ke server (dipanggil setelah drop).
    let persist_order = move || {
        let ids: Vec<i64> = items.get_untracked().iter().map(|p| p.id).collect();
        leptos::task::spawn_local(async move {
            let _ = reorder_activity_photos_action(ids).await;
        });
    };
    let move_item = move |from: usize, to: usize| {
        items.update(|v| {
            if from < v.len() && to < v.len() && from != to {
                let it = v.remove(from);
                v.insert(to, it);
            }
        });
        drag_from.set(None);
        persist_order();
    };

    let file_input: NodeRef<leptos::html::Input> = NodeRef::new();
    let on_pick = move |_ev: leptos::ev::Event| {
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            let Some(input) = file_input.get() else { return };
            let Some(files) = input.files() else { return };
            let n = files.length();
            if n == 0 {
                return;
            }
            uploading.set(true);
            msg.set(None);
            leptos::task::spawn_local(async move {
                let mut ok_count = 0u32;
                for i in 0..n {
                    let Some(file) = files.get(i) else { continue };
                    let form = web_sys::FormData::new().unwrap();
                    let _ = form.append_with_blob("file", &file);
                    let window = web_sys::window().unwrap();
                    let opts = web_sys::RequestInit::new();
                    opts.set_method("POST");
                    opts.set_body(form.as_ref());
                    let req = match web_sys::Request::new_with_str_and_init(
                        "/api/activity-photos/upload",
                        &opts,
                    ) {
                        Ok(r) => r,
                        Err(_) => continue,
                    };
                    if let Ok(resp) =
                        wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&req)).await
                    {
                        let resp: web_sys::Response = resp.dyn_into().unwrap();
                        if resp.ok() {
                            if let Ok(js) =
                                wasm_bindgen_futures::JsFuture::from(resp.json().unwrap()).await
                            {
                                let id = js_sys::Reflect::get(
                                    &js,
                                    &wasm_bindgen::JsValue::from_str("id"),
                                )
                                .ok()
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0) as i64;
                                let url = js_sys::Reflect::get(
                                    &js,
                                    &wasm_bindgen::JsValue::from_str("url"),
                                )
                                .ok()
                                .and_then(|v| v.as_string())
                                .unwrap_or_default();
                                if id > 0 && !url.is_empty() {
                                    items.update(|v| {
                                        let ord = v.len() as i32;
                                        v.push(ActivityPhoto {
                                            id,
                                            url,
                                            caption: String::new(),
                                            sort_order: ord,
                                        });
                                    });
                                    ok_count += 1;
                                }
                            }
                        }
                    }
                }
                if let Some(inp) = file_input.get() {
                    inp.set_value("");
                }
                uploading.set(false);
                msg.set(Some(if ok_count == 0 {
                    (false, "Upload gagal — periksa file/koneksi.".into())
                } else {
                    (true, format!("{ok_count} foto terunggah."))
                }));
            });
        }
        let _ = &file_input;
    };

    let delete_photo = move |id: i64| {
        items.update(|v| v.retain(|p| p.id != id));
        leptos::task::spawn_local(async move {
            let _ = delete_activity_photo_action(id).await;
        });
    };

    view! {
        <Title text="Galeri Foto Kegiatan — PPM AFM" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader
                    title="Galeri Foto Kegiatan"
                    subtitle="Foto yang tampil di beranda publik"
                    back_href="/staf"
                />

                <div class="px-5 pt-5 space-y-4 stagger">
                    // ── Panel unggah (khusus admin/dewan guru) ────────────────
                    <Show when=move || manage.get() fallback=|| ()>
                        <div class="ppm-card p-4 space-y-3">
                            <h3 class="text-body-md font-bold text-on-background flex items-center gap-2">
                                <span class="material-symbols-outlined text-primary">"cloud_upload"</span>
                                "Unggah Foto"
                            </h3>
                            <p class="text-body-sm text-on-surface-variant">
                                "Pilih beberapa foto sekaligus (jpg/png/webp, maks 10MB). Seret foto di bawah untuk mengubah urutan tampil."
                            </p>
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
                                type="file"
                                node_ref=file_input
                                accept="image/jpeg,image/png,image/webp,image/gif"
                                multiple=true
                                class="w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface"
                                on:change=on_pick
                            />
                            {move || {
                                uploading
                                    .get()
                                    .then(|| view! {
                                        <p class="text-body-sm text-primary flex items-center gap-2">
                                            <span class="material-symbols-outlined animate-spin text-[18px]">"autorenew"</span>
                                            "Mengunggah…"
                                        </p>
                                    })
                            }}
                        </div>
                    </Show>

                    // ── Grid foto (drag untuk urutkan, hapus per foto) ─────────
                    <Suspense fallback=|| {
                        view! {
                            <div class="grid grid-cols-2 gap-3 animate-pulse">
                                <div class="aspect-square bg-surface-container rounded-2xl"></div>
                                <div class="aspect-square bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                    Ok(_) => {
                                        view! {
                                            {move || {
                                                if items.get().is_empty() {
                                                    view! {
                                                        <EmptyState
                                                            icon="grid_on"
                                                            title="Belum ada foto"
                                                            subtitle="Unggah foto kegiatan lewat panel di atas."
                                                        />
                                                    }
                                                        .into_any()
                                                } else {
                                                    view! {
                                                        <div class="grid grid-cols-2 md:grid-cols-3 gap-3">
                                                            {move || {
                                                                // Baca peran reaktif: grid re-render saat `manage` flip.
                                                                let mgr = manage.get();
                                                                items
                                                                    .get()
                                                                    .into_iter()
                                                                    .enumerate()
                                                                    .map(|(idx, p)| {
                                                                        let pid = p.id;
                                                                        let url = p.url.clone();
                                                                        let dim = move || {
                                                                            if drag_from.get() == Some(idx) { "opacity:.4" } else { "" }
                                                                        };
                                                                        view! {
                                                                            <div
                                                                                draggable=if mgr { "true" } else { "false" }
                                                                                style=move || format!(
                                                                                    "position:relative;aspect-ratio:1;border-radius:16px;overflow:hidden;{}{}",
                                                                                    if mgr { "cursor:grab;" } else { "" }, dim(),
                                                                                )
                                                                                on:dragstart=move |_| { if mgr { drag_from.set(Some(idx)); } }
                                                                                on:dragover=move |e| { if mgr { e.prevent_default(); } }
                                                                                on:drop=move |e| {
                                                                                    if mgr {
                                                                                        e.prevent_default();
                                                                                        if let Some(from) = drag_from.get_untracked() {
                                                                                            move_item(from, idx);
                                                                                        }
                                                                                    }
                                                                                }
                                                                                on:dragend=move |_| drag_from.set(None)
                                                                            >
                                                                                <img
                                                                                    src=url
                                                                                    style="width:100%;height:100%;object-fit:cover"
                                                                                    alt="Foto kegiatan"
                                                                                />
                                                                                {mgr
                                                                                    .then(|| view! {
                                                                                        <button
                                                                                            class="absolute top-1.5 right-1.5 w-7 h-7 rounded-lg bg-black/55 text-white flex items-center justify-center"
                                                                                            on:click=move |_| delete_photo(pid)
                                                                                            aria-label="Hapus foto"
                                                                                        >
                                                                                            <span class="material-symbols-outlined text-[16px]">"delete"</span>
                                                                                        </button>
                                                                                    })}
                                                                            </div>
                                                                        }
                                                                    })
                                                                    .collect_view()
                                                            }}
                                                        </div>
                                                    }
                                                        .into_any()
                                                }
                                            }}
                                        }
                                            .into_any()
                                    }
                                })
                        }}
                    </Suspense>
                </div>
            </div>
        </DeviceFrame>
    }
}
