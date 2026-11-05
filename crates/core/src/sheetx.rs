//! Sheets as lists: the AutoFilter (which rows a filter hides), data validation (the values a cell
//! may take), links in cells and defined names. The window shows the result; what a cell shows comes
//! from `sheet::sheet_view`.
use crate::model::{sheet_range, ColFilter, DefName, SheetData, Validation};
use gridcraft_core::{CellRef, Value};
use std::collections::{BTreeSet, HashMap};

/// The conditions a filtered column can have (besides a list of values).
pub const FILTER_OPS: [&str; 17] = ["eq", "ne", "gt", "ge", "lt", "le", "between", "contains", "notcontains", "begins", "ends", "blank", "notblank", "top", "bottom", "above", "below"];

/// A typed value as a number: 12, 1,200, 12%, a date (as its serial).
fn num(x: &str) -> Option<f64> {
    match gridcraft_core::parse::parse_input(x.trim(), gridcraft_core::DateSystem::default()).value { Value::Number(n) => Some(n), _ => None }
}

/// Whether a cell (what it shows and its number) passes a column's filter; `col` holds the column's
/// numbers for top/bottom and the average.
pub fn passes(f: &ColFilter, text: &str, n: Option<f64>, col: &[f64]) -> bool {
    let t = text.to_lowercase();
    if let Some(v) = &f.values { if !v.iter().any(|x| x.to_lowercase() == t) { return false } }
    let a = f.a.to_lowercase();
    let cmp = |x: &str| -> Option<std::cmp::Ordering> { Some(n?.total_cmp(&num(x)?)) };
    let avg = || col.iter().sum::<f64>() / col.len().max(1) as f64;
    let nth = |hi: bool| { let mut s = col.to_vec(); s.sort_by(|x, y| if hi { y.total_cmp(x) } else { x.total_cmp(y) }); let k = num(&f.a).unwrap_or(10.0).max(1.0) as usize; s.get(k.min(s.len()).saturating_sub(1)).copied() };
    use std::cmp::Ordering::*;
    match f.op.as_str() {
        "eq" => cmp(&f.a).map_or(t == a, |o| o == Equal),
        "ne" => cmp(&f.a).map_or(t != a, |o| o != Equal),
        "gt" => cmp(&f.a) == Some(Greater),
        "ge" => matches!(cmp(&f.a), Some(Greater | Equal)),
        "lt" => cmp(&f.a) == Some(Less),
        "le" => matches!(cmp(&f.a), Some(Less | Equal)),
        "between" => matches!(cmp(&f.a), Some(Greater | Equal)) && matches!(cmp(&f.b), Some(Less | Equal)),
        "contains" => t.contains(&a),
        "notcontains" => !t.contains(&a),
        "begins" => t.starts_with(&a),
        "ends" => t.ends_with(&a),
        "blank" => text.trim().is_empty(),
        "notblank" => !text.trim().is_empty(),
        "top" => n.zip(nth(true)).is_some_and(|(x, k)| x >= k),
        "bottom" => n.zip(nth(false)).is_some_and(|(x, k)| x <= k),
        "above" => n.is_some_and(|x| x > avg()),
        "below" => n.is_some_and(|x| x < avg()),
        _ => true,
    }
}

/// The rows (0-based) the sheet's AutoFilter hides, given what a cell (row, column) shows and its number.
pub fn filtered(s: &SheetData, shown: impl Fn(u32, u32) -> (String, Option<f64>)) -> Vec<u32> {
    let Some(f) = &s.filter else { return vec![] };
    let Some((r0, _, r1, _)) = sheet_range(&f.range) else { return vec![] };
    let mut out = BTreeSet::new();
    for cf in &f.cols {
        let vals: Vec<(u32, String, Option<f64>)> = (r0 + 1..=r1.min(r0 + 100_000)).map(|r| { let (t, n) = shown(r, cf.col); (r, t, n) }).collect();
        let col: Vec<f64> = vals.iter().filter_map(|v| v.2).collect();
        out.extend(vals.iter().filter(|(_, t, n)| !passes(cf, t, *n, &col)).map(|v| v.0));
    }
    out.into_iter().collect()
}

