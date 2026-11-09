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
    p.tasks[1].preds = vec![Link { pred: p.tasks[0].uid, kind: LinkType::FS, lag: 0, pct: false, elapsed: false }];
    let _ = su;
    let a = schedule(&p).unwrap();
    let q = round_trip(&p, "xer");
    let b = schedule(&q).unwrap();
    assert_eq!(b.finish, a.finish, "summary link dropped: finish {} vs {}", b.finish / DAY, a.finish / DAY);
}

// A milestone at the end of a day (a contract date held by Finish No Earlier Than) stays on its day,
// and a deadline stays (in XER as a secondary constraint).
#[test]
fn xer_keeps_finish_milestone_on_its_day_and_deadlines() {
    let mut p = plan(&[(5, ""), (0, "")]);
    (p.tasks[1].constraint, p.tasks[1].cdate) = (Constraint::FNET, Some(d(2027, 3, 5)));
    p.tasks[0].deadline = Some(d(2026, 11, 20));
    let a = schedule(&p).unwrap();
    for ext in ["xer", "xml"] {
        let q = round_trip(&p, ext);
        let b = schedule(&q).unwrap();
        assert_eq!((b.es[1], b.ef[1]), (a.es[1], a.ef[1]), "{ext}: milestone moved from {} to {}", a.es[1], b.es[1]);
        // A deadline (finish on or before) is kept, and with it the float it sets.
        assert_eq!((q.tasks[0].deadline, b.tf[0]), (p.tasks[0].deadline, a.tf[0]), "{ext}: deadline lost");
    }
}

// A task held by a task in another plan opens at the same dates (the file cannot hold the link).
#[test]
fn exports_keep_dates_held_by_other_plans() {
    // A summary held by the other plan, two tasks under it, a task that mirrors one there and a milestone.
    let mut p = plan(&[(0, ""), (3, ""), (2, "2"), (1, ""), (0, "")]);
    (p.tasks[1].level, p.tasks[2].level) = (1, 1);
    let f = at(d(2027, 2, 10)) + 17 * 60;
    let x = |kind, start, finish| ExtLink { file: "other.dplan".into(), uid: 7, kind, lag: 0, elapsed: false, plan: "Other".into(), code: String::new(), name: "X".into(), start, finish };
    p.tasks[0].ext = vec![x(LinkType::FS, f - 5 * DAY, f)];
    p.tasks[3].ext = vec![x(LinkType::SS, at(d(2027, 4, 5)) + 480, at(d(2027, 4, 16)) + 1020), x(LinkType::FF, at(d(2027, 4, 5)) + 480, at(d(2027, 4, 16)) + 1020)];
    p.tasks[4].ext = vec![x(LinkType::FS, f - 5 * DAY, f)];
    p.normalize();
    let a = schedule(&p).unwrap();
    assert!(a.es[1] > at(d(2027, 2, 10)) && a.es[4] == f, "the links hold the plan");
    for ext in ["xer", "xml"] {
        let q = round_trip(&p, ext);
        let b = schedule(&q).unwrap();
        for t in &p.tasks {
            let (i, j) = (p.idx(t.uid).unwrap(), q.tasks.iter().position(|u| u.name == t.name).unwrap());
            assert_eq!((b.es[j], b.ef[j]), (a.es[i], a.ef[i]), "{ext}: {} moved", t.name);
        }
    }
}

