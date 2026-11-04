//! Workbooks into Sheets: every sheet of an .xlsx, .xlsm, .xls, .xlsb or .ods file becomes a sheet tab,
//! with its values, formulas, number formats, cell styles (fonts, fills, borders, alignment), merged
//! cells, column widths, row heights, tab colour, frozen panes, page setup and pictures. A workbook
//! written by Datum Plan also brings back what only Datum Plan knows (country flags), from its hidden
//! "DatumPlan" sheet.
use crate::model::{Picture, SheetData, SheetImage, SheetPage};
use calamine::{open_workbook_auto, Data, Reader, Sheets};
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::Path;

/// The hidden sheet in which an exported workbook keeps what only Datum Plan shows.
pub const META_SHEET: &str = "DatumPlan";
/// Sheets an export adds for the plan itself; they are not brought back as tabs.
const PLAN_SHEETS: [&str; 3] = ["Tasks", "Plan", "Resources"];

/// What a workbook brings: the sheets, the pictures they use (ids 1, 2, … within this import) and
/// each sheet's frozen rows and columns.
#[derive(Debug, Default)]
pub struct Imported { pub sheets: Vec<SheetData>, pub pictures: Vec<Picture>, pub freeze: Vec<(String, u32, u32)> }

pub fn import(path: &Path) -> Result<Imported, String> {
    let mut wb = open_workbook_auto(path).map_err(|e| format!("Could not read the workbook: {e}"))?;
    let names = wb.sheet_names().to_vec();
    if names.len() > 200 { return Err("That workbook has more than 200 sheets.".into()) }
    // A workbook written by Datum Plan: its flags, and the plan's own sheets to leave out.
    let meta: serde_json::Value = if names.iter().any(|n| n == META_SHEET) {
        wb.worksheet_range(META_SHEET).ok().and_then(|r| r.get_value((0, 0)).map(|v| v.to_string())).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    } else { serde_json::Value::Null };
    let ours = !meta.is_null();
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    // Calamine builds the whole box around a sheet's cells: two far-apart cells would ask for
    // hundreds of gigabytes (an abort, not an error). Measure first, as the table import does.
    if matches!(ext.as_str(), "xls" | "ods") { super::table::sheet_size(path, &ext)?; }
    if ext == "xlsb" { xlsb_size(path)?; }
    let parts = if matches!(ext.as_str(), "xlsx" | "xlsm") { xml_parts(path).ok() } else { None };
    let mut out = Imported::default();
    // Its own tabs, as the workbook lists them (older files: every sheet but the plan's).
    let tabs: Option<Vec<&str>> = meta["tabs"].as_array().map(|t| t.iter().filter_map(|x| x.as_str()).collect());
    for name in names.iter().filter(|n| *n != META_SHEET && tabs.as_ref().map_or(!(ours && PLAN_SHEETS.contains(&n.as_str())), |t| t.contains(&n.as_str()))) {
        let mut s = SheetData { name: name.clone(), ..Default::default() };
        if parts.as_ref().and_then(|p| p.sheets.get(name)).is_some_and(|l| l.span > 10_000_000) {
            return Err(format!("The sheet {name} spans more than 10 million cells (something far to the right or at the bottom). Clear the far cells in the workbook and try again."))
        }
        let values = wb.worksheet_range(name).map_err(|e| format!("Could not read the sheet {name}: {e}"))?;
        let formulas = wb.worksheet_formula(name).ok();
        let (vr, vc) = values.start().unwrap_or((0, 0));
        let mut cells = 0usize;
        for (r, c, v) in values.used_cells() {
            let (r, c) = (r as u32 + vr, c as u32 + vc);
            if r > 100_000 || c > 1_000 { continue }
            let a1 = a1(r, c);
            let input = match v {
                Data::Int(n) => n.to_string(),
                Data::Float(x) => x.to_string(),
                Data::Bool(b) => if *b { "TRUE".into() } else { "FALSE".into() },
                Data::DateTime(d) => { s.fmt.entry(a1.clone()).or_insert_with(|| "date".into()); d.as_f64().to_string() }
                Data::String(t) if t.starts_with(['=', '\'']) && t.len() > 1 => format!("'{t}"),
                Data::String(t) => {
                    // Text the plan would read as a number, date, percentage or TRUE stays text.
                    let looks = !matches!(gridcraft_core::parse::parse_input(t, gridcraft_core::DateSystem::default()).value, gridcraft_core::Value::Text(_))
                        || t.trim().starts_with(|ch: char| ch.is_ascii_digit()) && t.contains(['/', '-', '.']);
                    if looks { s.fmt.insert(a1.clone(), "@".into()); }
                    t.clone()
                }
                Data::DateTimeIso(t) | Data::DurationIso(t) => t.clone(),
                _ => String::new(),
            };
            if !input.is_empty() { s.cells.insert(a1, input); cells += 1; }
            if cells > 500_000 { return Err(format!("The sheet {name} has more than 500,000 filled cells.")) }
        }
        if let Some(f) = formulas {
            let (fr, fc) = f.start().unwrap_or((0, 0));
            for (r, c, t) in f.used_cells() {
                if t.is_empty() { continue }
                let (r, c) = (r as u32 + fr, c as u32 + fc);
                // OpenDocument formulas ("of:=SUM([.A1:.B2])"); one that does not read keeps its value.
                let t = if t.starts_with("of:") { match ods_formula(t) { Some(f) => f, None => continue } } else { t.clone() };
                let t = t.replace("_xlfn._xlws.", "").replace("_xlfn.", "").replace("_xlws.", "").replace("_xlpm.", "");
                s.cells.insert(a1(r, c), format!("={}", t.trim_start_matches('=')));
            }
        }
        if let Sheets::Xlsx(x) = &mut wb {
            if let Ok(st) = x.worksheet_style(name) {
                for (r, c, style) in st.cells() {
                    if r > 100_000 || c > 1_000 { continue }
                    let a1 = a1(r, c);
                    let text = style_text(style);
                    if !text.is_empty() { s.style.insert(a1.clone(), text); }
                    if let Some(nf) = style.number_format.as_ref().map(|n| n.format_code.trim().to_string()).filter(|f| !f.is_empty() && f != "General") {
                        if s.cells.contains_key(&a1) || !s.style.contains_key(&a1) { s.fmt.insert(a1, if nf == "@" { "@".into() } else { nf }); }
                    }
                }
            }
            if let Ok(ms) = x.merge_cells_by_sheet_name(name) {
                s.merges = ms.iter().take(5_000).map(|d| format!("{}:{}", a1(d.start.0, d.start.1), a1(d.end.0, d.end.1))).collect();
            }
        }
        // Flags Datum Plan wrote as country names.
        for a in meta["sheets"][name.as_str()]["flags"].as_array().into_iter().flatten().filter_map(|a| a.as_str()) { s.fmt.insert(a.to_string(), "flag".into()); }
        if let Some(p) = parts.as_ref().and_then(|p| p.sheets.get(name)) {
            s.widths = p.widths.clone();
            s.hide_rows = p.hide_rows.clone();
            s.hide_cols = p.hide_cols.clone();
            s.heights = p.heights.clone();
            s.color = p.color.clone();
            s.hidden = p.hidden;
            s.page = p.page.clone().map(Box::new);
            if let Some((r, c)) = p.freeze { out.freeze.push((name.clone(), r, c)); }
            for (cell, w, h, data, mime) in &p.pictures {
                let id = out.pictures.len() as u32 + 1;
                out.pictures.push(Picture { id, name: format!("{name} picture {id}"), role: "Other".into(), data: format!("data:{mime};base64,{}", base64(data)) });
                s.images.push(SheetImage { pic: id, cell: cell.clone(), w: *w, h: *h, fit: meta["sheets"][name.as_str()]["fit"].as_array().is_some_and(|f| f.iter().any(|x| x.as_str() == Some(cell))) });
            }
        }
        if let Ok(cond) = serde_json::from_value::<Vec<crate::model::CondRule>>(meta["sheets"][name.as_str()]["cond"].clone()) { s.cond = cond; }
        if meta["sheets"][name.as_str()]["page"].as_bool() == Some(false) { s.page = None; }
        if meta["sheets"][name.as_str()]["no_hf"].as_bool() == Some(true) { s.page.get_or_insert_with(Default::default).no_hf = true; }
        out.sheets.push(s);
    }
    if out.sheets.is_empty() { return Err("That workbook has no sheets to bring in.".into()) }
    Ok(out)
}

