//! web/app.rs — Shell HTML (SSR) + komponen root `App` + tabel route.
//!
//! Design system "Islamic Institutional" (emerald + Work Sans, Material 3)
//! disuntik via Tailwind Play CDN + config token di `<head>` shell. Untuk
//! produksi disarankan mengompilasi Tailwind (CLI) — lihat catatan di README.

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Title};
use leptos_router::{
    components::{FlatRoutes, Route, Router},
    path,
};

use crate::web::pages::*;

/// Konfigurasi Tailwind (token warna + tipografi PPM AFM). Disuntik sbg <script>.
const TAILWIND_CONFIG: &str = r##"
tailwind.config = {
  darkMode: "class",
  theme: { extend: {
    colors: {
      surface:"#f9f9ff","surface-dim":"#d3daea","surface-bright":"#f9f9ff",
      "surface-container-lowest":"#ffffff","surface-container-low":"#f0f3ff",
      "surface-container":"#e7eefe","surface-container-high":"#e2e8f8",
      "surface-container-highest":"#dce2f3","on-surface":"#151c27",
      "on-surface-variant":"#404944","inverse-surface":"#2a313d",
      "inverse-on-surface":"#ebf1ff",outline:"#707974","outline-variant":"#bfc9c3",
      "surface-tint":"#2b6954",primary:"#003527","on-primary":"#ffffff",
      "primary-container":"#064e3b","on-primary-container":"#80bea6",
      "inverse-primary":"#95d3ba",secondary:"#416656","on-secondary":"#ffffff",
      "secondary-container":"#c3ecd7","on-secondary-container":"#476c5b",
      tertiary:"#2c2f30","on-tertiary":"#ffffff","tertiary-container":"#424546",
      "on-tertiary-container":"#b0b2b3",error:"#ba1a1a","on-error":"#ffffff",
      "error-container":"#ffdad6","on-error-container":"#93000a",
      "primary-fixed":"#b0f0d6","primary-fixed-dim":"#95d3ba",
      "on-primary-fixed":"#002117","on-primary-fixed-variant":"#0b513d",
      "secondary-fixed":"#c3ecd7","secondary-fixed-dim":"#a8cfbc",
      "on-secondary-fixed":"#002115","on-secondary-fixed-variant":"#294e3f",
      "tertiary-fixed":"#e1e3e4","tertiary-fixed-dim":"#c5c7c8",
      "on-tertiary-fixed":"#191c1d","on-tertiary-fixed-variant":"#454748",
      background:"#f9f9ff","on-background":"#151c27","surface-variant":"#dce2f3",
      "success":"#059669","warning":"#f59e0b","info":"#2563eb"
    },
    borderRadius:{DEFAULT:"0.25rem",lg:"0.5rem",xl:"0.75rem","2xl":"1rem",full:"9999px"},
    fontFamily:{sans:["Work Sans","system-ui","sans-serif"],
      "display-lg":["Work Sans"],"display-md":["Work Sans"],"headline-sm":["Work Sans"],
      "body-lg":["Work Sans"],"body-md":["Work Sans"],"body-sm":["Work Sans"],"label-md":["Work Sans"]},
    fontSize:{
      "display-lg":["32px",{lineHeight:"40px",letterSpacing:"-0.02em",fontWeight:"700"}],
      "display-md":["24px",{lineHeight:"32px",letterSpacing:"-0.01em",fontWeight:"600"}],
      "headline-sm":["20px",{lineHeight:"28px",fontWeight:"600"}],
      "body-lg":["18px",{lineHeight:"28px",fontWeight:"400"}],
      "body-md":["16px",{lineHeight:"24px",fontWeight:"400"}],
      "body-sm":["14px",{lineHeight:"20px",fontWeight:"400"}],
      "label-md":["12px",{lineHeight:"16px",letterSpacing:"0.05em",fontWeight:"600"}]
    }
  }}
};
"##;

