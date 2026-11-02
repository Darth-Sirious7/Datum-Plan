//! Spreadsheet formulas through the Gridcraft engine: formula columns on tasks (`=[% Complete]-[Baseline
//! % Complete]`) and the plan's spreadsheet tabs, which can also read a "Plan" sheet of all tasks.
use crate::cal::DAY;
use crate::fields::{money, Calc, Kind};
use crate::formula::F;
use crate::model::SheetData;
use gridcraft_core::{CellRef, Value};
use gridcraft_model::{Cell, Formula, Sheet, Workbook};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

/// 1970-01-01 as an Excel date serial.
const EPOCH: f64 = 25569.0;

/// Our value as a spreadsheet value: dates become serials, durations become days.
pub fn to_value(v: &F, kind: Kind, day_min: i64) -> Value {
    match v {
        F::Null => Value::Empty,
        F::Num(n) if kind == Kind::Dur => Value::number(n / day_min as f64),
        F::Num(n) => Value::number(*n),
        F::Str(s) => Value::text(s.as_str()),
        F::Bool(b) => Value::Bool(*b),
        F::Date(m) => Value::number(*m as f64 / DAY as f64 + EPOCH),
    }
}

/// A spreadsheet result in the field's kind.
pub fn from_value(v: &Value, kind: Kind, day_min: i64) -> F {
    let num = || match v { Value::Number(n) => Some(*n), Value::Bool(b) => Some(*b as u8 as f64), _ => None };
    match (v, kind) {
        (Value::Empty, _) => F::Null,
        (Value::Error(_), _) => F::Str(v.display()),
        (_, Kind::Date | Kind::Fin) => num().map_or_else(|| F::Str(v.display()), |n| F::Date(((n - EPOCH) * DAY as f64).round() as i64)),
        (_, Kind::Dur) => num().map_or_else(|| F::Str(v.display()), |n| F::Num((n * day_min as f64).round())),
        (_, Kind::Flag) => F::Bool(v.to_bool().unwrap_or(false)),
        (_, Kind::Text) => F::Str(v.display()),
        _ => num().map_or_else(|| F::Str(v.display()), F::Num),
    }
}

/// Column letters for a 0-based index (0 → A, 26 → AA).
pub fn col_name(mut c: u32) -> String {
    let mut s = String::new();
    loop {
        s.insert(0, (b'A' + (c % 26) as u8) as char);
        if c < 26 {
            return s;
        }
        c = c / 26 - 1;
    }
}

/// Replaces `[Field]` references (outside string literals) using `col`, which returns the cell
/// reference to put in their place. Unknown fields are an error. A table reference such as
/// `Tasks[Cost]` goes to `col` whole; when it has no answer it stays as written.
pub fn translate(src: &str, mut col: impl FnMut(&str) -> Option<String>) -> Result<String, String> {
    let body = src.trim().strip_prefix('=').unwrap_or(src.trim());
    let (mut out, mut quoted, mut chars) = (String::new(), false, body.chars().peekable());
    while let Some(ch) = chars.next() {
        let table = out.len() - out.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_' || c == '.').len();
        match ch {
            '"' => { quoted = !quoted; out.push(ch) }
            '[' if !quoted && table > 0 => {
                // Up to the matching "]", so Tasks[[#This Row],[Cost]] stays whole.
                let (mut r, mut depth) = (String::from("["), 1);
                for c in chars.by_ref() {
                    r.push(c);
                    depth += match c { '[' => 1, ']' => -1, _ => 0 };
                    if depth == 0 { break }
                }
                let whole = format!("{}{r}", &out[out.len() - table..]);
                if let Some(x) = col(&whole) { out.truncate(out.len() - table); out.push_str(&x) } else { out.push_str(&r) }
            }
            '[' if !quoted => {
                let name: String = chars.by_ref().take_while(|&c| c != ']').collect();
                out.push_str(&col(name.trim()).ok_or_else(|| format!("There is no field called [{}].", name.trim()))?);
            }
            _ => out.push(ch),
        }
    }
    Ok(out)
}

/// Checks a spreadsheet-style formula: its fields exist and it parses.
pub fn check(src: &str, known: impl Fn(&str) -> bool) -> Result<(), String> {
    let t = translate(src, |n| known(n).then(|| "A1".into()))?;
    match Formula::new(&t).expr {
        Some(_) => Ok(()),
        None => Err("The formula is not valid. Check brackets, quotes and commas.".into()),
    }
}

