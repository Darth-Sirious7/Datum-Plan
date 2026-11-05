use crate::{cal::{at, date_of, WorkCal, DAY}, doc::{copy, Doc, Op}, fields::Calc, load, parse, sched::schedule, *};
use chrono::NaiveDate;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}
const H: i64 = 60;
const D: i64 = 480;

/// Leaf tasks with (working days, preds) on the standard calendar starting Monday 2 Nov 2026.
fn plan(rows: &[(i64, &str)]) -> Project {
    let mut p = Project::new("T", d(2026, 11, 2));
    for (i, &(dur, _)) in rows.iter().enumerate() {
        let t = p.new_task(&format!("T{}", i + 1), 0, dur * D);
        p.tasks.push(t);
    }
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    for (t, &(_, pr)) in p.tasks.iter_mut().zip(rows) {
        t.preds = parse::preds(pr, &ids, D).unwrap();
    }
    p
}

/// "dd hh:mm" of an instant, for readable assertions.
fn when(t: i64) -> String {
    format!("{} {:02}:{:02}", date_of(t).format("%d"), t.rem_euclid(DAY) / 60, t.rem_euclid(60))
}

fn op(doc: &mut Doc, o: Op) -> Result<(), String> {
    doc.apply(o, "%d-%b-%y")
}

#[test]
fn calendar_arithmetic() {
    let c = WorkCal::new(&Calendar::standard("S"));
    let mon = at(d(2026, 11, 2));
    assert_eq!(when(c.next_start(mon)), "02 08:00");
    assert_eq!(when(c.add(mon + 8 * H, 5 * H)), "02 14:00"); // lunch skipped
    assert_eq!(when(c.add(mon + 8 * H, 5 * D)), "06 17:00");
    assert_eq!(when(c.add(at(d(2026, 11, 6)) + 17 * H, 1)), "09 08:01"); // over the weekend
    assert_eq!(when(c.sub(at(d(2026, 11, 9)) + 8 * H, H)), "06 16:00");
    assert_eq!(c.between(mon, mon + 7 * DAY), 5 * D);
    assert_eq!(when(c.prev_end(at(d(2026, 11, 9)))), "06 17:00");
    let mut hol = Calendar::standard("H");
    hol.exceptions.push(Exception { name: "Holiday".into(), from: d(2026, 11, 3), to: d(2026, 11, 4), hours: vec![] });
    let c = WorkCal::new(&hol);
    assert_eq!(when(c.add(mon + 8 * H, 2 * D)), "05 17:00");
}

#[test]
fn forward_backward_and_float() {
    // 1 → 2 → 4 and 1 → 3 → 4; 3 is shorter so it has float.
    let p = plan(&[(2, ""), (5, "1"), (2, "1"), (1, "2,3")]);
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.es[1]), "04 08:00");
    assert_eq!(when(s.ef[1]), "10 17:00");
    assert_eq!(when(s.es[3]), "11 08:00");
    assert_eq!(s.tf, vec![0, 0, 3 * D, 0]);
    assert_eq!(s.ff[2], 3 * D);
}

#[test]
fn link_types_lags_and_hours() {
    let mut p = plan(&[(4, ""), (3, "1SS+2d"), (3, "1FF"), (2, "1FS-1d"), (0, "1")]);
    p.tasks[1].preds[0].lag = 4 * H; // SS+4h
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.es[1]), "02 13:00");
    assert_eq!(when(s.ef[2]), "05 17:00");
    assert_eq!(when(s.es[3]), "05 08:00");
    assert_eq!(when(s.es[4]), "05 17:00"); // a milestone sits at its predecessor's finish
}

#[test]
fn constraints_deadlines_and_alap() {
    let mut p = plan(&[(2, ""), (2, "1"), (1, "")]);
    p.tasks[0].constraint = Constraint::SNET;
    p.tasks[0].cdate = Some(d(2026, 11, 4));
    p.tasks[1].deadline = Some(d(2026, 11, 6));
    p.tasks[2].constraint = Constraint::ALAP;
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.es[0]), "04 08:00");
    assert_eq!(when(s.ef[1]), "09 17:00");
    assert_eq!(s.tf[1], -D); // a missed deadline gives negative float
    assert_eq!(when(s.es[2]), "09 08:00"); // as late as possible
    p.tasks[0].constraint = Constraint::MFO;
    p.tasks[0].cdate = Some(d(2026, 11, 3));
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.ef[0]), "03 17:00");
}

#[test]
fn task_calendars_summary_links_and_inactive() {
    let mut p = plan(&[(0, ""), (2, ""), (2, "2"), (1, "1"), (3, "")]);
    p.calendars.push(Calendar::full("24 Hours"));
    p.tasks[1].level = 1;
    p.tasks[2].level = 1;
    p.tasks[2].cal = Some("24 Hours".into());
    p.tasks[4].inactive = true;
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.ef[2]), "04 09:00"); // 16 working hours on a 24-hour calendar
    assert_eq!(when(s.es[3]), "04 09:00"); // the link from the summary waits for all its tasks
    assert_eq!(s.finish, s.ef[3]); // the inactive task does not extend the project
    p.tasks[1].preds = vec![Link { pred: p.tasks[0].uid, kind: LinkType::FS, lag: 0, pct: false, elapsed: false }];
    assert!(schedule(&p).err().unwrap().contains("own summary"));
}

#[test]
fn progress_against_the_status_date() {
    let mut p = plan(&[(4, ""), (4, "1"), (2, "")]);
    p.status = Some(d(2026, 11, 4));
    p.tasks[0].act_start = Some(d(2026, 11, 2));
    p.tasks[0].pct = 50;
    p.tasks[2].act_start = Some(d(2026, 11, 2));
    p.tasks[2].act_finish = Some(d(2026, 11, 3));
    p.tasks[2].pct = 100;
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.ef[0]), "06 17:00"); // 2 days remain after the status date
    assert_eq!(when(s.ef[2]), "03 17:00");
    let mut doc = Doc::new(p);
    let uid = doc.p.tasks[1].uid;
    op(&mut doc, Op::Set { uid, field: "pct".into(), value: "100".into() }).unwrap();
    assert!(doc.p.tasks[1].act_start.is_some() && doc.p.tasks[1].act_finish.is_some());
    op(&mut doc, Op::Set { uid, field: "pct".into(), value: "0".into() }).unwrap();
    assert!(doc.p.tasks[1].act_start.is_none());
    let u0 = doc.p.tasks[0].uid;
    op(&mut doc, Op::MarkOnTrack { uids: vec![u0, uid] }).unwrap();
    assert_eq!(doc.p.tasks[1].pct, 0);
}

#[test]
fn edits_undo_and_cycles() {
    let mut doc = Doc::new(plan(&[(1, ""), (1, "1"), (1, ""), (1, "")]));
    let u: Vec<u32> = doc.p.tasks.iter().map(|t| t.uid).collect();
    assert!(op(&mut doc, Op::Set { uid: u[0], field: "preds".into(), value: "2".into() }).unwrap_err().contains("Circular"));
    op(&mut doc, Op::Set { uid: u[0], field: "dur".into(), value: "2w".into() }).unwrap();
    assert_eq!((doc.p.tasks[0].dur, doc.p.tasks[0].unit), (10 * D, Unit::W));
    doc.undo();
    assert_eq!(doc.p.tasks[0].dur, D);
    doc.redo();
    op(&mut doc, Op::Indent { uids: vec![u[1], u[2]] }).unwrap();
    assert!(doc.p.is_summary(0));
    assert!(doc.p.tasks[1].preds.is_empty()); // a link to its own new summary is dropped
    assert!(doc.p.tasks[0].preds.is_empty());
    op(&mut doc, Op::Outdent { uids: vec![u[1], u[2]] }).unwrap();
    op(&mut doc, Op::Move { uids: vec![u[3]], up: true }).unwrap();
    assert_eq!(doc.p.tasks[2].uid, u[3]);
    op(&mut doc, Op::Move { uids: vec![u[3]], up: false }).unwrap();
    assert_eq!(doc.p.tasks[3].uid, u[3]);
    op(&mut doc, Op::Set { uid: u[1], field: "preds".into(), value: "1".into() }).unwrap();
    let clip = copy(&doc.p, &[u[0], u[1]]);
    op(&mut doc, Op::Paste { at: 4, tasks: clip }).unwrap();
    assert_eq!(doc.p.tasks.len(), 6);
    assert_eq!(doc.p.tasks[5].preds[0].pred, doc.p.tasks[4].uid); // links inside the copy follow it
    op(&mut doc, Op::Set { uid: u[2], field: "finish".into(), value: "2026-11-06".into() }).unwrap();
    assert_eq!(doc.p.tasks[2].dur, 5 * D);
    op(&mut doc, Op::Delete { uids: vec![u[0]] }).unwrap();
    assert!(doc.p.tasks.iter().all(|t| t.preds.iter().all(|l| l.pred != u[0])));
}

