/** Tailwind config AFM SMART — di-compile cargo-leptos (tailwind-input-file).
 *  content = sumber kelas untuk purge → hanya kelas yang DIPAKAI ikut ke CSS.
 *  Tema Material-3 (dulu disuntik via Play CDN <script>), kini di sini. */
module.exports = {
  // ⚠️ JANGAN HAPUS BARIS INI tanpa membereskan dulu kelas `dark:` di markup.
  //
  // Sekilas ia tampak konfigurasi mati: `tailwind.css` menetapkan
  // `:root { color-scheme: light }` dan paletnya hanya punya varian terang.
  // Tapi ADA 9 utility `dark:` yang benar-benar terpakai — semuanya di mockup
  // `src/web/pages/html/*.html`, yang MASIH HIDUP: `design_pages.rs`
  // menyuntikkannya lewat `include_str!` dan halaman-halaman itu punya rute
  // sendiri (halaqah, rekaman, koneksi-ortu, …).
  //
  // Dengan `"class"`, utility itu hanya menyala bila ada leluhur `.dark` —
  // dan tak ada satu pun di aplikasi ini, jadi efektifnya mati. Hapus barisnya
  // dan Tailwind v4 jatuh ke `prefers-color-scheme`: pengguna yang ponselnya
  // bermode gelap tiba-tiba mendapat nav & header gelap di halaman yang
  // sisanya terang. Bukan mode gelap — separuh mode gelap.
  //
  // Urutan yang benar bila mode gelap mau dikerjakan sungguhan: siapkan token
  // gelap untuk SELURUH palet di bawah, baru ubah baris ini.
  darkMode: "class",
  content: ["./src/**/*.rs", "./src/**/*.html"],
  // Tak ada `safelist`: seluruh kelas ditulis literal di markup Rust — tak ada
  // yang dirakit lewat `format!("text-{…}")`, jadi purge tak bisa membuang
  // kelas yang sebenarnya terpakai. Kalau suatu saat menambah kelas dinamis,
  // safelist-nya WAJIB ikut ditambah atau gayanya hilang senyap di produksi.
  theme: {
    extend: {
      colors: {
        surface: "#f9f9ff", "surface-dim": "#d3daea", "surface-bright": "#f9f9ff",
        "surface-container-lowest": "#ffffff", "surface-container-low": "#f0f3ff",
        "surface-container": "#e7eefe", "surface-container-high": "#e2e8f8",
        "surface-container-highest": "#dce2f3", "on-surface": "#151c27",
        "on-surface-variant": "#404944", "inverse-surface": "#2a313d",
        "inverse-on-surface": "#ebf1ff", outline: "#707974", "outline-variant": "#bfc9c3",
        "surface-tint": "#2b6954", primary: "#003527", "on-primary": "#ffffff",
        "primary-container": "#064e3b", "on-primary-container": "#80bea6",
        "inverse-primary": "#95d3ba", secondary: "#416656", "on-secondary": "#ffffff",
        "secondary-container": "#c3ecd7", "on-secondary-container": "#476c5b",
        tertiary: "#2c2f30", "on-tertiary": "#ffffff", "tertiary-container": "#424546",
        "on-tertiary-container": "#b0b2b3", error: "#ba1a1a", "on-error": "#ffffff",
        "error-container": "#ffdad6", "on-error-container": "#93000a",
        "primary-fixed": "#b0f0d6", "primary-fixed-dim": "#95d3ba",
        "on-primary-fixed": "#002117", "on-primary-fixed-variant": "#0b513d",
        "secondary-fixed": "#c3ecd7", "secondary-fixed-dim": "#a8cfbc",
        "on-secondary-fixed": "#002115", "on-secondary-fixed-variant": "#294e3f",
        "tertiary-fixed": "#e1e3e4", "tertiary-fixed-dim": "#c5c7c8",
        "on-tertiary-fixed": "#191c1d", "on-tertiary-fixed-variant": "#454748",
        background: "#f9f9ff", "on-background": "#151c27", "surface-variant": "#dce2f3",
        success: "#059669", warning: "#f59e0b", info: "#2563eb",
      },
      borderRadius: { DEFAULT: "0.25rem", lg: "0.5rem", xl: "0.75rem", "2xl": "1rem", full: "9999px" },
      fontFamily: {
        sans: ["Work Sans", "system-ui", "sans-serif"],
        "display-lg": ["Work Sans"], "display-md": ["Work Sans"], "headline-sm": ["Work Sans"],
        "body-lg": ["Work Sans"], "body-md": ["Work Sans"], "body-sm": ["Work Sans"], "label-md": ["Work Sans"],
      },
      fontSize: {
        "display-lg": ["32px", { lineHeight: "40px", letterSpacing: "-0.02em", fontWeight: "700" }],
        "display-md": ["24px", { lineHeight: "32px", letterSpacing: "-0.01em", fontWeight: "600" }],
        "headline-sm": ["20px", { lineHeight: "28px", fontWeight: "600" }],
        "body-lg": ["18px", { lineHeight: "28px", fontWeight: "400" }],
        "body-md": ["16px", { lineHeight: "24px", fontWeight: "400" }],
        "body-sm": ["14px", { lineHeight: "20px", fontWeight: "400" }],
        "label-md": ["12px", { lineHeight: "16px", letterSpacing: "0.05em", fontWeight: "600" }],
      },
    },
  },
  plugins: [],
};