pub fn is_excel(formula: &str) -> bool {
    formula.trim_start().starts_with('=')
}

/// Evaluates every spreadsheet formula column for every task. Formula columns may read each other.
pub fn task_formulas(c: &Calc, defs: &[(String, String)]) -> HashMap<String, Vec<F>> {
    let p = c.p;
    let n = p.tasks.len();
    let mut wb = Workbook::new();
    // Formula columns first (A, B, …), then the plain fields they read.
    let mut cols: HashMap<String, u32> = defs.iter().enumerate().map(|(k, (key, _))| (format!("c:{key}"), k as u32)).collect();
    let mut texts = vec![];
    let (tcols, rows, mut res) = (task_columns(c), n.max(1), false);
    for (_, f) in defs {
        let t = translate(f, |name| {
            // Tasks[Col] is that column of every task; Resources[Col] reads a Resources sheet as in the tabs.
            if let Some((tb, col)) = name.strip_suffix(']').and_then(|x| x.split_once('[')).filter(|(_, x)| !x.contains(['[', '#'])) {
                let col = col.trim();
                if tb.eq_ignore_ascii_case("resources") {
                    let k = RES_COLS.iter().position(|(_, x)| x.eq_ignore_ascii_case(col))? as u32;
                    res = true;
                    return Some(format!("Resources!${0}$2:${0}${1}", col_name(k), p.resources.len().max(1) + 1));
                }
                if !tb.eq_ignore_ascii_case("tasks") { return None }
                let key = tcols.iter().find(|(_, x)| x.eq_ignore_ascii_case(col)).map(|x| x.0.clone()).or_else(|| c.key_of(col))?;
                let next = cols.len() as u32;
                let k = *cols.entry(key).or_insert(next);
                return Some(format!("${0}$1:${0}${rows}", col_name(k)));
            }
            let key = c.key_of(name)?;
            let next = cols.len() as u32;
            let col = *cols.entry(key).or_insert(next);
            Some(format!("{}{{ROW}}", col_name(col)))
        });
        texts.push(t.ok());
    }
    if res { wb.sheets.push(Arc::new(res_sheet(c))) }
    let sheet: &mut Sheet = match wb.sheet_mut(0) { Some(s) => s, None => return HashMap::new() };
    for (key, &col) in cols.iter().filter(|(k, _)| !defs.iter().any(|(d, _)| &&format!("c:{d}") == k)) {
        let kind = c.kind(key);
        for i in 0..n {
            sheet.set_value(CellRef::new(i as u32, col), to_value(&c.val(i, key), kind, p.day_min));
        }
    }
    for (k, t) in texts.iter().enumerate() {
        for i in 0..n {
            let cell = match t {
                Some(t) => Cell::formula(Formula::new(&t.replace("{ROW}", &(i + 1).to_string()))),
                None => Cell::value(Value::Error(gridcraft_core::CellError::Name)),
            };
            sheet.set_cell(CellRef::new(i as u32, k as u32), cell);
        }
    }
    gridcraft_calc::Calc::new().recalc_all(&mut wb);
    let sheet = &wb.sheets[0];
    defs.iter().enumerate().map(|(k, (key, _))| {
        let kind = c.kind(&format!("c:{key}"));
        (key.clone(), (0..n).map(|i| from_value(&sheet.value(CellRef::new(i as u32, k as u32)), kind, p.day_min)).collect())
    }).collect()
}

