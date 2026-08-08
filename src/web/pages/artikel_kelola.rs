//! web/pages/artikel_kelola.rs — Kelola artikel halaman depan (migrasi 69).
//! Admin/ketua: tulis, sunting, terbitkan/tarik ke draf, hapus.
//!
//! Daftar di sini menampilkan DRAF juga (server fn `articles_admin_data`),
//! sedangkan halaman publik hanya yang terbit. Itu sebabnya keduanya memakai
//! endpoint berbeda alih-alih satu daftar yang disaring di klien: draf tak boleh
//! ikut terkirim ke pengunjung sama sekali.
//!
//! Alamat publik (`slug`) dibuat dari judul saat artikel PERTAMA kali disimpan
//! dan tak berubah lagi setelah itu — tautan yang sudah dibagikan tak boleh
//! mati hanya karena judulnya dirapikan.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{slugify, Article};
use crate::web::api::{articles_admin_data, delete_article_action, save_article_action};
use crate::web::components::{DeviceFrame, EmptyState, FetchError, MobileHeader, Sheet};

#[component]
pub fn KelolaArtikelPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async move { articles_admin_data().await });

    Effect::new(move |_| {
        if let Some(Err(e)) = data.get() {
            let msg = e.to_string();
            if crate::web::components::is_auth_error(&msg) || msg.contains("forbidden") {
                #[cfg(target_arch = "wasm32")]
                if let Some(w) = web_sys::window() {
                    let _ = w.location().replace("/login");
                }
            }
        }
    });

    // `None` = tak ada form terbuka. `Some(None)` = artikel BARU.
    // `Some(Some(a))` = sedang menyunting `a`.
    let editing: RwSignal<Option<Option<Article>>> = RwSignal::new(None);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let hapus = move |id: i64, judul: String| {
        #[cfg(target_arch = "wasm32")]
        {
            let ok = web_sys::window()
                .and_then(|w| {
                    w.confirm_with_message(&format!("Hapus artikel “{judul}”? Tak bisa dibatalkan."))
                        .ok()
                })
                .unwrap_or(false);
            if !ok {
                return;
            }
        }
        let _ = &judul;
        leptos::task::spawn_local(async move {
            match delete_article_action(id).await {
                Ok(()) => {
                    data.refetch();
                    msg.set(Some((true, "Artikel dihapus.".into())));
                }
                Err(e) => msg.set(Some((false, e.to_string()))),
            }
        });
    };

    view! {
        <Title text="Kelola Artikel — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader
                    title="Kelola Artikel"
                    subtitle="Tulisan yang tampil di halaman depan"
                    back_href="/staf"
                />

                <div class="px-5 pt-5 space-y-4 stagger">
                    <div class="flex items-center gap-2">
                        <button
                            class="flex items-center gap-2 px-5 py-2.5 bg-primary text-on-primary rounded-xl text-body-sm font-semibold press cursor-pointer"
                            on:click=move |_| {
                                msg.set(None);
                                editing.set(Some(None));
                            }
                        >
                            <span class="material-symbols-outlined text-lg">"edit_note"</span>
                            "Tulis Artikel"
                        </button>
                        <a
                            href="/artikel"
                            class="flex items-center gap-2 px-4 py-2.5 border border-outline-variant rounded-xl text-body-sm font-semibold text-on-surface hover:border-primary hover:text-primary transition-colors"
                        >
                            <span class="material-symbols-outlined text-lg">"public"</span>
                            "Lihat Publik"
                        </a>
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

                    <Suspense fallback=|| {
                        view! {
                            <div class="space-y-3 animate-pulse">
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                                <div class="h-24 bg-surface-container rounded-2xl"></div>
                            </div>
                        }
                    }>
                        {move || {
                            data.get()
                                .map(|res| match res {
                                    Err(e) => view! { <FetchError err=e.to_string() /> }.into_any(),
                                    Ok(list) if list.is_empty() => {
                                        view! {
                                            <EmptyState
                                                icon="article"
                                                title="Belum ada artikel"
                                                subtitle="Tulis artikel pertama lewat tombol di atas."
                                            />
                                        }
                                            .into_any()
                                    }
                                    Ok(list) => {
                                        view! {
                                            <div class="space-y-3 md:grid md:grid-cols-2 md:gap-3 md:space-y-0">
                                                {list
                                                    .into_iter()
                                                    .map(|a| {
                                                        let id = a.id;
                                                        let judul = a.title.clone();
                                                        let untuk_edit = a.clone();
                                                        view! {
                                                            <div class="ppm-card p-4 flex gap-3">
                                                                {match a.cover_url.clone() {
                                                                    Some(url) => {
                                                                        view! {
                                                                            <img
                                                                                src=url
                                                                                alt=""
                                                                                loading="lazy"
                                                                                class="w-16 h-16 rounded-xl object-cover bg-surface-container shrink-0"
                                                                            />
                                                                        }
                                                                            .into_any()
                                                                    }
                                                                    None => {
                                                                        view! {
                                                                            <span class="w-16 h-16 ppm-tile rounded-xl shrink-0">
                                                                                <span class="material-symbols-outlined">"article"</span>
                                                                            </span>
                                                                        }
                                                                            .into_any()
                                                                    }
                                                                }}
                                                                <div class="flex-1 min-w-0">
                                                                    <div class="flex items-start gap-2">
                                                                        <p class="text-body-md font-bold text-on-background flex-1 min-w-0">
                                                                            {a.title.clone()}
                                                                        </p>
                                                                        <span class=if a.published {
                                                                            "ppm-chip-sm bg-success/10 text-success shrink-0"
                                                                        } else {
                                                                            "ppm-chip-sm bg-surface-container-high text-on-surface-variant shrink-0"
                                                                        }>
                                                                            {if a.published { "TERBIT" } else { "DRAF" }}
                                                                        </span>
                                                                    </div>
                                                                    <p class="text-body-sm text-on-surface-variant mt-0.5 line-clamp-2">
                                                                        {a.summary()}
                                                                    </p>
                                                                    <p class="text-[11px] text-on-surface-variant/70 mt-1 truncate">
                                                                        {format!("{} · /artikel/{}", a.created_at, a.slug)}
                                                                    </p>
                                                                    <div class="flex items-center gap-2 mt-2">
                                                                        <button
                                                                            class="px-3 py-1.5 rounded-lg border border-outline-variant text-body-sm font-semibold text-on-surface hover:border-primary hover:text-primary transition-colors cursor-pointer"
                                                                            on:click=move |_| {
                                                                                msg.set(None);
                                                                                editing.set(Some(Some(untuk_edit.clone())));
                                                                            }
                                                                        >
                                                                            "Sunting"
                                                                        </button>
                                                                        <button
                                                                            class="px-3 py-1.5 rounded-lg border border-error/40 text-body-sm font-semibold text-error cursor-pointer"
                                                                            on:click=move |_| hapus(id, judul.clone())
                                                                        >
                                                                            "Hapus"
                                                                        </button>
                                                                    </div>
                                                                </div>
                                                            </div>
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
                </div>

                {move || {
                    editing
                        .get()
                        .map(|slot| {
                            let judul = if slot.is_some() { "Sunting Artikel" } else { "Artikel Baru" };
                            view! {
                                <Sheet title=judul on_close=move || editing.set(None)>
                                    <ArtikelForm
                                        awal=slot.clone()
                                        on_saved=move |teks| {
                                            editing.set(None);
                                            data.refetch();
                                            msg.set(Some((true, teks)));
                                        }
                                    />
                                </Sheet>
                            }
                        })
                }}
            </div>
        </DeviceFrame>
    }
}

/// Form tulis/sunting satu artikel.
///
/// Ringkasan boleh kosong: kartu di halaman depan jatuh ke awal isi
/// (`Article::summary`). Yang WAJIB hanya judul — tanpa itu tak ada yang bisa
/// dijadikan alamat maupun tautan.
#[component]
fn ArtikelForm(
    /// `None` = artikel baru.
    awal: Option<Article>,
    on_saved: impl Fn(String) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let id = awal.as_ref().map(|a| a.id);
    let slug_lama = awal.as_ref().map(|a| a.slug.clone());
    let title = RwSignal::new(awal.as_ref().map(|a| a.title.clone()).unwrap_or_default());
    let excerpt = RwSignal::new(awal.as_ref().map(|a| a.excerpt.clone()).unwrap_or_default());
    let body = RwSignal::new(awal.as_ref().map(|a| a.body.clone()).unwrap_or_default());
    let cover = RwSignal::new(
        awal.as_ref().and_then(|a| a.cover_url.clone()).unwrap_or_default(),
    );
    // Status terbit TIDAK disimpan sebagai signal: ia ditentukan oleh TOMBOL
    // yang ditekan ("Simpan Draf" / "Terbitkan"), bukan oleh isian form. Satu
    // signal untuknya hanya akan jadi salinan yang tak pernah dibaca.
    let busy = RwSignal::new(false);
    let uploading = RwSignal::new(false);
    let err = RwSignal::new(String::new());
    let cover_input: NodeRef<leptos::html::Input> = NodeRef::new();

    // Alamat yang akan terbentuk — diperlihatkan sambil mengetik supaya
    // pengelola tahu tautan apa yang akan dibagikan. Untuk artikel yang sudah
    // ada, yang ditampilkan adalah slug TERSIMPAN: judul boleh berubah, alamat
    // tidak.
    let alamat = move || {
        slug_lama.clone().unwrap_or_else(|| {
            let s = slugify(&title.get());
            if s.is_empty() { "…".to_string() } else { s }
        })
    };

    let pick_cover = move |_ev: leptos::ev::Event| {
        err.set(String::new());
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            let Some(file) = cover_input
                .get_untracked()
                .and_then(|i| i.files())
                .and_then(|f| f.get(0))
            else {
                return;
            };
            uploading.set(true);
            leptos::task::spawn_local(async move {
                let form = web_sys::FormData::new().unwrap();
                let _ = form.append_with_blob("file", &file);
                let window = web_sys::window().unwrap();
                let opts = web_sys::RequestInit::new();
                opts.set_method("POST");
                opts.set_body(form.as_ref());
                let mut pesan = String::new();
                if let Ok(req) =
                    web_sys::Request::new_with_str_and_init("/api/articles/cover", &opts)
                {
                    if let Ok(resp) =
                        wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&req)).await
                    {
                        let resp: web_sys::Response = resp.dyn_into().unwrap();
                        if resp.ok() {
                            if let Ok(js) =
                                wasm_bindgen_futures::JsFuture::from(resp.json().unwrap()).await
                            {
                                if let Some(url) = js_sys::Reflect::get(
                                    &js,
                                    &wasm_bindgen::JsValue::from_str("url"),
                                )
                                .ok()
                                .and_then(|v| v.as_string())
                                {
                                    cover.set(url);
                                }
                            }
                        } else {
                            pesan = wasm_bindgen_futures::JsFuture::from(resp.text().unwrap())
                                .await
                                .ok()
                                .and_then(|t| t.as_string())
                                .unwrap_or_else(|| "Unggah sampul gagal.".into());
                        }
                    }
                }
                uploading.set(false);
                if !pesan.is_empty() {
                    err.set(pesan);
                }
                if let Some(inp) = cover_input.get_untracked() {
                    inp.set_value("");
                }
            });
        }
        let _ = (&cover_input, &uploading, &cover, &err);
    };

    let simpan = move |terbit: bool| {
        if busy.get_untracked() || uploading.get_untracked() {
            return;
        }
        if title.get_untracked().trim().is_empty() {
            err.set("Judul artikel wajib diisi.".into());
            return;
        }
        err.set(String::new());
        busy.set(true);
        let (t, e, b, c) = (
            title.get_untracked(),
            excerpt.get_untracked(),
            body.get_untracked(),
            cover.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            let hasil = save_article_action(id, t, e, b, c, terbit).await;
            busy.set(false);
            match hasil {
                Ok(_) => on_saved(
                    if terbit { "Artikel diterbitkan." } else { "Disimpan sebagai draf." }
                        .to_string(),
                ),
                Err(er) => err.set(er.to_string()),
            }
        });
    };

    view! {
        <label class="block text-body-sm font-semibold text-on-background mb-1.5">"Judul"</label>
        <input
            type="text"
            maxlength="200"
            placeholder="Mis. Wisuda Santri Angkatan 2025"
            class="w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface"
            prop:value=move || title.get()
            on:input=move |ev| title.set(event_target_value(&ev))
        />
        <p class="text-[11px] text-on-surface-variant mt-1 mb-4 truncate">
            {move || format!("Alamat publik: /artikel/{}", alamat())}
        </p>

        <label class="block text-body-sm font-semibold text-on-background mb-1.5">
            "Ringkasan " <span class="font-normal opacity-70">"(opsional)"</span>
        </label>
        <textarea
            rows="2"
            maxlength="400"
            placeholder="Satu-dua kalimat untuk kartu di halaman depan."
            class="w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface mb-4"
            prop:value=move || excerpt.get()
            on:input=move |ev| excerpt.set(event_target_value(&ev))
        ></textarea>

        <label class="block text-body-sm font-semibold text-on-background mb-1.5">"Isi"</label>
        <textarea
            rows="10"
            placeholder="Tulis isi artikel. Pisahkan paragraf dengan satu baris kosong."
            class="w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface mb-4"
            prop:value=move || body.get()
            on:input=move |ev| body.set(event_target_value(&ev))
        ></textarea>

        <label class="block text-body-sm font-semibold text-on-background mb-1.5">
            "Gambar sampul " <span class="font-normal opacity-70">"(opsional)"</span>
        </label>
        {move || {
            let url = cover.get();
            (!url.is_empty())
                .then(|| {
                    view! {
                        <div class="relative mb-2">
                            <img
                                src=url.clone()
                                alt="Sampul artikel"
                                class="w-full aspect-[16/9] object-cover rounded-xl bg-surface-container"
                            />
                            <button
                                class="absolute top-2 right-2 w-8 h-8 rounded-lg bg-black/55 text-white flex items-center justify-center cursor-pointer"
                                on:click=move |_| cover.set(String::new())
                                aria-label="Hapus sampul"
                            >
                                <span class="material-symbols-outlined text-[18px]">"delete"</span>
                            </button>
                        </div>
                    }
                })
        }}
        <input
            type="file"
            node_ref=cover_input
            accept="image/jpeg,image/png,image/webp,image/gif"
            class="w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface"
            on:change=pick_cover
        />
        <Show when=move || uploading.get()>
            <p class="text-body-sm text-on-surface-variant mt-1.5 flex items-center gap-1">
                <span class="material-symbols-outlined text-[16px] pulse-dot">"sync"</span>
                "Mengunggah sampul…"
            </p>
        </Show>

        <Show when=move || !err.get().is_empty()>
            <div class="mt-4 p-2.5 bg-error-container text-on-error-container rounded-lg text-body-sm">
                {move || err.get()}
            </div>
        </Show>

        <div class="mt-5 flex gap-2">
            <button
                class="flex-1 py-3 rounded-xl border-2 border-outline-variant text-on-surface-variant font-semibold press cursor-pointer disabled:opacity-60"
                prop:disabled=move || busy.get() || uploading.get()
                on:click=move |_| simpan(false)
            >
                "Simpan Draf"
            </button>
            <button
                class="flex-1 py-3 rounded-xl bg-primary text-on-primary font-semibold press cursor-pointer disabled:opacity-60"
                prop:disabled=move || busy.get() || uploading.get()
                on:click=move |_| simpan(true)
            >
                {move || if busy.get() { "Menyimpan…" } else { "Terbitkan" }}
            </button>
        </div>
    }
}
