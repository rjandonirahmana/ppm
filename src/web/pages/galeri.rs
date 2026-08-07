//! web/pages/galeri.rs — Galeri "Foto Kegiatan" (migrasi 34). Kelola foto yang
//! tampil di beranda publik: unggah banyak sekaligus, geser (drag-and-drop asli)
//! untuk mengubah urutan, dan hapus. Hanya admin/dewan_guru yang bisa mengelola.
//!
//! ALUR UNGGAH: pilih berkas → **atur bidikan dulu** → baru terkirim. Bidikan
//! (titik fokus, perbesaran, mode isi — migrasi 54 & 55) ikut di request unggah
//! yang sama, jadi foto tak pernah sempat tampil dengan bidikan yang bukan
//! pilihan siapa pun. Berkas diunggah satu per satu lewat
//! POST /api/activity-photos/upload (multipart, di luar server-fn).
//! Urutan disimpan via `reorder_activity_photos_action`.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{frame_style_of, ActivityPhoto, PhotoFit, SessionUser, FOCUS_DEFAULT};
use crate::web::api::{
    activity_photos_data, delete_activity_photo_action, reorder_activity_photos_action,
    set_activity_photo_focus_action,
};
use crate::web::components::{
    DeviceFrame, EmptyState, FetchError, MobileHeader, PhotoFrame, Sheet,
};

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
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    // ── Antrean unggah: berkas yang SUDAH dipilih tapi BELUM dikirim ──────────
    //
    // Berkasnya sendiri tetap tinggal di elemen <input> dan diambil ulang lewat
    // indeks saat dibutuhkan. Itu sengaja: `web_sys::File` bukan tipe yang bisa
    // disimpan di signal Leptos (tak `Send`/`Sync`), dan membungkusnya hanya
    // untuk itu berarti kode khusus-wasm merembes ke seluruh komponen. Yang
    // disimpan di sini cukup angka dan satu URL pratinjau — keduanya biasa saja
    // di kedua target build.
    //
    // `<input>` baru dikosongkan setelah SELURUH antrean selesai; mengosongkannya
    // lebih awal akan membuang berkas yang belum sempat diunggah.
    let pending_total = RwSignal::new(0usize);
    let pending_idx = RwSignal::new(0usize);
    // URL objek (blob:) pratinjau berkas yang sedang diatur.
    let pending_src = RwSignal::new(String::new());

    // Foto TERSIMPAN yang sedang disunting ulang lewat tombol "Atur".
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
            match web_sys::Url::create_object_url_with_blob(&file) {
                Ok(u) => {
                    pending_idx.set(idx);
                    pending_src.set(u);
                    return true;
                }
                Err(_) => return false,
            }
        }
        #[allow(unreachable_code)]
        {
            let _ = (idx, &file_input, &pending_idx);
            false
        }
    };

    // Antrean unggah selesai / dibatalkan → bersihkan semuanya.
    let finish_batch = move || {
        revoke_preview();
        pending_total.set(0);
        pending_idx.set(0);
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

    // Kirim berkas yang sedang diatur, LENGKAP dengan bidikannya, lalu lanjut ke
    // berkas berikutnya dalam antrean.
    let upload_current = move |fx: f32, fy: f32, z: f32, fit: String| {
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
                let _ = form.append_with_str("focus_x", &fx.to_string());
                let _ = form.append_with_str("focus_y", &fy.to_string());
                let _ = form.append_with_str("zoom", &z.to_string());
                let _ = form.append_with_str("fit", &fit);

                let window = web_sys::window().unwrap();
                let opts = web_sys::RequestInit::new();
                opts.set_method("POST");
                opts.set_body(form.as_ref());
                let mut ok = false;
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
                                    // merapikan bidikan ke rentang sahnya, jadi
                                    // memakai nilai kiriman sendiri bisa membuat
                                    // grid berbeda dari yang tersimpan.
                                    items.update(|v| {
                                        let ord = v.len() as i32;
                                        v.push(ActivityPhoto {
                                            id,
                                            url,
                                            caption: String::new(),
                                            sort_order: ord,
                                            focus_x: get_num("focus_x").unwrap_or(0.5) as f32,
                                            focus_y: get_num("focus_y").unwrap_or(0.5) as f32,
                                            zoom: get_num("zoom").unwrap_or(1.0) as f32,
                                            fit: get_str("fit")
                                                .unwrap_or_else(|| "cover".into()),
                                        });
                                    });
                                    ok = true;
                                }
                            }
                        }
                    }
                }
                busy.set(false);
                if !ok {
                    msg.set(Some((
                        false,
                        "Unggah gagal — periksa berkas/koneksi, lalu coba lagi.".into(),
                    )));
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
                msg.set(Some((true, format!("{total} foto terunggah."))));
            });
        }
        // Di build SSR seluruh blok di atas tak ada, jadi parameternya menganggur.
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (fx, fy, z, fit, &busy, &msg);
    };

    // Simpan bidikan foto yang SUDAH tersimpan (tombol "Atur" di kartu).
    let save_existing = move |id: i64, fx: f32, fy: f32, z: f32, fit: String| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        let fit2 = fit.clone();
        leptos::task::spawn_local(async move {
            let ok = set_activity_photo_focus_action(id, fx, fy, z, fit2.clone())
                .await
                .is_ok();
            busy.set(false);
            if ok {
                items.update(|v| {
                    if let Some(p) = v.iter_mut().find(|p| p.id == id) {
                        p.focus_x = fx;
                        p.focus_y = fy;
                        p.zoom = z;
                        p.fit = fit2;
                    }
                });
                editing.set(None);
            } else {
                msg.set(Some((false, "Gagal menyimpan bidikan — coba lagi.".into())));
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

    view! {
        <Title text="Galeri Foto Kegiatan — AFM SMART" />
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
                                <span class="material-symbols-outlined text-primary">
                                    "cloud_upload"
                                </span>
                                "Unggah Foto"
                            </h3>
                            <p class="text-body-sm text-on-surface-variant">
                                "Pilih foto (jpg/png/webp, maks 10MB). Setelah dipilih, atur \
                                 dulu bidikannya — baru terkirim. Seret foto di bawah untuk \
                                 mengubah urutan tampil."
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
                                                                        let photo = p.clone();
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
                                                                                // bukan cuma di beranda — supaya yang dilihat
                                                                                // pengelola sama dengan yang dilihat pengunjung.
                                                                                <PhotoFrame
                                                                                    src=p.url.clone()
                                                                                    style=p.frame_style()
                                                                                    backdrop=p.fit().needs_backdrop()
                                                                                    alt="Foto kegiatan"
                                                                                    class="aspect-square"
                                                                                    lazy=true
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
                                                                                        <button
                                                                                            class="absolute bottom-1.5 right-1.5 h-7 px-2 rounded-lg bg-black/55 text-white flex items-center gap-1"
                                                                                            on:click=move |_| editing.set(Some(photo.clone()))
                                                                                            aria-label="Atur bidikan foto"
                                                                                        >
                                                                                            <span class="material-symbols-outlined text-[16px]">"crop"</span>
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

                // ── Editor bidikan: berkas BARU (sebelum diunggah) ─────────────
                {move || {
                    (pending_total.get() > 0 && !pending_src.get().is_empty())
                        .then(|| {
                            let (fx, fy, z) = FOCUS_DEFAULT;
                            let total = pending_total.get();
                            let nth = pending_idx.get() + 1;
                            view! {
                                <Sheet title="Sesuaikan Foto Sebelum Diunggah" on_close=finish_batch>
                                    <PhotoFocusEditor
                                        src=pending_src.get()
                                        focus_x=fx
                                        focus_y=fy
                                        zoom=z
                                        fit="cover"
                                        commit_label="Unggah"
                                        busy=busy
                                        progress=(nth, total)
                                        on_commit=move |fx, fy, z, fit| upload_current(fx, fy, z, fit)
                                    />
                                </Sheet>
                            }
                        })
                }}

                // ── Editor bidikan: foto yang SUDAH tersimpan ──────────────────
                {move || {
                    editing
                        .get()
                        .map(|p| {
                            let id = p.id;
                            view! {
                                <Sheet
                                    title="Sesuaikan Tampilan Foto"
                                    on_close=move || editing.set(None)
                                >
                                    <PhotoFocusEditor
                                        src=p.url.clone()
                                        focus_x=p.focus_x
                                        focus_y=p.focus_y
                                        zoom=p.zoom
                                        fit=p.fit.clone()
                                        commit_label="Simpan"
                                        busy=busy
                                        on_commit=move |fx, fy, z, fit| {
                                            save_existing(id, fx, fy, z, fit)
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

/// Editor bidikan satu foto: **seret untuk menggeser, slider untuk memperbesar,
/// dan pilih cara foto mengisi bingkai**.
///
/// Kenapa ini ada: galeri menampilkan foto pada bingkai dengan rasio tetap. Foto
/// yang panjang ke atas atau lebar ke samping otomatis terpotong dari tengah,
/// sehingga bagian pentingnya hilang — wajah di tepi atas, kegiatan di sisi
/// kanan. Untuk foto yang sangat tidak sebentuk bingkai, memindahkan titik fokus
/// saja tidak menolong: apa pun bidikannya tetap ada yang terbuang. Karena itu
/// ada mode **"Muat seluruhnya"**, yang menampilkan foto UTUH dan mengisi ruang
/// sisanya dengan versi buram foto itu sendiri.
///
/// Yang disimpan BUKAN hasil potongan, melainkan cara memandangnya (migrasi 54 &
/// 55). Bedanya penting: foto yang sama tampil di dua rasio berbeda — 3:4 di
/// beranda publik, 1:1 di grid pengelola — dan satu hasil potongan mustahil pas
/// di keduanya. Berkas aslinya juga tetap utuh, jadi bidikannya bisa diubah lagi
/// kapan saja.
///
/// Pratinjaunya memakai rasio 3:4 mengikuti beranda publik, karena itulah
/// tampilan yang dilihat orang luar.
#[component]
fn PhotoFocusEditor(
    /// Sumber gambar: URL objek (berkas baru) atau URL publik (foto tersimpan).
    #[prop(into)]
    src: String,
    focus_x: f32,
    focus_y: f32,
    zoom: f32,
    #[prop(into)] fit: String,
    /// Teks tombol utama — "Unggah" untuk berkas baru, "Simpan" untuk suntingan.
    #[prop(into)]
    commit_label: String,
    /// Dipakai bersama halaman supaya tombol nonaktif selama proses berjalan.
    busy: RwSignal<bool>,
    /// (ke-berapa, dari berapa) saat mengunggah beberapa berkas sekaligus.
    #[prop(optional)]
    progress: Option<(usize, usize)>,
    /// Dipanggil saat tombol utama ditekan: (focus_x, focus_y, zoom, fit).
    on_commit: impl Fn(f32, f32, f32, String) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let fx = RwSignal::new(focus_x);
    let fy = RwSignal::new(focus_y);
    let zoom = RwSignal::new(zoom);
    let fit = RwSignal::new(PhotoFit::from_str(&fit));
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
    // Pada mode "muat seluruhnya" dengan zoom 1, keduanya nol: foto sudah utuh
    // di dalam bingkai, tak ada yang tersembunyi, jadi menyeret memang tak
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
        on_commit(
            fx.get_untracked(),
            fy.get_untracked(),
            zoom.get_untracked(),
            fit.get_untracked().as_str().to_string(),
        );
    };

    let reset = move |_| {
        let (x, y, z) = FOCUS_DEFAULT;
        fx.set(x);
        fy.set(y);
        zoom.set(z);
    };

    // Beralih ke "muat seluruhnya" MENGEMBALIKAN zoom ke 1 dan fokus ke tengah:
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

    view! {
        {progress
            .filter(|(_, total)| *total > 1)
            .map(|(nth, total)| {
                view! {
                    <p class="text-body-sm text-primary font-semibold mb-2">
                        {format!("Foto {nth} dari {total}")}
                    </p>
                }
            })}

        // ── Pilihan cara foto mengisi bingkai ─────────────────────────────────
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
                src=src
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

        <div class="mt-5 flex gap-2">
            <button
                class="flex-1 py-3 rounded-xl border-2 border-outline-variant text-on-surface-variant font-semibold press"
                on:click=reset
            >
                "Atur Ulang"
            </button>
            <button
                class="flex-1 py-3 rounded-xl bg-primary text-on-primary font-semibold press disabled:opacity-60"
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
                    "flex-1 py-2 px-2 rounded-lg bg-surface text-on-background shadow-sm"
                } else {
                    "flex-1 py-2 px-2 rounded-lg text-on-surface-variant"
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
