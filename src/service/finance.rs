//! service/finance.rs — Aturan pembayaran santri (migrasi 37 & 75).
//!
//! Sampai sekarang urusan keuangan dipanggil langsung dari `web/api.rs` ke
//! repository. Itu bisa bertahan selama isinya hanya "ambil daftar", tapi alur
//! pengajuan membawa aturan sungguhan — siapa boleh mengajukan atas nama siapa,
//! periode apa yang sah, kapan WhatsApp boleh dikirim — dan aturan yang tinggal
//! di lapisan web akan terlewat oleh server fn berikutnya yang memanggil
//! repository yang sama.
//!
//! ALUR PENGAJUAN. Keluarga menyetor uang → mengunggah nominal + foto bukti
//! transfer (`bills` status `menunggu`, periode masih kosong) → pengurus
//! keuangan mencocokkannya dengan mutasi rekening lalu menetapkan periode
//! berlakunya (`lunas`) atau menolaknya dengan alasan (`ditolak`).

use anyhow::Result;
use chrono::NaiveDate;
use deadpool_postgres::Pool;

use crate::models::{BillItem, TunggakanData, TunggakanItem};
use crate::repository as repo;

use super::fmt::{fmt_ago, today_wib};

/// Batas atas nominal satu setoran (Rp 100 juta).
///
/// Bukan aturan pondok, melainkan pagar salah ketik: nol berlebih pada isian
/// nominal adalah kesalahan yang paling mudah terjadi dan paling
/// membingungkan akibatnya di laporan keuangan.
const MAX_NOMINAL: i64 = 100_000_000;

/// Periode terpanjang yang masuk akal untuk satu setoran (2 tahun).
const MAX_HARI_PERIODE: i64 = 730;

// ── Pengajuan (santri / orang tua) ───────────────────────────────────────────

/// Bolehkah `aktor` mengajukan pembayaran atas nama `student_id`?
///
/// Dua jalur saja: santri untuk dirinya sendiri, atau orang tua yang koneksinya
/// SUDAH disetujui. Koneksi berstatus `pending` sengaja tidak cukup — kalau
/// tidak, siapa pun bisa mengirim permintaan koneksi lalu langsung menempelkan
/// bukti transfer ke akun santri mana pun.
pub async fn boleh_mengajukan(pool: &Pool, aktor_id: i64, student_id: i64) -> Result<bool> {
    if aktor_id == student_id {
        return Ok(true);
    }
    repo::is_connected(pool, aktor_id, student_id).await
}

/// Validasi nominal setoran. Dipakai jalur pengajuan DAN verifikasi supaya
/// keduanya tak bisa diam-diam menerima angka yang berbeda.
pub fn periksa_nominal(amount: i64) -> Result<()> {
    if amount <= 0 {
        bail_user!("Nominal pembayaran harus lebih dari 0.");
    }
    if amount > MAX_NOMINAL {
        bail_user!("Nominal terlalu besar — periksa lagi jumlah nolnya.");
    }
    Ok(())
}

/// Berapa anak yang boleh masuk dalam satu kiriman.
///
/// Bukan aturan pondok — pagar salah kirim. Keluarga terbesar di sini punya
/// segelintir anak; angka di atas itu berarti isian yang dibuat program, bukan
/// orang tua yang sedang membayar.
const MAX_ANAK_PER_KIRIMAN: usize = 10;

