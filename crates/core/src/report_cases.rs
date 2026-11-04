//! Hand-checked report figures: % complete, planned %, earned value, earned schedule, S-curves,
//! manpower, cost curves and period reports on small plans, and their edge cases.
use crate::{doc::{Doc, Op}, fields::Calc, progress, report, *};
use chrono::NaiveDate;

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
const F: &str = "%d-%b-%y";

/// A plan starting Monday 2 Nov 2026: rows "duration|preds|field=value;…" set through the editor.
fn doc(rows: &[&str]) -> Doc {
    let mut p = Project::new("R", d(2026, 11, 2));
    for i in 0..rows.len() { let t = p.new_task(&format!("T{}", i + 1), 0, 0); p.tasks.push(t) }
    let mut doc = Doc::new(p);
    for (i, r) in rows.iter().enumerate() {
        let mut parts = r.split('|');
        let (dur, preds, extra) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        let uid = doc.p.tasks[i].uid;
        for (k, v) in [("dur", dur), ("preds", preds)].into_iter().chain(extra.split(';').filter(|s| !s.is_empty()).map(|kv| kv.split_once('=').unwrap_or((kv, "")))) {
            doc.apply(Op::Set { uid, field: k.into(), value: v.into() }, F).unwrap_or_else(|e| panic!("row {} {k}: {e}", i + 1));
        }
    }
    doc
}
fn op(doc: &mut Doc, o: Op) { doc.apply(o, F).unwrap_or_else(|e| panic!("{e}")) }
fn set(doc: &mut Doc, k: usize, f: &str, v: &str) { let uid = doc.p.tasks[k].uid; op(doc, Op::Set { uid, field: f.into(), value: v.into() }) }
fn baseline(doc: &mut Doc) { op(doc, Op::Baseline { uids: vec![], clear: false, n: 0 }) }
fn status(doc: &mut Doc, s: &str) { op(doc, Op::ProjectSet { field: "status".into(), value: serde_json::json!(s) }) }
fn calc(doc: &Doc, today: NaiveDate) -> Calc<'static> { Calc::new(Box::leak(Box::new(doc.p.clone())), F, today) }
fn close(a: f64, b: f64) -> bool { (a - b).abs() < 1e-6 }
fn finite(v: &[f64]) -> bool { v.iter().all(|x| x.is_finite()) }
fn names(c: &Calc) -> std::collections::HashMap<&'static str, String> { crate::sheet::project_names(c).into_iter().map(|(n, v, _)| (n, v.display())).collect() }

/// Two fitters' tasks in a row and a welder's task beside them, baselined, at Tuesday 10 Nov:
/// T1 (5d, Fitter $10/h) done, T2 (5d after T1, Fitter + Welder $20/h) 20 %, T3 (3d, Welder) not started.
fn crew() -> Doc {
    let mut x = doc(&["5d", "5d|1", "3d"]);
    x.p.resources.push(Resource { uid: 1, name: "Fitter".into(), max: 1.0, group: "Mech".into(), rate: 10.0, ..Default::default() });
    x.p.resources.push(Resource { uid: 2, name: "Welder".into(), max: 1.0, rate: 20.0, ..Default::default() });
    set(&mut x, 0, "res", "Fitter"); set(&mut x, 1, "res", "Fitter,Welder"); set(&mut x, 2, "res", "Welder");
    baseline(&mut x); set(&mut x, 0, "pct", "100"); set(&mut x, 1, "pct", "20"); status(&mut x, "2026-11-10");
    x
}

