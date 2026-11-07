//! The Procurement Register module: sub-orders from enquiry to delivery. Planned dates (typed, or
//! from the task that stands for the order), actuals and the vendor's promised delivery against
//! the date the plan needs the item; weighted progress (manufacturing pro rata), an expediting
//! log, S-curves, and (when the user links them) the vendor documents each order waits for.
use crate::{cal::{date_of, DAY}, fields::Calc, model::*};
use chrono::{Datelike, Days, NaiveDate};
use serde::Serialize;

/// The five milestones of an order.
pub const MILESTONES: [&str; 5] = ["RFQ issued", "PO placed", "Manufacturing", "Inspection release (IRN)", "Delivered"];
/// Weight presets (%): balanced, more credit at the order, more credit at delivery.
pub const PRESETS: [(&str, [f64; 5]); 3] = [("balanced", [5.0, 15.0, 60.0, 10.0, 10.0]), ("order", [5.0, 25.0, 50.0, 10.0, 10.0]), ("delivery", [5.0, 10.0, 50.0, 15.0, 20.0])];

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct ItemOut {
    pub id: u32,
    /// RFQ, PO, manufacturing start, IRN, delivery: planned (typed or from the task) and actual.
    pub planned: [Option<NaiveDate>; 5],
    pub actual: [Option<NaiveDate>; 5],
    /// Actual delivery, else the vendor's promise, else the plan.
    pub forecast: Option<NaiveDate>,
    /// Forecast against planned delivery (days, + late).
    pub var: Option<i64>,
    /// When the plan needs it (the needing task's start, or a date typed) and the float to it.
    pub required: Option<NaiveDate>,
    pub float: Option<i64>,
    /// "red", "amber", "green", "" (no required date).
    pub rag: String,
    pub status: String,
    pub next: String,
    pub next_date: Option<NaiveDate>,
    pub days_late: i64,
    pub weight: f64,
    pub earned: f64,
    pub planned_pct: f64,
    pub task_name: String,
    pub need_name: String,
    /// Linked vendor documents: how many, how many accepted, and whether they lag manufacturing.
    pub docs: usize,
    pub docs_ok: usize,
    pub docs_late: bool,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct Point { pub end: NaiveDate, pub planned: f64, pub actual: Option<f64> }

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct ProcOut {
    pub data_date: Option<NaiveDate>,
    pub items: Vec<ItemOut>,
    pub earned: f64,
    pub planned: f64,
    pub weights: [f64; 5],
    pub weights_ok: bool,
    pub weekly: Vec<Point>,
    pub monthly: Vec<Point>,
}

/// The weights in use: a preset, or the user's own.
pub fn weights(r: &ProcRegister) -> [f64; 5] {
    PRESETS.iter().find(|p| p.0 == r.preset).map(|p| p.1).unwrap_or_else(|| {
        let mut w = [0.0; 5];
        for (i, v) in r.weights.iter().take(5).enumerate() { w[i] = *v }
        w
    })
}

/// Earned share (0–1) of an order at day `e`, from its dates, with manufacturing pro rata.
fn share(w: &[f64; 5], d: &[Option<NaiveDate>; 5], e: NaiveDate, mfg: impl Fn(NaiveDate) -> f64) -> f64 {
    let on = |i: usize| d[i].is_some_and(|x| x <= e);
    (w[0] * f64::from(u8::from(on(0))) + w[1] * f64::from(u8::from(on(1))) + w[2] * mfg(e) + w[3] * f64::from(u8::from(on(3))) + w[4] * f64::from(u8::from(on(4)))) / 100.0
}
/// Manufacturing done at `e`, straight-line from its start to the inspection release.
fn line(a: Option<NaiveDate>, b: Option<NaiveDate>, e: NaiveDate) -> f64 {
    match (a, b) {
        (Some(a), _) if e < a => 0.0,
        (Some(a), Some(b)) if b > a => ((e - a).num_days() as f64 / (b - a).num_days() as f64).clamp(0.0, 1.0),
        (Some(_), _) => 1.0,
        _ => 0.0,
    }
}

pub fn status(c: &Calc) -> ProcOut {
    let (p, r) = (c.p, &c.p.procurement);
    let today = date_of(c.progress_at() - DAY);
    let w = weights(r);
    let docs = (r.link_docs && p.modules.iter().any(|m| m == "documents")).then(|| crate::docreg::status(c));
    let mut o = ProcOut { data_date: Some(today), weights: w, weights_ok: (w.iter().sum::<f64>() - 100.0).abs() < 0.01, ..Default::default() };
    let values: f64 = r.items.iter().map(|x| x.value.max(0.0)).sum();
    let mut wsum = 0.0;
    for it in &r.items {
        let task = (it.task > 0).then(|| p.idx(it.task)).flatten();
        let need = (it.need > 0).then(|| p.idx(it.need)).flatten();
        let mut pl = it.planned;
        if let Some(i) = task {
            // The task stands for manufacture and delivery: it starts manufacturing and finishes with delivery.
            pl[2] = pl[2].or(Some(date_of(c.s.es[i])));
            pl[4] = pl[4].or(Some(date_of(c.s.ef[i] - 1)));
        }
        if pl[3].is_none() { pl[3] = pl[4].and_then(|d| d.checked_sub_days(Days::new(r.irn_days.max(0) as u64))).map(|d| pl[2].map_or(d, |s| d.max(s))) }
        let a = it.actual;
        let forecast = a[4].or(it.promised).or(pl[4]);
        let required = it.required.or(need.map(|i| date_of(c.s.es[i])));
        let float = required.zip(forecast).map(|(r, f)| (r - f).num_days());
        let status = if a[4].is_some() { "Delivered" } else if a[3].is_some() { "Inspection released" } else if a[2].is_some() { "In manufacture" } else if a[1].is_some() { "PO placed" } else if a[0].is_some() { "RFQ issued" } else { "Not started" };
        let k = (0..5).find(|&i| a[i].is_none());
        let next_date = k.and_then(|i| pl[i]);
        let mfg_now = if a[3].is_some() || a[4].is_some() { 1.0 } else { (it.mfg_pct.clamp(0.0, 100.0)) / 100.0 };
        let earned = share(&w, &a, today, |_| if a[2].is_some() { mfg_now } else { 0.0 });
        let planned = share(&w, &pl, today, |e| line(pl[2], pl[3], e));
        let weight = match r.weighting.as_str() { "equal" => 1.0, "manual" => it.manual_w.max(0.0), _ => if values > 0.0 { it.value.max(0.0) } else { 1.0 } };
        let mut row = ItemOut {
            id: it.id, planned: pl, actual: a, forecast, var: forecast.zip(pl[4]).map(|(f, p)| (f - p).num_days()), required, float,
            rag: match float { None => "", Some(f) if f < 0 => "red", Some(f) if f < r.warn_days.max(0) => "amber", _ => "green" }.into(),
            status: status.into(), next: k.map(|i| MILESTONES[i].to_string()).unwrap_or_default(), next_date, days_late: next_date.map(|d| (today - d).num_days().max(0)).unwrap_or(0),
            weight, earned: earned * 100.0, planned_pct: planned * 100.0,
            task_name: task.map(|i| p.tasks[i].name.clone()).unwrap_or_default(), need_name: need.map(|i| p.tasks[i].name.clone()).unwrap_or_default(), ..Default::default()
        };
        if let Some(d) = &docs {
            let ok = |id: u32| d.docs.iter().find(|x| x.id == id).is_some_and(|x| x.actual[3].or(x.actual[1]).is_some());
            let mine: Vec<&crate::docreg::DocRow> = d.docs.iter().filter(|x| it.docs.contains(&x.id)).collect();
            row.docs = mine.len();
            row.docs_ok = mine.iter().filter(|x| ok(x.id)).count();
            // Drawings still not accepted when manufacturing should start.
            row.docs_late = a[2].is_none() && row.docs_ok < row.docs && pl[2].is_some_and(|s| mine.iter().any(|x| !ok(x.id) && x.forecast[3].or(x.forecast[1]).is_some_and(|f| f > s)) || s < today);
        }
        wsum += weight;
        o.items.push(row);
    }
    if wsum > 0.0 {
        o.earned = o.items.iter().map(|x| x.weight * x.earned).sum::<f64>() / wsum;
        o.planned = o.items.iter().map(|x| x.weight * x.planned_pct).sum::<f64>() / wsum;
    }
    let mfg = |x: &ItemOut, it: &PItem| -> Box<dyn Fn(NaiveDate) -> f64> {
        let (s, end) = (x.actual[2], x.actual[3].or(x.actual[4]));
        let pct = if end.is_some() { 1.0 } else { it.mfg_pct.clamp(0.0, 100.0) / 100.0 };
        Box::new(move |e: NaiveDate| match (s, end) { (Some(s), _) if e < s => 0.0, (Some(s), Some(f)) => line(Some(s), Some(f), e), (Some(s), None) => if today > s { pct * line(Some(s), Some(today), e) } else { pct }, _ => 0.0 })
    };
    let curve = |end_of: &dyn Fn(NaiveDate) -> NaiveDate| -> Vec<Point> {
        let all: Vec<NaiveDate> = o.items.iter().flat_map(|x| x.planned.iter().chain(x.actual.iter()).flatten().copied()).collect();
        let (Some(a), Some(b)) = (all.iter().min(), all.iter().max()) else { return vec![] };
        let (mut e, last, mut out) = (end_of(*a), end_of(*b), vec![]);
        while e <= last && out.len() < 1500 && wsum > 0.0 {
            let pl = o.items.iter().map(|x| x.weight * share(&w, &x.planned, e, |d| line(x.planned[2], x.planned[3], d))).sum::<f64>() / wsum * 100.0;
            let ac = (e <= end_of(today)).then(|| o.items.iter().zip(&r.items).map(|(x, it)| x.weight * share(&w, &x.actual, e.min(today), mfg(x, it))).sum::<f64>() / wsum * 100.0);
            out.push(Point { end: e, planned: pl, actual: ac });
            let n = end_of(e.checked_add_days(Days::new(1)).unwrap_or(e));
            if n == e { break }
            e = n;
        }
        out
    };
    o.weekly = curve(&|d: NaiveDate| { let back = (d.weekday().num_days_from_monday() as i64 - p.periods.week_end.min(6) as i64).rem_euclid(7); d.checked_add_days(Days::new(((7 - back) % 7) as u64)).unwrap_or(d) });
    o.monthly = curve(&|d: NaiveDate| { let (y, m) = if d.month() == 12 { (d.year() + 1, 1) } else { (d.year(), d.month() + 1) }; NaiveDate::from_ymd_opt(y, m, 1).and_then(|x| x.pred_opt()).unwrap_or(d) });
    o
}

/// Tidies a register before it is stored.
pub fn clean(r: &mut ProcRegister) {
    let t = |s: &mut String, n: usize| { if s.chars().count() > n { *s = s.chars().take(n).collect() } };
    let num = |v: &mut f64| { if !v.is_finite() || *v < 0.0 { *v = 0.0 } };
    r.weights.resize(5, 0.0);
    for v in &mut r.weights { num(v) }
    r.irn_days = r.irn_days.clamp(0, 3650);
    r.warn_days = r.warn_days.clamp(0, 3650);
    if !["value", "equal", "manual"].contains(&r.weighting.as_str()) { r.weighting = "value".into() }
    r.items.truncate(5000);
    let mut seen = std::collections::HashSet::new();
    let mut next = r.items.iter().map(|x| x.id).max().unwrap_or(0) + 1;
    for it in &mut r.items {
        if it.id == 0 || !seen.insert(it.id) { it.id = next; next += 1; seen.insert(it.id); }
        for v in [&mut it.value, &mut it.manual_w, &mut it.mfg_pct] { num(v) }
        it.mfg_pct = it.mfg_pct.min(100.0);
        for s in [&mut it.tag, &mut it.name, &mut it.vendor, &mut it.origin, &mut it.po_no, &mut it.remarks] { t(s, 300) }
        it.log.truncate(2000);
        for x in &mut it.log { t(&mut x.note, 2000); t(&mut x.by, 120); }
        it.docs.dedup();
        it.docs.truncate(500);
    }
}

/// The register as an Excel workbook: the orders and the expediting log.
pub fn write_xlsx(p: &Project, o: &ProcOut, path: &std::path::Path) -> Result<(), String> {
    use rust_xlsxwriter::{Format, Workbook};
    let r = &p.procurement;
    let mut wb = Workbook::new();
    let head = Format::new().set_bold().set_background_color(0xEAF0F8);
    let dt = |d: Option<NaiveDate>| d.map(|x| x.format("%d-%b-%y").to_string()).unwrap_or_default();
    let e = |x: rust_xlsxwriter::XlsxError| x.to_string();
    let ws = wb.add_worksheet().set_name("Procurement").map_err(e)?;
    let mut cols: Vec<String> = ["Item", "Order / equipment", "Vendor", "Origin", "PO No.", "Value", "Long lead"].map(String::from).to_vec();
    cols.extend(MILESTONES.iter().map(|m| format!("Planned: {m}")));
    cols.extend(MILESTONES.iter().map(|m| format!("Actual: {m}")));
    cols.extend(["Mfg %", "Promised delivery", "Forecast delivery", "Delivery var. (d)", "Required", "Float (d)", "Status", "Next", "Next planned", "Days late", "Earned %", "Planned %", "Remarks"].map(String::from));
    for (j, h) in cols.iter().enumerate() { ws.write_with_format(0, j as u16, h, &head).map_err(e)?; }
    for (i, (it, x)) in r.items.iter().zip(&o.items).enumerate() {
        let mut v = vec![it.tag.clone(), it.name.clone(), it.vendor.clone(), it.origin.clone(), it.po_no.clone(), if it.value > 0.0 { it.value.to_string() } else { String::new() }, if it.long_lead { "Yes".into() } else { String::new() }];
        v.extend(x.planned.iter().map(|&q| dt(q)));
        v.extend(x.actual.iter().map(|&q| dt(q)));
        v.extend([format!("{:.0}", it.mfg_pct), dt(it.promised), dt(x.forecast), x.var.map(|v| v.to_string()).unwrap_or_default(), dt(x.required), x.float.map(|v| v.to_string()).unwrap_or_default(), x.status.clone(), x.next.clone(), dt(x.next_date), x.days_late.to_string(), format!("{:.1}", x.earned), format!("{:.1}", x.planned_pct), it.remarks.clone()]);
        for (j, c) in v.iter().enumerate() { ws.write((i + 1) as u32, j as u16, c).map_err(e)?; }
    }
    ws.set_freeze_panes(1, 2).map_err(e)?;
    let ws = wb.add_worksheet().set_name("Expediting Log").map_err(e)?;
    for (j, h) in ["Item", "Order / equipment", "Date", "Note", "By"].iter().enumerate() { ws.write_with_format(0, j as u16, *h, &head).map_err(e)?; }
    let mut row = 1u32;
    for it in &r.items { for x in &it.log { for (j, c) in [it.tag.clone(), it.name.clone(), dt(x.date), x.note.clone(), x.by.clone()].iter().enumerate() { ws.write(row, j as u16, c).map_err(e)?; } row += 1; } }
    wb.save(path).map_err(e)
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::doc::{Doc, Op};
    const F: &str = "%d/%m/%Y";
    fn d(y: i32, m: u32, dd: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, dd).unwrap() }
    fn plan() -> Doc {
        let mut doc = Doc::new(Project::new("Package", d(2026, 11, 2)));
        doc.apply(Op::Modules { on: vec!["procurement".into()] }, F).unwrap();
        let row = |n: &str, dur: &str| vec![("name".to_string(), n.to_string()), ("dur".to_string(), dur.to_string())];
        doc.apply(Op::InsertRows { at: 0, rows: vec![row("Forgings: manufacture and delivery", "50d"), row("Rotor machining", "20d")] }, F).unwrap();
        let uids = doc.p.tasks.iter().map(|t| t.uid).collect();
        doc.apply(Op::Link { uids }, F).unwrap();
        doc
    }

    #[test]
    fn an_order_against_the_plan_its_promise_and_the_date_it_is_needed() {
        let mut doc = plan();
        let (mfg, need) = (doc.p.tasks[0].uid, doc.p.tasks[1].uid);
        let r = ProcRegister {
            preset: "balanced".into(), weighting: "value".into(), irn_days: 14, warn_days: 14,
            items: vec![
                PItem { id: 1, tag: "1.1".into(), name: "Forgings".into(), value: 300.0, task: mfg, need, planned: [Some(d(2026, 11, 2)), Some(d(2026, 11, 9)), None, None, None],
                    actual: [Some(d(2026, 11, 2)), Some(d(2026, 11, 10)), Some(d(2026, 11, 16)), None, None], mfg_pct: 50.0, promised: Some(d(2027, 1, 22)), ..Default::default() },
                PItem { id: 2, tag: "1.2".into(), name: "Bearings".into(), value: 100.0, planned: [None, None, None, None, Some(d(2026, 12, 1))], ..Default::default() },
            ],
            ..Default::default()
        };
        doc.apply(Op::Procurement { r: Box::new(r) }, F).unwrap();
        let o = status(&Calc::new(&doc.p, F, d(2026, 12, 4)));
        let a = &o.items[0];
        assert_eq!(a.planned[2], Some(d(2026, 11, 2)), "manufacturing starts with its task");
        assert_eq!(a.planned[4], Some(d(2027, 1, 8)), "and delivery is the task's finish (50 working days)");
        assert_eq!(a.planned[3], Some(d(2026, 12, 25)), "IRN 14 days before delivery");
        assert_eq!((a.forecast, a.var), (Some(d(2027, 1, 22)), Some(14)), "the vendor's promise, two weeks late");
        assert_eq!(a.required, Some(d(2027, 1, 11)), "needed when the machining starts");
        assert_eq!((a.float, a.rag.as_str()), (Some(-11), "red"));
        assert_eq!((a.status.as_str(), a.next.as_str()), ("In manufacture", "Inspection release (IRN)"));
        // Balanced weights: 5 RFQ + 15 PO + 60 × 50 % manufacturing.
        assert!((a.earned - 50.0).abs() < 1e-9, "{}", a.earned);
        let b = &o.items[1];
        assert_eq!((b.status.as_str(), b.next.as_str(), b.days_late), ("Not started", "RFQ issued", 0), "no planned RFQ date: nothing is late yet");
        assert_eq!(b.rag, "", "no required date");
        // Weighted by value: (300 × 50 + 100 × 0) / 400.
        assert!((o.earned - 37.5).abs() < 1e-9, "{}", o.earned);
        assert!(o.weights_ok);
        assert!(o.monthly.windows(2).all(|w| w[0].planned <= w[1].planned + 1e-9));
        assert!(o.monthly.iter().filter(|p| p.end > d(2026, 12, 31)).all(|p| p.actual.is_none()));
    }

    #[test]
    fn linked_documents_count_only_when_the_user_links_them() {
        let mut doc = plan();
        doc.apply(Op::Modules { on: vec!["documents".into(), "procurement".into()] }, F).unwrap();
        let dr = DocRegister { p_date: Some(d(2026, 11, 2)), docs: vec![VDoc { id: 7, number: "D-7".into(), class: 1, offset: Some(1.0), weight: 1.0, progress: true, ..Default::default() }], ..Default::default() };
        doc.apply(Op::Documents { r: Box::new(dr) }, F).unwrap();
        let mut r = ProcRegister { items: vec![PItem { id: 1, name: "Seal system".into(), docs: vec![7], planned: [None, None, Some(d(2026, 11, 20)), None, Some(d(2027, 2, 1))], ..Default::default() }], ..Default::default() };
        doc.apply(Op::Procurement { r: Box::new(r.clone()) }, F).unwrap();
        let o = status(&Calc::new(&doc.p, F, d(2026, 12, 4)));
        assert_eq!((o.items[0].docs, o.items[0].docs_late), (0, false), "not linked: the documents are not read");
        r.link_docs = true;
        doc.apply(Op::Procurement { r: Box::new(r) }, F).unwrap();
        let o = status(&Calc::new(&doc.p, F, d(2026, 12, 4)));
        assert_eq!((o.items[0].docs, o.items[0].docs_ok, o.items[0].docs_late), (1, 0, true), "linked: the drawing is not approved and manufacturing was due to start");
    }

    #[test]
    fn presets_and_own_weights() {
        let mut r = ProcRegister { preset: "delivery".into(), ..Default::default() };
        assert_eq!(weights(&r), [5.0, 10.0, 50.0, 15.0, 20.0]);
        r.preset = "own".into();
        r.weights = vec![10.0, 10.0, 40.0, 20.0, 20.0];
        assert_eq!(weights(&r)[2], 40.0);
    }
}
