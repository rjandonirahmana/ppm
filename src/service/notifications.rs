//! service/notifications.rs — Notifikasi dalam aplikasi untuk alur izin.
//!
//! ── SIAPA DAPAT APA ────────────────────────────────────────────────────────
//!   • Santri mengajukan izin/sakit/pulang → WALI KELAS KBM-nya + SEMUA ADMIN.
//!   • Wali kelas memutuskan (setuju/tolak) → SANTRI yang mengajukan.
//!
//! Wali kelas ada di daftar karena dialah yang memutuskan; admin karena dialah
//! yang harus tahu ada yang tak beres — izin yang menggantung berhari-hari
//! tidak terlihat oleh siapa pun kalau hanya wali kelasnya yang diberi tahu,
//! dan wali kelas itu sendiri yang sedang tak membuka aplikasi.
//!
//! ── SEMUANYA BEST-EFFORT ───────────────────────────────────────────────────
//! Tak satu pun fungsi di sini mengembalikan `Result` ke pemanggilnya. Itu
//! disengaja, dan bukan kemalasan: notifikasi adalah KABAR tentang sesuatu yang
//! sudah terjadi. Kalau penulisan kabarnya gagal, yang benar adalah izinnya
//! tetap tercatat dan kegagalannya masuk log — bukan pengajuan santri ditolak
//! karena barisan notifikasinya bermasalah. Ini menyamai perlakuan yang sudah
//! dipakai untuk notifikasi WhatsApp di `service::permits`.

use deadpool_postgres::Pool;

use crate::models::notifikasi::jenis;
use crate::repository::{self as repo, NotifBaru};

/// Sebutan yang dikenali santri untuk tiap jenis izin.
///
/// Sengaja TIDAK memakai `permit_kind_label` yang dipakai layar antrean: di
/// sana labelnya berdiri sendiri di dalam kolom ("Sakit"), di sini ia masuk ke
/// tengah kalimat ("mengajukan izin sakit"), dan bentuk yang benar untuk
/// keduanya tidak sama.
fn sebutan(kind: &str) -> &'static str {
    match kind {
        "sick" => "izin sakit",
        "pulang" => "izin pulang",
        _ => "izin",
    }
}

/// Susun daftar penerima pengajuan izin: semua admin + wali kelasnya.
///
/// Dipisah jadi fungsi murni supaya aturannya bisa diuji tanpa database. Yang
/// dijaga di sini kecil tapi mudah rusak diam-diam: seorang admin yang KEBETULAN
/// juga wali kelas santri itu tak boleh menerima notifikasi yang sama dua kali,
/// dan urutan penulisannya harus tetap sama dari waktu ke waktu supaya barisnya
/// bisa dibandingkan saat menelusuri masalah.
fn penerima(admins: &[i64], wali_kelas_id: Option<i64>) -> Vec<i64> {
    let mut v: Vec<i64> = admins.to_vec();
    if let Some(w) = wali_kelas_id {
        v.push(w);
    }
    v.sort_unstable();
    v.dedup();
    v
}

/// Judul & isi notifikasi PENGAJUAN.
///
/// Dipisah dari fungsi yang menyentuh database supaya tiap cabangnya bisa
/// diuji: jenis izin, ada/tidaknya kelas, dan siapa yang mengajukan. Ketiganya
/// bergabung jadi satu kalimat yang dibaca wali kelas sebelum memutuskan, dan
/// kalimat yang salah di situ berarti ia menimbang dengan keterangan keliru.
///
/// `pengaju` = `None` berarti santri mengajukan sendiri; `Some((nama, peran))`
/// berarti orang lain atas namanya.
fn teks_pengajuan(
    kind: &str,
    nama_santri: &str,
    kelas: Option<&str>,
    rentang: &str,
    pengaju: Option<(&str, &str)>,
) -> (String, String) {
    let oleh = match pengaju {
        None => "santri sendiri".to_string(),
        Some((nama, peran)) => format!("{nama} ({peran})"),
    };
    // Kelas kosong TIDAK menghasilkan pemisah "·" yang menggantung tanpa apa-apa
    // di belakangnya — santri yang belum punya kelas KBM justru yang paling
    // sering muncul di sini (lihat catatan penerima di bawah).
    let kelas_label = kelas.map(|n| format!(" · {n}")).unwrap_or_default();
    (
        format!("Pengajuan {} baru", sebutan(kind)),
        format!("{nama_santri}{kelas_label}\n{rentang}\nDiajukan oleh: {oleh}"),
    )
}