#[test]
fn hand_checked_figures_of_a_small_crew() {
    let c = calc(&crew(), d(2026, 11, 10));
    let s = c.summary();
    // Work 40 + 80 + 24 = 144 h; cost 400 + (400 + 800) + 480 = 2,080.
    assert_eq!((s.work.as_str(), s.cost.as_str()), ("144 hrs", "$2,080.00"));
    // PV at the end of Tue 10 Nov: T1 400 + T3 480 + 2/5 of T2's 1,200 = 1,360; EV 400 + 240 = 640; AC the same.
    assert_eq!((s.pv.as_str(), s.ev.as_str(), s.ac.as_str()), ("$1,360.00", "$640.00", "$640.00"));
    assert!(close(s.spi.unwrap(), 640.0 / 1360.0) && close(s.cpi.unwrap(), 1.0));
    // Duration-weighted progress (5, 5, 3): actual (5 + 1) / 13; planned at the status date (5 + 2 + 3) / 13.
    assert!(close(s.pct, 600.0 / 13.0), "{}", s.pct);
    let r = report::period_report(&c, "week", None);
    assert!(close(r.actual, 600.0 / 13.0) && close(r.planned, 1000.0 / 13.0) && close(r.actual_prev, 500.0 / 13.0), "{} {} {}", r.actual, r.planned, r.actual_prev);
    // Hours per week on the current dates: T1's 40 in week 1, then T2 and T3 (not before the status date): 144 in all.
    assert!(close(r.hours[0].1, 40.0) && close(r.hours.iter().map(|h| h.1).sum(), 144.0), "{:?}", r.hours);
    // Cost curve per week: the planned and forecast lines add up to the budget, earned and actual to EV and AC.
    let cv = report::curve(&c, "week");
    let sum = |v: &[f64]| v.iter().sum::<f64>();
    assert!(close(sum(&cv.planned), 2080.0) && close(sum(&cv.forecast), 2080.0) && close(sum(&cv.earned), 640.0) && close(sum(&cv.actual), 640.0), "{:?} {:?} {:?} {:?}", cv.planned, cv.forecast, cv.earned, cv.actual);
    assert_eq!(cv.status_day, crate::cal::at(d(2026, 11, 10)) / crate::cal::DAY);
    // Manpower in people per week (40 h each): week 1 T1 + T3 = 64 h = 1.6; week 2 T2 = 80 h = 2.
    let m = progress::manpower(&c, "week");
    let all = &m.trades[0];
    assert_eq!(all.name, "All trades");
    assert!(close(all.planned[0], 1.6) && close(all.planned[1], 2.0), "{:?}", all.planned);
    assert!(close(all.actual[0].unwrap(), 1.0) && close(all.actual[1].unwrap(), 0.4), "{:?}", all.actual);
    let mech = m.trades.iter().find(|t| t.name == "Mech").unwrap();
    assert!(close(mech.planned.iter().sum(), 2.0), "{:?}", mech.planned);
    // Hours: the usage grid and the work-weighted S-curve both count 144 h.
    let u = report::usage(&c, "resource", "work", "week");
    assert!(close(u.rows.iter().filter(|r| r.of.is_none()).map(|r| r.total).sum(), 144.0));
    let sc = report::scurve(&c, "week", "work", None);
    assert!(close(sc.total, 144.0 * 60.0) && close(*sc.planned.last().unwrap(), 100.0) && close(*sc.forecast.last().unwrap(), 100.0));
}

#[test]
fn eac_vac_and_tcpi_by_hand() {
    // A 10-day task baselined at 80 h × $12.50 = 1,000; the rate goes up to $15 (EAC 1,200), half done.
    let mut x = doc(&["10d"]);
    x.p.resources.push(Resource { uid: 1, name: "Fitter".into(), max: 1.0, rate: 12.5, ..Default::default() });
    set(&mut x, 0, "res", "Fitter");
    baseline(&mut x);
    x.p.resources[0].rate = 15.0;
    set(&mut x, 0, "pct", "50"); status(&mut x, "2026-11-06");
    let c = calc(&x, d(2026, 11, 6));
    let v = |k: &str| c.val(0, k).num();
    assert_eq!((v("bac"), v("bcws"), v("bcwp"), v("acwp")), (1000.0, 500.0, 500.0, 600.0));
    assert_eq!((v("sv"), v("cv")), (0.0, -100.0));
    assert!(close(v("spi"), 1.0) && close(v("cpi"), 500.0 / 600.0));
    assert!(close(v("eac"), 1200.0) && close(v("vac"), -200.0) && close(v("tcpi"), 1.25), "{} {} {}", v("eac"), v("vac"), v("tcpi"));
}