/// Columns of the "Plan" sheet and the `Tasks` table: field key and column name. The first 18
/// keep their letters (A–R) so older `Plan!M:M` references still work.
pub const PLAN_COLS: [(&str, &str); 30] = [
    ("id", "ID"), ("code", "WBS"), ("name", "Name"), ("level", "Level"), ("summary", "Summary"), ("dur", "Duration"),
    ("start", "Start"), ("finish", "Finish"), ("pct", "% Complete"), ("b0start", "Baseline Start"), ("b0finish", "Baseline Finish"),
    ("b0dur", "Baseline Duration"), ("cost", "Cost"), ("b0cost", "Baseline Cost"), ("work", "Work"), ("tf", "Total Float"),
    ("critical", "Critical"), ("res", "Resources"), ("milestone", "Milestone"), ("astart", "Actual Start"), ("afinish", "Actual Finish"),
    ("rem", "Remaining Duration"), ("ff", "Free Float"), ("status", "Status"), ("late", "Late"), ("plannedpct", "Planned % Complete"),
    ("bcws", "Planned Value"), ("bcwp", "Earned Value"), ("acwp", "Actual Cost"), ("uid", "Unique ID"),
];
/// Columns of the "Resources" sheet and table: field key and column name.
pub const RES_COLS: [(&str, &str); 10] = [
    ("name", "Name"), ("kind", "Type"), ("group", "Group"), ("initials", "Initials"), ("max", "Max Units"), ("rate", "Rate"),
    ("work", "Work"), ("cost", "Cost"), ("country", "Country"), ("email", "Email"),
];
/// Column names a sheet formula can use after `Tasks[`: the plan columns, then the custom fields.
pub fn task_columns(c: &Calc) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = PLAN_COLS.iter().map(|(k, n)| (k.to_string(), n.to_string())).collect();
    for d in c.p.fields.iter().filter(|d| !d.key.starts_with('r')) {
        let name = if d.alias.is_empty() { c.name(&format!("c:{}", d.key)) } else { d.alias.clone() };
        if !v.iter().any(|(_, n)| n.eq_ignore_ascii_case(&name)) { v.push((format!("c:{}", d.key), name)); }
    }
    v
}

/// The plan's numbers as workbook names (`=ProjectFinish`, `=SPI`), with what each means.
pub fn project_names(c: &Calc) -> Vec<(&'static str, Value, &'static str)> {
    let (p, s, sm) = (c.p, &c.s, c.summary());
    let serial = |t: i64| Value::number(t as f64 / DAY as f64 + EPOCH);
    let opt = |v: Option<f64>| v.map_or(Value::Empty, Value::number);
    let bl = p.tasks.iter().filter_map(|t| t.bls.get(&0).map(|b| b.finish)).max();
    let top: Vec<usize> = (0..p.tasks.len()).filter(|&i| p.tasks[i].level == 0 && !s.off[i]).collect();
    let tot = |v: &[f64]| top.iter().map(|&i| v[i]).sum::<f64>();
    let pc = p.calendars.iter().position(|k| k.name == p.calendar).unwrap_or(0);
    let days = |a: i64, b: i64| { let m = if b >= a { s.cals[pc].between(a, b) } else { -s.cals[pc].between(b, a) }; m as f64 / p.day_min.max(1) as f64 };
    vec![
        ("ProjectName", Value::text(p.name.as_str()), "The plan's name"),
        ("ProjectStart", serial(s.start), "The plan's start date"),
        ("ProjectFinish", serial((s.finish - 1).max(s.start)), "The plan's finish date"),
        ("StatusDate", p.status.map_or(Value::Empty, |d| serial(crate::cal::at(d))), "The status date (empty when not set)"),
        ("BaselineFinish", bl.map_or(Value::Empty, |b| serial(b - 1)), "The baseline finish date"),
        ("FinishVariance", bl.map_or(Value::Empty, |b| Value::number(days(b, s.finish))), "Finish variance in working days (later is positive)"),
        ("PercentComplete", Value::number(sm.pct), "Duration-weighted % complete (0–100, like the task sheet)"),
        ("PlannedPercent", Value::number(planned_pct(c)), "Where the baseline says the plan should be at the status date (0–100, duration-weighted)"),
        ("TotalCost", Value::number(tot(&c.l.cost)), "Total cost"),
        ("PlannedValue", Value::number(tot(&c.ev.pv)), "Planned value (PV) at the status date"),
        ("EarnedValue", Value::number(tot(&c.ev.ev)), "Earned value (EV)"),
        ("ActualCost", Value::number(tot(&c.ev.ac)), "Actual cost (AC)"),
        ("SPI", opt(sm.spi), "Schedule performance index (EV/PV)"),
        ("CPI", opt(sm.cpi), "Cost performance index (EV/AC)"),
        ("TaskCount", Value::number(sm.tasks as f64), "Number of tasks (no summaries)"),
        ("TasksDone", Value::number(sm.done as f64), "Tasks 100% complete"),
        ("TasksLate", Value::number(sm.late as f64), "Late tasks"),
        ("CriticalCount", Value::number(sm.critical as f64), "Critical tasks"),
        ("MilestoneCount", Value::number(sm.milestones as f64), "Milestones"),
    ]
}

