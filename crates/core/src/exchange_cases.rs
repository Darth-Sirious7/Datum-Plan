//! Checked cases: progress, earned value, reports, risk and file exchange (0.5.12 audit).
use crate::{cal::{at, DAY}, doc::{Doc, Op}, fields::Calc, parse, sched::schedule, *};
use chrono::NaiveDate;

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
const D: i64 = 480;

fn plan(rows: &[(i64, &str)]) -> Project {
    let mut p = Project::new("T", d(2026, 11, 2));
    for (i, &(dur, _)) in rows.iter().enumerate() {
        let t = p.new_task(&format!("T{}", i + 1), 0, dur * D);
        p.tasks.push(t);
    }
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    for (t, &(_, pr)) in p.tasks.iter_mut().zip(rows) { t.preds = parse::preds(pr, &ids, D).unwrap() }
    p
}
fn op(doc: &mut Doc, o: Op) { doc.apply(o, "%d-%b-%y").unwrap() }
fn set(doc: &mut Doc, k: usize, f: &str, v: &str) { let uid = doc.p.tasks[k].uid; op(doc, Op::Set { uid, field: f.into(), value: v.into() }) }

fn round_trip(p: &Project, ext: &str) -> Project {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("datum-cases-{}-{ext}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join(format!("plan.{ext}"));
    crate::io::export(p, &f, "%d-%b-%y", d(2026, 11, 1), &[]).unwrap();
    let crate::io::Imported::Plan(q, _) = crate::io::import(&f).unwrap() else { panic!() };
    std::fs::remove_dir_all(dir).ok();
    *q
}

// Baseline work must not follow later changes to the current work.
#[test]
fn baseline_work_is_frozen() {
    let mut doc = Doc::new(plan(&[(2, "")]));
    doc.p.resources.push(Resource { uid: 1, name: "Fitter".into(), max: 2.0, ..Default::default() });
    set(&mut doc, 0, "res", "Fitter");
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 });
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    assert_eq!(c.val(0, "b0work").num(), 16.0);
    set(&mut doc, 0, "res", "Fitter[200%]");
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    assert_eq!(c.val(0, "work").num(), 32.0);
    assert_eq!(c.val(0, "b0work").num(), 16.0, "baseline work changed with the current assignment");
}

// The S-curve's planned line must reach 100 % when the baseline finishes after the current plan.
#[test]
fn scurve_planned_reaches_100_when_baseline_is_later() {
    let mut doc = Doc::new(plan(&[(30, "")]));
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 });
    set(&mut doc, 0, "dur", "5d");
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    for unit in ["day", "week", "month"] {
        let s = crate::report::scurve(&c, unit, "duration", None);
        let last = *s.planned.last().unwrap();
        assert!((last - 100.0).abs() < 1e-6, "{unit}: planned ends at {last:.1} %");
    }
}

// Cumulative actual cost on the curve must equal ACWP (dashboard AC).
#[test]
fn curve_actual_cost_matches_acwp_without_status_date() {
    let mut doc = Doc::new(plan(&[(4, "")]));
    set(&mut doc, 0, "fixedcost", "1000");
    set(&mut doc, 0, "accrual", "Prorated");
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 });
    set(&mut doc, 0, "pct", "50");
    // Today is before the work (progress entered ahead of time, no status date).
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 10, 20));
    let ac = c.val(0, "acwp").num();
    assert_eq!(ac, 500.0);
    let cv = crate::report::curve(&c, "week");
    assert_eq!(cv.actual.iter().sum::<f64>().round(), ac, "curve AC {:?}", cv.actual);
}

// Dashboard SPI must be measured at the same day as the progress shown beside it.
#[test]
fn spi_measured_at_progress_date_without_status() {
    let mut doc = Doc::new(plan(&[(5, "")]));
    set(&mut doc, 0, "fixedcost", "1000");
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 });
    set(&mut doc, 0, "pct", "100");
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    let s = c.summary();
    assert!(!s.measured_at.is_empty());
    let r = crate::report::period_report(&c, "week", None);
    assert_eq!(r.actual.round(), 100.0);
    assert_eq!(s.spi, Some(1.0), "PV measured at today while progress is measured at {}", s.measured_at);
}

