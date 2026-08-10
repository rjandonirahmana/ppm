//! web/multipart.rs — Menerima berkas unggahan TANPA memuatnya penuh ke RAM.
//!
//! Sebelumnya setiap handler unggah memanggil `field.bytes().await` lalu
//! `.to_vec()`. Dua langkah itu menaruh SELURUH berkas di memori server, dua
//! kali: `Bytes` hasil pembacaan, lalu salinannya sebagai `Vec<u8>` yang
//! dipegang sampai PUT ke RustFS selesai. Untuk rute bergambar 10 MB itu masih
//! wajar; untuk rute materi & video yang batasnya 100 MB (lihat [`limits`])
//! artinya ±200 MB per request — dua sampai empat unggahan bersamaan sudah
//! cukup membuat OOM-killer Linux membunuh proses di VPS 512 MB–1 GB, tanpa
//! pesan galat apa pun yang bisa dilihat pengguna maupun pengurus.
//!
//! Di sini potongan multipart ditulis langsung ke berkas sementara di disk
//! sambil dihitung ukurannya, lalu diunggah dari disk lewat
//! [`StorageService::upload_file`] yang memang sudah streaming (jalur yang
//! dipakai rekaman siaran sejak awal). Pemakaian memorinya jadi tetap —
//! sebesar satu potongan — berapa pun besar berkasnya.
//!
//! [`limits`]: crate::web::limits
//! [`StorageService::upload_file`]: crate::service::storage::StorageService::upload_file

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use axum::extract::multipart::Field;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tokio::io::AsyncWriteExt;

/// Direktori berkas sementara unggahan (env `UPLOAD_TMP_DIR`, default
/// `<temp OS>/ppm-upload`).
///
/// Bisa disetel karena letaknya menentukan apakah modul ini ada gunanya sama
/// sekali: pada sebagian penataan container `/tmp` dipasang sebagai `tmpfs`,
/// yang berarti RAM — menulis ke sana mengembalikan persis masalah yang
/// dihindari di sini, hanya dengan nama lain. Kalau `/tmp` di server Anda
/// tmpfs, arahkan env ini ke direktori di disk sungguhan.
pub fn upload_tmp_dir() -> PathBuf {
    std::env::var("UPLOAD_TMP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("ppm-upload"))
}

/// Byte pertama yang ditahan di memori untuk mengenali tipe isi.
///
/// [`filetype::sniff`] tak pernah melihat lebih dari 12 byte; 64 memberi ruang
/// bila suatu saat ada format yang tanda tangannya lebih panjang, dan tetap tak
/// berarti apa-apa bagi pemakaian memori.
///
/// [`filetype::sniff`]: crate::web::filetype::sniff
const KEPALA_MAKS: usize = 64;

/// Pembeda nama berkas dalam satu proses. Stempel waktu saja tak cukup: dua
/// unggahan bersamaan bisa jatuh di nanodetik yang sama, dan yang kalah akan
/// menimpa berkas milik request lain.
static URUTAN: AtomicU64 = AtomicU64::new(0);

/// Berkas unggahan yang sudah tersimpan di disk, siap diteruskan ke
/// penyimpanan objek.
///
/// Berkasnya dihapus saat nilai ini habis masa pakainya — termasuk saat handler
/// keluar lebih awal karena isian lain tak sah. Itu sebabnya penghapusannya ada
/// di `Drop` dan bukan dipanggil manual di tiap jalur keluar: jalur yang
/// terlupakan meninggalkan berkas yatim yang menumpuk diam-diam sampai disk
/// server penuh.
pub struct BerkasSementara {
    path: PathBuf,
    /// Ukuran sebenarnya, dihitung saat menulis — bukan dari header klien.
    pub ukuran: usize,
    kepala: Vec<u8>,
}

impl BerkasSementara {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Tipe isi menurut byte pertamanya; `None` bila bukan format yang didukung.
    pub fn tipe_isi(&self) -> Option<&'static str> {
        crate::web::filetype::sniff(&self.kepala)
    }

    /// Apakah isinya benar-benar bertipe `declared` (bukan sekadar namanya)?
    pub fn cocok(&self, declared: &str) -> bool {
        crate::web::filetype::matches(&self.kepala, declared)
    }
}

