//! File exchange: project XML, XER and PMXML, Excel (.xlsx), CSV, MPX, JSON, HTML and iCalendar.
pub mod sheetbook;
pub mod formats;
pub mod mspdi;
pub mod dplot;
pub mod table;
pub mod xer;
pub(crate) mod xml;

use crate::{fields::Calc, load, model::Project, sched};
use chrono::NaiveDate;
use std::path::Path;

pub enum Imported {
    /// A plan and the import report: what was changed or left out, and how its dates compare.
    Plan(Box<Project>, Vec<String>),
    /// A spreadsheet that needs its columns mapped first.
    Table(table::Table),
}

/// Tasks held by links to other plans, through their own links or a summary's above them.
pub(crate) fn ext_held(p: &Project) -> Vec<bool> {
    let mut stack: Vec<(u8, bool)> = vec![];
    p.tasks.iter().map(|t| {
        stack.retain(|x| x.0 < t.level);
        let h = !t.ext.is_empty() || stack.last().is_some_and(|x| x.1);
        stack.push((t.level, h));
        h
    }).collect()
}

/// The duration, constraint type and date a task is written with. A file cannot hold links to
/// other plans, so a task they hold (`ext`, from [`ext_held`]) keeps its scheduled start as
/// Start No Earlier Than (a milestone at the end of a day: Finish No Earlier Than), and a task that
/// mirrors one in another plan keeps its scheduled length.
pub(crate) fn held(p: &Project, s: &sched::Sched, i: usize, ext: bool) -> (i64, crate::model::Constraint, Option<NaiveDate>) {
    use crate::model::Constraint::{ASAP, FNET, SNET};
    let (t, c) = (&p.tasks[i], &s.cals[s.ci[i]]);
    let dur = if s.mirror[i] { c.between(s.es[i], s.ef[i]) } else { t.dur };
    match t.constraint {
        ASAP | SNET if ext && t.act_start.is_none() && !s.summary[i] => (dur, if dur == 0 && c.next_start(s.es[i]) != s.es[i] { FNET } else { SNET }, Some(crate::cal::date_of(s.es[i]))),
        k => (dur, k, t.cdate),
    }
}

/// A link's lag in working minutes, for formats without percent or elapsed lags (XER, PMXML): a
/// percent lag is that share of the predecessor's duration `pdur`; an elapsed lag the same share of
/// the project calendar's working week (7 elapsed days are 5 working days on a five-day week).
pub(crate) fn work_lag(p: &Project, l: &crate::model::Link, pdur: i64) -> i64 {
    if l.pct { return pdur * l.lag / 100 }
    if !l.elapsed { return l.lag }
    let week: i64 = p.calendars.iter().find(|c| c.name == p.calendar).map_or(2400, |c| c.week.iter().flatten().map(|&(a, b)| b.saturating_sub(a) as i64).sum::<i64>()).max(1);
    (l.lag as i128 * week as i128 / (7 * 1440)) as i64
}

/// What a format cannot hold of the plan, said after an export.
pub fn export_notes(p: &Project, path: &Path) -> Vec<String> {
    let n = p.tasks.iter().flat_map(|t| &t.preds).filter(|l| l.elapsed && l.lag != 0).count();
    match ext(path).as_str() {
        "xer" | "pmxml" if n > 0 => vec![format!("{n} elapsed lags were written as working time of the same share of a week (7 elapsed days are 5 working days on a five-day week): the format has no elapsed lags.")],
        _ => vec![],
    }
}

