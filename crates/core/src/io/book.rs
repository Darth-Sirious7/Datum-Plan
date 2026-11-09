//! Plain spreadsheets for the registers and the task table: typed cells (text, numbers, dates)
//! written as a workbook (.xlsx) or an OpenDocument spreadsheet (.ods), chosen by the file's
//! extension. The OpenDocument file is written here: the mimetype first and stored, a manifest
//! and the content.
use chrono::NaiveDate;
use std::io::Write;
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub enum Cell { Empty, Text(String), Num(f64), Date(NaiveDate) }

impl From<&str> for Cell { fn from(s: &str) -> Self { if s.is_empty() { Cell::Empty } else { Cell::Text(s.into()) } } }
impl From<String> for Cell { fn from(s: String) -> Self { if s.is_empty() { Cell::Empty } else { Cell::Text(s) } } }
impl From<Option<NaiveDate>> for Cell { fn from(d: Option<NaiveDate>) -> Self { d.map_or(Cell::Empty, Cell::Date) } }

/// One sheet: its name, column widths (characters) and rows; the first row is the headings.
pub struct Sheet { pub name: String, pub widths: Vec<f64>, pub rows: Vec<Vec<Cell>> }

/// Writes the sheets as .ods when the path ends so, else as .xlsx.
pub fn write(path: &Path, sheets: &[Sheet]) -> Result<(), String> {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ods")) { write_ods(path, sheets) } else { write_xlsx(path, sheets) }
}

fn write_xlsx(path: &Path, sheets: &[Sheet]) -> Result<(), String> {
    use rust_xlsxwriter::{Format, Workbook};
    let e = |x: rust_xlsxwriter::XlsxError| x.to_string();
    let mut wb = Workbook::new();
    let head = Format::new().set_bold().set_background_color(0xEAF0F8);
    let date = Format::new().set_num_format("dd-mmm-yy");
    for s in sheets {
        let ws = wb.add_worksheet();
        ws.set_name(&s.name).map_err(e)?;
        for (j, w) in s.widths.iter().enumerate() { ws.set_column_width(j as u16, *w).map_err(e)?; }
        for (i, row) in s.rows.iter().enumerate() {
            for (j, c) in row.iter().enumerate() {
                let (r, k) = (i as u32, j as u16);
                match c {
                    Cell::Empty => {}
                    Cell::Text(t) if i == 0 => { ws.write_string_with_format(r, k, t, &head).map_err(e)?; }
                    Cell::Text(t) => { ws.write_string(r, k, t).map_err(e)?; }
                    Cell::Num(n) => { ws.write_number(r, k, *n).map_err(e)?; }
                    Cell::Date(d) => { ws.write_number_with_format(r, k, serial(*d), &date).map_err(e)?; }
                }
            }
        }
        ws.set_freeze_panes(1, 0).map_err(e)?;
    }
    wb.save(path).map_err(e)
}

/// Days since 1899-12-30, as spreadsheets count dates.
fn serial(d: NaiveDate) -> f64 { (d - NaiveDate::from_ymd_opt(1899, 12, 30).unwrap_or_default()).num_days() as f64 }

fn esc(t: &str) -> String {
    t.chars().filter(|c| !c.is_control() || *c == '\t').map(|c| match c { '&' => "&amp;".into(), '<' => "&lt;".into(), '>' => "&gt;".into(), '"' => "&quot;".into(), c => c.to_string() }).collect()
}