#[test]
fn resources_costs_and_baseline() {
    let mut doc = Doc::new(plan(&[(2, ""), (3, "")]));
    let u: Vec<u32> = doc.p.tasks.iter().map(|t| t.uid).collect();
    op(&mut doc, Op::ResInsert { name: "Concrete".into() }).unwrap();
    let m = doc.p.resources[0].uid;
    op(&mut doc, Op::ResSet { uid: m, field: "kind".into(), value: "Material".into() }).unwrap();
    op(&mut doc, Op::ResSet { uid: m, field: "rate".into(), value: "50".into() }).unwrap();
    op(&mut doc, Op::Set { uid: u[0], field: "res".into(), value: "Engineer[50%], Concrete[12]".into() }).unwrap();
    op(&mut doc, Op::Set { uid: u[1], field: "res".into(), value: "Engineer".into() }).unwrap();
    let e = doc.p.resources[1].uid;
    op(&mut doc, Op::ResSet { uid: e, field: "rate".into(), value: "100".into() }).unwrap();
    let s = schedule(&doc.p).unwrap();
    let l = load::compute(&doc.p, &s);
    assert_eq!(l.work[0], 8.0 * H as f64); // 2 days at 50%
    assert_eq!(l.cost[0], 800.0 + 600.0);
    assert!(l.over[1]); // both tasks run on the same days
    assert_eq!(parse::fmt_res(&doc.p.tasks[0].res, &doc.p), "Engineer[50%], Concrete[12]");
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 }).unwrap();
    op(&mut doc, Op::Set { uid: u[0], field: "dur".into(), value: "4d".into() }).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    assert_eq!(c.text(0, "vfinish"), "2 days");
    assert_eq!(c.text(0, "cost"), "$2,200.00");
    let w = load::workload(&doc.p, &schedule(&doc.p).unwrap(), true);
    assert_eq!(w.rows[0].load[0], 16.0 + 24.0);
}

#[test]
fn calendars_and_project_settings() {
    let mut doc = Doc::new(plan(&[(1, "")]));
    let mut c = Calendar::standard("Site");
    c.week[5] = vec![(480, 720)];
    op(&mut doc, Op::CalPut { old: None, cal: c.clone() }).unwrap();
    let u0 = doc.p.tasks[0].uid;
    op(&mut doc, Op::Set { uid: u0, field: "cal".into(), value: "Site".into() }).unwrap();
    c.name = "Site 6-day".into();
    op(&mut doc, Op::CalPut { old: Some("Site".into()), cal: c }).unwrap();
    assert_eq!(doc.p.tasks[0].cal.as_deref(), Some("Site 6-day"));
    assert!(op(&mut doc, Op::CalDelete { name: "Standard".into() }).is_err());
    op(&mut doc, Op::Project { name: "P".into(), start: "2026-11-02".into(), status: "".into(), calendar: "Site 6-day".into(), day_hours: 10.0, manual_default: true }).unwrap();
    assert_eq!(doc.p.day_min, 600);
    op(&mut doc, Op::Insert { at: 1, name: "M".into(), ms: false }).unwrap();
    assert!(doc.p.tasks[1].manual);
}

#[test]
fn quick_add_and_rows() {
    let mut doc = Doc::new(plan(&[(1, "")]));
    op(&mut doc, Op::QuickAdd { at: 1, text: "Pour slab 5d after 1 @Site Crew[50%] by 20-Nov-26".into() }).unwrap();
    let t = &doc.p.tasks[1];
    assert_eq!((t.name.as_str(), t.dur, t.preds.len(), t.deadline), ("Pour slab", 5 * D, 1, Some(d(2026, 11, 20))));
    assert_eq!(doc.p.resources[0].name, "Site Crew");
    let rows = vec![vec![("name".into(), "A".into()), ("dur".into(), "3h".into())], vec![("name".into(), "B".into()), ("preds".into(), "1, 2".into())]];
    op(&mut doc, Op::InsertRows { at: 1, rows }).unwrap();
    assert_eq!(doc.p.tasks[1].dur, 3 * H);
    // IDs typed with new rows are the ones shown before the insert: 2 was "Pour slab", now row 4.
    assert_eq!(doc.p.tasks[2].preds.iter().map(|l| l.pred).collect::<Vec<_>>(), vec![doc.p.tasks[0].uid, doc.p.tasks[3].uid]);
}

#[test]
fn parsing() {
    assert_eq!(parse::dur("3", D), Ok((3 * D, Unit::D)));
    assert_eq!(parse::dur("2 wks", D), Ok((10 * D, Unit::W)));
    assert_eq!(parse::dur("4h", D), Ok((4 * H, Unit::H)));
    assert!(parse::dur("x", D).is_err());
    assert_eq!(parse::fmt_dur(12 * H, Unit::D, D), "1.5 days");
    assert_eq!(parse::date("24-Oct-26"), Ok(d(2026, 10, 24)));
    assert_eq!(parse::date_in("10/24/2026", "%m/%d/%Y"), Ok(d(2026, 10, 24)));
    let ls = parse::preds("2SS+3d, 1; 3ff-4h", &[10, 20, 30], D).unwrap();
    assert_eq!((ls[0].pred, ls[0].kind, ls[0].lag), (20, LinkType::SS, 3 * D));
    assert_eq!((ls[2].kind, ls[2].lag), (LinkType::FF, -4 * H));
    assert!(parse::preds("9", &[1], D).is_err());
    let ids = [(10, 1), (20, 2), (30, 3)].into_iter().collect();
    assert_eq!(parse::fmt_preds(&ls, &ids, D), "2SS+3d, 1, 3FF-4h");
    assert_eq!(parse::constraint("start no earlier than"), Ok(Constraint::SNET));
    let eur = Currency { code: "EUR".into(), symbol: "€".into(), decimals: 2, after: true };
    assert_eq!(fields::money(1234567.5, &eur), "1,234,567.50 €");
    assert_eq!(fields::money(-12.0, &Currency { decimals: 0, ..Currency::default() }), "-$12");
}

#[test]
fn templates_and_old_files() {
    for t in crate::templates::TEMPLATES {
        let p = crate::templates::build(t, d(2026, 11, 2));
        assert!(Calc::new(&p, "%d-%b-%y", d(2026, 11, 1)).error.is_none(), "{}", t.id);
        let back = Project::from_json(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
    }
    let v1 = r#"{"format":1,"name":"Old","start":"2026-11-02","calendar":{"name":"Standard","week":[true,true,true,true,true,false,false],"holidays":["2026-11-03"]},
        "tasks":[{"uid":1,"name":"A","dur":2,"snet":"2026-11-02"},{"uid":2,"name":"B","dur":1,"preds":[{"pred":1,"kind":"FS","lag":1}]}],"next_uid":3}"#;
    let p = Project::from_json(v1).map_err(|e| e.to_string()).unwrap();
    assert_eq!((p.tasks[0].dur, p.tasks[0].constraint, p.tasks[1].preds[0].lag), (2 * D, Constraint::SNET, D));
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.ef[0]), "04 17:00"); // the holiday carried over
}

fn rich() -> Project {
    let mut p = crate::templates::build(&crate::templates::TEMPLATES[0], d(2026, 11, 2));
    p.calendars[0].exceptions.push(Exception { name: "Holiday".into(), from: d(2026, 12, 24), to: d(2026, 12, 25), hours: vec![] });
    p.calendars.push(Calendar::full("24 Hours"));
    p.tasks[16].cal = Some("24 Hours".into());
    p.tasks[2].constraint = Constraint::SNET;
    p.tasks[2].cdate = Some(d(2026, 11, 5));
    p.tasks[1].pct = 100;
    p.tasks[1].act_start = Some(d(2026, 11, 2));
    p.tasks[1].act_finish = Some(d(2026, 11, 3));
    let mut doc = Doc::new(p);
    let u = doc.p.tasks[9].uid;
    op(&mut doc, Op::Set { uid: u, field: "res".into(), value: "CAD Designer, Design Engineer[50%]".into() }).unwrap();
    doc.p
}

fn same_schedule(a: &Project, b: &Project) {
    let (sa, sb) = (schedule(a).unwrap(), schedule(b).unwrap());
    assert_eq!(a.tasks.len(), b.tasks.len());
    assert_eq!(when(sa.finish), when(sb.finish));
    assert_eq!(a.tasks.iter().map(|t| t.preds.len()).sum::<usize>(), b.tasks.iter().map(|t| t.preds.len()).sum::<usize>());
    assert_eq!(a.resources.len(), b.resources.len());
}

