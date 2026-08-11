//! web/upload.rs — Mengunggah berkas dari peramban, SATU kali ditulis.
//!
//! Lima layar mengunggah berkas lewat handler multipart: materi, galeri, sampul
//! artikel, dan dua form pembayaran. Kelimanya dulu menyalin kerangka yang sama
//! persis — `FormData` → `RequestInit` → `Request` → `fetch` → periksa
//! `resp.ok()` → baca badan balasan — sekitar 40 baris interop `web_sys` mentah
//! per salinan.
//!
//! Itu kode paling rapuh di seluruh halaman: penuh `unwrap()` atas JsValue,
//! `dyn_into` yang bisa gagal diam-diam, dan penanganan galat yang berbeda-beda
//! di tiap salinan. Yang satu menampilkan pesan dari server, yang lain
//! menelannya jadi kalimat generik; yang satu membedakan koneksi putus dari
//! penolakan server, yang lain tidak. Perbedaan itu bukan keputusan desain,
//! hanya sisa dari urutan penulisannya.
//!
//! Di sini semuanya jadi satu fungsi, dan pesan galatnya jadi seragam — termasuk
//! yang paling sering terjadi di lapangan: unggahan besar yang putus di tengah
//! pada jaringan pondok.

/// Ambil berkas ke-`idx` dari sebuah `<input type="file">`.
///
/// `None` bila belum ada yang dipilih. Menggantikan rantai
/// `.get().and_then(|i| i.files()).and_then(|f| f.get(n))` yang disalin di tiap
/// layar unggah.
#[cfg(target_arch = "wasm32")]
pub fn berkas_ke(
    input: leptos::prelude::NodeRef<leptos::html::Input>,
    idx: u32,
) -> Option<web_sys::File> {
    use leptos::prelude::GetUntracked;
    input.get_untracked().and_then(|i| i.files()).and_then(|f| f.get(idx))
}

/// Berkas pertama dari sebuah `<input type="file">` — bentuk yang dipakai
/// hampir semua layar (hanya galeri yang mengunggah antrean).
#[cfg(target_arch = "wasm32")]
pub fn berkas_pertama(
    input: leptos::prelude::NodeRef<leptos::html::Input>,
) -> Option<web_sys::File> {
    berkas_ke(input, 0)
}

/// Berapa berkas yang sedang dipilih di sebuah `<input type="file">`.
#[cfg(target_arch = "wasm32")]
pub fn jumlah_berkas(input: leptos::prelude::NodeRef<leptos::html::Input>) -> u32 {
    use leptos::prelude::GetUntracked;
    input.get_untracked().and_then(|i| i.files()).map(|f| f.length()).unwrap_or(0)
}

/// Unggah satu berkas + kolom teks ke handler multipart.
///
/// `Ok(badan balasan)` — teks apa adanya; pemanggil yang butuh JSON tinggal
/// mengurainya. `Err(pesan)` sudah berupa kalimat yang pantas langsung
/// ditampilkan: badan balasan server bila ia mengirim penjelasan (handler di
/// proyek ini memang selalu mengirimnya dalam bahasa Indonesia), atau kalimat
/// bawaan bila tidak.
///
/// Koneksi yang putus dibedakan dari penolakan server. Dulu keduanya jatuh ke
/// pesan yang sama, dan pengguna disuruh "periksa berkasnya" padahal yang salah
/// adalah jaringannya — pada unggahan video puluhan MB di jaringan pondok, itu
/// justru kasus yang paling sering terjadi.
#[cfg(target_arch = "wasm32")]
pub async fn unggah(
    url: &str,
    berkas: &web_sys::File,
    kolom: &[(&str, String)],
) -> Result<String, String> {
    use wasm_bindgen::JsCast;

    let form = web_sys::FormData::new().map_err(|_| "Peramban ini tak mendukung unggahan berkas.".to_string())?;
    form.append_with_blob("file", berkas)
        .map_err(|_| "Berkasnya tak bisa dibaca peramban. Coba pilih ulang.".to_string())?;
    for (nama, nilai) in kolom {
        let _ = form.append_with_str(nama, nilai);
    }

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(form.as_ref());

    let req = web_sys::Request::new_with_str_and_init(url, &opts)
        .map_err(|_| "Alamat unggahan tidak sah.".to_string())?;
    let window = web_sys::window().ok_or_else(|| "Peramban tak siap.".to_string())?;

    let balasan = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&req))
        .await
        .map_err(|_| {
            "Koneksi terputus saat mengunggah. Berkas besar butuh jaringan yang stabil — \
             coba lagi setelah sinyalnya membaik."
                .to_string()
        })?;
    let resp: web_sys::Response = balasan
        .dyn_into()
        .map_err(|_| "Balasan server tak bisa dibaca.".to_string())?;

    // Badan balasan dibaca untuk KEDUA keadaan. Pada kegagalan, di situlah
    // penjelasan server berada ("Foto bukti transfer wajib diunggah (maks
    // 10MB)"); membuangnya berarti mengganti kalimat yang tepat dengan tebakan.
    let badan = match resp.text() {
        Ok(p) => wasm_bindgen_futures::JsFuture::from(p)
            .await
            .ok()
            .and_then(|t| t.as_string())
            .unwrap_or_default(),
        Err(_) => String::new(),
    };

    if resp.ok() {
        Ok(badan)
    } else if badan.trim().is_empty() {
        Err(format!("Unggahan ditolak server (HTTP {}).", resp.status()))
    } else {
        Err(badan)
    }
}

/// Ambil satu field teks dari badan balasan berbentuk JSON.
///
/// Dipakai handler yang membalas `{ "url": "…" }`. `serde_json` memang ikut ke
/// bundel WASM apa pun yang kita lakukan — `leptos`, `server_fn`, `codee`, dan
/// `gloo-net` semuanya menariknya untuk mendekode balasan server fn — jadi
/// memakainya di sini tak menambah satu byte pun, dan jauh lebih terbaca
/// daripada rangkaian `js_sys::Reflect::get` yang dipakai sebelumnya.
#[cfg(target_arch = "wasm32")]
pub fn field_teks(badan: &str, nama: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(badan)
        .ok()?
        .get(nama)?
        .as_str()
        .map(|s| s.to_string())
}
