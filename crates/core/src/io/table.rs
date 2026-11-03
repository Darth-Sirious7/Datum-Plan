//! Task lists in spreadsheets: CSV and Excel in, with a column mapping step; CSV and Excel out.
use crate::{doc, fields::{Calc, Kind}, formula::F, model::Project};
use calamine::{open_workbook_auto, Data, Reader};
use chrono::NaiveDate;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// The field each column most likely holds ("" = skip).
    pub guess: Vec<String>,
}

/// Fields a column can map to: key, label, header words that suggest it.
pub const FIELDS: [(&str, &str, &[&str]); 15] = [
    ("id", "ID (for predecessors)", &["id", "unique id", "#", "activity", "ref"]),
    ("level", "Outline level", &["outline level", "level", "indent"]),
    ("wbs", "WBS / outline number", &["wbs", "outline number"]),
    ("code", "Activity ID", &["activity id", "task code", "code"]),
    ("name", "Task name", &["name", "task", "activity name", "description", "title"]),
    ("dur", "Duration", &["duration", "dur", "days"]),
    ("start", "Start", &["start", "begin"]),
    ("finish", "Finish", &["finish", "end", "due"]),
    ("preds", "Predecessors", &["predecessor", "depends", "after"]),
    ("res", "Resources", &["resource", "assigned", "owner", "who"]),
    ("pct", "% complete", &["%", "complete", "progress", "done"]),
    ("deadline", "Deadline", &["deadline"]),
    ("astart", "Actual start", &["actual start"]),
    ("afinish", "Actual finish", &["actual finish"]),
    ("notes", "Notes", &["note", "comment", "remark"]),
];

/// The likely field of a header and how sure we are (longer matches win).
fn guess(h: &str) -> (String, usize) {
    let h = h.trim().to_lowercase();
    if let Some(f) = FIELDS.iter().find(|(k, l, _)| h == *k || h == l.to_lowercase()) {
        return (f.0.to_string(), 100);
    }
    FIELDS.iter().filter_map(|f| f.2.iter().filter(|w| h.contains(*w)).map(|w| w.len()).max().map(|n| (f.0.to_string(), n))).max_by_key(|x| x.1).unwrap_or_default()
}

/// One field per column: when two columns look alike, the better match keeps the field.
fn guesses(headers: &[String]) -> Vec<String> {
    let g: Vec<(String, usize)> = headers.iter().map(|h| guess(h)).collect();
    g.iter().enumerate().map(|(i, (f, n))| {
        let better = g.iter().enumerate().any(|(j, (f2, n2))| j != i && f2 == f && (n2 > n || (n2 == n && j < i)));
        if f.is_empty() || better { String::new() } else { f.clone() }
    }).collect()
}

fn csv_rows(text: &str) -> Vec<Vec<String>> {
    let first = text.lines().next().unwrap_or("");
    let sep = [',', ';', '\t'].into_iter().max_by_key(|c| first.matches(*c).count()).unwrap_or(',');
    let (mut rows, mut row, mut cell, mut q) = (vec![], vec![], String::new(), false);
    let mut it = text.trim_start_matches('\u{feff}').chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '"' if q && it.peek() == Some(&'"') => { cell.push('"'); it.next(); }
            '"' => q = !q,
            c if c == sep && !q => row.push(std::mem::take(&mut cell)),
            '\n' if !q => { row.push(std::mem::take(&mut cell)); rows.push(std::mem::take(&mut row)); }
            '\r' if !q => {}
            c => cell.push(c),
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows
}

fn cell(d: &Data) -> String {
    match d {
        Data::Empty => String::new(),
        Data::String(s) | Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Float(f) if f.fract() == 0.0 => format!("{}", *f as i64),
        Data::Float(f) => f.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(x) => {
            let v = x.as_f64();
            NaiveDate::from_ymd_opt(1899, 12, 30).and_then(|d| d.checked_add_days(chrono::Days::new(v.max(0.0) as u64))).map(|d| d.to_string()).unwrap_or_default()
        }
        Data::Error(_) => String::new(),
    }
}