/// Periksa seluruh isi satu kiriman multi-anak SEBELUM apa pun disimpan:
/// nominal tiap anak sah, tak ada anak ganda, pengaju berhak atas semuanya, dan
/// tak satu pun sedang punya pengajuan yang belum diperiksa.
///
/// Dijalankan sekaligus, bukan per-anak sambil menyimpan: kiriman yang separuh
/// masuk membuat keluarga melihat satu anak tercatat dan menyimpulkan yang lain
/// hilang, lalu mengirim ulang — dan setoran yang sama tercatat dua kali.
pub async fn periksa_kiriman(
    pool: &Pool,
    aktor_id: i64,
    items: &[repo::PengajuanAnak],
) -> Result<()> {
    if items.is_empty() {
        bail_user!("Pilih dulu santri yang mau dibayarkan.");
    }
    if items.len() > MAX_ANAK_PER_KIRIMAN {
        bail_user!("Terlalu banyak santri dalam satu kiriman (maks {MAX_ANAK_PER_KIRIMAN}).");
    }
    let mut ids: Vec<i64> = items.iter().map(|i| i.student_id).collect();
    ids.sort_unstable();
    let sebelum = ids.len();
    ids.dedup();
    if ids.len() != sebelum {
        bail_user!("Ada santri yang terpilih dua kali.");
    }
    for it in items {
        periksa_nominal(it.amount)?;
        if !boleh_mengajukan(pool, aktor_id, it.student_id).await? {
            bail_user!("Anda tidak terhubung dengan salah satu santri yang dipilih.");
        }
    }
    let menunggu = repo::punya_pengajuan_menunggu(pool, &ids).await?;
    if !menunggu.is_empty() {
        bail_user!(
            "Masih ada pengajuan yang sedang diperiksa untuk {}. Tunggu hasilnya dulu \
             supaya setoran tidak tercatat dua kali.",
            ringkas_nama(&menunggu)
        );
    }
    Ok(())
}

/// Daftar pembayaran satu santri untuk layarnya sendiri / layar orang tuanya.
/// Guard kepemilikan dijalankan di sini, bukan diserahkan ke pemanggil.
pub async fn riwayat_santri(
    pool: &Pool,
    aktor_id: i64,
    student_id: i64,
) -> Result<Vec<BillItem>> {
    if !boleh_mengajukan(pool, aktor_id, student_id).await? {
        bail_user!("Anda tidak terhubung dengan santri ini.");
    }
    repo::list_for_student(pool, student_id, 100).await
}

// ── Verifikasi (ketua / santri_finance) ──────────────────────────────────────

/// Setujui satu pengajuan dan tetapkan periode berlakunya.
///
/// Periode WAJIB diisi di sini — itulah inti pekerjaan verifikator, dan
/// pembayaran tanpa periode tak bisa dipakai menghitung siapa yang sudah
/// waktunya membayar lagi.
#[allow(clippy::too_many_arguments)]
pub async fn setujui(
    pool: &Pool,
    bill_id: i64,
    judul: &str,
    started: &str,
    expired: &str,
    paid_amount: i64,
    method: &str,
    verified_by: i64,
) -> Result<()> {
    let sd = urai_tanggal(started, "Tanggal mulai periode")?;
    let ed = urai_tanggal(expired, "Tanggal akhir periode")?;
    if ed < sd {
        bail_user!("Akhir periode tidak boleh sebelum awal periode.");
    }
    if (ed - sd).num_days() > MAX_HARI_PERIODE {
        bail_user!("Periode lebih dari 2 tahun — periksa lagi tanggalnya.");
    }
    periksa_nominal(paid_amount)?;
    let judul = judul.trim();
    let judul = if judul.is_empty() { "Pembayaran pondok" } else { judul };
    let method = match method.trim() {
        "tunai" => "tunai",
        _ => "transfer",
    };
    let ok =
        repo::setujui_pengajuan(pool, bill_id, judul, sd, ed, paid_amount, method, verified_by)
            .await?;
    if !ok {
        // Bukan "tidak ditemukan": barisnya hampir pasti ada, hanya sudah
        // diproses orang lain sedetik lebih dulu. Pesan yang menyebut itu
        // mencegah verifikator mengira sistemnya rusak lalu mencoba lagi.
        bail_user!("Pengajuan ini sudah diproses pengurus lain. Muat ulang daftarnya.");
    }
    Ok(())
}

/// Tolak satu pengajuan. Alasan WAJIB — santri membaca teks ini di layarnya,
/// dan penolakan tanpa sebab hanya memindahkan pertanyaannya ke grup WhatsApp.
pub async fn tolak(pool: &Pool, bill_id: i64, alasan: &str, verified_by: i64) -> Result<()> {
    let alasan = alasan.trim();
    if alasan.len() < 5 {
        bail_user!("Tulis alasan penolakan (mis. \"tidak ada mutasi masuk sejumlah itu\").");
    }
    if alasan.chars().count() > 300 {
        bail_user!("Alasan terlalu panjang (maks 300 karakter).");
    }
    if !repo::tolak_pengajuan(pool, bill_id, alasan, verified_by).await? {
        bail_user!("Pengajuan ini sudah diproses pengurus lain. Muat ulang daftarnya.");
    }
    Ok(())
}

