-- =============================================================================
-- 74_import_master_nis.sql — Daftar induk santri PPM AFM (512 orang) masuk
-- ke tabel `users`.
--
-- Sumber : MASTER_NIS SANTRI PPM AFM.xlsx (sheet NIS_GABUNGAN), 2026-08-08
-- Prasyarat: migrasi 73 (kolom mubalegh_status & pendidikan_status).
--
-- Ini DATA INDUK, bukan seed contoh. Bedanya penting, karena migrasi 70 baru
-- saja mengeluarkan `3_seed_users.sql` dari rantai ini: yang itu akun uji
-- ber-sandi publik yang tak boleh ada di produksi, sedangkan yang ini catatan
-- resmi pondok yang justru HARUS sama di setiap lingkungan. Menaruhnya sebagai
-- migrasi membuat database baru berisi daftar santri yang benar tanpa ada yang
-- perlu mengingat menjalankan skrip terpisah.
--
-- ── APA YANG SUDAH DIBERESKAN DARI BERKAS ASLINYA ────────────────────────────
-- Excel-nya berisi 616 baris, TAPI hanya 512 santri: 104 NIS muncul dua kali.
-- Salinan keduanya bukan orang lain — 11 di antaranya membawa karakter TAK
-- TERLIHAT U+2060 (WORD JOINER) di depan nama, sehingga berbeda bagi komputer
-- tapi identik di mata. `users.nis` UNIQUE, jadi impor mentah berhenti di
-- duplikat pertama.
--
-- Yang dilakukan skrip pembuat berkas ini:
--   • karakter tak terlihat dibuang, spasi ganda dirapatkan;
--   • baris ganda DIGABUNG — keterangan dari salinan mana pun dipertahankan
--     (satu salinan sering hanya mengisi sebagian kolom);
--   • jenis kelamin selain 'L'/'P' → NULL (3 santri);
--   • status Mubalegh/Sarjana dipetakan ke kode migrasi 73. SEMUA nilai di
--     sumber terpetakan — tak ada yang jatuh ke "tak dikenal".
--
-- Sebaran angkatan  : 2025:35, 2024:37, 2023:37, 2022:44, 2021:35, 2020:32, 2019:53, 2018:28, 2017:50, 2016:56, 2015:29, 2014:27, 2013:31, 2012:9, 2011:5, 2010:4
-- mubalegh_status   : belum:305 · iya:131 · NULL:58 · tugasan:18
-- pendidikan_status : sarjana:308 · NULL:175 · belum:16 · kuliah:13
--
-- ── TIGA KEPUTUSAN YANG PERLU DISADARI ───────────────────────────────────────
--
-- 1. SEMUA MASUK is_active = FALSE.
--    Daftar ini registri 2010–2025; sebagian besar sudah alumni. Halaman depan
--    menyebut 92 santri aktif, sementara berkas ini 512. Mengimpor semuanya
--    aktif membuat SETIAP angka dashboard salah — dan job auto-absent akan
--    menandai ratusan alumni sebagai alpa setiap hari. Aktifkan yang benar-benar
--    mondok lewat blok di akhir berkas ini (dijalankan TERPISAH, bukan bagian
--    dari migrasi — siapa yang mondok berubah tiap tahun).
--
-- 2. TANPA saldo poin.
--    `points` diisi 0 EKSPLISIT, bukan mengandalkan DEFAULT kolom — yang pernah
--    300 (migrasi 28) lalu kembali 0 (migrasi 72). Dengan begitu hasilnya sama
--    apa pun urutan migrasinya. Saldo awal 300 diberikan saat DIAKTIFKAN, lewat
--    `point_logs`, supaya `points = SUM(delta)` tetap benar.
--
-- 3. BELUM BISA LOGIN.
--    `password_hash` diisi satu hash bcrypt SAH dari string acak yang tidak
--    pernah dicatat di mana pun — tak ada sandi yang cocok dengannya. Hash
--    ngawur sengaja TIDAK dipakai: `service/auth.rs` memanggil
--    `bcrypt::verify(...)?`, dan hash tak sah menghasilkan GALAT SERVER alih-alih
--    pesan "sandi salah" yang rapi.
--
--    ⚠️ Registrasi mencocokkan NOMOR HP, bukan NIS. Santri yang mendaftar akan
--    membuat baris BARU, bukan menyambung ke baris ini.
--
-- ⚠️ SATU NIS DIPAKAI DUA NAMA — perlu diputuskan manusia:
--      NIS 500032760078250035
--        "Arnando Faaris"              ← yang tersimpan di sini
--        "Muhammad Arnando Al Faaris"  ← dibuang
--    Keduanya tanpa jenis kelamin, jadi aturan "ambil yang terlengkap" tak
--    memihak dan yang menang hanya yang muncul lebih dulu. Itu kebetulan urutan
--    baris, bukan keputusan. Kalau ternyata DUA orang, salah satunya butuh NIS
--    sendiri — `nis` UNIQUE, jadi yang kedua dilewati tanpa peringatan dan satu
--    santri hilang diam-diam.
--
-- TIDAK memuat BEGIN/COMMIT: `scripts/migrate.sh` sudah membungkus tiap migrasi
-- dalam satu transaksi bersama pencatatannya.
-- Idempotent. Jalankan setelah migrasi 1–73.
-- =============================================================================

