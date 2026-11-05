//! Spreadsheet formulas through the Gridcraft engine: formula columns on tasks (`=[% Complete]-[Baseline
//! % Complete]`) and the plan's spreadsheet tabs, which can also read a "Plan" sheet of all tasks.
use crate::cal::DAY;
use crate::fields::{money, Calc, Kind};
use crate::formula::F;
use crate::model::SheetData;
use gridcraft_core::{CellRef, Value};
use gridcraft_formula::{BinOp, Expr, Reference};
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

/// A task's value for a spreadsheet: a date is the day the task sheet shows, with no time of day (so
/// `Tasks[Finish]<=DATE(…)` holds on that day and Finish − Start counts whole days).
pub fn cell_value(c: &Calc, i: usize, key: &str) -> Value {
    let kind = c.kind(key);
    match c.val(i, key) {
        F::Date(m) => {
            let ms = !c.s.summary[i] && c.p.tasks[i].dur == 0 && matches!(key, "finish" | "ef");
            day(if kind == Kind::Fin && !ms && m.rem_euclid(DAY) == 0 { m - 1 } else { m })
        }
        v => to_value(&v, kind, c.p.day_min),
    }
}

/// The day of an instant as a spreadsheet date serial.
fn day(t: i64) -> Value { Value::number(t.div_euclid(DAY) as f64 + EPOCH) }

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
    if too_deep(&t) { return Err("The formula is nested more than 64 levels deep.".into()) }
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
        for i in 0..n {
            sheet.set_value(CellRef::new(i as u32, col), cell_value(c, i, key));
        }
    }
    for (k, t) in texts.iter().enumerate() {
        for i in 0..n {
            let cell = match t {
                Some(t) => formula_cell(&t.replace("{ROW}", &(i + 1).to_string()), &c.fmt),
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
    // Days, as the task sheet shows them (a finish at midnight is the day before).
    let serial = day;
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
    // A plan of milestones only takes the plain average, as PercentComplete does.
    if w <= 0.0 { return if leaves.is_empty() { 0.0 } else { leaves.iter().map(|&i| c.val(i, "plannedpct").num()).sum::<f64>() / leaves.len() as f64 } }
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
    pub cells: Vec<(String, String, String, bool, Option<f64>)>,
    /// Rows the AutoFilter hides, and the values of each drop-down list (see `sheetx`).
    pub filtered: Vec<u32>,
    pub lists: Vec<Vec<String>>,
    /// Cells that are links (a typed web or mail address, or `=HYPERLINK(address, text)`): cell and address.
    pub links: Vec<(String, String)>,
    /// Cells whose value breaks their data validation (Circle Invalid Data).
    pub invalid: Vec<String>,
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

/// Spreadsheets refuse formulas nested more than 64 levels deep; deeper ones (brackets, a run of
/// signs such as `----1` or `1%%%`, or `A1:A1:A1…`) are not parsed here either: a tree that deep
/// overflowed the stack and closed the app.
pub fn too_deep(f: &str) -> bool {
    let (mut depth, mut max, mut signs, mut colons, mut quoted) = (0i32, 0i32, 0i32, 0i32, false);
    for ch in f.chars() {
        if quoted { quoted = ch != '"'; continue }
        match ch {
            '"' => quoted = true,
            '(' => { depth += 1; max = max.max(depth) }
            ')' => depth -= 1,
            '+' | '-' | '%' | '@' => { signs += 1; max = max.max(signs) }
            ':' => { colons += 1; max = max.max(colons) }
            ch if ch.is_whitespace() => {}
            _ => signs = 0,
        }
        if "(),;*/^&=<>".contains(ch) { (signs, colons) = (0, 0) }
    }
    max > 64
}

/// A formula's tree, or None when it does not parse or is nested too deeply.
fn parse(body: &str) -> Option<Expr> {
    if too_deep(body) { None } else { gridcraft_formula::parse(body).ok() }
}

/// A formula cell for the engine; one nested too deeply is #VALUE!. Dates written as text in criteria,
/// DATEVALUE and arithmetic (`COUNTIF(A:A,">13/01/2026")`) are read in the plan's day order.
fn formula_cell(f: &str, fmt: &str) -> Cell {
    if too_deep(f) { return Cell::value(Value::Error(gridcraft_core::CellError::Value)) }
    let dated = f.contains('"') && f.contains(['/', '-', '.']);
    match parse(f.strip_prefix('=').unwrap_or(f)).filter(|_| dated) {
        Some(e) => Cell::formula(Formula::new(&format!("={}", gridcraft_formula::printer::print(&date_texts(e, fmt))))),
        None => Cell::formula(Formula::new(f)),
    }
}

/// Text that is a date typed as the plan writes it (with a criteria operator before it: ">=13/01/2026")
/// becomes the same date year first, which the engine reads one way only.
fn date_texts(e: Expr, fmt: &str) -> Expr {
    let iso = |t: &str| -> Option<String> {
        let op = t.len() - t.trim_start_matches(['<', '>', '=']).len();
        let (op, rest) = t.split_at(op);
        if !date_shaped(rest) { return None }
        let d = chrono::NaiveDate::from_ymd_opt(1899, 12, 30)?.checked_add_days(chrono::Days::new(typed_date(rest, fmt)? as u64))?;
        Some(format!("{op}{}", d.format("%Y-%m-%d")))
    };
    // A text argument, or the text parts of one built with & (">"&"10/01/2026").
    fn fix(e: Expr, iso: &dyn Fn(&str) -> Option<String>) -> Expr {
        match e {
            Expr::Text(t) => Expr::Text(iso(&t).map_or(t, Into::into)),
            Expr::Binary(BinOp::Concat, a, b) => Expr::Binary(BinOp::Concat, Box::new(fix(*a, iso)), Box::new(fix(*b, iso))),
            Expr::Paren(x) => Expr::Paren(Box::new(fix(*x, iso))),
            e => e,
        }
    }
    e.map(&mut |e| match e {
        Expr::Call(n, args) => {
            let crit = |i: usize| match n.as_str() {
                "COUNTIF" | "SUMIF" | "AVERAGEIF" => i == 1,
                "COUNTIFS" => i % 2 == 1,
                "SUMIFS" | "AVERAGEIFS" | "MAXIFS" | "MINIFS" => i > 0 && i.is_multiple_of(2),
                "DATEVALUE" => i == 0,
                _ => false,
            };
            let args = args.into_iter().enumerate().map(|(i, a)| if crit(i) { fix(a, &iso) } else { a }).collect();
            Expr::Call(n, args)
        }
        Expr::Binary(op @ (BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div), a, b) => {
            let one = |x: Box<Expr>| match *x { Expr::Text(t) => Box::new(Expr::Text(iso(&t).map_or(t, Into::into))), x => Box::new(x) };
            Expr::Binary(op, one(a), one(b))
        }
        e => e,
    })
}

/// A plan column that holds dates (Start, Finish, Baseline Start, Actual Finish…).
fn date_col(name: &str) -> bool {
    let n = name.trim().to_lowercase();
    n.ends_with("start") || n.ends_with("finish")
}

/// Works out which cells and formulas give dates, remembering each cell it looked at.
pub struct Dates<'a> {
    c: &'a Calc<'a>,
    memo: std::cell::RefCell<HashMap<(usize, String), bool>>,
    /// Each tab's filled cells, by column then row.
    index: std::cell::RefCell<HashMap<usize, std::rc::Rc<Filled>>>,
}

/// A tab's filled cells as (column, row, address), sorted.
type Filled = Vec<(u32, u32, String)>;

impl<'a> Dates<'a> {
    pub fn new(c: &'a Calc<'a>) -> Self { Dates { c, memo: Default::default(), index: Default::default() } }

    /// The number format a cell on tab `k` shows in when it has none of its own: a typed date in the
    /// plan's date format, a typed amount with its look (25%, $1,200, €12), a formula giving a date or
    /// a time, or one of the plan's percentages (0–100) with a % sign.
    pub fn code(&self, k: Option<usize>, input: &str) -> Option<String> {
        let c = self.c;
        let Some(body) = input.strip_prefix('=').filter(|b| !b.is_empty()) else {
            if typed_date(input, &c.fmt).is_some() { return Some(excel_date(&c.fmt)) }
            if date_shaped(input) { return None }
            return typed_money(input, &c.p.currency.symbol).map(|x| x.1)
                .or_else(|| gridcraft_core::parse::parse_input(input, Default::default()).format.filter(|f| *f != "General").map(str::to_string));
        };
        let e = parse(body)?;
        if self.expr(&e, k, 0) { return Some(excel_date(&c.fmt)) }
        if matches!(&e, Expr::Call(n, _) if n == "TIME" || n == "TIMEVALUE") { return Some("h:mm".into()) }
        pct_code(&e, body)
    }

    /// Whether a formula gives a date: a date function, a plan date (`Tasks[Finish]`, `ProjectFinish`,
    /// `Plan!H2`), a cell holding a date, a lookup or MIN/MAX of dates, or a date plus or minus days.
    /// A date minus a date is a number of days.
    fn expr(&self, e: &Expr, k: Option<usize>, depth: u8) -> bool {
        let d = |x: Option<&Expr>| x.is_some_and(|x| self.expr(x, k, depth));
        match e {
            Expr::Paren(x) => self.expr(x, k, depth),
            Expr::Binary(BinOp::Add | BinOp::Sub, ..) => {
                // Down a chain a+b−c… without recursing along it: dates added count one, dates taken
                // away count minus one; one date in all is a date.
                let (mut e, mut n) = (e, 0);
                while let Expr::Binary(op @ (BinOp::Add | BinOp::Sub), a, b) = e {
                    if d(Some(b)) { n += if *op == BinOp::Sub { -1 } else { 1 } }
                    e = a;
                }
                n + d(Some(e)) as i32 == 1
            }
            Expr::Name(n) => DATE_NAMES.contains(&n.to_uppercase().as_str()),
            Expr::Struct(s) => s.col_start.as_deref().is_some_and(date_col) && s.col_end.as_deref().is_none_or(date_col),
            Expr::Ref(r) => self.reads(r, k, depth),
            Expr::Call(n, a) => match n.as_str() {
                "TODAY" | "NOW" | "DATE" | "EDATE" | "EOMONTH" | "WORKDAY" | "WORKDAY.INTL" | "DATEVALUE" => true,
                "MIN" | "MAX" | "MEDIAN" | "AVERAGE" => a.iter().any(|x| d(Some(x))),
                "MINIFS" | "MAXIFS" | "SMALL" | "LARGE" | "INDEX" | "IFERROR" | "IFNA" | "FILTER" | "SORT" | "SORTBY" | "UNIQUE" | "TAKE" | "DROP" => d(a.first()),
                "IF" => d(a.get(1)) || d(a.get(2)),
                "XLOOKUP" => d(a.get(2)),
                "LOOKUP" => d(a.last()),
                // VLOOKUP over the Plan sheet or the Tasks table: the column it returns.
                "VLOOKUP" => {
                    let first = match a.get(1) {
                        Some(Expr::Ref(r)) if r.sheet_name().is_some_and(|s| s.eq_ignore_ascii_case("plan")) => r.range().start.col as i64,
                        Some(Expr::Struct(s)) if s.table.eq_ignore_ascii_case("tasks") && s.col_start.is_none() => 0,
                        _ => return false,
                    };
                    matches!(a.get(2), Some(Expr::Number(n)) if usize::try_from(first + *n as i64 - 1).ok().and_then(|i| PLAN_COLS.get(i)).is_some_and(|x| date_col(x.1)))
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// Whether a reference reads dates: a date column of the Plan sheet, or a cell of a tab with a date
    /// format, a typed date or a formula giving a date (any cell, for a range).
    fn reads(&self, r: &Reference, k: Option<usize>, depth: u8) -> bool {
        let (sheets, range) = (&self.c.p.sheets, r.range());
        let k = match r.sheet_name() {
            Some(n) if n.eq_ignore_ascii_case("plan") => return (range.start.col..=range.end.col).any(|i| PLAN_COLS.get(i as usize).is_some_and(|x| date_col(x.1))),
            Some(n) => sheets.iter().position(|s| s.name.eq_ignore_ascii_case(n)),
            None => k,
        };
        let (Some(k), Some(s)) = (k, k.and_then(|k| sheets.get(k))) else { return false };
        let idx = self.index.borrow_mut().entry(k).or_insert_with(|| {
            let mut v: Filled = s.cells.keys().filter_map(|a1| CellRef::parse(a1).map(|r| (r.col, r.row, a1.clone()))).collect();
            v.sort_unstable();
            std::rc::Rc::new(v)
        }).clone();
        // The first filled cells of the range decide (64 at most, so a running MAX(A$1:A9) stays quick).
        let (mut i, mut seen) = (idx.partition_point(|x| (x.0, x.1) < (range.start.col, range.start.row)), 0);
        while let Some((col, row, a1)) = idx.get(i) {
            if *col > range.end.col || seen >= 64 { return false }
            if *row > range.end.row { i = idx.partition_point(|x| (x.0, x.1) < (col + 1, range.start.row)); continue }
            if *row < range.start.row { i = idx.partition_point(|x| (x.0, x.1) < (*col, range.start.row)); continue }
            if self.cell(k, s, a1, depth) { return true }
            (i, seen) = (i + 1, seen + 1);
        }
        false
    }

    fn cell(&self, k: usize, s: &SheetData, a1: &str, depth: u8) -> bool {
        if let Some(f) = s.fmt.get(a1) { return f == "date" || f.contains("yy") || f.contains("mmm") }
        let Some(input) = s.cells.get(a1) else { return false };
        let Some(body) = input.strip_prefix('=') else { return typed_date(input, &self.c.fmt).is_some() };
        if depth >= 8 { return false }
        let key = (k, a1.to_string());
        if let Some(&x) = self.memo.borrow().get(&key) { return x }
        // A loop of references reads as no date.
        self.memo.borrow_mut().insert(key.clone(), false);
        let x = parse(body).is_some_and(|e| self.expr(&e, Some(k), depth + 1));
        self.memo.borrow_mut().insert(key, x);
        x
    }
}

/// `0"%"` for one of the plan's percentages (0–100); a ROUND keeps its decimals (`0.0"%"`).
fn pct_code(e: &Expr, body: &str) -> Option<String> {
    let up = body.to_uppercase();
    if !(up.contains("% COMPLETE]") || up.contains("PERCENTCOMPLETE") || up.contains("PLANNEDPERCENT")) { return None }
    let places = match e {
        Expr::Call(n, a) if matches!(n.as_str(), "ROUND" | "ROUNDUP" | "ROUNDDOWN") => match a.get(1) { Some(Expr::Number(x)) if *x >= 1.0 => (*x as usize).min(10), _ => 0 },
        Expr::Call(n, _) if ["MIN", "MAX", "AVERAGE", "AVERAGEIFS", "MEDIAN", "XLOOKUP", "VLOOKUP", "INDEX", "IF", "IFERROR"].contains(&n.as_str()) => 0,
        Expr::Name(_) | Expr::Ref(_) | Expr::Struct(_) => 0,
        _ => return None,
    };
    Some(if places == 0 { "0\"%\"".into() } else { format!("0.{}\"%\"", "0".repeat(places)) })
}

/// Whether 03/04/2026 is 3 April: the day comes first unless the plan's format starts with the month
/// (10/24/2026), as the window's fill handle reads it.
fn day_first(fmt: &str) -> bool { !fmt.starts_with("%m") }

/// A date typed in a sheet, as a serial: numbers in the plan's day order (24/10/2026 is 24 October
/// when the plan writes the day first; 3/4 is this year), two-digit years as spreadsheets read them
/// (00–29 → 2000s, 30–99 → 1900s), year-first ISO dates, or the plan's own date format.
fn typed_date(input: &str, fmt: &str) -> Option<f64> {
    let t = input.trim();
    if !t.starts_with(|ch: char| ch.is_ascii_digit()) || !t.contains(['/', '-', '.']) { return None }
    let serial = |d: chrono::NaiveDate| (d - chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap_or_default()).num_days() as f64 + EPOCH;
    let sep = t.chars().find(|ch| "/-.".contains(*ch))?;
    let n: Vec<&str> = t.split(sep).collect();
    if n.iter().all(|x| (1..=4).contains(&x.len()) && x.bytes().all(|b| b.is_ascii_digit())) {
        let v: Vec<i32> = n.iter().filter_map(|x| x.parse().ok()).collect();
        let (y, m, d) = match (n.len(), n[0].len()) {
            (3, 4) => (v[0], v[1], v[2]),
            (2 | 3, 1 | 2) => {
                let (m, d) = if day_first(fmt) { (v[1], v[0]) } else { (v[0], v[1]) };
                let y = match n.get(2).map(|y| y.len()) {
                    None if sep != '.' => { use chrono::Datelike; chrono::Local::now().year() }
                    Some(4) => v[2],
                    Some(2) if sep != '.' => if v[2] < 30 { 2000 + v[2] } else { 1900 + v[2] },
                    _ => return None,
                };
                (y, m, d)
            }
            _ => return None,
        };
        return chrono::NaiveDate::from_ymd_opt(y, u32::try_from(m).ok()?, u32::try_from(d).ok()?).map(serial);
    }
    use chrono::Datelike;
    chrono::NaiveDate::parse_from_str(t, fmt).ok().filter(|d| d.year() >= 1900).map(serial)
}

/// Numbers that look like a date (1.2.3, 13/01/2026 in a month-first plan) but that [`typed_date`]
/// does not read: they stay text rather than become some other day.
fn date_shaped(input: &str) -> bool {
    let t = input.trim();
    let Some(sep) = t.chars().find(|ch| "/-.".contains(*ch)) else { return false };
    let n: Vec<&str> = t.split(sep).collect();
    n.len() == 3 && n[0].len() <= 2 && n.iter().all(|x| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit()))
}

/// An amount typed with a currency symbol before or after it (€1,234.50, 12 £, -€5), as a number and
/// the format it shows in. Dollars the engine reads itself.
fn typed_money(input: &str, sym: &str) -> Option<(f64, String)> {
    let t = input.trim();
    let (neg, t) = t.strip_prefix('-').map_or((false, t), |r| (true, r.trim_start()));
    ["€", "£", "¥", "₹", sym.trim()].into_iter().filter(|s| !s.is_empty() && *s != "$").find_map(|s| {
        let (body, after) = match t.strip_prefix(s) { Some(b) => (b.trim(), false), None => (t.strip_suffix(s)?.trim(), true) };
        let (neg, body) = body.strip_prefix('-').map_or((neg, body), |b| (!neg, b));
        let digits = body.replace(',', "");
        if !body.starts_with(|ch: char| ch.is_ascii_digit()) || !digits.bytes().all(|b| b.is_ascii_digit() || b == b'.') || body.split('.').nth(1).is_some_and(|d| d.contains(',')) { return None }
        let n: f64 = digits.parse().ok()?;
        let dec = body.split_once('.').map_or(0, |x| x.1.len().min(4));
        let code = format!("#,##0{}{}", if dec > 0 { "." } else { "" }, "0".repeat(dec));
        let q = format!("\"{}\"", s.replace('"', ""));
        Some((if neg { -n } else { n }, if after { format!("{code} {q}") } else { format!("{q}{code}") }))
    })
}

fn show(v: &Value, fmt: &str, input: &str, auto: &Dates, k: Option<usize>, sys: gridcraft_core::DateSystem) -> (String, bool) {
    let c = auto.c;
    let code = match (v, fmt) {
        (Value::Number(n), "money") => return (money(*n, &c.p.currency), true),
        (_, "flag") => return (v.display(), false),
        (_, "") if input.starts_with('=') && !matches!(v, Value::Number(_)) => "General".into(),
        (_, "") => match auto.code(k, input) {
            // A formula's date with nothing in it (a lookup of an empty plan date) shows blank.
            Some(code) if input.starts_with('=') && matches!(v, Value::Number(x) if *x == 0.0) && code == excel_date(&c.fmt) => return (String::new(), false),
            code => code.unwrap_or_else(|| "General".into()),
        },
        _ => code_of(fmt, c),
    };
    let f = gridcraft_numfmt::format_value(v, &gridcraft_numfmt::NumberFormat::parse_cached(&code), sys);
    (f.text, f.numeric)
}

/// What a value typed in a cell is: text-formatted cells keep what was typed (001 stays 001); dates
/// follow the plan's date format; amounts may carry a currency symbol.
fn typed(c: &Calc, s: &SheetData, a1: &str, input: &str) -> Value {
    if s.fmt.get(a1).is_some_and(|f| f == "@") { Value::text(input) }
    else if let Some(n) = typed_date(input, &c.fmt) { Value::number(n) }
    else if let Some((n, _)) = typed_money(input, &c.p.currency.symbol) { Value::number(n) }
    else if date_shaped(input) { Value::text(input) }
    else { gridcraft_core::parse::parse_input(input, gridcraft_core::DateSystem::default()).value }
}

/// A validation's bound for the cell `at` of tab `k`: a value typed as in a cell (5, 31/12/2026, 13:30),
/// or a reference or formula ("=B1", "$B$1", "Lists!A1"), written for the range's first cell `first`.
fn bound(c: &Calc, wb: &Workbook, k: usize, first: CellRef, at: CellRef, x: &str) -> Option<f64> {
    let x = x.trim();
    if x.is_empty() { return None }
    let refish = x.starts_with('=') || x.contains('!') || CellRef::parse(&x.replace('$', "")).is_some();
    let v = if refish {
        let f = shift(&format!("={}", x.trim_start_matches('=')), at.row as i64 - first.row as i64, at.col as i64 - first.col as i64);
        gridcraft_calc::evaluate(wb, k + BUILT_IN, at, &if f.contains('[') { grid_names(c, &task_columns(c), &f) } else { f })
    } else { typed(c, &c.p.sheets[k], "", x) };
    match v { Value::Number(n) if n.is_finite() => Some(n), Value::Bool(b) => Some(b as u8 as f64), _ => None }
}

/// A validation's bound as a workbook formula (without =): a number for a typed value (a date as its
/// serial), the reference or formula itself otherwise, quoted text when it is neither.
pub fn bound_text(c: &Calc, k: usize, x: &str) -> String {
    let x = x.trim();
    if x.starts_with('=') || x.contains('!') || CellRef::parse(&x.replace('$', "")).is_some() { return excel_formula(c, &format!("={}", x.trim_start_matches('='))).trim_start_matches('=').to_string() }
    match c.p.sheets.get(k).map(|s| typed(c, s, "", x)) { Some(Value::Number(n)) => n.to_string(), _ if x.is_empty() => "0".into(), _ => format!("\"{}\"", x.replace('"', "\"\"")) }
}

/// Whether cell `at` of tab `k`, showing `text` with the number `n`, passes validation `i`.
fn passes(c: &Calc, wb: &Workbook, k: usize, i: usize, at: CellRef, (text, n): (&str, Option<f64>), lists: &[Vec<String>]) -> bool {
    let v = &c.p.sheets[k].valid[i];
    let first = crate::model::sheet_range(&v.range).map_or(at, |g| CellRef::new(g.0, g.1));
    let empty = vec![];
    crate::sheetx::valid(v, text, n, bound(c, wb, k, first, at, &v.a), bound(c, wb, k, first, at, &v.b), lists.get(i).unwrap_or(&empty))
}

/// Checks a value typed into cell `a1` of tab `k` against the cell's data validation: None when it may
/// go in; otherwise the message and whether it is only a warning (the user may keep it).
pub fn check_input(c: &Calc, k: usize, a1: &str, input: &str) -> Option<(String, bool)> {
    let s = c.p.sheets.get(k)?;
    let at = CellRef::parse(a1)?;
    let i = s.valid.iter().position(|v| crate::model::sheet_range(&v.range).is_some_and(|g| at.row >= g.0 && at.row <= g.2 && at.col >= g.1 && at.col <= g.3))?;
    let wb = book(c);
    let v = if input.starts_with('=') && input.len() > 1 { gridcraft_calc::evaluate(&wb, k + BUILT_IN, at, &grid_names(c, &task_columns(c), input)) } else { typed(c, s, a1, input) };
    let n = match v { Value::Number(x) if x.is_finite() => Some(x), _ => None };
    let lists = if s.valid[i].kind.is_empty() { sheet_view(c, k).map(|o| o.lists).unwrap_or_default() } else { vec![] };
    let text = if input.starts_with('=') { v.display() } else { input.to_string() };
    (!passes(c, &wb, k, i, at, (&text, n), &lists)).then(|| (crate::sheetx::message(&s.valid[i]), s.valid[i].warn))
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
            for i in 0..p.tasks.len() {
                sh.set_value(CellRef::new(i as u32 + 1, k as u32), cell_value(c, i, key));
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
    // The plan's defined names (Rates, VAT), read like formulas (`Tasks[Total Slack]` works too).
    for n in &p.names {
        let f = grid_names(c, &cols, &n.formula);
        wb.names.push(gridcraft_model::DefinedName { name: n.name.clone(), scope: None, formula: f.trim_start_matches('=').to_string(), comment: n.comment.clone(), hidden: false });
    }
    for s in &p.sheets {
        let mut sh = Sheet::new(s.name.as_str());
        for (a1, input) in &s.cells {
            let Some(at) = CellRef::parse(a1) else { continue };
            let cell = if input.starts_with('=') && input.len() > 1 { formula_cell(&grid_names(c, &cols, input), &c.fmt) } else { Cell::value(typed(c, s, a1, input)) };
            sh.set_cell(at, cell);
        }
        wb.sheets.push(Arc::new(sh));
    }
    for (k, f) in extra.iter().enumerate() {
        let mut sh = Sheet::new(format!("__eval{k}"));
        let f = grid_names(c, &cols, &if f.starts_with('=') { f.clone() } else { format!("={f}") });
        sh.set_cell(CellRef::new(0, 0), formula_cell(&f, &c.fmt));
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
    let auto = Dates::new(c);
    items.iter().enumerate().map(|(k, (f, fmt))| {
        // `=Sheet!A1:C9` shows the sheet's own cells, with their formats.
        if let Some((tab, r)) = sheet_range(f).and_then(|(name, r)| p.sheets.iter().position(|s| s.name.eq_ignore_ascii_case(&name)).map(|t| (t, r))) {
            let (sh, sd) = (wb.sheet(tab + BUILT_IN), &p.sheets[tab]);
            // Up to the last cell in use, so a generous range (A1:Z5000) does not show hundreds of empty rows and series.
            let used = sh.and_then(|sh| sh.used_range()).map_or(CellRef::new(0, 0), |u| u.end);
            return (r.start.row..=r.end.row.min(used.row).min(r.start.row + 499)).map(|row| (r.start.col..=r.end.col.min(used.col).min(r.start.col + 39)).map(|col| {
                let at = CellRef::new(row, col);
                let a1 = format!("{}{}", col_name(col), row + 1);
                let v = sh.map_or(Value::Empty, |sh| sh.value(at));
                let f = sd.fmt.get(&a1).cloned().unwrap_or_default();
                (show(&v, &f, sd.cells.get(&a1).map_or("", String::as_str), &auto, Some(tab), wb.date_system).0, num(&v), f)
            }).collect()).collect();
        }
        let Some(sh) = wb.sheet(base + k) else { return vec![] };
        let end = sh.used_range().map_or(CellRef::new(0, 0), |u| u.end);
        (0..=end.row.min(499)).map(|row| (0..=end.col.min(39)).map(|col| {
            let v = sh.value(CellRef::new(row, col));
            // The formula decides date and % formats, for its spilled cells too (a list of finish dates).
            let input = if f.starts_with('=') { f.clone() } else { format!("={f}") };
            (show(&v, fmt, &input, &auto, None, wb.date_system).0, num(&v), fmt.clone())
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
    let auto = Dates::new(c);
    // `from`: the cell whose input and format decide the look (a spilled cell looks like its formula's).
    let mut add = |a1: String, input: String, at: CellRef, from: &str, cells: &mut Vec<(String, String, String, bool, Option<f64>)>| {
        let fmt = s.fmt.get(&a1).or_else(|| s.fmt.get(from)).map_or("", String::as_str);
        let (text, num) = show(&sh.value(at), fmt, s.cells.get(from).unwrap_or(&input), &auto, Some(k), wb.date_system);
        rows = rows.max(at.row + 1);
        cols = cols.max(at.col + 1);
        let n = match sh.value(at) { Value::Number(x) if x.is_finite() => Some(x), _ => None };
        cells.push((a1, input, text, num, n));
    };
    for (a1, input) in &s.cells {
        if let Some(at) = CellRef::parse(a1) { add(a1.clone(), input.clone(), at, a1, &mut cells); }
    }
    // Results that spill from a formula into the cells next to it (UNIQUE, FILTER, SORT…).
    if let Some(used) = sh.used_range() {
        for r in used.start.row..=used.end.row.min(used.start.row + 5000) {
            for col in used.start.col..=used.end.col.min(used.start.col + 200) {
                let at = CellRef::new(r, col);
                let a1 = format!("{}{}", col_name(col), r + 1);
                let Some(from) = sh.spill_anchor(at).filter(|x| *x != at) else { continue };
                if !s.cells.contains_key(&a1) && !matches!(sh.value(at), Value::Empty) {
                    add(a1, String::new(), at, &format!("{}{}", col_name(from.col), from.row + 1), &mut cells);
                }
            }
        }
    }
    let (filtered, lists) = crate::sheetx::for_view(s, &cells, |n, r, k| Some(wb.sheet(wb.sheet_index(n)?)?.value(CellRef::new(r, k)).display()));
    // Links: a typed address, or HYPERLINK's first argument worked out on this sheet.
    let links = s.cells.iter().filter_map(|(a1, input)| {
        let url = match input.strip_prefix('=') {
            Some(body) if body.trim_start().to_ascii_uppercase().starts_with("HYPERLINK") => match parse(body) {
                Some(Expr::Call(name, args)) if name == "HYPERLINK" => gridcraft_calc::recalc::evaluate_expr(&wb, k + BUILT_IN, CellRef::parse(a1)?, args.first()?).display(),
                _ => return None,
            },
            Some(_) => return None,
            None => input.clone(),
        };
        crate::sheetx::link(&url).map(|u| (a1.clone(), u))
    }).take(20_000).collect();
    // Cells that break their validation (only cells with something in them; blanks pass).
    let mut invalid = vec![];
    for (i, v) in s.valid.iter().enumerate() {
        let Some((r0, c0, r1, c1)) = crate::model::sheet_range(&v.range) else { continue };
        for x in cells.iter().filter(|x| !x.2.is_empty()) {
            let Some(at) = CellRef::parse(&x.0).filter(|a| a.row >= r0 && a.row <= r1 && a.col >= c0 && a.col <= c1) else { continue };
            if invalid.len() < 5_000 && !invalid.contains(&x.0) && !passes(c, &wb, k, i, at, (if x.1.starts_with('=') || x.1.is_empty() { &x.2 } else { &x.1 }, x.4), &lists) { invalid.push(x.0.clone()) }
        }
    }
    Some(SheetOut { name: s.name.clone(), rows, cols, cells, filtered, lists, links, invalid })
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
    rewrite(input, |e| gridcraft_formula::adjust::shift_relative(e, dr, dc))
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

fn rewrite(input: &str, f: impl FnOnce(Expr) -> Expr) -> String {
    match input.strip_prefix('=').and_then(parse) {
        Some(e) => format!("={}", gridcraft_formula::printer::print(&f(e))),
        None => input.into(),
    }
}

/// A sheet formula as a workbook reads it: task-sheet column names (`Tasks[Total Slack]`) become the
/// Tasks table's own (`Tasks[Total Float]`).
pub fn excel_formula(c: &Calc, input: &str) -> String {
    if !input.starts_with('=') { return input.into() }
    grid_names(c, &task_columns(c), input)
}