/// The values of each drop-down list on the sheet (in the order of `s.valid`): typed, or read from
/// the cells named by `src` (on this sheet, `shown`; on another, through `other(sheet, row, col)`).
pub fn lists(s: &SheetData, shown: impl Fn(u32, u32) -> String, other: impl Fn(&str, u32, u32) -> Option<String>) -> Vec<Vec<String>> {
    s.valid.iter().map(|v| {
        if v.src.trim().is_empty() { return v.list.clone() }
        let src = v.src.trim().trim_start_matches('=').replace('$', "");
        let (sheet, rng) = match src.rsplit_once('!') { Some((a, b)) => (Some(a.trim_matches('\'').to_string()), b.to_string()), None => (None, src) };
        let Some((r0, c0, r1, c1)) = sheet_range(&rng) else { return vec![] };
        let mut seen = BTreeSet::new();
        let mut out = vec![];
        for r in r0..=r1.min(r0 + 2_000) {
            for c in c0..=c1.min(c0 + 50) {
                let t = match &sheet { Some(n) if !n.eq_ignore_ascii_case(&s.name) => other(n, r, c).unwrap_or_default(), _ => shown(r, c) };
                if !t.is_empty() && seen.insert(t.clone()) { out.push(t) }
            }
        }
        out
    }).collect()
}

/// The filter and the lists of a sheet as shown, from the cells `sheet_view` lists.
pub fn for_view(s: &SheetData, cells: &[(String, String, String, bool, Option<f64>)], other: impl Fn(&str, u32, u32) -> Option<String>) -> (Vec<u32>, Vec<Vec<String>>) {
    if s.filter.is_none() && s.valid.is_empty() { return (vec![], vec![]) }
    let by: HashMap<(u32, u32), (&str, Option<f64>)> = cells.iter().filter_map(|x| CellRef::parse(&x.0).map(|r| ((r.row, r.col), (x.2.as_str(), x.4)))).collect();
    let get = |r: u32, c: u32| by.get(&(r, c)).map_or((String::new(), None), |x| (x.0.to_string(), x.1));
    (filtered(s, get), lists(s, |r, c| get(r, c).0, other))
}

/// What validated cells can take besides a list: whole numbers, decimals, dates, times, a text length,
/// values for which a formula is TRUE, anything (a message only).
pub const VALID_KINDS: [&str; 7] = ["whole", "decimal", "date", "time", "len", "custom", "any"];
/// How a validated value compares with `a` (and `b`).
pub const VALID_OPS: [&str; 8] = ["between", "notbetween", "eq", "ne", "gt", "lt", "ge", "le"];

/// Whether `n` passes the comparison `op` with `a` and `b` (a bound that could not be read fails).
pub fn compare(op: &str, n: f64, a: Option<f64>, b: Option<f64>) -> bool {
    let Some(a) = a else { return false };
    match op {
        "" | "between" => b.is_some_and(|b| n >= a.min(b) && n <= a.max(b)),
        "notbetween" => b.is_some_and(|b| n < a.min(b) || n > a.max(b)),
        "eq" => n == a, "ne" => n != a, "gt" => n > a, "lt" => n < a, "ge" => n >= a, "le" => n <= a,
        _ => true,
    }
}