/// The most cells a sheet may span: calamine builds the whole box around the used cells (and an
/// .xls reserves its stated size), so two far-apart cells in a tiny file would ask for hundreds
/// of gigabytes, an abort rather than an error.
const MAX_CELLS: u64 = 10_000_000;

/// Measures every sheet of an .xls or .ods file without building it, and refuses absurd sizes.
fn sheet_size(path: &Path, ext: &str) -> Result<(), String> {
    use std::io::Read;
    let bad = |e: &dyn std::fmt::Display| format!("Could not read the workbook: {e}");
    let too_big = || Err::<(), String>("This sheet spans more than 1,048,576 rows, 1,024 columns or 10 million cells. Copy the plan's rows to a new sheet, save it as .xlsx or .csv and import that.".into());
    // Each sheet's box: (first row, last row, first column, last column).
    let mut boxes: Vec<(u64, u64, u64, u64)> = vec![];
    let grow = |b: &mut Option<(u64, u64, u64, u64)>, r0: u64, r1: u64, c0: u64, c1: u64| {
        let o = b.get_or_insert((r0, r1, c0, c1));
        *o = (o.0.min(r0), o.1.max(r1), o.2.min(c0), o.3.max(c1));
    };
    let ok = |b: (u64, u64, u64, u64)| b.1 < 1_048_576 && b.3 < 1024 && (b.1 - b.0 + 1).saturating_mul(b.3 - b.2 + 1) <= MAX_CELLS;
    if ext == "xls" {
        let mut cf = cfb::open(path).map_err(|e| bad(&e))?;
        let mut st = cf.open_stream("/Workbook").or_else(|_| cf.open_stream("/Book")).map_err(|e| bad(&e))?;
        let mut s = vec![];
        st.by_ref().take(crate::doc::MAX_FILE).read_to_end(&mut s).map_err(|e| bad(&e))?;
        let u16_at = |d: &[u8], i: usize| d.get(i..i + 2).map_or(0, |x| u16::from_le_bytes([x[0], x[1]]) as u64);
        let u32_at = |d: &[u8], i: usize| d.get(i..i + 4).map_or(0, |x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]) as u64);
        let (mut i, mut cur) = (0, None);
        while i + 4 <= s.len() {
            let (ty, len) = (u16_at(&s, i), u16_at(&s, i + 2) as usize);
            let d = &s[i + 4..(i + 4 + len).min(s.len())];
            i += 4 + len;
            match ty {
                0x0809 => boxes.extend(cur.take()), // BOF: a new sheet
                // Number, Label, RString, BoolErr, Rk, LabelSst, Formula: row and column first.
                0x0203 | 0x0204 | 0x00D6 | 0x0205 | 0x027E | 0x00FD | 0x0006 => { let (r, c) = (u16_at(d, 0), u16_at(d, 2)); grow(&mut cur, r, r, c, c) }
                0x00BD if d.len() >= 6 => { let r = u16_at(d, 0); grow(&mut cur, r, r, u16_at(d, 2), u16_at(d, d.len() - 2)) }
                // Dimensions: calamine reserves room for all of it.
                0x0200 => {
                    let (rf, rl, cf, cl) = match d.len() { 10 => (u16_at(d, 0), u16_at(d, 2), u16_at(d, 4), u16_at(d, 6)), 14 | 16 => (u32_at(d, 0), u32_at(d, 4), u16_at(d, 8), u16_at(d, 10)), _ => continue };
                    let cf = if cf > 0xFF || cl < cf { 0 } else { cf };
                    if rl >= 1 && cl >= 1 { if rl - 1 < rf || cl - 1 < cf { return too_big() } boxes.push((rf, rl - 1, cf, cl - 1)) }
                }
                _ => {}
            }
        }
        boxes.extend(cur);
    } else {
        // content.xml, every table: rows and cells repeat by count, and only cells with content count.
        use quick_xml::events::Event;
        let file = std::fs::File::open(path).map_err(|e| bad(&e))?;
        let mut z = zip::ZipArchive::new(file).map_err(|e| bad(&e))?;
        let content = z.by_name("content.xml").map_err(|e| bad(&e))?;
        let mut rd = quick_xml::Reader::from_reader(std::io::BufReader::new(content));
        // As calamine reads it: <row/> is a row too.
        (rd.config_mut().expand_empty_elements, rd.config_mut().check_end_names) = (true, false);
        let (mut buf, mut cur, mut row, mut col, mut row_rep, mut end, mut raw) = (vec![], None, 0u64, 0u64, 1u64, 0u64, 0u64);
        let num = |e: &quick_xml::events::BytesStart, k: &str| e.attributes().flatten().find(|a| a.key.as_ref() == k).and_then(|a| a.value.trim().parse::<u64>().ok()).unwrap_or(1).max(1);
        loop {
            match rd.read_event_into(&mut buf).map_err(|e| bad(&e))? {
                Event::Start(e) => match e.name().as_ref() {
                    "table:table" => { boxes.extend(cur.take()); row = 0 }
                    "table:table-row" => { (col, row_rep, end) = (0, num(&e, "table:number-rows-repeated"), 0) }
                    "table:table-cell" | "table:covered-table-cell" => {
                        let n = num(&e, "table:number-columns-repeated");
                        if e.attributes().flatten().any(|a| a.key.as_ref().starts_with("office:") || a.key.as_ref() == "table:formula") {
                            grow(&mut cur, row, row.saturating_add(row_rep - 1), col, col.saturating_add(n - 1));
                            end = col.saturating_add(n);
                        }
                        col = col.saturating_add(n);
                    }
                    _ => {}
                },
                Event::End(e) if e.name().as_ref() == "table:table-row" => {
                    // Calamine first keeps each row up to its last cell with content.
                    raw = raw.saturating_add(end.min(16_384));
                    if raw > MAX_CELLS { return too_big() }
                    row = row.saturating_add(row_rep);
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }
        boxes.extend(cur);
    }
    if boxes.into_iter().all(ok) { Ok(()) } else { too_big() }
}

pub fn read(path: &Path) -> Result<Table, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let mut rows = if ext == "csv" || ext == "txt" {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        csv_rows(&String::from_utf8_lossy(&bytes))
    } else if ext == "xlsx" || ext == "xlsm" {
        // Cell by cell: a range is the box around every cell, so two far-apart cells in a tiny
        // file would ask for hundreds of gigabytes (an abort, not an error).
        let mut wb: calamine::Xlsx<_> = calamine::open_workbook(path).map_err(|e| format!("Could not read the workbook: {e}"))?;
        let name = wb.sheet_names().first().cloned().ok_or("The workbook has no sheets.")?;
        let mut rd = wb.worksheet_cells_reader(&name).map_err(|e| e.to_string())?;
        let mut got = vec![];
        while let Some(c) = rd.next_cell().map_err(|e| e.to_string())? {
            let ((y, x), v) = (c.get_position(), cell(&Data::from(c.get_value().clone())));
            if y < 1_048_576 && x < 1024 && !v.is_empty() { got.push((y, x, v)) }
        }
        let (y0, x0) = got.iter().fold((u32::MAX, u32::MAX), |(a, b), c| (a.min(c.0), b.min(c.1)));
        let mut rows: Vec<Vec<String>> = vec![];
        for (y, x, v) in got {
            let (y, x) = ((y - y0) as usize, (x - x0) as usize);
            if rows.len() <= y { rows.resize(y + 1, vec![]) }
            if rows[y].len() <= x { rows[y].resize(x + 1, String::new()) }
            rows[y][x] = v;
        }
        rows
    } else {
        sheet_size(path, &ext)?;
        let mut wb = open_workbook_auto(path).map_err(|e| format!("Could not read the workbook: {e}"))?;
        let range = wb.worksheet_range_at(0).ok_or("The workbook has no sheets.")?.map_err(|e| e.to_string())?;
        range.rows().map(|r| r.iter().map(cell).collect()).collect()
    };
    rows.retain(|r: &Vec<String>| r.iter().any(|c| !c.trim().is_empty()));
    if rows.is_empty() {
        return Err("The file has no rows.".into());
    }
    let headers: Vec<String> = rows.remove(0).into_iter().map(|h| h.trim().to_string()).collect();
    let guess = guesses(&headers);
    Ok(Table { headers, rows, guess })
}