/// An OpenDocument formula in the window's syntax: [.A1] → A1, [Sheet2.A1:.B2] → Sheet2!A1:B2, ; → ,.
fn ods_formula(t: &str) -> Option<String> {
    let body = t.strip_prefix("of:")?.trim_start_matches('=');
    let (mut out, mut q, mut it) = (String::new(), false, body.chars());
    while let Some(ch) = it.next() {
        match ch {
            '"' => { q = !q; out.push(ch) }
            '[' if !q => {
                let r: String = it.by_ref().take_while(|c| *c != ']').collect();
                let parts: Vec<String> = r.split(':').enumerate().map(|(i, p)| match p.trim_start_matches('$').rsplit_once('.') { Some((sh, a)) if !sh.is_empty() && i == 0 => format!("{sh}!{a}"), Some((_, a)) => a.to_string(), None => p.to_string() }).collect();
                out.push_str(&parts.join(":"));
            }
            ';' if !q => out.push(','),
            _ => out.push(ch),
        }
    }
    gridcraft_formula::parse(&out).ok().map(|_| out)
}

fn a1(r: u32, c: u32) -> String { format!("{}{}", crate::sheet::col_name(c), r + 1) }

/// A workbook cell style as a Datum Plan style string (see the window's fmt.ts).
fn style_text(st: &calamine::Style) -> String {
    use calamine::{BorderStyle as B, FillPattern, FontStyle, FontWeight, HorizontalAlignment as H, UnderlineStyle, VerticalAlignment as V};
    let hex = |c: &calamine::Color| format!("#{:02X}{:02X}{:02X}", c.red, c.green, c.blue);
    let mut out: Vec<String> = vec![];
    if let Some(f) = &st.font {
        if f.weight == FontWeight::Bold { out.push("b".into()) }
        if f.style == FontStyle::Italic { out.push("i".into()) }
        if f.underline != UnderlineStyle::None { out.push("u".into()) }
        if f.strikethrough { out.push("s".into()) }
        if let Some(c) = f.color.as_ref().filter(|c| (c.red, c.green, c.blue) != (0, 0, 0)) { out.push(format!("c:{}", hex(c))) }
        if let Some(z) = f.size.filter(|z| (*z - 11.0).abs() > 0.1 && *z > 3.0 && *z < 120.0) { out.push(format!("z:{}", (z * 4.0 / 3.0).round() as u32)) }
        if let Some(n) = f.name.as_ref().filter(|n| !["Calibri", "Aptos Narrow", "Aptos", "Arial"].contains(&n.as_str())) { out.push(format!("n:{n}")) }
    }
    if let Some(fl) = &st.fill {
        if fl.pattern == FillPattern::Solid { if let Some(c) = &fl.foreground_color { out.push(format!("f:{}", hex(c))) } }
    }
    if let Some(a) = &st.alignment {
        match a.horizontal { H::Center | H::CenterContinuous => out.push("a:c".into()), H::Right => out.push("a:r".into()), _ => {} }
        match a.vertical { V::Top => out.push("v:t".into()), V::Bottom => {}, _ => out.push("v:m".into()) }
        if a.wrap_text { out.push("w".into()) }
        if let Some(i) = a.indent.filter(|i| *i > 0) { out.push(format!("x:{}", i.min(15))) }
    }
    if let Some(b) = &st.borders {
        for (k, side) in [("bt", &b.top), ("bb", &b.bottom), ("bl", &b.left), ("br", &b.right)] {
            let kind = match side.style { B::None => continue, B::Medium | B::MediumDashed | B::MediumDashDot | B::MediumDashDotDot => "med", B::Thick => "thick", B::Double => "dbl", B::Dashed | B::DashDot | B::DashDotDot => "dash", B::Dotted | B::Hair => "dot", _ => "thin" };
            out.push(format!("{k}:{kind} {}", side.color.as_ref().map_or("#000000".into(), hex)));
        }
    }
    // Middle is the window's default; a workbook's default (bottom) is left unsaid.
    out.retain(|x| x != "v:m");
    out.join(";")
}