/// Whether a value passes a validation: `text` as typed or shown, `n` its number, `lo` and `hi` the
/// bounds worked out (for a custom rule `lo` is its formula's result: TRUE is 1), `list` the drop-down's
/// values. Blank cells always pass.
pub fn valid(v: &Validation, text: &str, n: Option<f64>, lo: Option<f64>, hi: Option<f64>, list: &[String]) -> bool {
    if text.trim().is_empty() { return true }
    match v.kind.as_str() {
        "" => list.iter().any(|x| x.eq_ignore_ascii_case(text.trim())),
        "any" => true,
        "custom" => lo.is_some_and(|x| x != 0.0),
        "len" => compare(&v.op, text.chars().count() as f64, lo, hi),
        k => n.is_some_and(|n| (k != "whole" || n.fract() == 0.0) && (k != "time" || (0.0..1.0).contains(&n)) && compare(&v.op, n, lo, hi)),
    }
}

/// The message when a value is refused: the validation's own, or what the cell takes.
pub fn message(v: &Validation) -> String {
    if !v.error.trim().is_empty() { return v.error.trim().to_string() }
    if v.kind == "custom" { return format!("This cell takes a value for which {} is TRUE.", v.a.trim().trim_start_matches('=')) }
    let what = match v.kind.as_str() { "" => return "This cell takes a value from its list.".into(), "whole" => "a whole number", "decimal" => "a number", "date" => "a date", "time" => "a time", "len" => "text whose length is", _ => "a value" };
    let (a, b) = (v.a.trim().trim_start_matches('='), v.b.trim().trim_start_matches('='));
    let how = match v.op.as_str() { "notbetween" => format!("not between {a} and {b}"), "eq" => format!("equal to {a}"), "ne" => format!("not equal to {a}"), "gt" => format!("greater than {a}"), "lt" => format!("less than {a}"), "ge" => format!("at least {a}"), "le" => format!("at most {a}"), _ => format!("between {a} and {b}") };
    let how = if v.kind == "len" { how } else { format!(" {how}") };
    format!("This cell takes {what}{}{how}.", if v.kind == "len" { " " } else { "" })
}

/// A link a cell may open: an http, https or mailto address, nothing else (no file:, no scripts), with
/// no spaces, quotes or control characters, at most 2,000 characters.
pub fn link(text: &str) -> Option<String> {
    let t = text.trim();
    let low = t.to_ascii_lowercase();
    let ok = ["http://", "https://", "mailto:"].iter().any(|p| low.starts_with(p) && t.len() > p.len() + 1);
    (ok && t.len() <= 2_000 && !t.chars().any(|c| c.is_whitespace() || c.is_control() || "\"<>`\\^{}|".contains(c))).then(|| t.to_string())
}

/// The plan's own names, which a defined name may not take.
pub const PLAN_NAMES: [&str; 23] = ["ProjectName", "ProjectStart", "ProjectFinish", "StatusDate", "BaselineFinish", "FinishVariance", "PercentComplete", "PlannedPercent", "TotalCost", "PlannedValue", "EarnedValue",
    "ActualCost", "SPI", "CPI", "TaskCount", "TasksDone", "TasksLate", "CriticalCount", "MilestoneCount", "Tasks", "Resources", "Plan", "DatumPlan"];

/// Why a defined name cannot be used (None: it can): it starts with a letter, _ or \, holds only
/// letters, digits, _ and ., is not a cell address (B12, TAX2026, R1C1) nor one of the plan's names.
pub fn name_problem(name: &str) -> Option<String> {
    let n = name.trim();
    let mut ch = n.chars();
    if n.is_empty() { return Some("Type a name.".into()) }
    if n.chars().count() > 255 { return Some("A name has at most 255 characters.".into()) }
    if !ch.next().is_some_and(|c| c.is_alphabetic() || c == '_' || c == '\\') || !ch.all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '\\') {
        return Some(format!("{n} is not a valid name: start with a letter or _, then letters, digits, _ or . (no spaces)."));
    }
    let up = n.to_ascii_uppercase();
    let rc = |s: &str| s.is_empty() || s.chars().all(|c| c.is_ascii_digit());
    let r1c1 = up.strip_prefix('R').is_some_and(|x| x.split_once('C').map_or(rc(x), |(a, b)| rc(a) && rc(b))) || up.strip_prefix('C').is_some_and(rc);
    if gridcraft_core::CellRef::parse(&up).is_some() || r1c1 || up == "TRUE" || up == "FALSE" { return Some(format!("{n} is a cell address; choose another name.")) }
    if PLAN_NAMES.iter().any(|x| x.eq_ignore_ascii_case(n)) { return Some(format!("{n} is one of the plan's own names; choose another.")) }
    None
}