/// Planned % complete at the status date, weighted by duration like `PercentComplete`.
fn planned_pct(c: &Calc) -> f64 {
    let (p, s) = (c.p, &c.s);
    let leaves: Vec<usize> = (0..p.tasks.len()).filter(|&i| !s.summary[i] && !s.off[i]).collect();
    let w: f64 = leaves.iter().map(|&i| p.tasks[i].dur as f64).sum();
    if w <= 0.0 { return 0.0 }
    leaves.iter().map(|&i| p.tasks[i].dur as f64 * c.val(i, "plannedpct").num()).sum::<f64>() / w
}

/// Names that hold dates (shown as dates when a cell has no format of its own).
const DATE_NAMES: [&str; 4] = ["PROJECTSTART", "PROJECTFINISH", "STATUSDATE", "BASELINEFINISH"];

#[derive(Serialize)]
pub struct SheetOut {
    pub name: String,
    pub rows: u32,
    pub cols: u32,
    /// Non-empty cells: address, what was typed (empty for spilled results), what is shown, and
    /// whether it is a number.
    pub cells: Vec<(String, String, String, bool)>,
}

/// The plan's date format as an Excel format code (`%d-%b-%y` → `dd-mmm-yy`).
pub fn excel_date(fmt: &str) -> String {
    let mut out = String::new();
    let mut it = fmt.chars();
    while let Some(ch) = it.next() {
        if ch != '%' { out.push(ch); continue; }
        out.push_str(match it.next() { Some('d') => "dd", Some('e') => "d", Some('m') => "mm", Some('b') => "mmm", Some('B') => "mmmm", Some('y') => "yy", Some('Y') => "yyyy", Some('a') => "ddd", Some('A') => "dddd", _ => "" });
    }
    out
}

/// The Excel format code for one of our short format names, or the code itself.
fn code_of(fmt: &str, c: &Calc) -> String {
    match fmt {
        "date" => excel_date(&c.fmt),
        "pct" => "0.0%".into(),
        "0" | "0.00" => fmt.into(),
        _ => fmt.into(),
    }
}

/// Whether a formula gives a date: a date function at the top, or a lookup or MIN/MAX over plan
/// dates.
fn gives_date(input: &str) -> bool {
    let Ok(e) = gridcraft_formula::parse(input.trim_start_matches('=')) else { return false };
    let up = input.to_uppercase();
    let dates = ["START]", "FINISH]", "PLAN!G", "PLAN!H", "PLAN!J", "PLAN!K", "PLAN!$G", "PLAN!$H", "PLAN!$J", "PLAN!$K"].iter().any(|d| up.contains(d)) || DATE_NAMES.iter().any(|n| up.contains(n));
    use gridcraft_formula::Expr;
    match &e {
        Expr::Call(n, _) if ["TODAY", "NOW", "DATE", "EDATE", "EOMONTH", "WORKDAY", "WORKDAY.INTL", "DATEVALUE"].contains(&n.as_str()) => true,
        Expr::Call(n, _) if ["MIN", "MAX", "MINIFS", "MAXIFS", "XLOOKUP", "VLOOKUP", "HLOOKUP", "LOOKUP", "INDEX", "IF", "IFERROR", "SMALL", "LARGE"].contains(&n.as_str()) => dates,
        Expr::Name(_) | Expr::Ref(_) | Expr::Struct(_) => dates,
        _ => false,
    }
}

/// Whether a formula gives one of the plan's percentages (0–100), shown with a % sign.
fn gives_pct(input: &str) -> bool {
    let up = input.to_uppercase();
    let pct = up.contains("% COMPLETE]") || up.contains("PERCENTCOMPLETE") || up.contains("PLANNEDPERCENT");
    let Ok(e) = gridcraft_formula::parse(input.trim_start_matches('=')) else { return false };
    use gridcraft_formula::Expr;
    pct && match &e {
        Expr::Call(n, _) => ["MIN", "MAX", "AVERAGE", "AVERAGEIFS", "MEDIAN", "XLOOKUP", "VLOOKUP", "INDEX", "IF", "IFERROR", "ROUND"].contains(&n.as_str()),
        Expr::Name(_) | Expr::Ref(_) | Expr::Struct(_) => true,
        _ => false,
    }
}