#[test]
fn exchange_round_trips() {
    let p = rich();
    let dir = std::env::temp_dir().join(format!("datum-io-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for ext in ["xml", "xer"] {
        let f = dir.join(format!("plan.{ext}"));
        crate::io::export(&p, &f, "%d-%b-%y", d(2026, 11, 1), &[]).unwrap();
        let crate::io::Imported::Plan(q, _) = crate::io::import(&f).unwrap() else { panic!("{ext} should import as a plan") };
        same_schedule(&p, &q);
        assert_eq!(q.tasks[1].act_finish, Some(d(2026, 11, 3)), "{ext}");
        assert!(q.calendars.iter().any(|c| c.exceptions.iter().any(|e| e.from == d(2026, 12, 24))), "{ext}");
    }
    for ext in ["csv", "xlsx"] {
        let f = dir.join(format!("plan.{ext}"));
        crate::io::export(&p, &f, "%d-%b-%y", d(2026, 11, 1), &[]).unwrap();
        let crate::io::Imported::Table(t) = crate::io::import(&f).unwrap() else { panic!("{ext} should import as a table") };
        assert!(t.guess.contains(&"name".to_string()) && t.guess.contains(&"preds".to_string()), "{ext}: {:?}", t.guess);
        let (q, bad) = crate::io::table::to_project(&t, &t.guess, "Q", d(2026, 11, 2), "%d-%b-%y").unwrap();
        assert_eq!(bad, 0, "{ext}");
        assert_eq!(q.tasks.len(), p.tasks.len());
        assert!(q.is_summary(0) && q.tasks[3].dur == 0);
        assert_eq!(q.tasks[10].preds[0].kind, LinkType::SS);
    }
    // Codes as IDs, and headers that look alike.
    let f = dir.join("codes.csv");
    std::fs::write(&f, "Activity,Description,Days,Depends on,Who\nA1,Survey,3,,Surveyor\nA2,Design,5,A1,Engineer\nA10,Build,8,\"A2SS+2d, A1\",Crew[200%]\n").unwrap();
    let crate::io::Imported::Table(t) = crate::io::import(&f).unwrap() else { panic!() };
    assert_eq!(t.guess, ["id", "name", "dur", "preds", "res"]);
    let (q, bad) = crate::io::table::to_project(&t, &t.guess, "Q", d(2026, 11, 2), "%d-%b-%y").unwrap();
    assert_eq!((bad, q.tasks[2].preds.len(), q.tasks[2].preds[0].kind), (0, 2, LinkType::SS));
    std::fs::remove_dir_all(dir).ok();
}

// ---------- 0.3 engine ----------

#[test]
fn elapsed_pct_lags_splits_and_from_finish() {
    let mut p = plan(&[(10, ""), (2, "1SS+50%"), (2, "")]);
    let (m, _, e) = parse::dur_any("2ed", D).unwrap();
    (p.tasks[2].dur, p.tasks[2].elapsed) = (m, e);
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.es[1]), "09 08:00"); // half of task 1's ten days later
    assert_eq!(when(s.ef[2]), "04 08:00"); // two days around the clock
    p.tasks[1].splits = vec![(D, D)];
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.ef[1]), "11 17:00"); // a one-day pause inside the work
    assert_eq!(parse::fmt_preds(&p.tasks[1].preds, &[(p.tasks[0].uid, 1)].into_iter().collect(), D), "1SS+50%");
    p.opts.from_finish = true;
    p.opts.finish_date = Some(d(2026, 11, 30));
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.ef[0]), "30 17:00");
}

#[test]
fn level_of_effort_progress_logic_and_longest_path() {
    let mut p = plan(&[(5, ""), (3, "1"), (0, "1SS"), (2, "")]);
    p.tasks[3].preds = parse::preds("2FF", &p.tasks.iter().map(|t| t.uid).collect::<Vec<_>>(), D).unwrap();
    p.tasks[2].act_type = ActType::Loe;
    p.tasks[2].preds = vec![Link { pred: p.tasks[0].uid, kind: LinkType::SS, lag: 0, pct: false, elapsed: false }];
    p.tasks[3].preds.clear();
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    p.tasks[2].dur = 0;
    p.tasks.push(Task { uid: 99, name: "after LOE".into(), dur: D, preds: parse::preds("3", &ids, D).unwrap(), ..Default::default() });
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.es[2]), "02 08:00");
    assert_eq!(when(s.ef[2]), "02 08:00"); // a level of effort only reaches its successors' needs…
    assert_eq!(when(s.es[4]), "02 08:00"); // …and never drives them
    assert!(s.lp[0] && s.lp[1] && !s.lp[3]);

    // Out-of-sequence progress: task 2 started before task 1 finished.
    let mut p = plan(&[(4, ""), (4, "1")]);
    p.status = Some(d(2026, 11, 3));
    p.tasks[1].act_start = Some(d(2026, 11, 2));
    p.tasks[1].pct = 50;
    assert_eq!(when(schedule(&p).unwrap().ef[1]), "11 17:00"); // retained logic waits for task 1 (which cannot start before the status date)
    p.opts.retained = false;
    assert_eq!(when(schedule(&p).unwrap().ef[1]), "05 17:00"); // progress override does not
}

#[test]
fn leveling_and_task_types() {
    let mut doc = Doc::new(plan(&[(2, ""), (3, ""), (1, "")]));
    let u: Vec<u32> = doc.p.tasks.iter().map(|t| t.uid).collect();
    for &uid in &u[..2] {
        op(&mut doc, Op::Set { uid, field: "res".into(), value: "Welder".into() }).unwrap();
    }
    op(&mut doc, Op::Level { uids: vec![], clear: false }).unwrap();
    let s = schedule(&doc.p).unwrap();
    assert!(!load::compute(&doc.p, &s).over.iter().any(|&o| o));
    assert!(s.es[1] >= s.ef[0] || s.es[0] >= s.ef[1]);
    op(&mut doc, Op::Level { uids: vec![], clear: true }).unwrap();
    assert!(doc.p.tasks.iter().all(|t| t.level_delay == 0));

    // Fixed work: two people halve the duration; fixed duration keeps it.
    op(&mut doc, Op::Set { uid: u[1], field: "type".into(), value: "Fixed Work".into() }).unwrap();
    op(&mut doc, Op::Set { uid: u[1], field: "res".into(), value: "Welder, Fitter".into() }).unwrap();
    assert_eq!(doc.p.tasks[1].dur, 3 * D / 2);
    op(&mut doc, Op::Set { uid: u[0], field: "res".into(), value: "Welder, Fitter".into() }).unwrap();
    assert_eq!(doc.p.tasks[0].dur, 2 * D);
    op(&mut doc, Op::Set { uid: u[0], field: "work".into(), value: "8".into() }).unwrap();
    assert_eq!(doc.p.tasks[0].res[0].units, 0.25); // 8h over 2 days with two people
    // Custom fields, codes, risks and steps.
    op(&mut doc, Op::Set { uid: u[2], field: "c:number1".into(), value: "12.5".into() }).unwrap();
    assert_eq!(doc.p.tasks[2].custom["number1"], serde_json::json!(12.5));
    op(&mut doc, Op::CodeDef { old: None, def: Some(CodeDef { name: "Discipline".into(), scope: String::new(), values: vec![CodeValue { value: "MECH".into(), ..Default::default() }] }) }).unwrap();
    assert!(op(&mut doc, Op::Set { uid: u[2], field: "code:Discipline".into(), value: "CIVIL".into() }).is_err());
    op(&mut doc, Op::Set { uid: u[2], field: "code:Discipline".into(), value: "MECH".into() }).unwrap();
    op(&mut doc, Op::TaskList { uid: u[2], field: "steps".into(), value: serde_json::json!([{"name": "a", "weight": 3, "done": true}, {"name": "b", "weight": 1}]) }).unwrap();
    assert_eq!(doc.p.tasks[2].phys, 75);
    op(&mut doc, Op::Risk { id: 0, risk: Some(Risk { name: "Late steel".into(), prob: 3, impact: 4, ..Default::default() }) }).unwrap();
    assert_eq!(doc.p.risks[0].id, 1);
    assert!(doc.p.tasks.iter().all(|t| t.code.starts_with('A') || t.code.is_empty()));
}

