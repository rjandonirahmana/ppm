//! web/app.rs — Shell HTML (SSR) + komponen root `App` + tabel route.
//!
//! Design system "Islamic Institutional" (emerald + Work Sans, Material 3).
//! Tailwind di-SELF-HOST: di-compile cargo-leptos dari `style/tailwind.css`
//! (+ `tailwind.config.js`) → `/pkg/ppm.css` (statis), di-`<link>` di shell.
//! Bukan lagi Play CDN (tak ada compile Tailwind di browser).

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, HashedStylesheet, MetaTags, Title};
use leptos_router::{
    components::{FlatRoutes, Route, Router},
    path,
};

use crate::web::api::get_session;
use crate::web::components::{BottomNav, DesktopSidebar};
use crate::web::pages::*;


/// Shell HTML — dipanggil Axum untuk tiap SSR request.
pub fn shell(options: leptos::config::LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="id" class="light">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <meta name="theme-color" content="#003527" />

                // ── Identitas: ikon & manifest ────────────────────────────
                // PNG, bukan .ico: seluruh browser yang dipakai santri (Chrome
                // & Safari mobile) membacanya, dan satu berkas sumber lebih
                // mudah dijaga tetap sama dengan logo aslinya.
                // Ikon dibuat BUJUR SANGKAR berlatar putih — logo aslinya lebih
                // tinggi daripada lebar, dan ikon non-persegi dipotong
                // sembarang oleh peluncur aplikasi.
                <link rel="icon" r#type="image/png" sizes="32x32" href="/icons/favicon-32.png" />
                <link rel="icon" r#type="image/png" sizes="192x192" href="/icons/icon-192.png" />
                <link rel="apple-touch-icon" href="/icons/apple-touch-icon.png" />
                <link rel="manifest" href="/manifest.webmanifest" />
                <meta name="apple-mobile-web-app-title" content="AFM SMART" />

                // ── CSS: Tailwind self-host (di-compile cargo-leptos → statis) ──
                // Ganti Play CDN: tanpa compile Tailwind di browser. Tema & CSS
                // kustom ada di style/tailwind.css + tailwind.config.js.
                // HashedStylesheet (bukan <link href="/pkg/ppm.css"> statis) —
                // hash-files = FALSE (Cargo.toml, Jul 2026) → nama file /pkg TETAP.
                // HashedStylesheet tetap dipakai karena aman di kedua mode: saat
                // hash mati ia menghasilkan tautan biasa. KONSEKUENSI mode ini:
                // nama file tak berubah antar rilis, jadi browser bisa memakai
                // CSS/JS lama dari cache — invalidasi diurus di layer proxy/CDN.
                <HashedStylesheet options=options.clone() id="leptos" />

                // ── Interaktivitas: scroll-reveal + count-up angka ───────────
                // 1) Tandai <html> reveal-js SINKRON → [data-reveal] hanya
                //    disembunyikan saat JS aktif (tanpa JS konten tetap tampil).
                <script inner_html="document.documentElement.classList.add('reveal-js');"></script>
                // 2) IntersectionObserver utk [data-reveal]; count-up utk
                //    [data-count] (angka naik 0→target saat terlihat).
                //    MutationObserver menangkap konten Suspense/SPA. Guard
                //    data-counted mencegah loop (mutasi textContent sendiri).
                <script inner_html=r#"
(function(){
  if(window.__ppmFx) return; window.__ppmFx=true;
  /* bfcache: halaman yang dipulihkan tombol Back masih memegang state sesi
     LAMA (mis. setelah logout) → muat ulang agar cookie dicek kembali.
     Tanpa ini logout tampak "gagal" saat user menekan Back. */
  window.addEventListener('pageshow',function(e){ if(e.persisted) location.reload(); });
  var reduce=window.matchMedia&&window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  function countUp(el){
    if(el.getAttribute('data-counted')==='1') return;
    el.setAttribute('data-counted','1');
    var target=parseInt(el.getAttribute('data-count'),10);
    if(isNaN(target)||reduce){ return; }
    var dur=700, t0=null;
    function step(t){
      if(!t0) t0=t;
      var p=Math.min((t-t0)/dur,1);
      p=1-Math.pow(1-p,3); /* ease-out */
      el.textContent=Math.round(target*p);
      if(p<1) requestAnimationFrame(step);
    }
    requestAnimationFrame(step);
  }
  var io=('IntersectionObserver' in window)?new IntersectionObserver(function(es){
    es.forEach(function(e){
      if(!e.isIntersecting) return;
      if(e.target.hasAttribute('data-reveal')) e.target.classList.add('is-visible');
      if(e.target.hasAttribute('data-count')) countUp(e.target);
      io.unobserve(e.target);
    });
  },{threshold:0.15}):null;
  function scan(){
    document.querySelectorAll('[data-reveal]:not([data-fx]),[data-count]:not([data-fx])')
      .forEach(function(el){
        el.setAttribute('data-fx','1');
        if(io){ io.observe(el); }
        else { el.classList.add('is-visible'); if(el.hasAttribute('data-count')) countUp(el); }
      });
  }
  /* Throttle scan ke SATU kali per frame (rAF): navigasi SPA Leptos memicu
     BANYAK mutasi DOM beruntun → tanpa throttle, scan() (querySelectorAll
     seluruh dokumen) jalan berkali-kali = jank saat pindah halaman. */
  var scanQueued=false;
  function queueScan(){
    if(scanQueued) return; scanQueued=true;
    requestAnimationFrame(function(){ scanQueued=false; scan(); });
  }
  function start(){
    scan();
    try{ new MutationObserver(queueScan)
      .observe(document.body,{childList:true,subtree:true}); }catch(e){}
    /* Pengaman: tampilkan semua reveal setelah 5 dtk apa pun yang terjadi. */
    setTimeout(function(){
      document.querySelectorAll('[data-reveal]').forEach(function(el){el.classList.add('is-visible');});
    },5000);
  }
  /* JANGAN memutasi DOM sebelum HYDRATION Leptos selesai: count-up menimpa
     textContent [data-count] & reveal menambah class — kalau menyentuh node
     yang sedang dihidrasi, hydration mismatch → event delegation tak terpasang
     → klik mati / section tersembunyi = blank (parah di Safari private karena
     WASM dimuat fresh/lebih lambat). Rust memanggil __ppmStartFx() SETELAH
     hydrate. Fallback: bila tak pernah dipanggil (hydrate absen/gagal), jalan
     sendiri setelah load — di kasus itu tak ada hydration yang bisa dirusak. */
  window.__ppmStartFx=function(){ if(window.__ppmFxStarted) return; window.__ppmFxStarted=true; start(); };
  window.addEventListener('load',function(){ setTimeout(window.__ppmStartFx,1000); });
})();
"#></script>

                // ── Ikon Material Symbols: SELF-HOST lokal (public/fonts, ~28KB) ──
                // Tak lagi dari Google CDN. Preload agar diunduh dini (dipakai di
                // hampir semua halaman). @font-face ada di /pkg/ppm.css.
                <link
                    rel="preload"
                    href="/fonts/material-symbols.woff2"
                    attr:as="font"
                    r#type="font/woff2"
                    crossorigin=""
                />
                // ── Work Sans (teks) masih dari CDN (degradasi mulus ke system font).
                <link rel="preconnect" href="https://fonts.googleapis.com" />
                <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="" />
                <link
                    href="https://fonts.googleapis.com/css2?family=Work+Sans:wght@300;400;500;600;700&display=swap"
                    rel="stylesheet"
                />

                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <MetaTags />
            </head>
            <body class="bg-surface text-on-surface">
                <App />
            </body>
        </html>
    }
}