/// The layout a workbook keeps outside the cells, read straight from its parts.
#[derive(Default)]
struct Layout { span: u64, hide_rows: Vec<u32>, hide_cols: Vec<u32>, widths: BTreeMap<u32, u16>, heights: BTreeMap<u32, u16>, color: String, hidden: bool, freeze: Option<(u32, u32)>, page: Option<SheetPage>, pictures: Vec<(String, u16, u16, Vec<u8>, String)> }
struct Parts { sheets: HashMap<String, Layout> }

fn attr(tag: &str, name: &str) -> Option<String> {
    let k = format!(" {name}=\"");
    let i = tag.find(&k)? + k.len();
    let j = tag[i..].find('"')? + i;
    Some(tag[i..j].replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'"))
}
/// Each `<name ...>` tag in an XML text (the opening tags only).
fn tags<'a>(xml: &'a str, name: &str) -> impl Iterator<Item = &'a str> + 'a {
    let (open, open2) = (format!("<{name} "), format!("<{name}>"));
    let mut at: Vec<usize> = xml.match_indices(&open).chain(xml.match_indices(&open2)).map(|(i, _)| i).collect();
    at.sort_unstable();
    at.into_iter().filter_map(move |i| xml[i..].find('>').map(|j| &xml[i..i + j + 1]))
}
fn resolve(base: &str, target: &str) -> String {
    if let Some(t) = target.strip_prefix('/') { return t.to_string() }
    let mut parts: Vec<&str> = base.split('/').collect();
    parts.pop();
    for seg in target.split('/') { if seg == ".." { parts.pop(); } else if seg != "." { parts.push(seg); } }
    parts.join("/")
}