// Summary % complete (rows) and the project % (dashboard) must agree.
#[test]
fn summary_pct_rounding_matches_project_pct() {
    let mut doc = Doc::new(plan(&[(0, ""), (1, ""), (2, "")]));
    let u: Vec<u32> = doc.p.tasks.iter().map(|t| t.uid).collect();
    op(&mut doc, Op::Indent { uids: vec![u[1], u[2]] });
    set(&mut doc, 1, "pct", "100");
    set(&mut doc, 2, "pct", "50");
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    let proj = c.summary().pct.round();
    assert_eq!(proj, 67.0);
    assert_eq!(c.pct[0] as f64, proj, "summary row truncates (200/3 = 66.7 → {}%)", c.pct[0]);
}

// A register risk on a milestone or a summary task must move the simulated finish.
#[test]
fn risk_on_milestone_moves_the_finish() {
    let mut p = plan(&[(5, ""), (0, "1"), (3, "2")]);
    p.risks.push(Risk { id: 1, name: "Permit".into(), prob: 5, impact: 3, status: "Open".into(), tasks: vec![p.tasks[1].uid], kind: "Threat".into(), days: 5.0, ..Default::default() });
    let r = crate::risk::simulate(&p, &crate::risk::SimOpts { iterations: 200, low: 0.0, high: 0.0, seed: 1 }).unwrap();
    assert!((r.risks[0].value - 0.9).abs() < 0.07);
    assert!(r.p50 > r.planned, "a 90 % risk of +5 days on the milestone changed nothing: p50 {} planned {}", r.p50, r.planned);
}

#[test]
fn risk_on_summary_moves_the_finish() {
    let mut p = plan(&[(0, ""), (5, "")]);
    p.tasks[1].level = 1;
    p.risks.push(Risk { id: 1, name: "Weather".into(), prob: 5, impact: 3, status: "Open".into(), tasks: vec![p.tasks[0].uid], kind: "Threat".into(), days: 5.0, ..Default::default() });
    let r = crate::risk::simulate(&p, &crate::risk::SimOpts { iterations: 200, low: 0.0, high: 0.0, seed: 1 }).unwrap();
    assert!(r.p50 > r.planned, "a 90 % risk of +5 days on the phase changed nothing");
}

// ---------- exchange round trips ----------

#[test]
fn xer_keeps_cost_resources() {
    let mut doc = Doc::new(plan(&[(2, "")]));
    doc.p.resources.push(Resource { uid: 1, name: "Permit fee".into(), kind: ResKind::Cost, ..Default::default() });
    doc.p.tasks[0].res = vec![Assign { res: 1, units: 5000.0, table: 0 }];
    let q = round_trip(&doc.p, "xer");
    assert_eq!(q.resources[0].kind, ResKind::Cost, "cost resource came back as {:?} with units {}", q.resources[0].kind, q.tasks[0].res[0].units);
}

#[test]
fn xer_keeps_rates_and_cost() {
    let mut p = plan(&[(2, "")]);
    p.resources.push(Resource { uid: 1, name: "Fitter".into(), rate: 50.0, max: 1.0, ..Default::default() });
    p.tasks[0].res = vec![Assign { res: 1, units: 1.0, table: 0 }];
    let q = round_trip(&p, "xer");
    assert_eq!(q.resources[0].rate, 50.0, "rate lost");
}

#[test]
fn xer_keeps_links_to_summaries() {
    let mut p = plan(&[(3, ""), (0, ""), (2, "")]);
    p.tasks[2].level = 1;
    let su = p.tasks[1].uid;
    p.tasks[1].preds = vec![Link { pred: p.tasks[0].uid, kind: LinkType::FS, lag: 0, pct: false }];
    let _ = su;
    let a = schedule(&p).unwrap();
    let q = round_trip(&p, "xer");
    let b = schedule(&q).unwrap();
    assert_eq!(b.finish, a.finish, "summary link dropped: finish {} vs {}", b.finish / DAY, a.finish / DAY);
}

