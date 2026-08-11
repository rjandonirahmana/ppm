//! web/pages/beranda.rs — Beranda PUBLIK untuk pengunjung (profil PPM AFM).
//!
//! Konten dari situs resmi (afm-website-gilt.vercel.app): video kepala halaman,
//! tentang, kegiatan harian, statistik, struktur kepengurusan, fasilitas,
//! artikel, kampus sekitar, lokasi & kontak. Desain memakai design system
//! portal (emerald + Work Sans) agar senada.
//!
//! Yang DINAMIS (dikelola dari dalam portal, bukan ditulis di kode ini):
//!   • video/foto kepala halaman + foto kegiatan + foto fasilitas → /galeri
//!     (kategori `video_utama` / `kegiatan` / `fasilitas`, migrasi 69)
//!   • artikel → /kelola-artikel (admin)
//! Sisanya profil pondok yang jarang berubah, jadi tetap di kode.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{role_home, ActivityPhoto, Article, MediaCategory, SessionUser};
use crate::web::components::{MediaFrame, PhotoFrame};

/// Berapa artikel terbaru yang muncul di halaman depan. Sisanya di /artikel.
const ARTIKEL_DI_BERANDA: i64 = 3;

#[component]
pub fn BerandaPage() -> impl IntoView {
    // Galeri (migrasi 34 & 69) — publik. Satu bacaan untuk SELURUH halaman,
    // lalu dipilah per kategori di bawah: tiga resource terpisah berarti tiga
    // request untuk tabel berisi puluhan baris, dan tiga saat berbeda di mana
    // bagian halaman muncul.
    // BLOCKING, tak seperti `artikel` di bawahnya. Kepala halaman adalah hal
    // PERTAMA yang terlihat: dengan resource biasa, HTML pertama yang dikirim
    // berisi `HeroPolos` dan video baru menyusul — pengunjung melihat hero
    // polos berkedip lalu berganti video, dan perayap yang tak menjalankan JS
    // tak pernah melihat videonya sama sekali. Tabelnya puluhan baris dengan
    // index, jadi menunggunya murah. Kegagalan DB tetap turun anggun ke
    // `HeroPolos` — yang ditambahkan hanya jeda, bukan titik gagal baru.
    let gallery = Resource::new_blocking(
        || (),
        |_| async move { crate::web::api::activity_photos_data().await },
    );
    let artikel = Resource::new(
        || (),
        |_| async move { crate::web::api::articles_data(ARTIKEL_DI_BERANDA).await },
    );

    // Media satu kategori, terurut seperti yang disusun pengelola.
    let of_cat = move |cat: MediaCategory| -> Vec<ActivityPhoto> {
        gallery
            .get()
            .and_then(|r| r.ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.category() == cat)
            .collect()
    };

    view! {
        <Title text="PPM Al-Faqih Mandiri — Pondok Pesantren Mahasiswa Depok" />
        <div class="min-h-screen bg-surface text-on-surface">

            <PublicNav home=true />

            // ── Hero: video berjalan (atau hero polos bila belum ada) ────────
            <Suspense fallback=|| view! { <HeroPolos /> }>
                {move || {
                    match of_cat(MediaCategory::VideoUtama).into_iter().next() {
                        // Yang PALING ATAS yang dipakai; sisanya cadangan yang
                        // tinggal digeser di halaman kelola.
                        Some(m) => view! { <HeroMedia m=m /> }.into_any(),
                        None => view! { <HeroPolos /> }.into_any(),
                    }
                }}
            </Suspense>

            // ── Tentang ─────────────────────────────────────────────────────
            <section id="tentang" class="max-w-6xl mx-auto px-5 py-16 md:py-20" data-reveal="1">
                <div class="grid md:grid-cols-2 gap-10 items-center">
                    <div>
                        <p class="text-label-md text-primary uppercase tracking-[0.25em]">"Tentang PPM AFM"</p>
                        <h2 class="text-display-md text-on-background mt-3">
                            "Membina Mahasiswa Menjadi Profesional Religius"
                        </h2>
                        <p class="text-body-md text-on-surface-variant mt-4 leading-relaxed">
                            "PPM Al-Faqih Mandiri adalah pondok pesantren mahasiswa di Depok dan sekitarnya. "
                            "Santri dibina agar lulus sebagai sarjana yang profesional religius serta aktif "
                            "berkontribusi dalam dakwah dan masyarakat."
                        </p>
                        <ul class="mt-6 space-y-3">
                            {[
                                "Pengajian Al-Qur'an dan Al-Hadits bersama dewan guru",
                                "Pembinaan akhlak dan kemandirian sehari-hari",
                                "Lingkungan asrama yang tertib, bersih, dan kekeluargaan",
                            ]
                                .into_iter()
                                .map(|t| {
                                    view! {
                                        <li class="flex items-start gap-3">
                                            <span class="material-symbols-outlined text-primary mt-0.5">
                                                "check_circle"
                                            </span>
                                            <span class="text-body-md text-on-surface">{t}</span>
                                        </li>
                                    }
                                })
                                .collect_view()}
                        </ul>
                    </div>
                    <Suspense fallback=|| view! { <FotoPlaceholder /> }>
                        {move || {
                            let photos = of_cat(MediaCategory::Kegiatan);
                            if photos.is_empty() {
                                view! { <FotoPlaceholder /> }.into_any()
                            } else {
                                view! {
                                    // Dek yang DIGESER, bukan petak 2 kolom yang
                                    // memotong sisanya. Dulu `.take(6)`: foto
                                    // ketujuh dan seterusnya hilang tanpa jejak,
                                    // padahal galerinya terus bertambah — dan
                                    // tak ada apa pun di layar yang memberi tahu
                                    // masih ada yang lain.
                                    //
                                    // `ppm-swipe` sudah menangani semuanya
                                    // secara native: geser jari di ponsel, dua
                                    // jari di trackpad, Shift+roda di desktop,
                                    // berhenti pas di tiap kartu. `-fade`
                                    // memudarkan tepi kanan sebagai penanda
                                    // kedua bahwa deknya masih berlanjut.
                                    <div class="ppm-swipe ppm-swipe-besar ppm-swipe-fade">
                                        {photos
                                            .into_iter()
                                            .take(12)
                                            .map(|p| view! { <KartuFoto p=p /> })
                                            .collect_view()}
                                    </div>
                                }
                                    .into_any()
                            }
                        }}
                    </Suspense>
                </div>
            </section>

            // ── Kegiatan harian ─────────────────────────────────────────────
            <section id="kegiatan" class="bg-surface-container-low border-y border-outline-variant/40">
                <div class="max-w-6xl mx-auto px-5 py-16 md:py-20">
                    <div class="text-center mb-12">
                        <p class="text-label-md text-primary uppercase tracking-[0.25em]">"Kegiatan Santri"</p>
                        <h2 class="text-display-md text-on-background mt-3">"Rutinitas Pembinaan"</h2>
                    </div>
                    <div class="grid sm:grid-cols-2 lg:grid-cols-3 gap-5 stagger">
                        <KegiatanCard
                            icon="auto_stories"
                            title="Sesi Al-Qur'an & Hadits"
                            desc="Pengajian bersama dewan guru setiap setelah Subuh dan setelah Isya — dua kali sehari."
                        />
                        <KegiatanCard
                            icon="dark_mode"
                            title="Sholat Tahajud"
                            desc="Pembiasaan qiyamul lail berjamaah untuk membangun kedekatan dengan Allah."
                        />
                        <KegiatanCard
                            icon="campaign"
                            title="Ceramah & Nasihat"
                            desc="Ceramah agama dan pembinaan karakter secara rutin dari dewan guru."
                        />
                        <KegiatanCard
                            icon="cleaning_services"
                            title="Piket Kebersihan"
                            desc="Jadwal piket harian menjaga kebersihan asrama — melatih tanggung jawab."
                        />
                        <KegiatanCard
                            icon="sports_martial_arts"
                            title="Pencak Silat ASAD"
                            desc="Latihan bela diri persinas ASAD untuk kebugaran dan kedisiplinan."
                        />
                        <KegiatanCard
                            icon="workspace_premium"
                            title="Pembinaan Karakter"
                            desc="29 karakter luhur sebagai bekal menjadi insan profesional religius."
                        />
                    </div>
                </div>
            </section>

            // ── Statistik ───────────────────────────────────────────────────
            <section id="statistik" class="max-w-6xl mx-auto px-5 py-16 md:py-20" data-reveal="1">
                <div class="spiritual-gradient rounded-3xl p-8 md:p-14 text-on-primary relative overflow-hidden">
                    <span class="material-symbols-outlined absolute -right-8 -bottom-8 text-[220px] opacity-10">
                        "mosque"
                    </span>
                    <div class="relative">
                        <p class="text-label-md uppercase tracking-[0.25em] text-primary-fixed">
                            "PPM AFM dalam Angka"
                        </p>
                        <div class="grid grid-cols-2 md:grid-cols-4 gap-8 mt-8">
                            <StatItem num="92" label="Total Santri" />
                            <StatItem num="44" label="Santri Putra" />
                            <StatItem num="48" label="Santri Putri" />
                            <StatItem num="5" label="Dewan Guru" />
                        </div>
                        <p class="text-body-sm opacity-70 mt-8">"Data per Januari 2025"</p>
                    </div>
                </div>
            </section>

            <StrukturSection />

            <FasilitasSection fotos=Signal::derive(move || of_cat(MediaCategory::Fasilitas)) />

            // ── Artikel ─────────────────────────────────────────────────────
            <section id="artikel" class="max-w-6xl mx-auto px-5 py-16 md:py-20" data-reveal="1">
                <div class="flex flex-wrap items-end justify-between gap-4 mb-10">
                    <div>
                        <p class="text-label-md text-primary uppercase tracking-[0.25em]">"Artikel AFM"</p>
                        <h2 class="text-display-md text-on-background mt-3">"Kabar & Tulisan Terbaru"</h2>
                    </div>
                    <a
                        href="/artikel"
                        class="flex items-center gap-2 px-5 py-2.5 border border-outline-variant rounded-xl text-body-sm font-semibold text-on-surface hover:border-primary hover:text-primary transition-colors"
                    >
                        "Lihat Semua"
                        <span class="material-symbols-outlined text-lg">"arrow_forward"</span>
                    </a>
                </div>
                <Suspense fallback=|| {
                    view! {
                        <div class="grid sm:grid-cols-2 lg:grid-cols-3 gap-5 animate-pulse">
                            <div class="h-64 bg-surface-container rounded-2xl"></div>
                            <div class="h-64 bg-surface-container rounded-2xl"></div>
                            <div class="h-64 bg-surface-container rounded-2xl"></div>
                        </div>
                    }
                }>
                    {move || {
                        let list = artikel.get().and_then(|r| r.ok()).unwrap_or_default();
                        if list.is_empty() {
                            view! {
                                <div class="ppm-card p-10 text-center">
                                    <span class="material-symbols-outlined text-4xl text-on-surface-variant/60">
                                        "article"
                                    </span>
                                    <p class="text-body-md text-on-surface-variant mt-2">
                                        "Belum ada artikel yang diterbitkan."
                                    </p>
                                </div>
                            }
                                .into_any()
                        } else {
                            view! {
                                <div class="grid sm:grid-cols-2 lg:grid-cols-3 gap-5 stagger">
                                    {list
                                        .into_iter()
                                        .map(|a| view! { <ArtikelCard a=a /> })
                                        .collect_view()}
                                </div>
                            }
                                .into_any()
                        }
                    }}
                </Suspense>
            </section>

            // ── Kampus sekitar ──────────────────────────────────────────────
            <section class="bg-surface-container-low border-y border-outline-variant/40">
                <div class="max-w-6xl mx-auto px-5 py-16 md:py-20 text-center" data-reveal="1">
                    <p class="text-label-md text-primary uppercase tracking-[0.25em]">"Lokasi Strategis"</p>
                    <h2 class="text-display-md text-on-background mt-3">"Dekat Kawasan Kampus Depok"</h2>
                    <p class="text-body-md text-on-surface-variant max-w-xl mx-auto mt-4">
                        "Berlokasi di Pondok Cina, Beji — hanya beberapa menit dari kampus-kampus besar di Depok dan sekitarnya."
                    </p>
                    <div class="flex flex-wrap justify-center gap-3 mt-8">
                        {[
                            "Universitas Indonesia",
                            "Universitas Gunadarma",
                            "Politeknik Negeri Jakarta",
                            "Universitas Pancasila",
                            "dan kampus sekitarnya",
                        ]
                            .into_iter()
                            .map(|k| {
                                view! {
                                    <span class="px-5 py-2.5 bg-surface-container-lowest border border-outline-variant/60 rounded-full text-body-sm font-medium text-on-surface">
                                        {k}
                                    </span>
                                }
                            })
                            .collect_view()}
                    </div>
                </div>
            </section>

            // ── Lokasi & kontak ─────────────────────────────────────────────
            <section id="lokasi">
                <div class="max-w-6xl mx-auto px-5 py-16 md:py-20 grid md:grid-cols-2 gap-10">
                    <div>
                        <p class="text-label-md text-primary uppercase tracking-[0.25em]">"Kunjungi Kami"</p>
                        <h2 class="text-display-md text-on-background mt-3">"Lokasi & Kontak"</h2>
                        <div class="mt-7 space-y-5">
                            <KontakItem icon="location_on" title="Alamat">
                                "Jl. Sawo No.33B, Pondok Cina, Kec. Beji, Kota Depok, Jawa Barat 16424"
                            </KontakItem>
                            <KontakItem icon="call" title="Telepon">
                                "+62 858-8268-5011"
                            </KontakItem>
                            <KontakItem icon="mail" title="Email">
                                "ppm.alfaqihmandiri@gmail.com"
                            </KontakItem>
                            <KontakItem icon="schedule" title="Jam Kunjungan">
                                "Senin – Sabtu, 06.00 – 22.00 WIB"
                            </KontakItem>
                        </div>
                    </div>
                    <div class="bg-surface-container-lowest border border-outline-variant/60 rounded-3xl p-8 flex flex-col items-center justify-center text-center">
                        <div class="w-16 h-16 spiritual-gradient rounded-2xl flex items-center justify-center mb-5">
                            <span class="material-symbols-outlined text-on-primary text-4xl">"badge"</span>
                        </div>
                        <h3 class="text-headline-sm text-on-background">"Sudah menjadi bagian PPM AFM?"</h3>
                        <p class="text-body-md text-on-surface-variant mt-2 max-w-sm">
                            "Santri, dewan guru, pamong, dan orang tua dapat mengakses portal absensi & pembinaan."
                        </p>
                        <a
                            href="/login"
                            class="mt-6 px-7 py-3.5 bg-primary text-on-primary rounded-xl font-semibold hover:bg-primary-container transition-colors shadow-lg shadow-primary/20"
                        >
                            "Masuk Portal Absensi"
                        </a>
                    </div>
                </div>
            </section>

            <PublicFooter />
        </div>
    }
}