impl Drop for BerkasSementara {
    fn drop(&mut self) {
        // `std::fs`, bukan `tokio::fs`: `Drop` tak bisa menunggu. Satu `unlink`
        // memang menghentikan worker sesaat, tapi itu jauh lebih murah daripada
        // menyimpan penghapusan ke dalam task terpisah yang bisa hilang saat
        // proses berhenti.
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Tulis satu field berkas ke disk sambil menghitung ukurannya.
///
/// `maks` adalah batas KERAS rute itu (lihat [`limits`]); batas yang lebih
/// ketat per jenis isi — mis. "gambar maks 10MB pada rute yang juga menerima
/// video" — diperiksa pemanggil lewat [`BerkasSementara::ukuran`] sesudahnya,
/// karena jenisnya baru diketahui setelah byte pertamanya terbaca.
///
/// Galat dikembalikan sebagai [`Response`] siap pakai: seluruh pemanggilnya
/// adalah handler axum yang memang membalas apa adanya ke layar pengunggah.
///
/// [`limits`]: crate::web::limits
pub async fn terima_berkas(
    mut field: Field<'_>,
    maks: usize,
    batas_pesan: &str,
) -> Result<BerkasSementara, Response> {
    let dir = upload_tmp_dir();
    if let Err(e) = tokio::fs::create_dir_all(&dir).await {
        crate::service::telegram::report_error(500, "Upload tmp dir", e.to_string());
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Server tidak bisa menyiapkan ruang unggahan. Hubungi admin teknis.",
        )
            .into_response());
    }
    let path = dir.join(format!(
        "{}-{}-{}.part",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        URUTAN.fetch_add(1, Ordering::Relaxed)
    ));

    let mut file = match tokio::fs::File::create(&path).await {
        Ok(f) => f,
        Err(e) => {
            crate::service::telegram::report_error(500, "Upload tmp create", e.to_string());
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Server gagal menyimpan berkas sementara. Coba lagi.",
            )
                .into_response());
        }
    };

    // Dibungkus SEJAK AWAL supaya berkas separuh jadi ikut terhapus lewat `Drop`
    // pada setiap `?` di bawah ini.
    let mut berkas = BerkasSementara { path, ukuran: 0, kepala: Vec::with_capacity(KEPALA_MAKS) };

    loop {
        let potongan = match field.chunk().await {
            Ok(Some(c)) => c,
            Ok(None) => break,
            Err(e) => return Err((StatusCode::BAD_REQUEST, e.to_string()).into_response()),
        };
        berkas.ukuran += potongan.len();
        // Diperiksa di TENGAH aliran, bukan sesudahnya: menunggu sampai selesai
        // berarti menulis berkas 2 GB ke disk hanya untuk menolaknya.
        if berkas.ukuran > maks {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("Ukuran file tidak valid ({batas_pesan})."),
            )
                .into_response());
        }
        if berkas.kepala.len() < KEPALA_MAKS {
            let ambil = (KEPALA_MAKS - berkas.kepala.len()).min(potongan.len());
            berkas.kepala.extend_from_slice(&potongan[..ambil]);
        }
        if let Err(e) = file.write_all(&potongan).await {
            crate::service::telegram::report_error(500, "Upload tmp write", e.to_string());
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Server gagal menyimpan berkas sementara. Coba lagi.",
            )
                .into_response());
        }
    }

    // `flush` sebelum dibaca ulang oleh pengunggah S3 — tanpa ini byte terakhir
    // bisa masih tertahan di buffer dan berkas terunggah dalam keadaan terpotong.
    if let Err(e) = file.flush().await {
        crate::service::telegram::report_error(500, "Upload tmp flush", e.to_string());
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Server gagal menyimpan berkas sementara. Coba lagi.",
        )
            .into_response());
    }

    Ok(berkas)
}