fn xml_parts(path: &Path) -> Result<Parts, String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let mut read = |name: &str| -> Option<Vec<u8>> {
        let mut e = zip.by_name(name).ok()?;
        if e.size() > 200_000_000 { return None }
        let mut v = Vec::with_capacity(e.size() as usize);
        e.by_ref().take(200_000_000).read_to_end(&mut v).ok()?;
        Some(v)
    };
    let text = |b: Option<Vec<u8>>| b.map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let book = text(read("xl/workbook.xml"));
    let rels = text(read("xl/_rels/workbook.xml.rels"));
    let rel: HashMap<String, String> = tags(&rels, "Relationship").filter_map(|t| Some((attr(t, "Id")?, attr(t, "Target")?))).collect();
    // Print areas and title rows are defined names local to a sheet.
    let mut names: HashMap<(usize, String), String> = HashMap::new();
    for (i, _) in book.match_indices("<definedName ") {
        let Some(end) = book[i..].find("</definedName>") else { continue };
        let el = &book[i..i + end];
        let (Some(n), Some(id)) = (attr(el, "name"), attr(el, "localSheetId").and_then(|x| x.parse().ok())) else { continue };
        if let Some(gt) = el.find('>') { names.insert((id, n), el[gt + 1..].replace("&apos;", "'").replace("&amp;", "&")); }
    }
    let mut out = Parts { sheets: HashMap::new() };
    for (idx, t) in tags(&book, "sheet").enumerate() {
        let (Some(name), Some(id)) = (attr(t, "name"), attr(t, "r:id")) else { continue };
        let Some(target) = rel.get(&id) else { continue };
        let part = resolve("xl/workbook.xml", target);
        let xml = text(read(&part));
        let mut l = Layout { hidden: attr(t, "state").is_some_and(|s| s != "visible"), ..Default::default() };
        let (mut r0, mut r1, mut c0, mut c1) = (u32::MAX, 0, u32::MAX, 0);
        for c in tags(&xml, "c").filter_map(|c| gridcraft_core::CellRef::parse(&attr(c, "r")?)) { (r0, r1, c0, c1) = (r0.min(c.row), r1.max(c.row), c0.min(c.col), c1.max(c.col)); }
        if r0 <= r1 { l.span = (r1 - r0 + 1) as u64 * (c1 - c0 + 1) as u64; }
        for c in tags(&xml, "col") {
            let (Some(a), Some(b), Some(w)) = (attr(c, "min").and_then(|x| x.parse::<u32>().ok()), attr(c, "max").and_then(|x| x.parse::<u32>().ok()), attr(c, "width").and_then(|x| x.parse::<f64>().ok())) else { continue };
            if matches!(attr(c, "hidden").as_deref(), Some("1") | Some("true")) { for k in a.max(1)..=b.min(a + 1_000) { l.hide_cols.push(k - 1); } continue }
            if attr(c, "customWidth").as_deref() != Some("1") { continue }
            for k in a.max(1)..=b.min(a + 1_000) { l.widths.insert(k - 1, ((w * 7.0 + 5.0).round() as u16).clamp(16, 1200)); }
        }
        for r in tags(&xml, "row") {
            if let (Some(n), Some("1" | "true")) = (attr(r, "r").and_then(|x| x.parse::<u32>().ok()), attr(r, "hidden").as_deref()) { if n > 0 { l.hide_rows.push(n - 1); } }
            let (Some(n), Some(h)) = (attr(r, "r").and_then(|x| x.parse::<u32>().ok()), attr(r, "ht").and_then(|x| x.parse::<f64>().ok())) else { continue };
            if attr(r, "customHeight").as_deref() == Some("1") && n > 0 { l.heights.insert(n - 1, ((h * 4.0 / 3.0).round() as u16).clamp(6, 600)); }
        }
        if let Some(c) = tags(&xml, "tabColor").next().and_then(|t| attr(t, "rgb")) { if c.len() >= 6 { l.color = format!("#{}", &c[c.len() - 6..]); } }
        if let Some(p) = tags(&xml, "pane").next().filter(|p| attr(p, "state").is_some_and(|s| s.starts_with("frozen"))) {
            let n = |k: &str| attr(p, k).and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0) as u32;
            l.freeze = Some((n("ySplit"), n("xSplit")));
        }
        // Page setup.
        let mut pg = SheetPage::default();
        if let Some(s) = tags(&xml, "pageSetup").next() {
            pg.paper = match attr(s, "paperSize").as_deref() { Some("1") => "Letter", Some("3") => "Tabloid", Some("5") => "Legal", Some("8") => "A3", Some("9") => "A4", Some("11") => "A5", _ => "" }.into();
            pg.orient = attr(s, "orientation").unwrap_or_default();
            let fit = tags(&xml, "pageSetUpPr").next().and_then(|t| attr(t, "fitToPage")).is_some_and(|v| v == "1" || v == "true");
            if fit { pg.fit_w = attr(s, "fitToWidth").and_then(|x| x.parse().ok()).unwrap_or(1); pg.fit_h = attr(s, "fitToHeight").and_then(|x| x.parse().ok()).unwrap_or(1); }
            else if let Some(sc) = attr(s, "scale").and_then(|x| x.parse::<u16>().ok()).filter(|x| *x != 100) { pg.scale = sc; }
        }
        if let Some(m) = tags(&xml, "pageMargins").next() {
            let v = |k: &str| attr(m, k).and_then(|x| x.parse::<f32>().ok()).map(|i| (i * 25.4).round());
            if let (Some(t), Some(r), Some(b), Some(lf)) = (v("top"), v("right"), v("bottom"), v("left")) { pg.margins = Some([t, r, b, lf]); }
        }
        let opt = tags(&xml, "printOptions").next();
        let on = |k: &str| opt.and_then(|o| attr(o, k)).is_some_and(|v| v == "1" || v == "true");
        pg.no_grid = !on("gridLines");
        pg.headings = on("headings");
        pg.center = on("horizontalCentered");
        if let Some(i) = xml.find("<colBreaks") { if let Some(j) = xml[i..].find("</colBreaks>") { pg.col_breaks = tags(&xml[i..i + j], "brk").filter_map(|b| attr(b, "id")?.parse().ok()).collect(); } }
        if let Some(i) = xml.find("<rowBreaks") { if let Some(j) = xml[i..].find("</rowBreaks>") { pg.breaks = tags(&xml[i..i + j], "brk").filter_map(|b| attr(b, "id")?.parse().ok()).collect(); } }
        if let Some(a) = names.get(&(idx, "_xlnm.Print_Area".into())) { pg.area = a.rsplit('!').next().unwrap_or("").replace('$', "").split(',').next().unwrap_or("").to_string(); }
        if let Some(t) = names.get(&(idx, "_xlnm.Print_Titles".into())) { if let Some(rows) = t.split(',').map(|x| x.rsplit('!').next().unwrap_or("").replace('$', "")).find(|x| x.chars().all(|ch| ch.is_ascii_digit() || ch == ':') && !x.is_empty()) { pg.titles = rows; } }
        if pg != SheetPage::default() { l.page = Some(pg); }
        // Pictures: the sheet's drawing, each picture's top-left cell and size.
        let srel = text(read(&resolve(&part, &format!("_rels/{}.rels", part.rsplit('/').next().unwrap_or("")))));
        if let Some(d) = tags(&srel, "Relationship").find(|t| attr(t, "Type").is_some_and(|x| x.ends_with("/drawing"))).and_then(|t| attr(t, "Target")) {
            let dpart = resolve(&part, &d);
            let dxml = text(read(&dpart));
            let drel = text(read(&resolve(&dpart, &format!("_rels/{}.rels", dpart.rsplit('/').next().unwrap_or("")))));
            let media: HashMap<String, String> = tags(&drel, "Relationship").filter_map(|t| Some((attr(t, "Id")?, attr(t, "Target")?))).collect();
            for anchor in dxml.split("Anchor").skip(1).take(500) {
                let num = |tag: &str| anchor.find(&format!("<xdr:{tag}>")).and_then(|i| { let s = &anchor[i + tag.len() + 6..]; s[..s.find('<')?].trim().parse::<u32>().ok() });
                let (Some(col), Some(row)) = (num("col"), num("row")) else { continue };
                let Some(embed) = tags(anchor, "a:blip").next().and_then(|t| attr(t, "r:embed")) else { continue };
                let Some(target) = media.get(&embed) else { continue };
                let mpart = resolve(&dpart, target);
                let Some(bytes) = read(&mpart) else { continue };
                if bytes.len() > 4_000_000 { continue }
                let ext = tags(anchor, "a:ext").chain(tags(anchor, "xdr:ext")).find(|e| attr(e, "cx").is_some());
                let emu = |k: &str| ext.and_then(|e| attr(e, k)).and_then(|x| x.parse::<f64>().ok()).map(|v| (v / 9525.0).round().clamp(8.0, 4000.0) as u16);
                let mime = match mpart.rsplit('.').next().unwrap_or("").to_lowercase().as_str() { "jpg" | "jpeg" => "image/jpeg", "gif" => "image/gif", "svg" => "image/svg+xml", _ => "image/png" };
                l.pictures.push((a1(row, col), emu("cx").unwrap_or(120), emu("cy").unwrap_or(60), bytes, mime.into()));
            }
        }
        out.sheets.insert(name, l);
    }
    Ok(out)
}