/// Bilah navigasi halaman PUBLIK — dipakai beranda dan halaman artikel supaya
/// pengunjung tak kehilangan menu begitu membuka satu tulisan.
///
/// `home` menentukan bentuk tautan bagiannya: di beranda cukup `#tentang`
/// (gulir mulus, tanpa memuat ulang), di halaman lain harus `/#tentang` —
/// jangkar tanpa path di sana menunjuk ke bagian yang tidak ada.
#[component]
pub fn PublicNav(#[prop(optional)] home: bool) -> impl IntoView {
    // Sesi (bila ada) → tombol portal langsung ke dashboard peran, tanpa login.
    // Pakai CONTEXT sesi global dari App — jangan fetch sendiri.
    let session = use_context::<Resource<Option<SessionUser>>>();
    let prefix = if home { "" } else { "/" };
    // "Artikel" menuju HALAMAN tersendiri, bukan jangkar: daftarnya bisa
    // panjang dan tiap tulisan punya alamat sendiri untuk dibagikan.
    let menu: Vec<(String, &'static str)> = vec![
        (format!("{prefix}#beranda"), "Beranda"),
        (format!("{prefix}#tentang"), "Tentang AFM"),
        (format!("{prefix}#struktur"), "Struktur"),
        (format!("{prefix}#fasilitas"), "Fasilitas"),
        ("/artikel".to_string(), "Artikel"),
    ];

    view! {
        <nav class="sticky top-0 z-30 bg-surface/90 backdrop-blur border-b border-outline-variant/50">
            <div class="max-w-6xl mx-auto px-5 py-3 flex items-center justify-between gap-4">
                <a href="/" class="flex items-center gap-3 shrink-0">
                    // Logo asli PPM Al-Faqih Mandiri, bukan glyph masjid bawaan
                    // font — sama dengan halaman login. Alasnya PUTIH, bukan
                    // gradasi hijau seperti dulu: logonya sendiri hijau dua-nada
                    // dengan latar transparan, jadi di atas gradasi hijau ia
                    // praktis lenyap.
                    <div class="w-10 h-10 bg-white rounded-xl flex items-center justify-center overflow-hidden ring-1 ring-outline-variant/50 shrink-0">
                        <img
                            src="/icons/logo.png"
                            alt="Logo PPM Al-Faqih Mandiri"
                            class="w-full h-full object-contain p-1"
                        />
                    </div>
                    <div class="leading-tight">
                        <p class="font-bold text-primary">"AFM SMART"</p>
                        <p class="text-[11px] text-on-surface-variant uppercase tracking-widest hidden sm:block">
                            "Al-Faqih Mandiri"
                        </p>
                    </div>
                </a>
                <div class="hidden lg:flex items-center gap-6 text-body-sm font-medium text-on-surface-variant">
                    {menu
                        .iter()
                        .map(|(href, label)| {
                            view! {
                                <a class="hover:text-primary transition-colors whitespace-nowrap" href=href.clone()>
                                    {*label}
                                </a>
                            }
                        })
                        .collect_view()}
                </div>
                <div class="flex items-center gap-2">
                    // Tombol Buku Tamu (publik) — tamu isi data & dapat kode.
                    <a
                        href="/tamu"
                        class="hidden sm:flex items-center gap-2 px-3 sm:px-4 py-2.5 border border-outline-variant text-on-surface rounded-xl text-body-sm font-semibold hover:border-primary hover:text-primary transition-colors shrink-0 whitespace-nowrap"
                    >
                        <span class="material-symbols-outlined text-lg">"how_to_reg"</span>
                        "Buku Tamu"
                    </a>
                    <Suspense fallback=|| {
                        view! {
                            <a
                                href="/login"
                                class="flex items-center gap-2 px-5 py-2.5 bg-primary text-on-primary rounded-xl text-body-sm font-semibold hover:bg-primary-container transition-colors shadow-md shadow-primary/20"
                            >
                                <span class="material-symbols-outlined text-lg">"login"</span>
                                "Masuk Portal"
                            </a>
                        }
                    }>
                        {move || {
                            let sess = session.and_then(|s| s.get()).flatten();
                            let (href, icon, label) = match &sess {
                                Some(u) => (role_home(&u.role), "arrow_forward", "Buka Portal"),
                                None => ("/login", "login", "Masuk Portal"),
                            };
                            view! {
                                <a
                                    href=href
                                    class="flex items-center gap-2 px-5 py-2.5 bg-primary text-on-primary rounded-xl text-body-sm font-semibold hover:bg-primary-container transition-colors shadow-md shadow-primary/20"
                                >
                                    <span class="material-symbols-outlined text-lg">{icon}</span>
                                    {label}
                                </a>
                            }
                        }}
                    </Suspense>
                </div>
            </div>
            // Menu bagian untuk layar sempit: satu baris yang bisa digeser,
            // bukan tombol hamburger — lima tautan tak cukup banyak untuk
            // membenarkan panel yang harus dibuka dulu.
            <div class="lg:hidden overflow-x-auto border-t border-outline-variant/40">
                <div class="flex items-center gap-5 px-5 py-2.5 text-body-sm font-medium text-on-surface-variant w-max">
                    {menu
                        .iter()
                        .map(|(href, label)| {
                            view! {
                                <a class="hover:text-primary transition-colors whitespace-nowrap" href=href.clone()>
                                    {*label}
                                </a>
                            }
                        })
                        .collect_view()}
                    <a class="hover:text-primary transition-colors whitespace-nowrap sm:hidden" href="/tamu">
                        "Buku Tamu"
                    </a>
                </div>
            </div>
        </nav>
    }
}