struct Row1(std::collections::HashMap<&'static str, crate::formula::F>);
impl crate::formula::Ctx for Row1 {
    fn field(&self, n: &str) -> Option<crate::formula::F> { self.0.get(n.to_lowercase().as_str()).cloned() }
    fn now(&self) -> i64 { at(d(2026, 11, 2)) }
    fn work_between(&self, a: i64, b: i64) -> i64 { WorkCal::new(&Calendar::standard("S")).between(a, b) }
    fn work_add(&self, a: i64, m: i64) -> i64 { WorkCal::new(&Calendar::standard("S")).add(a, m) }
}

#[test]
fn formulas() {
    use crate::formula::{eval, parse as fp, F};
    let row = Row1([("duration", F::Num(960.0)), ("finish", F::Date(at(d(2026, 11, 6)))), ("baseline finish", F::Date(at(d(2026, 11, 4)))), ("name", F::Str("Pump".into()))].into_iter().collect());
    let ev = |s: &str| eval(&fp(s).unwrap(), &row).unwrap();
    assert_eq!(ev("[Duration] / 480"), F::Num(2.0));
    assert_eq!(ev(r#"IIf([Finish] > [Baseline Finish], "Late", "On time")"#), F::Str("Late".into()));
    assert_eq!(ev("[Finish] - [Baseline Finish]"), F::Num(2.0));
    assert_eq!(ev("ProjDateDiff([Baseline Finish], [Finish]) / 60"), F::Num(16.0));
    assert_eq!(ev(r#"Switch([Duration] > 1000, "Long", [Duration] > 100, "Medium")"#), F::Str("Medium".into()));
    assert_eq!(ev(r#"UCase(Left([Name], 2)) & "-" & 7 Mod 4"#), F::Str("PU-3".into()));
    assert_eq!(ev("Not (1 > 2) And 3 >= 3"), F::Bool(true));
    assert!(fp("IIf(1, 2").is_err() && fp("Foo(1)").is_err());
    assert!(eval(&fp("1 / 0").unwrap(), &row).is_err());
}

#[test]
fn fields_earned_value_queries_and_checks() {
    use crate::query::{Filter, Query, Rule};
    let mut doc = Doc::new(plan(&[(0, ""), (2, ""), (4, "2"), (0, "3")]));
    let u: Vec<u32> = doc.p.tasks.iter().map(|t| t.uid).collect();
    op(&mut doc, Op::Indent { uids: vec![u[1], u[2]] }).unwrap();
    op(&mut doc, Op::Set { uid: u[0], field: "name".into(), value: "Phase".into() }).unwrap();
    op(&mut doc, Op::Set { uid: u[1], field: "fixedcost".into(), value: "1000".into() }).unwrap();
    op(&mut doc, Op::Set { uid: u[2], field: "fixedcost".into(), value: "2000".into() }).unwrap();
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 }).unwrap();
    op(&mut doc, Op::Set { uid: u[1], field: "pct".into(), value: "100".into() }).unwrap();
    op(&mut doc, Op::Set { uid: u[2], field: "pct".into(), value: "25".into() }).unwrap();
    // Status at the end of Wednesday: task 2 (Mon–Tue) done, task 3 one day in of four.
    op(&mut doc, Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-11-04") }).unwrap();
    let def = CustomDef { key: "number1".into(), alias: "Double cost".into(), formula: "[Cost] * 2".into(), ..CustomDef::default() };
    op(&mut doc, Op::FieldDef { def }).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    assert_eq!((c.val(1, "bcws").num(), c.val(2, "bcws").num()), (1000.0, 500.0));
    assert_eq!((c.val(0, "bcwp").num(), c.val(0, "bac").num()), (1500.0, 3000.0));
    assert_eq!(c.text(0, "spi"), "1");
    assert_eq!(c.text(1, "c:number1"), "2000");
    assert_eq!(c.key_of("Double cost").as_deref(), Some("c:number1"));
    assert_eq!(c.text(0, "pct"), "50%"); // weighted by duration: (2×100 + 4×25) / 6
    // Filters keep the summary of matching tasks; sorting keeps the outline.
    let q = |f: Filter, sort: Vec<(String, bool)>| c.grid(&Query { cols: vec!["name".into(), "Duration".into()], filter: f, sort, ..Query::default() });
    let crit = Filter { rules: vec![Rule { field: "Milestone".into(), test: "equals".into(), value: "Yes".into(), ..Rule::default() }], summaries: true, ..Filter::default() };
    let g = q(crit, vec![]);
    assert_eq!(g.rows.iter().map(|r| r.cells[0].as_str()).collect::<Vec<_>>(), ["T4"]);
    let g = q(Filter::default(), vec![("dur".into(), true)]);
    assert_eq!(g.rows.iter().map(|r| r.cells[0].as_str()).collect::<Vec<_>>(), ["Phase", "T3", "T2", "T4"]);
    assert_eq!(g.cols[1].name, "Duration");
    let grouped = c.grid(&Query { cols: vec!["name".into()], group: vec!["milestone".into()], ..Query::default() });
    assert_eq!(grouped.rows.iter().map(|r| (r.kind, r.cells[0].as_str())).collect::<Vec<_>>(), [("group", "Milestone: No"), ("task", "T2"), ("task", "T3"), ("group", "Milestone: Yes"), ("task", "T4")]);
    let checks = crate::check::run(&c, &Default::default());
    let logic = checks.iter().find(|k| k.key == "logic").unwrap();
    assert_eq!((logic.count, logic.pass), (0, true));
    assert!(checks.iter().find(|k| k.key == "resources").is_some_and(|k| k.count == 0 && k.pass));
    let net = crate::network::layout(&c, &[1, 2, 3]);
    assert_eq!(net.nodes.iter().map(|n| n.col).collect::<Vec<_>>(), [0, 1, 2]);
    let curve = crate::report::curve(&c, "week");
    assert_eq!(curve.planned.iter().sum::<f64>().round(), 3000.0);
    assert_eq!(c.summary().tasks, 3);
    let html = crate::io::formats::html(&c, &["name".into(), "cost".into()]);
    assert!(html.contains("$3,000.00") && html.contains("T3"));
    assert!(crate::io::formats::pmxml(&c).contains("<Relationship>"));
}

#[test]
fn excel_formula_columns_sheets_and_periods() {
    let mut doc = Doc::new(plan(&[(5, ""), (5, "1"), (0, "2")]));
    let u: Vec<u32> = doc.p.tasks.iter().map(|t| t.uid).collect();
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 }).unwrap();
    op(&mut doc, Op::Cells { cells: vec![(u[0], "pct".into(), "100".into()), (u[1], "pct".into(), "40".into()), (u[1], "dur".into(), "oops".into())] }).unwrap();
    let def = |key: &str, alias: &str, f: &str| Op::FieldDef { def: CustomDef { key: key.into(), alias: alias.into(), formula: f.into(), ..CustomDef::default() } };
    op(&mut doc, def("number1", "Days left", "=[Duration]*(100-[% Complete])/100")).unwrap();
    op(&mut doc, def("text1", "Health", r#"=IF([Days left]>2,"Watch","OK")"#)).unwrap();
    op(&mut doc, def("date1", "Due", "=WORKDAY([Finish],2)")).unwrap();
    assert!(op(&mut doc, def("number2", "", "=[No such field]+1")).is_err());
    op(&mut doc, Op::Title { key: "dur".into(), title: "Days".into() }).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    assert_eq!((c.text(0, "c:number1"), c.text(1, "c:number1")), ("0".to_string(), "3".to_string()));
    assert_eq!((c.text(0, "c:text1"), c.text(1, "c:text1")), ("OK".to_string(), "Watch".to_string()));
    assert_eq!(c.text(0, "c:date1"), "10-Nov-26"); // Fri 6 Nov + 2 working days
    assert_eq!(c.name("dur"), "Days");
    op(&mut doc, def("number3", "Progress variance", "=[% Complete]-[Baseline % Complete]")).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    assert_eq!(c.text(0, "c:number3"), "0"); // no status date: measured when the progress was made, as the dashboard does
    // A spreadsheet tab reading the Plan sheet.
    let sheet = SheetData { name: "Calc".into(), ..Default::default() };
    op(&mut doc, Op::Sheet { k: None, sheet: Some(sheet) }).unwrap();
    op(&mut doc, Op::SheetCells { k: 0, cells: vec![("A1".into(), "=SUM(Plan!F2:F4)".into()), ("b1".into(), "=A1*2".into()), ("C1".into(), "hello".into())], fmt: None }).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    let v = crate::sheet::sheet_view(&c, 0).unwrap();
    assert_eq!(v.cells.iter().map(|x| x.2.as_str()).collect::<Vec<_>>(), ["10", "20", "hello"]);
    // Weekly periods end on Fridays; the S-curve reaches 100% planned.
    op(&mut doc, Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-11-10") }).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    let ends = crate::report::period_ends(&c, "week");
    assert_eq!(date_of(ends[0] * DAY), d(2026, 11, 6));
    let sc = crate::report::scurve(&c, "week", "duration", None);
    assert!((sc.planned.last().unwrap() - 100.0).abs() < 1e-6);
    let r = crate::report::period_report(&c, "week", None);
    assert_eq!(r.to, ends[1]);
    assert!(r.actual > 50.0 && r.actual < 80.0, "{}", r.actual);
    assert_eq!(r.milestones.len(), 1);
}

#[test]
fn recurring_tasks_and_excel_export_with_formulas() {
    let mut doc = Doc::new(plan(&[(2, "")]));
    op(&mut doc, Op::Recurring { at: 1, name: "Site meeting".into(), dur: "2h".into(), every: "week".into(), interval: 1, count: 4, start: "2026-11-04".into() }).unwrap();
    let p = &doc.p;
    assert_eq!(p.tasks.len(), 6);
    assert_eq!((p.tasks[1].name.as_str(), p.tasks[5].name.as_str(), p.tasks[5].level), ("Site meeting", "Site meeting 4", 1));
    let s = schedule(p).unwrap();
    assert_eq!(date_of(s.es[5]), d(2026, 11, 25));
    let codes: std::collections::HashSet<&str> = p.tasks.iter().map(|t| t.code.as_str()).collect();
    assert_eq!(codes.len(), 6, "Activity IDs stay unique");
    let def = CustomDef { key: "number1".into(), alias: "Double".into(), formula: "=[Duration]*2".into(), ..CustomDef::default() };
    op(&mut doc, Op::FieldDef { def }).unwrap();
    op(&mut doc, Op::Sheet { k: None, sheet: Some(SheetData { name: "Calc".into(), ..Default::default() }) }).unwrap();
    op(&mut doc, Op::SheetCells { k: 0, cells: vec![("A1".into(), "=SUM(Plan!F2:F7)".into())], fmt: None }).unwrap();
    let f = std::env::temp_dir().join(format!("dp-fx-{}.xlsx", std::process::id()));
    crate::io::export(&doc.p, &f, "%d-%b-%y", d(2026, 11, 1), &["name".into(), "dur".into(), "c:number1".into()]).unwrap();
    let bytes = std::fs::read(&f).unwrap();
    let _ = std::fs::remove_file(&f);
    let mut formulas = Vec::new();
    // The sheet XML is deflated inside the zip; check through calamine that the formula is there.
    use calamine::{open_workbook_auto, Reader};
    let tmp = std::env::temp_dir().join(format!("dp-fx2-{}.xlsx", std::process::id()));
    std::fs::write(&tmp, &bytes).unwrap();
    let mut wb = open_workbook_auto(&tmp).unwrap();
    let fx = wb.worksheet_formula("Tasks").unwrap();
    formulas.extend(fx.used_cells().map(|(_, _, f)| f.clone()));
    assert!(formulas.iter().any(|f| f.contains("B2*2")), "{formulas:?}");
    assert!(wb.sheet_names().contains(&"Plan".to_string()) && wb.sheet_names().contains(&"Calc".to_string()));
    let _ = std::fs::remove_file(&tmp);
}

// Typed dates follow the plan's date format; Text keeps what was typed; a merge shrinks with deleted
// rows; a change to the tab's layout never undoes a cell edit.
#[test]
fn sheet_cells_keep_what_was_typed() {
    let mut doc = Doc::new(plan(&[(2, "")]));
    op(&mut doc, Op::Sheet { k: None, sheet: Some(SheetData { name: "S".into(), merges: vec!["B2:C4".into()], ..Default::default() }) }).unwrap();
    op(&mut doc, Op::SheetCells { k: 0, cells: vec![("A1".into(), "13/01/2026".into()), ("A2".into(), "=A1+1".into()), ("A3".into(), "001".into())], fmt: None }).unwrap();
    op(&mut doc, Op::SheetCells { k: 0, cells: vec![("A3".into(), String::new())], fmt: Some("@".into()) }).unwrap();
    let c = Calc::new(&doc.p, "%d/%m/%Y", d(2026, 11, 2));
    let v = crate::sheet::sheet_view(&c, 0).unwrap();
    let at = |a: &str| v.cells.iter().find(|x| x.0 == a).map(|x| x.2.clone()).unwrap_or_default();
    assert_eq!((at("A1"), at("A2"), at("A3")), ("13/01/2026".to_string(), "14/01/2026".to_string(), "001".to_string()));
    // A layout change made from an older copy of the tab keeps the cells typed since.
    let mut old = doc.p.sheets[0].clone();
    old.cells.clear();
    old.widths.insert(0, 150);
    op(&mut doc, Op::Sheet { k: Some(0), sheet: Some(old) }).unwrap();
    assert_eq!((doc.p.sheets[0].cells.len(), doc.p.sheets[0].widths.get(&0)), (3, Some(&150)));
    op(&mut doc, Op::SheetInsert { k: 0, cols: false, at: 1, count: -1 }).unwrap();
    assert_eq!(doc.p.sheets[0].merges, vec!["B2:C3".to_string()], "deleting the top row of a merge shrinks it");
}

#[test]
fn sheets_read_the_plan_by_name_and_behave_like_excel() {
    let mut doc = Doc::new(plan(&[(2, ""), (3, "1"), (0, "2")]));
    op(&mut doc, Op::Sheet { k: None, sheet: Some(SheetData { name: "Dash".into(), ..Default::default() }) }).unwrap();
    let cells = [
        ("A1", "=SUM(Tasks[Duration])"), ("A2", "=ROWS(Tasks)"), ("A3", "=ProjectFinish"), ("A4", "=XLOOKUP(\"T3\",Tasks[Name],Tasks[Finish])"),
        ("A5", "=COUNTIF(Tasks[Milestone],TRUE)"), ("A6", "=TODAY()-TODAY()"), ("A7", "1234.5"), ("A8", "=MAX(Tasks[% Complete])"), ("A9", "=SEQUENCE(3)"),
        ("B1", "=A1*2"), ("C1", "=Dash!A1+1"), ("D1", "=XLOOKUP(\"T3\",Tasks[Task Name],Tasks[Finish])"), ("D2", "=SUM(Tasks[Total Slack])=SUM(Tasks[Total Float])"),
    ];
    op(&mut doc, Op::SheetCells { k: 0, cells: cells.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), fmt: None }).unwrap();
    op(&mut doc, Op::SheetCells { k: 0, cells: vec![("A7".into(), String::new())], fmt: Some("#,##0.00".into()) }).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 2));
    let v = crate::sheet::sheet_view(&c, 0).unwrap();
    let at = |a: &str| v.cells.iter().find(|x| x.0 == a).map(|x| x.2.clone()).unwrap_or_default();
    assert_eq!((at("A1"), at("A2"), at("A5")), ("5".to_string(), "3".to_string(), "1".to_string()));
    assert_eq!(at("A3"), "06-Nov-26", "a date name shows as a date");
    assert_eq!(at("A4"), at("A3"), "lookups of plan dates show as dates");
    assert_eq!((at("D1"), at("D2")), (at("A3"), "TRUE".to_string()), "columns named as on the task sheet work too");
    assert_eq!(at("A6"), "0", "date arithmetic stays a number");
    assert_eq!(at("A7"), "1,234.50", "spreadsheet number formats");
    assert_eq!(at("A8"), "0%", "plan percentages (0–100) get a % sign");
    assert_eq!((at("A10"), at("A11")), ("2".to_string(), "3".to_string()), "spilled results are shown");
    assert_eq!(v.cells.iter().find(|x| x.0 == "A10").map(|x| x.1.as_str()), Some(""), "a spilled cell has no input of its own");
    // Fill and paste move relative references; absolute ones stay.
    assert_eq!(crate::sheet::shift("=A1*$B$1+Tasks[Cost]", 2, 1), "=B3*$B$1+Tasks[Cost]");
    assert_eq!(crate::sheet::shift("12", 2, 1), "12");
    // Inserting a row above A1 moves the cells and the references to them.
    op(&mut doc, Op::SheetInsert { k: 0, cols: false, at: 0, count: 1 }).unwrap();
    let s = &doc.p.sheets[0];
    assert_eq!((s.cells.get("B2").map(String::as_str), s.cells.get("C2").map(String::as_str)), (Some("=A2*2"), Some("=Dash!A2+1")));
    assert!(s.fmt.contains_key("A8") && !s.cells.contains_key("A1"));
    // Deleting a column turns references into it into #REF!.
    op(&mut doc, Op::SheetInsert { k: 0, cols: true, at: 0, count: -1 }).unwrap();
    assert_eq!(doc.p.sheets[0].cells.get("A2").map(String::as_str), Some("=#REF!*2"));
    // Renaming a sheet updates formulas that name it; names must stay unique.
    op(&mut doc, Op::Sheet { k: None, sheet: Some(SheetData { name: "Other".into(), cells: [("A1".to_string(), "=Dash!B2".to_string())].into(), ..Default::default() }) }).unwrap();
    let mut s = doc.p.sheets[0].clone();
    s.name = "Board".into();
    s.cells.clear();
    op(&mut doc, Op::Sheet { k: Some(0), sheet: Some(s) }).unwrap();
    assert_eq!(doc.p.sheets[1].cells.get("A1").map(String::as_str), Some("=Board!B2"));
    assert!(!doc.p.sheets[0].cells.is_empty(), "a rename keeps the cells");
    assert!(op(&mut doc, Op::Sheet { k: None, sheet: Some(SheetData { name: "board".into(), ..Default::default() }) }).is_err());
    assert!(op(&mut doc, Op::Sheet { k: None, sheet: Some(SheetData { name: "Plan".into(), ..Default::default() }) }).is_err());
    op(&mut doc, Op::Sheet { k: Some(0), sheet: None }).unwrap();
    assert_eq!(doc.p.sheets[0].cells.get("A1").map(String::as_str), Some("=#REF!"));
    assert!(crate::sheet::functions().iter().any(|f| f.0 == "SUMIFS" && f.1.contains("criteria")));
}

#[test]
fn dashboard_tiles_evaluate_formulas_spills_and_sheet_ranges() {
    let mut doc = Doc::new(plan(&[(2, ""), (3, "1"), (0, "2")]));
    op(&mut doc, Op::Sheet { k: None, sheet: Some(SheetData { name: "Vendors".into(), ..Default::default() }) }).unwrap();
    op(&mut doc, Op::SheetCells { k: 0, cells: vec![("A1".into(), "Vendor".into()), ("B1".into(), "Country".into()), ("A2".into(), "Steel Co".into()), ("B2".into(), "DE".into()), ("C2".into(), "1500".into())], fmt: None }).unwrap();
    op(&mut doc, Op::SheetCells { k: 0, cells: vec![("B2".into(), String::new())], fmt: Some("flag".into()) }).unwrap();
    op(&mut doc, Op::SheetCells { k: 0, cells: vec![("C2".into(), String::new())], fmt: Some("#,##0".into()) }).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 2));
    let items: Vec<(String, String)> = [("=ROWS(Tasks)", ""), ("=ProjectFinish", ""), ("=Tasks[Name]", ""), ("=Vendors!A1:C2", ""), ("=1/2", "0%"), ("=NOSUCH()", ""), ("=PlannedPercent", "")]
        .iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    let r = crate::sheet::eval(&c, &items);
    assert_eq!((r[0][0][0].0.as_str(), r[0][0][0].1), ("3", Some(3.0)));
    assert_eq!(r[1][0][0].0, "06-Nov-26", "a plan date shows as a date");
    assert_eq!(r[2].iter().map(|row| row[0].0.as_str()).collect::<Vec<_>>(), ["T1", "T2", "T3"], "a spilled column becomes rows");
    assert_eq!(r[3].len(), 2);
    assert_eq!((r[3][1][1].0.as_str(), r[3][1][1].2.as_str()), ("DE", "flag"), "a sheet range keeps the sheet's formats");
    assert_eq!(r[3][1][2].0, "1,500");
    assert_eq!(r[4][0][0].0, "50%", "the tile's own format");
    assert!(r[5][0][0].0.starts_with('#'), "an error shows as an error");
    assert!(r[6][0][0].0.ends_with('%'));
}

#[test]
fn plans_linked_to_each_other_move_together() {
    let mut plans = crate::templates::linked_example(d(2026, 11, 2));
    assert_eq!(plans.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["Main schedule.dplan", "Engineering.dplan", "Procurement.dplan", "Construction.dplan"]);
    assert_eq!(plans.iter().map(|x| x.1.tasks.iter().map(|t| t.ext.len()).sum::<usize>()).sum::<usize>(), 13);
    let date = |plans: &[(String, Project)], k: usize, name: &str| {
        let p = &plans[k].1;
        let s = schedule(p).unwrap();
        let i = p.tasks.iter().position(|t| t.name == name).unwrap_or_else(|| panic!("{name}"));
        s.ef[i]
    };
    // A link carries the other plan's dates: the main schedule's mechanical completion is construction's.
    assert_eq!(date(&plans, 0, "Mechanical completion"), date(&plans, 3, "Mechanical completion"));
    assert!(plans.iter().all(|x| x.1.tasks.iter().flat_map(|t| &t.ext).all(|e| e.finish > 0 && !e.name.is_empty())), "every link knows its dates");
    // The grid marks a linked task with what it waits for, as people read it.
    let c = crate::fields::Calc::new(&plans[0].1, "%d-%b-%y", d(2026, 11, 2));
    let g = c.grid(&crate::query::Query::default());
    let row = g.rows.iter().find(|r| r.name == "Engineering complete").unwrap();
    assert!(row.ext.len() == 1 && row.ext[0].starts_with("Engineering › ") && row.ext[0].ends_with("(FS)"), "{:?}", row.ext);
    // One project: the master lists its subplans in order, and each subplan knows its master.
    assert_eq!(plans[0].1.subplans.iter().map(|s| s.file.as_str()).collect::<Vec<_>>(), ["Engineering.dplan", "Procurement.dplan", "Construction.dplan"]);
    assert!(plans[1..].iter().all(|x| x.1.master == "Main schedule.dplan"));
    // The hierarchy has no jumps and each file is listed once.
    let mut m = plans[0].1.clone();
    m.subplans.push(crate::model::SubPlan { file: "Civil.dplan".into(), name: "Civil".into(), level: 4 });
    m.subplans.push(crate::model::SubPlan { file: "./engineering.DPLAN".into(), name: "Again".into(), level: 0 });
    m.normalize();
    assert_eq!(m.subplans.iter().map(|s| (s.name.as_str(), s.level)).collect::<Vec<_>>(), [("Engineering", 0), ("Procurement", 0), ("Construction", 0), ("Civil", 1)]);
    // No work over Christmas: every calendar in the example closes from 24 December to 1 January.
    assert!(plans.iter().all(|x| x.1.calendars.iter().all(|c| c.exceptions.iter().any(|e| e.name.starts_with("Christmas")))));
    // Three weeks more for the compressor's manufacture moves the handover by about as much.
    let before = date(&plans, 0, "Handover to operations");
    let i = plans[2].1.tasks.iter().position(|t| t.name == "Manufacture and factory acceptance test").unwrap();
    plans[2].1.tasks[i].dur += 15 * 480;
    assert_eq!(date(&plans, 0, "Handover to operations"), before, "not until the links are updated");
    let passes = crate::links::update_all(&mut plans).unwrap().0;
    assert!(passes >= 3, "the change travels procurement → construction → main ({passes} passes)");
    let after = date(&plans, 0, "Handover to operations");
    assert!(after - before >= 18 * DAY && after - before <= 24 * DAY, "{}", (after - before) / DAY);
    // A loop between plans is refused.
    let uid = plans[1].1.tasks[1].uid;
    let back = plans[0].1.tasks[1].ext.len();
    plans[0].1.tasks[1].ext.push(crate::model::ExtLink { file: "Engineering.dplan".into(), uid, kind: LinkType::FS, lag: 0, plan: String::new(), code: String::new(), name: String::new(), start: 0, finish: 0 });
    assert!(crate::links::update_all(&mut plans).is_err(), "contract award ← engineering kick-off ← contract award");
    plans[0].1.tasks[1].ext.truncate(back);
    // The links survive saving and opening a plan.
    let q: Project = serde_json::from_str(&serde_json::to_string(&plans[3].1).unwrap()).unwrap();
    assert_eq!(q.tasks.iter().map(|t| t.ext.len()).sum::<usize>(), 5);
    assert_eq!(crate::links::states(&q, &mut |_| Err("closed".into())).len(), 5);
}

#[test]
fn text_formatting_on_tasks_and_sheets() {
    let mut doc = Doc::new(plan(&[(2, ""), (3, "1")]));
    let u = doc.p.tasks[0].uid;
    op(&mut doc, Op::Style { cells: vec![(u, "name".into(), "b;c:#C0392B".into()), (u, "*".into(), "f:#FFF2CC".into())] }).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 2));
    let g = c.grid(&crate::query::Query { cols: vec!["name".into()], ..Default::default() });
    assert_eq!(g.rows[0].style.get("name").map(String::as_str), Some("b;c:#C0392B"));
    assert_eq!(g.rows[0].style.get("*").map(String::as_str), Some("f:#FFF2CC"));
    op(&mut doc, Op::Style { cells: vec![(u, "name".into(), String::new()), (u, "*".into(), String::new())] }).unwrap();
    assert!(doc.p.tasks[0].style.is_null(), "clearing the last style leaves nothing in the file");
    op(&mut doc, Op::Sheet { k: None, sheet: Some(SheetData { name: "Q".into(), ..Default::default() }) }).unwrap();
    op(&mut doc, Op::SheetStyle { k: 0, cells: vec![("b2".into(), "i;a:r".into())] }).unwrap();
    assert_eq!(doc.p.sheets[0].style.get("B2").map(String::as_str), Some("i;a:r"));
}