fn show(v: &Value, fmt: &str, input: &str, c: &Calc, sys: gridcraft_core::DateSystem) -> (String, bool) {
    let code = match (v, fmt) {
        (Value::Number(n), "money") => return (money(*n, &c.p.currency), true),
        (_, "flag") => return (v.display(), false),
        (_, "") if input.starts_with('=') && matches!(v, Value::Number(_)) && gives_date(input) => excel_date(&c.fmt),
        (_, "") if input.starts_with('=') && matches!(v, Value::Number(_)) && gives_pct(input) => "0\"%\"".into(),
        (_, "") => gridcraft_core::parse::parse_input(input, sys).format.unwrap_or("General").to_string(),
        _ => code_of(fmt, c),
    };
    let f = gridcraft_numfmt::format_value(v, &gridcraft_numfmt::NumberFormat::parse_cached(&code), sys);
    (f.text, f.numeric)
}

/// A workbook for the plan: the Plan and Resources sheets with their tables, the project names,
/// then the user's tabs, all calculated.
pub fn book(c: &Calc) -> Workbook {
    book_with(c, &[])
}

/// `Tasks[Task Name]`, `Tasks[Total Slack]`: a column called as the task sheet names it reads the
/// plan column with that field, so names copied from the grid work too.
fn grid_names(c: &Calc, cols: &[(String, String)], f: &str) -> String {
    if !f.contains('[') { return f.to_string() }
    translate(f, |name| {
        let Some((tb, col)) = name.strip_suffix(']').and_then(|x| x.split_once('[')).filter(|(t, x)| t.eq_ignore_ascii_case("tasks") && !x.contains(['[', '#'])) else { return (!name.contains('[')).then(|| format!("[{name}]")) };
        if cols.iter().any(|(_, n)| n.eq_ignore_ascii_case(col.trim())) { return None }
        let key = c.key_of(col)?;
        cols.iter().find(|(k, _)| *k == key).map(|(_, n)| format!("{tb}[{n}]"))
    }).map_or_else(|_| f.to_string(), |x| format!("={x}"))
}

/// The "Resources" sheet and table.
fn res_sheet(c: &Calc) -> Sheet {
    let p = c.p;
    let mut rs = Sheet::new("Resources");
    for (k, (key, name)) in RES_COLS.iter().enumerate() {
        rs.set_value(CellRef::new(0, k as u32), Value::text(*name));
        for r in 0..p.resources.len() {
            let v = match *key {
                "max" => Value::number(p.resources[r].max),
                "rate" => Value::number(p.resources[r].rate),
                "work" => Value::number(c.l.res_work[r] / 60.0),
                "cost" => Value::number(c.l.res_cost[r]),
                "country" => Value::text(p.resources[r].country.as_str()),
                _ => Value::text(crate::fields::res_text(c, r, key).as_str()),
            };
            rs.set_value(CellRef::new(r as u32 + 1, k as u32), v);
        }
    }
    rs.tables.push(table(2, "Resources", RES_COLS.iter().map(|x| x.1.to_string()).collect(), p.resources.len() as u32));
    rs
}

/// [`book`] with one extra sheet per formula in `extra` (after the user's tabs), each in its A1.
fn book_with(c: &Calc, extra: &[String]) -> Workbook {
    let p = c.p;
    let mut wb = Workbook::new();
    let cols = task_columns(c);
    let n = p.tasks.len() as u32;
    if let Some(sh) = wb.sheet_mut(0) {
        sh.name = "Plan".into();
        for (k, (key, name)) in cols.iter().enumerate() {
            sh.set_value(CellRef::new(0, k as u32), Value::text(name.as_str()));
            let kind = c.kind(key);
            for i in 0..p.tasks.len() {
                sh.set_value(CellRef::new(i as u32 + 1, k as u32), to_value(&c.val(i, key), kind, p.day_min));
            }
        }
        sh.tables.push(table(1, "Tasks", cols.iter().map(|x| x.1.clone()).collect(), n));
    }
    wb.sheets.push(Arc::new(res_sheet(c)));
    for (name, v, _) in project_names(c) {
        let formula = match &v {
            Value::Number(x) => format!("{x}"),
            Value::Empty => "\"\"".into(),
            v => format!("\"{}\"", v.display().replace('"', "\"\"")),
        };
        wb.names.push(gridcraft_model::DefinedName { name: name.into(), scope: None, formula, comment: String::new(), hidden: false });
    }
    for s in &p.sheets {
        let mut sh = Sheet::new(s.name.as_str());
        for (a1, input) in &s.cells {
            let Some(at) = CellRef::parse(a1) else { continue };
            let cell = if input.starts_with('=') && input.len() > 1 {
                Cell::formula(Formula::new(&grid_names(c, &cols, input)))
            } else {
                Cell::value(gridcraft_core::parse::parse_input(input, wb.date_system).value)
            };
            sh.set_cell(at, cell);
        }
        wb.sheets.push(Arc::new(sh));
    }
    for (k, f) in extra.iter().enumerate() {
        let mut sh = Sheet::new(format!("__eval{k}"));
        let f = grid_names(c, &cols, &if f.starts_with('=') { f.clone() } else { format!("={f}") });
        sh.set_cell(CellRef::new(0, 0), Cell::formula(Formula::new(&f)));
        wb.sheets.push(Arc::new(sh));
    }
    gridcraft_calc::Calc::new().recalc_all(&mut wb);
    wb
}

