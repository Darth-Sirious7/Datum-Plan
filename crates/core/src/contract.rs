//! Modules a plan can take on (Project › Modules), and the first of them: Contract Dates &
//! Payments. Contract dates are measured against the forecast of the task that achieves them;
//! late ones run up delay damages; payments fall due after their event, giving the cash-in curve.
use crate::{cal::{date_of, DAY}, fields::Calc, model::*};
use chrono::{Datelike, Days, NaiveDate};
use serde::Serialize;

/// The modules: key, name, what it adds. A plan has only the ones it switched on.
pub const MODULES: [(&str, &str, &str); 2] = [
    ("contract", "Contract Dates & Payments", "Contract dates against their forecast, delay damages at risk, payment milestones and the cash-in curve."),
    ("documents", "Document Register", "Vendor documents and every submission: review cycles in working days, return codes, status, client delay, weighted progress and S-curves."),
];

/// What a contract date shows.
#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct MsOut {
    pub id: u32,
    pub name: String,
    pub task: u32,
    pub task_name: String,
    pub date: Option<NaiveDate>,
    /// The task's finish: actual when done, else forecast.
    pub forecast: Option<NaiveDate>,
    pub baseline: Option<NaiveDate>,
    pub done: bool,
    /// Days late (+) or early (−) against the contract date.
    pub var: Option<i64>,
    /// "Met", "Met late", "On track", "At risk", "Late", "No date", "No task".
    pub status: String,
    /// Delay damages at today's forecast, and their cap.
    pub ld: f64,
    pub ld_max: f64,
    /// The forecast at each period close (snapshots) and now: (date, forecast).
    pub trend: Vec<(NaiveDate, NaiveDate)>,
}

/// What a payment shows.
#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct PayOut {
    pub id: u32,
    pub name: String,
    pub amount: f64,
    pub task_name: String,
    /// Due by the baseline (or, without one, by the contract's planned dates) and by the forecast.
    pub planned: Option<NaiveDate>,
    pub due: Option<NaiveDate>,
    pub invoiced: Option<NaiveDate>,
    pub paid: Option<NaiveDate>,
    /// "Paid", "Invoiced", "Overdue", "Due", "Forecast", "No task".
    pub status: String,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct Out {
    pub value: f64,
    pub status_day: Option<NaiveDate>,
    pub milestones: Vec<MsOut>,
    pub payments: Vec<PayOut>,
    pub late: usize,
    pub at_risk: usize,
    pub ld: f64,
    pub ld_max: f64,
    /// Share of the contract value the payments cover (100 when complete).
    pub pay_pct: f64,
    pub received: f64,
    /// Month ends with cumulative cash in: planned, forecast, received (to the status date).
    pub cash: Vec<(NaiveDate, f64, f64, Option<f64>)>,
}

/// The finish of a task as a date: actual when it has one.
fn finish(c: &Calc, i: usize) -> NaiveDate {
    c.p.tasks[i].act_finish.unwrap_or_else(|| date_of(c.s.ef[i] - 1))
}

/// Delay damages for `late` days: a rate in % of the base per week or part week after the grace
/// days, up to the cap (% of the base).
pub fn damages(m: &ContractDate, value: f64, late: i64) -> (f64, f64) {
    let base = if m.value > 0.0 { m.value } else { value };
    let max = base * m.ld_cap.max(0.0) / 100.0;
    let weeks = ((late - m.grace.max(0)).max(0) + 6).div_euclid(7) as f64;
    ((base * m.ld_rate.max(0.0) / 100.0 * weeks).min(if m.ld_cap > 0.0 { max } else { f64::INFINITY }), max)
}