#[test]
fn delete_custom_field_removes_values() {
    let mut doc = Doc::new(plan(&[(2, "")]));
    let u = doc.p.tasks[0].uid;
    op(&mut doc, Op::FieldDef { def: CustomDef { key: "text1".into(), alias: "Area".into(), ..CustomDef::default() } }).unwrap();
    op(&mut doc, Op::Cells { cells: vec![(u, "c:text1".into(), "North".into())] }).unwrap();
    assert!(doc.p.tasks[0].custom.contains_key("text1"));
    op(&mut doc, Op::DeleteField { key: "text1".into() }).unwrap();
    assert!(doc.p.fields.is_empty() && doc.p.tasks[0].custom.is_empty());
}

#[test]
fn excel_export_keeps_cell_formatting() {
    let mut doc = Doc::new(plan(&[(2, ""), (3, "1")]));
    let u = doc.p.tasks[0].uid;
    op(&mut doc, Op::Style { cells: vec![(u, "name".into(), "b;c:#C0392B;f:#FFF2CC;n:Georgia".into())] }).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 2));
    let f = std::env::temp_dir().join(format!("dp-style-{}.xlsx", std::process::id()));
    crate::io::table::write_xlsx(&c, &["name".into(), "dur".into()], &f).unwrap();
    // The styles part of the workbook carries the colours and font (checked with unzip where available).
    if let Ok(out) = std::process::Command::new("unzip").arg("-p").arg(&f).arg("xl/styles.xml").output() {
        let x = String::from_utf8_lossy(&out.stdout);
        if !x.is_empty() { assert!(x.contains("FFC0392B") && x.contains("FFFFF2CC") && x.contains("Georgia"), "{x}"); }
    }
    let _ = std::fs::remove_file(f);
}

