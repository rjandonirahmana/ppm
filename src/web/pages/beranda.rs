//! web/pages/beranda.rs — Beranda PUBLIK untuk pengunjung (profil PPM AFM).
//!
//! Konten dari situs resmi (afm-website-gilt.vercel.app): tentang, kegiatan
//! harian, statistik santri/pengajar, kampus sekitar, lokasi & kontak.
//! Desain memakai design system portal (emerald + Work Sans) agar senada.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::models::{role_home, SessionUser};

#[component]
pub fn BerandaPage() -> impl IntoView {
    // Sesi (bila ada) → tombol portal langsung ke dashboard peran, tanpa login.
    // Pakai CONTEXT sesi global dari App (audit poin 4) — jangan fetch sendiri.
    let session = use_context::<Resource<Option<SessionUser>>>();
    // Foto kegiatan (galeri, migrasi 34) — publik. Bila kosong, jatuh ke
    // placeholder ikon (FotoCard) agar tampilan tetap terisi.
    let gallery = Resource::new(
        || (),
        |_| async move { crate::web::api::activity_photos_data().await },
    );

    view! {
        <Title text="PPM Al-Faqih Mandiri — Pondok Pesantren Mahasiswa Depok" />
        <div class="min-h-screen bg-surface text-on-surface">

            // ── Navbar ──────────────────────────────────────────────────────
            <nav class="sticky top-0 z-30 bg-surface/90 backdrop-blur border-b border-outline-variant/50">
                <div class="max-w-6xl mx-auto px-5 py-3 flex items-center justify-between gap-4">
                    <a href="/" class="flex items-center gap-3">
                        <div class="w-10 h-10 spiritual-gradient rounded-xl flex items-center justify-center">
                            <span class="material-symbols-outlined text-on-primary text-2xl">"mosque"</span>
                        </div>
                        <div class="leading-tight">
                            <p class="font-bold text-primary">"PPM AFM"</p>
                            <p class="text-[11px] text-on-surface-variant uppercase tracking-widest hidden sm:block">
                                "Al-Faqih Mandiri"
                            </p>
                        </div>
                    </a>
                    <div class="hidden md:flex items-center gap-6 text-body-sm font-medium text-on-surface-variant">
                        <a class="hover:text-primary transition-colors" href="#tentang">"Tentang"</a>
                        <a class="hover:text-primary transition-colors" href="#kegiatan">"Kegiatan"</a>
                        <a class="hover:text-primary transition-colors" href="#statistik">"Statistik"</a>
                        <a class="hover:text-primary transition-colors" href="#lokasi">"Lokasi"</a>
                    </div>
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
            </nav>

            // ── Hero ────────────────────────────────────────────────────────
            <header class="relative overflow-hidden">
                <div class="absolute inset-0 pattern-bg"></div>
                <div class="max-w-6xl mx-auto px-5 py-20 md:py-28 relative text-center anim-in">
                    <p class="text-label-md text-primary uppercase tracking-[0.3em]">
                        "Pondok Pesantren Mahasiswa"
                    </p>
                    <h1 class="text-display-lg md:text-[52px] md:leading-[60px] font-bold text-primary mt-4">
                        "Al-Faqih Mandiri"
                    </h1>
                    <p class="text-body-lg text-on-surface-variant max-w-2xl mx-auto mt-5">
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
                            class="px-7 py-3.5 border border-outline-variant rounded-xl font-semibold text-on-surface hover:border-primary hover:text-primary transition-colors"
                        >
                            "Portal Absensi Santri"
                        </a>
                    </div>

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
                    <Suspense fallback=|| view! {
                        <div class="grid grid-cols-2 gap-4">
                            <FotoCard icon="menu_book" label="Kajian Kitab" tall=true />
                            <FotoCard icon="mosque" label="Sholat Berjamaah" tall=false />
                            <FotoCard icon="diversity_3" label="Kebersamaan" tall=false />
                            <FotoCard icon="volunteer_activism" label="Kontribusi Dakwah" tall=true />
                        </div>
                    }>
                        {move || {
                            let photos = gallery.get().and_then(|r| r.ok()).unwrap_or_default();
                            if photos.is_empty() {
                                view! {
                                    <div class="grid grid-cols-2 gap-4">
                                        <FotoCard icon="menu_book" label="Kajian Kitab" tall=true />
                                        <FotoCard icon="mosque" label="Sholat Berjamaah" tall=false />
                                        <FotoCard icon="diversity_3" label="Kebersamaan" tall=false />
                                        <FotoCard icon="volunteer_activism" label="Kontribusi Dakwah" tall=true />
                                    </div>
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <div class="grid grid-cols-2 gap-4">
                                        {photos
                                            .into_iter()
                                            .take(6)
                                            .map(|p| view! {
                                                <div class="rounded-2xl overflow-hidden aspect-[3/4] bg-surface-container">
                                                    <img
                                                        src=p.url
                                                        alt=p.caption
                                                        loading="lazy"
                                                        class="w-full h-full object-cover"
                                                    />
                                                </div>
                                            })
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

            // ── Kampus sekitar ──────────────────────────────────────────────
            <section class="max-w-6xl mx-auto px-5 pb-16 md:pb-20 text-center" data-reveal="1">
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
            </section>

            // ── Lokasi & kontak ─────────────────────────────────────────────
            <section id="lokasi" class="bg-surface-container-low border-t border-outline-variant/40">
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

            // ── Footer ──────────────────────────────────────────────────────
            <footer class="bg-primary text-on-primary">
                <div class="max-w-6xl mx-auto px-5 py-10 flex flex-col md:flex-row items-center justify-between gap-4">
                    <div class="flex items-center gap-3">
                        <span class="material-symbols-outlined text-2xl">"mosque"</span>
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