pub fn status(c: &Calc) -> Out {
    let (p, k) = (c.p, &c.p.contract);
    let today = date_of(c.progress_at() - DAY);
    let idx = |uid: u32| if uid == 0 { None } else { p.idx(uid) };
    let mut o = Out { value: k.value, status_day: Some(today), ..Default::default() };
    for m in &k.dates {
        let i = idx(m.task);
        let mut r = MsOut { id: m.id, name: m.name.clone(), task: m.task, date: m.date, ..Default::default() };
        if let Some(i) = i {
            let t = &p.tasks[i];
            r.task_name = t.name.clone();
            r.done = c.pct[i] >= 100;
            r.forecast = Some(finish(c, i));
            r.baseline = t.bls.get(&0).map(|b| date_of(b.finish - 1));
            let mut snaps: Vec<&Snapshot> = p.snapshots.iter().filter(|s| s.date.is_some()).collect();
            snaps.sort_by_key(|s| s.date);
            r.trend = snaps.iter().filter_map(|s| Some((s.date?, date_of(s.rows.iter().find(|x| x.uid == t.uid)?.finish - 1)))).collect();
            if r.trend.last().is_none_or(|x| x.0 < today) { r.trend.push((today, finish(c, i))); }
        }
        r.var = r.forecast.zip(m.date).map(|(f, d)| (f - d).num_days());
        r.status = match (i, m.date, r.var) {
            (None, ..) => "No task",
            (_, None, _) | (_, _, None) => "No date",
            (_, _, Some(v)) if r.done => if v > 0 { "Met late" } else { "Met" },
            (_, _, Some(v)) if v > 0 => "Late",
            (_, _, Some(v)) if -v < k.warn_days.max(0) => "At risk",
            _ => "On track",
        }.into();
        if let Some(v) = r.var { (r.ld, r.ld_max) = damages(m, k.value, v); } else { r.ld_max = damages(m, k.value, 0).1; }
        o.late += usize::from(r.status == "Late");
        o.at_risk += usize::from(r.status == "At risk");
        o.ld += r.ld;
        o.ld_max += r.ld_max;
        o.milestones.push(r);
    }
    for pm in &k.payments {
        let amount = if pm.amount > 0.0 { pm.amount } else { k.value * pm.pct / 100.0 };
        let terms = Days::new(pm.terms.max(0) as u64);
        let mut r = PayOut { id: pm.id, name: pm.name.clone(), amount, invoiced: pm.invoiced, paid: pm.paid, ..Default::default() };
        let i = idx(pm.task);
        if let Some(i) = i {
            r.task_name = p.tasks[i].name.clone();
            r.due = finish(c, i).checked_add_days(terms);
            r.planned = p.tasks[i].bls.get(&0).map(|b| date_of(b.finish - 1)).unwrap_or_else(|| date_of(c.s.ef[i] - 1)).checked_add_days(terms);
        }
        let event_done = i.is_some_and(|i| c.pct[i] >= 100);
        r.status = if r.paid.is_some() { "Paid" } else if r.invoiced.is_some() { "Invoiced" } else if i.is_none() { "No task" } else if event_done && r.due.is_some_and(|d| d < today) { "Overdue" } else if event_done { "Due" } else { "Forecast" }.into();
        o.pay_pct += if pm.amount > 0.0 && k.value > 0.0 { pm.amount / k.value * 100.0 } else { pm.pct };
        if r.paid.is_some_and(|d| d <= today) { o.received += amount; }
        o.payments.push(r);
    }
    o.cash = cash(&o.payments, today);
    o
}