#[test]
fn templates_come_with_people_man_hours_and_costs() {
    for t in crate::templates::TEMPLATES {
        let p = crate::templates::build(t, d(2026, 11, 2));
        let c = Calc::new(&p, "%d-%b-%y", d(2026, 11, 2));
        let num = |i: usize, k: &str| if let crate::formula::F::Num(n) = c.val(i, k) { n } else { 0.0 };
        let work: f64 = (0..p.tasks.len()).filter(|&i| !c.s.summary[i]).map(|i| num(i, "work")).sum();
        let cost: f64 = (0..p.tasks.len()).filter(|&i| !c.s.summary[i]).map(|i| num(i, "cost")).sum();
        assert!(p.resources.len() >= 4 && work > 100.0 && cost > 1000.0, "{}: {} people, {work} h, {cost}", t.name, p.resources.len());
        assert_eq!(p.titles.get("work").map(String::as_str), Some("Man-hours"));
    }
}

#[test]
fn network_by_wbs_bands_and_summaries() {
    let p = crate::templates::build(&crate::templates::TEMPLATES[0], d(2026, 11, 2));
    let c = Calc::new(&p, "%d-%b-%y", d(2026, 11, 2));
    let tasks: Vec<usize> = (0..p.tasks.len()).filter(|&i| !c.s.summary[i]).collect();
    let sums: Vec<usize> = (0..p.tasks.len()).filter(|&i| c.s.summary[i]).collect();
    let n = crate::network::layout_with(&c, &tasks, &sums, true);
    assert_eq!(n.bands.len(), sums.len(), "one band per phase");
    assert_eq!(n.nodes.len(), p.tasks.len());
    // Every task sits inside its phase's band, below the phase's own box.
    for (b, &sm) in n.bands.iter().zip(&sums) {
        assert_eq!(b.name, p.tasks[sm].name);
        let at = |i: usize| n.nodes.iter().find(|x| x.uid == p.tasks[i].uid).unwrap().row;
        assert_eq!(at(sm), b.row);
        (sm + 1..p.tasks.len()).take_while(|&k| p.tasks[k].level > 0).for_each(|k| assert!(at(k) > b.row && at(k) < b.row + b.rows));
    }
}