// The actual line must run to the status date, also when it falls inside a week or month.
#[test]
fn scurve_actual_reaches_the_status_date_in_every_unit() {
    let mut x = doc(&["10d"]);
    baseline(&mut x); set(&mut x, 0, "pct", "30"); status(&mut x, "2026-11-04");
    let c = calc(&x, d(2026, 11, 4));
    assert!(close(report::period_report(&c, "week", None).actual, 30.0));
    // Planned progress is measured on the same day as actual: on plan on Wednesday is no variance.
    for kind in ["week", "month"] {
        let r = report::period_report(&c, kind, None);
        assert!(close(r.actual, 30.0) && close(r.planned, 30.0), "{kind}: actual {} vs planned {} (the plan is exactly on time)", r.actual, r.planned);
    }
    let past = report::period_report(&c, "week", Some(report::period_ends(&c, "week")[0] - 7));
    assert!(close(past.planned, 0.0) && close(past.actual, 0.0));
    for unit in ["day", "week", "month"] {
        let s = report::scurve(&c, unit, "duration", None);
        let last = s.actual.iter().flatten().last().copied().unwrap_or(-1.0);
        assert!(close(last, 30.0), "{unit}: the actual line ends at {last:.1} % (the report says 30 %)");
        let k = s.days.iter().position(|&d| d >= s.status_day).unwrap();
        assert!(s.actual[k].is_some() && s.actual[k + 1..].iter().all(Option::is_none), "{unit}: {:?}", s.actual);
    }
    let r = report::scurve(&calc(&crew(), d(2026, 11, 10)), "month", "duration", None);
    assert!(close(r.actual.iter().flatten().last().copied().unwrap(), 600.0 / 13.0), "{:?}", r.actual);
}

#[test]
fn scurve_of_a_very_long_plan_ends_at_100() {
    let mut x = doc(&["15000d"]);
    baseline(&mut x); set(&mut x, 0, "pct", "10");
    let c = calc(&x, d(2026, 11, 4));
    for unit in ["day", "week", "month"] {
        let s = report::scurve(&c, unit, "duration", None);
        assert!(close(*s.planned.last().unwrap(), 100.0) && close(*s.forecast.last().unwrap(), 100.0), "{unit}: ends at {:?} / {:?}", s.planned.last(), s.forecast.last());
    }
}

#[test]
fn planned_percent_of_a_plan_of_milestones() {
    let mut x = doc(&["0d", "0d|1"]);
    baseline(&mut x); status(&mut x, "2026-11-06");
    let n = names(&calc(&x, d(2026, 11, 6)));
    assert_eq!((n["PercentComplete"].as_str(), n["PlannedPercent"].as_str()), ("0", "100"), "both milestones were due on Monday");
}

// Without a status date, progress entered ahead of time is measured on the day the work would
// reach it in working time: a task exactly on plan is neither ahead nor behind.
#[test]
fn progress_without_a_status_date_is_measured_in_working_time() {
    let mut x = doc(&["10d"]);
    baseline(&mut x); set(&mut x, 0, "pct", "30");
    let c = calc(&x, d(2026, 11, 2));
    assert_eq!(c.progress_date(), d(2026, 11, 4), "30 % of Mon 2 – Fri 13 Nov is done on Wednesday evening");
    assert!(close(report::period_report(&c, "week", None).actual, 30.0));
    assert_eq!(names(&c)["PlannedPercent"], "30");
    // The cost chart's status marker stands on the same day (it stood on the first period without a status date).
    assert_eq!(report::curve(&c, "week").status_day, crate::cal::at(d(2026, 11, 4)) / crate::cal::DAY);
    let e = progress::earned_schedule(&c).unwrap();
    assert!(close(e.spi_t, 1.0), "{e:?}");
}

