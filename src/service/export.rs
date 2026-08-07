//! service/export.rs — Ekspor laporan ke PDF & Excel (.xlsx).
//!
//! Satu model generik `ReportDoc` (judul + statistik + seksi tabel) dipakai
//! SEMUA varian /laporan (admin/pamong, guru/dewan guru, orang tua, santri).
//! Adapter di bawah mengubah data yang SUDAH diambil oleh service::laporan
//! (tak ada query DB baru di sini) jadi ReportDoc; render_pdf/render_xlsx
//! generik atas struktur itu — satu tempat utk tambah format baru nanti.

use printpdf::{
    BuiltinFont, Color, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Point, Pt,
    Rgb, TextItem,
};
use rust_xlsxwriter::{Format, Workbook};

use crate::models::{
    AnalisisData, LaporanAdminData, LaporanGuruExtra, LaporanOrtuData, LaporanSantriData,
    OutsideRow,
};

pub struct ReportSection {
    pub title: String,
    pub headers: Vec<&'static str>,
    pub rows: Vec<Vec<String>>,
}

pub struct ReportDoc {
    pub title: String,
    pub subtitle: String,
    pub generated_label: String,
    pub stats: Vec<(&'static str, String)>,
    pub sections: Vec<ReportSection>,
}

// ── Adapter: data /laporan yang sudah ada → ReportDoc ───────────────────────

pub fn admin_doc(d: &LaporanAdminData, outside: &[OutsideRow], generated_label: String) -> ReportDoc {
    ReportDoc {
        title: "Laporan Institusi — AFM SMART".into(),
        subtitle: "Ringkasan performa akademik & administratif institusi".into(),
        generated_label,
        stats: vec![
            ("Rata-rata Kehadiran", format!("{}%", d.attendance_pct)),
            ("Poin Pelanggaran Aktif (30 hari)", d.active_violation_points.to_string()),
            ("Santri di Luar Pondok", d.santri_di_luar.to_string()),
        ],
        sections: vec![
            ReportSection {
                title: "Performa Kelas".into(),
                headers: vec!["Kelas", "Santri", "Kehadiran", "Status"],
                rows: d
                    .classes
                    .iter()
                    .map(|c| {
                        vec![
                            c.name.clone(),
                            c.member_count.to_string(),
                            format!("{}%", c.attendance_pct),
                            c.status_label.clone(),
                        ]
                    })
                    .collect(),
            },
            ReportSection {
                title: "Ringkasan Poin Terbaru".into(),
                headers: vec!["Nama", "Kelas", "Alasan", "Poin"],
                rows: d
                    .points_recent
                    .iter()
                    .map(|p| {
                        vec![p.name.clone(), p.class_name.clone(), p.reason.clone(), fmt_delta(p.delta)]
                    })
                    .collect(),
            },
            ReportSection {
                title: "Sedang di Luar Pondok".into(),
                headers: vec!["Nama", "NIS", "Kelas", "Sejak"],
                rows: outside
                    .iter()
                    .map(|o| vec![o.name.clone(), o.nis.clone(), o.class_name.clone(), o.since_label.clone()])
                    .collect(),
            },
        ],
    }
}

pub fn guru_doc(d: &AnalisisData, extra: &LaporanGuruExtra, generated_label: String) -> ReportDoc {
    let scope = if d.is_dewan { "Seluruh pesantren" } else { "Kelas yang diampu" };
    ReportDoc {
        title: "Laporan Kelas Akademik — AFM SMART".into(),
        subtitle: format!("Cakupan: {scope}"),
        generated_label,
        stats: vec![
            ("Rata-rata Kehadiran", format!("{}%", d.attendance_pct)),
            ("Rata-rata Poin", d.avg_points.to_string()),
            ("Absensi Terverifikasi", d.sessions_verified.to_string()),
        ],
        sections: vec![
            ReportSection {
                title: "Santri Teladan — Hafalan".into(),
                headers: vec!["Nama", "Kelas", "Juz", "Poin"],
                rows: extra
                    .hafalan_top
                    .iter()
                    .map(|s| {
                        vec![s.name.clone(), s.class_name.clone(), s.juz_count.to_string(), s.points.to_string()]
                    })
                    .collect(),
            },
            ReportSection {
                title: "Kehadiran per Kelas".into(),
                headers: vec!["Kelas", "Santri", "Kehadiran", "Rata Poin"],
                rows: d
                    .class_ranking
                    .iter()
                    .map(|r| {
                        vec![
                            r.name.clone(),
                            r.santri_count.to_string(),
                            format!("{}%", r.attendance_pct),
                            r.avg_points.to_string(),
                        ]
                    })
                    .collect(),
            },
        ],
    }
}

pub fn ortu_doc(d: &LaporanOrtuData, generated_label: String) -> ReportDoc {
    ReportDoc {
        title: format!("Laporan Perkembangan Santri — {}", d.child_name),
        subtitle: format!("{} • NIS {}", d.class_name, d.nis),
        generated_label,
        stats: vec![
            ("Hadir", d.hadir.to_string()),
            ("Izin", d.izin.to_string()),
            ("Alpa", d.alpa.to_string()),
            ("Persentase Kehadiran", format!("{}%", d.attendance_pct)),
            ("Total Poin", d.points.to_string()),
            ("Status Gerbang", format!("{} (sejak {})", gate_label(&d.gate_status), d.gate_at_label)),
            ("Juz Hafalan", d.juz_count.to_string()),
        ],
        sections: point_and_hafalan_sections(&d.prestasi, &d.pelanggaran, &d.hafalan),
    }
}

pub fn santri_doc(d: &LaporanSantriData, generated_label: String) -> ReportDoc {
    ReportDoc {
        title: "Rapor Pribadi Santri — AFM SMART".into(),
        subtitle: String::new(),
        generated_label,
        stats: vec![
            ("Hadir", d.hadir.to_string()),
            ("Izin", d.izin.to_string()),
            ("Alpa", d.alpa.to_string()),
            ("Persentase Kehadiran", format!("{}%", d.attendance_pct)),
            ("Total Poin", d.points.to_string()),
            ("Status Gerbang", format!("{} (sejak {})", gate_label(&d.gate_status), d.gate_at_label)),
            ("Juz Hafalan", d.juz_count.to_string()),
        ],
        sections: point_and_hafalan_sections(&d.prestasi, &d.pelanggaran, &d.hafalan),
    }
}

fn gate_label(status: &str) -> &'static str {
    if status == "out" { "Di Luar Pondok" } else { "Di Dalam Pondok" }
}