/// Kaki halaman publik — sama untuk beranda dan halaman artikel.
#[component]
pub fn PublicFooter() -> impl IntoView {
    view! {
        <footer class="bg-primary text-on-primary">
            <div class="max-w-6xl mx-auto px-5 py-10 flex flex-col md:flex-row items-center justify-between gap-4">
                <div class="flex items-center gap-3">
                    // Ubin putih seperti di navbar & login: kaki halaman
                    // berlatar `bg-primary` (hijau tua), dan logo hijau di
                    // atasnya takkan terbaca.
                    <div class="w-10 h-10 bg-white rounded-xl flex items-center justify-center overflow-hidden shrink-0">
                        <img
                            src="/icons/logo.png"
                            alt="Logo PPM Al-Faqih Mandiri"
                            class="w-full h-full object-contain p-1"
                        />
                    </div>
                    <div class="leading-tight">
                        <p class="font-bold">"PPM Al-Faqih Mandiri"</p>
                        <p class="text-[11px] opacity-70 uppercase tracking-widest">
                            "Pondok Pesantren Mahasiswa"
                        </p>
                    </div>
                </div>
                <p class="text-body-sm opacity-70 text-center">
                    "Jl. Sawo No.33B, Pondok Cina, Beji, Depok · ppm.alfaqihmandiri@gmail.com"
                </p>
            </div>
        </footer>
    }
}