/// Jenis, judul & isi notifikasi KEPUTUSAN.
///
/// Mengembalikan jenisnya sekalian karena jenis dan judul harus selalu sepakat:
/// dipisah, "disetujui" bisa terkirim dengan ikon penolakan, dan tak ada yang
/// menangkapnya sampai ada yang mengeluh.
fn teks_keputusan(
    kind: &str,
    disetujui: bool,
    rentang: &str,
    wali: Option<&str>,
) -> (&'static str, String, String) {
    let (jenis_notif, kata) = if disetujui {
        (jenis::IZIN_DISETUJUI, "disetujui")
    } else {
        (jenis::IZIN_DITOLAK, "ditolak")
    };
    // Wali tanpa nama (izin yang naik ke admin karena santrinya belum berkelas)
    // tak menghasilkan baris "Oleh:" kosong.
    let oleh = wali.map(|w| format!("\nOleh: {w}")).unwrap_or_default();
    (jenis_notif, format!("{} {kata}", sebutan(kind)), format!("{rentang}{oleh}"))
}

/// Ada pengajuan izin baru → beri tahu WALI KELAS-nya + SEMUA ADMIN.
///
/// Menerima id barisnya, bukan detail pengajuannya, lalu MEMBACA ULANG baris
/// itu. Alasannya sama seperti pada keputusan di bawah: yang dikabarkan harus
/// persis apa yang tersimpan. Ia juga membuat pemanggilnya tak perlu mengurai
/// sendiri tanggal & jam yang sudah diurai `submit_permit` — satu tempat yang
/// mengerti bentuk waktunya, bukan dua yang bisa berbeda pendapat.
///
/// Diberi `&[i64]` karena satu pengajuan secara historis bisa terpecah ke
/// beberapa wali (migrasi 46). Sejak migrasi 65 isinya selalu satu, tapi
/// bentuknya dipertahankan supaya pemecahan yang mungkin kembali tak diam-diam
/// kehilangan notifikasinya.
pub async fn izin_diajukan(pool: &Pool, permit_ids: &[i64]) {
    // Admin sama untuk seluruh baris — ambil SEKALI, di luar loop. Di dalam
    // loop ia jadi satu query per baris untuk jawaban yang tak pernah berubah.
    let admins = match repo::notif_admin_ids(pool).await {
        Ok(ids) => ids,
        Err(e) => {
            tracing::warn!("notif izin: gagal ambil daftar admin: {e}");
            Vec::new()
        }
    };

    for &permit_id in permit_ids {
        let d = match repo::permit_detail(pool, permit_id).await {
            Ok(Some(d)) => d,
            Ok(None) => continue,
            Err(e) => {
                tracing::warn!(permit_id, "notif izin baru: gagal baca detail: {e}");
                continue;
            }
        };

        let penerima = penerima(&admins, d.wali_kelas_id);

        if penerima.is_empty() {
            tracing::warn!(permit_id, "notif izin baru: tak ada penerima");
            continue;
        }

        let (title, body) = teks_pengajuan(
            &d.kind,
            &d.student_name,
            d.class_name.as_deref(),
            &super::fmt::fmt_rentang(d.mulai, d.selesai),
            // Peran pengaju dibaca dari barisnya, bukan dititipkan pemanggil —
            // satu sumber kebenaran, dan mustahil terbalik.
            (d.requested_by != d.user_id).then_some((&d.requester_name, &d.requester_role)),
        );

        let items: Vec<NotifBaru> = penerima
            .into_iter()
            .map(|user_id| NotifBaru {
                user_id,
                kind: jenis::IZIN_BARU.into(),
                title: title.clone(),
                body: body.clone(),
                link: Some("/izin-staf".into()),
            })
            .collect();

        if let Err(e) = repo::notif_insert_many(pool, &items).await {
            tracing::warn!(permit_id, "notif izin baru gagal ditulis: {e}");
        }
    }
}

