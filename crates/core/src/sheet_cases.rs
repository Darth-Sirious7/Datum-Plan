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

#[test]
fn notes_filters_and_drop_down_lists_go_to_the_workbook_and_back() {
    let mut doc = base();
    let mut s = sheet("Vendors", &[("A1", "Vendor"), ("B1", "Country"), ("C1", "Value"), ("A2", "Steel Co"), ("B2", "Germany"), ("C2", "100"), ("A3", "Pipe Ltd"), ("B3", "Italy"), ("C3", "250"), ("A4", "Valve AG"), ("B4", "Germany"), ("C4", "40")]);
    s.filter = Some(SheetFilter { range: "A1:C4".into(), cols: vec![ColFilter { col: 1, values: Some(vec!["Germany".into()]), ..Default::default() }, ColFilter { col: 9, op: "gt".into(), ..Default::default() }] });
    s.valid = vec![Validation { range: "D2:D4".into(), list: vec!["Open".into(), "Closed".into()], ..Default::default() }, Validation { range: "E2:E4".into(), src: "$B$2:$B$4".into(), warn: true, ..Default::default() }, Validation { range: "bad".into(), list: vec!["x".into()], ..Default::default() }];
    op(&mut doc, Op::Sheet { k: None, sheet: Some(s) });
    assert_eq!(doc.p.sheets[0].filter.as_ref().unwrap().cols.len(), 1, "a column outside the range goes");
    assert_eq!(doc.p.sheets[0].valid.len(), 2, "a list on no range goes");
    op(&mut doc, Op::SheetNotes { k: 0, cells: vec![("b2".into(), "Price confirmed\nby email".into()), ("C9".into(), " ".into())] });
    assert_eq!(doc.p.sheets[0].notes.keys().collect::<Vec<_>>(), vec!["B2"]);
    // A layout edit from the window keeps the notes (they have their own edit).
    let mut copy = doc.p.sheets[0].clone();
    copy.notes.clear();
    op(&mut doc, Op::Sheet { k: Some(0), sheet: Some(copy) });
    assert!(doc.p.sheets[0].notes.contains_key("B2"));
    let c = Calc::new(&doc.p, "%d/%m/%Y", d(2026, 11, 2));
    let v = crate::sheet::sheet_view(&c, 0).unwrap();
    assert_eq!((v.filtered.clone(), v.lists.clone()), (vec![2], vec![vec!["Open".to_string(), "Closed".into()], vec!["Germany".into(), "Italy".into()]]));
    let f = export_sheets(&doc.p).unwrap();
    let x = part(&f, "xl/worksheets/sheet1.xml");
    assert!(x.contains("<autoFilter ref=\"A1:C4\"") && x.contains("<dataValidation") && x.contains("hidden=\"1\""), "{x}");
    assert!(part(&f, "xl/comments1.xml").contains("Price confirmed"));
    let back = crate::io::sheetbook::import(&f).unwrap();
    let b = &back.sheets[0];
    assert_eq!((&b.filter, &b.valid), (&doc.p.sheets[0].filter, &doc.p.sheets[0].valid));
    assert_eq!(b.notes.get("B2").map(String::as_str), Some("Price confirmed\nby email"));
    // From the workbook's own parts, as one from another program (no Datum Plan sheet).
    let g = patch(&f, "xl/workbook.xml", |x| x.replace("name=\"DatumPlan\"", "name=\"Other\""));
    let b2 = crate::io::sheetbook::import(&g).unwrap();
    let b2 = b2.sheets.iter().find(|s| s.name == "Vendors").unwrap();
    assert_eq!(b2.filter.as_ref().map(|f| (f.range.clone(), f.cols[0].col, f.cols[0].values.clone())), Some(("A1:C4".into(), 1, Some(vec!["Germany".into()]))));
    assert_eq!(b2.valid.iter().map(|v| (v.range.clone(), v.list.clone(), v.src.clone(), v.warn)).collect::<Vec<_>>(), vec![("D2:D4".into(), vec!["Open".to_string(), "Closed".into()], String::new(), false), ("E2:E4".into(), vec![], "$B$2:$B$4".into(), true)]);
    assert_eq!(b2.notes.get("B2").map(String::as_str), Some("Price confirmed\nby email"));
    // Inserting a column before B moves the filtered column, the lists and the note.
    op(&mut doc, Op::SheetInsert { k: 0, cols: true, at: 1, count: 1 });
    let s = &doc.p.sheets[0];
    assert_eq!((s.filter.as_ref().map(|f| (f.range.clone(), f.cols[0].col)), s.valid[0].range.clone(), s.notes.contains_key("C2")), (Some(("A1:D4".into(), 2)), "E2:E4".into(), true));
}