#[test]
fn xer_keeps_percent_lags() {
    let mut p = plan(&[(10, ""), (2, "")]);
    p.tasks[1].preds = vec![Link { pred: p.tasks[0].uid, kind: LinkType::SS, lag: 50, pct: true, elapsed: false }];
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

#[test]
fn xer_keeps_physical_percent() {
    let mut p = plan(&[(4, ""), (4, "")]);
    for t in &mut p.tasks { t.act_start = Some(d(2026, 11, 2)) }
    (p.tasks[0].pct_type, p.tasks[0].phys, p.tasks[0].pct) = (PctType::Physical, 40, 25);
    p.tasks[1].pct = 50;
    let q = round_trip(&p, "xer");
    assert_eq!((q.tasks[0].pct_type, q.tasks[0].phys, q.tasks[0].pct), (PctType::Physical, 40, 25), "physical % written as the duration %");
    assert_eq!((q.tasks[1].pct_type, q.tasks[1].pct), (PctType::Duration, 50));
}

// PMXML, like XER: a link to a summary holds every activity under it; a percent lag is hours.
#[test]
fn pmxml_keeps_links_to_summaries_and_percent_lags() {
    let mut p = plan(&[(10, ""), (0, ""), (2, ""), (2, "")]);
    p.tasks[2].level = 1;
    p.tasks[1].preds = vec![Link { pred: p.tasks[0].uid, kind: LinkType::FS, lag: 0, pct: false, elapsed: false }];
    p.tasks[3].preds = vec![Link { pred: p.tasks[0].uid, kind: LinkType::SS, lag: 50, pct: true, elapsed: false }];
    let x = crate::io::formats::pmxml(&Calc::new(&p, "%d-%b-%y", d(2026, 11, 1)));
    let rel = |a: usize, b: usize| format!("<PredecessorActivityObjectId>{}</PredecessorActivityObjectId><SuccessorActivityObjectId>{}</SuccessorActivityObjectId>", p.tasks[a].uid, p.tasks[b].uid);
    assert!(x.contains(&rel(0, 2)), "link to the summary dropped");
    assert!(x.contains(&format!("{}<Type>Start to Start</Type><Lag>40</Lag>", rel(0, 3))), "50 % of 10 days is 40 h");
}

#[test]
fn mpx_writes_lags_and_assignments_by_resource_id() {
    let mut p = plan(&[(10, ""), (2, ""), (2, "")]);
    p.tasks[1].preds = vec![Link { pred: p.tasks[0].uid, kind: LinkType::FS, lag: 2 * D, pct: false, elapsed: false }];
    p.tasks[2].preds = vec![Link { pred: p.tasks[0].uid, kind: LinkType::SS, lag: 50, pct: true, elapsed: false }];
    p.resources.push(Resource { uid: 7, name: "Fitter".into(), rate: 50.0, ..Default::default() });
    p.tasks[1].res = vec![Assign { res: 7, units: 1.0, table: 0 }];
    let x = crate::io::formats::mpx(&Calc::new(&p, "%d-%b-%y", d(2026, 11, 1)));
    assert!(x.contains(",1FS+2d,") && x.contains(",1SS+5d,"), "lags lost:\n{x}");
    // Resources carry ID and Unique ID; an assignment names both (ID 1, units 1 = 100 %, Unique ID 7).
    assert!(x.contains("41,40,49,1,42,44\r\n50,1,7,Fitter,50/h,0\r\n"), "{x}");
    assert!(x.contains("\r\n75,1,1,,,,,,,,,,,7\r\n"), "{x}");
}

// A tiny .xls or .ods with cells far apart must be an error, not a sheet hundreds of GB big.
fn xls(path: &std::path::Path, cells: &[(u16, u16)]) {
    use std::io::Write;
    let rec = |o: &mut Vec<u8>, ty: u16, d: &[u8]| { o.extend(ty.to_le_bytes()); o.extend((d.len() as u16).to_le_bytes()); o.extend(d) };
    let bof = |dt: u16| [&0x0600u16.to_le_bytes()[..], &dt.to_le_bytes(), &[0; 12]].concat();
    let mut s = vec![];
    rec(&mut s, 0x0809, &bof(5));
    let at = s.len() + 4 + 13 + 4;
    rec(&mut s, 0x0085, &[&(at as u32).to_le_bytes()[..], &[0, 0, 5, 0], b"Sheet"].concat());
    rec(&mut s, 0x000A, &[]);
    rec(&mut s, 0x0809, &bof(0x10));
    for &(r, c) in cells { rec(&mut s, 0x0203, &[&r.to_le_bytes()[..], &c.to_le_bytes(), &[0, 0], &1.0f64.to_le_bytes()].concat()) }
    rec(&mut s, 0x000A, &[]);
    let mut f = cfb::create(path).unwrap();
    f.create_stream("Workbook").unwrap().write_all(&s).unwrap();
    f.flush().unwrap();
}
fn ods(path: &std::path::Path, rows: &str) {
    use std::io::Write;
    let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    z.start_file("mimetype", o).unwrap();
    z.write_all(b"application/vnd.oasis.opendocument.spreadsheet").unwrap();
    z.start_file("META-INF/manifest.xml", o).unwrap();
    z.write_all(br#"<?xml version="1.0"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0"/>"#).unwrap();
    z.start_file("content.xml", o).unwrap();
    write!(z, r#"<?xml version="1.0"?><office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0"><office:body><office:spreadsheet><table:table table:name="S">{rows}</table:table></office:spreadsheet></office:body></office:document-content>"#).unwrap();
    z.finish().unwrap();
}

#[test]
fn xls_and_ods_with_far_apart_cells_are_refused() {
    let dir = std::env::temp_dir().join(format!("datum-far-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("a.xls");
    // A small sheet reads; one cell at row 65,536 of column IV (16.7 million cells) does not.
    xls(&f, &[(0, 0), (1, 1)]);
    assert_eq!(crate::io::table::read(&f).map(|t| t.rows.len()), Ok(1));
    xls(&f, &[(0, 0), (65_535, 255)]);
    assert!(crate::io::table::read(&f).is_err_and(|e| e.contains("10 million cells")));
    let f = dir.join("a.ods");
    let cell = r#"<table:table-cell office:value-type="float" office:value="1"/>"#;
    ods(&f, &format!("<table:table-row>{cell}</table:table-row><table:table-row><table:table-cell/>{cell}</table:table-row>"));
    assert_eq!(crate::io::table::read(&f).map(|t| t.rows.len()), Ok(1));
    // Many rows that each reach column XFD: calamine would keep 16,384 cells for every one.
    let far = format!(r#"<table:table-row><table:table-cell table:number-columns-repeated="16383"/>{cell}</table:table-row>"#);
    ods(&f, &format!("<table:table-row>{cell}</table:table-row>{}", far.repeat(700)));
    assert!(crate::io::table::read(&f).is_err_and(|e| e.contains("10 million cells")));
    std::fs::remove_dir_all(dir).ok();
}

// XER files write working periods as (s|08:00|f|12:00) or (f|12:00|s|08:00).
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

// The plan keeps its own pictures (logos of the parties, images) and the pictures placed on sheets.
#[test]
fn pictures_are_kept_in_the_plan_and_placed_on_sheets() {
    let mut doc = Doc::new(plan(&[(2, "")]));
    let png = "data:image/png;base64,iVBORw0KGgo=".to_string();
    op(&mut doc, Op::Picture { id: 0, pic: Some(Picture { id: 0, name: "Client".into(), role: "Client".into(), data: png.clone() }) });
    op(&mut doc, Op::Picture { id: 0, pic: Some(Picture { id: 0, name: "Vendor".into(), role: "Sub-vendor".into(), data: png.clone() }) });
    assert_eq!(doc.p.pictures.iter().map(|x| x.id).collect::<Vec<_>>(), vec![1, 2]);
    assert!(doc.apply(Op::Picture { id: 0, pic: Some(Picture { data: "javascript:alert(1)".into(), ..Default::default() }) }, "%d-%b-%y").is_err(), "only images");
    op(&mut doc, Op::Sheet { k: None, sheet: Some(SheetData { name: "Board".into(), ..Default::default() }) });
    op(&mut doc, Op::SheetImages { k: 0, images: vec![SheetImage { pic: 2, cell: "B2".into(), w: 120, h: 60, fit: false }, SheetImage { pic: 9, cell: "C3".into(), w: 1, h: 1, fit: false }] });
    assert_eq!(doc.p.sheets[0].images, vec![SheetImage { pic: 2, cell: "B2".into(), w: 120, h: 60, fit: false }], "unknown pictures are left out");
    // Renaming the sheet keeps its pictures; saving and opening keeps everything.
    let s = SheetData { name: "Client board".into(), ..doc.p.sheets[0].clone() };
    op(&mut doc, Op::Sheet { k: Some(0), sheet: Some(SheetData { images: vec![], ..s }) });
    assert_eq!(doc.p.sheets[0].images.len(), 1);
    let q = Project::from_json(&serde_json::to_string(&doc.p).unwrap()).unwrap();
    assert_eq!((q.pictures.len(), q.sheets[0].images.len()), (2, 1));
    // Removing a picture removes it from the sheets too.
    op(&mut doc, Op::Picture { id: 2, pic: None });
    assert!(doc.p.sheets[0].images.is_empty() && doc.p.pictures.len() == 1);
}

// A sheet's pictures go into the Excel (.xlsx) workbook at their cell.
#[test]
fn sheet_pictures_go_into_the_workbook() {
    let mut doc = Doc::new(plan(&[(2, "")]));
    // A 1×1 PNG.
    let png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
    op(&mut doc, Op::Picture { id: 0, pic: Some(Picture { id: 0, name: "Logo".into(), role: "Client".into(), data: png.into() }) });
    op(&mut doc, Op::Sheet { k: None, sheet: Some(SheetData { name: "Board".into(), ..Default::default() }) });
    op(&mut doc, Op::SheetImages { k: 0, images: vec![SheetImage { pic: 1, cell: "B2".into(), w: 80, h: 40, fit: true }] });
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let f = std::env::temp_dir().join(format!("dp-pics-{}-{}.xlsx", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    crate::io::export(&doc.p, &f, "%d-%b-%y", d(2026, 11, 1), &[]).unwrap();
    let bytes = std::fs::read(&f).unwrap();
    std::fs::remove_file(&f).ok();
    let zip = String::from_utf8_lossy(&bytes);
    assert!(zip.contains("xl/media/image1.png"), "the picture is in the workbook");
}

// Sheets laid out for a client: merged titles, borders, wrapped text, row heights and tab colours
// are kept, follow inserted rows, and go into the workbook.
#[test]
fn sheet_layout_is_kept_and_exported() {
    let mut doc = Doc::new(plan(&[(2, "")]));
    let st = "b;w;v:t;bt:thin #000000;bb:dbl #0B5CAD".to_string();
    let sheet = SheetData { name: "Board".into(), cells: [("A1".to_string(), "Status report".to_string()), ("A3".into(), "=1+1".into())].into(), style: [("A1".to_string(), st)].into(),
        merges: vec!["A1:D2".into(), "B2:C3".into(), "E5".into(), "zz".into()], heights: [(0, 40), (9, 0)].into(), color: "#C00000".into(), ..Default::default() };
    op(&mut doc, Op::Sheet { k: None, sheet: Some(sheet) });
    let s = &doc.p.sheets[0];
    assert_eq!(s.merges, vec!["A1:D2"], "overlapping, single-cell and invalid merges are dropped");
    assert_eq!((s.heights.get(&0), s.heights.len(), s.color.as_str()), (Some(&40), 1, "#C00000"));
    op(&mut doc, Op::SheetInsert { k: 0, cols: false, at: 0, count: 2 });
    let s = &doc.p.sheets[0];
    assert_eq!((s.merges.clone(), s.heights.get(&2)), (vec!["A3:D4".to_string()], Some(&40)), "merges and heights follow inserted rows");
    op(&mut doc, Op::Sheet { k: None, sheet: Some(SheetData { name: "Second".into(), ..Default::default() }) });
    op(&mut doc, Op::SheetMove { from: 1, to: 0 });
    assert_eq!(doc.p.sheets.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Second", "Board"]);
    doc.undo();
    assert_eq!(doc.p.sheets[0].name, "Board", "a move undoes");
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let f = std::env::temp_dir().join(format!("dp-layout-{}-{}.xlsx", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    crate::io::export(&doc.p, &f, "%d-%b-%y", d(2026, 11, 1), &[]).unwrap();
    let bytes = std::fs::read(&f).unwrap();
    std::fs::remove_file(&f).ok();
    assert!(bytes.len() > 1000, "the workbook is written with merges, borders and heights");
    // The sheet's own page setup is kept (and cleaned), and goes into the workbook.
    let mut s = doc.p.sheets[0].clone();
    s.page = Some(Box::new(SheetPage { paper: "A3".into(), orient: "landscape".into(), fit_w: 1, area: "A1:D20".into(), titles: "1:2".into(), no_hf: true, breaks: vec![12, 0, 12, 5], scale: 9999, ..Default::default() }));
    op(&mut doc, Op::Sheet { k: Some(0), sheet: Some(s) });
    let pg = doc.p.sheets[0].page.clone().unwrap();
    assert_eq!((pg.breaks, pg.scale, pg.area.as_str(), pg.no_hf), (vec![5, 12], 400, "A1:D20", true));
    crate::io::export(&doc.p, &f, "%d-%b-%y", d(2026, 11, 1), &[]).unwrap();
    let bytes = std::fs::read(&f).unwrap();
    std::fs::remove_file(&f).ok();
    assert!(bytes.len() > 1000, "the workbook is written with the sheet's page setup");
}

// A sheet saved as a workbook and brought back: values, real dates, formulas, number formats, styles,
// merges, sizes, tab colour, frozen panes, page setup and flags all come back as they were.
#[test]
fn sheets_go_to_a_workbook_and_back() {
    let mut doc = Doc::new(plan(&[(2, "")]));
    let cells: Vec<(String, String)> = [("A1", "Status report"), ("B2", "13/01/2026"), ("C2", "=B2+7"), ("D2", "Germany"), ("E2", "1234.5"), ("F2", "=SUM(E2,10)"), ("G2", "001"), ("A3", "12%")]
        .iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    let sheet = SheetData { name: "Board".into(), cells: cells.into_iter().collect(), style: [("A1".to_string(), "b;c:#FFFFFF;f:#0B5CAD;a:c;bb:thick #C00000".to_string())].into(),
        fmt: [("D2".to_string(), "flag".to_string()), ("E2".to_string(), "money".to_string()), ("G2".to_string(), "@".to_string())].into(),
        hide_rows: vec![7], hide_cols: vec![8], merges: vec!["A1:C1".into()], widths: [(0, 180)].into(), cond: vec![CondRule { range: "E2:E9".into(), op: "gt".into(), a: "1000".into(), style: "c:#9C0006;f:#FFC7CE".into(), ..Default::default() }, CondRule { range: "F2:F9".into(), op: "bar".into(), style: "#2F6FDE".into(), ..Default::default() }, CondRule { range: "B2:B9".into(), op: "date".into(), a: "past".into(), style: "c:#C00000".into(), ..Default::default() }, CondRule { range: "A1".into(), op: "nonsense".into(), ..Default::default() }], heights: [(0, 40)].into(), color: "#00B050".into(),
        page: Some(Box::new(SheetPage { paper: "A3".into(), orient: "landscape".into(), fit_w: 1, area: "A1:G9".into(), titles: "1:1".into(), no_hf: true, center: true, ..Default::default() })), ..Default::default() };
    op(&mut doc, Op::Sheet { k: None, sheet: Some(sheet) });
    op(&mut doc, Op::Ui { key: "freeze".into(), value: serde_json::json!({ "Board": [1, 1] }) });
    let png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
    op(&mut doc, Op::Picture { id: 0, pic: Some(Picture { id: 0, name: "Logo".into(), role: "Client".into(), data: png.into() }) });
    op(&mut doc, Op::SheetImages { k: 0, images: vec![SheetImage { pic: 1, cell: "E4".into(), w: 90, h: 45, fit: true }] });
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let f = std::env::temp_dir().join(format!("dp-rt-{}-{}.xlsx", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let c = Calc::new(&doc.p, "%d/%m/%Y", d(2026, 11, 2));
    crate::io::table::write_sheets_xlsx(&c, &f).unwrap();
    if let Ok(keep) = std::env::var("DP_RT_OUT") { std::fs::copy(&f, keep).ok(); }
    let back = crate::io::sheetbook::import(&f).unwrap();
    std::fs::remove_file(&f).ok();
    assert_eq!(back.sheets.len(), 1, "only the sheet tab comes back, not the plan's own data sheets");
    let s = &back.sheets[0];
    assert_eq!(s.cells.get("C2").map(String::as_str), Some("=B2+7"), "formulas come back");
    assert_eq!(s.cells.get("B2").map(String::as_str), Some("46035"), "a typed date is a real date in the workbook");
    assert!(s.fmt.get("B2").is_some_and(|f| f.contains("yy")), "with a date format: {:?}", s.fmt.get("B2"));
    assert_eq!((s.fmt.get("D2").map(String::as_str), s.cells.get("D2").map(String::as_str)), (Some("flag"), Some("Germany")), "flags come back");
    assert!(s.fmt.get("E2").is_some_and(|f| f.contains("#,##0")), "money keeps a money format: {:?}", s.fmt.get("E2"));
    assert_eq!((s.cells.get("G2").map(String::as_str), s.fmt.get("G2").map(String::as_str)), (Some("001"), Some("@")), "text stays text");
    assert!(s.fmt.get("A3").is_some_and(|f| f.contains('%')), "a typed percentage keeps its look");
    let st = s.style.get("A1").cloned().unwrap_or_default();
    assert!(st.contains('b') && st.contains("f:#0B5CAD") && st.contains("c:#FFFFFF") && st.contains("a:c") && st.contains("bb:thick #C00000"), "styles come back: {st}");
    assert_eq!(s.merges, vec!["A1:C1".to_string()]);
    assert_eq!((s.hide_rows.clone(), s.hide_cols.clone()), (vec![7], vec![8]), "hidden rows and columns come back");
    assert_eq!(s.cond.iter().map(|c| c.op.as_str()).collect::<Vec<_>>(), ["gt", "bar", "date"], "conditional formatting comes back (an unknown rule was dropped)");
    assert_eq!((s.widths.get(&0).copied().map(|w| (w as i32 - 180).abs() <= 8), s.heights.get(&0).copied().map(|h| (h as i32 - 40).abs() <= 2)), (Some(true), Some(true)), "sizes: {:?} {:?}", s.widths, s.heights);
    assert_eq!(s.color, "#00B050");
    assert_eq!(back.freeze, vec![("Board".to_string(), 1, 1)]);
    let pg = s.page.clone().unwrap();
    assert_eq!((pg.paper.as_str(), pg.orient.as_str(), pg.fit_w, pg.area.as_str(), pg.titles.as_str(), pg.no_hf, pg.center), ("A3", "landscape", 1, "A1:G9", "1:1", true, true));
    assert_eq!((back.pictures.len(), s.images.len(), s.images.first().map(|i| (i.cell.as_str(), i.fit))), (1, 1, Some(("E4", true))), "pictures come back at their cell");
    assert!(back.pictures[0].data.starts_with("data:image/png;base64,"));
    // Brought into a plan: one step, a free name.
    op(&mut doc, Op::AddSheets { sheets: back.sheets, pictures: back.pictures, freeze: back.freeze });
    assert_eq!(doc.p.sheets.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Board", "Board (2)"]);
    assert_eq!(doc.p.ui["freeze"]["Board (2)"], serde_json::json!([1, 1]));
    doc.undo();
    assert_eq!(doc.p.sheets.len(), 1, "an import undoes in one step");
}

// Writing and reading XER, PMXML and MPX take time in step with the plan's size, not its square
// (tasks were found by a search through the whole plan for every link and assignment).
#[test]
fn exchange_time_grows_with_the_plan() {
    let time = |n: usize| {
        let mut p = Project::new("Big", d(2026, 11, 2));
        p.resource("Crew");
        for i in 0..n {
            let mut t = p.new_task(&format!("T{i}"), (i % 50 != 0) as u8, if i % 50 == 0 { 0 } else { 480 });
            if i % 50 > 1 { t.preds.push(Link { pred: t.uid - 1, kind: LinkType::FS, lag: 0, pct: false, elapsed: false }) }
            t.res = vec![Assign { res: 1, units: 1.0, table: 0 }];
            p.tasks.push(t);
        }
        let s = schedule(&p).unwrap();
        let x = crate::io::xer::write(&p, &s, d(2026, 11, 1));
        let c = Calc::new(&p, "%d-%b-%y", d(2026, 11, 1));
        (0..2).map(|_| {
            let t = std::time::Instant::now();
            std::hint::black_box((crate::io::xer::write(&p, &s, d(2026, 11, 1)), crate::io::xer::read(&x).unwrap(), crate::io::formats::pmxml(&c), crate::io::formats::mpx(&c)));
            t.elapsed().as_secs_f64()
        }).fold(f64::MAX, f64::min)
    };
    let (a, b) = (time(2000), time(16_000));
    assert!(b < a * 16.0, "8x the tasks took {:.1}x the time", b / a);
}

// Elapsed lags go through Project XML (LagFormat edays) and CSV as they are; XER has no elapsed
// lags, so they become working time of the same share of a week, and the export says so.
#[test]
fn elapsed_lags_in_files() {
    let p = plan(&[(5, ""), (2, "1FS+7ed"), (1, "2SS-2ed")]);
    let lags = |q: &Project| q.tasks.iter().flat_map(|t| t.preds.iter().map(|l| (l.lag, l.elapsed))).collect::<Vec<_>>();
    assert_eq!(lags(&p), [(7 * DAY, true), (-2 * DAY, true)]);
    let x = round_trip(&p, "xml");
    assert_eq!(lags(&x), lags(&p));
    let (s, sx) = (schedule(&p).unwrap(), schedule(&x).unwrap());
    assert_eq!((&s.es, &s.ef), (&sx.es, &sx.ef));
    // XER: 7 elapsed days are 5 working days (40 h); −2 elapsed days are −685 working minutes (−2 × 2400 / 7).
    assert_eq!(lags(&round_trip(&p, "xer")), [(5 * D, false), (-685, false)]);
    let note = crate::io::export_notes(&p, std::path::Path::new("plan.xer"));
    assert!(note.len() == 1 && note[0].starts_with("2 elapsed lags"), "{note:?}");
    assert!(crate::io::export_notes(&p, std::path::Path::new("plan.xml")).is_empty());
    // MPX writes them in elapsed days.
    let mpx = crate::io::formats::mpx(&Calc::new(&p, "%d-%b-%y", d(2026, 11, 2)));
    assert!(mpx.contains("1FS+7ed") && mpx.contains("2SS-2ed"), "{mpx}");
    // CSV: the Predecessors text carries them.
    let dir = std::env::temp_dir().join(format!("datum-elag-csv-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("plan.csv");
    crate::io::export(&p, &f, "%d-%b-%y", d(2026, 11, 1), &[]).unwrap();
    let crate::io::Imported::Table(t) = crate::io::import(&f).unwrap() else { panic!() };
    std::fs::remove_dir_all(dir).ok();
    let (q, bad) = crate::io::table::to_project(&t, &t.guess, "Q", d(2026, 11, 2), "%d-%b-%y").unwrap();
    assert_eq!((bad, lags(&q)), (0, lags(&p)));
}

#[test]
fn project_work_with_no_resource_goes_back_unassigned() {
    // As Microsoft Project writes it: 40 h on a 2-day task, 0 h on another, both on resource -65535.
    let xml = r#"<Project xmlns="http://schemas.microsoft.com/project"><StartDate>2026-11-02T08:00:00</StartDate><StatusDate>2026-11-04T17:00:00</StatusDate>
<MoveRemainingStartsForward>0</MoveRemainingStartsForward><Tasks>
<Task><UID>1</UID><ID>1</ID><Name>A</Name><OutlineLevel>1</OutlineLevel><Duration>PT16H0M0S</Duration><DurationFormat>7</DurationFormat><Work>PT40H0M0S</Work></Task>
<Task><UID>2</UID><ID>2</ID><Name>B</Name><OutlineLevel>1</OutlineLevel><Duration>PT16H0M0S</Duration><DurationFormat>7</DurationFormat><Work>PT0H0M0S</Work></Task></Tasks>
<Resources><Resource><UID>0</UID><ID>0</ID><IsNull>0</IsNull></Resource></Resources><Assignments>
<Assignment><UID>1</UID><TaskUID>1</TaskUID><ResourceUID>-65535</ResourceUID><Units>1</Units><Work>PT40H0M0S</Work></Assignment>
<Assignment><UID>2</UID><TaskUID>2</TaskUID><ResourceUID>-65535</ResourceUID><Units>1</Units><Work>PT16H0M0S</Work></Assignment></Assignments></Project>"#;
    let (p, _) = io::mspdi::read_notes(xml).unwrap();
    assert!(p.opts.msp_status, "a Project file measures progress only");
    assert_eq!(p.resources.len(), 1);
    assert!(p.resources[0].unassigned);
    assert_eq!(p.tasks[0].res[0].units, 2.5);
    assert!(p.tasks[1].res.is_empty(), "no work, no assignment");
    let s = schedule(&p).unwrap();
    let out = io::mspdi::write(&p, &s, &vec![0.0; p.tasks.len()]);
    assert!(out.contains("<ResourceUID>-65535</ResourceUID>") && out.contains("<Work>PT40H0M0S</Work>"));
    assert!(!out.contains("Project work"), "the stand-in resource is not written");
    assert!(out.contains("<MoveRemainingStartsForward>0</MoveRemainingStartsForward>"));
    let (q, _) = io::mspdi::read_notes(&out).unwrap();
    assert_eq!((q.resources.len(), q.tasks[0].res[0].units, q.opts.msp_status), (1, 2.5, true));
}