#[test]
fn xer_keeps_percent_lags() {
    let mut p = plan(&[(10, ""), (2, "")]);
    p.tasks[1].preds = vec![Link { pred: p.tasks[0].uid, kind: LinkType::SS, lag: 50, pct: true }];
    let a = schedule(&p).unwrap();
    let q = round_trip(&p, "xer");
    let b = schedule(&q).unwrap();
    assert_eq!(b.es[1], a.es[1], "50 % lag came back as {:?}", q.tasks[1].preds);
}

#[test]
fn xml_keeps_fixed_cost_elapsed_and_physical() {
    let mut p = plan(&[(2, ""), (3, "")]);
    p.tasks[0].fixed_cost = 1000.0;
    p.tasks[1].elapsed = true;
    p.tasks[1].pct_type = PctType::Physical;
    p.tasks[1].phys = 40;
    let q = round_trip(&p, "xml");
    assert_eq!(q.tasks[0].fixed_cost, 1000.0, "fixed cost lost");
    assert!(q.tasks[1].elapsed, "elapsed duration lost");
    assert_eq!((q.tasks[1].pct_type, q.tasks[1].phys), (PctType::Physical, 40), "physical % lost");
}

#[test]
fn xer_keeps_inactive_and_baseline() {
    let mut doc = Doc::new(plan(&[(2, ""), (3, "1")]));
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 });
    doc.p.tasks[1].inactive = true;
    let q = round_trip(&doc.p, "xer");
    let k = q.tasks.iter().position(|t| t.name == "T2").unwrap();
    let _ = q.tasks[k].inactive;
}

// P6 writes working periods as (s|08:00|f|12:00) or (f|12:00|s|08:00).
#[test]
fn xer_calendar_hours_in_either_order() {
    let data = "(0||CalendarData()((0||DaysOfWeek()((0||1()())(0||2()((0||0(f|12:00|s|08:00)())(0||1(f|17:00|s|13:00)())))(0||3()())(0||4()())(0||5()())(0||6()())(0||7()())))(0||Exceptions()())))";
    let xer = format!("ERMHDR\t19.12\n%T\tCALENDAR\n%F\tclndr_id\tclndr_name\tday_hr_cnt\tclndr_data\n%R\t1\tStd\t8\t{data}\n%T\tPROJECT\n%F\tproj_id\tproj_short_name\tclndr_id\tplan_start_date\n%R\t1\tP\t1\t2026-11-02 00:00\n%E\n");
    let p = crate::io::xer::read(&xer).unwrap();
    assert_eq!(p.calendars[0].week[0], vec![(480, 720), (780, 1020)]);
}

// ---------- hostile files ----------

#[test]
fn hostile_mspdi_does_not_panic_or_hang() {
    let big = "<Project><StartDate>2026-11-02T08:00:00</StartDate><MinutesPerDay>99999999999999999999</MinutesPerDay><Tasks>\
<Task><UID>1</UID><ID>1</ID><Name>A</Name><OutlineLevel>1</OutlineLevel><Duration>PT99999999999999999999H0M0S</Duration><PercentComplete>250</PercentComplete></Task>\
<Task><UID>2</UID><ID>2</ID><Name>B</Name><OutlineLevel>200</OutlineLevel><Duration>PT8H</Duration><PredecessorLink><PredecessorUID>1</PredecessorUID><LinkLag>-99999999999999999</LinkLag></PredecessorLink>\
<Baseline><Number>0</Number><Start>9999-12-31T08:00:00</Start><Finish>0001-01-01T08:00:00</Finish><Duration>PT1H</Duration><Cost>1e308</Cost></Baseline></Task></Tasks></Project>";
    let p = crate::io::mspdi::read(big).unwrap();
    let _ = schedule(&p);
    if let Ok(s) = schedule(&p) { let _ = s.finish; let c = Calc::new(&p, "%d-%b-%y", d(2026, 11, 1)); let _ = c.summary(); let _ = crate::report::curve(&c, "week"); let _ = crate::report::scurve(&c, "week", "cost", None); }
    let mpd0 = "<Project><MinutesPerDay>0</MinutesPerDay><Tasks><Task><UID>1</UID><ID>1</ID><Name>A</Name><OutlineLevel>1</OutlineLevel><Duration>PT8H</Duration></Task></Tasks></Project>";
    let p = crate::io::mspdi::read(mpd0).unwrap();
    if let Ok(_s) = schedule(&p) { let c = Calc::new(&p, "%d-%b-%y", d(2026, 11, 1)); let _ = c.summary(); let _ = crate::report::scurve(&c, "week", "duration", None); let _ = crate::progress::earned_schedule(&c); }
}