/// Builds a plan from table rows, `map[c]` naming the field of column c. Returns the plan and
/// how many cells could not be used.
pub fn to_project(t: &Table, map: &[String], name: &str, today: NaiveDate, fmt: &str) -> Result<(Project, usize), String> {
    let col = |f: &str| map.iter().position(|m| m == f);
    let name_col = col("name").ok_or("Choose which column holds the task names.")?;
    let rows: Vec<&Vec<String>> = t.rows.iter().filter(|r| r.get(name_col).is_some_and(|c| !c.trim().is_empty())).collect();
    let get = |r: &Vec<String>, f: &str| col(f).and_then(|c| r.get(c)).map(|s| s.trim().to_string()).unwrap_or_default();
    let start = rows.iter().filter_map(|r| crate::parse::date_in(&get(r, "start"), fmt).ok()).min().unwrap_or(today);
    let mut p = Project::new(name, start);
    for r in &rows {
        let level = if col("level").is_some() {
            get(r, "level").parse::<i32>().map(|l| (l - 1).max(0)).unwrap_or(0)
        } else if col("wbs").is_some() {
            get(r, "wbs").matches('.').count() as i32
        } else { 0 };
        let task = p.new_task("", level.min(30) as u8, p.day_min);
        p.tasks.push(task);
    }
    p.normalize();
    // Predecessors refer to source IDs (or row numbers when there is no ID column).
    let ids: Vec<String> = rows.iter().enumerate().map(|(k, r)| if col("id").is_some() { get(r, "id") } else { (k + 1).to_string() }).collect();
    let mut bad = 0;
    for (i, r) in rows.iter().enumerate() {
        let has_preds = !get(r, "preds").is_empty();
        for f in ["name", "code", "dur", "start", "finish", "res", "pct", "deadline", "astart", "afinish", "notes"] {
            let v = get(r, f);
            if v.is_empty() || (f == "start" && has_preds) || (f == "finish" && col("dur").is_some()) {
                continue;
            }
            if doc::set(&mut p, i, f, &v, fmt).is_err() && !p.is_summary(i) {
                bad += 1;
            }
        }
    }
    for (i, r) in rows.iter().enumerate() {
        let v = get(r, "preds");
        if v.is_empty() || p.is_summary(i) {
            continue;
        }
        // IDs can be codes like A10, so match the longest ID the token starts with: "A10SS+2d".
        let mapped: Vec<String> = v.split([',', ';']).filter_map(|tok| {
            let up = tok.trim().to_uppercase();
            let (k, id) = ids.iter().enumerate().filter(|(_, x)| !x.is_empty() && up.starts_with(&x.to_uppercase())).max_by_key(|(_, x)| x.len())?;
            Some(format!("{}{}", k + 1, &up[id.len()..]))
        }).collect();
        if doc::set(&mut p, i, "preds", &mapped.join(","), fmt).is_err() {
            bad += 1;
        }
    }
    p.normalize();
    if crate::sched::schedule(&p).is_err() {
        p.tasks.iter_mut().for_each(|t| t.preds.clear());
        bad += 1;
    }
    Ok((p, bad))
}