/// The names a workbook brings, added to the plan's: valid ones only, none the plan already has in the
/// same scope, and names local to a sheet only when that sheet is one of `sheets` (the tabs it brings
/// under their own names).
pub fn merge_names(have: &[DefName], new: Vec<DefName>, sheets: &[String]) -> Vec<DefName> {
    let mut out = have.to_vec();
    for n in new {
        let scope = n.sheet.is_empty() || sheets.iter().any(|s| s.eq_ignore_ascii_case(&n.sheet));
        if scope && name_problem(&n.name).is_none() && !out.iter().any(|x| x.name.eq_ignore_ascii_case(&n.name) && x.sheet.eq_ignore_ascii_case(&n.sheet)) && out.len() < 1_000 { out.push(n) }
    }
    out
}

/// Calls `f(range, formula)` for each reference or formula a sheet's rules hold, and keeps what it
/// returns: validation bounds and custom formulas (`range` is the rule's), list sources (`range` None:
/// a list reads the same cells for every cell) and conditional formatting values that start with "=".
/// A bound typed without "=" ("$B$1", "Lists!A1") stays without it.
pub fn rule_formulas(s: &mut SheetData, f: &mut dyn FnMut(Option<&str>, &str) -> String) {
    let mut one = |x: &mut String, range: Option<&str>, loose: bool| {
        let t = x.trim();
        let eq = t.starts_with('=');
        if t.len() < 2 || !(eq || loose && (t.contains('!') || sheet_range(&t.replace('$', "")).is_some())) { return }
        let out = f(range, &format!("={}", t.trim_start_matches('=')));
        *x = if eq { out } else { out.trim_start_matches('=').to_string() };
    };
    for v in &mut s.valid {
        let custom = v.kind == "custom";
        if custom && !v.a.trim().starts_with('=') { v.a.insert(0, '=') }
        one(&mut v.a, Some(&v.range), true);
        one(&mut v.b, Some(&v.range), true);
        one(&mut v.src, None, true);
    }
    for c in s.cond.iter_mut().filter(|c| !matches!(c.op.as_str(), "bar" | "scale" | "date" | "top" | "bottom")) {
        one(&mut c.a, Some(&c.range), false);
        one(&mut c.b, Some(&c.range), false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SheetFilter, Validation};
    fn sheet() -> SheetData {
        let cells = [("A1", "Vendor"), ("B1", "Country"), ("C1", "Value"), ("A2", "Steel Co"), ("B2", "Germany"), ("C2", "100"), ("A3", "Pipe Ltd"), ("B3", "Italy"), ("C3", "250"),
            ("A4", "Valve AG"), ("B4", "Germany"), ("C4", "40"), ("A5", "Pumps Inc"), ("B5", ""), ("C5", "900")];
        SheetData { name: "Vendors".into(), cells: cells.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), ..Default::default() }
    }
    fn shown(s: &SheetData) -> impl Fn(u32, u32) -> (String, Option<f64>) + '_ {
        move |r, c| { let t = s.cells.get(&format!("{}{}", crate::sheet::col_name(c), r + 1)).cloned().unwrap_or_default(); let n = t.parse().ok(); (t, n) }
    }
    fn with(cols: Vec<ColFilter>) -> SheetData { SheetData { filter: Some(SheetFilter { range: "A1:C5".into(), cols }), ..sheet() } }
    fn hid(cols: Vec<ColFilter>) -> Vec<u32> { let s = with(cols); filtered(&s, shown(&s)) }

    #[test]
    fn autofilter_hides_the_rows_that_do_not_pass() {
        let f = |col: u32, op: &str, a: &str| ColFilter { col, op: op.into(), a: a.into(), ..Default::default() };
        assert_eq!(hid(vec![]), Vec::<u32>::new());
        // Values: Germany only (case does not matter); blanks are "".
        assert_eq!(hid(vec![ColFilter { col: 1, values: Some(vec!["germany".into()]), ..Default::default() }]), vec![2, 4]);
        assert_eq!(hid(vec![ColFilter { col: 1, values: Some(vec!["".into(), "Italy".into()]), ..Default::default() }]), vec![1, 3]);
        assert_eq!(hid(vec![f(2, "gt", "100")]), vec![1, 3]);
        assert_eq!(hid(vec![f(2, "between", "50")]), vec![1, 2, 3, 4], "no upper bound passes nothing");
        assert_eq!(hid(vec![ColFilter { b: "300".into(), ..f(2, "between", "50") }]), vec![3, 4]);
        assert_eq!(hid(vec![f(0, "contains", "co")]), vec![2, 3, 4]);
        assert_eq!(hid(vec![f(0, "begins", "p")]), vec![1, 3]);
        assert_eq!(hid(vec![f(1, "blank", "")]), vec![1, 2, 3]);
        assert_eq!(hid(vec![f(2, "top", "2")]), vec![1, 3]);
        assert_eq!(hid(vec![f(2, "bottom", "1")]), vec![1, 2, 4]);
        assert_eq!(hid(vec![f(2, "above", "")]), vec![1, 2, 3], "average 322.5");
        // Two columns: both must pass.
        assert_eq!(hid(vec![f(1, "eq", "Germany"), f(2, "lt", "50")]), vec![1, 2, 4]);
    }

    #[test]
    fn validation_compares_numbers_dates_times_and_lengths() {
        let v = |kind: &str, op: &str| Validation { kind: kind.into(), op: op.into(), a: "1".into(), b: "10".into(), ..Default::default() };
        let ok = |x: &Validation, t: &str, n: Option<f64>| valid(x, t, n, x.a.parse().ok(), x.b.parse().ok(), &[]);
        assert!(ok(&v("whole", ""), "5", Some(5.0)) && !ok(&v("whole", ""), "5.5", Some(5.5)) && !ok(&v("whole", "between"), "11", Some(11.0)));
        assert!(ok(&v("decimal", "notbetween"), "0.5", Some(0.5)) && !ok(&v("decimal", "notbetween"), "3", Some(3.0)));
        assert!(ok(&v("decimal", "gt"), "2", Some(2.0)) && !ok(&v("decimal", "lt"), "2", Some(2.0)) && ok(&v("decimal", "le"), "1", Some(1.0)) && ok(&v("decimal", "ne"), "2", Some(2.0)));
        assert!(!ok(&v("decimal", ""), "abc", None), "text is not a number");
        assert!(ok(&v("whole", ""), "", None), "a blank cell passes");
        assert!(!ok(&v("len", "le"), "abc", None) && ok(&v("len", "le"), "a", None) && !ok(&v("len", "lt"), "a", None) && ok(&v("len", "eq"), "a", None) && ok(&v("len", ""), "abcde", None));
        let t = Validation { kind: "time".into(), op: "ge".into(), a: "0.5".into(), ..Default::default() };
        assert!(ok(&t, "13:00", Some(13.0 / 24.0)) && !ok(&t, "09:00", Some(0.375)) && !ok(&t, "2", Some(2.0)), "a time is part of a day");
        assert!(!valid(&v("whole", ""), "5", Some(5.0), None, Some(10.0), &[]), "a bound that cannot be read fails");
        let l = Validation { list: vec!["Open".into()], ..Default::default() };
        assert!(valid(&l, "open", None, None, None, &["Open".into()]) && !valid(&l, "Shut", None, None, None, &["Open".into()]));
        assert_eq!(message(&v("whole", "")), "This cell takes a whole number between 1 and 10.");
        assert_eq!(message(&Validation { kind: "len".into(), op: "le".into(), a: "=$B$1".into(), ..Default::default() }), "This cell takes text whose length is at most $B$1.");
        assert_eq!(message(&Validation { error: "Up to 10 only".into(), ..v("whole", "") }), "Up to 10 only");
    }

    #[test]
    fn only_web_and_mail_links_open() {
        assert_eq!(link(" https://example.com/a?b=1#c "), Some("https://example.com/a?b=1#c".into()));
        assert!(link("HTTP://x.org").is_some() && link("mailto:a@b.com").is_some());
        for bad in ["file:///C:/x.exe", "javascript:alert(1)", "https://", "http://a b", "https://x\"onmouseover", "ftp://x.org", "\\\\server\\share", "C:\\x.exe", "data:text/html,x", "https://x\u{0}y", "www.example.com"] {
            assert_eq!(link(bad), None, "{bad}");
        }
    }

    #[test]
    fn defined_names_are_checked() {
        for ok in ["Rates", "VAT", "_x", "Tax.Rate", "Rate2026Q1", "ABCD1", "Ständig"] { assert_eq!(name_problem(ok), None, "{ok}"); }
        for bad in ["", "2x", "A1", "tax2026", "XFD9", "R1C1", "r", "C", "R2C", "My name", "a-b", "TRUE", "ProjectFinish", "tasks", "SPI"] { assert!(name_problem(bad).is_some(), "{bad}"); }
        let have = vec![DefName { name: "VAT".into(), formula: "=0.05".into(), ..Default::default() }];
        let m = merge_names(&have, vec![DefName { name: "vat".into(), formula: "=1".into(), ..Default::default() }, DefName { name: "A1".into(), formula: "=1".into(), ..Default::default() }, DefName { name: "Rate".into(), formula: "=2".into(), ..Default::default() },
            DefName { name: "VAT".into(), formula: "=0.2".into(), sheet: "Prices".into(), ..Default::default() }, DefName { name: "Fx".into(), formula: "=1".into(), sheet: "Gone".into(), ..Default::default() }], &["prices".into()]);
        assert_eq!(m.iter().map(|n| (n.name.as_str(), n.sheet.as_str())).collect::<Vec<_>>(), vec![("VAT", ""), ("Rate", ""), ("VAT", "Prices")], "a sheet's own VAT joins the plan's; a name of a sheet not brought in does not");
        assert_eq!(message(&Validation { kind: "custom".into(), a: "=AND(A2>0,A2<=B2)".into(), ..Default::default() }), "This cell takes a value for which AND(A2>0,A2<=B2) is TRUE.");
        assert!(valid(&Validation { kind: "custom".into(), ..Default::default() }, "5", Some(5.0), Some(1.0), None, &[]) && !valid(&Validation { kind: "custom".into(), ..Default::default() }, "5", Some(5.0), Some(0.0), None, &[]));
    }

    #[test]
    fn drop_down_lists_read_typed_values_or_cells() {
        let s = SheetData { valid: vec![Validation { range: "D2:D9".into(), list: vec!["Open".into(), "Closed".into()], ..Default::default() }, Validation { range: "E2:E9".into(), src: "=$B$2:$B$5".into(), ..Default::default() },
            Validation { range: "F2".into(), src: "Lists!A1:A3".into(), ..Default::default() }], ..sheet() };
        let l = lists(&s, |r, c| shown(&s)(r, c).0, |n, r, _| (n == "Lists").then(|| format!("L{r}")));
        assert_eq!(l, vec![vec!["Open".to_string(), "Closed".into()], vec!["Germany".into(), "Italy".into()], vec!["L0".into(), "L1".into(), "L2".into()]]);
    }
}