// A plan finished on time stays on time: actual time stops when the work is complete.
#[test]
fn earned_schedule_of_a_finished_plan_stops_at_its_finish() {
    let mut x = doc(&["5d", "5d|1"]);
    baseline(&mut x); set(&mut x, 0, "pct", "100"); set(&mut x, 1, "pct", "100"); status(&mut x, "2026-11-30");
    let e = progress::earned_schedule(&calc(&x, d(2026, 11, 30))).unwrap();
    assert!(close(e.es, 10.0) && close(e.at, 10.0) && close(e.spi_t, 1.0) && e.forecast_finish == e.planned_finish, "{e:?}");
}

// Edge cases: nothing in a report may be NaN or infinite, and every planned curve ends at 100 %.
#[test]
fn report_figures_stay_finite_on_edge_plans() {
    let mut plans: Vec<(&str, Doc, NaiveDate)> = vec![("empty", Doc::new(Project::new("E", d(2026, 11, 2))), d(2026, 11, 6)), ("one task", doc(&["5d"]), d(2026, 11, 6))];
    let mut m = doc(&["0d", "0d|1"]); baseline(&mut m); status(&mut m, "2026-11-06"); plans.push(("milestones", m, d(2026, 11, 6)));
    let mut a = doc(&["5d||fixedcost=100", "5d|1|fixedcost=100"]); baseline(&mut a);
    for k in 0..2 { set(&mut a, k, "pct", "100") }
    plans.push(("all done", a, d(2026, 11, 20)));
    let mut n = doc(&["5d||fixedcost=100", "5d|1|fixedcost=100"]); baseline(&mut n); status(&mut n, "2026-10-01"); plans.push(("status before start", n, d(2026, 11, 6)));
    let mut s = doc(&["0d", "5d"]); s.p.tasks[1].level = 1; plans.push(("only a summary", s, d(2026, 11, 6)));
    let mut l = doc(&["10d||deadline=2026-11-04"]); set(&mut l, 0, "pct", "10"); plans.push(("negative float", l, d(2026, 11, 6)));
    for (name, x, today) in &plans {
        let c = calc(x, *today);
        let sm = c.summary();
        assert!(sm.pct.is_finite() && sm.spi.is_none_or(f64::is_finite) && sm.cpi.is_none_or(f64::is_finite), "{name}");
        for unit in ["day", "week", "month"] {
            for how in ["duration", "cost", "work", "physical"] {
                let sc = report::scurve(&c, unit, how, None);
                assert!(finite(&sc.planned) && finite(&sc.forecast) && sc.actual.iter().flatten().all(|v| v.is_finite()), "{name} {unit} {how}");
                assert!(sc.days.len() == sc.planned.len() && sc.days.windows(2).all(|w| w[0] < w[1]), "{name} {unit}: {:?}", sc.days);
                if !x.p.tasks.is_empty() { assert!(close(*sc.planned.last().unwrap(), 100.0), "{name} {unit} {how}: {:?}", sc.planned) }
            }
            let cv = report::curve(&c, unit);
            assert!(finite(&cv.planned) && finite(&cv.earned) && finite(&cv.actual) && finite(&cv.forecast), "{name} {unit}");
            let mp = progress::manpower(&c, unit);
            assert!(mp.trades.iter().all(|t| finite(&t.planned) && t.actual.iter().chain(&t.forecast).flatten().all(|v| v.is_finite())), "{name} {unit}");
        }
        let r = report::period_report(&c, "week", None);
        assert!([r.planned, r.planned_prev, r.actual, r.actual_prev].iter().all(|v| v.is_finite() && (-1e-9..=100.0 + 1e-9).contains(v)), "{name}");
        if let Some(e) = progress::earned_schedule(&c) { assert!([e.es, e.at, e.pd, e.spi_t, e.sv_t, e.ieac].iter().all(|v| v.is_finite()), "{name}: {e:?}") }
        assert!(crate::sheet::project_names(&c).iter().all(|(_, v, _)| !matches!(v, gridcraft_core::Value::Number(n) if !n.is_finite())), "{name}");
    }
}