#[test]
fn progress_without_a_status_date() {
    // A plan in the future with finished work: progress is measured at the latest progress, not today (0 %).
    let mut p = plan(&[(5, ""), (5, "1")]);
    p.tasks[0].pct = 100;
    let today = d(2026, 10, 1);
    let c = Calc::new(&p, "%d-%b-%y", today);
    assert_eq!(c.progress_date(), d(2026, 11, 6));
    assert_eq!(c.summary().measured_at, "06-Nov-26");
    let r = report::period_report(&c, "week", None);
    assert!(r.actual > 40.0, "actual progress {}", r.actual);
    p.status = Some(d(2026, 11, 3));
    let c = Calc::new(&p, "%d-%b-%y", today);
    assert_eq!((c.progress_date(), c.summary().measured_at.as_str()), (d(2026, 11, 3), ""));
}

#[test]
fn workload_by_trade_with_capacity() {
    let mut p = plan(&[(5, ""), (5, ""), (5, "")]);
    for (uid, name, group) in [(1, "Alice", "Electrical"), (2, "Bob", "Electrical"), (3, "Carl", "Civil"), (4, "Dan", "")] {
        p.resources.push(Resource { uid, name: name.into(), group: group.into(), max: 1.0, ..Default::default() });
    }
    p.tasks[0].res = vec![Assign { res: 1, units: 1.0, table: 0 }, Assign { res: 2, units: 1.0, table: 0 }];
    p.tasks[1].res = vec![Assign { res: 2, units: 1.0, table: 0 }];
    p.tasks[2].res = vec![Assign { res: 3, units: 0.5, table: 0 }];
    let c = Calc::new(&p, "%d-%b-%y", d(2026, 11, 1));
    let u = report::usage(&c, "group", "work", "week");
    let row = |n: &str| u.rows.iter().find(|r| r.name == n && r.of.is_none()).unwrap_or_else(|| panic!("{n}"));
    let names: Vec<&str> = u.rows.iter().filter(|r| r.of.is_none()).map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["Civil", "Electrical", "No trade"]);
    let e = row("Electrical");
    let k = e.vals.iter().position(|&v| v > 0.0).unwrap_or(0);
    // Alice 40 h + Bob 80 h (two tasks at once) against 2 people × 40 h: over capacity.
    assert_eq!((e.vals[k], e.cap[k], e.over[k]), (120.0, 80.0, true));
    assert_eq!((row("Civil").vals[k], row("Civil").over[k]), (20.0, false));
    assert_eq!(u.person[k], 40.0);
    assert_eq!(u.rows.iter().filter(|r| r.of == Some(e.uid)).count(), 2);
    // By resource, rows now carry their capacity too.
    let r = report::usage(&c, "resource", "work", "week");
    assert_eq!(r.rows.iter().find(|x| x.name == "Bob").map(|x| (x.vals[k], x.cap[k])), Some((80.0, 40.0)));
}

#[test]
fn levelling_preview_changes_nothing() {
    let mut p = plan(&[(5, ""), (5, ""), (2, "2")]);
    p.resources.push(Resource { uid: 1, name: "Alice".into(), max: 1.0, ..Default::default() });
    p.tasks[0].res = vec![Assign { res: 1, units: 1.0, table: 0 }];
    p.tasks[1].res = vec![Assign { res: 1, units: 1.0, table: 0 }];
    let before = p.clone();
    let v = crate::level::preview(&p, &[]).unwrap_or_default();
    assert_eq!(p, before);
    // Task 1 has the float (tasks 2 and 3 are the longer path), so it is the one that waits for Alice.
    assert_eq!(v.moved.iter().map(|m| (m.name.as_str(), when(m.new_start))).collect::<Vec<_>>(), [("T1", "09 08:00".to_string())]);
    assert_eq!((when(v.finish), when(v.new_finish)), ("10 17:00".to_string(), "13 17:00".to_string()));
    assert_eq!((v.over_before, v.over_after), (5, 0));
}

#[test]
fn import_report_lists_changes_and_compares_dates() {
    let dir = std::env::temp_dir().join(format!("datum-import-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let src = |f: &str| std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/parity").join(f)).unwrap_or_default().replace("\r\n", "\n");
    let notes = |name: &str, text: &str| {
        let f = dir.join(name);
        let _ = std::fs::write(&f, text);
        match crate::io::import(&f) { Ok(crate::io::Imported::Plan(_, n)) => n, _ => panic!("{name} did not import as a plan") }
    };
    assert_eq!(notes("a.xml", &src("network.xml")), ["All 9 tasks get the same start and finish as in the file."]);
    // A relationship to an activity that is not there, a secondary constraint, and one date moved in the file.
    let xer = src("network.xer").replace("%E", "").replace("cstr_type\n", "cstr_type\tcstr_type2\n")
        .replace("2026-11-17 08:00\t2026-11-17 17:00\t", "2026-11-18 08:00\t2026-11-18 17:00\t\tCS_MSO")
        + "%R\t99\t6\t777\t1\t1\tPR_FS\t0\n%E\n";
    let n = notes("b.xer", &xer);
    assert!(n.iter().any(|x| x.starts_with("1 relationships point to activities that are not in this project")), "{n:?}");
    assert!(n.iter().any(|x| x.starts_with("1 activities have a secondary constraint")), "{n:?}");
    assert!(n.iter().any(|x| x.starts_with("1 of 9 tasks get different dates")), "{n:?}");
    assert!(n.iter().any(|x| x.contains("\"F Test\": file 2026-11-18 08:00")), "{n:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn actual_progress_follows_the_stored_periods() {
    // One 10-day task, baselined. On Wed 4 Nov the period closed at 50 %; on Mon 9 Nov it is 60 %.
    let mut doc = Doc::new(plan(&[(10, "")]));
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 }).unwrap();
    doc.p.tasks[0].act_start = Some(d(2026, 11, 2));
    (doc.p.tasks[0].pct, doc.p.status) = (50, Some(d(2026, 11, 4)));
    op(&mut doc, Op::Snapshot { label: "Week 1".into(), comment: String::new() }).unwrap();
    (doc.p.tasks[0].pct, doc.p.status) = (60, Some(d(2026, 11, 9)));
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 9));
    let sc = report::scurve(&c, "day", "duration", None);
    let at = |day: u32| sc.days.iter().position(|&x| x == at(d(2026, 11, day)) / DAY).and_then(|k| sc.actual[k]).unwrap_or(-1.0);
    // Not a straight line from 0 to 60 %: 50 % at the stored period, then on to 60 % at the status date.
    assert_eq!((at(4).round(), at(9).round()), (50.0, 60.0));
    assert!(at(3) > 30.0 && at(6) > 50.0 && at(6) < 60.0, "{} {}", at(3), at(6));
    // Earned schedule: 60 % done is where the plan was after 6 of 10 days; 6 days have passed.
    let es = crate::progress::earned_schedule(&c).unwrap();
    assert_eq!((es.es.round(), es.at, es.pd), (6.0, 6.0, 10.0));
    assert!((es.spi_t - 1.0).abs() < 0.02, "{}", es.spi_t);
}

#[test]
fn earned_schedule_when_behind() {
    // 25 % of a 10-day task done after 5 days: ES 2.5 days, SPI(t) 0.5, finish forecast at 20 days.
    let mut doc = Doc::new(plan(&[(10, "")]));
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 }).unwrap();
    doc.p.tasks[0].act_start = Some(d(2026, 11, 2));
    (doc.p.tasks[0].pct, doc.p.status) = (25, Some(d(2026, 11, 6)));
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 6));
    let es = crate::progress::earned_schedule(&c).unwrap();
    assert_eq!((es.es, es.at, es.spi_t, es.sv_t, es.ieac), (2.5, 5.0, 0.5, -2.5, 20.0));
    assert_eq!(when(es.forecast_finish), "27 17:00");
}

#[test]
fn manpower_by_trade_planned_done_and_forecast() {
    let mut p = plan(&[(5, ""), (5, "")]);
    for (uid, name, group) in [(1, "Ann", "Civil"), (2, "Ben", "Electrical")] {
        p.resources.push(Resource { uid, name: name.into(), group: group.into(), max: 1.0, ..Default::default() });
    }
    p.tasks[0].res = vec![Assign { res: 1, units: 1.0, table: 0 }];
    p.tasks[1].res = vec![Assign { res: 2, units: 2.0, table: 0 }];
    let mut doc = Doc::new(p);
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 }).unwrap();
    doc.p.tasks[0].act_start = Some(d(2026, 11, 2));
    (doc.p.tasks[0].pct, doc.p.status) = (40, Some(d(2026, 11, 3)));
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 3));
    let m = crate::progress::manpower(&c, "week");
    let t = |n: &str| m.trades.iter().find(|t| t.name == n).unwrap_or_else(|| panic!("{n}"));
    assert_eq!(m.trades.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["All trades", "Civil", "Electrical"]);
    let k = m.buckets.iter().position(|&b| b <= at(d(2026, 11, 2)) / DAY && b + 7 > at(d(2026, 11, 2)) / DAY).unwrap();
    // Planned: Ann alone, Ben's task needs two people; done so far: 40 % of Ann's 40 h = 16 h = 0.4 people-weeks.
    assert_eq!((t("Civil").planned[k], t("Electrical").planned[k], t("All trades").planned[k]), (1.0, 2.0, 3.0));
    assert_eq!(t("Civil").actual[k].map(|x| (x * 100.0).round() / 100.0), Some(0.4));
    assert!(t("Civil").forecast[k].is_some_and(|x| x > 0.4) && t("Civil").actual.get(k + 1).is_none_or(|x| x.is_none()));
}