/// The month ends from the first payment date to the last, with cumulative cash in.
fn cash(ps: &[PayOut], today: NaiveDate) -> Vec<(NaiveDate, f64, f64, Option<f64>)> {
    let when = |r: &PayOut| r.paid.or(r.due);
    let all: Vec<NaiveDate> = ps.iter().flat_map(|r| [r.planned, when(r)]).flatten().collect();
    let (Some(a), Some(b)) = (all.iter().min(), all.iter().max()) else { return vec![] };
    let month_end = |d: NaiveDate| {
        let (y, m) = if d.month() == 12 { (d.year() + 1, 1) } else { (d.year(), d.month() + 1) };
        NaiveDate::from_ymd_opt(y, m, 1).and_then(|x| x.pred_opt()).unwrap_or(d)
    };
    let (mut d, end, mut out) = (month_end(*a), month_end(*b), vec![]);
    while d <= end && out.len() < 600 {
        let sum = |f: &dyn Fn(&PayOut) -> Option<NaiveDate>| ps.iter().filter(|r| f(r).is_some_and(|x| x <= d)).map(|r| r.amount).sum::<f64>();
        let got = (d <= month_end(today)).then(|| sum(&|r| r.paid.filter(|x| *x <= today)));
        out.push((d, sum(&|r| r.planned), sum(&when), got));
        d = month_end(d.checked_add_days(Days::new(1)).unwrap_or(d));
        if d == end && out.last().is_some_and(|x| x.0 == end) { break }
    }
    out
}

