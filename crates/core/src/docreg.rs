//! The Document Register module: vendor documents with their class, a transmittal log of every
//! submission and the client's return code, planned dates from the P date and review cycles in
//! working days (each on the plan's calendar or one chosen for it), status, client delay, earned
//! progress by weighted steps and the weekly and monthly S-curves.
use crate::{cal::{date_of, WorkCal, DAY}, fields::Calc, model::*};
use chrono::{Datelike, Days, NaiveDate};
use serde::Serialize;
use std::collections::HashMap;

/// The six progress steps of a document's cycle.
pub const STEPS: [&str; 6] = ["First issue submitted", "First issue accepted", "Approval issue submitted", "Approval issue accepted", "Construction issue submitted", "Final issue submitted"];

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct DocRow {
    pub id: u32,
    pub planned: [Option<NaiveDate>; 6],
    pub actual: [Option<NaiveDate>; 6],
    pub forecast: [Option<NaiveDate>; 6],
    pub subs: usize,
    /// Submissions the client sent back to be revised (codes that ask for a resubmission).
    pub cycles: usize,
    pub rev: String,
    pub purpose: String,
    pub sent: Option<NaiveDate>,
    pub due: Option<NaiveDate>,
    pub returned: Option<NaiveDate>,
    pub code: String,
    pub status: String,
    /// First issue: days after its planned date (to today while not sent).
    pub late1: i64,
    /// Days the client kept the document past the review period, over all submissions.
    pub client_late: i64,
    pub earned: f64,
    pub planned_pct: f64,
    pub task_name: String,
    /// The linked task's start: when the document is needed.
    pub need: Option<NaiveDate>,
    pub late_for_task: bool,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct SubRow {
    pub id: u32,
    pub due: Option<NaiveDate>,
    pub accepted: bool,
    /// Days with the client (to today while not returned).
    pub days: i64,
    /// Days past the review period.
    pub late: i64,
    /// "Returned", "In review", "Review overdue", "Not sent".
    pub state: String,
}

/// One point of an S-curve: period end, planned and actual cumulative % (actual stops at the data date),
/// first issues planned and done in the period.
#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct Point { pub end: NaiveDate, pub planned: f64, pub actual: Option<f64>, pub first_planned: usize, pub first_done: usize }

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct RegOut {
    pub data_date: Option<NaiveDate>,
    pub docs: Vec<DocRow>,
    pub subs: Vec<SubRow>,
    pub earned: f64,
    pub planned: f64,
    pub client_late: i64,
    pub weekly: Vec<Point>,
    pub monthly: Vec<Point>,
    pub weights_ok: [bool; 3],
}

fn add_wd(c: &WorkCal, d: NaiveDate, n: i64) -> NaiveDate {
    let (mut x, mut k) = (d, 0);
    for _ in 0..20_000 {
        if k >= n { break }
        x = x.succ_opt().unwrap_or(x);
        if c.is_work_day(x) { k += 1 }
    }
    x
}

pub fn status(c: &Calc) -> RegOut {
    let (p, r) = (c.p, &c.p.documents);
    let today = date_of(c.progress_at() - DAY);
    let review = WorkCal::new(p.cal((!r.review_cal.is_empty()).then_some(r.review_cal.as_str())));
    let work = WorkCal::new(p.cal((!r.work_cal.is_empty()).then_some(r.work_cal.as_str())));
    let (rv, tn) = (r.review_days.max(0), r.turnaround_days.max(0));
    let code = |s: &str| r.codes.iter().find(|x| x.code.eq_ignore_ascii_case(s.trim()));
    let accepted = |s: &str| code(s).is_some_and(|x| x.accepted);
    let resubmit = |s: &str| code(s).is_some_and(|x| x.resubmit);
    let p_date = r.p_date.unwrap_or(p.start);
    let unit = if r.p_unit == "days" { 1.0 } else { 7.0 };
    let mut by_doc: HashMap<u32, Vec<&Submission>> = HashMap::new();
    for s in &r.log { by_doc.entry(s.doc).or_default().push(s) }
    for v in by_doc.values_mut() { v.sort_by_key(|s| (s.sent, s.id)) }
    let mut o = RegOut { data_date: Some(today), ..Default::default() };
    for (k, w) in o.weights_ok.iter_mut().enumerate() { *w = (r.weights.iter().map(|s| s.get(k).copied().unwrap_or(0.0)).sum::<f64>() - 100.0).abs() < 0.01 }
    let (mut wsum, mut esum, mut psum) = (0.0, 0.0, 0.0);
    let mut eff: Vec<(f64, usize)> = vec![];
    for d in &r.docs {
        let subs = by_doc.get(&d.id).cloned().unwrap_or_default();
        let three = d.class == 3;
        let (first, appr, cons, fin) = (if three { &r.flow.construction } else { &r.flow.first }, &r.flow.approval, &r.flow.construction, &r.flow.final_);
        let of = |purpose: &str| subs.iter().filter(|s| s.purpose.eq_ignore_ascii_case(purpose)).copied().collect::<Vec<_>>();
        let sent = |v: &[&Submission]| v.iter().filter_map(|s| s.sent).min();
        let acc = |v: &[&Submission]| v.iter().filter(|s| accepted(&s.code)).filter_map(|s| s.returned).min();
        let (s1, s3, s5, s6) = (of(first), of(appr), of(cons), of(fin));
        let mut a = [sent(&s1), acc(&s1), sent(&s3), acc(&s3), sent(&s5), sent(&s6)];
        if three { a[2] = None; a[3] = None; a[4] = a[0]; }
        // Planned: the first issue from the P date (or a date of its own), then review and turnaround in working days.
        let mut pl = [None; 6];
        if let Some(f) = d.first.or_else(|| d.offset.and_then(|n| p_date.checked_add_days(Days::new((n.max(0.0) * unit).round() as u64)))) {
            pl[0] = Some(f);
            pl[1] = Some(add_wd(&review, f, rv));
            if three { pl[4] = Some(f) } else {
                pl[2] = Some(add_wd(&work, pl[1].unwrap_or(f), tn));
                pl[3] = Some(add_wd(&review, pl[2].unwrap_or(f), rv));
                pl[4] = Some(add_wd(&work, pl[3].unwrap_or(f), tn));
            }
            pl[5] = pl[4].map(|x| add_wd(&work, add_wd(&review, x, rv), tn));
        }
        // Forecast: actual, else the chain from the last thing done, never before today.
        let pending = |v: &[&Submission], f_sent: Option<NaiveDate>| -> Option<NaiveDate> {
            let last = v.last()?;
            if last.returned.is_some() && resubmit(&last.code) { return Some(add_wd(&review, add_wd(&work, last.returned?, tn).max(today), rv)) }
            if last.returned.is_none() { return Some(add_wd(&review, last.sent.or(f_sent)?, rv).max(today)) }
            None
        };
        let mut f = [None; 6];
        f[0] = a[0].or(pl[0].map(|x| x.max(today)));
        f[1] = a[1].or_else(|| pending(&s1, f[0])).or(f[0].map(|x| add_wd(&review, x, rv).max(today)));
        if !three {
            f[2] = a[2].or(f[1].map(|x| add_wd(&work, x, tn).max(today)));
            f[3] = a[3].or_else(|| pending(&s3, f[2])).or(f[2].map(|x| add_wd(&review, x, rv).max(today)));
            f[4] = a[4].or(f[3].map(|x| add_wd(&work, x, tn).max(today)));
        } else { f[4] = f[0] }
        f[5] = a[5].or(f[4].map(|x| add_wd(&work, add_wd(&review, x, rv), tn).max(today)));
        let last = subs.last();
        let due = last.and_then(|s| s.sent).map(|x| add_wd(&review, x, rv));
        let status = match last {
            None => if pl[0].is_some_and(|x| x < today) { "Not submitted – overdue" } else { "Not submitted" },
            Some(_) if a[5].is_some() => "Final issued",
            Some(s) if s.returned.is_none() => if due.is_some_and(|x| today > x) { "Review overdue" } else { "Under review" },
            Some(s) if resubmit(&s.code) => "Resubmit",
            _ => "Accepted – next issue",
        };
        let cls = (d.class.clamp(1, 3) - 1) as usize;
        let wt = |i: usize| r.weights.get(i).and_then(|s| s.get(cls)).copied().unwrap_or(0.0) / 100.0;
        let earned: f64 = (0..6).filter(|&i| a[i].is_some_and(|x| x <= today)).map(wt).sum();
        let planned: f64 = (0..6).filter(|&i| pl[i].is_some_and(|x| x <= today)).map(wt).sum();
        let mut row = DocRow {
            id: d.id, planned: pl, actual: a, forecast: f, subs: subs.len(), cycles: subs.iter().filter(|s| s.returned.is_some() && resubmit(&s.code)).count(),
            rev: last.map(|s| s.rev.clone()).unwrap_or_default(), purpose: last.map(|s| s.purpose.clone()).unwrap_or_default(),
            sent: last.and_then(|s| s.sent), due, returned: last.and_then(|s| s.returned), code: last.map(|s| s.code.clone()).unwrap_or_default(),
            status: status.into(), late1: pl[0].map(|x| (a[0].unwrap_or(today) - x).num_days().max(0)).unwrap_or(0), earned: earned * 100.0, planned_pct: planned * 100.0, ..Default::default()
        };
        for s in &subs {
            let due = s.sent.map(|x| add_wd(&review, x, rv));
            let end = s.returned.unwrap_or(today);
            let late = due.map(|x| (end - x).num_days().max(0)).unwrap_or(0);
            row.client_late += late;
            o.subs.push(SubRow { id: s.id, due, accepted: accepted(&s.code), days: s.sent.map(|x| (end - x).num_days().max(0)).unwrap_or(0), late,
                state: (if s.sent.is_none() { "Not sent" } else if s.returned.is_some() { "Returned" } else if due.is_some_and(|x| today > x) { "Review overdue" } else { "In review" }).into() });
        }
        if let Some(i) = (d.task > 0).then(|| p.idx(d.task)).flatten() {
            row.task_name = p.tasks[i].name.clone();
            row.need = Some(date_of(c.s.es[i]));
            row.late_for_task = f[4].zip(row.need).is_some_and(|(x, n)| x > n);
        }
        o.client_late += row.client_late;
        let w = if d.progress && !d.recurring { d.weight.max(0.0) } else { 0.0 };
        wsum += w; esum += w * earned; psum += w * planned;
        eff.push((w, cls));
        o.docs.push(row);
    }
    if wsum > 0.0 { o.earned = esum / wsum * 100.0; o.planned = psum / wsum * 100.0; }
    o.weekly = curve(r, &o.docs, &eff, wsum, today, |d| { let back = (d.weekday().num_days_from_monday() as i64 - p.periods.week_end.min(6) as i64).rem_euclid(7); d.checked_add_days(Days::new(((7 - back) % 7) as u64)).unwrap_or(d) });
    o.monthly = curve(r, &o.docs, &eff, wsum, today, |d| { let (y, m) = if d.month() == 12 { (d.year() + 1, 1) } else { (d.year(), d.month() + 1) }; NaiveDate::from_ymd_opt(y, m, 1).and_then(|x| x.pred_opt()).unwrap_or(d) });
    o
}

/// Cumulative planned and actual % at each period end, from the first planned date to the last.
fn curve(r: &DocRegister, docs: &[DocRow], eff: &[(f64, usize)], wsum: f64, today: NaiveDate, end_of: impl Fn(NaiveDate) -> NaiveDate) -> Vec<Point> {
    let all: Vec<NaiveDate> = docs.iter().flat_map(|d| d.planned.iter().chain(d.actual.iter()).flatten().copied()).collect();
    let (Some(a), Some(b)) = (all.iter().min(), all.iter().max()) else { return vec![] };
    let wt = |i: usize, cls: usize| r.weights.get(i).and_then(|s| s.get(cls)).copied().unwrap_or(0.0) / 100.0;
    let at = |e: NaiveDate, actual: bool| {
        if wsum <= 0.0 { return 0.0 }
        docs.iter().zip(eff).map(|(d, &(w, cls))| w * (0..6).filter(|&i| (if actual { d.actual[i] } else { d.planned[i] }).is_some_and(|x| x <= e)).map(|i| wt(i, cls)).sum::<f64>()).sum::<f64>() / wsum * 100.0
    };
    let (mut e, last, mut out, mut prev) = (end_of(*a), end_of(*b), vec![], a.pred_opt().unwrap_or(*a));
    while e <= last && out.len() < 1500 {
        let n = |v: &dyn Fn(&DocRow) -> Option<NaiveDate>| docs.iter().filter(|d| v(d).is_some_and(|x| x > prev && x <= e)).count();
        out.push(Point { end: e, planned: at(e, false), actual: (e <= end_of(today) || e <= today).then(|| at(e.min(today), true)), first_planned: n(&|d| d.planned[0]), first_done: n(&|d| d.actual[0]) });
        prev = e;
        e = end_of(e.checked_add_days(Days::new(1)).unwrap_or(e));
        if e == prev { break }
    }
    out
}

/// Tidies a register before it is stored: classes 1–3, finite weights, unique ids, short texts,
/// submissions only for documents in the register.
pub fn clean(r: &mut DocRegister) {
    let t = |s: &mut String, n: usize| { if s.chars().count() > n { *s = s.chars().take(n).collect() } };
    r.review_days = r.review_days.clamp(0, 365);
    r.turnaround_days = r.turnaround_days.clamp(0, 365);
    if r.weights.len() != 6 || r.weights.iter().any(|s| s.len() != 3) { r.weights = DocRegister::default().weights }
    for s in &mut r.weights { for v in s { if !v.is_finite() || *v < 0.0 { *v = 0.0 } } }
    r.docs.truncate(10_000);
    r.log.truncate(50_000);
    let mut seen = std::collections::HashSet::new();
    let mut next = r.docs.iter().map(|d| d.id).max().unwrap_or(0) + 1;
    for d in &mut r.docs {
        if d.id == 0 || !seen.insert(d.id) { d.id = next; next += 1; seen.insert(d.id); }
        d.class = d.class.clamp(1, 3);
        if !d.weight.is_finite() || d.weight < 0.0 { d.weight = 0.0 }
        if d.offset.is_some_and(|v| !v.is_finite() || !(-1000.0..=10_000.0).contains(&v)) { d.offset = None }
        for s in [&mut d.number, &mut d.title, &mut d.vendor, &mut d.package, &mut d.disc, &mut d.kind, &mut d.remarks] { t(s, 300) }
        d.flags.retain(|f| !f.is_empty()); d.flags.truncate(20);
    }
    let docs = seen;
    r.log.retain(|s| docs.contains(&s.doc));
    let mut seen = std::collections::HashSet::new();
    let mut next = r.log.iter().map(|s| s.id).max().unwrap_or(0) + 1;
    for s in &mut r.log {
        if s.id == 0 || !seen.insert(s.id) { s.id = next; next += 1; seen.insert(s.id); }
        for x in [&mut s.rev, &mut s.purpose, &mut s.transmittal, &mut s.ret_transmittal, &mut s.code, &mut s.crs] { t(x, 300) }
    }
}

/// The register and the transmittal log as an Excel workbook.
pub fn write_xlsx(p: &Project, o: &RegOut, path: &std::path::Path) -> Result<(), String> {
    use rust_xlsxwriter::{Format, Workbook};
    let r = &p.documents;
    let mut wb = Workbook::new();
    let head = Format::new().set_bold().set_background_color(0xEAF0F8);
    let dt = |d: Option<NaiveDate>| d.map(|x| x.format("%d-%b-%y").to_string()).unwrap_or_default();
    let e = |x: rust_xlsxwriter::XlsxError| x.to_string();
    let ws = wb.add_worksheet().set_name("Register").map_err(e)?;
    let mut cols: Vec<String> = ["No.", "Document No.", "Title", "Vendor", "Package", "Discipline", "Type", "Class", "Key", "Flags", "Weight", "In progress"].map(String::from).to_vec();
    cols.extend(STEPS.iter().map(|s| format!("Planned: {s}")));
    cols.extend(STEPS.iter().map(|s| format!("Actual: {s}")));
    cols.extend(["Submissions", "Latest rev", "Latest purpose", "Latest sent", "Review due", "Returned", "Code", "Status", "1st issue late (d)", "Client late (d)", "Earned %", "Planned %", "Forecast final issue", "Remarks"].map(String::from));
    for (j, h) in cols.iter().enumerate() { ws.write_with_format(0, j as u16, h, &head).map_err(e)?; }
    for (i, (d, x)) in r.docs.iter().zip(&o.docs).enumerate() {
        let mut v: Vec<String> = vec![(i + 1).to_string(), d.number.clone(), d.title.clone(), d.vendor.clone(), d.package.clone(), d.disc.clone(), d.kind.clone(), d.class.to_string(), if d.key { "Yes".into() } else { String::new() }, d.flags.join(", "), d.weight.to_string(), if d.progress && !d.recurring { "Yes".into() } else { "No".into() }];
        v.extend(x.planned.iter().map(|&q| dt(q)));
        v.extend(x.actual.iter().map(|&q| dt(q)));
        v.extend([x.subs.to_string(), x.rev.clone(), x.purpose.clone(), dt(x.sent), dt(x.due), dt(x.returned), x.code.clone(), x.status.clone(), x.late1.to_string(), x.client_late.to_string(), format!("{:.1}", x.earned), format!("{:.1}", x.planned_pct), dt(x.forecast[5]), d.remarks.clone()]);
        for (j, c) in v.iter().enumerate() { ws.write((i + 1) as u32, j as u16, c).map_err(e)?; }
    }
    ws.set_freeze_panes(1, 3).map_err(e)?;
    let ws = wb.add_worksheet().set_name("Transmittal Log").map_err(e)?;
    let cols = ["Document No.", "Title", "Revision", "Purpose", "Transmittal", "Sent", "Review due", "Return transmittal", "Returned", "Code", "Comments ref.", "Accepted", "Days with client", "Days late", "State"];
    for (j, h) in cols.iter().enumerate() { ws.write_with_format(0, j as u16, *h, &head).map_err(e)?; }
    let byid: HashMap<u32, &VDoc> = r.docs.iter().map(|d| (d.id, d)).collect();
    for (i, (s, x)) in r.log.iter().zip(&o.subs).enumerate() {
        let d = byid.get(&s.doc);
        let v = [d.map(|d| d.number.clone()).unwrap_or_default(), d.map(|d| d.title.clone()).unwrap_or_default(), s.rev.clone(), s.purpose.clone(), s.transmittal.clone(), dt(s.sent), dt(x.due), s.ret_transmittal.clone(), dt(s.returned), s.code.clone(), s.crs.clone(), if x.accepted { "Yes".into() } else { String::new() }, x.days.to_string(), x.late.to_string(), x.state.clone()];
        for (j, c) in v.iter().enumerate() { ws.write((i + 1) as u32, j as u16, c).map_err(e)?; }
    }
    ws.set_freeze_panes(1, 1).map_err(e)?;
    wb.save(path).map_err(e)
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::doc::{Doc, Op};
    const F: &str = "%d/%m/%Y";
    fn d(y: i32, m: u32, dd: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, dd).unwrap() }
    fn sub(id: u32, doc: u32, rev: &str, purpose: &str, sent: NaiveDate, returned: Option<NaiveDate>, code: &str) -> Submission {
        Submission { id, doc, rev: rev.into(), purpose: purpose.into(), sent: Some(sent), returned, code: code.into(), ..Default::default() }
    }

    /// A Monday start, Monday–Friday calendar; P date Monday 2 Nov 2026, review 10 and turnaround 10 working days.
    fn reg() -> Doc {
        let mut doc = Doc::new(Project::new("Package", d(2026, 11, 2)));
        doc.apply(Op::Modules { on: vec!["documents".into()] }, F).unwrap();
        let mut r = DocRegister { p_date: Some(d(2026, 11, 2)), ..Default::default() };
        r.docs = vec![
            VDoc { id: 1, number: "DOC-001".into(), title: "General arrangement".into(), class: 1, offset: Some(2.0), weight: 1.0, progress: true, ..Default::default() },
            VDoc { id: 2, number: "DOC-002".into(), title: "Spare parts list".into(), class: 3, offset: Some(4.0), weight: 1.0, progress: true, ..Default::default() },
            VDoc { id: 3, number: "DOC-003".into(), title: "Monthly report".into(), class: 2, offset: Some(1.0), weight: 1.0, progress: true, recurring: true, ..Default::default() },
        ];
        doc.apply(Op::Documents { r: Box::new(r) }, F).unwrap();
        doc
    }

    #[test]
    fn planned_dates_follow_the_p_date_and_working_day_cycles() {
        let doc = reg();
        let o = status(&Calc::new(&doc.p, F, d(2026, 11, 2)));
        let a = &o.docs[0].planned;
        assert_eq!(a[0], Some(d(2026, 11, 16)), "P + 2 weeks");
        assert_eq!(a[1], Some(d(2026, 11, 30)), "+10 working days of review");
        assert_eq!(a[2], Some(d(2026, 12, 14)), "+10 working days of turnaround");
        assert_eq!(a[5], Some(d(2027, 2, 8)), "final issue: review + turnaround after the construction issue (11 Jan)");
        let b = &o.docs[1].planned;
        assert_eq!((b[2], b[3], b[4]), (None, None, b[0]), "class 3 goes straight to the construction issue");
        assert!(o.weights_ok.iter().all(|x| *x), "default weights add up to 100 for every class");
    }

    #[test]
    fn the_log_drives_status_progress_and_client_delay() {
        let mut doc = reg();
        let mut r = doc.p.documents.clone();
        // Sent on time, back with code 3 after 20 calendar days (10 working days are due on 30 Nov), resent, then accepted.
        r.log = vec![
            sub(1, 1, "A", "ICR", d(2026, 11, 16), Some(d(2026, 12, 4)), "Code 3"),
            sub(2, 1, "B", "ICR", d(2026, 12, 11), Some(d(2026, 12, 18)), "Code 2"),
            sub(3, 1, "C", "IFA", d(2027, 1, 4), None, ""),
            sub(4, 99, "A", "ICR", d(2026, 12, 1), None, ""),
        ];
        doc.apply(Op::Documents { r: Box::new(r) }, F).unwrap();
        assert_eq!(doc.p.documents.log.len(), 3, "a submission for a document not in the register is dropped");
        let o = status(&Calc::new(&doc.p, F, d(2027, 1, 25)));
        let x = &o.docs[0];
        assert_eq!(x.actual[0], Some(d(2026, 11, 16)));
        assert_eq!(x.actual[1], Some(d(2026, 12, 18)), "accepted at the first accepted return");
        assert_eq!((x.subs, x.cycles, x.rev.as_str()), (3, 1, "C"));
        assert_eq!(x.due, Some(d(2027, 1, 18)), "10 working days after 4 Jan");
        assert_eq!(x.status, "Review overdue");
        assert_eq!(o.subs[0].late, 4, "returned 4 Dec, due 30 Nov");
        assert_eq!(o.subs[2].late, 7, "still with the client a week after it was due (to 25 Jan)");
        assert_eq!(x.client_late, 11);
        // Class 1 weights: 35 + 15 + 10 (IFA sent) = 60 %.
        assert!((x.earned - 60.0).abs() < 1e-9, "{}", x.earned);
        assert_eq!(o.docs[1].status, "Not submitted – overdue");
        // The recurring report is left out of progress: (60 + 0) / 2.
        assert!((o.earned - 30.0).abs() < 1e-9, "{}", o.earned);
        assert!(!o.weekly.is_empty() && o.weekly.iter().all(|p| p.end.weekday() == chrono::Weekday::Fri), "weeks end on the plan's week-end day");
        assert!(o.monthly.windows(2).all(|w| w[0].planned <= w[1].planned));
        assert_eq!(o.monthly.last().map(|p| p.planned.round()), Some(100.0));
        assert!(o.weekly.iter().filter(|p| p.end > d(2027, 1, 29)).all(|p| p.actual.is_none()) && o.weekly.iter().any(|p| p.end == d(2027, 1, 29) && p.actual.is_some()), "the actual line stops in the week of the data date");
    }

    #[test]
    fn review_days_count_on_their_own_calendar() {
        let mut doc = reg();
        let mut cal = Calendar::standard("Client week");
        cal.week[0] = vec![];
        cal.week[4] = vec![];
        cal.week[6] = vec![(480, 1020)];
        doc.apply(Op::CalPut { old: None, cal }, F).unwrap();
        let mut r = doc.p.documents.clone();
        r.review_cal = "Client week".into();
        doc.apply(Op::Documents { r: Box::new(r) }, F).unwrap();
        let o = status(&Calc::new(&doc.p, F, d(2026, 11, 2)));
        // Sunday and Tuesday–Thursday: 10 working days after Mon 16 Nov is Wed 2 Dec (30 Nov on the plan's calendar).
        assert_eq!(o.docs[0].planned[1], Some(d(2026, 12, 2)));
    }
}