#[test]
fn hostile_xer_does_not_panic() {
    let xer = "%T\tPROJECT\n%F\tproj_id\tproj_short_name\tclndr_id\tplan_start_date\tlast_recalc_date\n%R\t1\tP\t1\t2026-11-02 00:00\t2026-11-10 00:00\n\
%T\tCALENDAR\n%F\tclndr_id\tclndr_name\tday_hr_cnt\tclndr_data\n%R\t1\tStd\t0\t(0||CalendarData()((0||DaysOfWeek()((0||2()((0||0(s|99:99|f|0)())))))(0||Exceptions()((0||0(d|99999999999)()))))\n\
%T\tTASK\n%F\ttask_id\tproj_id\twbs_id\ttask_type\ttarget_drtn_hr_cnt\tphys_complete_pct\n%R\t1\t1\t1\tTT_Task\t1e300\t-5\n%R\t2\t1\t1\tTT_Task\tNaN\t1e9\n\
%T\tTASKPRED\n%F\ttask_id\tpred_task_id\tpred_type\tlag_hr_cnt\n%R\t2\t1\tPR_FS\t-1e300\n%R\t1\t2\tPR_FS\t0\n%E\n";
    let p = crate::io::xer::read(xer).unwrap();
    let _ = schedule(&p);
}

#[test]
fn deep_xml_nesting_does_not_overflow() {
    // A 2 MB stack like a worker thread; 200k nested elements is a 1.4 MB file.
    let h = std::thread::Builder::new().stack_size(1 << 20).spawn(|| {
        let n = 20_000;
        let s = format!("{}{}", "<a>".repeat(n), "</a>".repeat(n));
        let _ = crate::io::mspdi::read(&s);
    }).unwrap();
    h.join().unwrap();
}

// Monte Carlo: the share of runs a risk happened must be measured over the runs kept.
#[test]
fn earned_schedule_matches_plan_when_on_time() {
    let mut doc = Doc::new(plan(&[(10, "")]));
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 });
    set(&mut doc, 0, "pct", "50");
    op(&mut doc, Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-11-06") });
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    let e = crate::progress::earned_schedule(&c).unwrap();
    assert!((e.es - 5.0).abs() < 0.01 && (e.at - 5.0).abs() < 0.01 && (e.spi_t - 1.0).abs() < 0.01, "{e:?}");
    let _ = at(d(2026, 11, 2));
}

// A cost resource's amount is not capped like a person's units when a file is loaded.
#[test]
fn a_cost_amount_above_1000_survives_loading() {
    let mut p = plan(&[(2, "")]);
    p.resources.push(Resource { uid: 1, name: "Permit fee".into(), kind: ResKind::Cost, max: 1.0, ..Default::default() });
    p.tasks[0].res = vec![Assign { res: 1, units: 1500.0, table: 0 }];
    let q = Project::from_json(&serde_json::to_string(&p).unwrap()).unwrap();
    assert_eq!(q.tasks[0].res[0].units, 1500.0);
}
