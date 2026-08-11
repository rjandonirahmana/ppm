//! service/storage.rs — RustFS (S3-compatible) untuk REKAMAN siaran sesi.
//!
//! Pola StorageService e-ticketing/wedding-web, dipangkas untuk ppm: hanya
//! upload FILE rekaman dari disk (streaming via ByteStream::from_path — file
//! tak pernah dimuat penuh ke RAM) dengan KEY deterministik bermakna
//! `ppm/<kelas>/<sesi>/<tanggal>-sesi-<id>.webm` (bukan uuid acak).
//! ACL per-object tak didukung RustFS — set bucket public via
//! `mc anonymous set public` di server (catatan e-ticketing).

use std::path::Path;

use anyhow::{anyhow, Result};
use aws_sdk_s3::{
    config::{BehaviorVersion, Builder, Credentials, Region},
    primitives::ByteStream,
    Client,
};

use crate::config::RustFsConfig;

#[derive(Clone)]
pub struct StorageService {
    pub client: Client,
    pub bucket: String,
    /// Base public URL tanpa trailing slash.
    pub public_url: String,
}

impl StorageService {
    pub fn new(cfg: &RustFsConfig) -> Self {
        let creds = Credentials::new(&cfg.access_key, &cfg.secret_key, None, None, "rustfs");
        let config = Builder::new()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new("us-east-1"))
            .endpoint_url(&cfg.endpoint)
            .credentials_provider(creds)
            .force_path_style(true)
            .build();
        Self {
            client: Client::from_conf(config),
            bucket: cfg.bucket.clone(),
            public_url: cfg.public_url.trim_end_matches('/').to_string(),
        }
    }

    /// Pastikan bucket ada; best-effort (tak fatal bila list gagal).
    pub async fn init(&self) -> Result<()> {
        match self.client.list_buckets().send().await {
            Ok(list) => {
                let exists = list.buckets().iter().any(|b| b.name() == Some(&self.bucket));
                if !exists {
                    self.client
                        .create_bucket()
                        .bucket(&self.bucket)
                        .send()
                        .await
                        .map_err(|e| anyhow!("gagal membuat bucket: {e}"))?;
                    tracing::info!("Bucket '{}' dibuat", self.bucket);
                } else {
                    tracing::info!("Bucket '{}' siap", self.bucket);
                }
            }
            Err(e) => {
                tracing::warn!("Tidak bisa list bucket ({e}); asumsikan '{}' ada", self.bucket)
            }
        }
        Ok(())
    }

    /// Upload file dari disk (streaming) ke `key`, balas URL publik
    /// `{public_url}/{bucket}/{key}`. Key yang sama = overwrite (idempotent —
    /// dipakai finalisasi ulang sesi yang sama).
    pub async fn upload_file(&self, path: &Path, key: &str, content_type: &str) -> Result<String> {
        let body = ByteStream::from_path(path)
            .await
            .map_err(|e| anyhow!("gagal membuka file rekaman: {e}"))?;
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type(content_type)
            .body(body)
            .send()
            .await
            .map_err(|e| anyhow!("RustFS upload gagal: {e}"))?;
        Ok(format!("{}/{}/{}", self.public_url, self.bucket, key))
    }

    /// Kunci objek dari URL publiknya. `None` bila URL itu bukan milik bucket
    /// ini — tautan luar (YouTube di Materials Library, foto lama dari
    /// penyimpanan sebelumnya) tak boleh diperlakukan sebagai objek kita.
    pub fn key_dari_url(&self, url: &str) -> Option<String> {
        url.strip_prefix(&format!("{}/{}/", self.public_url, self.bucket))
            .filter(|k| !k.is_empty())
            .map(|k| k.to_string())
    }

    /// Hapus objek berdasarkan URL publiknya. `Ok(false)` = bukan objek kita,
    /// tak ada yang dihapus.
    ///
    /// ── KENAPA INI ADA ───────────────────────────────────────────────────────
    /// Sebelumnya tak ada satu pun jalan untuk menghapus objek. Menghapus foto
    /// galeri, materi, atau rekaman hanya membuang BARISNYA di basis data;
    /// berkasnya tetap di RustFS selamanya, tak tertaut apa pun dan tak
    /// terlihat siapa pun. Dengan batas video 100 MB dan disk server 100 GB,
    /// beberapa ratus unggahan yang "sudah dihapus" cukup untuk memenuhinya —
    /// dan gejalanya baru muncul sebagai unggahan yang gagal tanpa sebab jelas.
    ///
    /// Kegagalan hapus TIDAK boleh menggagalkan operasinya: yang penting bagi
    /// pengguna adalah barisnya hilang dari layar. Objek yang gagal dihapus
    /// menjadi sampah yang bisa disapu belakangan — jauh lebih ringan daripada
    /// baris basis data yang tak jadi terhapus karena penyimpanan sedang mati.
    pub async fn delete_by_url(&self, url: &str) -> Result<bool> {
        let Some(key) = self.key_dari_url(url) else {
            return Ok(false);
        };
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| anyhow!("RustFS hapus gagal: {e}"))?;
        Ok(true)
    }

    /// Hapus best-effort: kegagalannya dicatat, bukan diteruskan.
    ///
    /// Bentuk yang dipakai hampir semua pemanggil — mereka menghapus baris DB
    /// sebagai pekerjaan utama, dan pembersihan berkas hanyalah kerapian.
    pub async fn delete_by_url_best_effort(&self, url: &str) {
        match self.delete_by_url(url).await {
            Ok(true) => tracing::info!(%url, "objek penyimpanan dihapus"),
            Ok(false) => {}
            Err(e) => tracing::warn!(%url, "gagal menghapus objek penyimpanan: {e:#}"),
        }
    }

    /// Upload dari memori (mis. body multipart satu-kali, materials library) —
    /// beda dari `upload_file` yang streaming dari disk (dipakai rekaman yang
    /// ditulis bertahap per chunk).
    pub async fn upload_bytes(&self, bytes: Vec<u8>, key: &str, content_type: &str) -> Result<String> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type(content_type)
            .body(ByteStream::from(bytes))
            .send()
            .await
            .map_err(|e| anyhow!("RustFS upload gagal: {e}"))?;
        Ok(format!("{}/{}/{}", self.public_url, self.bucket, key))
    }
}