#[test]
fn remove_duplicates_keeps_the_first_of_each() {
    let mut doc = base();
    op(&mut doc, Op::Sheet { k: None, sheet: Some(sheet("V", &[("A1", "Vendor"), ("B1", "Country"), ("A2", "Steel Co"), ("B2", "DE"), ("A3", "steel co "), ("B3", "de"), ("A4", "Pipe"), ("B4", "IT"), ("A5", "Steel Co"), ("B5", "FR"), ("D3", "keep")])) });
    // By both columns: row 3 is row 2 again (case and spaces do not count).
    op(&mut doc, Op::SheetDedupe { k: 0, range: "A1:B5".into(), cols: vec![], header: true });
    let col = |d: &Doc, c: &str| (1..=5).map(|r| d.p.sheets[0].cells.get(&format!("{c}{r}")).cloned().unwrap_or_default()).collect::<Vec<_>>();
    assert_eq!(col(&doc, "A"), vec!["Vendor", "Steel Co", "Pipe", "Steel Co", ""]);
    assert_eq!(doc.p.sheets[0].cells.get("D3").map(String::as_str), Some("keep"), "cells outside the range stay");
    // By the vendor only.
    op(&mut doc, Op::SheetDedupe { k: 0, range: "A1:B4".into(), cols: vec![0], header: true });
    assert_eq!((col(&doc, "A"), col(&doc, "B")), (vec!["Vendor".to_string(), "Steel Co".into(), "Pipe".into(), "".into(), "".into()], vec!["Country".to_string(), "DE".into(), "IT".into(), "".into(), "".into()]));
}

#[test]
fn inserting_and_deleting_cells_moves_only_their_columns() {
    let mut doc = base();
    op(&mut doc, Op::Sheet { k: None, sheet: Some(sheet("S", &[("A1", "a"), ("B1", "1"), ("B2", "2"), ("B3", "3"), ("C2", "x"), ("D1", "=SUM(B1:B3)"), ("D2", "=B3")])) });
    op(&mut doc, Op::SheetNotes { k: 0, cells: vec![("B2".into(), "two".into())] });
    op(&mut doc, Op::SheetShift { k: 0, range: "B2".into(), down: true, del: false });
    let s = &doc.p.sheets[0];
    assert_eq!((s.cells.get("B3").map(String::as_str), s.cells.get("B4").map(String::as_str), s.cells.get("B2"), s.cells.get("C2").map(String::as_str)), (Some("2"), Some("3"), None, Some("x")));
    assert_eq!((s.cells["D1"].as_str(), s.cells["D2"].as_str(), s.notes.contains_key("B3")), ("=SUM(B1:B4)", "=B4", true));
    op(&mut doc, Op::SheetShift { k: 0, range: "B2:B3".into(), down: true, del: true });
    let s = &doc.p.sheets[0];
    assert_eq!((s.cells.get("B2").map(String::as_str), s.cells.get("B3"), s.cells["D2"].as_str()), (Some("3"), None, "=B2"));
    // Left: the cells to the right of A1 in row 1 move left.
    op(&mut doc, Op::SheetShift { k: 0, range: "A1".into(), down: false, del: true });
    assert_eq!(doc.p.sheets[0].cells.get("A1").map(String::as_str), Some("1"));
    // Part of a merged cell cannot move.
    let s = SheetData { merges: vec!["F1:G2".into()], ..doc.p.sheets[0].clone() };
    op(&mut doc, Op::Sheet { k: Some(0), sheet: Some(s) });
    assert!(doc.apply(Op::SheetShift { k: 0, range: "G1".into(), down: true, del: false }, "%d/%m/%Y").is_err());
    assert!(doc.apply(Op::SheetShift { k: 0, range: "F1:G1".into(), down: true, del: false }, "%d/%m/%Y").is_ok(), "a whole merge moves");
    assert_eq!(doc.p.sheets[0].merges, vec!["F2:G3"]);
}