fn fmt_delta(delta: i32) -> String {
    if delta >= 0 { format!("+{delta}") } else { delta.to_string() }
}

fn point_and_hafalan_sections(
    prestasi: &[crate::models::LaporanPointItem],
    pelanggaran: &[crate::models::LaporanPointItem],
    hafalan: &[crate::models::HafalanItem],
) -> Vec<ReportSection> {
    vec![
        ReportSection {
            title: "Prestasi Terbaru".into(),
            headers: vec!["Alasan", "Tanggal", "Poin"],
            rows: prestasi
                .iter()
                .map(|p| vec![p.reason.clone(), p.date_label.clone(), fmt_delta(p.delta)])
                .collect(),
        },
        ReportSection {
            title: "Pelanggaran & Teguran".into(),
            headers: vec!["Alasan", "Tanggal", "Poin"],
            rows: pelanggaran
                .iter()
                .map(|p| vec![p.reason.clone(), p.date_label.clone(), fmt_delta(p.delta)])
                .collect(),
        },
        ReportSection {
            title: "Capaian Hafalan".into(),
            headers: vec!["Surah", "Ayat", "Kualitas", "Dicatat Oleh"],
            rows: hafalan
                .iter()
                .map(|h| vec![h.surah.clone(), h.ayat_range.clone(), h.quality_label.clone(), h.note.clone()])
                .collect(),
        },
    ]
}

// ── Render PDF (A4, font base-14, pagination manual — tanpa fitur "html") ───

const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN: f32 = 16.0;
const LINE_H: f32 = 6.0;
const BRAND: (f32, f32, f32) = (0.024, 0.208, 0.161); // emerald DESIGN.md

