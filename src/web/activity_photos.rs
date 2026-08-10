//! web/activity_photos.rs — Upload media galeri (migrasi 34 & 69).
//! Handler axum murni (multipart, di luar server-fn — sama alasan materials.rs).
//! Auth cookie manual; hanya admin/dewan_guru. Balas JSON `{ "id", "url", … }`.
//!
//! Sejak migrasi 69 rute ini menerima FOTO maupun VIDEO: video utama halaman
//! depan dikelola di galeri yang sama, jadi memisahkannya ke rute sendiri hanya
//! menggandakan seluruh alur auth-simpan-catat untuk perbedaan satu MIME.

use std::sync::Arc;

use axum::extract::Multipart;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde_json::json;

use crate::state::AppState;

/// Peran yang boleh mengelola galeri — SATU sumber bersama server fn galeri
/// (`web/api.rs::GALLERY_MANAGE_ROLES`). Dipisah sebelumnya, dan daftarnya
/// sempat berbeda dari yang dipakai halaman: ketua melihat tombol unggah tapi
/// selalu ditolak 403.
fn is_manager(role: &str) -> bool {
    crate::web::api::GALLERY_MANAGE_ROLES.contains(&role)
}

/// Baca satu angka bidikan dari field multipart; apa pun yang tak terbaca
/// jatuh ke nilai bawaan alih-alih menggagalkan seluruh unggahan — foto yang
/// sudah terkirim jauh lebih berharga daripada satu angka posisi.
fn parse_f32(field: Result<String, axum::extract::multipart::MultipartError>, dflt: f32) -> f32 {
    field
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .unwrap_or(dflt)
}

/// Ekstensi/mime yang diterima galeri — gambar DAN video (migrasi 69).
///
/// `.mov` ikut diterima: itu format bawaan kamera iPhone, dan menolaknya berarti
/// pengelola harus mengonversi dulu setiap klip sebelum bisa mengunggah.
fn classify_media(filename: &str) -> Option<&'static str> {
    let ext = filename.rsplit('.').next()?.to_lowercase();
    Some(match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        _ => return None,
    })
}

/// Kenapa sebuah berkas ditolak — dalam kalimat yang bisa ditindaklanjuti.
///
/// HEIC diberi penjelasan sendiri karena itu format bawaan iPhone dan penyebab
/// penolakan yang paling sering: pesan "jenis file tidak didukung" saja membuat
/// pengelola menyimpulkan unggahannya rusak, padahal cukup mengubah setelan
/// kamera sekali.
fn alasan_ditolak(filename: &str) -> String {
    let ext = filename.rsplit('.').next().unwrap_or_default().to_lowercase();
    match ext.as_str() {
        "heic" | "heif" => "Foto HEIC (bawaan iPhone) belum didukung peramban. Ubah di HP: \
             Pengaturan → Kamera → Format → \"Paling Kompatibel\", lalu foto ulang; \
             atau bagikan fotonya sebagai JPG lebih dulu."
            .to_string(),
        "" => "Berkas tanpa ekstensi tidak bisa dikenali — gunakan jpg/png/webp/gif \
             atau mp4/mov/webm."
            .to_string(),
        lain => format!(
            "Jenis berkas \".{lain}\" tidak didukung galeri. Gunakan jpg, png, webp, gif \
             (foto) atau mp4, mov, webm (video)."
        ),
    }
}

