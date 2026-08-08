//! web/pages/galeri.rs — Galeri media pondok (migrasi 34 & 69). Kelola apa yang
//! tampil di halaman depan publik: unggah banyak sekaligus, geser (drag-and-drop
//! asli) untuk mengubah urutan, sunting keterangan, dan hapus. Hanya
//! admin/dewan_guru yang bisa mengelola.
//!
//! TIGA KATEGORI (migrasi 69), dipilih lewat tab:
//!   Video Utama → media yang berjalan di kepala halaman depan (yang TERATAS
//!                 yang dipakai; sisanya cadangan yang tinggal digeser ke atas)
//!   Kegiatan    → grid foto kegiatan santri
//!   Fasilitas   → grid foto sarana pondok
//!
//! ALUR UNGGAH: pilih berkas → **isi keterangan & atur bidikan dulu** → baru
//! terkirim. Semuanya ikut di request unggah yang sama, jadi media tak pernah
//! sempat tampil di halaman depan tanpa keterangan atau dengan bidikan yang
//! bukan pilihan siapa pun. Berkas diunggah satu per satu lewat
//! POST /api/activity-photos/upload (multipart, di luar server-fn).
//! Urutan disimpan via `reorder_activity_photos_action`.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{
    frame_style_of, ActivityPhoto, MediaCategory, PhotoFit, SessionUser, FOCUS_DEFAULT,
};
use crate::web::api::{
    activity_photos_data, delete_activity_photo_action, reorder_activity_photos_action,
    set_activity_photo_focus_action, set_activity_photo_meta_action,
};
use crate::web::components::{
    DeviceFrame, EmptyState, FetchError, MediaFrame, MobileHeader, Sheet,
};

/// Semua nilai yang bisa diubah pengelola untuk satu media — dikirim sekaligus
/// oleh editor. Digabung dalam satu struct, bukan enam parameter berderet,
/// karena keduanya (unggah baru & sunting tersimpan) memakai daftar yang sama
/// dan urutan enam argumen bertipe mirip adalah tempat yang bagus untuk salah.
#[derive(Clone, Debug)]
struct MediaDraft {
    focus_x: f32,
    focus_y: f32,
    zoom: f32,
    fit: String,
    caption: String,
    category: MediaCategory,
}