/// The content of an .ods: one table per sheet, headings bold, dates as dates.
pub fn ods_content(sheets: &[Sheet]) -> String {
    let mut x = String::from(r##"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:number="urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0" office:version="1.2"><office:automatic-styles><number:date-style style:name="N1"><number:day number:style="long"/><number:text>-</number:text><number:month number:textual="true"/><number:text>-</number:text><number:year/></number:date-style><style:style style:name="ce1" style:family="table-cell" style:data-style-name="N1"/><style:style style:name="ceh" style:family="table-cell"><style:table-cell-properties fo:background-color="#eaf0f8"/><style:text-properties fo:font-weight="bold"/></style:style>"##);
    let mut widths: Vec<f64> = sheets.iter().flat_map(|s| s.widths.iter().copied()).collect();
    widths.sort_by(f64::total_cmp);
    widths.dedup();
    for (k, w) in widths.iter().enumerate() { x += &format!(r#"<style:style style:name="co{k}" style:family="table-column"><style:table-column-properties style:column-width="{:.2}cm"/></style:style>"#, w * 0.19) }
    x += "</office:automatic-styles><office:body><office:spreadsheet>";
    for s in sheets {
        x += &format!(r#"<table:table table:name="{}">"#, esc(&s.name));
        for w in &s.widths { x += &format!(r#"<table:table-column table:style-name="co{}"/>"#, widths.iter().position(|v| v == w).unwrap_or(0)) }
        for (i, row) in s.rows.iter().enumerate() {
            x += "<table:table-row>";
            for c in row {
                x += &match c {
                    Cell::Empty => "<table:table-cell/>".into(),
                    Cell::Text(t) => format!(r#"<table:table-cell{} office:value-type="string"><text:p>{}</text:p></table:table-cell>"#, if i == 0 { r#" table:style-name="ceh""# } else { "" }, esc(t)),
                    Cell::Num(n) if n.is_finite() => format!(r#"<table:table-cell office:value-type="float" office:value="{n}"><text:p>{n}</text:p></table:table-cell>"#),
                    Cell::Num(_) => "<table:table-cell/>".into(),
                    Cell::Date(d) => format!(r#"<table:table-cell table:style-name="ce1" office:value-type="date" office:date-value="{}"><text:p>{}</text:p></table:table-cell>"#, d.format("%Y-%m-%d"), d.format("%d-%b-%y")),
                };
            }
            x += "</table:table-row>";
        }
        x += "</table:table>";
    }
    x + "</office:spreadsheet></office:body></office:document-content>"
}

fn write_ods(path: &Path, sheets: &[Sheet]) -> Result<(), String> {
    use zip::write::SimpleFileOptions;
    let e = |x: zip::result::ZipError| x.to_string();
    let io = |x: std::io::Error| x.to_string();
    let tmp = path.with_extension("ods.part");
    let mut z = zip::ZipWriter::new(std::fs::File::create(&tmp).map_err(io)?);
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let packed = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    z.start_file("mimetype", stored).map_err(e)?;
    z.write_all(b"application/vnd.oasis.opendocument.spreadsheet").map_err(io)?;
    z.start_file("META-INF/manifest.xml", packed).map_err(e)?;
    z.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.2"><manifest:file-entry manifest:full-path="/" manifest:version="1.2" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/><manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/></manifest:manifest>"#).map_err(io)?;
    z.start_file("content.xml", packed).map_err(e)?;
    z.write_all(ods_content(sheets).as_bytes()).map_err(io)?;
    z.finish().map_err(e)?;
    std::fs::rename(&tmp, path).map_err(io)
}

#[cfg(test)]
mod t {
    use super::*;
    fn sample() -> Vec<Sheet> {
        let d = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap();
        vec![Sheet { name: "Tasks & <dates>".into(), widths: vec![30.0, 12.0], rows: vec![vec!["Name".into(), "Finish".into()], vec!["Pump \"A\" & skid".into(), Cell::Date(d)], vec![Cell::Num(2.5), Cell::Empty]] }]
    }

    #[test]
    fn an_ods_is_a_valid_package_with_typed_cells() {
        let dir = std::env::temp_dir().join(format!("datum-ods-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.ods");
        write(&p, &sample()).unwrap();
        let mut z = zip::ZipArchive::new(std::fs::File::open(&p).unwrap()).unwrap();
        // The mimetype comes first, stored, so other programs recognise the file.
        let first = z.by_index(0).unwrap();
        assert_eq!((first.name().unwrap().to_string(), first.compression()), ("mimetype".to_string(), zip::CompressionMethod::Stored));
        drop(first);
        let mut s = String::new();
        std::io::Read::read_to_string(&mut z.by_name("content.xml").unwrap(), &mut s).unwrap();
        assert!(s.contains(r#"table:name="Tasks &amp; &lt;dates&gt;""#) && s.contains("Pump &quot;A&quot; &amp; skid"), "text is escaped");
        assert!(s.contains(r#"office:value-type="date" office:date-value="2026-11-05""#) && s.contains(r#"office:value-type="float" office:value="2.5""#), "typed cells");
        assert!(z.by_name("META-INF/manifest.xml").is_ok());
        assert!(!dir.join("t.ods.part").exists());
        // Well-formed: every element opened is closed.
        assert_eq!(s.matches("<table:table-row>").count(), s.matches("</table:table-row>").count());
        let x = dir.join("t.xlsx");
        write(&x, &sample()).unwrap();
        assert!(std::fs::metadata(&x).unwrap().len() > 1000);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_task_table_exports_as_ods_with_dates_and_days() {
        use crate::doc::{Doc, Op};
        let mut d = Doc::new(crate::model::Project::new("Plant", NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()));
        let row = |n: &str, dur: &str| vec![("name".to_string(), n.to_string()), ("dur".to_string(), dur.to_string())];
        d.apply(Op::InsertRows { at: 0, rows: vec![row("Design & <review>", "5d")] }, "%d/%m/%Y").unwrap();
        let p = std::env::temp_dir().join(format!("datum-tasks-{}.ods", std::process::id()));
        crate::io::export(&d.p, &p, "%d/%m/%Y", NaiveDate::from_ymd_opt(2026, 11, 2).unwrap(), &["name".into(), "dur".into(), "start".into()]).unwrap();
        let mut z = zip::ZipArchive::new(std::fs::File::open(&p).unwrap()).unwrap();
        let mut s = String::new();
        std::io::Read::read_to_string(&mut z.by_name("content.xml").unwrap(), &mut s).unwrap();
        assert!(s.contains("Design &amp; &lt;review&gt;"), "{s}");
        assert!(s.contains(r#"office:value="5""#), "the duration in days");
        assert!(s.contains(r#"office:date-value="2026-11-02""#), "the start as a date");
        let _ = std::fs::remove_file(&p);
    }
}