fn urai_tanggal(s: &str, label: &str) -> Result<NaiveDate> {
    match NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d") {
        Ok(d) => Ok(d),
        Err(_) => bail_user!("{label} belum diisi atau formatnya salah."),
    }
}

// ── Periode terlewat + pengingat ─────────────────────────────────────────────

/// Santri yang masa berlaku pembayarannya habis, dipisah dari yang belum
/// pernah tercatat sama sekali (lihat [`TunggakanData`]).
pub async fn tunggakan(pool: &Pool) -> Result<TunggakanData> {
    let hari_ini = today_wib();
    let mut data = TunggakanData::default();
    for r in repo::periode_terlewat(pool).await? {
        let belum_pernah = r.habis.is_none();
        let item = TunggakanItem {
            user_id: r.user_id,
            name: r.full_name,
            nis: r.nis.filter(|s| !s.is_empty()).unwrap_or_else(|| "-".into()),
            class_name: r.class_name.filter(|s| !s.is_empty()).unwrap_or_else(|| "-".into()),
            habis_tanggal: r.habis.map(|d| d.to_string()).unwrap_or_default(),
            hari_lewat: r.habis.map(|d| (hari_ini - d).num_days()).unwrap_or(0),
            belum_pernah,
            punya_hp: r.punya_hp,
            jumlah_ortu: r.jumlah_ortu,
            diingatkan: r.diingatkan.map(fmt_ago).unwrap_or_default(),
        };
        if belum_pernah {
            data.belum_pernah.push(item);
        } else {
            data.terlewat.push(item);
        }
    }
    Ok(data)
}

/// Berapa santri sekaligus yang boleh diingatkan dalam satu tekan.
///
/// Bukan batas teknis WAHA, melainkan batas KEKELIRUAN: satu klik yang mengirim
/// pesan tagihan ke ratusan keluarga tak bisa ditarik kembali, dan angka
/// sebesar ini memaksa pengurus melakukannya bertahap — cukup untuk bekerja,
/// terlalu kecil untuk jadi bencana.
pub const MAX_PENGINGAT_SEKALI: usize = 30;

/// Kirim pengingat WhatsApp ke santri + orang tuanya yang terhubung.
///
/// Best-effort per nomor: satu nomor mati tak boleh membatalkan sisanya, dan
/// hasilnya dilaporkan apa adanya ("12 terkirim, 2 gagal") — bukan "berhasil"
/// yang menyembunyikan bahwa dua keluarga tak pernah menerima apa pun.
///
/// `bill_reminded_at` hanya ditandai untuk santri yang PALING TIDAK satu
/// nomornya berhasil dikirimi; menandai yang gagal akan membuat layar menulis
/// "sudah diingatkan" tentang pesan yang tak pernah sampai.
pub async fn kirim_pengingat(
    http: &reqwest::Client,
    waha: &crate::config::WahaConfig,
    pool: &Pool,
    ids: &[i64],
) -> Result<String> {
    if ids.is_empty() {
        bail_user!("Pilih dulu santri yang mau diingatkan.");
    }
    if ids.len() > MAX_PENGINGAT_SEKALI {
        bail_user!(
            "Maksimal {MAX_PENGINGAT_SEKALI} santri sekali kirim — pilih sebagian dulu."
        );
    }
    let tujuan = repo::tujuan_pengingat(pool, ids).await?;
    let mut terkirim = 0_usize;
    let mut gagal = 0_usize;
    let mut tanpa_nomor: Vec<String> = Vec::new();
    let mut berhasil_ids: Vec<i64> = Vec::new();

    for t in tujuan {
        if t.nomor.is_empty() {
            tanpa_nomor.push(t.student_name);
            continue;
        }
        let pesan = pesan_pengingat(&t.student_name);
        let mut ada_yang_masuk = false;
        for nomor in &t.nomor {
            match super::registration::send_wa_text(http, waha, &wa_phone(nomor), &pesan).await {
                Ok(()) => {
                    terkirim += 1;
                    ada_yang_masuk = true;
                }
                Err(e) => {
                    gagal += 1;
                    tracing::warn!(santri = %t.student_name, "pengingat bayar gagal: {e}");
                }
            }
        }
        if ada_yang_masuk {
            berhasil_ids.push(t.user_id);
        }
    }

    if !berhasil_ids.is_empty() {
        // Gagal menandai tak boleh menggagalkan pengiriman yang sudah terjadi —
        // pesannya sudah di HP orang, dan melaporkan galat di sini akan membuat
        // pengurus menekan tombolnya sekali lagi.
        if let Err(e) = repo::tandai_diingatkan(pool, &berhasil_ids).await {
            tracing::warn!("gagal menandai bill_reminded_at: {e}");
        }
    }

    let mut hasil = format!("{terkirim} pesan terkirim");
    if gagal > 0 {
        hasil.push_str(&format!(", {gagal} gagal"));
    }
    if !tanpa_nomor.is_empty() {
        hasil.push_str(&format!(
            " — tanpa nomor HP: {}",
            ringkas_nama(&tanpa_nomor)
        ));
    }
    hasil.push('.');
    Ok(hasil)
}