/// Judul & tombol yang menumpang di atas media kepala halaman. Dipisah supaya
/// hero bervideo dan hero polos tak menyalin teks yang sama dua kali — sekali
/// berbeda, halaman akan menampilkan janji yang berbeda tergantung ada tidaknya
/// video, dan tak ada yang menyadarinya.
#[component]
fn HeroIsi(
    /// Di atas video teksnya putih; di atas latar terang memakai warna tema.
    on_video: bool,
) -> impl IntoView {
    let (eyebrow, judul, teks) = if on_video {
        ("text-primary-fixed", "text-white", "text-white/85")
    } else {
        ("text-primary", "text-primary", "text-on-surface-variant")
    };
    view! {
        <p class=format!("text-label-md uppercase tracking-[0.3em] {eyebrow}")>
            "Pondok Pesantren Mahasiswa"
        </p>
        <h1 class=format!(
            "text-display-lg md:text-[52px] md:leading-[60px] font-bold mt-4 {judul}",
        )>"Al-Faqih Mandiri"</h1>
        <p class=format!("text-body-lg max-w-2xl mx-auto mt-5 {teks}")>
            "Tempat pembinaan mahasiswa untuk tumbuh dalam akhlak, ilmu, dan kontribusi nyata — mencetak sarjana yang profesional religius."
        </p>
        <div class="flex flex-wrap items-center justify-center gap-4 mt-9">
            <a
                href="#tentang"
                class="px-7 py-3.5 bg-primary text-on-primary rounded-xl font-semibold hover:bg-primary-container transition-colors shadow-lg shadow-primary/20"
            >
                "Kenali Kami"
            </a>
            <a
                href="/login"
                class=if on_video {
                    "px-7 py-3.5 border border-white/60 rounded-xl font-semibold text-white hover:bg-white/10 transition-colors"
                } else {
                    "px-7 py-3.5 border border-outline-variant rounded-xl font-semibold text-on-surface hover:border-primary hover:text-primary transition-colors"
                }
            >
                "Portal Absensi Santri"
            </a>
        </div>
    }
}