-- Daftarnya ditulis SEKALI sebagai CTE lalu dipakai dua kali: menyisipkan yang
-- belum ada, dan mengisi status pada baris yang mungkin sudah masuk lebih dulu
-- (mis. lewat dev/import_master_nis.sql sebelum migrasi 73 ada). Menyalin 512
-- baris dua kali akan menggandakan berkas ini — dan menjamin keduanya menyimpang
-- begitu salah satunya disunting.
WITH d(full_name, nis, entry_year, gender, mubalegh_status, pendidikan_status) AS (
  VALUES
    ('Wahyu Indra Pramadhana', '500032760078100001', 2010, 'L', 'belum', 'sarjana'),
    ('Bayu Suntoro', '500032760078100002', 2010, 'L', 'belum', 'belum'),
    ('Wiwi Mustika', '500032760078100003', 2010, 'P', 'belum', 'sarjana'),
    ('Dewan Twi Kusumaningtyas', '500032760078100004', 2010, 'P', 'iya', 'sarjana'),
    ('Hanifan Lidinillah', '500032760078110001', 2011, 'L', 'iya', 'belum'),
    ('Annisa Azka Hikmawati Aulia', '500032760078110002', 2011, 'P', 'belum', 'belum'),
    ('Tri Hidayanti', '500032760078110003', 2011, 'P', 'iya', 'sarjana'),
    ('Thoyibun Muchamad Nazhif', '500032760078110004', 2011, 'L', 'belum', 'sarjana'),
    ('Zulva Ibadati', '500032760078110005', 2011, 'P', 'iya', 'sarjana'),
    ('Wildan Abdillah Wahab', '500032760078120001', 2012, 'L', 'belum', 'sarjana'),
    ('Aninda Fatkhurroji', '500032760078120002', 2012, 'P', 'iya', 'sarjana'),
    ('Rizki Wibowo', '500032760078120003', 2012, 'L', 'belum', 'sarjana'),
    ('Ratih Meliyana', '500032760078120004', 2012, 'P', 'iya', 'sarjana'),
    ('Hubaidiyah Diagusdin Fauzi', '500032760078120005', 2012, 'P', 'belum', 'sarjana'),
    ('Fauziah Isyana Kusumawardhani', '500032760078120006', 2012, 'P', 'belum', 'sarjana'),
    ('Putri Aristya Devi', '500032760078120007', 2012, 'P', 'tugasan', 'sarjana'),
    ('Amelia Rahayu', '500032760078120008', 2012, 'P', 'belum', 'sarjana'),
    ('Nivia Nurbayani', '500032760078120009', 2012, 'P', 'iya', 'sarjana'),
    ('Muhammad Ridho Robby', '500032760078130001', 2013, 'L', 'iya', 'sarjana'),
    ('Ahmad Nur Sholikin', '500032760078130002', 2013, 'L', 'belum', 'sarjana'),
    ('Nur Meliyana', '500032760078130003', 2013, 'P', 'belum', 'sarjana'),
    ('Raihana Rifkin Tsania', '500032760078130004', 2013, 'P', 'iya', 'sarjana'),
    ('Farida Lusiana Dewi', '500032760078130005', 2013, 'P', 'iya', 'sarjana'),
    ('Albajili Gading Paramanandana', '500032760078130006', 2013, 'L', 'iya', 'sarjana'),
    ('Rabid Yahya Putradasa', '500032760078130007', 2013, 'L', 'belum', 'sarjana'),
    ('Irma Gayatri', '500032760078130008', 2013, 'P', 'iya', 'sarjana'),
    ('Rifqy Rahmana Putra', '500032760078130009', 2013, 'L', 'iya', 'sarjana'),
    ('Septian Dwi Cahya', '500032760078130010', 2013, 'L', 'belum', 'sarjana'),
    ('Riri Novani Putri', '500032760078130011', 2013, 'P', 'iya', 'sarjana'),
    ('Oxye Dara Tri Janah', '500032760078130012', 2013, 'P', 'iya', 'sarjana'),
    ('Aicha Shavira Ashuryani', '500032760078130013', 2013, 'P', 'iya', 'sarjana'),
    ('Prima Afiari', '500032760078130014', 2013, 'P', 'belum', 'sarjana'),
    ('Hafidh Nurhidayat', '500032760078130015', 2013, 'L', 'belum', 'sarjana'),
    ('Anjiansyah', '500032760078130016', 2013, 'L', 'tugasan', 'sarjana'),
    ('Camelia Wulandari', '500032760078130017', 2013, 'P', 'belum', 'sarjana'),
    ('Azizul Pin Zulfa', '500032760078130018', 2013, 'P', 'belum', 'sarjana'),
    ('Andrean Wardani', '500032760078130019', 2013, 'L', 'belum', 'sarjana'),
    ('Destin Anggini Putri', '500032760078130020', 2013, 'P', 'tugasan', 'sarjana'),
    ('Hafizka Chandra Dewanti', '500032760078130021', 2013, 'P', 'belum', 'sarjana'),
    ('Adriano Fazar Kausari', '500032760078130022', 2013, 'L', 'belum', 'belum'),
    ('Anshorulloh Abd Fath', '500032760078130023', 2013, 'L', 'belum', 'sarjana'),
    ('Oberheim Zildjian', '500032760078130024', 2013, 'L', 'belum', 'sarjana'),
    ('Ira Fitri Auliana', '500032760078130025', 2013, 'P', 'belum', 'sarjana'),
    ('Arini Idza Safarina', '500032760078130026', 2013, 'P', 'belum', 'sarjana'),
    ('Dian Indah Safitri', '500032760078130027', 2013, 'P', 'belum', 'sarjana'),
    ('Nesya Fauziah Herdiana', '500032760078130028', 2013, 'P', 'iya', 'sarjana'),
    ('Sabila Fazadina', '500032760078130029', 2013, 'P', 'belum', 'sarjana'),
    ('Annisa Nur', '500032760078130030', 2013, 'P', 'belum', 'sarjana'),
    ('Yaumil Fauzul Choiri', '500032760078130031', 2013, 'L', 'belum', 'sarjana'),
    ('Yudi Hermawan', '500032760078140001', 2014, 'L', 'tugasan', 'sarjana'),
    ('Ilma Navarinda', '500032760078140002', 2014, 'P', 'belum', 'sarjana'),
    ('Dicky Ramadhan Adhitama', '500032760078140003', 2014, 'L', 'tugasan', 'sarjana'),
    ('Choirul Umam', '500032760078140004', 2014, 'L', 'belum', 'sarjana'),
    ('Putri Andriani', '500032760078140005', 2014, 'P', 'belum', 'sarjana'),
    ('Nuri Andini', '500032760078140006', 2014, 'P', 'belum', 'sarjana'),
    ('Muhammad Emir Herbawono', '500032760078140007', 2014, 'L', 'belum', 'sarjana'),
    ('Fajar Abdul Sahel Alchadad', '500032760078140008', 2014, 'L', 'iya', 'sarjana'),
    ('Nisfulaili Abdurrohman', '500032760078140009', 2014, 'L', 'iya', 'sarjana'),
    ('Karima Marfuatun Hidayati', '500032760078140010', 2014, 'P', 'iya', 'sarjana'),
    ('Dian Puspitasari', '500032760078140011', 2014, 'P', 'belum', 'sarjana'),
    ('Abdul Faqih Fiamrillah', '500032760078140012', 2014, 'L', 'belum', 'sarjana'),
    ('Riza Satria Permana', '500032760078140013', 2014, 'L', 'iya', 'sarjana'),
    ('Lazuardi Imani', '500032760078140014', 2014, 'L', 'iya', 'sarjana'),
    ('Faisal Arief Hutomo', '500032760078140015', 2014, 'P', 'belum', 'sarjana'),
    ('Qonitha Aulia Fadhilah', '500032760078140016', 2014, 'P', 'belum', 'sarjana'),
    ('Nuraini', '500032760078140017', 2014, 'L', 'iya', 'sarjana'),
    ('Annisa Azizah', '500032760078140018', 2014, 'L', 'iya', 'sarjana'),
    ('Muhammad Fikri Akbar', '500032760078140019', 2014, 'L', 'belum', 'sarjana'),
    ('Sabila Salsa Mazaya', '500032760078140020', 2014, 'P', 'iya', 'sarjana'),
    ('Levi Hidayati', '500032760078140021', 2014, 'P', 'belum', 'sarjana'),
    ('Fiqi Aris Supriatna', '500032760078140022', 2014, 'L', 'belum', 'sarjana'),
    ('Hana Zakiyyah Fatinah', '500032760078140023', 2014, 'P', 'belum', 'sarjana'),
    ('Zidny Ilma Andromeda', '500032760078140024', 2014, 'P', 'belum', 'sarjana'),
    ('Arizko Sisti Oktavian', '500032760078140025', 2014, 'L', 'belum', 'sarjana'),
    ('Delima Rohatullatifa', '500032760078140026', 2014, 'P', 'tugasan', 'sarjana'),
    ('Dian Aris Priyanti', '500032760078140027', 2014, 'P', 'belum', 'sarjana'),
    ('Abdillah Muhith', '500032760078150001', 2015, 'L', 'belum', 'sarjana'),
    ('Afina Febrani', '500032760078150002', 2015, 'P', 'tugasan', 'sarjana'),
    ('Afina Romadhona', '500032760078150003', 2015, 'P', 'tugasan', 'sarjana'),
    ('Alya Adninta', '500032760078150004', 2015, 'P', 'belum', 'sarjana'),
    ('Anugerah Wicaksana Nurroyyatul', '500032760078150005', 2015, 'L', 'belum', 'sarjana'),
    ('Bobby Kaisar Hardi', '500032760078150006', 2015, 'L', 'belum', 'sarjana'),
    ('Dharna Aisyah Avilyanty', '500032760078150007', 2015, 'P', 'tugasan', 'sarjana'),
    ('Gendrany Rara Pinilih', '500032760078150008', 2015, 'P', 'iya', 'sarjana'),
    ('Khairul Umam', '500032760078150009', 2015, 'L', 'iya', 'sarjana'),
    ('Kukuh Firdaus', '500032760078150010', 2015, 'L', 'iya', 'sarjana'),
    ('Muhammad Rizky Fadhillah', '500032760078150011', 2015, 'L', 'belum', 'sarjana'),
    ('Nadya Dias Fadhila', '500032760078150012', 2015, 'P', 'belum', 'sarjana'),
    ('Nadya Tyas Putri Ayudhi', '500032760078150013', 2015, 'P', 'belum', 'sarjana'),
    ('Naiza Astri', '500032760078150014', 2015, 'P', 'belum', 'belum'),
    ('Naufal Ghifari Rahmat', '500032760078150015', 2015, 'L', 'belum', 'sarjana'),
    ('Okta Hernawan Bagus Prastya', '500032760078150016', 2015, 'L', 'belum', 'belum'),
    ('Priska Humaira', '500032760078150017', 2015, 'P', 'belum', 'sarjana'),
    ('Reihayati Auliana', '500032760078150018', 2015, 'P', 'belum', 'sarjana'),
    ('Riski Karnila', '500032760078150019', 2015, 'P', 'belum', 'sarjana'),
    ('Sabila Aghniya Khoirunnisa', '500032760078150020', 2015, 'P', 'belum', 'sarjana'),
    ('Sarah Ayu Setiawan', '500032760078150021', 2015, 'P', 'belum', 'sarjana'),
    ('Sofa Femilia', '500032760078150022', 2015, 'P', 'iya', 'sarjana'),
    ('Syanies Aulia', '500032760078150023', 2015, 'P', 'belum', 'sarjana'),
    ('Tsalitsa Maysa Aunidina', '500032760078150024', 2015, 'P', 'belum', 'sarjana'),
    ('Vania Rin Winarti', '500032760078150025', 2015, 'P', 'belum', 'sarjana'),
    ('Yunitasari Suprapto', '500032760078150026', 2015, 'P', 'belum', 'sarjana'),
    ('Zaki Nur Wahyudi', '500032760078150027', 2015, 'L', 'belum', 'sarjana'),
    ('Irfan Maulana', '500032760078150028', 2015, 'L', 'iya', 'sarjana'),
    ('Rjandoni Rahmana', '500032760078150029', 2015, 'L', 'belum', 'sarjana'),
    ('Albert Marconi Lubis', '500032760078160001', 2016, 'L', 'belum', 'sarjana'),
    ('Amanda Safira Yasmin', '500032760078160002', 2016, 'P', 'belum', 'sarjana'),
    ('Amar Wildan Fadillah', '500032760078160003', 2016, 'L', 'belum', 'sarjana'),
    ('Amirul Iman', '500032760078160004', 2016, 'L', 'belum', 'sarjana'),
    ('Amrina Rosyada', '500032760078160005', 2016, 'P', 'belum', 'sarjana'),
    ('Ardhini Risfa Jacinda', '500032760078160006', 2016, 'P', 'belum', 'sarjana'),
    ('Arfanto Chalawathal Iman', '500032760078160007', 2016, 'L', 'belum', 'sarjana'),
    ('Auliya Muksith Hafiz', '500032760078160008', 2016, 'L', 'iya', 'sarjana'),
    ('Dea Apriliani', '500032760078160009', 2016, 'P', 'iya', 'sarjana'),
    ('Debra Hanifah', '500032760078160010', 2016, 'P', 'iya', 'sarjana'),
    ('Dzaki Alyafi', '500032760078160011', 2016, 'L', 'iya', 'sarjana'),
    ('Eko Bayu Priyambodo', '500032760078160012', 2016, 'L', 'belum', 'sarjana'),
    ('Fatimah Putri Nur Wijayanti', '500032760078160013', 2016, 'P', 'belum', 'sarjana'),
    ('Febri Darmawan Akbar', '500032760078160014', 2016, 'L', 'iya', 'sarjana'),
    ('Ferrany Thifla', '500032760078160015', 2016, 'P', 'belum', 'sarjana'),
    ('Hanifa Nur Haliza', '500032760078160016', 2016, 'P', 'belum', 'sarjana'),
    ('Ikhwan Nursetiawan', '500032760078160017', 2016, 'L', 'belum', 'sarjana'),
    ('Ina Lestari', '500032760078160018', 2016, 'P', 'belum', 'sarjana'),
    ('Iqbal Fawaz Al- Amir', '500032760078160019', 2016, 'L', 'tugasan', 'sarjana'),
    ('Irma Novitasari', '500032760078160020', 2016, 'P', 'belum', 'sarjana'),
    ('Isfania Jihan Naviulana', '500032760078160021', 2016, 'P', 'belum', 'sarjana'),
    ('Izza Gamapat Qonita', '500032760078160022', 2016, 'P', 'iya', 'sarjana'),
    ('Juan Antonio Cedric', '500032760078160023', 2016, 'L', 'belum', 'sarjana'),
    ('Mahmudi Anshari', '500032760078160024', 2016, 'L', 'iya', 'sarjana'),
    ('Maulida Rohmatul Fadhilla', '500032760078160025', 2016, 'P', 'iya', 'sarjana'),
    ('Mauludi Afina Mirza', '500032760078160026', 2016, 'P', 'belum', 'sarjana'),
    ('Miftahul Jannah', '500032760078160027', 2016, 'P', 'belum', 'sarjana'),
    ('Miranda Dwi Astuti', '500032760078160028', 2016, 'P', 'iya', 'sarjana'),
    ('Mohamad Alfin Ramadhani', '500032760078160029', 2016, 'L', 'iya', 'sarjana'),
    ('Mohammad Fajri Assalam', '500032760078160030', 2016, 'L', 'iya', 'sarjana'),
    ('Muhammad Anwar Fikri', '500032760078160031', 2016, 'L', 'belum', 'sarjana'),
    ('Muhammad Ghulam Fathi', '500032760078160032', 2016, 'L', 'iya', 'sarjana'),
    ('Muhammad Hadi Pursai', '500032760078160033', 2016, 'L', 'belum', 'sarjana'),
    ('Muhammad Rafly', '500032760078160034', 2016, 'L', 'iya', 'sarjana'),
    ('Muhammad Salman Ar Rifqi', '500032760078160035', 2016, 'L', 'tugasan', 'sarjana'),
    ('Muhammad Rheza Hilfaziyan Lubis', '500032760078160036', 2016, 'L', 'iya', 'sarjana'),
    ('Muhammad Zulfan At Tirmidzi', '500032760078160037', 2016, 'L', 'belum', 'sarjana'),
    ('Negi Aditya Rahadian', '500032760078160038', 2016, 'L', 'belum', 'sarjana'),
    ('Nia Varamita', '500032760078160039', 2016, 'P', 'belum', 'sarjana'),
    ('Niken Pangesti', '500032760078160040', 2016, 'P', 'iya', 'sarjana'),
    ('Nur Aghniatun Sholiha', '500032760078160041', 2016, 'P', 'iya', 'sarjana'),
    ('Panji Nugraha', '500032760078160042', 2016, 'L', 'belum', 'sarjana'),
    ('Ridwan Hadi Kusuma', '500032760078160043', 2016, 'L', 'iya', 'sarjana'),
    ('Roihana Alya Nabilah', '500032760078160044', 2016, 'P', 'belum', 'sarjana'),
    ('Sefi Aulia Husein', '500032760078160045', 2016, 'P', 'iya', 'sarjana'),
    ('Shafira Aristianti', '500032760078160046', 2016, 'P', 'belum', 'sarjana'),
    ('Sheila Fajarina Safety', '500032760078160047', 2016, 'P', 'belum', 'sarjana'),
    ('Siska Dewi', '500032760078160048', 2016, 'P', 'belum', 'sarjana'),
    ('Surya Pradana Adipatiarga', '500032760078160049', 2016, 'L', 'belum', 'belum'),
    ('Tomy Wilian', '500032760078160050', 2016, 'L', 'tugasan', 'sarjana'),
    ('Viola Septia Irfanda', '500032760078160051', 2016, 'P', 'iya', 'sarjana'),
    ('Widia Karnikafa', '500032760078160052', 2016, 'P', 'iya', 'sarjana'),
    ('Winda Rahayu Pratiwi', '500032760078160053', 2016, 'P', 'belum', 'sarjana'),
    ('Yudistyan Farichal Audry', '500032760078160054', 2016, 'L', 'belum', 'belum'),
    ('Zidnih Rahmah', '500032760078160055', 2016, 'P', 'belum', 'sarjana'),
    ('Annisa Nabilah Nugroho', '500032760078160056', 2016, 'P', 'belum', 'belum'),
    ('Adam Fauzan', '500032760078170001', 2017, 'L', 'belum', 'sarjana'),
    ('Adhi Rizqi Alfaqih', '500032760078170002', 2017, 'L', 'belum', 'sarjana'),
    ('Adhianto Leksono', '500032760078170003', 2017, 'L', 'belum', 'kuliah'),
    ('Agni Nurlaila Kusumaningdyah', '500032760078170004', 2017, 'P', 'belum', 'sarjana'),
    ('Agung Rizki Satria', '500032760078170005', 2017, 'L', 'iya', 'sarjana'),
    ('Ahmad Zohir Fatakhu Lubis Saputra', '500032760078170006', 2017, 'L', 'tugasan', 'belum'),
    ('Ananda Sabiila Rosyada', '500032760078170007', 2017, 'P', 'iya', 'sarjana'),
    ('Anjani Eka Lestari', '500032760078170008', 2017, 'P', 'belum', 'sarjana'),
    ('Annisa Faradilla Ulfa', '500032760078170009', 2017, 'P', 'iya', 'sarjana'),
    ('Bunga Anastasia', '500032760078170010', 2017, 'P', 'belum', 'sarjana'),
    ('Catur Satria Adhi Fauzi', '500032760078170011', 2017, 'L', 'tugasan', 'sarjana'),
    ('Dede Satria Rangga', '500032760078170012', 2017, 'L', 'belum', 'sarjana'),
    ('Dinda Aulia Zulkarnain', '500032760078170013', 2017, 'P', 'belum', 'sarjana'),
    ('Dinna Ayu Sekarwangi', '500032760078170014', 2017, 'P', 'belum', 'sarjana'),
    ('Diva Aulia Syafira', '500032760078170015', 2017, 'P', 'belum', 'sarjana'),
    ('Fadil At-Taubah', '500032760078170016', 2017, 'L', 'belum', 'belum'),
    ('Fadillah Indah Nuraini', '500032760078170017', 2017, 'P', 'iya', 'sarjana'),
    ('Farhan Ashar Haryudha', '500032760078170018', 2017, 'L', 'belum', 'sarjana'),
    ('Fauzul Fadli', '500032760078170019', 2017, 'L', 'iya', 'sarjana'),
    ('Fitri Annisa Ahlul Jannah', '500032760078170020', 2017, 'P', 'belum', 'sarjana'),
    ('Galang Alfian Saujana', '500032760078170021', 2017, 'L', 'belum', 'belum'),
    ('Galih Damar Dwiatmaka', '500032760078170022', 2017, 'L', 'belum', 'sarjana'),
    ('Ginindha Izzati Sabila', '500032760078170023', 2017, 'P', 'belum', 'sarjana'),
    ('Hegina Salshabila Azani', '500032760078170024', 2017, 'P', 'belum', 'sarjana'),
    ('Helma Justia Feyruzi', '500032760078170025', 2017, 'P', 'belum', 'sarjana'),
    ('Hirzi Syarifuddin', '500032760078170026', 2017, 'L', 'belum', 'sarjana'),
    ('Kukuh Prio Pambudi', '500032760078170027', 2017, 'L', 'belum', 'sarjana'),
    ('Lola Miftahul Fidini', '500032760078170028', 2017, 'P', 'iya', 'sarjana'),
    ('Marsella Khairunnisa', '500032760078170029', 2017, 'P', 'belum', 'sarjana'),
    ('Mohammad Rizki Akbarrollah Sularno', '500032760078170030', 2017, 'L', 'belum', 'sarjana'),
    ('Muhamad Rival Priatama', '500032760078170031', 2017, 'L', 'belum', 'sarjana'),
    ('Muhammad Adiprawira Rudiyat', '500032760078170032', 2017, 'L', 'belum', 'sarjana'),
    ('Muhammad Hanif Budiman', '500032760078170033', 2017, 'L', 'belum', 'sarjana'),
    ('Muhammad Rifqi Alamsyah', '500032760078170034', 2017, 'L', 'belum', 'sarjana'),
    ('Nararya Thirafi Arinta', '500032760078170035', 2017, 'L', 'belum', 'sarjana'),
    ('Navany Bilqisthy', '500032760078170036', 2017, 'P', 'belum', 'sarjana'),
    ('Niswatin Thoyibah', '500032760078170037', 2017, 'P', 'belum', 'sarjana'),
    ('Nur Gina Hasanah', '500032760078170038', 2017, 'P', 'iya', 'sarjana'),
    ('Raald Afan Abdillah', '500032760078170039', 2017, 'L', 'belum', 'sarjana'),
    ('Raya Makarim Penantian', '500032760078170040', 2017, 'P', 'belum', 'sarjana'),
    ('Reza Wahyudi', '500032760078170041', 2017, 'L', 'belum', 'sarjana'),
    ('Risky Putra', '500032760078170042', 2017, 'L', 'belum', 'sarjana'),
    ('Rizaldi Muflih Santoso', '500032760078170043', 2017, 'L', 'belum', 'sarjana'),
    ('Rizki Azi Syaputra', '500032760078170044', 2017, 'L', 'belum', 'sarjana'),
    ('Savira Auria Haviva', '500032760078170045', 2017, 'P', 'belum', 'sarjana'),
    ('Sultan Aris Arramadhan', '500032760078170046', 2017, 'L', 'iya', 'belum'),
    ('Unggul Yudanira', '500032760078170047', 2017, 'L', 'iya', 'sarjana'),
    ('Jihan Putri Sekar Arum', '500032760078170048', 2017, 'P', 'iya', 'sarjana'),
    ('Laily Rahmatika Khomsah', '500032760078170049', 2017, 'P', 'belum', 'sarjana'),
    ('Puspita Handayani', '500032760078170050', 2017, 'P', 'belum', 'sarjana'),
    ('Ahmat Bagus Kuncoro', '500032760078180001', 2018, 'L', 'belum', 'sarjana'),
    ('Astrid Budi Ati', '500032760078180002', 2018, 'P', 'belum', 'sarjana'),
    ('Bilqis Royyan Firdaus', '500032760078180003', 2018, 'P', 'iya', 'sarjana'),
    ('Bukhori Fathullah Al Husna', '500032760078180004', 2018, 'L', 'iya', 'sarjana'),
    ('Cindy Novita Aulia Putri', '500032760078180005', 2018, 'P', 'belum', 'sarjana'),
    ('Dwi Suci Rahmawati', '500032760078180006', 2018, 'P', 'iya', 'sarjana'),
    ('Emyr Reyhan Wijaya', '500032760078180007', 2018, 'L', 'belum', 'sarjana'),
    ('Faisal Amir Maz', '500032760078180008', 2018, 'L', 'iya', 'sarjana'),
    ('Farel Al Rasyid', '500032760078180009', 2018, 'L', 'belum', 'sarjana'),
    ('Fuad Mudzakir', '500032760078180010', 2018, 'L', 'iya', 'sarjana'),
    ('Herlia Alifiah', '500032760078180011', 2018, 'P', 'belum', 'sarjana'),
    ('Ilham M Faqih A', '500032760078180012', 2018, 'L', 'belum', 'sarjana'),
    ('Irna Dwi Indriyani', '500032760078180013', 2018, 'P', 'belum', 'sarjana'),
    ('M Fajar Ibrahim', '500032760078180014', 2018, 'L', 'iya', 'sarjana'),
    ('Naufal Abdullah Fawwas Kamal', '500032760078180015', 2018, 'L', 'iya', 'sarjana'),
    ('Nida Setianingrum', '500032760078180016', 2018, 'P', 'belum', 'sarjana'),
    ('Novita Royyan Kusuma Dewi', '500032760078180017', 2018, 'P', 'belum', 'sarjana'),
    ('Nurul Akhyuri Utami', '500032760078180018', 2018, 'P', 'iya', 'sarjana'),
    ('Rafida Ramadhanty', '500032760078180019', 2018, 'P', 'belum', 'sarjana'),
    ('Riska Khoiruningrum', '500032760078180020', 2018, 'P', 'belum', 'sarjana'),
    ('Risma Rosalia', '500032760078180021', 2018, 'P', 'iya', 'sarjana'),
    ('Sherly Oliviadela', '500032760078180022', 2018, 'P', 'belum', 'sarjana'),
    ('Shita Laila Nurjanah', '500032760078180023', 2018, 'P', 'belum', 'belum'),
    ('Tengku Fatimah Azhara', '500032760078180024', 2018, 'P', 'iya', 'sarjana'),
    ('Uswatun Khasanah Enggar S', '500032760078180025', 2018, 'P', 'iya', 'sarjana'),
    ('Via Anggreani', '500032760078180026', 2018, 'P', 'belum', 'sarjana'),
    ('Ziyadiani Fadilla', '500032760078180027', 2018, 'P', 'belum', 'sarjana'),
    ('Wildan Fatchu Rizqi', '500032760078180028', 2018, 'L', 'belum', 'sarjana'),
    ('Alhazmi Fadillah', '500032760078190001', 2019, 'L', 'belum', 'sarjana'),
    ('Alifah Azka Nisrina', '500032760078190002', 2019, 'P', 'iya', 'sarjana'),
    ('Alifya Addina', '500032760078190003', 2019, 'P', 'iya', 'sarjana'),
    ('Anindya Zakiyya Qurrota A''yun', '500032760078190004', 2019, 'P', 'iya', 'sarjana'),
    ('Annisa Aufa Dina Lathifa', '500032760078190005', 2019, 'P', 'iya', 'sarjana'),
    ('Annisa Nurhidayati', '500032760078190006', 2019, 'P', 'belum', 'sarjana'),
    ('Ardhi Abdul Malik', '500032760078190007', 2019, 'L', 'belum', 'sarjana'),
    ('Bagus Dewanto Nur Sabiliy', '500032760078190008', 2019, 'L', 'iya', 'sarjana'),
    ('Claresta Aptarini', '500032760078190009', 2019, 'P', 'belum', 'sarjana'),
    ('Dara Melinda Hepta', '500032760078190010', 2019, 'P', 'iya', 'sarjana'),
    ('David Muhammad Aldani', '500032760078190011', 2019, 'L', 'iya', 'sarjana'),
    ('Denis Al Malik Aziz', '500032760078190012', 2019, 'L', 'belum', 'sarjana'),
    ('Dimas Al Malik Aziz', '500032760078190013', 2019, 'L', 'belum', 'sarjana'),
    ('Evilia Rahmawati', '500032760078190014', 2019, 'P', 'belum', 'sarjana'),
    ('Fannisa Agustina Yohandi', '500032760078190015', 2019, 'P', 'belum', 'sarjana'),
    ('Fathurrizki Emir Elkarim', '500032760078190016', 2019, 'L', 'belum', 'sarjana'),
    ('Febilkis Noor Rachma Kisma', '500032760078190017', 2019, 'P', 'belum', 'sarjana'),
    ('Felita Aulia Antoni Griselda', '500032760078190018', 2019, 'P', 'belum', 'sarjana'),
    ('Givon Fatakhul Khisan', '500032760078190019', 2019, 'L', 'iya', 'sarjana'),
    ('Hayuning Widiastuti', '500032760078190020', 2019, 'P', 'iya', 'sarjana'),
    ('Isnaini Meiannaristi', '500032760078190021', 2019, 'P', 'iya', 'sarjana'),
    ('Jannie Aldriana Shofia', '500032760078190022', 2019, 'P', 'iya', 'sarjana'),
    ('Jodhistira Sarwa Adhigana', '500032760078190023', 2019, 'L', 'belum', 'sarjana'),
    ('Jordan Fadillah Izma', '500032760078190024', 2019, 'L', 'belum', 'sarjana'),
    ('Laksmita Yuniarza', '500032760078190025', 2019, 'P', 'belum', 'sarjana'),
    ('Maheswary Mufidda Brinzqy Almaahi', '500032760078190026', 2019, 'P', 'belum', 'sarjana'),
    ('Marsa Nur Alifah', '500032760078190027', 2019, 'P', 'belum', 'sarjana'),
    ('Mita Novela', '500032760078190028', 2019, 'P', 'belum', 'sarjana'),
    ('Muhamad Faisal Majid', '500032760078190029', 2019, 'L', 'belum', 'sarjana'),
    ('Muhammad Dimas Trilaksono', '500032760078190030', 2019, 'L', 'belum', 'sarjana'),
    ('Muhammad Sulton Aulia', '500032760078190031', 2019, 'L', 'belum', 'sarjana'),
    ('Pandu Tri Wibowo', '500032760078190032', 2019, 'L', 'belum', 'sarjana'),
    ('Putri Adistya Asj''ary', '500032760078190033', 2019, 'P', 'belum', 'sarjana'),
    ('Rafa Nadia Farahani', '500032760078190034', 2019, 'P', 'belum', 'sarjana'),
    ('Raisa Nur Afifah', '500032760078190035', 2019, 'P', 'belum', 'sarjana'),
    ('Ridwan Toyyibun', '500032760078190036', 2019, 'L', 'belum', 'sarjana'),
    ('Rizqi Qowiyu', '500032760078190037', 2019, 'P', 'iya', 'sarjana'),
    ('Rudy Salam Warsono Putra', '500032760078190038', 2019, 'L', 'belum', 'belum'),
    ('Said Arina Hendra', '500032760078190039', 2019, 'L', 'belum', 'sarjana'),
    ('Salsabila Minarbika', '500032760078190040', 2019, 'P', 'belum', 'sarjana'),
    ('Salsabila Iza Fanida', '500032760078190041', 2019, 'P', 'belum', 'sarjana'),
    ('Shanti Nur A''bidah S', '500032760078190042', 2019, 'P', 'iya', 'sarjana'),
    ('Alfay Anshorulloh Majid', '500032760078190043', 2019, 'L', 'belum', 'sarjana'),
    ('Arya Guntur Syihabuddin', '500032760078190044', 2019, 'L', 'belum', 'kuliah'),
    ('Azka Dini Yuntari', '500032760078190045', 2019, 'P', 'belum', 'sarjana'),
    ('Cornelis Banu Shoumi Illyin Pangestu', '500032760078190046', 2019, 'L', 'belum', 'sarjana'),
    ('Hana Sabila Yasaro', '500032760078190047', 2019, 'P', 'belum', 'sarjana'),
    ('Indah Kurnia Putri', '500032760078190048', 2019, 'P', 'belum', 'sarjana'),
    ('Mustajib', '500032760078190049', 2019, 'L', 'iya', 'sarjana'),
    ('Rayhan Janatama', '500032760078190050', 2019, 'L', 'iya', 'sarjana'),
    ('Reffi Ramadhan', '500032760078190051', 2019, 'L', 'iya', 'sarjana'),
    ('Rizki Hidayat Prasetyo', '500032760078190052', 2019, 'L', 'iya', 'sarjana'),
    ('Yasfa Azkaa Wiwaha', '500032760078190053', 2019, 'P', 'belum', 'sarjana'),
    ('Ahmad Fhaqih Adang', '500032760078200001', 2020, 'L', 'belum', 'sarjana'),
    ('Alma Tiara Cantika', '500032760078200002', 2020, 'P', 'iya', 'sarjana'),
    ('Andre Royan Saputra', '500032760078200003', 2020, 'L', 'iya', 'sarjana'),
    ('Arga Fahrizal Perwira', '500032760078200004', 2020, 'L', 'belum', 'sarjana'),
    ('Bian Nur Solichin', '500032760078200005', 2020, 'L', 'belum', 'belum'),
    ('Bulan Kartika Maharani', '500032760078200006', 2020, 'P', 'belum', 'sarjana'),
    ('Dennisa Arofani', '500032760078200007', 2020, 'P', 'belum', 'sarjana'),
    ('Dita Hassifa Asmarawati', '500032760078200008', 2020, 'P', 'iya', 'sarjana'),
    ('Emir Muhamad Zaid', '500032760078200009', 2020, 'L', 'tugasan', 'sarjana'),
    ('Fauzan Andri', '500032760078200010', 2020, 'L', 'iya', 'sarjana'),
    ('Indah Prasetyarini', '500032760078200011', 2020, 'P', 'belum', 'sarjana'),
    ('Indra Gunawan', '500032760078200012', 2020, 'L', 'belum', 'kuliah'),
    ('Jihan Syifa Prayudipta', '500032760078200013', 2020, 'P', 'belum', 'sarjana'),
    ('Kheyyana Izzudinovic Haque', '500032760078200014', 2020, 'P', 'belum', 'sarjana'),
    ('Kireyna Salsabila Gunawan', '500032760078200015', 2020, 'P', 'belum', 'sarjana'),
    ('Laverda Raul Firdaus', '500032760078200016', 2020, 'L', 'belum', 'kuliah'),
    ('M. Anwar Hadid Putra Hidayat', '500032760078200017', 2020, 'L', 'belum', 'sarjana'),
    ('M. Farid Arrosid', '500032760078200018', 2020, 'L', 'belum', 'sarjana'),
    ('M. Iqbal Firmansyah', '500032760078200019', 2020, 'L', 'iya', 'kuliah'),
    ('Mikael Alvian Rizky', '500032760078200020', 2020, 'L', 'belum', 'sarjana'),
    ('Muhammad Hawari Ibadillah', '500032760078200021', 2020, 'L', 'belum', 'sarjana'),
    ('Nabil Choiri', '500032760078200022', 2020, 'L', 'iya', 'sarjana'),
    ('Nadiva Regita Anjani', '500032760078200023', 2020, 'P', 'belum', 'sarjana'),
    ('Nolland Zidane Tamar Dandika', '500032760078200024', 2020, 'L', 'belum', 'sarjana'),
    ('Novi Yusfita', '500032760078200025', 2020, 'P', 'iya', 'sarjana'),
    ('Rini Isnaini Khoirunnisa', '500032760078200026', 2020, 'P', 'belum', 'sarjana'),
    ('Salma Sita Rosulina', '500032760078200027', 2020, 'P', 'belum', 'sarjana'),
    ('Salwa Rubia Darussalam', '500032760078200028', 2020, 'P', 'iya', 'sarjana'),
    ('Sania Rizqi Maharani', '500032760078200029', 2020, 'P', 'belum', 'sarjana'),
    ('Sekar Adelia Putri', '500032760078200030', 2020, 'P', 'belum', 'sarjana'),
    ('Ummu Humairoh', '500032760078200031', 2020, 'P', 'iya', 'sarjana'),
    ('Ziki Izamil Haq', '500032760078200032', 2020, 'P', 'iya', 'sarjana'),
    ('Abiyu Iqbal Maulana', '500032760078210001', 2021, 'L', 'belum', 'kuliah'),
    ('Anastasia Ajeng Vilallba', '500032760078210002', 2021, 'P', 'belum', 'kuliah'),
    ('Anshor Nugroho', '500032760078210003', 2021, 'L', NULL, NULL),
    ('Bima Sakti Sumbaga', '500032760078210004', 2021, 'L', NULL, NULL),
    ('Muhammad Fatkhul Farizi', '500032760078210005', 2021, 'L', 'tugasan', 'kuliah'),
    ('Daru Rahayu', '500032760078210006', 2021, 'P', 'belum', 'sarjana'),
    ('Devina Fitri Handayani', '500032760078210007', 2021, 'P', 'belum', 'sarjana'),
    ('Dinda aulia', '500032760078210008', 2021, 'P', 'belum', 'kuliah'),
    ('Ghulam Muhammad Firdaus', '500032760078210009', 2021, 'L', 'tugasan', 'kuliah'),
    ('Ilham Syakur Fidina', '500032760078210010', 2021, 'L', 'belum', 'sarjana'),
    ('Iman Kurnia', '500032760078210011', 2021, 'L', 'belum', 'kuliah'),
    ('Jihan Maisaroh', '500032760078210012', 2021, 'P', 'belum', 'kuliah'),
    ('Krida Wahyu Khoirunnisa', '500032760078210013', 2021, 'P', 'belum', 'sarjana'),
    ('Muhammad Jourdan Febriansyah', '500032760078210014', 2021, 'L', 'belum', 'sarjana'),
    ('Nienda Biellani', '500032760078210015', 2021, 'P', 'tugasan', 'kuliah'),
    ('Ponco Prakoso', '500032760078210016', 2021, 'L', 'belum', NULL),
    ('Rafi indra karunia', '500032760078210017', 2021, 'L', 'iya', NULL),
    ('Rianda Dawud Abdurrohman', '500032760078210018', 2021, 'L', 'belum', NULL),
    ('Rofy Candra Rusdiana', '500032760078210019', 2021, 'L', 'belum', NULL),
    ('Salma Labibah', '500032760078210020', 2021, 'P', 'belum', NULL),
    ('Satrio Bhaskara Adi Pradana', '500032760078210021', 2021, 'L', 'belum', NULL),
    ('Silvia Agustina', '500032760078210022', 2021, 'P', 'belum', NULL),
    ('Syakila Fidina Auzi', '500032760078210023', 2021, 'P', 'iya', NULL),
    ('Wahyulia Widya Lestari', '500032760078210024', 2021, 'P', 'belum', NULL),
    ('Zahra Andini Salsabila', '500032760078210025', 2021, 'P', 'belum', NULL),
    ('Zakiya Rozqi Auliya''', '500032760078210026', 2021, 'P', 'belum', NULL),
    ('Zalfaa Suris Desfianti', '500032760078210027', 2021, 'P', 'belum', NULL),
    ('Muhammad Isaura Priyadi Putra', '500032760078210028', 2021, 'L', 'belum', NULL),
    ('Salsabila Kintan Azzahra', '500032760078210029', 2021, 'P', 'iya', NULL),
    ('Muhamad Rafly Fazri', '500032760078210030', 2021, 'L', 'belum', NULL),
    ('Akbar Gunawan', '500032760078210031', 2021, 'L', 'belum', NULL),
    ('Assof Silahudin Abdulah Haci', '500032760078210032', 2021, 'L', 'belum', NULL),
    ('Putri Chairunnisa', '500032760078210033', 2021, 'P', 'belum', NULL),
    ('Ismail Zhanfeari', '500032760078210034', 2021, 'L', 'belum', NULL),
    ('Elen Pramai Sella', '500032760078210035', 2021, 'P', 'belum', NULL),
    ('Adinka Meylani Wulandari', '500032760078220001', 2022, 'P', 'belum', NULL),
    ('Ananda Naufalindo Dava', '500032760078220002', 2022, 'L', 'belum', NULL),
    ('Anisa Nur Fadhila', '500032760078220003', 2022, 'P', 'belum', NULL),
    ('Aprilia Najwa Nafifah', '500032760078220004', 2022, 'P', 'belum', NULL),
    ('Barron Breviantho', '500032760078220005', 2022, 'L', 'belum', NULL),
    ('Cinta Rizkia Fernanda', '500032760078220006', 2022, 'P', 'belum', NULL),
    ('Cucu Auliya Rama Putri', '500032760078220007', 2022, 'P', 'iya', NULL),
    ('Daniah Kholda', '500032760078220008', 2022, 'P', 'belum', NULL),
    ('Diaz Sabrina Nurafifa', '500032760078220009', 2022, 'P', 'belum', NULL),
    ('Dinda Hamidah', '500032760078220010', 2022, 'P', 'iya', NULL),
    ('Fa''aza Siva''a Fitri Raafiah', '500032760078220011', 2022, 'P', 'belum', NULL),
    ('Faqihuddin', '500032760078220012', 2022, 'L', 'belum', NULL),
    ('Febrialam Akbar Sanjaya', '500032760078220013', 2022, 'L', 'belum', NULL),
    ('Galang Nurzaman', '500032760078220014', 2022, 'L', 'belum', NULL),
    ('Irvian Zakky Marta', '500032760078220015', 2022, 'L', 'belum', NULL),
    ('Jasmine Adhisty Fiqannawati', '500032760078220016', 2022, 'P', 'belum', NULL),
    ('Kessya Melvy Ananda', '500032760078220017', 2022, 'P', 'belum', NULL),
    ('Khafiludin Ulil Fauzan Arroz', '500032760078220018', 2022, 'L', 'belum', NULL),
    ('Maya Nurfadhila', '500032760078220019', 2022, 'P', 'belum', NULL),
    ('Muhamad Satria Budi Bintang', '500032760078220020', 2022, 'L', 'belum', NULL),
    ('Muhammad Jannatan Firdaus', '500032760078220021', 2022, 'L', 'iya', NULL),
    ('Muhammad Rizki Fadlan Riantono', '500032760078220022', 2022, 'L', 'belum', NULL),
    ('Muhammad Sani Fatra', '500032760078220023', 2022, 'L', 'iya', NULL),
    ('Nurul Awaliah', '500032760078220024', 2022, 'P', 'iya', NULL),
    ('Qissisin Aina Weqolby', '500032760078220025', 2022, 'L', 'belum', NULL),
    ('Rafa Nadia Farahani', '500032760078220026', 2022, 'L', 'belum', NULL),
    ('Rafadhila Pramudita Hamadi', '500032760078220027', 2022, 'L', 'belum', NULL),
    ('Raihan Muhammad Naufal', '500032760078220028', 2022, 'L', 'belum', NULL),
    ('Revalina Rizki Andiani', '500032760078220029', 2022, 'P', 'belum', NULL),
    ('Rini Isnaini Khoirunnisa', '500032760078220030', 2022, 'P', 'belum', NULL),
    ('Riva syifa aulia walidayn', '500032760078220031', 2022, 'P', 'belum', NULL),
    ('Salwahani Marjanis Nashr', '500032760078220032', 2022, 'P', 'belum', NULL),
    ('Satrio Jati Pamungkas', '500032760078220033', 2022, 'L', 'belum', NULL),
    ('Syach Khurin Mubalighotuzahra', '500032760078220034', 2022, 'P', 'iya', NULL),
    ('Syafira Eliza Firdaus', '500032760078220035', 2022, 'P', 'belum', NULL),
    ('Visca Chaerunnisa Bachri', '500032760078220036', 2022, 'P', 'iya', NULL),
    ('Ihza Tiffani Nurhaliza', '500032760078220037', 2022, 'P', 'iya', NULL),
    ('Clarissa Elsarina', '500032760078220038', 2022, 'P', 'belum', NULL),
    ('Kalingga Kencana Luhur Abadi', '500032760078220039', 2022, 'L', 'belum', NULL),
    ('Muhammad Baihaqi', '500032760078220040', 2022, 'L', 'iya', NULL),
    ('Dazza Ghazy Alhakim', '500032760078220041', 2022, 'L', 'belum', NULL),
    ('Alifya Wandha Putri', '500032760078220042', 2022, 'P', 'iya', NULL),
    ('Nanang Syaifudin', '500032760078220043', 2022, 'L', 'belum', NULL),
    ('Sarah Salsabila', '500032760078220044', 2022, 'P', 'belum', NULL),
    ('Abira Wisnunggal', '500032760078230001', 2023, 'L', 'belum', NULL),
    ('Alya Husna', '500032760078230002', 2023, 'P', 'belum', NULL),
    ('Andyana Habrizi Aqsha', '500032760078230003', 2023, 'L', 'belum', NULL),
    ('Axel Wilson', '500032760078230004', 2023, 'L', 'belum', NULL),
    ('Baskoro Bayu Baruno', '500032760078230005', 2023, 'L', 'belum', NULL),
    ('Cantik Suci Arilla', '500032760078230006', 2023, 'P', 'belum', NULL),
    ('Chiquita Labitta', '500032760078230007', 2023, 'P', 'belum', NULL),
    ('Denia Asha Rushdina', '500032760078230008', 2023, 'P', 'belum', NULL),
    ('Faza Rizky Widyatama', '500032760078230009', 2023, 'L', 'belum', NULL),
    ('Ginirza Izzati Sabila', '500032760078230010', 2023, 'P', 'belum', NULL),
    ('Ginton Afnanin Khoir', '500032760078230011', 2023, 'L', 'belum', NULL),
    ('Hana Khalisa Zahra', '500032760078230012', 2023, 'P', 'belum', NULL),
    ('Hanif Al Ubaidani', '500032760078230013', 2023, 'L', 'belum', NULL),
    ('Hoa Hayun Bintari', '500032760078230014', 2023, 'P', 'belum', NULL),
    ('Ikhsan Jordan Dwi Putra', '500032760078230015', 2023, 'L', 'belum', NULL),
    ('Jwan Ahmad Lintang Pawikan', '500032760078230016', 2023, 'L', 'belum', NULL),
    ('Kilta Aufa Qorina', '500032760078230017', 2023, 'P', 'belum', NULL),
    ('Larasati Disralyndi', '500032760078230018', 2023, 'P', 'belum', NULL),
    ('Matsna Aura Sabila', '500032760078230019', 2023, 'P', 'belum', NULL),
    ('Mirza Athallah Salman', '500032760078230020', 2023, 'L', 'belum', NULL),
    ('Muchamad Faridh Alfafa', '500032760078230021', 2023, 'L', 'belum', NULL),
    ('Muhammad Fikri Asadillah Ilhamsyah', '500032760078230022', 2023, 'L', 'iya', NULL),
    ('Muhammad Handriano Marceleno', '500032760078230023', 2023, 'L', 'iya', NULL),
    ('Muhammad Irsyan Ghothfan', '500032760078230024', 2023, 'L', 'belum', NULL),
    ('Muhammad Yafaz Fabian', '500032760078230025', 2023, 'L', 'iya', NULL),
    ('Nur Fazriyanda Rifqi Alhafiz', '500032760078230026', 2023, 'L', 'belum', NULL),
    ('Qynata Hurryn Ainayya', '500032760078230027', 2023, 'P', 'belum', NULL),
    ('Reyhan Abdilah Mabruri', '500032760078230028', 2023, 'L', 'belum', NULL),
    ('Tiara Aziza', '500032760078230029', 2023, 'P', 'iya', NULL),
    ('Zulfa Nur Aini Putri', '500032760078230030', 2023, 'P', 'belum', NULL),
    ('Adriana Anjamaniz', '500032760078230031', 2023, 'P', 'belum', NULL),
    ('Gracia Kayla Sujarwadi', '500032760078230032', 2023, 'P', 'belum', NULL),
    ('Muhammad Akyas Rifki Fernando', '500032760078230033', 2023, 'L', 'belum', NULL),
    ('Muhammad Beri Al Fauzu', '500032760078230034', 2023, 'L', 'belum', NULL),
    ('Zulfadhli Mahardika', '500032760078230035', 2023, 'L', 'belum', NULL),
    ('Rahis Galih Pramudya Darmawan', '500032760078230036', 2023, 'L', 'belum', NULL),
    ('Mushab Hirson Firdaus', '500032760078230037', 2023, 'L', 'belum', NULL),
    ('Aan Adriyana', '500032760078240001', 2024, 'L', NULL, NULL),
    ('Abdulloh Hasan Al Kahfi', '500032760078240002', 2024, 'L', NULL, NULL),
    ('Adinda Nur Fathiya, S.Farm', '500032760078240003', 2024, 'P', NULL, NULL),
    ('Adnan Windfall Al Choiri', '500032760078240004', 2024, 'L', NULL, NULL),
    ('Akhmad Aminulloh Aldi Fadilah', '500032760078240005', 2024, 'L', NULL, NULL),
    ('Anis Adriyani', '500032760078240006', 2024, 'P', NULL, NULL),
    ('Ardhito Faza Akhnaf', '500032760078240007', 2024, 'L', 'iya', NULL),
    ('Danil Abdul Azis', '500032760078240008', 2024, 'L', NULL, NULL),
    ('Givaniora Azzahra', '500032760078240009', 2024, 'P', 'iya', NULL),
    ('Hanni Rezky Azzahra', '500032760078240010', 2024, 'P', NULL, NULL),
    ('Haris Azzahra Lunaaya', '500032760078240011', 2024, 'P', NULL, NULL),
    ('Laetitia Kayla Alika', '500032760078240012', 2024, 'P', NULL, NULL),
    ('Muhammad Rizky Akbar', '500032760078240013', 2024, 'L', 'iya', NULL),
    ('Muhammad Zaidan Aliyuddin', '500032760078240014', 2024, 'L', 'iya', NULL),
    ('Mutiara Rahma Waris', '500032760078240015', 2024, 'P', NULL, NULL),
    ('Neal Guarddin', '500032760078240016', 2024, 'L', NULL, NULL),
    ('Novia Putri Ramadhani', '500032760078240017', 2024, 'P', NULL, NULL),
    ('Prabandaru Nurizza Daksa Buwana', '500032760078240018', 2024, 'L', 'iya', NULL),
    ('Rainer Adityatama', '500032760078240019', 2024, 'L', NULL, NULL),
    ('Rara Cahya Putri', '500032760078240020', 2024, 'P', NULL, NULL),
    ('Zulfi Ana Pratiwi', '500032760078240021', 2024, 'P', NULL, NULL),
    ('Addiena Haqqi', '500032760078240022', 2024, 'P', NULL, NULL),
    ('Aghnia Sahala Rizky', '500032760078240023', 2024, 'P', NULL, NULL),
    ('Alina Prafasya Laili Ramadhani', '500032760078240024', 2024, 'P', NULL, NULL),
    ('Aufa Aulia Ulhaq', '500032760078240025', 2024, 'P', NULL, NULL),
    ('Bacharin Masandi Sanursen', '500032760078240026', 2024, 'L', NULL, NULL),
    ('Bara Wiguna Pangestu', '500032760078240027', 2024, 'L', NULL, NULL),
    ('Fahmi Putra Wibowo', '500032760078240028', 2024, 'L', 'iya', NULL),
    ('Fandi Ahmad', '500032760078240029', 2024, 'P', NULL, NULL),
    ('Fathonah Azka Sakhiyyah', '500032760078240030', 2024, 'L', NULL, NULL),
    ('Hady Firdaus', '500032760078240031', 2024, 'P', NULL, NULL),
    ('Hafida Hasna Fitrina', '500032760078240032', 2024, 'P', 'iya', NULL),
    ('Nadine Samiya Rahmadhani', '500032760078240033', 2024, 'P', NULL, NULL),
    ('Nayla Malika Anjani', '500032760078240034', 2024, 'P', NULL, NULL),
    ('Novita Syaifani', '500032760078240035', 2024, 'P', NULL, NULL),
    ('Syafika Bilkis', '500032760078240036', 2024, 'P', NULL, NULL),
    ('Nabila Atika Rohmah', '500032760078240037', 2024, 'P', 'iya', NULL),
    ('Abdul Malik', '500032760078250001', 2025, 'L', NULL, NULL),
    ('Adis Heksa Ismadi', '500032760078250002', 2025, 'P', 'iya', NULL),
    ('Aisha Azkiya Raihana', '500032760078250003', 2025, 'L', NULL, NULL),
    ('Alfian Istawa Dinaza', '500032760078250004', 2025, 'L', NULL, NULL),
    ('Alin Khoirunnisa', '500032760078250005', 2025, 'P', NULL, NULL),
    ('Alka Dewa', '500032760078250006', 2025, 'L', NULL, NULL),
    ('Arrifanisa Fauzia', '500032760078250007', 2025, 'P', 'iya', NULL),
    ('Brian Faiz Zulkarnain', '500032760078250008', 2025, 'L', NULL, NULL),
    ('Daniar Arfa Qanitah', '500032760078250009', 2025, 'P', NULL, NULL),
    ('Dimas Maulana Hafiez', '500032760078250010', 2025, 'L', NULL, NULL),
    ('Fatimah Zahra Choirunnisa', '500032760078250011', 2025, 'P', NULL, NULL),
    ('Ferdian Satria Rachman', '500032760078250012', 2025, 'L', NULL, NULL),
    ('Kenaz Shidqi Baswara', '500032760078250013', 2025, 'L', NULL, NULL),
    ('Khulwa Arika Resti', '500032760078250014', 2025, 'P', NULL, NULL),
    ('M Daffa Rasi A', '500032760078250015', 2025, 'L', 'iya', NULL),
    ('Mohammad Faiz Masyhur', '500032760078250016', 2025, 'L', NULL, NULL),
    ('Mourabel Indra Aulia', '500032760078250017', 2025, 'P', NULL, NULL),
    ('Muhammad Rendy Saputra', '500032760078250018', 2025, 'L', 'iya', NULL),
    ('Muhammad Reyes', '500032760078250019', 2025, 'L', NULL, NULL),
    ('Muhammad Wildan Habiibi', '500032760078250020', 2025, 'L', 'iya', NULL),
    ('Najuni Fahma Aidina', '500032760078250021', 2025, 'P', NULL, NULL),
    ('Pandu Kartika Wiratirta', '500032760078250022', 2025, 'L', NULL, NULL),
    ('Panji Ayatullloh Mubarok Ula', '500032760078250023', 2025, 'L', NULL, NULL),
    ('Renata Nadila Shaliha', '500032760078250024', 2025, 'P', NULL, NULL),
    ('Tiffani Amanda Azzahra', '500032760078250025', 2025, 'P', NULL, NULL),
    ('Yosinta Hera Elviana', '500032760078250026', 2025, 'P', NULL, NULL),
    ('Hadistia Fadilatunnisa', '500032760078250027', 2025, 'P', NULL, NULL),
    ('Oktavia Handayani', '500032760078250028', 2025, 'P', 'iya', NULL),
    ('Renda panca buana', '500032760078250029', 2025, 'L', 'iya', NULL),
    ('Safana Fuadah Azzahra', '500032760078250030', 2025, 'P', 'iya', NULL),
    ('Sintani Nur Qolbi', '500032760078250031', 2025, 'P', NULL, NULL),
    ('Syadida Aghnia Fahma', '500032760078250032', 2025, 'P', NULL, NULL),
    ('Athallah Dzaky Rulifa', '500032760078250033', 2025, NULL, NULL, NULL),
    ('Taufik Hidayat', '500032760078250034', 2025, NULL, NULL, NULL),
    ('Muhammad Arnando Al Faaris', '500032760078250035', 2025, NULL, NULL, NULL)
),
ins AS (
    INSERT INTO users (full_name, nis, entry_year, gender,
                       mubalegh_status, pendidikan_status,
                       role, password_hash, points, is_active)
    SELECT d.full_name, d.nis, d.entry_year, d.gender,
           d.mubalegh_status, d.pendidikan_status,
           'santri', '$2b$10$odabBtuZK1PYcDP7sDUaNeVPSpiElrRna9phfgWCvF8fKT6HNKwvm', 0, FALSE
      FROM d
    ON CONFLICT (nis) DO NOTHING
    RETURNING nis
)
-- Baris yang SUDAH ada tak tersentuh INSERT di atas — statusnya diisikan di
-- sini. Baris yang baru saja disisipkan tak ikut kena: CTE pengubah data tak
-- saling melihat efeknya dalam satu pernyataan, dan nilainya memang sudah benar
-- dari INSERT.
--
-- Hanya mengisi yang MASIH KOSONG (COALESCE): koreksi manual yang sudah dibuat
-- lewat aplikasi tak boleh dibatalkan hanya karena migrasi ini dijalankan ulang.
UPDATE users u
   SET mubalegh_status   = COALESCE(u.mubalegh_status, d.mubalegh_status),
       pendidikan_status = COALESCE(u.pendidikan_status, d.pendidikan_status)
  FROM d
 WHERE u.nis = d.nis
   AND (u.mubalegh_status IS NULL OR u.pendidikan_status IS NULL);