// --- defined names, validation beyond lists, links, line breaks, shifting cells ----------------

fn shown(doc: &Doc, k: usize, a1: &str) -> String {
    let c = Calc::new(&doc.p, "%d/%m/%Y", d(2026, 11, 2));
    crate::sheet::sheet_view(&c, k).unwrap().cells.iter().find(|x| x.0 == a1).map(|x| x.2.clone()).unwrap_or_default()
}
fn dn(name: &str, formula: &str) -> DefName { DefName { name: name.into(), formula: formula.into(), ..Default::default() } }

#[test]
fn defined_names_work_in_formulas_follow_their_sheet_and_go_to_the_workbook() {
    let mut doc = base();
    op(&mut doc, Op::Sheet { k: None, sheet: Some(sheet("Prices", &[("B2", "100"), ("B3", "200"), ("C1", "=SUM(Rates)*(1+VAT)"), ("C2", "=Twice(4)")])) });
    op(&mut doc, Op::Names { names: vec![dn("Rates", "Prices!$B$2:$B$3"), dn(" VAT ", "=0.05"), dn("Twice", "=LAMBDA(x,x*2)")] });
    assert_eq!(doc.p.names.iter().map(|n| (n.name.as_str(), n.formula.as_str())).collect::<Vec<_>>(), vec![("Rates", "=Prices!$B$2:$B$3"), ("VAT", "=0.05"), ("Twice", "=LAMBDA(x,x*2)")], "trimmed, each a formula");
    assert_eq!((shown(&doc, 0, "C1"), shown(&doc, 0, "C2")), ("315".to_string(), "8".to_string()));
    // Refused: a cell address, a plan name, two of the same, a formula that does not read; the names stay as they were.
    for bad in [vec![dn("A1", "=1")], vec![dn("ProjectFinish", "=1")], vec![dn("x", "=1"), dn("X", "=2")], vec![dn("y", "=SUM(")], vec![dn("z", "")]] {
        assert!(doc.apply(Op::Names { names: bad.clone() }, "%d/%m/%Y").is_err(), "{bad:?}");
    }
    assert_eq!(doc.p.names.len(), 3);
    // Renaming the sheet, and inserting a row above the range, rewrite the name.
    let s = SheetData { name: "Price list".into(), ..doc.p.sheets[0].clone() };
    op(&mut doc, Op::Sheet { k: Some(0), sheet: Some(s) });
    op(&mut doc, Op::SheetInsert { k: 0, cols: false, at: 0, count: 1 });
    assert_eq!(doc.p.names[0].formula, "='Price list'!$B$3:$B$4");
    assert_eq!(shown(&doc, 0, "C2"), "315");
    // Undo brings the old names back.
    doc.undo();
    doc.undo();
    assert_eq!(doc.p.names[0].formula, "=Prices!$B$2:$B$3");
    // To the workbook as defined names, and back from its own parts.
    let f = export_sheets(&doc.p).unwrap();
    let wbx = part(&f, "xl/workbook.xml");
    assert!(wbx.contains("<definedName name=\"Rates\">Prices!$B$2:$B$3</definedName>") && wbx.contains("<definedName name=\"VAT\">0.05</definedName>"), "{wbx}");
    let back = crate::io::sheetbook::import(&f).unwrap();
    let got: Vec<(String, String)> = back.names.iter().map(|n| (n.name.clone(), n.formula.clone())).collect();
    assert!(got.contains(&("Rates".into(), "=Prices!$B$2:$B$3".into())) && got.contains(&("VAT".into(), "=0.05".into())) && !got.iter().any(|n| n.0 == "ProjectFinish"), "{got:?}");
    // An older plan file without names still reads; a damaged name goes.
    let mut p = doc.p.clone();
    p.names.push(dn("B2", "=1"));
    p.normalize();
    assert_eq!(p.names.len(), 3);
    let old: Project = serde_json::from_str(&serde_json::to_string(&Project::new("Old", d(2026, 1, 5))).unwrap().replace(",\"names\":[]", "")).unwrap();
    assert!(old.names.is_empty());
}