/// Susun baris notifikasi keputusan.
///
/// Penerimanya SELALU santri pemilik izin (`user_id`), bukan pengajunya. Bila
/// orang tua yang mengajukan, santrilah yang perlu tahu boleh atau tidak ia
/// pergi — dialah yang menanggung akibat keputusannya.
///
/// Orang tua pengaju IKUT diberi tahu, dengan nama santri ditambahkan ke
/// isinya: seorang ibu bisa punya beberapa anak di pondok ini, dan "izin
/// disetujui" tanpa menyebut siapa tak menjawab apa pun.
fn item_keputusan(
    user_id: i64,
    requested_by: i64,
    nama_santri: &str,
    kind: &str,
    title: &str,
    body: &str,
) -> Vec<NotifBaru> {
    let mut items = vec![NotifBaru {
        user_id,
        kind: kind.into(),
        title: title.into(),
        body: body.into(),
        link: Some("/izin".into()),
    }];
    if requested_by != user_id {
        items.push(NotifBaru {
            user_id: requested_by,
            kind: kind.into(),
            title: title.into(),
            body: format!("{body}\nSantri: {nama_santri}"),
            link: Some("/izin".into()),
        });
    }
    items
}

/// Izin sudah diputuskan → beri tahu SANTRI yang mengajukan.
///
/// Dipanggil SESUDAH keputusannya tercatat, dan membaca ulang barisnya dari
/// database alih-alih menerima detailnya sebagai parameter. Itu disengaja:
/// yang diberitakan harus persis apa yang tersimpan, bukan apa yang dikira
/// pemanggil tersimpan.
pub async fn izin_diputuskan(pool: &Pool, permit_id: i64, disetujui: bool) {
    let d = match repo::permit_detail(pool, permit_id).await {
        Ok(Some(d)) => d,
        Ok(None) => {
            tracing::warn!(permit_id, "notif keputusan izin: barisnya tak ditemukan");
            return;
        }
        Err(e) => {
            tracing::warn!(permit_id, "notif keputusan izin: gagal baca detail: {e}");
            return;
        }
    };

    let (kind, title, body) = teks_keputusan(
        &d.kind,
        disetujui,
        &super::fmt::fmt_rentang(d.mulai, d.selesai),
        d.wali_name.as_deref(),
    );

    let items =
        item_keputusan(d.user_id, d.requested_by, &d.student_name, kind, &title, &body);

    if let Err(e) = repo::notif_insert_many(pool, &items).await {
        tracing::warn!(permit_id, "notif keputusan izin gagal ditulis: {e}");
    }
}

/// Feed lonceng untuk satu orang: daftar + jumlah belum dibaca.
///
/// Batas 30 dengan sengaja tanpa paginasi: lonceng adalah "apa yang baru",
/// bukan arsip. Menggulir ratusan notifikasi di popover kecil bukan sesuatu
/// yang dilakukan orang, dan menyediakan halamannya berarti menyediakan
/// query-nya juga.
const BATAS_FEED: i64 = 30;