/// One value as a dashboard shows it: the text, the number (if any) and the cell's format.
pub type Shown = (String, Option<f64>, String);

/// Works out dashboard formulas: each `(formula, format)` gives a table of shown values, one cell
/// for a single value, more when the result spills (FILTER, a range). A plain reference to a range
/// on a sheet keeps that sheet's formats (dates, money, flags). Tables are capped at 500 × 40.
pub fn eval(c: &Calc, items: &[(String, String)]) -> Vec<Vec<Vec<Shown>>> {
    let p = c.p;
    let wb = book_with(c, &items.iter().map(|x| x.0.clone()).collect::<Vec<_>>());
    let base = BUILT_IN + p.sheets.len();
    let num = |v: &Value| match v { Value::Number(n) => Some(*n), _ => None };
    items.iter().enumerate().map(|(k, (f, fmt))| {
        // `=Sheet!A1:C9` shows the sheet's own cells, with their formats.
        if let Some((tab, r)) = sheet_range(f).and_then(|(name, r)| p.sheets.iter().position(|s| s.name.eq_ignore_ascii_case(&name)).map(|t| (t, r))) {
            let (sh, sd) = (wb.sheet(tab + BUILT_IN), &p.sheets[tab]);
            return (r.start.row..=r.end.row.min(r.start.row + 499)).map(|row| (r.start.col..=r.end.col.min(r.start.col + 39)).map(|col| {
                let at = CellRef::new(row, col);
                let a1 = format!("{}{}", col_name(col), row + 1);
                let v = sh.map_or(Value::Empty, |sh| sh.value(at));
                let f = sd.fmt.get(&a1).cloned().unwrap_or_default();
                (show(&v, &f, sd.cells.get(&a1).map_or("", String::as_str), c, wb.date_system).0, num(&v), f)
            }).collect()).collect();
        }
        let Some(sh) = wb.sheet(base + k) else { return vec![] };
        let end = sh.used_range().map_or(CellRef::new(0, 0), |u| u.end);
        (0..=end.row.min(499)).map(|row| (0..=end.col.min(39)).map(|col| {
            let v = sh.value(CellRef::new(row, col));
            // The first cell's formula decides date and % formats; spilled cells use the given format.
            let input = if row == 0 && col == 0 { f.as_str() } else { "" };
            (show(&v, fmt, input, c, wb.date_system).0, num(&v), fmt.clone())
        }).collect()).collect()
    }).collect()
}

/// `=Sheet!A1:C9` or `='My sheet'!B2` → the sheet name and the range.
fn sheet_range(f: &str) -> Option<(String, gridcraft_core::RangeRef)> {
    let (name, r) = f.trim().strip_prefix('=')?.rsplit_once('!')?;
    let name = name.trim().trim_matches('\'').replace("''", "'");
    let cell = |x: &str| CellRef::parse(&x.replace('$', ""));
    let (a, b) = match r.split_once(':') { Some((a, b)) => (cell(a)?, cell(b)?), None => (cell(r)?, cell(r)?) };
    Some((name, gridcraft_core::RangeRef::new(a, b)))
}

/// The sheets before the user's tabs in [`book`].
pub const BUILT_IN: usize = 2;