fn ext(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

pub fn import(path: &Path) -> Result<Imported, String> {
    let read = || crate::doc::read_limited(path);
    match ext(path).as_str() {
        "xml" | "xer" => {
            let text = read()?;
            let xml = ext(path) == "xml";
            let (p, mut notes) = if xml { mspdi::read_notes(&text)? } else { xer::read_notes(&text)? };
            notes.extend(compare_dates(&p, &stated_dates(&text, xml)));
            Ok(Imported::Plan(Box::new(p), notes))
        }
        "json" | "dplan" => Project::from_json(&read()?).map(|p| Imported::Plan(Box::new(p), vec![])),
        "csv" | "txt" | "xlsx" | "xlsm" | "xls" | "ods" => {
            let len = std::fs::metadata(path).map_err(|e| format!("Could not open {}: {e}", path.display()))?.len();
            if len > crate::doc::MAX_FILE { return Err(format!("{} is too large to import.", path.display())) }
            table::read(path).map(Imported::Table)
        }
        "mpp" => Err(".mpp files are a closed format. Save the plan as XML in the program that made it, then import the XML file.".into()),
        e => Err(format!("Datum Plan cannot import .{e} files. Use XML, XER, XLSX, CSV or JSON.")),
    }
}

/// Task uid → (start, finish) as a Project XML or XER file states them, "YYYY-MM-DD HH:MM".
pub(crate) fn stated_dates(text: &str, is_xml: bool) -> std::collections::HashMap<u32, (String, String)> {
    let cut = |s: &str| s.replace('T', " ").chars().take(16).collect::<String>();
    if is_xml {
        let Ok(root) = xml::parse(text) else { return Default::default() };
        return root.get("Tasks").into_iter().flat_map(|t| t.all("Task"))
            .filter(|t| t.val("OutlineLevel") != "0" && t.val("Start").len() >= 16)
            .filter_map(|t| Some((t.val("UID").parse().ok()?, (cut(t.val("Start")), cut(t.val("Finish"))))))
            .collect();
    }
    let (mut fields, mut table, mut out) = (vec![], "", std::collections::HashMap::new());
    for line in text.lines() {
        let v: Vec<&str> = line.split('\t').collect();
        match v[0] {
            "%T" => table = v.get(1).copied().unwrap_or(""),
            "%F" => fields = v[1..].to_vec(),
            "%R" if table == "TASK" => {
                let f = |k: &str| fields.iter().position(|x| *x == k).and_then(|i| v.get(i + 1)).map_or(String::new(), |s| cut(s));
                if let (Ok(uid), true) = (f("task_id").parse(), f("early_start_date").len() >= 16) { out.insert(uid, (f("early_start_date"), f("early_end_date"))); }
            }
            _ => {}
        }
    }
    out
}

/// Datum Plan's start and finish against the file's, as notes for the import report.
fn compare_dates(p: &Project, want: &std::collections::HashMap<u32, (String, String)>) -> Vec<String> {
    let Ok(s) = sched::schedule(p) else { return vec!["The plan does not schedule yet (see the message in the window).".into()] };
    let fmt = |t: i64| crate::cal::date_of(t).format("%Y-%m-%d").to_string() + &format!(" {:02}:{:02}", t.rem_euclid(1440) / 60, t.rem_euclid(60));
    let (mut n, mut diff) = (0, vec![]);
    for (i, t) in p.tasks.iter().enumerate() {
        let Some((ws, wf)) = want.get(&t.uid) else { continue };
        n += 1;
        let (gs, gf) = (fmt(s.es[i]), fmt(s.ef[i]));
        if (&gs, &gf) != (ws, wf) { diff.push(format!("\"{}\": file {ws} → {wf}, Datum Plan {gs} → {gf}", t.name)) }
    }
    match (n, diff.len()) {
        (0, _) => vec![],
        (n, 0) => vec![format!("All {n} tasks get the same start and finish as in the file.")],
        (n, d) => {
            let mut v = vec![format!("{d} of {n} tasks get different dates in Datum Plan than in the file. Calendars, progress or scheduling options that work differently are the usual reasons. The first ones:")];
            v.extend(diff.into_iter().take(5).map(|x| format!("  {x}")));
            v
        }
    }
}

/// Writes the plan to `path` in the format its extension names. `cols` chooses the columns of
/// table-like formats (CSV, Excel, JSON, HTML); empty means the standard set.
pub fn export(p: &Project, path: &Path, fmt: &str, today: NaiveDate, cols: &[String]) -> Result<(), String> {
    let s = sched::schedule(p)?;
    let write = |text: String| crate::doc::write_atomic(path, text.as_bytes());
    let std_cols: Vec<String> = table::OUT.iter().map(|c| c.to_string()).collect();
    let cols = if cols.is_empty() { &std_cols[..] } else { cols };
    let calc = || Calc::new(p, fmt, today);
    match ext(path).as_str() {
        "xml" => write(mspdi::write(p, &s, &load::compute(p, &s).cost)),
        "xer" => write(xer::write(p, &s, today)),
        "csv" => table::write_csv(&calc(), cols, path),
        "xlsx" => table::write_xlsx(&calc(), cols, path),
        "json" => write(formats::json(&calc(), cols)),
        "html" | "htm" => write(formats::html(&calc(), cols)),
        "ics" => write(formats::ics(&calc())),
        "mpx" => write(formats::mpx(&calc())),
        "pmxml" => write(formats::pmxml(&calc())),
        e => Err(format!("Datum Plan cannot export .{e} files.")),
    }
}