#[test]
fn validation_takes_numbers_dates_times_and_lengths() {
    let mut doc = base();
    let mut s = sheet("V", &[("B1", "10"), ("A2", "5"), ("A3", "12"), ("A4", "2.5"), ("C2", "15/11/2026"), ("C3", "15/01/2027"), ("D2", "abc"), ("D3", "abcdefgh"), ("E2", "13:30")]);
    s.valid = vec![
        Validation { range: "A2:A9".into(), kind: "whole".into(), op: "between".into(), a: "1".into(), b: "=$B$1".into(), prompt: "1 to the limit in B1".into(), ..Default::default() },
        Validation { range: "C2:C9".into(), kind: "date".into(), op: "le".into(), a: "31/12/2026".into(), warn: true, error: "Before year end".into(), ..Default::default() },
        Validation { range: "D2:D9".into(), kind: "len".into(), op: "le".into(), a: "5".into(), ..Default::default() },
        Validation { range: "E2:E9".into(), kind: "time".into(), op: "ge".into(), a: "12:00".into(), ..Default::default() },
        Validation { range: "F1".into(), kind: "whole".into(), ..Default::default() },
        Validation { range: "G1".into(), kind: "odd".into(), a: "1".into(), ..Default::default() },
    ];
    op(&mut doc, Op::Sheet { k: None, sheet: Some(s) });
    assert_eq!(doc.p.sheets[0].valid.len(), 4, "a rule with no value and an unknown kind go");
    let c = Calc::new(&doc.p, "%d/%m/%Y", d(2026, 11, 2));
    assert_eq!(crate::sheet::sheet_view(&c, 0).unwrap().invalid, vec!["A3", "A4", "C3", "D3"]);
    let chk = |a1: &str, v: &str| crate::sheet::check_input(&c, 0, a1, v);
    assert_eq!(chk("A5", "7"), None);
    assert_eq!(chk("A5", "11"), Some(("This cell takes a whole number between 1 and $B$1.".into(), false)));
    assert_eq!(chk("A5", "=B1+1"), Some(("This cell takes a whole number between 1 and $B$1.".into(), false)), "a formula's result is checked");
    assert_eq!(chk("C4", "01/02/2027"), Some(("Before year end".into(), true)), "only a warning");
    assert_eq!((chk("D4", "abcde"), chk("E4", "09:15").is_some(), chk("E4", "14:00"), chk("Z9", "x"), chk("A6", "")), (None, true, None, None, None));
    // To the workbook as its own rules, and back (as from another program: no Datum Plan sheet).
    let f = export_sheets(&doc.p).unwrap();
    let x = part(&f, "xl/worksheets/sheet1.xml");
    assert!(x.contains("type=\"whole\"") && x.contains("<formula2>$B$1</formula2>") && x.contains("type=\"date\"") && x.contains("operator=\"lessThanOrEqual\"") && x.contains("type=\"textLength\"") && x.contains("prompt=\"1 to the limit in B1\""), "{x}");
    let g = patch(&f, "xl/workbook.xml", |x| x.replace("name=\"DatumPlan\"", "name=\"Other\""));
    let b = crate::io::sheetbook::import(&g).unwrap();
    let v = &b.sheets.iter().find(|s| s.name == "V").unwrap().valid;
    assert_eq!(v.iter().map(|v| (v.range.as_str(), v.kind.as_str(), v.op.as_str(), v.a.as_str(), v.b.as_str(), v.warn)).collect::<Vec<_>>(),
        vec![("A2:A9", "whole", "between", "1", "=$B$1", false), ("C2:C9", "date", "le", "46387", "", true), ("D2:D9", "len", "le", "5", "", false), ("E2:E9", "time", "ge", "0.5", "", false)]);
    assert_eq!((v[0].prompt.as_str(), v[1].error.as_str()), ("1 to the limit in B1", "Before year end"));
    // A workbook written here comes back exactly.
    assert_eq!(crate::io::sheetbook::import(&f).unwrap().sheets[0].valid, doc.p.sheets[0].valid);
}