/// Komponen root — universal SSR + hydration.
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    // Sesi GLOBAL (role) — dibaca oleh <BottomNav/> yang persisten. Blocking agar
    // role sudah ada di HTML SSR (navbar benar sejak awal, tanpa kedip). Di-cache
    // (key `()`), jadi tak refetch saat pindah halaman → navbar tak berubah.
    let session = Resource::new_blocking(|| (), |_| async move { get_session().await.ok().flatten() });
    provide_context(session);

    // Jejak halaman yang sudah dilalui, untuk tombol kembali (lihat
    // `components::RiwayatNav`). Disediakan DI SINI — konteks hanya menurun ke
    // anak, jadi memasangnya di dalam <Router> tak akan terlihat halaman.
    crate::web::components::sediakan_riwayat_nav();

    // Nyalakan efek DOM (count-up + reveal) HANYA setelah hydration selesai —
    // Effect cuma jalan di klien, pasca-mount. Ini mencegah skrip inline
    // memutasi node yang sedang dihidrasi (hydration mismatch → klik mati /
    // blank, terutama Safari private). Konten SPA baru ditangani MutationObserver.
    #[cfg(target_arch = "wasm32")]
    Effect::new(|_| {
        use wasm_bindgen::JsCast;
        if let Some(win) = web_sys::window() {
            if let Ok(f) =
                js_sys::Reflect::get(&win, &wasm_bindgen::JsValue::from_str("__ppmStartFx"))
            {
                if let Ok(func) = f.dyn_into::<js_sys::Function>() {
                    let _ = func.call0(&win);
                }
            }
        }
    });

    view! {
        <Title text="AFM SMART — Portal Absensi Santri" />
        <Router>
            // Pencatat perpindahan halaman — dibaca tombol kembali di header
            // supaya ia benar-benar mundur ke halaman SEBELUMNYA, bukan ke
            // alamat tetap yang ditulis tiap halaman. Harus DI DALAM <Router>:
            // `use_location()` baru ada setelah router terpasang.
            <PelacakNav />
            <PreviewMode />
            <FlatRoutes fallback=|| view! { <NotFoundPage /> }>
                // Publik: beranda profil pesantren untuk pengunjung.
                <Route path=path!("/") view=BerandaPage />
                // Artikel PUBLIK (migrasi 69) — daftar & satu tulisan.
                <Route path=path!("/artikel") view=ArtikelListPage />
                <Route path=path!("/artikel/:slug") view=ArtikelDetailPage />
                <Route path=path!("/login") view=LoginPage />
                <Route path=path!("/register") view=RegisterPage />
                <Route path=path!("/tamu") view=TamuPage />
                <Route path=path!("/menu") view=MenuPage />

                // Santri (dinamis — data DB)
                <Route path=path!("/santri") view=SantriDashboardPage />
                <Route path=path!("/kelas-saya") view=KelasSayaPage />
                <Route path=path!("/izin") view=IzinPage />
                <Route path=path!("/riwayat") view=RiwayatPage />
                <Route path=path!("/sesi") view=SesiPage />
                <Route path=path!("/sesi/:id") view=SesiDetailPage />
                <Route path=path!("/sesi/:id/live") view=SesiLivePage />
                <Route path=path!("/poin-saya") view=PoinSayaPage />
                <Route path=path!("/profil") view=ProfilPage />
                <Route path=path!("/ganti-sandi") view=GantiSandiPage />
                <Route path=path!("/laporan") view=LaporanPage />
                <Route path=path!("/akademik") view=AkademikSantriPage />
                <Route path=path!("/kalender") view=KalenderPage />

                // Staf / Guru / Dewan Guru
                <Route path=path!("/staf") view=StafDashboardPage />
                <Route path=path!("/guru") view=GuruDashboardPage />
                <Route path=path!("/dewan-guru") view=DewanGuruDashboardPage />
                <Route path=path!("/poin") view=PoinPage />
                <Route path=path!("/poin/:id") view=PoinDetailPage />
                <Route path=path!("/verifikasi-pamong") view=VerifikasiPamongPage />
                <Route path=path!("/verifikasi-tahap-2") view=VerifikasiTahap2Page />
                <Route path=path!("/izin-staf") view=IzinStafPage />
                <Route path=path!("/materi") view=MateriPage />
                <Route path=path!("/galeri") view=GaleriPage />
                <Route path=path!("/kelola-artikel") view=KelolaArtikelPage />
                <Route path=path!("/tagihan") view=FinancePage />
                <Route path=path!("/tagihan-saya") view=MyBillsPage />
                <Route path=path!("/kontrol-pengguna") view=KontrolPenggunaPage />
                <Route path=path!("/manajemen-user") view=ManajemenUserPage />
                <Route path=path!("/status-server") view=StatusServerPage />
                <Route path=path!("/rekap-mingguan") view=RekapMingguanPage />
                <Route path=path!("/students") view=StudentsPage />
                <Route path=path!("/kelas") view=KelasPage />
                <Route path=path!("/kelas/:id") view=KelasDetailPage />

                // Halaqah
                <Route path=path!("/halaqah") view=HalaqahDaftarPage />
                <Route path=path!("/halaqah/mulai") view=HalaqahMulaiPage />
                <Route path=path!("/halaqah/live") view=HalaqahLivePage />
                <Route path=path!("/rekaman") view=RekamanPage />

                // Orang tua (dinamis — data DB, koneksi butuh approval santri)
                <Route path=path!("/orang-tua") view=OrtuBerandaPage />
                <Route path=path!("/orang-tua/izin") view=OrtuIzinPage />
                <Route path=path!("/orang-tua/riwayat") view=OrtuRiwayatPage />
                <Route path=path!("/orang-tua/pembayaran") view=OrtuPembayaranPage />
                <Route path=path!("/koneksi-ortu") view=KoneksiOrtuPage />

                // Penjaga gerbang (migrasi 83): memeriksa kecocokan data tamu.
                <Route path=path!("/tamu-masuk") view=TamuMasukPage />
            </FlatRoutes>
            // Navbar bawah PERSISTEN — di luar <FlatRoutes> agar tak ikut
            // ter-swap/shimmer saat pindah halaman (hanya konten yang shimmer).
            <BottomNav />
            <DesktopSidebar />
        </Router>
    }
}