/// Tidies a contract before it is stored: finite, non-negative numbers, unique ids, short texts.
pub fn clean(k: &mut Contract) {
    let num = |v: &mut f64| { if !v.is_finite() || *v < 0.0 { *v = 0.0 } };
    let text = |s: &mut String, n: usize| { if s.chars().count() > n { *s = s.chars().take(n).collect() } };
    num(&mut k.value);
    k.warn_days = k.warn_days.clamp(0, 3650);
    text(&mut k.number, 120);
    let mut next = 1;
    let mut seen = std::collections::HashSet::new();
    for m in &mut k.dates {
        if m.id == 0 || !seen.insert(m.id) { while seen.contains(&next) { next += 1 } m.id = next; seen.insert(next); }
        for v in [&mut m.value, &mut m.ld_rate, &mut m.ld_cap] { num(v) }
        m.grace = m.grace.clamp(0, 3650);
        text(&mut m.name, 200);
        text(&mut m.note, 2000);
    }
    let mut seen = std::collections::HashSet::new();
    next = 1;
    for x in &mut k.payments {
        if x.id == 0 || !seen.insert(x.id) { while seen.contains(&next) { next += 1 } x.id = next; seen.insert(next); }
        for v in [&mut x.amount, &mut x.pct] { num(v) }
        x.terms = x.terms.clamp(0, 3650);
        text(&mut x.name, 200);
    }
    k.dates.truncate(2000);
    k.payments.truncate(2000);
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::doc::{Doc, Op};

    const F: &str = "%d/%m/%Y";
    fn d(y: i32, m: u32, dd: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, dd).unwrap() }

    /// A unit delivered by a task, with its contract date, damages and two payments.
    fn plan() -> Doc {
        let mut doc = Doc::new(Project::new("Compressor package", d(2026, 11, 2)));
        doc.apply(Op::Modules { on: vec!["contract".into()] }, F).unwrap();
        let row = |n: &str, dur: &str| vec![("name".to_string(), n.to_string()), ("dur".to_string(), dur.to_string())];
        doc.apply(Op::InsertRows { at: 0, rows: vec![row("Manufacture", "40d"), row("Delivery FCA", "0d")] }, F).unwrap();
        let uids = doc.p.tasks.iter().map(|t| t.uid).collect();
        doc.apply(Op::Link { uids }, F).unwrap();
        doc
    }

    #[test]
    fn a_contract_date_against_its_forecast_with_damages_and_cash() {
        let mut doc = plan();
        let fca = doc.p.tasks[1].uid;
        let c = Calc::new(&doc.p, F, d(2026, 11, 2));
        let fc = finish(&c, 1);
        drop(c);
        // The contract date 10 days before the forecast: 2 weeks (or part) of 0.5 % on the unit's 1,000,000, capped at 10 %.
        let k = Contract {
            value: 4_000_000.0, warn_days: 14,
            dates: vec![
                ContractDate { id: 1, name: "Unit FCA".into(), task: fca, date: fc.checked_sub_days(Days::new(10)), value: 1_000_000.0, ld_rate: 0.5, ld_cap: 10.0, ..Default::default() },
                ContractDate { id: 1, name: "Unit FCA (second)".into(), task: fca, date: fc.checked_add_days(Days::new(5)), ..Default::default() },
                ContractDate { name: "No task".into(), date: Some(fc), ..Default::default() },
            ],
            payments: vec![
                Payment { id: 1, name: "Advance".into(), pct: 10.0, task: doc.p.tasks[0].uid, terms: 30, ..Default::default() },
                Payment { id: 2, name: "On delivery".into(), pct: 90.0, task: fca, terms: 60, paid: None, ..Default::default() },
            ],
            ..Default::default()
        };
        doc.apply(Op::Contract { c: Box::new(k) }, F).unwrap();
        assert_eq!(doc.p.contract.dates.iter().map(|m| m.id).collect::<Vec<_>>(), vec![1, 2, 3], "duplicate ids are renumbered");
        let o = status(&Calc::new(&doc.p, F, d(2026, 11, 2)));
        let m = &o.milestones;
        assert_eq!((m[0].var, m[0].status.as_str()), (Some(10), "Late"));
        assert_eq!((m[0].ld, m[0].ld_max), (10_000.0, 100_000.0), "2 weeks × 0.5 % of 1,000,000, cap 10 %");
        assert_eq!((m[1].var, m[1].status.as_str(), m[1].ld), (Some(-5), "At risk", 0.0), "5 days of float is under the 14-day warning");
        assert_eq!(m[2].status, "No task");
        assert_eq!((o.late, o.at_risk, o.ld), (1, 1, 10_000.0));
        assert_eq!(o.payments[1].amount, 3_600_000.0);
        assert_eq!(o.payments[1].due, fc.checked_add_days(Days::new(60)));
        assert_eq!(o.payments[0].status, "Forecast");
        assert_eq!(o.pay_pct, 100.0);
        let last = o.cash.last().unwrap();
        assert_eq!((last.1, last.2), (4_000_000.0, 4_000_000.0), "planned and forecast cash reach the full value");
        assert!(o.cash.windows(2).all(|w| w[0].0 < w[1].0 && w[0].2 <= w[1].2));
        assert_eq!(m[0].trend.len(), 1, "no period closes yet: the trend is today's forecast");
    }

    #[test]
    fn damages_follow_grace_part_weeks_and_the_cap() {
        let m = ContractDate { ld_rate: 1.0, ld_cap: 5.0, grace: 7, ..Default::default() };
        assert_eq!(damages(&m, 100.0, 7), (0.0, 5.0), "within the grace days");
        assert_eq!(damages(&m, 100.0, 8), (1.0, 5.0), "one day into a week counts the week");
        assert_eq!(damages(&m, 100.0, 70), (5.0, 5.0), "capped");
        assert_eq!(damages(&ContractDate { ld_rate: 1.0, ..Default::default() }, 100.0, 70).0, 10.0, "no cap");
        assert_eq!(damages(&m, 100.0, -20).0, 0.0);
    }

    #[test]
    fn modules_switch_on_and_off_and_off_can_drop_the_data() {
        let mut doc = plan();
        assert!(doc.apply(Op::Modules { on: vec!["nonsense".into()] }, F).is_err());
        doc.apply(Op::Contract { c: Box::new(Contract { value: 5.0, ..Default::default() }) }, F).unwrap();
        doc.apply(Op::Modules { on: vec![] }, F).unwrap();
        assert!(doc.p.modules.is_empty() && doc.p.contract.value == 5.0, "switched off, the data stays");
        doc.apply(Op::Contract { c: Box::default() }, F).unwrap();
        let json = serde_json::to_string(&doc.p).unwrap();
        assert!(!json.contains("\"contract\"") && !json.contains("\"modules\""), "a plan without modules saves as before");
    }
}