#[test]
fn links_show_open_only_web_and_mail_and_go_to_the_workbook() {
    let mut doc = base();
    op(&mut doc, Op::Sheet { k: None, sheet: Some(sheet("L", &[("A1", "https://example.com/spec?x=1"), ("A2", "=HYPERLINK(\"mailto:site@example.com\",\"Mail the site\")"), ("A3", "=HYPERLINK(B9&\"/docs\",\"Docs\")"), ("B9", "https://example.org"),
        ("A4", "javascript:alert(1)"), ("A5", "=HYPERLINK(\"file:///C:/x.exe\",\"x\")"), ("A6", "see https://example.com")])) });
    let c = Calc::new(&doc.p, "%d/%m/%Y", d(2026, 11, 2));
    let v = crate::sheet::sheet_view(&c, 0).unwrap();
    assert_eq!(v.links, vec![("A1".to_string(), "https://example.com/spec?x=1".to_string()), ("A2".into(), "mailto:site@example.com".into()), ("A3".into(), "https://example.org/docs".into()), ("B9".into(), "https://example.org".into())]);
    assert_eq!(v.cells.iter().find(|x| x.0 == "A2").map(|x| x.2.as_str()), Some("Mail the site"));
    let f = export_sheets(&doc.p).unwrap();
    let x = part(&f, "xl/worksheets/sheet1.xml");
    assert!(x.contains("<hyperlink ref=\"A1\"") && x.contains("HYPERLINK(\"mailto:site@example.com\""), "{x}");
    assert!(part(&f, "xl/worksheets/_rels/sheet1.xml.rels").contains("Target=\"https://example.com/spec?x=1\""));
    let b = crate::io::sheetbook::import(&f).unwrap();
    assert_eq!((b.sheets[0].cells["A1"].as_str(), b.sheets[0].cells["A2"].as_str()), ("https://example.com/spec?x=1", "=HYPERLINK(\"mailto:site@example.com\",\"Mail the site\")"));
    // A workbook link on a cell that shows other text comes in as HYPERLINK; one to a file does not.
    let g = patch(&f, "xl/worksheets/_rels/sheet1.xml.rels", |x| x.replace("https://example.com/spec?x=1", "https://example.com/other"));
    let g = patch(&g, "xl/sharedStrings.xml", |x| x.replace(">https://example.com/spec?x=1<", ">Spec<"));
    assert_eq!(crate::io::sheetbook::import(&g).unwrap().sheets[0].cells["A1"], "=HYPERLINK(\"https://example.com/other\",\"Spec\")");
    let h = patch(&f, "xl/worksheets/_rels/sheet1.xml.rels", |x| x.replace("https://example.com/spec?x=1", "file:///C:/run.exe"));
    assert_eq!(crate::io::sheetbook::import(&h).unwrap().sheets[0].cells["A1"], "https://example.com/spec?x=1");
}

#[test]
fn line_breaks_stay_in_the_cell_and_wrap_in_the_workbook() {
    let mut doc = base();
    op(&mut doc, Op::Sheet { k: None, sheet: Some(sheet("N", &[("A1", "Line one\nLine two")])) });
    assert_eq!(shown(&doc, 0, "A1"), "Line one\nLine two");
    let f = export_sheets(&doc.p).unwrap();
    assert!(part(&f, "xl/styles.xml").contains("wrapText=\"1\""));
    assert_eq!(crate::io::sheetbook::import(&f).unwrap().sheets[0].cells["A1"], "Line one\nLine two");
}