pub async fn feed(pool: &Pool, user_id: i64) -> anyhow::Result<crate::models::NotifData> {
    let (rows, belum_dibaca) = tokio::try_join!(
        repo::notif_list_for_user(pool, user_id, BATAS_FEED),
        repo::notif_unread_count(pool, user_id),
    )?;

    Ok(crate::models::NotifData {
        items: rows
            .into_iter()
            .map(|r| crate::models::NotifItem {
                id: r.id,
                kind: r.kind,
                title: r.title,
                body: r.body,
                link: r.link.unwrap_or_default(),
                dibaca: r.read_at.is_some(),
                waktu_label: super::fmt::fmt_ago(r.created_at),
            })
            .collect(),
        belum_dibaca,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Penerima pengajuan ───────────────────────────────────────────────────

    /// Admin yang juga wali kelas santri itu hanya dapat SATU notifikasi.
    ///
    /// Ini bukan soal kerapian: notifikasi ganda dari satu kejadian membuat
    /// penghitung di lonceng berbohong, dan penghitung yang berbohong adalah
    /// alasan orang berhenti mempercayainya.
    #[test]
    fn admin_yang_juga_wali_tak_dapat_dua_kali() {
        assert_eq!(penerima(&[1, 2, 3], Some(2)), vec![1, 2, 3]);
    }

    #[test]
    fn wali_ditambahkan_ke_daftar_admin() {
        assert_eq!(penerima(&[1, 2], Some(9)), vec![1, 2, 9]);
    }

    /// Santri tanpa kelas KBM tetap menghasilkan notifikasi ke admin — izinnya
    /// naik ke mereka, dan justru keadaan inilah yang paling perlu terlihat.
    #[test]
    fn tanpa_wali_tetap_ada_penerima() {
        assert_eq!(penerima(&[4, 5], None), vec![4, 5]);
    }

    /// Tanpa admin tapi ada wali: izin tetap sampai ke orang yang memutuskannya.
    #[test]
    fn tanpa_admin_wali_tetap_dapat() {
        assert_eq!(penerima(&[], Some(7)), vec![7]);
    }

    /// Tanpa admin DAN tanpa wali, tak ada yang bisa diberi tahu. Pemanggil
    /// mengandalkan daftar kosong ini untuk mencatat peringatan alih-alih
    /// menulis baris notifikasi yang tak berpenerima.
    #[test]
    fn tanpa_siapa_pun_hasilnya_kosong() {
        assert!(penerima(&[], None).is_empty());
    }

    #[test]
    fn urutan_penerima_stabil() {
        assert_eq!(penerima(&[7, 3, 5], Some(1)), vec![1, 3, 5, 7]);
    }

    /// Daftar admin yang sudah memuat duplikat (data kotor) tak boleh lolos.
    #[test]
    fn duplikat_di_daftar_admin_ikut_dibersihkan() {
        assert_eq!(penerima(&[2, 2, 5], Some(5)), vec![2, 5]);
    }

    // ── Sebutan jenis izin ───────────────────────────────────────────────────

    /// Sebutan dipakai di TENGAH kalimat ("Pengajuan izin sakit baru"), jadi ia
    /// harus sudah memuat kata "izin" — berbeda dari label kolom di layar
    /// antrean yang berdiri sendiri.
    #[test]
    fn sebutan_jenis_izin() {
        assert_eq!(sebutan("sick"), "izin sakit");
        assert_eq!(sebutan("pulang"), "izin pulang");
        assert_eq!(sebutan("leave"), "izin");
    }

    /// Jenis yang belum dikenal tak boleh menghasilkan kalimat rusak — ia
    /// jatuh ke sebutan umum, bukan string kosong.
    #[test]
    fn sebutan_jenis_tak_dikenal_tetap_masuk_akal() {
        assert_eq!(sebutan("entah_apa"), "izin");
        assert_eq!(sebutan(""), "izin");
    }

    // ── Teks pengajuan ───────────────────────────────────────────────────────

    #[test]
    fn teks_pengajuan_oleh_santri_sendiri() {
        let (judul, isi) =
            teks_pengajuan("sick", "Ahmad", Some("3 Ula"), "5 Sep 2026", None);
        assert_eq!(judul, "Pengajuan izin sakit baru");
        assert_eq!(isi, "Ahmad · 3 Ula\n5 Sep 2026\nDiajukan oleh: santri sendiri");
    }

    #[test]
    fn teks_pengajuan_oleh_orang_tua() {
        let (judul, isi) = teks_pengajuan(
            "pulang",
            "Ahmad",
            Some("3 Ula"),
            "5 Sep 2026",
            Some(("Bu Siti", "parent")),
        );
        assert_eq!(judul, "Pengajuan izin pulang baru");
        assert!(isi.ends_with("Diajukan oleh: Bu Siti (parent)"), "{isi}");
    }

    /// Santri tanpa kelas KBM: TIDAK boleh menyisakan pemisah "·" yang
    /// menggantung tanpa apa-apa di belakangnya.
    #[test]
    fn teks_pengajuan_tanpa_kelas_tak_menggantungkan_pemisah() {
        let (_, isi) = teks_pengajuan("leave", "Ahmad", None, "5 Sep 2026", None);
        assert!(!isi.contains('·'), "{isi}");
        assert!(isi.starts_with("Ahmad\n"), "{isi}");
    }

    /// Rentang waktunya diteruskan apa adanya — pemformatannya milik
    /// `service::fmt`, dan menyalinnya ke sini berarti dua aturan yang bisa
    /// menyimpang.
    #[test]
    fn teks_pengajuan_memuat_rentang_apa_adanya() {
        let (_, isi) =
            teks_pengajuan("sick", "Ahmad", None, "5 Sep 2026, 07:00 – 12:00 WIB", None);
        assert!(isi.contains("5 Sep 2026, 07:00 – 12:00 WIB"), "{isi}");
    }

    // ── Teks keputusan ───────────────────────────────────────────────────────

    /// Jenis dan judul HARUS selalu sepakat: dipisah, "disetujui" bisa terkirim
    /// dengan ikon penolakan tanpa ada yang menangkapnya.
    #[test]
    fn teks_keputusan_disetujui() {
        let (jenis_notif, judul, isi) =
            teks_keputusan("sick", true, "5 Sep 2026", Some("Ust. Ali"));
        assert_eq!(jenis_notif, jenis::IZIN_DISETUJUI);
        assert_eq!(judul, "izin sakit disetujui");
        assert_eq!(isi, "5 Sep 2026\nOleh: Ust. Ali");
    }

    #[test]
    fn teks_keputusan_ditolak() {
        let (jenis_notif, judul, _) =
            teks_keputusan("pulang", false, "5 Sep 2026", Some("Ust. Ali"));
        assert_eq!(jenis_notif, jenis::IZIN_DITOLAK);
        assert_eq!(judul, "izin pulang ditolak");
    }

    /// Izin yang naik ke admin (santri belum berkelas) tak punya nama wali —
    /// jangan menyisakan baris "Oleh:" yang kosong.
    #[test]
    fn teks_keputusan_tanpa_wali_tak_menyisakan_baris_kosong() {
        let (_, _, isi) = teks_keputusan("leave", true, "5 Sep 2026", None);
        assert_eq!(isi, "5 Sep 2026");
        assert!(!isi.contains("Oleh:"), "{isi}");
    }

    // ── Baris notifikasi keputusan ───────────────────────────────────────────

    /// Santri mengajukan sendiri → tepat satu baris, untuknya.
    #[test]
    fn keputusan_santri_sendiri_satu_baris() {
        let items = item_keputusan(10, 10, "Ahmad", jenis::IZIN_DISETUJUI, "judul", "isi");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].user_id, 10);
        assert_eq!(items[0].body, "isi");
        assert_eq!(items[0].link.as_deref(), Some("/izin"));
    }

    /// Ortu yang mengajukan → DUA baris: santri dan ortunya.
    #[test]
    fn keputusan_oleh_ortu_dua_baris() {
        let items = item_keputusan(10, 77, "Ahmad", jenis::IZIN_DITOLAK, "judul", "isi");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].user_id, 10, "santri selalu lebih dulu");
        assert_eq!(items[1].user_id, 77);
    }

    /// Ortu bisa punya beberapa anak di pondok ini, jadi barisnya HARUS
    /// menyebut santri yang mana — sementara baris milik santri sendiri tidak
    /// perlu menyebut namanya lagi.
    #[test]
    fn baris_ortu_menyebut_nama_santri_baris_santri_tidak() {
        let items = item_keputusan(10, 77, "Ahmad", jenis::IZIN_DISETUJUI, "judul", "isi");
        assert!(!items[0].body.contains("Santri:"), "{}", items[0].body);
        assert!(items[1].body.contains("Santri: Ahmad"), "{}", items[1].body);
    }

    /// Kedua baris membawa jenis yang sama — ikon di lonceng tak boleh berbeda
    /// untuk satu kejadian yang sama.
    #[test]
    fn kedua_baris_keputusan_sejenis() {
        let items = item_keputusan(10, 77, "Ahmad", jenis::IZIN_DITOLAK, "judul", "isi");
        assert_eq!(items[0].kind, jenis::IZIN_DITOLAK);
        assert_eq!(items[1].kind, jenis::IZIN_DITOLAK);
        assert_eq!(items[0].title, items[1].title);
    }

    // ── Jenis notifikasi ─────────────────────────────────────────────────────

    /// Nilai `kind` tersimpan di database dan dibaca UI untuk memilih ikon.
    /// Mengubah ejaannya membuat seluruh notifikasi lama jatuh ke ikon bawaan
    /// tanpa satu pun galat — jadi ejaannya dikunci di sini.
    #[test]
    fn ejaan_jenis_terkunci() {
        assert_eq!(jenis::IZIN_BARU, "izin_baru");
        assert_eq!(jenis::IZIN_DISETUJUI, "izin_disetujui");
        assert_eq!(jenis::IZIN_DITOLAK, "izin_ditolak");
    }
}