-- ═ AKTIVASI — JALANKAN TERPISAH, BUKAN BAGIAN DARI MIGRASI ═══════════════════
--
-- Sengaja tidak dijalankan di sini: siapa yang masih mondok berubah tiap tahun,
-- sedangkan migrasi hanya berjalan sekali. Menaruhnya di sini berarti database
-- baru mengaktifkan angkatan yang sudah lulus.
--
-- Saldo awal ditulis sebagai baris `point_logs`, BUKAN dengan menyetel
-- `users.points` langsung: trigger `trg_point_logs_balance` (migrasi 32) yang
-- memindahkan saldonya, sehingga `points = SUM(delta)` tetap benar.
--
--   BEGIN;
--   WITH aktif AS (
--       UPDATE users SET is_active = TRUE
--        WHERE role = 'santri'
--          AND entry_year IN (2022, 2023, 2024, 2025)   -- ← SESUAIKAN
--          AND is_active = FALSE
--        RETURNING id
--   )
--   INSERT INTO point_logs (user_id, delta, reason, category)
--   SELECT id, 300, 'Saldo awal santri', 'other' FROM aktif;
--   COMMIT;
--
-- ═ VERIFIKASI ════════════════════════════════════════════════════════════════
--   SELECT count(*) FROM users WHERE role = 'santri';          -- harap ≥ 512
--   SELECT entry_year, count(*) FILTER (WHERE is_active) AS aktif, count(*) AS total
--     FROM users WHERE role = 'santri' GROUP BY entry_year ORDER BY entry_year DESC;
--   SELECT mubalegh_status, count(*) FROM users WHERE role='santri' GROUP BY 1;
--   SELECT pendidikan_status, count(*) FROM users WHERE role='santri' GROUP BY 1;
--
-- Saldo = jumlah lognya (harus 0 baris):
--   SELECT u.id, u.full_name, u.points, COALESCE(SUM(pl.delta),0)
--     FROM users u LEFT JOIN point_logs pl ON pl.user_id = u.id
--    WHERE u.role = 'santri'
--    GROUP BY u.id, u.full_name, u.points
--   HAVING u.points <> COALESCE(SUM(pl.delta),0);