struct PdfCursor {
    pages: Vec<PdfPage>,
    ops: Vec<Op>,
    y: f32,
}

impl PdfCursor {
    fn new() -> Self {
        Self { pages: Vec::new(), ops: Vec::new(), y: PAGE_H - MARGIN }
    }

    fn ensure_space(&mut self, needed: f32) {
        if self.y - needed < MARGIN {
            self.flush_page();
        }
    }

    fn flush_page(&mut self) {
        let ops = std::mem::take(&mut self.ops);
        self.pages.push(PdfPage::new(Mm(PAGE_W), Mm(PAGE_H), ops));
        self.y = PAGE_H - MARGIN;
    }

    fn gap(&mut self, h: f32) {
        self.y -= h;
        if self.y < MARGIN {
            self.flush_page();
        }
    }

    fn text_at(&mut self, x: f32, text: &str, size: f32, bold: bool, color: (f32, f32, f32)) {
        if text.is_empty() {
            return;
        }
        let font = if bold { BuiltinFont::HelveticaBold } else { BuiltinFont::Helvetica };
        self.ops.extend_from_slice(&[
            Op::StartTextSection,
            Op::SetFont { font: PdfFontHandle::Builtin(font), size: Pt(size) },
            Op::SetLineHeight { lh: Pt(size + 2.0) },
            Op::SetFillColor {
                col: Color::Rgb(Rgb { r: color.0, g: color.1, b: color.2, icc_profile: None }),
            },
            Op::SetTextCursor { pos: Point::new(Mm(x), Mm(self.y)) },
            Op::ShowText { items: vec![TextItem::Text(truncate_cell(text, 40))] },
            Op::EndTextSection,
        ]);
    }

    fn line(&mut self, text: &str, size: f32, bold: bool, color: (f32, f32, f32)) {
        self.ensure_space(LINE_H);
        self.text_at(MARGIN, text, size, bold, color);
        self.y -= LINE_H.max(size * 0.45);
    }

    fn row(&mut self, cols: &[String], widths: &[f32], size: f32, bold: bool) {
        self.ensure_space(LINE_H);
        let mut x = MARGIN;
        for (i, c) in cols.iter().enumerate() {
            let w = widths.get(i).copied().unwrap_or(30.0);
            let max_chars = ((w / (size * 0.13)) as usize).max(4);
            self.text_at(x, &truncate_cell(c, max_chars), size, bold, (0.12, 0.12, 0.12));
            x += w;
        }
        self.y -= LINE_H;
    }
}