#[test]
fn rules_of_credit_and_installed_quantities() {
    let mut doc = Doc::new(plan(&[(5, ""), (5, ""), (5, "")]));
    let (a, b) = (doc.p.tasks[0].uid, doc.p.tasks[1].uid);
    op(&mut doc, Op::ApplyCredit { uids: vec![a], rule: "Cable".into() }).unwrap();
    let t = &doc.p.tasks[0];
    assert_eq!((t.steps.len(), t.pct_type), (3, PctType::Physical));
    let mut steps = t.steps.clone();
    steps[0].done = true; // pulled: 60 %
    op(&mut doc, Op::TaskList { uid: a, field: "steps".into(), value: serde_json::to_value(&steps).unwrap() }).unwrap();
    assert_eq!(doc.p.tasks[0].phys, 60);
    // A plan's own rule replaces the built-in one of the same name.
    op(&mut doc, Op::Credit { old: None, rule: Some(CreditRule { name: "Cable".into(), steps: vec![("Pulled".into(), 1.0), ("Done".into(), 1.0)] }) }).unwrap();
    op(&mut doc, Op::ApplyCredit { uids: vec![b], rule: "Cable".into() }).unwrap();
    assert_eq!(doc.p.tasks[1].steps.len(), 2);
    assert!(op(&mut doc, Op::ApplyCredit { uids: vec![b], rule: "No such rule".into() }).is_err());
    // Installed quantities: 50 of 200 m is 25 % physical progress, and earned value uses it.
    let c = doc.p.tasks[2].uid;
    op(&mut doc, Op::Set { uid: c, field: "qty".into(), value: "200".into() }).unwrap();
    op(&mut doc, Op::Set { uid: c, field: "qtydone".into(), value: "50".into() }).unwrap();
    op(&mut doc, Op::Set { uid: c, field: "qtyunit".into(), value: "m".into() }).unwrap();
    let t = &doc.p.tasks[2];
    assert_eq!((t.phys, t.pct_type, t.qty_unit.as_str()), (25, PctType::Physical, "m"));
    assert!(op(&mut doc, Op::Set { uid: c, field: "qty".into(), value: "lots".into() }).is_err());
}

#[test]
fn lookahead_and_constraint_log() {
    let mut doc = Doc::new(plan(&[(5, ""), (10, "1"), (5, "2"), (5, "3")]));
    doc.p.status = Some(d(2026, 11, 6));
    let t = &mut doc.p.tasks[0];
    (t.act_start, t.act_finish, t.pct) = (Some(d(2026, 11, 2)), Some(d(2026, 11, 6)), 100);
    let (b, c3) = (doc.p.tasks[1].uid, doc.p.tasks[2].uid);
    op(&mut doc, Op::Log { id: 0, item: Some(LogItem { uid: b, kind: "Material".into(), text: "Cable drums on site".into(), need: Some(d(2026, 11, 5)), ..Default::default() }) }).unwrap();
    op(&mut doc, Op::Log { id: 0, item: Some(LogItem { uid: c3, kind: "Permit".into(), text: "Hot work permit".into(), status: "Cleared".into(), ..Default::default() }) }).unwrap();
    assert!(op(&mut doc, Op::Log { id: 0, item: Some(LogItem { uid: 999, ..Default::default() }) }).is_err());
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 6));
    let la = crate::progress::lookahead(&c, 3);
    // From Sat 7 Nov for three weeks: tasks 2 (9–20 Nov) and 3 (23–27 Nov); task 1 ends before, task 4 starts after.
    assert_eq!(la.rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["T2", "T3"]);
    assert_eq!((la.rows[0].open, la.rows[0].overdue, la.rows[1].open), (1, 1, 0));
    assert_eq!(doc.p.log.iter().map(|x| (x.id, x.status.as_str())).collect::<Vec<_>>(), [(1, "Open"), (2, "Cleared")]);
    op(&mut doc, Op::Log { id: 1, item: None }).unwrap();
    assert_eq!(doc.p.log.len(), 1);
}

#[test]
fn long_spans_count_working_time_fast_and_right() {
    // The whole-week shortcut must agree with counting day by day, holidays included.
    let mut cal = Calendar::standard("S");
    cal.exceptions.push(Exception { name: "Xmas".into(), from: d(2026, 12, 24), to: d(2027, 1, 1), hours: vec![] });
    cal.exceptions.push(Exception { name: "Sat shift".into(), from: d(2027, 2, 6), to: d(2027, 2, 6), hours: vec![(480, 720)] });
    let c = WorkCal::new(&cal);
    let slow = |a: i64, b: i64| { let mut n = 0; let mut t = a; while t < b { n += c.between(t, (t + 20 * DAY).min(b)); t += 20 * DAY; } n };
    for (a, b) in [(at(d(2026, 11, 2)) + 9 * H, at(d(2027, 3, 17)) + 15 * H), (at(d(2026, 12, 20)), at(d(2027, 2, 9))), (at(d(2026, 1, 1)) + 1, at(d(2030, 6, 30)) + 777)] {
        assert_eq!(c.between(a, b), slow(a, b));
    }
    let t = std::time::Instant::now();
    assert!(c.between(at(d(1900, 1, 1)), at(d(2500, 1, 1))) > 0);
    assert!(t.elapsed().as_millis() < 200);
}

#[test]
fn exports_cannot_run_formulas_and_saves_are_atomic() {
    let dir = std::env::temp_dir().join(format!("datum-safe-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let mut p = plan(&[(5, ""), (3, "1")]);
    p.tasks[0].name = "=HYPERLINK(\"http://x\",\"click\")".into();
    p.tasks[1].name = "-2+3".into();
    let c = Calc::new(&p, "%d-%b-%y", d(2026, 11, 1));
    let f = dir.join("a.csv");
    crate::io::table::write_csv(&c, &["name".to_string(), "tf".to_string()], &f).unwrap();
    let text = std::fs::read_to_string(&f).unwrap();
    assert!(text.contains("\"'=HYPERLINK(") && text.contains("'-2+3"), "{text}");
    assert!(!std::fs::read_dir(&dir).unwrap().any(|e| e.unwrap().path().to_string_lossy().ends_with(".saving")));
    // A file that is too large is refused before it is read.
    let big = dir.join("big.dplan");
    let fh = std::fs::File::create(&big).unwrap();
    fh.set_len(crate::doc::MAX_FILE + 1).unwrap();
    assert!(Doc::open(&big).is_err_and(|e| e.contains("MB")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn money_is_exact_to_the_cent() {
    // 3 tasks at $33.333/h for 3 h: each costs $99.999 → $100.00, and the project $300.00, not $299.997.
    let mut p = plan(&[(0, ""), (0, ""), (0, "")]);
    p.resources.push(Resource { uid: 1, name: "Tech".into(), rate: 33.333, max: 1.0, ..Default::default() });
    for t in &mut p.tasks { t.dur = 180; t.res = vec![Assign { res: 1, units: 1.0, table: 0 }] }
    let all = p.new_task("All", 0, 0);
    p.tasks.insert(0, all);
    for t in &mut p.tasks[1..] { t.level = 1 }
    let s = schedule(&p).unwrap();
    let l = load::compute(&p, &s);
    assert_eq!((l.cost[1], l.cost[0]), (100.0, 300.0));
    // A typed fixed cost keeps whole cents.
    let mut doc = Doc::new(p);
    let uid = doc.p.tasks[1].uid;
    op(&mut doc, Op::Set { uid, field: "fixedcost".into(), value: "$1,234.5678".into() }).unwrap();
    assert_eq!(doc.p.tasks[1].fixed_cost, 1234.57);
}

#[test]
fn a_long_hand_back_chain_between_two_plans_settles() {
    // Two plans hand work back and forth 15 times (a0 → b0 → a1 → … → b7): no loop, so it must settle.
    let mut a = Project::new("A", d(2026, 11, 2));
    let mut b = Project::new("B", d(2026, 11, 2));
    for k in 0..8 {
        let t = a.new_task(&format!("a{k}"), 0, 480); a.tasks.push(t);
        let t = b.new_task(&format!("b{k}"), 0, 480); b.tasks.push(t);
    }
    let x = |file: &str, uid: u32| crate::model::ExtLink { file: file.into(), uid, kind: LinkType::FS, lag: 0, plan: String::new(), code: String::new(), name: String::new(), start: 0, finish: 0 };
    for k in 0..8 {
        let (ua, ub) = (a.tasks[k].uid, b.tasks[k].uid);
        b.tasks[k].ext.push(x("A.dplan", ua));
        if k + 1 < 8 { a.tasks[k + 1].ext.push(x("B.dplan", ub)); }
    }
    let mut plans = vec![("A.dplan".to_string(), a), ("B.dplan".to_string(), b)];
    crate::links::update_all(&mut plans).expect("no loop");
    let s = schedule(&plans[1].1).unwrap();
    assert_eq!(crate::cal::date_of(s.ef[7] - 1), d(2026, 11, 23), "b7 is the 16th one-day task in a row");
}
