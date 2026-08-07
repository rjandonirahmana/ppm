//! web/live_audio_ui.rs — Klien SIARAN SUARA sesi (pasangan web/live_audio.rs).
//!
//! Dipasang di /sesi/:id/live sebagai dok melayang di atas bilah chat, DI LUAR
//! Suspense — polling refetch 4 dtk membangun ulang isi Suspense, sedangkan
//! siaran/putar audio tidak boleh ikut mati. Handle JS (recorder/closure)
//! hidup di thread_local, bukan state komponen, karena alasan yang sama.
//!
//! Guru : mic → MediaRecorder (Opus/WebM) potongan 4 dtk → antre → POST
//!        berurutan ke /chunk?seq=N dgn retry → server append = rekaman.
//! Santri: poll /data?from=offset → append ke MediaSource. Latensi ~4–8 dtk.
//! Pola closure DI-HOLD (bukan .forget()) — pelajaran audit e-ticketing.

use leptos::prelude::*;

#[component]
pub fn AudioDock(
    #[prop(into)] session_id: Signal<i64>,
    #[prop(into)] is_live: Signal<bool>,
    #[prop(into)] can_manage: Signal<bool>,
    /// Kategori kelas mengizinkan rekam suara (HANYA "Pengajian") — kontrol
    /// siaran/dengar disembunyikan total bila false (sholat & kategori lain).
    #[prop(into)] can_record: Signal<bool>,
    #[prop(into)] recording_url: Signal<Option<String>>,
) -> impl IntoView {
    // Sinyal UI (primitif Send) — logika JS ada di mod wasm (thread_local).
    let bc_on = RwSignal::new(false); // guru sedang siaran?
    let bc_sent = RwSignal::new(0u64); // potongan terkirim
    let bc_queued = RwSignal::new(0usize); // potongan antre (jaringan putus)
    let bc_err = RwSignal::new(Option::<String>::None);
    let ls_on = RwSignal::new(false); // santri sedang mendengarkan?
    let ls_status = RwSignal::new(String::new());
    let audio_ref = NodeRef::<leptos::html::Audio>::new();

    let start_bc = move |_| {
        #[cfg(target_arch = "wasm32")]
        wasm::start_broadcast(session_id.get_untracked(), bc_on, bc_sent, bc_queued, bc_err);
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (session_id, bc_on, bc_sent, bc_queued, bc_err);
    };
    let stop_bc = move |_| {
        #[cfg(target_arch = "wasm32")]
        wasm::stop_broadcast(bc_on);
    };
    let start_ls = move |_| {
        #[cfg(target_arch = "wasm32")]
        if let Some(audio) = audio_ref.get_untracked() {
            wasm::start_listen(session_id.get_untracked(), audio, ls_on, ls_status);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (audio_ref, ls_on, ls_status);
    };
    let stop_ls = move |_| {
        #[cfg(target_arch = "wasm32")]
        {
            wasm::stop_listen();
            ls_on.set(false);
            if let Some(audio) = audio_ref.get_untracked() {
                let _ = audio.pause();
            }
        }
    };

    // Sesi berakhir / pindah sesi → hentikan siaran & pemutar otomatis.
    #[cfg(target_arch = "wasm32")]
    {
        Effect::new(move |_| {
            if !is_live.get() {
                if bc_on.get_untracked() {
                    wasm::stop_broadcast(bc_on);
                }
                if ls_on.get_untracked() {
                    wasm::stop_listen();
                    let _ = ls_on.try_set(false);
                }
            }
        });
        Effect::new(move |prev: Option<i64>| {
            let id = session_id.get();
            if prev.is_some_and(|p| p != id) {
                wasm::stop_broadcast(bc_on);
                wasm::stop_listen();
                let _ = ls_on.try_set(false);
            }
            id
        });
        on_cleanup(move || {
            wasm::stop_broadcast(bc_on);
            wasm::stop_listen();
        });
    }

    view! {
        // Elemen audio statis (di luar blok reaktif) → playback tak ikut remount.
        <audio node_ref=audio_ref class="hidden"></audio>
        <div class="fixed bottom-[76px] inset-x-0 max-w-md mx-auto px-4 z-20 pointer-events-none">
            {move || {
                if is_live.get() && can_manage.get() && can_record.get() {
                    Some(
                        view! {
                            <div class="pointer-events-auto ppm-card shadow-lg p-3 space-y-2 anim-in">
                                {move || bc_err.get().map(|e| view! {
                                    <p class="text-[11px] text-error bg-error-container rounded-lg px-3 py-1.5">{e}</p>
                                })}
                                {move || {
                                    if bc_on.get() {
                                        view! {
                                            <div class="flex items-center gap-3">
                                                <span class="w-2 h-2 rounded-full bg-error pulse-dot shrink-0"></span>
                                                <div class="flex-1 min-w-0">
                                                    <p class="text-body-sm font-bold text-on-background">"Siaran suara berlangsung"</p>
                                                    <p class="text-[11px] text-on-surface-variant">
                                                        {move || {
                                                            let q = bc_queued.get();
                                                            let base = format!("{} potongan terkirim", bc_sent.get());
                                                            if q > 0 { format!("{base} · {q} antre") } else { base }
                                                        }}
                                                    </p>
                                                </div>
                                                <button
                                                    class="px-4 py-2 rounded-xl bg-error text-on-error text-body-sm font-bold press flex items-center gap-1.5"
                                                    on:click=stop_bc
                                                >
                                                    <span class="material-symbols-outlined text-[18px]">"mic_off"</span>
                                                    "Berhenti"
                                                </button>
                                            </div>
                                        }
                                            .into_any()
                                    } else {
                                        view! {
                                            <button
                                                class="w-full py-3 rounded-xl bg-primary text-on-primary font-bold text-body-md flex items-center justify-center gap-2 press"
                                                on:click=start_bc
                                            >
                                                <span class="material-symbols-outlined">"mic"</span>
                                                "Mulai Siaran Suara"
                                            </button>
                                        }
                                            .into_any()
                                    }
                                }}
                            </div>
                        }
                            .into_any(),
                    )
                } else if is_live.get() && can_record.get() {
                    Some(
                        view! {
                            <div class="pointer-events-auto ppm-card shadow-lg p-3 anim-in">
                                {move || {
                                    if ls_on.get() {
                                        view! {
                                            <div class="flex items-center gap-3">
                                                <span class="material-symbols-outlined text-primary">"graphic_eq"</span>
                                                <p class="flex-1 min-w-0 text-body-sm text-on-background truncate">
                                                    {move || ls_status.get()}
                                                </p>
                                                <button
                                                    class="px-4 py-2 rounded-xl bg-surface-container-highest text-on-background text-body-sm font-bold press"
                                                    on:click=stop_ls
                                                >
                                                    "Berhenti"
                                                </button>
                                            </div>
                                        }
                                            .into_any()
                                    } else {
                                        view! {
                                            <button
                                                class="w-full py-3 rounded-xl bg-primary text-on-primary font-bold text-body-md flex items-center justify-center gap-2 press"
                                                on:click=start_ls
                                            >
                                                <span class="material-symbols-outlined">"headphones"</span>
                                                "Dengarkan Siaran"
                                            </button>
                                        }
                                            .into_any()
                                    }
                                }}
                            </div>
                        }
                            .into_any(),
                    )
                } else if is_live.get() {
                    // Live tapi kategori kelas tak boleh rekam suara (sholat,
                    // dll.) — dock tak ditampilkan sama sekali.
                    None
                } else {
                    recording_url.get().map(|url| {
                        view! {
                            <a
                                href=url
                                class="pointer-events-auto flex items-center gap-3 ppm-card shadow-lg p-3 press anim-in"
                            >
                                <span class="w-10 h-10 rounded-full bg-secondary-container text-primary flex items-center justify-center">
                                    <span class="material-symbols-outlined">"download"</span>
                                </span>
                                <div class="flex-1">
                                    <p class="text-body-sm font-bold text-on-background">"Unduh Rekaman Sesi"</p>
                                    <p class="text-[11px] text-on-surface-variant">"Audio siaran tersimpan otomatis"</p>
                                </div>
                            </a>
                        }
                            .into_any()
                    })
                }
            }}
        </div>
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Logika browser (WASM saja). Handle JS di thread_local: satu siaran / satu
// pendengar per tab — cukup untuk kasus guru & santri.
// ═══════════════════════════════════════════════════════════════════════════════
#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::rc::Rc;

    use gloo_timers::future::TimeoutFuture;
    use leptos::prelude::*;
    use leptos::task::spawn_local;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{
        Blob, BlobEvent, HtmlAudioElement, MediaRecorder, MediaRecorderOptions, MediaSource,
        MediaSourceReadyState, MediaStream, MediaStreamConstraints, MediaStreamTrack,
    };

    /// Codec siaran: Opus dalam WebM (Chrome/Android; guru pakai laptop/HP Android).
    const MIME: &str = "audio/webm;codecs=opus";
    /// Durasi tiap potongan MediaRecorder.
    const CHUNK_MS: i32 = 4_000;

    pub struct BroadcastHandle {
        recorder: MediaRecorder,
        stream: MediaStream,
        alive: Rc<Cell<bool>>,
        _on_data: Closure<dyn FnMut(BlobEvent)>,
    }

    thread_local! {
        static BROADCAST: RefCell<Option<BroadcastHandle>> = const { RefCell::new(None) };
        static LISTENER: RefCell<Option<Rc<Cell<bool>>>> = const { RefCell::new(None) };
    }

    // ── Guru: siaran ─────────────────────────────────────────────────────────

    pub fn start_broadcast(
        session_id: i64,
        on: RwSignal<bool>,
        sent: RwSignal<u64>,
        queued: RwSignal<usize>,
        err: RwSignal<Option<String>>,
    ) {
        if BROADCAST.with(|b| b.borrow().is_some()) {
            return;
        }
        spawn_local(async move {
            let _ = err.try_set(None);
            let _ = sent.try_set(0);
            if let Err(e) = init_broadcast(session_id, on, sent, queued, err).await {
                let _ = err.try_set(Some(e));
            }
        });
    }

    async fn init_broadcast(
        session_id: i64,
        on: RwSignal<bool>,
        sent: RwSignal<u64>,
        queued: RwSignal<usize>,
        err: RwSignal<Option<String>>,
    ) -> Result<(), String> {
        let media = web_sys::window()
            .ok_or("window tidak tersedia")?
            .navigator()
            .media_devices()
            .map_err(|_| "Peramban tidak mendukung akses mikrofon".to_string())?;
        let constraints = MediaStreamConstraints::new();
        constraints.set_audio(&JsValue::TRUE);
        let stream: MediaStream = JsFuture::from(
            media
                .get_user_media_with_constraints(&constraints)
                .map_err(|_| "Gagal meminta mikrofon".to_string())?,
        )
        .await
        .map_err(|_| "Izin mikrofon ditolak".to_string())?
        .unchecked_into();

        let opts = MediaRecorderOptions::new();
        if MediaRecorder::is_type_supported(MIME) {
            opts.set_mime_type(MIME);
        }
        let recorder = MediaRecorder::new_with_media_stream_and_media_recorder_options(
            &stream, &opts,
        )
        .map_err(|_| "MediaRecorder tidak didukung peramban ini".to_string())?;

        let queue: Rc<RefCell<VecDeque<(u64, Blob)>>> = Rc::new(RefCell::new(VecDeque::new()));
        let next_seq = Rc::new(Cell::new(0u64));
        let alive = Rc::new(Cell::new(true));

        let on_data = {
            let queue = queue.clone();
            let next_seq = next_seq.clone();
            Closure::<dyn FnMut(BlobEvent)>::new(move |ev: BlobEvent| {
                if let Some(blob) = ev.data() {
                    if blob.size() > 0.0 {
                        let seq = next_seq.get();
                        next_seq.set(seq + 1);
                        queue.borrow_mut().push_back((seq, blob));
                        let _ = queued.try_set(queue.borrow().len());
                    }
                }
            })
        };
        recorder.set_ondataavailable(Some(on_data.as_ref().unchecked_ref()));
        recorder
            .start_with_time_slice(CHUNK_MS)
            .map_err(|_| "Gagal memulai perekaman".to_string())?;

        BROADCAST.with(|b| {
            *b.borrow_mut() = Some(BroadcastHandle {
                recorder,
                stream,
                alive: alive.clone(),
                _on_data: on_data,
            })
        });
        let _ = on.try_set(true);
        spawn_local(upload_loop(session_id, queue, alive, sent, queued, err));
        Ok(())
    }

    /// Pengunggah berurutan: satu potongan sesudah yang lain (urutan seq wajib
    /// terjaga — server hanya APPEND). Gagal → kembali ke DEPAN antrean, retry.
    async fn upload_loop(
        session_id: i64,
        queue: Rc<RefCell<VecDeque<(u64, Blob)>>>,
        alive: Rc<Cell<bool>>,
        sent: RwSignal<u64>,
        queued: RwSignal<usize>,
        err: RwSignal<Option<String>>,
    ) {
        let mut fails_after_stop = 0u32;
        loop {
            let item = queue.borrow_mut().pop_front();
            let Some((seq, blob)) = item else {
                if !alive.get() {
                    break; // siaran usai & antrean tuntas → rekaman utuh di server
                }
                TimeoutFuture::new(250).await;
                continue;
            };
            let url = format!("/api/live-audio/{session_id}/chunk?seq={seq}");
            // Status HTTP dibedakan, bukan sekadar ok/tidak. Server kini bisa
            // menolak secara PERMANEN — sesi sudah berakhir (410), bukan hak
            // pengirim (403), atau urutan potongan tak nyambung setelah proses
            // server restart (409). Semuanya percuma diulang; versi lama
            // menganggap semua kegagalan sebagai "jaringan putus" dan mengulang
            // selamanya tiap 2 detik sambil menampilkan pesan yang keliru.
            let hasil = match gloo_net::http::Request::post(&url).body(blob.clone()) {
                Ok(req) => req.send().await.ok().map(|r| r.status()),
                Err(_) => None,
            };
            let ok = matches!(hasil, Some(200..=299));
            if let Some(code @ (403 | 409 | 410)) = hasil {
                let pesan = match code {
                    403 => "Anda bukan petugas siaran sesi ini.",
                    410 => "Sesi sudah ditutup — siaran dihentikan.",
                    _ => "Sambungan siaran terputus di server. Mulai siaran lagi \
                          untuk melanjutkan (rekaman sebelumnya tetap tersimpan).",
                };
                let _ = err.try_set(Some(pesan.into()));
                let _ = queued.try_set(0);
                queue.borrow_mut().clear();
                break;
            }
            if ok {
                let _ = sent.try_update(|n| *n += 1);
                let _ = queued.try_set(queue.borrow().len());
                let _ = err.try_set(None);
            } else {
                queue.borrow_mut().push_front((seq, blob));
                let _ = queued.try_set(queue.borrow().len());
                let _ = err
                    .try_set(Some("Jaringan putus — potongan diantre & dicoba ulang…".into()));
                if !alive.get() {
                    fails_after_stop += 1;
                    if fails_after_stop > 5 {
                        break; // menyerah: jangan retry selamanya setelah berhenti
                    }
                }
                TimeoutFuture::new(2_000).await;
            }
        }
    }

    pub fn stop_broadcast(on: RwSignal<bool>) {
        if let Some(handle) = BROADCAST.with(|b| b.borrow_mut().take()) {
            let _ = handle.recorder.stop(); // memicu dataavailable terakhir (async)
            // Matikan mic SEGERA (indikator rekam browser padam).
            for track in handle.stream.get_tracks().iter() {
                if let Ok(track) = track.dyn_into::<MediaStreamTrack>() {
                    track.stop();
                }
            }
            // Closure ondataavailable harus tetap hidup sampai event terakhir
            // tiba; baru kemudian uploader disuruh selesai & handle di-drop.
            spawn_local(async move {
                TimeoutFuture::new(2_000).await;
                handle.alive.set(false);
                drop(handle);
            });
        }
        let _ = on.try_set(false);
    }

    // ── Santri: mendengarkan ─────────────────────────────────────────────────

    pub fn start_listen(
        session_id: i64,
        audio: HtmlAudioElement,
        on: RwSignal<bool>,
        status: RwSignal<String>,
    ) {
        stop_listen();
        let Ok(source) = MediaSource::new() else {
            let _ = status.try_set(
                "Peramban tidak mendukung siaran langsung (gunakan Chrome/Android)".into(),
            );
            return;
        };
        let Ok(url) = web_sys::Url::create_object_url_with_source(&source) else {
            let _ = status.try_set("Gagal menyiapkan pemutar".into());
            return;
        };
        // src + play() SINKRON di dalam gesture klik → lolos kebijakan autoplay.
        audio.set_src(&url);
        let _ = audio.play();

        let alive = Rc::new(Cell::new(true));
        LISTENER.with(|l| *l.borrow_mut() = Some(alive.clone()));
        let _ = on.try_set(true);
        let _ = status.try_set("Menyambungkan…".into());
        spawn_local(async move {
            if let Err(e) = listen_loop(session_id, audio, source, alive, status).await {
                let _ = status.try_set(e);
            }
        });
    }

    pub fn stop_listen() {
        LISTENER.with(|l| {
            if let Some(alive) = l.borrow_mut().take() {
                alive.set(false);
            }
        });
    }

    async fn listen_loop(
        session_id: i64,
        audio: HtmlAudioElement,
        source: MediaSource,
        alive: Rc<Cell<bool>>,
        status: RwSignal<String>,
    ) -> Result<(), String> {
        // Tunggu sourceopen (poll ready_state — tanpa closure ekstra).
        let mut waited = 0u32;
        while source.ready_state() != MediaSourceReadyState::Open {
            if waited > 5_000 {
                return Err("Pemutar tidak siap — coba lagi".into());
            }
            TimeoutFuture::new(50).await;
            waited += 50;
        }
        let sb = source
            .add_source_buffer(MIME)
            .map_err(|_| "Codec audio tidak didukung peramban ini".to_string())?;

        let mut from: u64 = 0;
        let mut playing = false;
        loop {
            if !alive.get() {
                break;
            }
            match fetch_chunk(session_id, from).await {
                Fetched::Data { mut bytes, next } => {
                    if next < from {
                        // Server truncate (guru memulai siaran BARU) → sesi putar usang.
                        return Err("Siaran dimulai ulang — ketuk Dengarkan lagi".into());
                    }
                    if bytes.is_empty() {
                        TimeoutFuture::new(2_000).await;
                        continue;
                    }
                    let got = bytes.len();
                    while sb.updating() {
                        TimeoutFuture::new(40).await;
                    }
                    if source.ready_state() != MediaSourceReadyState::Open {
                        break;
                    }
                    if sb.append_buffer_with_u8_array(&mut bytes).is_err() {
                        return Err("Gagal memutar audio siaran".into());
                    }
                    from = next;
                    if !playing {
                        playing = true;
                        let _ = audio.play();
                        let _ = status.try_set("LIVE — sedang mendengarkan".into());
                    }
                    // Jaga tetap dekat ujung LIVE: bila tertinggal jauh
                    // (habis putus jaringan), lompat mendekati ujung buffer.
                    let buffered = audio.buffered();
                    if buffered.length() > 0 {
                        if let Ok(end) = buffered.end(buffered.length() - 1) {
                            if end - audio.current_time() > 25.0 {
                                audio.set_current_time(end - 5.0);
                            }
                        }
                    }
                    // Potongan penuh (1MB) = sedang mengejar → langsung lanjut.
                    TimeoutFuture::new(if got >= 900_000 { 200 } else { 2_500 }).await;
                }
                Fetched::NotFound => {
                    if !playing {
                        let _ = status.try_set("Menunggu guru memulai siaran…".into());
                    }
                    TimeoutFuture::new(3_000).await;
                }
                Fetched::Error => {
                    let _ = status.try_set("Jaringan putus — mencoba lagi…".into());
                    TimeoutFuture::new(3_000).await;
                }
            }
        }
        Ok(())
    }

    enum Fetched {
        Data { bytes: Vec<u8>, next: u64 },
        NotFound,
        Error,
    }

    async fn fetch_chunk(session_id: i64, from: u64) -> Fetched {
        let url = format!("/api/live-audio/{session_id}/data?from={from}");
        let Ok(resp) = gloo_net::http::Request::get(&url).send().await else {
            return Fetched::Error;
        };
        match resp.status() {
            200 => {
                let next = resp
                    .headers()
                    .get("x-next")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(from);
                match resp.binary().await {
                    Ok(bytes) => Fetched::Data { bytes, next },
                    Err(_) => Fetched::Error,
                }
            }
            404 => Fetched::NotFound,
            _ => Fetched::Error,
        }
    }
}