/// Kepala halaman BERVIDEO — video berjalan sendiri, membisu, berulang, dengan
/// tirai gelap agar teks di atasnya tetap terbaca.
///
/// `muted` + `playsinline` wajib ada: tanpa keduanya browser seluler menolak
/// memutar otomatis (atau justru membuka pemutar layar penuh), dan yang tampil
/// hanyalah kotak hitam diam. `poster` tak dipasang karena media pengganti yang
/// masuk akal justru dikelola sebagai media cadangan di halaman galeri.
///
/// Kategori `video_utama` juga menerima FOTO — pondok yang belum punya rekaman
/// tetap mendapat kepala halaman bergambar, bukan hero kosong.
#[component]
fn HeroMedia(m: ActivityPhoto) -> impl IntoView {
    let is_video = m.is_video();
    // Bidikan tersimpan (migrasi 54/55) ikut menentukan bagian yang tampil;
    // ukuran bingkai diatur kelas di bawah, bukan oleh gaya itu.
    let style = format!("{}position:absolute;inset:0;", m.frame_style());
    let alt = m.caption.clone();
    view! {
        <header
            id="beranda"
            class="relative overflow-hidden min-h-[78vh] md:min-h-screen flex items-center"
        >
            {if is_video {
                view! {
                    <video
                        src=m.url.clone()
                        style=style.clone()
                        autoplay="autoplay"
                        muted="muted"
                        prop:muted=true
                        r#loop="loop"
                        playsinline="playsinline"
                        preload="metadata"
                        aria-label=alt.clone()
                    ></video>
                }
                    .into_any()
            } else {
                view! { <img src=m.url.clone() style=style.clone() alt=alt.clone() /> }.into_any()
            }}
            // Tirai: video pondok terang di siang hari, dan teks putih di
            // atasnya tanpa tirai praktis tak terbaca.
            <div class="absolute inset-0 bg-gradient-to-b from-black/55 via-black/40 to-black/70"></div>
            <div class="max-w-6xl mx-auto px-5 py-24 relative text-center anim-in w-full">
                <HeroIsi on_video=true />
            </div>
            {(!m.caption.trim().is_empty())
                .then(|| {
                    view! {
                        <p class="absolute bottom-4 inset-x-0 text-center text-[11px] text-white/70 px-5">
                            {m.caption.clone()}
                        </p>
                    }
                })}
        </header>
    }
}