/// POST /api/activity-photos/upload
///
/// multipart: `file` (wajib), `caption`, dan bidikan dari editor —
/// `focus_x`, `focus_y`, `zoom`, `fit`. Bidikan ikut di request yang SAMA karena
/// pengelola sudah mengaturnya sebelum menekan unggah: menyimpannya belakangan
/// lewat request kedua berarti ada jeda di mana foto tampil dengan bidikan
/// tengah yang mungkin terpotong buruk, dan jika request kedua gagal, foto
/// tertinggal dengan bidikan yang bukan pilihan siapa pun.
pub async fn upload(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    mut form: Multipart,
) -> Response {
    let claims = match crate::web::live_audio::auth(&state, &headers) {
        Ok(c) => c,
        Err(s) => return s.into_response(),
    };
    if !is_manager(&claims.role) {
        // Dengan badan pesan, bukan 403 kosong: klien menampilkan teks balasan
        // apa adanya, dan 403 tanpa isi muncul di layar sebagai "Unggah gagal —
        // periksa berkas/koneksi" yang menuduh hal yang salah.
        return (
            StatusCode::FORBIDDEN,
            "Peran Anda tidak berhak mengelola galeri. Hubungi admin.",
        )
            .into_response();
    }

    let Some(storage) = state.storage.clone() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Penyimpanan file (RustFS) belum dikonfigurasi di server.",
        )
            .into_response();
    };

    let mut caption = String::new();
    // Berkasnya ditulis ke DISK sambil diterima, bukan ditumpuk di RAM: rute ini
    // menerima video sampai 100 MB, dan beberapa unggahan bersamaan cukup untuk
    // menghabiskan memori VPS. Lihat `web::multipart`.
    let mut berkas: Option<crate::web::multipart::BerkasSementara> = None;
    let mut filename = String::new();
    let (mut focus_x, mut focus_y, mut zoom) = crate::models::FOCUS_DEFAULT;
    let mut fit = crate::models::PhotoFit::Cover;
    let mut category = crate::models::MediaCategory::Kegiatan;

    loop {
        let field = match form.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
        };
        match field.name().unwrap_or_default() {
            "caption" => caption = field.text().await.unwrap_or_default(),
            // Bidikan opsional: klien lama / request tanpa field ini tetap
            // diterima dan memakai bawaan (tengah, tanpa perbesaran, cover).
            "focus_x" => focus_x = parse_f32(field.text().await, focus_x),
            "focus_y" => focus_y = parse_f32(field.text().await, focus_y),
            "zoom" => zoom = parse_f32(field.text().await, zoom),
            "fit" => {
                let raw = field.text().await.unwrap_or_default();
                fit = crate::models::PhotoFit::from_str(raw.trim());
            }
            // Kategori juga opsional: klien lama mengunggah foto kegiatan, dan
            // itulah nilai bawaannya.
            "category" => {
                let raw = field.text().await.unwrap_or_default();
                category = crate::models::MediaCategory::from_str(raw.trim());
            }
            "file" => {
                filename = field.file_name().unwrap_or_default().to_string();
                // Batas yang dipasang di sini adalah batas KERAS rute (video);
                // batas gambar yang lebih ketat baru bisa ditentukan setelah
                // jenis isinya diketahui, jadi diperiksa di bawah.
                match crate::web::multipart::terima_berkas(
                    field,
                    crate::web::limits::VIDEO_MAX,
                    "video maks 100MB",
                )
                .await
                {
                    Ok(b) => berkas = Some(b),
                    Err(resp) => return resp,
                }
            }
            _ => {}
        }
    }

    let Some(berkas) = berkas else {
        return (StatusCode::BAD_REQUEST, "File wajib diunggah.").into_response();
    };
    let Some(content_type) = classify_media(&filename) else {
        return (StatusCode::BAD_REQUEST, alasan_ditolak(&filename)).into_response();
    };
    let kind = crate::models::MediaKind::of_mime(content_type);
    // Batas ISI dibedakan per jenis meski rute-nya satu: layer body di router
    // harus melonggar sampai ukuran video, dan tanpa pemeriksaan ini foto pun
    // ikut boleh 100 MB. Berkas yang melewati batas gambar sudah terlanjur
    // ditulis ke disk — tapi ke DISK, dan berkasnya terhapus begitu `berkas`
    // habis masa pakainya di akhir fungsi ini.
    let (max, batas_pesan) = match kind {
        crate::models::MediaKind::Video => (crate::web::limits::VIDEO_MAX, "video maks 100MB"),
        crate::models::MediaKind::Image => (crate::web::limits::IMAGE_MAX, "gambar maks 10MB"),
    };
    if berkas.ukuran == 0 || berkas.ukuran > max {
        return (
            StatusCode::BAD_REQUEST,
            format!("Ukuran file tidak valid ({batas_pesan})."),
        )
            .into_response();
    }
    // Ekstensi cuma nama yang dipilih pengunggah; isinya yang menentukan. Tanpa
    // cek ini, berkas apa pun bisa dinamai `.png` lalu tersimpan berlabel
    // `image/png` dan disajikan kembali dengan label itu.
    if !berkas.cocok(content_type) {
        // Kasus tersering bukan berkas palsu, melainkan foto HEIC iPhone yang
        // namanya berakhiran .jpg — jadi isinya disebutkan bila terdeteksi.
        let isi = berkas.tipe_isi();
        return (
            StatusCode::BAD_REQUEST,
            match isi {
                Some("image/heic") => "Berkas ini sebenarnya foto HEIC (bawaan iPhone) meski \
                     bernama lain. Ubah di HP: Pengaturan → Kamera → Format → \"Paling \
                     Kompatibel\", atau bagikan sebagai JPG lebih dulu."
                    .to_string(),
                Some(nyata) => format!(
                    "Isi berkas ({nyata}) tidak cocok dengan ekstensi namanya. Ganti nama \
                     berkasnya sesuai isi, lalu unggah ulang."
                ),
                None => "Isi berkas tidak dikenali sebagai gambar atau video — pastikan \
                     berkasnya tidak rusak."
                    .to_string(),
            },
        )
            .into_response();
    }

    let ext = crate::web::filetype::ext_for(content_type);
    // Prefix `ppm/` dibuang: bucket sudah "ppm" (dulu jadi `/ppm/ppm/activity/...`).
    // Tanpa dep uuid di ppm: nanos + id pengunggah cukup unik untuk galeri.
    let key = format!(
        "activity/{}-{}.{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        claims.user_id,
        ext
    );

    // Streaming dari disk (`upload_file`), bukan dari RAM (`upload_bytes`).
    let url = match storage.upload_file(berkas.path(), &key, content_type).await {
        Ok(u) => u,
        Err(e) => {
            crate::service::telegram::report_error(502, "Activity photo upload", e.to_string());
            return (StatusCode::BAD_GATEWAY, format!("Upload gagal: {e}")).into_response();
        }
    };

    let caption = caption.trim();
    // Dirapikan di SERVER: nilai dari klien tak pernah dipercaya, dan CHECK di
    // tabel adalah jaring pengaman terakhir — bukan tempat memvalidasi masukan.
    let (fx, fy, z) = crate::models::clamp_focus(focus_x, focus_y, zoom);
    let framing = crate::repository::PhotoFraming {
        focus_x: fx,
        focus_y: fy,
        zoom: z,
        fit: fit.as_str(),
    };
    match crate::repository::insert_activity_photo(
        &state.pool,
        &url,
        caption,
        claims.user_id,
        framing,
        category.as_str(),
        kind.as_str(),
    )
    .await
    {
        Ok(id) => (
            StatusCode::OK,
            Json(json!({
                "id": id,
                "url": url,
                "focus_x": fx,
                "focus_y": fy,
                "zoom": z,
                "fit": fit.as_str(),
                "caption": caption,
                "category": category.as_str(),
                "media_type": kind.as_str(),
            })),
        )
            .into_response(),
        Err(e) => {
            crate::service::telegram::report_error(500, "Activity photo insert", e.to_string());
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}