/// Berkas apa saja yang boleh dipilih untuk sebuah kategori.
///
/// Video hanya masuk akal di kepala halaman depan; grid kegiatan & fasilitas
/// menampilkan puluhan petak sekaligus, dan video di sana berarti berbelas MB
/// terunduh untuk petak yang mungkin tak pernah dilihat.
fn accept_for(cat: MediaCategory) -> &'static str {
    match cat {
        MediaCategory::VideoUtama => {
            "video/mp4,video/webm,image/jpeg,image/png,image/webp,image/gif"
        }
        _ => "image/jpeg,image/png,image/webp,image/gif",
    }
}

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

    // Tab kategori yang sedang dilihat — sekaligus kategori bawaan untuk berkas
    // yang baru dipilih (pengelola yang sedang membuka tab "Fasilitas" hampir
    // pasti sedang mengunggah foto fasilitas).
    let tab = RwSignal::new(MediaCategory::Kegiatan);

    let drag_from: RwSignal<Option<usize>> = RwSignal::new(None);
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    // ── Antrean unggah: berkas yang SUDAH dipilih tapi BELUM dikirim ──────────
    //
    // Berkasnya sendiri tetap tinggal di elemen <input> dan diambil ulang lewat
    // indeks saat dibutuhkan. Itu sengaja: `web_sys::File` bukan tipe yang bisa
    // disimpan di signal Leptos (tak `Send`/`Sync`), dan membungkusnya hanya
    // untuk itu berarti kode khusus-wasm merembes ke seluruh komponen. Yang
    // disimpan di sini cukup angka, satu URL pratinjau, dan satu penanda jenis
    // — semuanya biasa saja di kedua target build.
    //
    // `<input>` baru dikosongkan setelah SELURUH antrean selesai; mengosongkannya
    // lebih awal akan membuang berkas yang belum sempat diunggah.
    let pending_total = RwSignal::new(0usize);
    let pending_idx = RwSignal::new(0usize);
    // URL objek (blob:) pratinjau berkas yang sedang diatur.
    let pending_src = RwSignal::new(String::new());
    // Berkas yang sedang diatur berupa video? Menentukan bentuk editornya.
    let pending_video = RwSignal::new(false);

    // Media TERSIMPAN yang sedang disunting ulang lewat tombol "Atur".
    let editing: RwSignal<Option<ActivityPhoto>> = RwSignal::new(None);

    let file_input: NodeRef<leptos::html::Input> = NodeRef::new();

    // Lepas URL objek pratinjau. Tanpa ini, tiap berkas yang dipilih menahan
    // salinan gambarnya di memori tab sampai halaman ditutup.
    let revoke_preview = move || {
        #[cfg(target_arch = "wasm32")]
        {
            let old = pending_src.get_untracked();
            if !old.is_empty() {
                let _ = web_sys::Url::revoke_object_url(&old);
            }
        }
        pending_src.set(String::new());
    };

    // Siapkan pratinjau untuk berkas ke-`idx`; false bila indeks habis.
    // Hanya dipanggil dari jalur wasm (pemilihan berkas & unggah) — di build SSR
    // memang tak terpakai, sama seperti sisa alur unggah di halaman ini.
    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
    let show_pending = move |idx: usize| -> bool {
        revoke_preview();
        #[cfg(target_arch = "wasm32")]
        {
            let Some(input) = file_input.get_untracked() else { return false };
            let Some(files) = input.files() else { return false };
            let Some(file) = files.get(idx as u32) else { return false };
            // Jenis dari `File.type` (ditetapkan browser dari isi/ekstensi) —
            // server tetap memeriksa magic number-nya, ini cuma untuk memilih
            // bentuk pratinjau.
            let is_video = file.type_().starts_with("video/");
            match web_sys::Url::create_object_url_with_blob(&file) {
                Ok(u) => {
                    pending_idx.set(idx);
                    pending_video.set(is_video);
                    pending_src.set(u);
                    return true;
                }
                Err(_) => return false,
            }
        }
        #[allow(unreachable_code)]
        {
            let _ = (idx, &file_input, &pending_idx, &pending_video);
            false
        }
    };

    // Antrean unggah selesai / dibatalkan → bersihkan semuanya.
    let finish_batch = move || {
        revoke_preview();
        pending_total.set(0);
        pending_idx.set(0);
        pending_video.set(false);
        if let Some(inp) = file_input.get_untracked() {
            inp.set_value("");
        }
    };

    let on_pick = move |_ev: leptos::ev::Event| {
        msg.set(None);
        #[cfg(target_arch = "wasm32")]
        {
            let Some(input) = file_input.get_untracked() else { return };
            let Some(files) = input.files() else { return };
            let n = files.length() as usize;
            if n == 0 {
                return;
            }
            pending_total.set(n);
            // Buka editor untuk berkas pertama. Belum ada yang terkirim —
            // pengiriman terjadi saat pengelola menekan "Unggah" di editor.
            if !show_pending(0) {
                finish_batch();
            }
        }
        let _ = (&file_input, &pending_total, &msg);
    };

    // Kirim berkas yang sedang diatur, LENGKAP dengan keterangan, kategori, dan
    // bidikannya, lalu lanjut ke berkas berikutnya dalam antrean.
    let upload_current = move |d: MediaDraft| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            let idx = pending_idx.get_untracked();
            let total = pending_total.get_untracked();
            let file = file_input
                .get_untracked()
                .and_then(|i| i.files())
                .and_then(|f| f.get(idx as u32));
            let Some(file) = file else {
                busy.set(false);
                finish_batch();
                return;
            };
            leptos::task::spawn_local(async move {
                let form = web_sys::FormData::new().unwrap();
                let _ = form.append_with_blob("file", &file);
                let _ = form.append_with_str("focus_x", &d.focus_x.to_string());
                let _ = form.append_with_str("focus_y", &d.focus_y.to_string());
                let _ = form.append_with_str("zoom", &d.zoom.to_string());
                let _ = form.append_with_str("fit", &d.fit);
                let _ = form.append_with_str("caption", &d.caption);
                let _ = form.append_with_str("category", d.category.as_str());

                let window = web_sys::window().unwrap();
                let opts = web_sys::RequestInit::new();
                opts.set_method("POST");
                opts.set_body(form.as_ref());
                let mut ok = false;
                let mut gagal = String::new();
                if let Ok(req) = web_sys::Request::new_with_str_and_init(
                    "/api/activity-photos/upload",
                    &opts,
                ) {
                    if let Ok(resp) =
                        wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&req)).await
                    {
                        let resp: web_sys::Response = resp.dyn_into().unwrap();
                        if resp.ok() {
                            if let Ok(js) =
                                wasm_bindgen_futures::JsFuture::from(resp.json().unwrap()).await
                            {
                                let get_num = |k: &str| {
                                    js_sys::Reflect::get(&js, &wasm_bindgen::JsValue::from_str(k))
                                        .ok()
                                        .and_then(|v| v.as_f64())
                                };
                                let get_str = |k: &str| {
                                    js_sys::Reflect::get(&js, &wasm_bindgen::JsValue::from_str(k))
                                        .ok()
                                        .and_then(|v| v.as_string())
                                };
                                let id = get_num("id").unwrap_or(0.0) as i64;
                                let url = get_str("url").unwrap_or_default();
                                if id > 0 && !url.is_empty() {
                                    // Pakai nilai yang DIKEMBALIKAN server: server
                                    // merapikan bidikan & kategori ke nilai sahnya,
                                    // jadi memakai nilai kiriman sendiri bisa membuat
                                    // grid berbeda dari yang tersimpan.
                                    items.update(|v| {
                                        let ord = v.len() as i32;
                                        v.push(ActivityPhoto {
                                            id,
                                            url,
                                            caption: get_str("caption").unwrap_or_default(),
                                            sort_order: ord,
                                            focus_x: get_num("focus_x").unwrap_or(0.5) as f32,
                                            focus_y: get_num("focus_y").unwrap_or(0.5) as f32,
                                            zoom: get_num("zoom").unwrap_or(1.0) as f32,
                                            fit: get_str("fit")
                                                .unwrap_or_else(|| "cover".into()),
                                            category: get_str("category")
                                                .unwrap_or_else(|| "kegiatan".into()),
                                            media_type: get_str("media_type")
                                                .unwrap_or_else(|| "image".into()),
                                        });
                                    });
                                    ok = true;
                                }
                            }
                        } else {
                            // Server menjelaskan penolakannya dalam bahasa
                            // Indonesia (jenis file, ukuran) — jauh lebih
                            // berguna daripada "unggah gagal" yang generik.
                            gagal = wasm_bindgen_futures::JsFuture::from(resp.text().unwrap())
                                .await
                                .ok()
                                .and_then(|t| t.as_string())
                                .unwrap_or_default();
                        }
                    }
                }
                busy.set(false);
                if !ok {
                    let teks = if gagal.trim().is_empty() {
                        "Unggah gagal — periksa berkas/koneksi, lalu coba lagi.".to_string()
                    } else {
                        gagal
                    };
                    msg.set(Some((false, teks)));
                    // Berhenti di berkas yang gagal; sisa antrean dibuang agar
                    // pengelola tak menebak-nebak mana yang sudah masuk.
                    finish_batch();
                    return;
                }
                let next = idx + 1;
                if next < total && show_pending(next) {
                    return; // editor lanjut ke berkas berikutnya
                }
                finish_batch();
                msg.set(Some((true, format!("{total} media terunggah."))));
            });
        }
        // Di build SSR seluruh blok di atas tak ada, jadi parameternya menganggur.
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (d, &busy, &msg);
    };

    // Simpan suntingan media yang SUDAH tersimpan (tombol "Atur" di kartu).
    //
    // Dua server fn dipanggil berurutan karena memang dua hal berbeda di tabel:
    // bidikan (migrasi 54/55) dan keterangan+kategori (migrasi 69). Bidikan
    // dilewati untuk video — editornya memang tak menawarkannya, jadi mengirim
    // nilai bawaan justru akan MENGHAPUS bidikan yang mungkin sudah diatur.
    let save_existing = move |id: i64, is_video: bool, d: MediaDraft| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            let mut ok = set_activity_photo_meta_action(
                id,
                d.caption.clone(),
                d.category.as_str().to_string(),
            )
            .await
            .is_ok();
            if ok && !is_video {
                ok = set_activity_photo_focus_action(id, d.focus_x, d.focus_y, d.zoom, d.fit.clone())
                    .await
                    .is_ok();
            }
            busy.set(false);
            if ok {
                items.update(|v| {
                    if let Some(p) = v.iter_mut().find(|p| p.id == id) {
                        p.caption = d.caption.clone();
                        p.category = d.category.as_str().to_string();
                        if !is_video {
                            p.focus_x = d.focus_x;
                            p.focus_y = d.focus_y;
                            p.zoom = d.zoom;
                            p.fit = d.fit.clone();
                        }
                    }
                });
                editing.set(None);
            } else {
                msg.set(Some((false, "Gagal menyimpan — coba lagi.".into())));
            }
        });
    };

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

    let delete_photo = move |id: i64| {
        items.update(|v| v.retain(|p| p.id != id));
        leptos::task::spawn_local(async move {
            let _ = delete_activity_photo_action(id).await;
        });
    };

    // Media pada tab aktif, LENGKAP dengan indeks globalnya. Indeks global yang
    // dibawa, bukan indeks dalam hasil saring: drag-reorder menyusun ulang
    // daftar penuh, dan indeks hasil saring akan memindahkan media yang salah
    // begitu ada satu saja media kategori lain di depannya.
    let visible = move || -> Vec<(usize, ActivityPhoto)> {
        let cat = tab.get();
        items
            .get()
            .into_iter()
            .enumerate()
            .filter(|(_, p)| p.category() == cat)
            .collect()
    };

    view! {
        <Title text="Galeri Media — AFM SMART" />
        <DeviceFrame>
            <div class="min-h-screen bg-surface pb-24 max-w-md mx-auto ppm-wide">
                <MobileHeader
                    title="Galeri Media"
                    subtitle="Video & foto yang tampil di halaman depan"
                    back_href="/staf"
                />

                <div class="px-5 pt-5 space-y-4 stagger">
                    // ── Tab kategori ──────────────────────────────────────────
                    <div class="flex gap-1 bg-surface-container rounded-xl p-1">
                        {MediaCategory::ALL
                            .into_iter()
                            .map(|c| {
                                let count = move || {
                                    items.get().iter().filter(|p| p.category() == c).count()
                                };
                                view! {
                                    <button
                                        class=move || {
                                            if tab.get() == c {
                                                "flex-1 py-2 px-2 rounded-lg bg-surface text-on-background shadow-sm text-body-sm font-semibold cursor-pointer"
                                            } else {
                                                "flex-1 py-2 px-2 rounded-lg text-on-surface-variant text-body-sm font-semibold cursor-pointer"
                                            }
                                        }
                                        aria-pressed=move || (tab.get() == c).to_string()
                                        on:click=move |_| tab.set(c)
                                    >
                                        {c.label()}
                                        <span class="ml-1 opacity-60">{move || format!("({})", count())}</span>
                                    </button>
                                }
                            })
                            .collect_view()}
                    </div>

                    // ── Panel unggah (khusus admin/dewan guru) ────────────────
                    <Show when=move || manage.get() fallback=|| ()>
                        <div class="ppm-card p-4 space-y-3">
                            <h3 class="text-body-md font-bold text-on-background flex items-center gap-2">
                                <span class="material-symbols-outlined text-primary">
                                    "cloud_upload"
                                </span>
                                {move || format!("Unggah ke {}", tab.get().label())}
                            </h3>
                            <p class="text-body-sm text-on-surface-variant">
                                {move || {
                                    match tab.get() {
                                        MediaCategory::VideoUtama => {
                                            "Video yang berjalan di kepala halaman depan \
                                             (mp4/webm, maks 100MB) — boleh juga foto. Yang \
                                             PALING ATAS yang dipakai; sisanya cadangan."
                                        }
                                        MediaCategory::Kegiatan => {
                                            "Foto kegiatan santri (jpg/png/webp, maks 10MB). \
                                             Setelah dipilih, isi keterangan & atur bidikannya — \
                                             baru terkirim."
                                        }
                                        MediaCategory::Fasilitas => {
                                            "Foto sarana pondok (jpg/png/webp, maks 10MB). \
                                             Keterangan dipakai sebagai nama fasilitas di \
                                             halaman depan."
                                        }
                                    }
                                }}
                            </p>
                            <p class="text-body-sm text-on-surface-variant">
                                "Seret media di bawah untuk mengubah urutan tampil."
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
                                accept=move || accept_for(tab.get())
                                multiple=true
                                class="w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface"
                                on:change=on_pick
                            />
                        </div>
                    </Show>

                    // ── Grid media (drag untuk urutkan, hapus per media) ───────
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
                                                if visible().is_empty() {
                                                    view! {
                                                        <EmptyState
                                                            icon="grid_on"
                                                            title="Belum ada media"
                                                            subtitle="Unggah lewat panel di atas."
                                                        />
                                                    }
                                                        .into_any()
                                                } else {
                                                    view! {
                                                        <div class="grid grid-cols-2 md:grid-cols-3 gap-3">
                                                            {move || {
                                                                // Baca peran reaktif: grid re-render saat `manage` flip.
                                                                let mgr = manage.get();
                                                                visible()
                                                                    .into_iter()
                                                                    .map(|(idx, p)| {
                                                                        let pid = p.id;
                                                                        let photo = p.clone();
                                                                        let caption = p.caption.clone();
                                                                        let is_video = p.is_video();
                                                                        let dim = move || {
                                                                            if drag_from.get() == Some(idx) { "opacity:.4" } else { "" }
                                                                        };
                                                                        view! {
                                                                            <div
                                                                                draggable=if mgr { "true" } else { "false" }
                                                                                style=move || format!(
                                                                                    "position:relative;border-radius:16px;overflow:hidden;{}{}",
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
                                                                                // Bidikan yang tersimpan ikut dipakai di grid ini,
                                                                                // bukan cuma di halaman depan — supaya yang dilihat
                                                                                // pengelola sama dengan yang dilihat pengunjung.
                                                                                <MediaFrame
                                                                                    src=p.url.clone()
                                                                                    style=p.frame_style()
                                                                                    video=is_video
                                                                                    backdrop=p.fit().needs_backdrop()
                                                                                    alt=p.caption.clone()
                                                                                    class="aspect-square"
                                                                                    lazy=true
                                                                                />
                                                                                {is_video
                                                                                    .then(|| view! {
                                                                                        <span class="absolute top-1.5 left-1.5 h-6 px-1.5 rounded-lg bg-black/55 text-white flex items-center gap-1 pointer-events-none">
                                                                                            <span class="material-symbols-outlined text-[14px]">"movie"</span>
                                                                                            <span class="text-[10px] font-semibold">"Video"</span>
                                                                                        </span>
                                                                                    })}
                                                                                {(!caption.trim().is_empty())
                                                                                    .then(|| view! {
                                                                                        <span class="absolute inset-x-0 bottom-0 px-2 py-1.5 pr-16 bg-gradient-to-t from-black/70 to-transparent text-white text-[11px] font-medium truncate pointer-events-none">
                                                                                            {caption.clone()}
                                                                                        </span>
                                                                                    })}
                                                                                {mgr
                                                                                    .then(|| view! {
                                                                                        <button
                                                                                            class="absolute top-1.5 right-1.5 w-7 h-7 rounded-lg bg-black/55 text-white flex items-center justify-center cursor-pointer"
                                                                                            on:click=move |_| delete_photo(pid)
                                                                                            aria-label="Hapus media"
                                                                                        >
                                                                                            <span class="material-symbols-outlined text-[16px]">"delete"</span>
                                                                                        </button>
                                                                                        <button
                                                                                            class="absolute bottom-1.5 right-1.5 h-7 px-2 rounded-lg bg-black/55 text-white flex items-center gap-1 cursor-pointer"
                                                                                            on:click=move |_| editing.set(Some(photo.clone()))
                                                                                            aria-label="Atur media"
                                                                                        >
                                                                                            <span class="material-symbols-outlined text-[16px]">"tune"</span>
                                                                                            <span class="text-[10px] font-semibold">"Atur"</span>
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

                // ── Editor: berkas BARU (sebelum diunggah) ─────────────────────
                {move || {
                    (pending_total.get() > 0 && !pending_src.get().is_empty())
                        .then(|| {
                            let (fx, fy, z) = FOCUS_DEFAULT;
                            let total = pending_total.get();
                            let nth = pending_idx.get() + 1;
                            view! {
                                <Sheet title="Sesuaikan Media Sebelum Diunggah" on_close=finish_batch>
                                    <MediaEditor
                                        src=pending_src.get()
                                        is_video=pending_video.get()
                                        focus_x=fx
                                        focus_y=fy
                                        zoom=z
                                        fit="cover"
                                        caption=""
                                        category=tab.get()
                                        commit_label="Unggah"
                                        busy=busy
                                        progress=(nth, total)
                                        on_commit=move |d| upload_current(d)
                                    />
                                </Sheet>
                            }
                        })
                }}

                // ── Editor: media yang SUDAH tersimpan ─────────────────────────
                {move || {
                    editing
                        .get()
                        .map(|p| {
                            let id = p.id;
                            let is_video = p.is_video();
                            view! {
                                <Sheet
                                    title="Sesuaikan Media"
                                    on_close=move || editing.set(None)
                                >
                                    <MediaEditor
                                        src=p.url.clone()
                                        is_video=is_video
                                        focus_x=p.focus_x
                                        focus_y=p.focus_y
                                        zoom=p.zoom
                                        fit=p.fit.clone()
                                        caption=p.caption.clone()
                                        category=p.category()
                                        commit_label="Simpan"
                                        busy=busy
                                        on_commit=move |d| save_existing(id, is_video, d)
                                    />
                                </Sheet>
                            }
                        })
                }}
            </div>
        </DeviceFrame>
    }
}

/// Editor satu media: **keterangan, kategori, dan — untuk foto — bidikan
/// (seret untuk menggeser, slider untuk memperbesar, pilih cara mengisi
/// bingkai)**.
///
/// Kenapa bidikan ini ada: galeri menampilkan foto pada bingkai dengan rasio
/// tetap. Foto yang panjang ke atas atau lebar ke samping otomatis terpotong
/// dari tengah, sehingga bagian pentingnya hilang — wajah di tepi atas, kegiatan
/// di sisi kanan. Untuk foto yang sangat tidak sebentuk bingkai, memindahkan
/// titik fokus saja tidak menolong: apa pun bidikannya tetap ada yang terbuang.
/// Karena itu ada mode **"Foto Utuh"**, yang menampilkan foto UTUH dan mengisi
/// ruang sisanya dengan versi buram foto itu sendiri.
///
/// Yang disimpan BUKAN hasil potongan, melainkan cara memandangnya (migrasi 54 &
/// 55). Bedanya penting: foto yang sama tampil di dua rasio berbeda — 3:4 di
/// halaman depan, 1:1 di grid pengelola — dan satu hasil potongan mustahil pas
/// di keduanya. Berkas aslinya juga tetap utuh, jadi bidikannya bisa diubah lagi
/// kapan saja.
///
/// **Video tidak menawarkan bidikan.** Menyeret bingkai butuh ukuran asli
/// medianya, dan pada video ukuran itu baru diketahui setelah metadata terunduh
/// — pengelola akan menyeret bingkai yang belum bisa dihitung batas geserannya
/// lalu heran mengapa videonya tak bergeming. Video kepala halaman ditampilkan
/// memenuhi lebar layar, di mana bidikan tengah memang yang diinginkan.
#[component]
fn MediaEditor(
    /// Sumber media: URL objek (berkas baru) atau URL publik (media tersimpan).
    #[prop(into)]
    src: String,
    is_video: bool,
    focus_x: f32,
    focus_y: f32,
    zoom: f32,
    #[prop(into)] fit: String,
    #[prop(into)] caption: String,
    category: MediaCategory,
    /// Teks tombol utama — "Unggah" untuk berkas baru, "Simpan" untuk suntingan.
    #[prop(into)]
    commit_label: String,
    /// Dipakai bersama halaman supaya tombol nonaktif selama proses berjalan.
    busy: RwSignal<bool>,
    /// (ke-berapa, dari berapa) saat mengunggah beberapa berkas sekaligus.
    #[prop(optional)]
    progress: Option<(usize, usize)>,
    /// Dipanggil saat tombol utama ditekan.
    on_commit: impl Fn(MediaDraft) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let fx = RwSignal::new(focus_x);
    let fy = RwSignal::new(focus_y);
    let zoom = RwSignal::new(zoom);
    let fit = RwSignal::new(PhotoFit::from_str(&fit));
    let caption = RwSignal::new(caption);
    let category = RwSignal::new(category);
    let dragging = RwSignal::new(false);

    let frame: NodeRef<leptos::html::Div> = NodeRef::new();
    let img: NodeRef<leptos::html::Img> = NodeRef::new();
    // Posisi penunjuk pada gerakan sebelumnya (koordinat layar).
    let last = StoredValue::new((0.0f64, 0.0f64));

    // Seberapa jauh foto MASIH bisa digeser, dalam piksel, pada sumbu x dan y.
    //
    // Ini yang membuat foto mengikuti jari secara pas alih-alih meleset. Dua
    // sumber luapan dijumlahkan: (a) luapan dari cara foto mengisi bingkai —
    // sisi panjang yang melewati bingkai, dan (b) luapan dari perbesaran, yaitu
    // seluruh bingkai dikali (zoom − 1).
    //
    // Pada mode "foto utuh" dengan zoom 1, keduanya nol: foto sudah utuh di
    // dalam bingkai, tak ada yang tersembunyi, jadi menyeret memang tak
    // seharusnya melakukan apa pun.
    #[cfg(target_arch = "wasm32")]
    let pannable = move || -> (f64, f64) {
        let (Some(fr), Some(im)) = (frame.get_untracked(), img.get_untracked()) else {
            return (0.0, 0.0);
        };
        let r = fr.get_bounding_client_rect();
        let (fw, fh) = (r.width(), r.height());
        let (nw, nh) = (im.natural_width() as f64, im.natural_height() as f64);
        if fw <= 0.0 || fh <= 0.0 || nw <= 0.0 || nh <= 0.0 {
            return (0.0, 0.0);
        }
        let z = zoom.get_untracked() as f64;
        // `cover` memakai skala TERBESAR (menutup bingkai, sisanya meluap);
        // `contain` memakai skala TERKECIL (muat seluruhnya, tanpa luapan).
        let s = match fit.get_untracked() {
            PhotoFit::Cover => (fw / nw).max(fh / nh),
            PhotoFit::Contain => (fw / nw).min(fh / nh),
        };
        let over_x = (nw * s - fw).max(0.0);
        let over_y = (nh * s - fh).max(0.0);
        ((over_x + fw * (z - 1.0)) * z, (over_y + fh * (z - 1.0)) * z)
    };

    let on_down = move |ev: leptos::ev::PointerEvent| {
        dragging.set(true);
        last.set_value((ev.client_x() as f64, ev.client_y() as f64));
        // Tangkap penunjuk supaya geseran tetap terlacak walau jari/kursor
        // keluar dari bingkai di tengah gerakan.
        #[cfg(target_arch = "wasm32")]
        if let Some(fr) = frame.get_untracked() {
            let _ = fr.set_pointer_capture(ev.pointer_id());
        }
    };

    let on_move = move |ev: leptos::ev::PointerEvent| {
        if !dragging.get_untracked() {
            return;
        }
        // Cegah halaman ikut ter-scroll saat menyeret di layar sentuh.
        ev.prevent_default();
        #[cfg(target_arch = "wasm32")]
        {
            let (px, py) = last.get_value();
            let (cx, cy) = (ev.client_x() as f64, ev.client_y() as f64);
            last.set_value((cx, cy));
            let (pan_x, pan_y) = pannable();
            // Menyeret foto ke KANAN berarti ingin melihat bagian KIRI, jadi
            // titik fokus bergerak berlawanan arah dengan jari.
            if pan_x > 1.0 {
                let d = (cx - px) / pan_x;
                fx.update(|v| *v = (*v - d as f32).clamp(0.0, 1.0));
            }
            if pan_y > 1.0 {
                let d = (cy - py) / pan_y;
                fy.update(|v| *v = (*v - d as f32).clamp(0.0, 1.0));
            }
        }
        let _ = (&last, &fx, &fy);
    };

    let on_up = move |_ev: leptos::ev::PointerEvent| dragging.set(false);

    let commit = move |_| {
        if busy.get_untracked() {
            return;
        }
        on_commit(MediaDraft {
            focus_x: fx.get_untracked(),
            focus_y: fy.get_untracked(),
            zoom: zoom.get_untracked(),
            fit: fit.get_untracked().as_str().to_string(),
            caption: caption.get_untracked().trim().to_string(),
            category: category.get_untracked(),
        });
    };

    let reset = move |_| {
        let (x, y, z) = FOCUS_DEFAULT;
        fx.set(x);
        fy.set(y);
        zoom.set(z);
    };

    // Beralih ke "foto utuh" MENGEMBALIKAN zoom ke 1 dan fokus ke tengah:
    // maksud memilih mode itu adalah melihat foto utuh, dan membawa serta zoom
    // dari mode sebelumnya justru memotongnya lagi — persis yang ingin dihindari.
    let set_fit = move |f: PhotoFit| {
        if fit.get_untracked() == f {
            return;
        }
        fit.set(f);
        if f == PhotoFit::Contain {
            let (x, y, z) = FOCUS_DEFAULT;
            fx.set(x);
            fy.set(y);
            zoom.set(z);
        }
    };

    let preview_style = move || frame_style_of(fx.get(), fy.get(), zoom.get(), fit.get());
    let src_for_backdrop = src.clone();
    let src_for_video = src.clone();

    view! {
        {progress
            .filter(|(_, total)| *total > 1)
            .map(|(nth, total)| {
                view! {
                    <p class="text-body-sm text-primary font-semibold mb-2">
                        {format!("Media {nth} dari {total}")}
                    </p>
                }
            })}

        // ── Keterangan & kategori — berlaku untuk foto MAUPUN video ───────────
        <label class="block text-body-sm font-semibold text-on-background mb-1.5">
            "Keterangan"
        </label>
        <input
            type="text"
            maxlength="160"
            placeholder="Mis. Kajian kitab bakda Subuh"
            class="w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface mb-4"
            prop:value=move || caption.get()
            on:input=move |ev| caption.set(event_target_value(&ev))
        />

        <label class="block text-body-sm font-semibold text-on-background mb-1.5">
            "Tampil di bagian"
        </label>
        <div class="flex gap-1 bg-surface-container rounded-xl p-1 mb-4">
            {MediaCategory::ALL
                .into_iter()
                .map(|c| {
                    view! {
                        <button
                            class=move || {
                                if category.get() == c {
                                    "flex-1 py-2 px-2 rounded-lg bg-surface text-on-background shadow-sm text-body-sm font-semibold cursor-pointer"
                                } else {
                                    "flex-1 py-2 px-2 rounded-lg text-on-surface-variant text-body-sm font-semibold cursor-pointer"
                                }
                            }
                            aria-pressed=move || (category.get() == c).to_string()
                            on:click=move |_| category.set(c)
                        >
                            {c.label()}
                        </button>
                    }
                })
                .collect_view()}
        </div>

        {if is_video {
            // Video: pratinjau apa adanya, dengan kontrol pemutar supaya
            // pengelola bisa memastikan klipnya benar sebelum terbit.
            view! {
                <video
                    src=src_for_video.clone()
                    class="w-full max-w-[20rem] mx-auto rounded-2xl bg-black"
                    controls="controls"
                    muted="muted"
                    prop:muted=true
                    playsinline="playsinline"
                    preload="metadata"
                ></video>
                <p class="text-body-sm text-on-surface-variant mt-3">
                    "Video ditampilkan memenuhi lebar layar di kepala halaman depan, \
                     membisu dan berulang. Pastikan bagian pentingnya ada di tengah."
                </p>
            }
                .into_any()
        } else {
            view! {
                // ── Pilihan cara foto mengisi bingkai ─────────────────────────
                <div class="flex gap-1 bg-surface-container rounded-xl p-1 mb-3">
                    <FitBtn
                        fit=fit
                        value=PhotoFit::Cover
                        label="Isi Penuh"
                        hint="bingkai penuh, tepi terpotong"
                        on_pick=set_fit
                    />
                    <FitBtn
                        fit=fit
                        value=PhotoFit::Contain
                        label="Foto Utuh"
                        hint="seluruh foto terlihat"
                        on_pick=set_fit
                    />
                </div>

                <p class="text-body-sm text-on-surface-variant mb-3">
                    {move || {
                        if fit.get() == PhotoFit::Contain {
                            "Foto ditampilkan utuh tanpa terpotong. Ruang kosong di \
                             sekelilingnya diisi versi buram foto ini. Perbesar jika \
                             ingin memotong tepinya."
                        } else {
                            "Geser foto untuk memilih bagian yang ditampilkan, lalu atur perbesarannya."
                        }
                    }}
                </p>

                <div
                    node_ref=frame
                    class="relative w-full max-w-[16rem] mx-auto aspect-[3/4] rounded-2xl overflow-hidden bg-surface-container select-none"
                    style=move || {
                        let c = if dragging.get() { "grabbing" } else { "grab" };
                        format!("cursor:{c};touch-action:none")
                    }
                    on:pointerdown=on_down
                    on:pointermove=on_move
                    on:pointerup=on_up
                    on:pointercancel=on_up
                >
                    {move || {
                        (fit.get() == PhotoFit::Contain)
                            .then(|| {
                                view! {
                                    <img
                                        src=src_for_backdrop.clone()
                                        style=crate::models::BACKDROP_STYLE
                                        alt=""
                                        aria-hidden="true"
                                    />
                                }
                            })
                    }}
                    <img
                        node_ref=img
                        src=src.clone()
                        style=preview_style
                        alt="Pratinjau foto"
                        draggable="false"
                        class="relative"
                    />
                    // Garis bantu sepertiga — membantu menempatkan subjek, dan sekaligus
                    // memberi tanda bahwa bingkai ini memang bisa diutak-atik.
                    <div class="absolute inset-0 pointer-events-none opacity-40">
                        <div class="absolute left-1/3 top-0 bottom-0 w-px bg-white/70"></div>
                        <div class="absolute left-2/3 top-0 bottom-0 w-px bg-white/70"></div>
                        <div class="absolute top-1/3 left-0 right-0 h-px bg-white/70"></div>
                        <div class="absolute top-2/3 left-0 right-0 h-px bg-white/70"></div>
                    </div>
                </div>

                <div class="mt-4 flex items-center gap-3">
                    <span class="material-symbols-outlined text-on-surface-variant text-[20px]">
                        "zoom_out"
                    </span>
                    <input
                        type="range"
                        min="1"
                        max="3"
                        step="0.01"
                        class="flex-1 accent-primary"
                        prop:value=move || zoom.get().to_string()
                        on:input=move |ev| {
                            if let Ok(v) = event_target_value(&ev).parse::<f32>() {
                                zoom.set(v.clamp(1.0, 3.0));
                            }
                        }
                        aria-label="Perbesaran foto"
                    />
                    <span class="material-symbols-outlined text-on-surface-variant text-[20px]">
                        "zoom_in"
                    </span>
                    <span class="text-body-sm font-semibold text-on-surface-variant w-12 text-right">
                        {move || format!("{:.1}×", zoom.get())}
                    </span>
                </div>
            }
                .into_any()
        }}

        <div class="mt-5 flex gap-2">
            {(!is_video)
                .then(|| {
                    view! {
                        <button
                            class="flex-1 py-3 rounded-xl border-2 border-outline-variant text-on-surface-variant font-semibold press cursor-pointer"
                            on:click=reset
                        >
                            "Atur Ulang"
                        </button>
                    }
                })}
            <button
                class="flex-1 py-3 rounded-xl bg-primary text-on-primary font-semibold press cursor-pointer disabled:opacity-60"
                prop:disabled=move || busy.get()
                on:click=commit
            >
                {move || if busy.get() { "Memproses…".to_string() } else { commit_label.clone() }}
            </button>
        </div>
    }
}

/// Satu tombol pemilih mode isi bingkai.
#[component]
fn FitBtn(
    fit: RwSignal<PhotoFit>,
    value: PhotoFit,
    label: &'static str,
    hint: &'static str,
    on_pick: impl Fn(PhotoFit) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let active = move || fit.get() == value;
    view! {
        <button
            class=move || {
                if active() {
                    "flex-1 py-2 px-2 rounded-lg bg-surface text-on-background shadow-sm cursor-pointer"
                } else {
                    "flex-1 py-2 px-2 rounded-lg text-on-surface-variant cursor-pointer"
                }
            }
            aria-pressed=move || active().to_string()
            on:click=move |_| on_pick(value)
        >
            <span class="block text-body-sm font-semibold">{label}</span>
            <span class="block text-[10px] opacity-70">{hint}</span>
        </button>
    }
}