/// Kepala halaman TANPA media — dipakai selama galeri dimuat dan bila pengelola
/// memang belum mengunggah video utama.
#[component]
fn HeroPolos() -> impl IntoView {
    view! {
        <header id="beranda" class="relative overflow-hidden">
            <div class="absolute inset-0 pattern-bg"></div>
            <div class="max-w-6xl mx-auto px-5 py-20 md:py-28 relative text-center anim-in">
                <HeroIsi on_video=false />

                // Chip statistik ringkas
                <div class="flex flex-wrap items-center justify-center gap-3 mt-10">
                    <span class="inline-flex items-center gap-2 bg-secondary-container text-on-secondary-container px-4 py-2 rounded-full text-body-sm font-medium">
                        <span class="material-symbols-outlined text-lg">"groups"</span>
                        "92 Santri Mahasiswa"
                    </span>
                    <span class="inline-flex items-center gap-2 bg-secondary-container text-on-secondary-container px-4 py-2 rounded-full text-body-sm font-medium">
                        <span class="material-symbols-outlined text-lg">"school"</span>
                        "5 Dewan Guru"
                    </span>
                    <span class="inline-flex items-center gap-2 bg-secondary-container text-on-secondary-container px-4 py-2 rounded-full text-body-sm font-medium">
                        <span class="material-symbols-outlined text-lg">"location_on"</span>
                        "Pondok Cina, Depok"
                    </span>
                </div>
            </div>
        </header>
    }
}

/// Struktur kepengurusan. Isinya berubah setahun sekali, jadi tetap di kode —
/// membuat pengelolaannya lewat portal berarti satu tabel, satu halaman kelola,
/// dan satu peran lagi untuk data yang disunting sekali per periode.
#[component]
fn StrukturSection() -> impl IntoView {
    const PIMPINAN: &[(&str, &str)] = &[
        ("H. Dedy Rinaldi, S.T., M.M.", "Ketua PPM AFM"),
        ("Ust. Anji Hidayat", "Wakil Ketua PPM AFM"),
    ];
    const DEWAN: &[&str] = &[
        "Ust. Prakash Faqih Arifin",
        "Ust. Bachruddin",
        "Ust. M. Sulthon Aulia",
        "Ust. M. Ridho Asidiqi",
    ];
    view! {
        <section
            id="struktur"
            class="bg-surface-container-low border-y border-outline-variant/40"
        >
            <div class="max-w-6xl mx-auto px-5 py-16 md:py-20" data-reveal="1">
                <div class="text-center mb-12">
                    <p class="text-label-md text-primary uppercase tracking-[0.25em]">"Kepengurusan"</p>
                    <h2 class="text-display-md text-on-background mt-3">
                        "Struktur Pengurus PPM AFM"
                    </h2>
                    <p class="text-body-md text-on-surface-variant max-w-xl mx-auto mt-4">
                        "Di belakang layar: menata, menyiapkan, dan menjaga — karena Allah dan konsisten."
                    </p>
                </div>

                <div class="grid sm:grid-cols-2 gap-5 max-w-3xl mx-auto stagger">
                    {PIMPINAN
                        .iter()
                        .map(|(nama, jabatan)| {
                            view! {
                                <div class="ppm-card p-6 text-center">
                                    <div class="w-14 h-14 spiritual-gradient rounded-2xl flex items-center justify-center mx-auto mb-4">
                                        <span class="material-symbols-outlined text-on-primary text-3xl">
                                            "person"
                                        </span>
                                    </div>
                                    <p class="text-body-lg font-bold text-on-background">{*nama}</p>
                                    <p class="text-body-sm text-primary font-semibold mt-1">{*jabatan}</p>
                                </div>
                            }
                        })
                        .collect_view()}
                </div>

                <p class="text-label-md text-primary uppercase tracking-[0.25em] text-center mt-12 mb-5">
                    "Dewan Guru"
                </p>
                <div class="grid sm:grid-cols-2 lg:grid-cols-4 gap-4 stagger">
                    {DEWAN
                        .iter()
                        .map(|nama| {
                            view! {
                                <div class="ppm-card p-5 flex items-center gap-3">
                                    <span class="w-11 h-11 ppm-tile">
                                        <span class="material-symbols-outlined">"school"</span>
                                    </span>
                                    <div class="min-w-0">
                                        <p class="text-body-md font-semibold text-on-background">{*nama}</p>
                                        <p class="text-body-sm text-on-surface-variant">"Dewan Guru"</p>
                                    </div>
                                </div>
                            }
                        })
                        .collect_view()}
                </div>

                <div class="ppm-card p-6 mt-4 flex items-center gap-4">
                    <span class="w-12 h-12 ppm-tile">
                        <span class="material-symbols-outlined">"diversity_3"</span>
                    </span>
                    <div>
                        <p class="text-body-lg font-semibold text-on-background">
                            "Pamong Putra & Putri"
                        </p>
                        <p class="text-body-sm text-on-surface-variant mt-0.5">
                            "Mendampingi santri sehari-hari: kehadiran, perizinan, dan pembinaan akhlak."
                        </p>
                    </div>
                </div>
            </div>
        </section>
    }
}

