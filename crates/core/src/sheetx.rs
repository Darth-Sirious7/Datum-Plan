//! Sheets as lists: the AutoFilter (which rows a filter hides) and drop-down lists (the values a
//! cell may take). The window shows the result; what a cell shows comes from `sheet::sheet_view`.
use crate::model::{sheet_range, ColFilter, SheetData};
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
    fn drop_down_lists_read_typed_values_or_cells() {
        let s = SheetData { valid: vec![Validation { range: "D2:D9".into(), list: vec!["Open".into(), "Closed".into()], ..Default::default() }, Validation { range: "E2:E9".into(), src: "=$B$2:$B$5".into(), ..Default::default() },
            Validation { range: "F2".into(), src: "Lists!A1:A3".into(), ..Default::default() }], ..sheet() };
        let l = lists(&s, |r, c| shown(&s)(r, c).0, |n, r, _| (n == "Lists").then(|| format!("L{r}")));
        assert_eq!(l, vec![vec!["Open".to_string(), "Closed".into()], vec!["Germany".into(), "Italy".into()], vec!["L0".into(), "L1".into(), "L2".into()]]);
    }
}