/// Columns written when the caller does not choose: the current table's columns are used normally.
pub const OUT: [&str; 13] = ["id", "wbs", "level", "code", "name", "dur", "start", "finish", "preds", "res", "pct", "tf", "cost"];

fn cells(c: &Calc, cols: &[String]) -> (Vec<String>, Vec<(usize, Vec<String>)>) {
    let cols: Vec<String> = cols.iter().filter_map(|k| c.key_of(k)).collect();
    let head = cols.iter().map(|k| c.name(k)).collect();
    (head, (0..c.p.tasks.len()).map(|i| (i, cols.iter().map(|k| c.text(i, k)).collect())).collect())
}

pub fn write_csv(c: &Calc, cols: &[String], path: &Path) -> Result<(), String> {
    // Text that a spreadsheet would run as a formula (starting with = + - @, unless it is a plain
    // number) gets a leading apostrophe, which spreadsheets hide, so opening the CSV runs nothing.
    let safe = |s: &str| {
        let risky = s.starts_with(['=', '+', '-', '@', '\t', '\r']) && s.replace(',', "").trim().parse::<f64>().is_err();
        if risky { format!("'{s}") } else { s.to_string() }
    };
    let q = |s: &str| { let s = safe(s); if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s } };
    let (head, rows) = cells(c, cols);
    let mut out = String::from("\u{feff}");
    out.push_str(&head.iter().map(|h| q(h)).collect::<Vec<_>>().join(","));
    out.push('\n');
    for (_, r) in rows {
        out.push_str(&r.iter().map(|c| q(c)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    crate::doc::write_atomic(path, out.as_bytes())
}

/// Excel workbook: the table with real numbers and dates, formula columns as live workbook formulas,
/// the plan's sheet tabs, and a Plan sheet those tabs can read.
/// Applies a cell style (`b;i;u;s;c:#hex;f:#hex;z:12;a:c;n:Font`, see the UI's fmt.ts) to an Excel format.
fn styled(mut f: rust_xlsxwriter::Format, st: &str) -> rust_xlsxwriter::Format {
    use rust_xlsxwriter::{FormatAlign, FormatUnderline};
    let hex = |v: &str| u32::from_str_radix(v.trim_start_matches('#'), 16).ok();
    for t in st.split(';').filter(|t| !t.is_empty()) {
        let (k, v) = t.split_once(':').unwrap_or((t, ""));
        f = match k {
            "b" => f.set_bold(),
            "i" => f.set_italic(),
            "u" => f.set_underline(FormatUnderline::Single),
            "s" => f.set_font_strikethrough(),
            "c" => match hex(v) { Some(x) => f.set_font_color(x), None => f },
            "f" => match hex(v) { Some(x) => f.set_background_color(x), None => f },
            "z" => match v.parse::<f64>() { Ok(z) => f.set_font_size(z * 0.75), Err(_) => f },
            "a" => f.set_align(match v { "c" => FormatAlign::Center, "r" => FormatAlign::Right, _ => FormatAlign::Left }),
            "n" if !v.is_empty() => f.set_font_name(v),
            "w" => f.set_text_wrap(),
            "v" => f.set_align(match v { "t" => FormatAlign::Top, "b" => FormatAlign::Bottom, _ => FormatAlign::VerticalCenter }),
            "x" => match v.parse::<u8>() { Ok(n) => f.set_indent(n.min(15)), Err(_) => f },
            "bt" | "bb" | "bl" | "br" => {
                use rust_xlsxwriter::FormatBorder as Bd;
                let (kind, col) = v.split_once(' ').unwrap_or((v, ""));
                let b = match kind { "med" => Bd::Medium, "thick" => Bd::Thick, "dbl" => Bd::Double, "dash" => Bd::Dashed, "dot" => Bd::Dotted, "none" => Bd::None, _ => Bd::Thin };
                let c = hex(col).unwrap_or(0);
                match k { "bt" => f.set_border_top(b).set_border_top_color(c), "bb" => f.set_border_bottom(b).set_border_bottom_color(c), "bl" => f.set_border_left(b).set_border_left_color(c), _ => f.set_border_right(b).set_border_right_color(c) }
            }
            _ => f,
        };
    }
    f
}

/// A picture placed in its cell: the size that fits inside the cell (or the merged range it starts), keeping its shape.
fn fit_in_cell(s: &crate::model::SheetData, cell: &str, iw: f64, ih: f64) -> (f64, f64) {
    let Some((r0, c0, ..)) = crate::model::sheet_range(cell) else { return (iw, ih) };
    let (r1, c1) = s.merges.iter().filter_map(|m| crate::model::sheet_range(m)).find(|m| m.0 == r0 && m.1 == c0).map_or((r0, c0), |m| (m.2, m.3));
    let bw: f64 = (c0..=c1).map(|c| *s.widths.get(&c).unwrap_or(&96) as f64).sum::<f64>() - 4.0;
    let bh: f64 = (r0..=r1).map(|r| *s.heights.get(&r).unwrap_or(&22) as f64).sum::<f64>() - 4.0;
    let k = (bw / iw).min(bh / ih).max(0.01);
    (iw * k, ih * k)
}

/// The style of one task cell: the row's style, then the column's.
fn task_style(t: &crate::model::Task, key: &str) -> String {
    let get = |k: &str| t.style.get(k).and_then(|v| v.as_str()).unwrap_or("");
    [get("*"), get(key)].iter().filter(|x| !x.is_empty()).copied().collect::<Vec<_>>().join(";")
}

pub fn write_xlsx(c: &Calc, cols: &[String], path: &Path) -> Result<(), String> {
    use rust_xlsxwriter::{Format, Workbook, Worksheet};
    let e = |x: rust_xlsxwriter::XlsxError| x.to_string();
    let mut wb = Workbook::new();
    let head_fmt = Format::new().set_bold().set_background_color(0xE6F0FC);
    let date_fmt = Format::new().set_num_format("dd-mmm-yy");
    let days_fmt = Format::new().set_num_format("0.0# \"d\"");
    let money_fmt = Format::new().set_num_format(format!("\"{}\"#,##0.{}", c.p.currency.symbol, "0".repeat(c.p.currency.decimals as usize)).replace(".\"", "\""));
    // One typed cell: dates as workbook dates, durations in days, money as numbers.
    let put = |ws: &mut Worksheet, r: u32, k: u16, i: usize, key: &str, bold: bool, st: &str| -> Result<(), String> {
        let kind = c.kind(key);
        let v = c.val(i, key);
        let fx = |f: &Format| styled(if bold { f.clone().set_bold() } else { f.clone() }, st);
        match (&v, kind) {
            (F::Null, _) => Ok(()),
            (F::Date(m), _) => ws.write_number_with_format(r, k, *m as f64 / 1440.0 + 25569.0 - if kind == Kind::Fin && m.rem_euclid(1440) == 0 { 1.0 } else { 0.0 }, &fx(&date_fmt)).map(|_| ()).map_err(e),
            (F::Num(n), Kind::Dur) => ws.write_number_with_format(r, k, n / c.p.day_min as f64, &fx(&days_fmt)).map(|_| ()).map_err(e),
            (F::Num(n), Kind::Cost) => ws.write_number_with_format(r, k, *n, &fx(&money_fmt)).map(|_| ()).map_err(e),
            (F::Num(n), _) => ws.write_number_with_format(r, k, *n, &fx(&Format::new())).map(|_| ()).map_err(e),
            (F::Bool(b), _) => ws.write_boolean(r, k, *b).map(|_| ()).map_err(e),
            _ => ws.write_string_with_format(r, k, c.text(i, key), &fx(&Format::new())).map(|_| ()).map_err(e),
        }
    };
    let keys: Vec<String> = cols.iter().filter_map(|k| c.key_of(k)).collect();
    let ws = wb.add_worksheet();
    ws.set_name("Tasks").map_err(e)?;
    for (k, key) in keys.iter().enumerate() {
        ws.write_string_with_format(0, k as u16, c.name(key), &head_fmt).map_err(e)?;
        ws.set_column_width(k as u16, if key == "name" { 44.0 } else if key == "res" { 28.0 } else { 13.0 }).map_err(e)?;
    }
    for i in 0..c.p.tasks.len() {
        let r = i as u32 + 1;
        for (k, key) in keys.iter().enumerate() {
            let formula = key.strip_prefix("c:").and_then(|f| c.p.fields.iter().find(|d| d.key == f)).map(|d| d.formula.clone()).filter(|f| crate::sheet::is_excel(f));
            let live = formula.and_then(|f| crate::sheet::translate(&f, |n| c.key_of(n).and_then(|kk| keys.iter().position(|x| *x == kk)).map(|j| format!("{}{}", crate::sheet::col_name(j as u32), r + 1))).ok());
            let st = task_style(&c.p.tasks[i], key);
            match live {
                Some(f) => { ws.write_formula_with_format(r, k as u16, format!("={f}").as_str(), &styled(Format::new(), &st)).map_err(e)?; }
                None if key == "name" => { ws.write_string_with_format(r, k as u16, c.text(i, key), &styled({ let f = Format::new().set_indent(c.p.tasks[i].level); if c.s.summary[i] { f.set_bold() } else { f } }, &st)).map_err(e)?; }
                None => put(ws, r, k as u16, i, key, c.s.summary[i], &st)?,
            }
        }
    }
    ws.set_freeze_panes(1, 0).map_err(e)?;
    for s in &c.p.sheets {
        let ws = wb.add_worksheet();
        ws.set_name(s.name.chars().filter(|ch| !"[]:*?/\\".contains(*ch)).take(31).collect::<String>()).map_err(e)?;
        // Merged ranges first; the top-left cell's value is written over it below.
        for m in &s.merges {
            let Some((r0, c0, r1, c1)) = crate::model::sheet_range(m) else { continue };
            let a1 = format!("{}{}", crate::sheet::col_name(c0), r0 + 1);
            ws.merge_range(r0, c0 as u16, r1, c1 as u16, "", &styled(Format::new(), s.style.get(&a1).map(String::as_str).unwrap_or(""))).map_err(e)?;
        }
        for (r, h) in &s.heights { ws.set_row_height_pixels(*r, *h as u32).map_err(e)?; }
        for (a1, input) in &s.cells {
            let Some((r, k)) = a1_index(a1) else { continue };
            let f = styled(Format::new(), s.style.get(a1).map(String::as_str).unwrap_or(""));
            if input.starts_with('=') { ws.write_formula_with_format(r, k, input.as_str(), &f).map_err(e)?; }
            else if let Ok(n) = input.replace(',', "").parse::<f64>() { ws.write_number_with_format(r, k, n, &f).map_err(e)?; }
            else { ws.write_string_with_format(r, k, input, &f).map_err(e)?; }
        }
        for (a1, st) in s.style.iter().filter(|(a1, _)| !s.cells.contains_key(*a1)) {
            if let Some((r, k)) = a1_index(a1) { ws.write_blank(r, k, &styled(Format::new(), st)).map_err(e)?; }
        }
        for (k, w) in &s.widths { ws.set_column_width_pixels(*k as u16, *w as u32).map_err(e)?; }
        // The sheet's page setup.
        if let Some(pg) = &s.page {
            let paper = match pg.paper.as_str() { "Letter" => 1, "Legal" => 5, "A3" => 8, "A5" => 11, "Tabloid" => 3, "A4" => 9, _ => 0 };
            if paper > 0 { ws.set_paper_size(paper); }
            if pg.orient == "landscape" { ws.set_landscape(); }
            if let Some([t, r, b, l]) = pg.margins { let i = |mm: f32| (mm / 25.4) as f64; ws.set_margins(i(l), i(r), i(t), i(b), 0.3, 0.3); }
            if pg.fit_w > 0 || pg.fit_h > 0 { ws.set_print_fit_to_pages(pg.fit_w as u16, pg.fit_h as u16); } else if pg.scale > 0 { ws.set_print_scale(pg.scale); }
            if let Some((r0, c0, r1, c1)) = crate::model::sheet_range(&pg.area) { ws.set_print_area(r0, c0 as u16, r1, c1 as u16).map_err(e)?; }
            if let Some((a, b)) = pg.titles.split_once(':').or(Some((pg.titles.as_str(), pg.titles.as_str()))).and_then(|(a, b)| Some((a.trim().parse::<u32>().ok()?, b.trim().parse::<u32>().ok()?))) { if a > 0 && b >= a { ws.set_repeat_rows(a - 1, b - 1).map_err(e)?; } }
            ws.set_print_gridlines(!pg.no_grid).set_print_headings(pg.headings).set_print_center_horizontally(pg.center);
            if !pg.breaks.is_empty() { ws.set_page_breaks(&pg.breaks).map_err(e)?; }
        }
        // Pictures placed on the sheet, at their cell and size (SVG pictures cannot go into a workbook).
        for im in &s.images {
            let (Some((r, k)), Some(x)) = (a1_index(&im.cell), c.p.pictures.iter().find(|x| x.id == im.pic)) else { continue };
            let Some(bytes) = x.data.split_once(";base64,").filter(|(m, _)| !m.contains("svg")).and_then(|(_, b)| unbase64(b)) else { continue };
            let Ok(img) = rust_xlsxwriter::Image::new_from_buffer(&bytes) else { continue };
            let (iw, ih) = (img.width().max(1.0), img.height().max(1.0));
            let (w, h) = if im.fit { fit_in_cell(s, &im.cell, iw, ih) } else { (im.w as f64, im.h as f64) };
            let img = img.set_scale_width(w / iw).set_scale_height(h / ih).set_alt_text(&x.name);
            ws.insert_image(r, k, &img).map_err(e)?;
        }
    }
    if !c.p.sheets.is_empty() {
        // The sheets' formulas read the plan as tables (Tasks[Finish]) and names (=ProjectFinish).
        let table = |cols: &[String]| rust_xlsxwriter::Table::new().set_columns(&cols.iter().map(|n| rust_xlsxwriter::TableColumn::new().set_header(n)).collect::<Vec<_>>());
        let ws = wb.add_worksheet();
        ws.set_name("Plan").map_err(e)?;
        let cols = crate::sheet::task_columns(c);
        for (k, (key, _)) in cols.iter().enumerate() {
            for i in 0..c.p.tasks.len() {
                put(ws, i as u32 + 1, k as u16, i, key, false, "")?;
            }
        }
        let names: Vec<String> = cols.iter().map(|x| x.1.clone()).collect();
        ws.add_table(0, 0, c.p.tasks.len().max(1) as u32, names.len() as u16 - 1, &table(&names).set_name("Tasks")).map_err(e)?;
        let ws = wb.add_worksheet();
        ws.set_name("Resources").map_err(e)?;
        let book = crate::sheet::book(c);
        if let Some(rs) = book.sheet(1) {
            for r in 1..=c.p.resources.len() as u32 {
                for k in 0..crate::sheet::RES_COLS.len() as u32 {
                    match rs.value(gridcraft_core::CellRef::new(r, k)) {
                        gridcraft_core::Value::Number(n) => { ws.write_number(r, k as u16, n).map_err(e)?; }
                        v => { ws.write_string(r, k as u16, v.display()).map_err(e)?; }
                    }
                }
            }
        }
        let names: Vec<String> = crate::sheet::RES_COLS.iter().map(|x| x.1.to_string()).collect();
        ws.add_table(0, 0, c.p.resources.len().max(1) as u32, names.len() as u16 - 1, &table(&names).set_name("Resources")).map_err(e)?;
        for (name, v, _) in crate::sheet::project_names(c) {
            let f = match v { gridcraft_core::Value::Number(n) => format!("={n}"), v => format!("=\"{}\"", v.display().replace('"', "\"\"")) };
            wb.define_name(name, &f).map_err(e)?;
        }
    }
    wb.save(path).map_err(|x| format!("Could not save {}: {x}", path.display()))
}

/// "B12" → (row 11, column 1), 0-based.
fn a1_index(a1: &str) -> Option<(u32, u16)> {
    let split = a1.find(|ch: char| ch.is_ascii_digit())?;
    let col = a1[..split].bytes().try_fold(0u32, |acc, b| b.is_ascii_uppercase().then(|| acc * 26 + (b - b'A' + 1) as u32))?;
    Some((a1[split..].parse::<u32>().ok()?.checked_sub(1)?, col.checked_sub(1)? as u16))
}

/// Standard base64 (a picture's data URL) to bytes; None when it is not valid.
fn unbase64(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| match c { b'A'..=b'Z' => Some(c - b'A'), b'a'..=b'z' => Some(c - b'a' + 26), b'0'..=b'9' => Some(c - b'0' + 52), b'+' => Some(62), b'/' => Some(63), _ => None };
    let v: Vec<u8> = s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=').map(val).collect::<Option<_>>()?;
    Some(v.chunks(4).flat_map(|c| { let n = c.iter().enumerate().fold(0u32, |n, (k, &x)| n | (x as u32) << (18 - 6 * k)); (0..c.len().saturating_sub(1)).map(move |k| (n >> (16 - 8 * k)) as u8) }).collect())
}