/// Isi pesan pengingat. Sengaja pendek, menyebut nama santri, dan TIDAK
/// menyebut nominal: yang menentukan besarannya adalah pengurus, dan angka yang
/// salah di pesan otomatis jauh lebih merepotkan daripada tak ada angka.
fn pesan_pengingat(nama: &str) -> String {
    format!(
        "Assalamu'alaikum wr. wb.\n\nPengingat dari PPM Al-Faqih Mandiri: masa berlaku \
         pembayaran atas nama *{nama}* sudah berakhir.\n\nMohon dapat melanjutkan \
         pembayaran, lalu unggah bukti transfernya di aplikasi AFM SMART (menu \
         Pembayaran). Bila sudah membayar, abaikan pesan ini.\n\nJazaakumullahu khairan."
    )
}

/// "Ahmad, Budi, +3 lainnya" — daftar nama yang tak membuat pesan hasil
/// meledak saat yang tak punya nomor ada belasan.
fn ringkas_nama(nama: &[String]) -> String {
    if nama.len() <= 3 {
        return nama.join(", ");
    }
    format!("{}, +{} lainnya", nama[..3].join(", "), nama.len() - 3)
}

/// Normalisasi HP untuk chat-ID WAHA — satu aturan bersama
/// ([`crate::models::normalisasi_hp`]). Kosong = nomor tak bisa ditafsirkan;
/// pemanggil melewatinya alih-alih mengirim ke alamat yang tak sah.
fn wa_phone(p: &str) -> String {
    crate::models::normalisasi_hp(p).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nomor_dinormalkan_ke_62() {
        assert_eq!(wa_phone("081234567890"), "6281234567890");
        assert_eq!(wa_phone("+62 812-3456-7890"), "6281234567890");
        assert_eq!(wa_phone("6281234567890"), "6281234567890");
    }

    /// Nol berlebih pada isian nominal adalah salah ketik tersering di layar
    /// ini; pagar atasnya harus menangkapnya, bukan meneruskannya ke laporan.
    #[test]
    fn nominal_ditolak_bila_nol_atau_kelewat_besar() {
        assert!(periksa_nominal(0).is_err());
        assert!(periksa_nominal(-1).is_err());
        assert!(periksa_nominal(MAX_NOMINAL + 1).is_err());
        assert!(periksa_nominal(500_000).is_ok());
        assert!(periksa_nominal(MAX_NOMINAL).is_ok());
    }

    #[test]
    fn tanggal_wajib_format_iso() {
        assert!(urai_tanggal("2026-08-09", "x").is_ok());
        assert!(urai_tanggal("09/08/2026", "x").is_err());
        assert!(urai_tanggal("", "x").is_err());
    }

    #[test]
    fn ringkasan_nama_dipotong_setelah_tiga() {
        let n = |s: &str| s.to_string();
        assert_eq!(ringkas_nama(&[n("A"), n("B")]), "A, B");
        assert_eq!(ringkas_nama(&[n("A"), n("B"), n("C")]), "A, B, C");
        assert_eq!(ringkas_nama(&[n("A"), n("B"), n("C"), n("D"), n("E")]), "A, B, C, +2 lainnya");
    }
}
