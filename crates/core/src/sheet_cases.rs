//! Sheets: workbook import and export and sheet edits, under hostile and odd input.
use crate::{doc::{Doc, Op}, fields::Calc, *};
use chrono::NaiveDate;
use std::io::{Read, Write};

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
fn base() -> Doc {
    let mut p = Project::new("T", d(2026, 11, 2));
    let t = p.new_task("T1", 0, 960);
    p.tasks.push(t);
    Doc::new(p)
}
fn op(doc: &mut Doc, o: Op) { doc.apply(o, "%d/%m/%Y").unwrap() }
fn tmp(name: &str) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    std::env::temp_dir().join(format!("h22-{}-{}-{name}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)))
}
fn sheet(name: &str, cells: &[(&str, &str)]) -> SheetData {
    SheetData { name: name.into(), cells: cells.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), ..Default::default() }
}
fn export_sheets(p: &Project) -> Result<std::path::PathBuf, String> {
    let f = tmp("s.xlsx");
    let c = Calc::new(p, "%d/%m/%Y", d(2026, 11, 2));
    crate::io::table::write_sheets_xlsx(&c, &f).map(|_| f)
}
fn export_full(p: &Project) -> Result<std::path::PathBuf, String> {
    let f = tmp("f.xlsx");
    let c = Calc::new(p, "%d/%m/%Y", d(2026, 11, 2));
    crate::io::table::write_xlsx(&c, &["name".to_string()], &f).map(|_| f)
}
fn part(path: &std::path::Path, name: &str) -> String {
    let mut z = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut s = String::new();
    z.by_name(name).map(|mut e| { e.read_to_string(&mut s).unwrap(); }).ok();
    s
}
/// Rewrites one part of a zip.
fn patch(path: &std::path::Path, name: &str, f: impl Fn(String) -> String) -> std::path::PathBuf {
    let out = tmp("p.xlsx");
    let mut z = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut w = zip::ZipWriter::new(std::fs::File::create(&out).unwrap());
    for i in 0..z.len() {
        let mut e = z.by_index(i).unwrap();
        let n = e.name().unwrap().to_string();
        let mut b = vec![];
        e.read_to_end(&mut b).unwrap();
        if n == name { b = f(String::from_utf8(b).unwrap()).into_bytes(); }
        w.start_file(n, zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(&b).unwrap();
    }
    w.finish().unwrap();
    out
}

// --- sheet names ---------------------------------------------------------------------------------

#[test]
fn h_name_31_chars_and_its_copy() {
    let mut doc = base();
    let long = "Quarterly cost report for vendo"; // 31 characters, allowed by the window
    assert_eq!(long.chars().count(), 31);
    op(&mut doc, Op::Sheet { k: None, sheet: Some(sheet(long, &[("A1", "1")])) });
    // Move or Copy › Create a copy: "<name> (2)".
    doc.p.sheets.push(sheet(&format!("{long} (2)"), &[("A1", "2")]));
    let r = export_sheets(&doc.p);
    assert!(r.is_ok(), "export of a 31-char sheet and its copy: {r:?}");
}

#[test]
fn h_name_apostrophe_edges() {
    let mut doc = base();
    doc.p.sheets.push(sheet("'Q1'", &[("A1", "1")]));
    let r = export_sheets(&doc.p);
    assert!(r.is_ok(), "a name with apostrophes at its ends (allowed by rename): {r:?}");
}

#[test]
fn h_name_tasks_full_export() {
    let mut doc = base();
    doc.p.sheets.push(sheet("Tasks", &[("A1", "1")]));
    let r = export_full(&doc.p);
    assert!(r.is_ok(), "a sheet called Tasks and Save as Excel: {r:?}");
}

#[test]
fn h_name_tasks_round_trip() {
    let mut doc = base();
    doc.p.sheets.push(sheet("Tasks", &[("A1", "keep me")]));
    op(&mut doc, Op::Sheet { k: None, sheet: Some(SheetData { cells: [("A1".to_string(), "Germany".to_string())].into(), fmt: [("A1".to_string(), "flag".to_string())].into(), ..sheet("Other", &[]) }) });
    let f = export_sheets(&doc.p).unwrap();
    let back = crate::io::sheetbook::import(&f).unwrap();
    let names: Vec<_> = back.sheets.iter().map(|s| s.name.clone()).collect();
    assert!(names.contains(&"Tasks".to_string()), "the user's Tasks sheet comes back: {names:?}");
}

#[test]
fn h_name_datumplan() {
    let mut doc = base();
    doc.p.sheets.push(SheetData { fmt: [("A1".to_string(), "flag".to_string())].into(), ..sheet("DatumPlan", &[("A1", "Germany")]) });
    let r = export_sheets(&doc.p);
    assert!(r.is_ok(), "a sheet called DatumPlan with a flag: {r:?}");
}

#[test]
fn h_name_meta_keys_follow_written_name() {
    let mut doc = base();
    // A name that an import or the window cannot make, but a .dplan file can hold: written cut to 31.
    let long = "Vendor list with flags and more text";
    doc.p.sheets.push(SheetData { fmt: [("A1".to_string(), "flag".to_string())].into(), ..sheet(long, &[("A1", "Germany")]) });
    let f = export_sheets(&doc.p).unwrap();
    let back = crate::io::sheetbook::import(&f).unwrap();
    assert_eq!(back.sheets[0].fmt.get("A1").map(String::as_str), Some("flag"), "flags of a long-named sheet come back ({})", back.sheets[0].name);
}

// --- conditional formatting -----------------------------------------------------------------------

fn cond_export(rule: CondRule) -> Result<std::path::PathBuf, String> {
    let mut doc = base();
    op(&mut doc, Op::Sheet { k: None, sheet: Some(SheetData { cond: vec![rule], ..sheet("C", &[("A1", "1"), ("A2", "2")]) }) });
    export_sheets(&doc.p)
}
#[test]
fn h_cond_top_5000() { let r = cond_export(CondRule { range: "A1:A9".into(), op: "top".into(), a: "5000".into(), style: "b".into(), ..Default::default() }); assert!(r.is_ok(), "{r:?}"); }
#[test]
fn h_cond_top_0() { let r = cond_export(CondRule { range: "A1:A9".into(), op: "top".into(), a: "0".into(), style: "b".into(), ..Default::default() }); assert!(r.is_ok(), "{r:?}"); }
#[test]
fn h_cond_gt_empty() { let r = cond_export(CondRule { range: "A1:A9".into(), op: "gt".into(), a: "".into(), style: "b".into(), ..Default::default() }); assert!(r.is_ok(), "{r:?}"); }
#[test]
fn h_cond_between_empty() { let r = cond_export(CondRule { range: "A1:A9".into(), op: "between".into(), style: "b".into(), ..Default::default() }); assert!(r.is_ok(), "{r:?}"); }
#[test]
fn h_cond_contains_empty() { let r = cond_export(CondRule { range: "A1:A9".into(), op: "contains".into(), style: "b".into(), ..Default::default() }); assert!(r.is_ok(), "{r:?}"); }
#[test]
fn h_cond_scale_bad_colour() { let r = cond_export(CondRule { range: "A1:A9".into(), op: "scale".into(), a: "red".into(), b: "".into(), ..Default::default() }); assert!(r.is_ok(), "{r:?}"); }

// --- formulas -------------------------------------------------------------------------------------

#[test]
fn h_future_functions_prefixed() {
    let mut doc = base();
    op(&mut doc, Op::Sheet { k: None, sheet: Some(sheet("F", &[("A1", "1"), ("B1", "=XLOOKUP(1,A1:A3,A1:A3)"), ("C1", "=TEXTJOIN(\",\",TRUE,A1:A3)"), ("D1", "=IFS(A1>0,1,TRUE,2)")])) });
    let f = export_sheets(&doc.p).unwrap();
    let x = part(&f, "xl/worksheets/sheet1.xml");
    let fs: Vec<&str> = x.match_indices("<f>").map(|(i, _)| &x[i..i + x[i..].find("</f>").unwrap()]).collect();
    println!("{fs:?}");
    assert!(x.contains("_xlfn.XLOOKUP"), "newer functions carry their prefix in the file: {fs:?}");
}

#[test]
fn h_text_starting_with_equals() {
    // A workbook text cell "=== Totals ===" (text, not a formula).
    let f = tmp("eq.xlsx");
    let mut wb = rust_xlsxwriter::Workbook::new();
    let ws = wb.add_worksheet();
    ws.write_string(0, 0, "=== Totals ===").unwrap();
    ws.write_string(1, 0, "= see note").unwrap();
    wb.save(&f).unwrap();
    let back = crate::io::sheetbook::import(&f).unwrap();
    let s = &back.sheets[0];
    println!("{:?} {:?}", s.cells, s.fmt);
    let mut doc = base();
    op(&mut doc, Op::AddSheets { sheets: back.sheets, pictures: vec![], freeze: vec![] });
    let c = Calc::new(&doc.p, "%d/%m/%Y", d(2026, 11, 2));
    let v = crate::sheet::sheet_view(&c, 0).unwrap();
    println!("{:?}", v.cells);
    let shown = v.cells.iter().find(|x| x.0 == "A1").map(|x| x.2.clone());
    assert_eq!(shown.as_deref(), Some("=== Totals ==="), "text that starts with = stays text");
}

#[test]
fn h_text_that_looks_like_values() {
    let f = tmp("tx.xlsx");
    let mut wb = rust_xlsxwriter::Workbook::new();
    let ws = wb.add_worksheet();
    for (i, t) in ["12%", "TRUE", "1/2/2024", "$5", "1,234", "13/01/2026", "2024-01-05", " 7 "].iter().enumerate() { ws.write_string(i as u32, 0, *t).unwrap(); }
    wb.save(&f).unwrap();
    let back = crate::io::sheetbook::import(&f).unwrap();
    let mut doc = base();
    op(&mut doc, Op::AddSheets { sheets: back.sheets, pictures: vec![], freeze: vec![] });
    let c = Calc::new(&doc.p, "%d/%m/%Y", d(2026, 11, 2));
    let v = crate::sheet::sheet_view(&c, 0).unwrap();
    let bad: Vec<_> = v.cells.iter().filter(|x| x.4.is_some()).map(|x| (x.1.clone(), x.2.clone())).collect();
    assert!(bad.is_empty(), "workbook text became numbers: {bad:?}");
}

#[test]
fn h_sheet_refs_and_quotes_round_trip() {
    let mut doc = base();
    op(&mut doc, Op::Sheet { k: None, sheet: Some(sheet("It's mine", &[("A1", "5")])) });
    op(&mut doc, Op::Sheet { k: None, sheet: Some(sheet("Q 2", &[("A1", "='It''s mine'!A1*2"), ("A2", "=\"say \"\"hi\"\"\""), ("A3", "=SUM('It''s mine'!A1:A3)")])) });
    let f = export_sheets(&doc.p).unwrap();
    let back = crate::io::sheetbook::import(&f).unwrap();
    let s = back.sheets.iter().find(|s| s.name == "Q 2").unwrap();
    println!("{:?}", s.cells);
    let mut d2 = base();
    op(&mut d2, Op::AddSheets { sheets: back.sheets.clone(), pictures: vec![], freeze: vec![] });
    let c = Calc::new(&d2.p, "%d/%m/%Y", d(2026, 11, 2));
    let v = crate::sheet::sheet_view(&c, 1).unwrap();
    println!("{:?}", v.cells);
    assert_eq!(v.cells.iter().find(|x| x.0 == "A1").map(|x| x.2.clone()).as_deref(), Some("10"));
}

#[test]
fn h_shared_formulas_import() {
    let f = tmp("sh.xlsx");
    let mut wb = rust_xlsxwriter::Workbook::new();
    let ws = wb.add_worksheet();
    for r in 0..4 { ws.write_number(r, 0, r as f64 + 1.0).unwrap(); }
    ws.write_formula(0, 1, "=A1*2").unwrap();
    wb.save(&f).unwrap();
    // Turn B1 into a shared formula over B1:B4, as spreadsheets write a filled-down column.
    let g = patch(&f, "xl/worksheets/sheet1.xml", |x| {
        let x = x.replace("<f>A1*2</f>", "<f t=\"shared\" ref=\"B1:B4\" si=\"0\">A1*2</f>");
        let mut x = x;
        for r in 2..=4 { x = x.replace(&format!("<c r=\"A{r}\"><v>{r}</v></c>"), &format!("<c r=\"A{r}\"><v>{r}</v></c><c r=\"B{r}\"><f t=\"shared\" si=\"0\"/><v>{}</v></c>", r * 2)); }
        x
    });
    let back = crate::io::sheetbook::import(&g).unwrap();
    println!("{:?}", back.sheets[0].cells);
    assert_eq!(back.sheets[0].cells.get("B3").map(String::as_str), Some("=A3*2"), "shared formulas expand");
}

#[test]
fn h_ods_formulas() {
    let f = tmp("o.ods");
    let mut w = zip::ZipWriter::new(std::fs::File::create(&f).unwrap());
    let st = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    w.start_file("mimetype", st).unwrap();
    w.write_all(b"application/vnd.oasis.opendocument.spreadsheet").unwrap();
    w.start_file("META-INF/manifest.xml", st).unwrap();
    w.write_all(br#"<?xml version="1.0" encoding="UTF-8"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.2"><manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/><manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/></manifest:manifest>"#).unwrap();
    w.start_file("content.xml", st).unwrap();
    w.write_all(br#"<?xml version="1.0" encoding="UTF-8"?><office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" office:version="1.2"><office:body><office:spreadsheet><table:table table:name="Sheet1"><table:table-row><table:table-cell office:value-type="float" office:value="1"><text:p>1</text:p></table:table-cell><table:table-cell office:value-type="float" office:value="2"><text:p>2</text:p></table:table-cell><table:table-cell table:formula="of:=[.A1]+[.B1]" office:value-type="float" office:value="3"><text:p>3</text:p></table:table-cell><table:table-cell table:formula="of:=SUM([.A1:.B1])" office:value-type="float" office:value="3"><text:p>3</text:p></table:table-cell></table:table-row></table:table></office:spreadsheet></office:body></office:document-content>"#).unwrap();
    w.finish().unwrap();
    let back = crate::io::sheetbook::import(&f).unwrap();
    println!("{:?}", back.sheets[0].cells);
    let mut doc = base();
    op(&mut doc, Op::AddSheets { sheets: back.sheets, pictures: vec![], freeze: vec![] });
    let c = Calc::new(&doc.p, "%d/%m/%Y", d(2026, 11, 2));
    let v = crate::sheet::sheet_view(&c, 0).unwrap();
    println!("{:?}", v.cells);
    assert_eq!(v.cells.iter().find(|x| x.0 == "C1").map(|x| x.2.clone()).as_deref(), Some("3"), "an .ods formula works");
}

// --- pictures -------------------------------------------------------------------------------------

const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
#[test]
fn h_picture_size_with_extlst() {
    let mut doc = base();
    op(&mut doc, Op::Picture { id: 0, pic: Some(Picture { id: 0, name: "Logo".into(), role: "Client".into(), data: format!("data:image/png;base64,{PNG}") }) });
    op(&mut doc, Op::Sheet { k: None, sheet: Some(sheet("P", &[("A1", "x")])) });
    op(&mut doc, Op::SheetImages { k: 0, images: vec![SheetImage { pic: 1, cell: "C3".into(), w: 200, h: 100, fit: false }] });
    let f = export_sheets(&doc.p).unwrap();
    let dx = part(&f, "xl/drawings/drawing1.xml");
    println!("{dx}");
    // As a spreadsheet writes it: an extension list in the picture's properties before its size.
    let g = patch(&f, "xl/drawings/drawing1.xml", |x| {
        let i = x.find("<xdr:cNvPr ").unwrap();
        let j = i + x[i..].find("/>").unwrap();
        format!("{}><a:extLst><a:ext uri=\"{{FF2B5EF4-FFF2-40B4-BE49-F238E27FC236}}\"><a16:creationId xmlns:a16=\"http://schemas.microsoft.com/office/drawing/2014/main\" id=\"{{1}}\"/></a:ext></a:extLst></xdr:cNvPr>{}", &x[..j], &x[j + 2..])
    });
    let back = crate::io::sheetbook::import(&g).unwrap();
    let im = &back.sheets[0].images[0];
    assert_eq!((im.cell.as_str(), im.w, im.h), ("C3", 200, 100), "size of a picture with an extension list");
}

// --- workbook XML checks --------------------------------------------------------------------------

#[test]
fn h_export_rich_sheet_xml() {
    let mut doc = base();
    op(&mut doc, Op::Picture { id: 0, pic: Some(Picture { id: 0, name: "Logo".into(), role: "Client".into(), data: format!("data:image/png;base64,{PNG}") }) });
    let mut s = sheet("My Sheet", &[("A1", "Title"), ("B2", "13/01/2026"), ("C2", "=B2+7")]);
    s.merges = vec!["A1:XFD1".into()];
    s.cond = vec![CondRule { range: "B2:B9".into(), op: "gt".into(), a: "5".into(), style: "c:#9C0006;f:#FFC7CE".into(), ..Default::default() }, CondRule { range: "C2:C9".into(), op: "dup".into(), style: "b".into(), ..Default::default() }];
    s.page = Some(Box::new(SheetPage { area: "A1:C20".into(), titles: "1:2".into(), breaks: vec![5, 10], ..Default::default() }));
    op(&mut doc, Op::Sheet { k: None, sheet: Some(s) });
    for i in 0..30 { op(&mut doc, Op::Sheet { k: None, sheet: Some(sheet(&format!("S{i}"), &[])) }); }
    op(&mut doc, Op::SheetImages { k: 0, images: vec![SheetImage { pic: 1, cell: "B5".into(), w: 50, h: 50, fit: true }] });
    let f = export_full(&doc.p).unwrap();
    let wbx = part(&f, "xl/workbook.xml");
    println!("{}", &wbx[wbx.find("<definedNames").unwrap_or(0)..]);
    let back = crate::io::sheetbook::import(&f).unwrap();
    let s = &back.sheets[0];
    println!("{:?}", s.page);
    assert_eq!(s.page.as_ref().map(|p| (p.area.as_str(), p.titles.as_str(), p.breaks.clone())), Some(("A1:C20", "1:2", vec![5, 10])));
}

#[test]
fn h_page_none_round_trip_keeps_gridlines() {
    let mut doc = base();
    op(&mut doc, Op::Sheet { k: None, sheet: Some(sheet("G", &[("A1", "1")])) });
    let f = export_sheets(&doc.p).unwrap();
    let back = crate::io::sheetbook::import(&f).unwrap();
    assert!(!back.sheets[0].page.as_ref().map(|p| p.no_grid).unwrap_or(false), "a sheet printed with gridlines still prints with them: {:?}", back.sheets[0].page);
}

// --- hostile workbooks ----------------------------------------------------------------------------

#[test]
fn h_hostile_workbook_never_panics() {
    let f = tmp("host.xlsx");
    let mut wb = rust_xlsxwriter::Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name("Ünïcødé ✓ 'x").unwrap();
    ws.write_string(1_048_575, 3, "far").unwrap();
    ws.write_string(0, 0, "日本語 🙂").unwrap();
    ws.write_formula(1, 0, "=_xlfn.XLOOKUP(1,A1:A3,A1:A3)").unwrap();
    ws.merge_range(5, 0, 6, 3, "m", &rust_xlsxwriter::Format::new()).unwrap();
    ws.set_row_height(1_048_575, 40).unwrap();
    ws.set_column_width(16_383, 30).unwrap();
    let ws2 = wb.add_worksheet();
    ws2.write_blank(3, 3, &rust_xlsxwriter::Format::new().set_bold()).unwrap();
    wb.add_worksheet();
    wb.save(&f).unwrap();
    // Overlapping merges and a huge column range, by hand.
    let g = patch(&f, "xl/worksheets/sheet1.xml", |x| x.replace("<mergeCell ref=\"A6:D7\"/>", "<mergeCell ref=\"A6:D7\"/><mergeCell ref=\"B6:E9\"/><mergeCell ref=\"A1048575:B1048576\"/>").replace("<mergeCells count=\"1\">", "<mergeCells count=\"3\">"));
    let g = patch(&g, "xl/worksheets/sheet1.xml", |x| x.replacen("<cols>", "<cols><col min=\"20\" max=\"4294967295\" width=\"9\" customWidth=\"1\"/>", 1));
    let back = crate::io::sheetbook::import(&g);
    let back = back.unwrap();
    println!("{:?}", back.sheets.iter().map(|s| (&s.name, s.cells.len(), &s.merges)).collect::<Vec<_>>());
    println!("{:?}", back.sheets[0].cells);
    let mut doc = base();
    op(&mut doc, Op::AddSheets { sheets: back.sheets, pictures: back.pictures, freeze: back.freeze });
    println!("{:?}", doc.p.sheets.iter().map(|s| (&s.name, &s.merges)).collect::<Vec<_>>());
    let _ = export_sheets(&doc.p).unwrap();
}

// --- ops ------------------------------------------------------------------------------------------

#[test]
fn h_layout_save_never_overwrites() {
    let mut doc = base();
    op(&mut doc, Op::Sheet { k: None, sheet: Some(sheet("L", &[("A1", "1")])) });
    let stale = doc.p.sheets[0].clone();
    op(&mut doc, Op::SheetCells { k: 0, cells: vec![("A2".into(), "2".into())], fmt: None });
    op(&mut doc, Op::SheetStyle { k: 0, cells: vec![("A2".into(), "b".into())] });
    op(&mut doc, Op::Sheet { k: Some(0), sheet: Some(SheetData { widths: [(0, 120)].into(), ..stale }) });
    let s = &doc.p.sheets[0];
    assert_eq!((s.cells.len(), s.style.len(), s.widths.get(&0)), (2, 1, Some(&120)));
    doc.undo();
    assert!(doc.p.sheets[0].widths.is_empty());
    doc.redo();
    assert_eq!(doc.p.sheets[0].widths.get(&0), Some(&120));
}

#[test]
fn h_insert_delete_moves_pictures_area_titles() {
    let mut doc = base();
    op(&mut doc, Op::Picture { id: 0, pic: Some(Picture { id: 0, name: "Logo".into(), role: "Client".into(), data: format!("data:image/png;base64,{PNG}") }) });
    let mut s = sheet("I", &[("A1", "Head"), ("A5", "x")]);
    s.page = Some(Box::new(SheetPage { area: "A2:D20".into(), titles: "2:2".into(), ..Default::default() }));
    op(&mut doc, Op::Sheet { k: None, sheet: Some(s) });
    op(&mut doc, Op::SheetImages { k: 0, images: vec![SheetImage { pic: 1, cell: "B6".into(), w: 50, h: 50, fit: false }] });
    // Delete rows 5..7 (0-based 4..6): the picture's row goes.
    op(&mut doc, Op::SheetInsert { k: 0, cols: false, at: 4, count: -3 });
    let im = doc.p.sheets[0].images[0].cell.clone();
    // Insert a row at the top: the print area and title rows move down.
    op(&mut doc, Op::SheetInsert { k: 0, cols: false, at: 0, count: 1 });
    let pg = doc.p.sheets[0].page.clone().unwrap();
    let im2 = doc.p.sheets[0].images[0].cell.clone();
    println!("pic after delete {im}, after insert {im2}; page {pg:?}");
    assert_eq!((pg.area.as_str(), pg.titles.as_str()), ("A3:D18", "3:3"), "print area and title rows follow inserted/deleted rows");
    assert_eq!(im, "B5", "a picture whose row was deleted moves to where the rows were");
}

#[test]
fn h_addsheets_names() {
    let mut doc = base();
    let long = "A".repeat(40);
    op(&mut doc, Op::AddSheets { sheets: vec![sheet(&long, &[]), sheet(&long, &[]), sheet("tasks", &[]), sheet("'x'", &[])], pictures: vec![], freeze: vec![] });
    println!("{:?}", doc.p.sheets.iter().map(|s| (s.name.clone(), s.name.chars().count())).collect::<Vec<_>>());
    assert!(doc.p.sheets.iter().all(|s| s.name.chars().count() <= 31));
    let r = export_full(&doc.p);
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn h_cond_xml_dump() {
    for r in [
        CondRule { range: "A1:A9".into(), op: "gt".into(), a: "".into(), style: "b".into(), ..Default::default() },
        CondRule { range: "A1:A9".into(), op: "between".into(), style: "b".into(), ..Default::default() },
        CondRule { range: "A1:A9".into(), op: "contains".into(), style: "b".into(), ..Default::default() },
        CondRule { range: "A1:A9".into(), op: "eq".into(), a: "say \"hi\"".into(), style: "b".into(), ..Default::default() },
        CondRule { range: "A1:A9".into(), op: "eq".into(), a: "Done".into(), style: "b".into(), ..Default::default() },
        CondRule { range: "A1:A9".into(), op: "contains".into(), a: "a\"b".into(), style: "b".into(), ..Default::default() },
        CondRule { range: "A1:A9".into(), op: "scale".into(), a: "red".into(), ..Default::default() },
    ] {
        let f = cond_export(r.clone()).unwrap();
        let x = part(&f, "xl/worksheets/sheet1.xml");
        let i = x.find("<conditionalFormatting").map(|i| x[i..].to_string()).unwrap_or_else(|| "NONE".into());
        println!("{} {:?}: {}", r.op, r.a, &i[..i.len().min(400)]);
    }
}

#[test]
fn h_far_cell_crash_probe() {
    let mut doc = base();
    op(&mut doc, Op::Sheet { k: None, sheet: Some(sheet("Far", &[("A1", "1"), ("XFD1048576", "x")])) });
    let f = export_sheets(&doc.p).unwrap();
    println!("exported; importing");
    if std::env::var("H22_FAR").is_ok() { let _ = crate::io::sheetbook::import(&f); }
}

#[test]
fn h_bad_names_refused() {
    let mut doc = base();
    for n in ["'Q1'", "Tasks", "datumplan", &"x".repeat(32), "a/b", " "] { assert!(doc.apply(Op::Sheet { k: None, sheet: Some(sheet(n, &[])) }, "%d/%m/%Y").is_err(), "{n}"); }
}

#[test]
fn a_binary_sheets_size_is_read_before_anything_is_built() {
    // A record before it (type 0x81, 0 bytes), then BrtWsDim: rows 0..1,048,575, columns 0..16,383.
    let mut b = vec![0x81, 0x01, 0x00, 0x94, 0x01, 0x10];
    for x in [0u32, 1_048_575, 0, 16_383] { b.extend_from_slice(&x.to_le_bytes()); }
    assert_eq!(crate::io::sheetbook::xlsb_dim(&b), Some((0, 1_048_575, 0, 16_383)));
    assert_eq!(crate::io::sheetbook::xlsb_dim(&[0x81, 0x01]), None, "cut short: nothing read");
}

#[test]
fn column_page_breaks_go_to_the_workbook_and_back() {
    let mut doc = base();
    let mut s = sheet("Wide", &[("A1", "1"), ("Z1", "2")]);
    s.page = Some(Box::new(SheetPage { breaks: vec![12, 4, 4], col_breaks: vec![8, 0, 3], ..Default::default() }));
    op(&mut doc, Op::Sheet { k: None, sheet: Some(s) });
    assert_eq!(doc.p.sheets[0].page.as_ref().map(|p| (p.breaks.clone(), p.col_breaks.clone())), Some((vec![4, 12], vec![3, 8])), "sorted, no duplicates, none at the first row or column");
    let f = export_sheets(&doc.p).unwrap();
    let x = part(&f, "xl/worksheets/sheet1.xml");
    assert!(x.contains("<colBreaks") && x.contains("<rowBreaks"), "{x}");
    // Read from the sheet's own XML, as a workbook from another program would be.
    let g = patch(&f, "xl/worksheets/sheet1.xml", |x| x);
    let back = crate::io::sheetbook::import(&g).unwrap();
    assert_eq!(back.sheets[0].page.as_ref().map(|p| (p.breaks.clone(), p.col_breaks.clone())), Some((vec![4, 12], vec![3, 8])));
    // Inserting two columns before C moves both column breaks.
    op(&mut doc, Op::SheetInsert { k: 0, cols: true, at: 1, count: 2 });
    assert_eq!(doc.p.sheets[0].page.as_ref().unwrap().col_breaks, vec![5, 10]);
}