#[test]
fn shifting_cells_moves_formats_filters_lists_and_notes_with_them() {
    let mut doc = base();
    let mut s = sheet("S", &[("A1", "Item"), ("B1", "Qty"), ("A2", "a"), ("B2", "1"), ("A3", "b"), ("B3", "2"), ("D1", "x")]);
    s.cond = vec![CondRule { range: "B2:B3".into(), op: "gt".into(), a: "1".into(), style: "b".into(), ..Default::default() }, CondRule { range: "D1:D2".into(), op: "blank".into(), style: "b".into(), ..Default::default() }, CondRule { range: "A1:D9".into(), op: "blank".into(), style: "i".into(), ..Default::default() }];
    s.valid = vec![Validation { range: "B2:B3".into(), kind: "whole".into(), op: "ge".into(), a: "0".into(), ..Default::default() }];
    s.filter = Some(SheetFilter { range: "A1:B3".into(), cols: vec![ColFilter { col: 1, op: "gt".into(), a: "0".into(), ..Default::default() }] });
    op(&mut doc, Op::Sheet { k: None, sheet: Some(s) });
    op(&mut doc, Op::SheetNotes { k: 0, cells: vec![("B3".into(), "checked".into())] });
    // Two cells inserted at A1:B1, the rest moving down within columns A and B.
    op(&mut doc, Op::SheetShift { k: 0, range: "A1:B1".into(), down: true, del: false });
    let s = &doc.p.sheets[0];
    assert_eq!(s.cond.iter().map(|c| c.range.as_str()).collect::<Vec<_>>(), vec!["B3:B4", "D1:D2", "A1:D9"], "in the band they move; across its edge they stay");
    assert_eq!((s.valid[0].range.as_str(), s.filter.as_ref().map(|f| f.range.as_str()), s.notes.contains_key("B4")), ("B3:B4", Some("A2:B4"), true));
    // Deleting column A's cells in rows 1 to 9 moves column B left: the filter's column follows its heading.
    op(&mut doc, Op::SheetShift { k: 0, range: "A1:A9".into(), down: false, del: true });
    let s = &doc.p.sheets[0];
    assert_eq!((s.valid[0].range.as_str(), s.filter.as_ref().map(|f| (f.range.as_str(), f.cols.iter().map(|c| c.col).collect::<Vec<_>>()))), ("A3:A4", Some(("A2:A4", vec![0]))));
    assert_eq!(s.cond[0].range, "A3:A4");
    // Deleting all of a range's cells removes the rule.
    op(&mut doc, Op::SheetShift { k: 0, range: "A3:A4".into(), down: true, del: true });
    let s = &doc.p.sheets[0];
    assert!(s.valid.is_empty() && s.cond.iter().all(|c| c.range != "A3:A4"), "{:?}", s.cond);
    assert_eq!(s.filter.as_ref().map(|f| f.range.as_str()), Some("A2"));
    doc.undo();
    assert_eq!(doc.p.sheets[0].valid[0].range, "A3:A4", "undo puts it back");
}

#[test]
fn remove_duplicates_shrinks_the_ranges_on_the_list() {
    let mut doc = base();
    let mut s = sheet("D", &[("A1", "Vendor"), ("A2", "Steel"), ("A3", "steel"), ("A4", "Pipe"), ("A5", "Pipe")]);
    s.cond = vec![CondRule { range: "A2:A5".into(), op: "dup".into(), style: "b".into(), ..Default::default() }];
    s.valid = vec![Validation { range: "A2:A5".into(), kind: "len".into(), op: "le".into(), a: "20".into(), ..Default::default() }];
    s.filter = Some(SheetFilter { range: "A1:A5".into(), cols: vec![] });
    op(&mut doc, Op::Sheet { k: None, sheet: Some(s) });
    op(&mut doc, Op::SheetNotes { k: 0, cells: vec![("A4".into(), "main supplier".into())] });
    op(&mut doc, Op::SheetDedupe { k: 0, range: "A1:A5".into(), cols: vec![], header: true });
    let s = &doc.p.sheets[0];
    assert_eq!((s.cond[0].range.as_str(), s.valid[0].range.as_str(), s.filter.as_ref().unwrap().range.as_str()), ("A2:A3", "A2:A3", "A1:A3"));
    assert_eq!((s.cells.get("A3").map(String::as_str), s.notes.contains_key("A3"), s.notes.len()), (Some("Pipe"), true, 1));
}