/// Mencatat perpindahan halaman (lihat `components::RiwayatNav`).
/// Komponen kosong — ia hanya perlu tempat di dalam `<Router>` untuk hidup.
#[component]
fn PelacakNav() -> impl IntoView {
    crate::web::components::lacak_perpindahan();
    view! {}
}

/// Pratinjau tampilan PONSEL di layar desktop — dinyalakan `?preview=mobile`.
///
/// Sejak mode sidebar desktop ada, layout ponsel hanya muncul di bawah 768px.
/// Mengecilkan jendela peramban bukan pengganti yang setia: `pointer: fine`
/// tetap berlaku (jadi aturan yang membedakan sentuh vs tetikus tak ikut
/// berubah) dan tak ada emulasi sentuh sama sekali.
///
/// Komponen ini hanya MENYETEL satu atribut di `<body>`; seluruh perubahan
/// tampilannya ada di CSS (`body[data-preview="mobile"]` di tailwind.css).
/// Tanpa query param, tak ada satu pun aturan itu yang aktif — jadi aman ikut
/// terpasang di produksi, dan berguna untuk memeriksa tampilan ponsel pada
/// server sungguhan, bukan cuma di mesin pengembang.
///
/// Atributnya ditulis lewat Effect (hanya jalan di klien): menyentuh `<body>`
/// saat render server tak ada gunanya — HTML-nya sudah terkirim — dan membaca
/// URL saat SSR akan berbeda dari hasil hidrasi.
#[component]
fn PreviewMode() -> impl IntoView {
    let query = leptos_router::hooks::use_query_map();
    Effect::new(move |_| {
        let mobile = query.with(|q| q.get("preview").as_deref() == Some("mobile"));
        #[cfg(target_arch = "wasm32")]
        if let Some(body) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.body())
        {
            if mobile {
                let _ = body.set_attribute("data-preview", "mobile");
            } else {
                let _ = body.remove_attribute("data-preview");
            }
        }
        let _ = mobile;
    });
    view! {}
}