fn base64(b: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(b.len() * 4 / 3 + 4);
    for ch in b.chunks(3) {
        let n = (ch[0] as u32) << 16 | (*ch.get(1).unwrap_or(&0) as u32) << 8 | *ch.get(2).unwrap_or(&0) as u32;
        for k in 0..4 { s.push(if k <= ch.len() { T[(n >> (18 - 6 * k) & 63) as usize] as char } else { '=' }); }
    }
    s
}

/// A binary workbook's sheets, measured from each sheet's dimension record before anything is built:
/// a sheet spanning more than 10 million cells is refused (it would not fit in memory).
fn xlsb_size(path: &Path) -> Result<(), String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(f).map_err(|e| format!("Could not read the workbook: {e}"))?;
    let names: Vec<String> = zip.file_names().filter_map(|n| n.ok().map(|n| n.to_string())).filter(|n| n.starts_with("xl/worksheets/") && n.ends_with(".bin")).collect();
    for n in names {
        let mut head = Vec::new();
        zip.by_name(&n).map_err(|e| e.to_string())?.take(65_536).read_to_end(&mut head).map_err(|e| e.to_string())?;
        if let Some((r0, r1, c0, c1)) = xlsb_dim(&head) {
            if (r1.saturating_sub(r0) as u64 + 1) * (c1.saturating_sub(c0) as u64 + 1) > 10_000_000 {
                return Err("A sheet in that workbook spans more than 10 million cells (something far to the right or at the bottom). Clear the far cells in the workbook and try again.".into());
            }
        }
    }
    Ok(())
}
/// The first rows and columns record (BrtWsDim) of a binary sheet: first and last row and column.
pub(crate) fn xlsb_dim(b: &[u8]) -> Option<(u32, u32, u32, u32)> {
    let mut i = 0;
    while i < b.len() {
        let mut t = (b[i] & 0x7F) as u32;
        if b[i] & 0x80 != 0 { i += 1; t |= (*b.get(i)? as u32 & 0x7F) << 7; }
        i += 1;
        let (mut size, mut shift) = (0u32, 0);
        for _ in 0..4 { let x = *b.get(i)?; i += 1; size |= ((x & 0x7F) as u32) << shift; shift += 7; if x & 0x80 == 0 { break } }
        if t == 0x94 {
            let u = |k: usize| b.get(i + k..i + k + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]));
            return Some((u(0)?, u(4)?, u(8)?, u(12)?));
        }
        i += size as usize;
    }
    None
}