/// Fasilitas: foto dari galeri (kategori `fasilitas`) di atas, daftar penunjang
/// di bawah.
///
/// Foto dan daftarnya sengaja bukan satu sumber. Foto ikut apa yang sudah
/// diunggah pengelola dan bisa kosong sama sekali; daftar penunjang adalah
/// keterangan yang berlaku terlepas dari ada tidaknya foto, dan pengunjung yang
/// menimbang mendaftar justru menanyakan hal-hal itu.
#[component]
fn FasilitasSection(fotos: Signal<Vec<ActivityPhoto>>) -> impl IntoView {
    const PENUNJANG: &[(&str, &str, &str)] = &[
        ("wifi", "WiFi", "Koneksi internet untuk kegiatan belajar"),
        ("shield_person", "Keamanan 24 Jam", "Dijaga Senkom dan dilengkapi CCTV"),
        ("local_laundry_service", "Mesin Cuci", "Area untuk mencuci dan menjemur pakaian"),
        ("water_drop", "Air Minum Refill", "Tersedia untuk kebutuhan santri"),
        ("bed", "Kamar 2 Orang", "Dengan kasur bertingkat yang nyaman"),
        ("restaurant", "Makan 2× Sehari", "Kebutuhan makanan santri terjamin"),
    ];
    view! {
        <section id="fasilitas" class="max-w-6xl mx-auto px-5 py-16 md:py-20" data-reveal="1">
            <div class="text-center mb-12">
                <p class="text-label-md text-primary uppercase tracking-[0.25em]">"Fasilitas"</p>
                <h2 class="text-display-md text-on-background mt-3">"Sarana Penunjang Santri"</h2>
                <p class="text-body-md text-on-surface-variant max-w-xl mx-auto mt-4">
                    "Menunjang kegiatan pembelajaran dan pengembangan diri santri dengan fasilitas yang memadai."
                </p>
            </div>

            <Suspense fallback=|| ()>
                {move || {
                    let list = fotos.get();
                    (!list.is_empty())
                        .then(|| {
                            view! {
                                <div class="grid sm:grid-cols-2 lg:grid-cols-3 gap-5 mb-12 stagger">
                                    {list
                                        .into_iter()
                                        .map(|p| {
                                            let caption = p.caption.clone();
                                            view! {
                                                <figure class="ppm-card overflow-hidden">
                                                    <PhotoFrame
                                                        src=p.url.clone()
                                                        style=p.frame_style()
                                                        backdrop=p.fit().needs_backdrop()
                                                        alt=caption.clone()
                                                        class="aspect-[4/3] bg-surface-container"
                                                        lazy=true
                                                    />
                                                    {(!caption.trim().is_empty())
                                                        .then(|| {
                                                            view! {
                                                                <figcaption class="px-5 py-4 text-body-md font-semibold text-on-background">
                                                                    {caption.clone()}
                                                                </figcaption>
                                                            }
                                                        })}
                                                </figure>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            }
                        })
                }}
            </Suspense>

            <div class="grid sm:grid-cols-2 lg:grid-cols-3 gap-4 stagger">
                {PENUNJANG
                    .iter()
                    .map(|(icon, nama, desc)| {
                        view! {
                            <div class="ppm-card p-5 flex items-start gap-3">
                                <span class="w-11 h-11 ppm-tile">
                                    <span class="material-symbols-outlined">{*icon}</span>
                                </span>
                                <div class="min-w-0">
                                    <p class="text-body-md font-semibold text-on-background">{*nama}</p>
                                    <p class="text-body-sm text-on-surface-variant mt-0.5">{*desc}</p>
                                </div>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        </section>
    }
}

/// Kartu satu artikel — dipakai beranda dan halaman /artikel.
#[component]
pub fn ArtikelCard(a: Article) -> impl IntoView {
    let href = format!("/artikel/{}", a.slug);
    let ringkas = a.summary();
    view! {
        <a href=href class="ppm-card overflow-hidden flex flex-col card-hover hover:border-primary/40">
            {match a.cover_url.clone() {
                Some(url) => {
                    view! {
                        <img
                            src=url
                            alt=a.title.clone()
                            loading="lazy"
                            class="w-full aspect-[16/9] object-cover bg-surface-container"
                        />
                    }
                        .into_any()
                }
                None => {
                    view! {
                        <div class="w-full aspect-[16/9] spiritual-gradient flex items-center justify-center">
                            <span class="material-symbols-outlined text-on-primary text-5xl opacity-80">
                                "article"
                            </span>
                        </div>
                    }
                        .into_any()
                }
            }}
            <div class="p-5 flex-1 flex flex-col">
                <p class="text-[11px] text-on-surface-variant uppercase tracking-widest">
                    {a.created_at.clone()}
                </p>
                <h3 class="text-body-lg font-bold text-on-background mt-1.5">{a.title.clone()}</h3>
                <p class="text-body-sm text-on-surface-variant mt-2 leading-relaxed flex-1">
                    {ringkas}
                </p>
                <span class="text-body-sm font-semibold text-primary mt-4 inline-flex items-center gap-1">
                    "Baca selengkapnya"
                    <span class="material-symbols-outlined text-[18px]">"arrow_forward"</span>
                </span>
            </div>
        </a>
    }
}

/// Satu foto kegiatan di bagian "Tentang".
#[component]
fn KartuFoto(p: ActivityPhoto) -> impl IntoView {
    // Bidikan dari editor galeri (migrasi 54 & 55) — gaya yang sama dipakai
    // grid pengelola meski rasionya beda. Foto tanpa pengaturan tampil seperti
    // dulu.
    //
    // RASIO 4:3 MENDATAR, bukan 3:4 tegak seperti dulu. Foto kegiatan pondok
    // hampir seluruhnya diambil mendatar — foto rombongan, apel, kegiatan
    // bersama — dan bingkai tegak memotongnya justru di bagian yang penting:
    // orang-orangnya. Yang tampil malah langit-langit dan tembok, sementara
    // barisan santri terpangkas di kedua sisi.
    //
    // `MediaFrame`, bukan `PhotoFrame`: kategori "Kegiatan" menerima VIDEO juga
    // (migrasi 69), dan `PhotoFrame` merender apa pun sebagai `<img>`. Video
    // yang diunggah ke kategori ini karena itu tampil sebagai kotak kosong —
    // bingkainya ada, isinya tidak — tanpa galat apa pun yang bisa dilihat
    // pengelola maupun pengunjung.
    view! {
        <MediaFrame
            src=p.url.clone()
            style=p.frame_style()
            video=p.is_video()
            backdrop=p.fit().needs_backdrop()
            alt=p.caption.clone()
            class="rounded-2xl aspect-[4/3] bg-surface-container"
            lazy=true
        />
    }
}

/// Petak dekoratif saat belum ada satu pun foto kegiatan yang diunggah.
#[component]
fn FotoPlaceholder() -> impl IntoView {
    view! {
        <div class="grid grid-cols-2 gap-4">
            <FotoCard icon="menu_book" label="Kajian Kitab" tall=true />
            <FotoCard icon="mosque" label="Sholat Berjamaah" tall=false />
            <FotoCard icon="diversity_3" label="Kebersamaan" tall=false />
            <FotoCard icon="volunteer_activism" label="Kontribusi Dakwah" tall=true />
        </div>
    }
}

/// Kartu "foto" dekoratif (ikon di atas gradient — placeholder foto kegiatan).
#[component]
fn FotoCard(icon: &'static str, label: &'static str, tall: bool) -> impl IntoView {
    let cls = if tall {
        "spiritual-gradient rounded-2xl p-5 flex flex-col justify-end min-h-[180px] text-on-primary shadow-lg shadow-primary/10"
    } else {
        "bg-secondary-container rounded-2xl p-5 flex flex-col justify-end min-h-[140px] text-on-secondary-container"
    };
    view! {
        <div class=cls>
            <span class="material-symbols-outlined text-4xl opacity-90">{icon}</span>
            <p class="text-body-sm font-semibold mt-2">{label}</p>
        </div>
    }
}

#[component]
fn KegiatanCard(icon: &'static str, title: &'static str, desc: &'static str) -> impl IntoView {
    view! {
        <div class="ppm-card p-6 hover:border-primary/40 card-hover">
            <div class="w-12 h-12 rounded-xl bg-secondary-container flex items-center justify-center text-primary mb-4">
                <span class="material-symbols-outlined text-2xl">{icon}</span>
            </div>
            <h3 class="text-body-lg font-semibold text-on-background">{title}</h3>
            <p class="text-body-sm text-on-surface-variant mt-2 leading-relaxed">{desc}</p>
        </div>
    }
}

#[component]
fn StatItem(num: &'static str, label: &'static str) -> impl IntoView {
    view! {
        <div>
            <p class="text-[44px] leading-none font-bold" data-count=num>{num}</p>
            <p class="text-body-sm opacity-80 mt-2">{label}</p>
        </div>
    }
}

#[component]
fn KontakItem(icon: &'static str, title: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class="flex items-start gap-4">
            <div class="w-11 h-11 ppm-tile">
                <span class="material-symbols-outlined">{icon}</span>
            </div>
            <div>
                <p class="text-body-md font-semibold text-on-background">{title}</p>
                <p class="text-body-sm text-on-surface-variant mt-0.5">{children()}</p>
            </div>
        </div>
    }
}