fn truncate_cell(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

pub fn render_pdf(doc: &ReportDoc) -> Vec<u8> {
    let mut c = PdfCursor::new();
    c.line(&doc.title, 17.0, true, BRAND);
    if !doc.subtitle.is_empty() {
        c.line(&doc.subtitle, 10.0, false, (0.35, 0.35, 0.35));
    }
    c.line(&format!("Dicetak: {}", doc.generated_label), 8.5, false, (0.55, 0.55, 0.55));
    c.gap(4.0);

    if !doc.stats.is_empty() {
        c.line("Ringkasan", 12.5, true, BRAND);
        for (label, value) in &doc.stats {
            c.row(&[label.to_string(), value.clone()], &[110.0, 60.0], 10.0, false);
        }
        c.gap(5.0);
    }

    for section in &doc.sections {
        if section.rows.is_empty() {
            continue;
        }
        c.ensure_space(LINE_H * 3.0);
        c.line(&section.title, 12.5, true, BRAND);
        let n = section.headers.len().max(1);
        let content_w = PAGE_W - 2.0 * MARGIN;
        let widths = vec![content_w / n as f32; n];
        let header_row: Vec<String> = section.headers.iter().map(|h| h.to_string()).collect();
        c.row(&header_row, &widths, 9.5, true);
        for row in &section.rows {
            c.ensure_space(LINE_H);
            c.row(row, &widths, 9.0, false);
        }
        c.gap(5.0);
    }
    c.flush_page();

    let mut pdf_doc = PdfDocument::new(&doc.title);
    pdf_doc.with_pages(c.pages).save(&PdfSaveOptions::default(), &mut Vec::new())
}

// ── Render Excel (.xlsx) ─────────────────────────────────────────────────────

pub fn render_xlsx(doc: &ReportDoc) -> Result<Vec<u8>, String> {
    let mut wb = Workbook::new();
    let sheet = wb.add_worksheet();
    sheet.set_name("Laporan").map_err(|e| e.to_string())?;

    let title_fmt = Format::new().set_bold().set_font_size(14);
    let header_fmt = Format::new().set_bold().set_background_color(rust_xlsxwriter::Color::RGB(0xE8EAF6));

    let mut row = 0u32;
    sheet.write_string_with_format(row, 0, &doc.title, &title_fmt).map_err(|e| e.to_string())?;
    row += 1;
    if !doc.subtitle.is_empty() {
        sheet.write_string(row, 0, &doc.subtitle).map_err(|e| e.to_string())?;
        row += 1;
    }
    sheet
        .write_string(row, 0, format!("Dicetak: {}", doc.generated_label))
        .map_err(|e| e.to_string())?;
    row += 2;

    if !doc.stats.is_empty() {
        sheet.write_string_with_format(row, 0, "Ringkasan", &header_fmt).map_err(|e| e.to_string())?;
        row += 1;
        for (label, value) in &doc.stats {
            sheet.write_string(row, 0, *label).map_err(|e| e.to_string())?;
            sheet.write_string(row, 1, value).map_err(|e| e.to_string())?;
            row += 1;
        }
        row += 1;
    }

    for section in &doc.sections {
        if section.rows.is_empty() {
            continue;
        }
        sheet.write_string_with_format(row, 0, &section.title, &header_fmt).map_err(|e| e.to_string())?;
        row += 1;
        for (i, h) in section.headers.iter().enumerate() {
            sheet
                .write_string_with_format(row, i as u16, *h, &header_fmt)
                .map_err(|e| e.to_string())?;
        }
        row += 1;
        for r in &section.rows {
            for (i, v) in r.iter().enumerate() {
                sheet.write_string(row, i as u16, v).map_err(|e| e.to_string())?;
            }
            row += 1;
        }
        row += 1;
    }

    for i in 0..6u16 {
        sheet.set_column_width(i, 24.0).map_err(|e| e.to_string())?;
    }

    wb.save_to_buffer().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_doc() -> ReportDoc {
        ReportDoc {
            title: "Laporan Uji — AFM SMART".into(),
            subtitle: "Contoh subjudul".into(),
            generated_label: "23 Jul 2026, 10:00 WIB".into(),
            stats: vec![("Rata-rata Kehadiran", "92%".into()), ("Total Santri", "150".into())],
            sections: vec![ReportSection {
                title: "Performa Kelas".into(),
                headers: vec!["Kelas", "Santri", "Kehadiran"],
                rows: vec![
                    vec!["Kelas A".into(), "20".into(), "95%".into()],
                    vec!["Kelas B".into(), "18".into(), "88%".into()],
                ],
            }],
        }
    }

    #[test]
    fn pdf_has_valid_header() {
        let bytes = render_pdf(&sample_doc());
        assert!(bytes.len() > 200, "PDF terlalu kecil: {} byte", bytes.len());
        assert_eq!(&bytes[0..5], b"%PDF-", "harus diawali magic header PDF");
    }

    #[test]
    fn xlsx_has_valid_zip_header() {
        let bytes = render_xlsx(&sample_doc()).expect("render_xlsx gagal");
        assert!(bytes.len() > 200, "XLSX terlalu kecil: {} byte", bytes.len());
        assert_eq!(&bytes[0..2], b"PK", "xlsx adalah arsip zip (magic PK)");
    }

    #[test]
    fn empty_sections_are_skipped_without_panic() {
        let mut doc = sample_doc();
        doc.sections.push(ReportSection { title: "Kosong".into(), headers: vec!["A"], rows: vec![] });
        let _ = render_pdf(&doc);
        render_xlsx(&doc).expect("render_xlsx gagal dgn seksi kosong");
    }
}