// Dashboard tiles: errors come back as error codes with no number, ranges stop at the last cell in
// use, results are capped, and nothing is ever an infinite or NaN number.
#[test]
fn dashboard_tiles_on_odd_formulas_and_ranges() {
    let mut x = doc(&["5d"]);
    x.p.sheets.push(SheetData { name: "Data".into(), cells: [("A1", "x"), ("B1", "=1/0"), ("A2", "y"), ("B2", "-3")].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), ..Default::default() });
    let c = calc(&x, d(2026, 11, 2));
    let items: Vec<(String, String)> = [("=1/0", "0%"), ("=Gone!A1:B3", ""), ("=Data!A1:Z5000", ""), ("=SQRT(-1)", ""), ("=1E308*10", ""), ("=\"abc\"", "money"), ("=Data!Z99", ""), ("", ""), ("=", ""), ("=SEQUENCE(1000,100)", ""), ("=Data!C5:D9", "")]
        .iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    let r = crate::sheet::eval(&c, &items);
    let first = |k: usize| r[k][0][0].clone();
    assert_eq!((first(0).0, first(0).1), ("#DIV/0!".to_string(), None));
    assert_eq!(first(1).0, "#REF!", "a range on a deleted sheet");
    assert_eq!((r[2].len(), r[2][0].len()), (2, 2), "A1:Z5000 stops at the last cell in use");
    assert_eq!(r[2][1][1].1, Some(-3.0));
    assert_eq!((first(3).0, first(4).0), ("#NUM!".to_string(), "#NUM!".to_string()));
    assert_eq!((first(5).0, first(5).1), ("abc".to_string(), None), "text in a currency tile stays text");
    assert!(r[6].is_empty(), "an empty cell past the cells in use");
    assert_eq!((first(7).0, first(8).0), (String::new(), String::new()));
    assert_eq!((r[9].len(), r[9][0].len()), (500, 40), "spills are capped at 500 × 40");
    assert!(r[10].is_empty(), "a range past the cells in use is empty: {:?}", r[10]);
    assert!(r.iter().flatten().flatten().all(|c| c.1.is_none_or(f64::is_finite)));
    // The "Vendors and crews" tile of the template counts names: a table always keeps one (empty) row.
    let count = |c: &Calc| crate::sheet::eval(c, &[("=COUNTA(Resources[Name])".into(), String::new())])[0][0][0].0.clone();
    assert_eq!(count(&c), "0", "no resources");
    assert_eq!(count(&calc(&crew(), d(2026, 11, 2))), "2");
}

// A phase that starts after the plan: its S-curve runs forward in time from the day before it starts.
#[test]
fn phase_scurve_days_run_forward() {
    let mut x = doc(&["5d", "0d", "5d|1"]);
    x.p.tasks[2].level = 1;
    baseline(&mut x);
    let c = calc(&x, d(2026, 11, 2));
    for unit in ["day", "week", "month"] {
        let s = report::scurve(&c, unit, "duration", Some(x.p.tasks[1].uid));
        assert!(s.days.windows(2).all(|w| w[0] < w[1]) && s.planned.windows(2).all(|w| w[0] <= w[1]), "{unit}: {:?} {:?}", s.days, s.planned);
        assert!(close(s.planned[0], 0.0) && close(*s.planned.last().unwrap(), 100.0));
    }
}