fn table(id: u32, name: &str, cols: Vec<String>, rows: u32) -> gridcraft_model::Table {
    use gridcraft_core::RangeRef;
    gridcraft_model::Table {
        id, name: name.into(), range: RangeRef::new(CellRef::new(0, 0), CellRef::new(rows.max(1), cols.len().max(1) as u32 - 1)),
        header_row: true, totals_row: false,
        columns: cols.into_iter().map(|name| gridcraft_model::TableColumn { name, totals: Default::default(), totals_label: None, formula: None, unique_name: None, xml: None }).collect(),
        style: String::new(), banded_rows: false, banded_cols: false, first_col: false, last_col: false, filter_button: false, filter: vec![], table_type: None, connection_id: None,
    }
}

/// One spreadsheet tab as shown: `k` is its index in `Project.sheets` (the Plan and Resources
/// sheets are not editable; they are shown through the task and resource views).
pub fn sheet_view(c: &Calc, k: usize) -> Option<SheetOut> {
    let s: &SheetData = c.p.sheets.get(k)?;
    let wb = book(c);
    let sh = wb.sheet(k + BUILT_IN)?;
    let (mut rows, mut cols, mut cells) = (0, 0, vec![]);
    let mut add = |a1: String, input: String, at: CellRef, cells: &mut Vec<(String, String, String, bool)>| {
        let (text, num) = show(&sh.value(at), s.fmt.get(&a1).map_or("", String::as_str), &input, c, wb.date_system);
        rows = rows.max(at.row + 1);
        cols = cols.max(at.col + 1);
        cells.push((a1, input, text, num));
    };
    for (a1, input) in &s.cells {
        if let Some(at) = CellRef::parse(a1) { add(a1.clone(), input.clone(), at, &mut cells); }
    }
    // Results that spill from a formula into the cells next to it (UNIQUE, FILTER, SORT…).
    if let Some(used) = sh.used_range() {
        for r in used.start.row..=used.end.row.min(used.start.row + 5000) {
            for k in used.start.col..=used.end.col.min(used.start.col + 200) {
                let at = CellRef::new(r, k);
                let a1 = format!("{}{}", col_name(k), r + 1);
                if !s.cells.contains_key(&a1) && sh.spill_anchor(at).is_some_and(|x| x != at) && !matches!(sh.value(at), Value::Empty) {
                    add(a1, String::new(), at, &mut cells);
                }
            }
        }
    }
    Some(SheetOut { name: s.name.clone(), rows, cols, cells })
}

/// Every worksheet function: name, signature, one-line description and category.
pub fn functions() -> Vec<(&'static str, &'static str, &'static str, String)> {
    let mut v: Vec<_> = gridcraft_functions::all().iter().map(|f| (f.name, f.signature, f.description, format!("{:?}", f.category))).collect();
    v.sort_by_key(|f| f.0);
    v
}

/// Moves a formula's relative references by `dr` rows and `dc` columns (fill and paste).
/// Inputs that are not formulas come back unchanged.
pub fn shift(input: &str, dr: i64, dc: i64) -> String {
    let Some(body) = input.strip_prefix('=') else { return input.into() };
    match gridcraft_formula::parse(body) {
        Ok(e) => format!("={}", gridcraft_formula::printer::print(&gridcraft_formula::adjust::shift_relative(e, dr, dc))),
        Err(_) => input.into(),
    }
}

/// A formula after rows or columns were inserted or deleted on sheet `target`.
pub fn adjust(input: &str, host: &str, target: &str, edit: &gridcraft_formula::adjust::Edit) -> String {
    rewrite(input, |e| gridcraft_formula::adjust::adjust(e, host, target, edit))
}

/// A formula after sheet `old` was renamed to `new` (`new` empty: deleted, its references go to #REF!).
pub fn rename(input: &str, old: &str, new: &str, order: &[String]) -> String {
    use gridcraft_formula::adjust as a;
    rewrite(input, |e| if new.is_empty() { a::delete_sheet(e, old, order) } else { a::rename_sheet(e, old, new) })
}

fn rewrite(input: &str, f: impl FnOnce(gridcraft_formula::Expr) -> gridcraft_formula::Expr) -> String {
    let Some(body) = input.strip_prefix('=') else { return input.into() };
    match gridcraft_formula::parse(body) {
        Ok(e) => format!("={}", gridcraft_formula::printer::print(&f(e))),
        Err(_) => input.into(),
    }
}