/// CSS kustom kecil (kelas non-utility yang dipakai desain).
const CUSTOM_STYLE: &str = r##"
body{font-family:'Work Sans',sans-serif;background-color:#f9f9ff;}
.spiritual-gradient{background:linear-gradient(135deg,#003527 0%,#064e3b 100%);}
.pattern-bg{background-image:url('https://www.transparenttextures.com/patterns/cubes.png');opacity:.03;}
.input-focus-ring:focus{box-shadow:0 0 0 4px rgba(0,53,39,.1);}
.material-symbols-outlined{font-variation-settings:'FILL' 0,'wght' 400,'GRAD' 0,'opsz' 24;}

/* ── Interaktivitas global ──────────────────────────────────────────────── */
button,a{-webkit-tap-highlight-color:transparent}
button{transition:transform .15s ease,background-color .15s ease,border-color .15s ease,
  box-shadow .15s ease,opacity .15s ease}
button:active{transform:scale(.96)}
.press{transition:transform .15s ease,box-shadow .15s ease}
.press:active{transform:scale(.97)}
.card-hover{transition:transform .18s ease,box-shadow .18s ease,border-color .18s ease}
.card-hover:hover{transform:translateY(-2px);box-shadow:0 10px 26px rgba(0,53,39,.10)}

/* Animasi masuk + berjenjang (stagger) untuk anak-anak sebuah container. */
@keyframes pp-in{from{opacity:0;transform:translateY(14px)}to{opacity:1;transform:none}}
.anim-in{animation:pp-in .5s cubic-bezier(.2,.7,.2,1) both}
.stagger>*{animation:pp-in .5s cubic-bezier(.2,.7,.2,1) both}
.stagger>*:nth-child(1){animation-delay:.04s}.stagger>*:nth-child(2){animation-delay:.09s}
.stagger>*:nth-child(3){animation-delay:.14s}.stagger>*:nth-child(4){animation-delay:.19s}
.stagger>*:nth-child(5){animation-delay:.24s}.stagger>*:nth-child(6){animation-delay:.29s}
.stagger>*:nth-child(7){animation-delay:.34s}.stagger>*:nth-child(8){animation-delay:.39s}
.stagger>*:nth-child(9){animation-delay:.44s}.stagger>*:nth-child(n+10){animation-delay:.5s}

/* Bar progres tumbuh dari kiri. */
@keyframes pp-bar{from{transform:scaleX(0)}to{transform:scaleX(1)}}
.bar-grow{transform-origin:left;animation:pp-bar 1s .25s cubic-bezier(.2,.7,.2,1) both}

/* Titik hidup (live/menunggu) berdenyut. */
@keyframes pp-pulse{0%,100%{opacity:1}50%{opacity:.35}}
.pulse-dot{animation:pp-pulse 1.6s ease-in-out infinite}

/* Bottom-sheet naik (modal QR dsb). */
@keyframes pp-sheet{from{transform:translateY(100%)}to{transform:none}}
.sheet-in{animation:pp-sheet .3s cubic-bezier(.2,.8,.2,1) both}
@keyframes pp-fade{from{opacity:0}to{opacity:1}}
.fade-in{animation:pp-fade .25s ease both}

/* Scroll-reveal (elemen [data-reveal]) — disembunyikan HANYA saat JS aktif. */
.reveal-js [data-reveal]{opacity:0;transform:translateY(20px);
  transition:opacity .65s cubic-bezier(.22,.61,.24,1),transform .65s cubic-bezier(.22,.61,.24,1)}
.reveal-js [data-reveal].is-visible{opacity:1;transform:none}

@media (prefers-reduced-motion:reduce){
  .anim-in,.stagger>*,.bar-grow,.pulse-dot,.sheet-in,.fade-in{animation:none}
  .reveal-js [data-reveal]{opacity:1;transform:none;transition:none}
}

/* Halaman mobile: DESKTOP = SAMA seperti MOBILE — kolom ponsel (max-w-md di
   markup halaman) terpusat, SCROLL DI WINDOW (bukan container ber-transform!).
   PENTING: jangan beri transform/overflow pada kolom — itu mengubah containing
   block position:fixed → bottom-nav ikut ter-scroll ke tengah konten (bug).
   Dengan window-scroll: nav `fixed bottom-0 inset-x-0 max-w-md mx-auto` otomatis
   terkunci di dasar viewport & sejajar kolom. FAB diselaraskan via .ppm-fab. */
.ppm-stage{min-height:100vh;}
@media (min-width:768px){
  .ppm-stage{background:#e9edf8;}
  .ppm-stage>div{
    min-height:100vh;
    background:#f9f9ff;
    border-inline:1px solid rgba(112,121,116,.18);
    box-shadow:0 0 44px rgba(21,28,39,.10);
  }
  /* FAB: di mobile right-5 (tepi layar); di desktop rapat ke tepi KOLOM
     (kolom max-w-md = 28rem → tepi kanan kolom = 50% - 14rem). */
  .ppm-fab{right:calc(50% - 14rem + 1.25rem) !important;}
}
"##;

/// Shell HTML — dipanggil Axum untuk tiap SSR request.
pub fn shell(options: leptos::config::LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="id" class="light">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <meta name="theme-color" content="#003527" />

                // ── Tailwind (Play CDN) + token desain PPM AFM ──────────────
                <script src="https://cdn.tailwindcss.com?plugins=forms,container-queries"></script>
                <script inner_html=TAILWIND_CONFIG></script>
                <style inner_html=CUSTOM_STYLE></style>

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
  function start(){
    scan();
    try{ new MutationObserver(function(){ scan(); })
      .observe(document.body,{childList:true,subtree:true}); }catch(e){}
    /* Pengaman: tampilkan semua reveal setelah 5 dtk apa pun yang terjadi. */
    setTimeout(function(){
      document.querySelectorAll('[data-reveal]').forEach(function(el){el.classList.add('is-visible');});
    },5000);
  }
  if(document.readyState==='loading'){ document.addEventListener('DOMContentLoaded',start); }
  else { start(); }
})();
"#></script>

                // ── Fonts: Work Sans + Material Symbols ─────────────────────
                <link rel="preconnect" href="https://fonts.googleapis.com" />
                <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="" />
                <link
                    href="https://fonts.googleapis.com/css2?family=Work+Sans:wght@300;400;500;600;700&display=swap"
                    rel="stylesheet"
                />
                <link
                    href="https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:opsz,wght,FILL,GRAD@20..48,100..700,0..1,-50..200&display=swap"
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

    view! {
        <Title text="PPM AFM — Portal Absensi Santri" />
        <Router>
            <FlatRoutes fallback=|| view! { <NotFoundPage /> }>
                // Publik: beranda profil pesantren untuk pengunjung.
                <Route path=path!("/") view=BerandaPage />
                <Route path=path!("/login") view=LoginPage />
                <Route path=path!("/menu") view=MenuPage />

                // Santri (dinamis — data DB)
                <Route path=path!("/santri") view=SantriDashboardPage />
                <Route path=path!("/izin") view=IzinPage />
                <Route path=path!("/riwayat") view=RiwayatPage />
                <Route path=path!("/sesi") view=SesiPage />
                <Route path=path!("/profil") view=ProfilPage />

                // Staf / Guru / Dewan Guru
                <Route path=path!("/staf") view=StafDashboardPage />
                <Route path=path!("/guru") view=GuruDashboardPage />
                <Route path=path!("/dewan-guru") view=DewanGuruDashboardPage />
                <Route path=path!("/poin") view=PoinPage />
                <Route path=path!("/poin-dewan") view=PoinDewanPage />
                <Route path=path!("/verifikasi-pamong") view=VerifikasiPamongPage />
                <Route path=path!("/verifikasi-tahap-2") view=VerifikasiTahap2Page />

                // Halaqah
                <Route path=path!("/halaqah") view=HalaqahDaftarPage />
                <Route path=path!("/halaqah/mulai") view=HalaqahMulaiPage />
                <Route path=path!("/halaqah/live") view=HalaqahLivePage />
                <Route path=path!("/rekaman") view=RekamanPage />

                // Orang tua (dinamis — data DB, koneksi butuh approval santri)
                <Route path=path!("/orang-tua") view=OrtuBerandaPage />
                <Route path=path!("/orang-tua/izin") view=OrtuIzinPage />
                <Route path=path!("/orang-tua/riwayat") view=OrtuRiwayatPage />
                <Route path=path!("/koneksi-ortu") view=KoneksiOrtuPage />
            </FlatRoutes>
        </Router>
    }
}
