//! Checked cases: a planner's day on the task sheet (rows, links, progress, undo, exchange).
use crate::{cal::date_of, doc::{Doc, Op}, sched::schedule, *};
use chrono::NaiveDate;

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
const F: &str = "%d-%b-%y";

/// A plan from rows "name | duration | preds | field=value; …" set through the edits the window sends.
fn doc(rows: &[&str]) -> Doc {
    let mut doc = Doc::new(Project::new("Case", d(2026, 11, 2)));
    for (i, _) in rows.iter().enumerate() { op(&mut doc, Op::Insert { at: i, name: format!("T{}", i + 1), ms: false }) }
    for (i, r) in rows.iter().enumerate() {
        let mut parts = r.split('|').map(str::trim);
        let uid = doc.p.tasks[i].uid;
        for (f, v) in [("dur", parts.next().unwrap_or("")), ("preds", parts.next().unwrap_or(""))] {
            if !v.is_empty() { op(&mut doc, Op::Set { uid, field: f.into(), value: v.into() }) }
        }
        for kv in parts.next().unwrap_or("").split(';').map(str::trim).filter(|s| !s.is_empty()) {
            let (k, v) = kv.split_once('=').unwrap();
            op(&mut doc, Op::Set { uid, field: k.trim().into(), value: v.trim().into() });
        }
    }
    doc
}
fn op(doc: &mut Doc, o: Op) { doc.apply(o, F).unwrap_or_else(|e| panic!("{e}")) }
fn uids(doc: &Doc, rows: &[usize]) -> Vec<u32> { rows.iter().map(|&i| doc.p.tasks[i].uid).collect() }
fn names(doc: &Doc) -> Vec<String> { doc.p.tasks.iter().map(|t| format!("{}{}", " ".repeat(t.level as usize), t.name)).collect() }
fn at(t: i64) -> String { format!("{} {:02}:{:02}", date_of(t).format("%b%d"), t.rem_euclid(1440) / 60, t.rem_euclid(60)) }
fn dates(p: &Project) -> Vec<String> {
    let s = schedule(p).unwrap_or_else(|e| panic!("{e}"));
    (0..p.tasks.len()).map(|i| format!("{} {} {}", at(s.es[i]), at(s.ef[i]), s.tf[i] as f64 / 480.0)).collect()
}

#[test]
fn moving_a_block_at_the_top_keeps_its_order() {
    let mut dc = doc(&["1d||", "1d||", "1d||", "1d||"]);
    let u = uids(&dc, &[0, 1]);
    let _ = dc.apply(Op::Move { uids: u, up: true }, F);
    assert_eq!(names(&dc), ["T1", "T2", "T3", "T4"]);
    let u = uids(&dc, &[2, 3]);
    let _ = dc.apply(Op::Move { uids: u, up: false }, F);
    assert_eq!(names(&dc), ["T1", "T2", "T3", "T4"]);
    // A block in the middle moves as one.
    let u = uids(&dc, &[1, 2]);
    op(&mut dc, Op::Move { uids: u.clone(), up: true });
    assert_eq!(names(&dc), ["T2", "T3", "T1", "T4"]);
    op(&mut dc, Op::Move { uids: u, up: false });
    let u = uids(&dc, &[1, 2]);
    op(&mut dc, Op::Move { uids: u, up: false });
    assert_eq!(names(&dc), ["T1", "T4", "T2", "T3"]);
}

#[test]
fn moving_a_summary_with_a_selected_child_keeps_the_child_inside() {
    let mut dc = doc(&["1d||", "1d||", "1d||", "1d||"]);
    let u = uids(&dc, &[2, 3]);
    op(&mut dc, Op::Indent { uids: u });
    // T1, T2 { T3, T4 }: select T2 and T3 (its first child) and move up.
    let u = uids(&dc, &[1, 2]);
    op(&mut dc, Op::Move { uids: u, up: true });
    assert_eq!(names(&dc), ["T2", " T3", " T4", "T1"]);
}

#[test]
fn remaining_duration_longer_than_planned_extends_the_task() {
    // 5d task started Mon 2 Nov, 40 % done (2d); the foreman says 6 more days are needed.
    let mut dc = doc(&["5d||astart=02-Nov-26; pct=40"]);
    dc.p.status = Some(d(2026, 11, 3));
    let uid = dc.p.tasks[0].uid;
    op(&mut dc, Op::Set { uid, field: "rem".into(), value: "6d".into() });
    let t = &dc.p.tasks[0];
    assert_eq!((t.act_start, t.dur), (Some(d(2026, 11, 2)), 8 * 480), "2 days done + 6 remaining");
    assert_eq!(t.pct, 25);
    // Less remaining than that still reads as progress, without undoing work done: 4d left of 8d.
    op(&mut dc, Op::Set { uid, field: "rem".into(), value: "4d".into() });
    assert_eq!((dc.p.tasks[0].dur, dc.p.tasks[0].pct), (8 * 480, 50));
    // More remaining than its duration on a task not started is its new duration.
    let mut dc = doc(&["5d||"]);
    let uid = dc.p.tasks[0].uid;
    op(&mut dc, Op::Set { uid, field: "rem".into(), value: "7d".into() });
    assert_eq!((dc.p.tasks[0].dur, dc.p.tasks[0].pct, dc.p.tasks[0].act_start), (7 * 480, 0, None));
}

#[test]
fn paste_keeps_links_inside_the_copy_and_undo_restores() {
    let mut dc = doc(&["2d||", "3d|1|", "1d|2SS+1d|", "1d|3|"]);
    let u = uids(&dc, &[1, 2]);
    op(&mut dc, Op::Indent { uids: u });
    let before = dc.p.clone();
    let tasks = doc::copy(&dc.p, &uids(&dc, &[0]));
    assert_eq!(tasks.len(), 3);
    op(&mut dc, Op::Paste { at: 4, tasks });
    assert_eq!(names(&dc), ["T1", " T2", " T3", "T4", "T1", " T2", " T3"]);
    let (a, b) = (dc.p.tasks[5].uid, &dc.p.tasks[6]);
    assert_eq!((b.preds.len(), b.preds[0].pred, b.preds[0].kind, b.preds[0].lag), (1, a, LinkType::SS, 480));
    let ds = dates(&dc.p);
    assert_eq!(ds[4], ds[0]);
    dc.undo();
    assert!(dc.p == before);
    dc.redo();
    assert_eq!(dc.p.tasks.len(), 7);
}

#[test]
fn deleting_a_linked_task_drops_its_links_and_undo_brings_them_back() {
    let mut dc = doc(&["2d||", "3d|1|", "1d|2|"]);
    let u = uids(&dc, &[1]);
    op(&mut dc, Op::Delete { uids: u });
    assert!(dc.p.tasks[1].preds.is_empty());
    assert_eq!(dates(&dc.p)[1], "Nov02 08:00 Nov02 17:00 1");
    dc.undo();
    assert_eq!(dc.p.tasks[2].preds.len(), 1);
    assert_eq!(dates(&dc.p)[2], "Nov09 08:00 Nov09 17:00 0");
}

#[test]
fn a_hand_checked_network_with_every_link_type() {
    // A 4d; B 3d FS+2d after A; C 2d SS+1d after A; D 5d FF after C and FS-1d after B; E 1d SF+2d from D (finish ≥ D start + 2d).
    // A Mon 2 – Thu 5. B Tue 10 – Thu 12 (2d lag over Fri 6 and Mon 9). C Tue 3 – Wed 4.
    // D: FS-1d from B → start Thu 12 08:00 (B ends Thu 12 17:00, minus 1d); FF from C → finish ≥ Wed 4 17:00.
    //   D Thu 12 – Wed 18. E: finish ≥ D start + 2d = Mon 16 08:00 → E finish Fri 13 17:00 at the end of a shift… it starts Fri 13.
    let dc = doc(&["4d||", "3d|1FS+2d|", "2d|1SS+1d|", "5d|3FF,2FS-1d|", "1d|4SF+2d|"]);
    assert_eq!(dates(&dc.p), [
        "Nov02 08:00 Nov05 17:00 0", "Nov10 08:00 Nov12 17:00 0", "Nov03 08:00 Nov04 17:00 10",
        "Nov12 08:00 Nov18 17:00 0", "Nov13 08:00 Nov13 17:00 3",
    ]);
}

#[test]
fn recurring_meeting_every_month_on_a_weekend_day() {
    let mut dc = Doc::new(Project::new("R", d(2026, 11, 2)));
    op(&mut dc, Op::Recurring { at: 0, name: "Review".into(), dur: "2h".into(), every: "month".into(), interval: 1, count: 3, start: "31-Oct-26".into() });
    assert_eq!(names(&dc), ["Review", " Review 1", " Review 2", " Review 3"]);
    let ds = dates(&dc.p);
    // 31 Oct is a Saturday before the project start; 30 Nov a Monday; 31 Dec a Thursday.
    assert_eq!(&ds[1][..11], "Nov02 08:00");
    assert_eq!(&ds[2][..11], "Nov30 08:00");
    assert_eq!(&ds[3][..11], "Dec31 08:00");
    dc.undo();
    assert!(dc.p.tasks.is_empty());
}

#[test]
fn setting_a_finish_date_keeps_the_typed_unit_sensible() {
    let mut dc = doc(&["1w||"]);
    let uid = dc.p.tasks[0].uid;
    op(&mut dc, Op::Set { uid, field: "finish".into(), value: "13-Nov-26".into() });
    assert_eq!(dc.p.tasks[0].dur, 10 * 480);
    let c = crate::fields::Calc::new(&dc.p, F, d(2026, 11, 2));
    assert_eq!(c.text(0, "dur"), "2 wks");
    assert_eq!(c.text(0, "finish"), "13-Nov-26");
}

/// A plan with a bit of everything a planner sets on a real job.
fn busy() -> Doc {
    let mut dc = doc(&[
        "0||", "4d||", "3d|2FS+2d|ctype=Finish No Later Than; cdate=13-Nov-26", "2d|2SS+1d|", "5d|4FF,3FS-1d|deadline=18-Nov-26",
        "1d|5SF+2d|", "0|5|", "2ed|7|", "3d|8|ctype=Must Start On; cdate=23-Nov-26", "2d|9FS-4h|",
    ]);
    let u = uids(&dc, &[1, 2, 3, 4]);
    op(&mut dc, Op::Indent { uids: u });
    let night = vec![(0, 360), (1320, 1440)];
    let mut cal = Calendar::standard("Nights");
    cal.week = [vec![(1320, 1440)], night.clone(), night.clone(), night.clone(), night, vec![(0, 360)], vec![]];
    op(&mut dc, Op::CalPut { old: None, cal });
    let mut c0 = dc.p.calendars[0].clone();
    c0.exceptions.push(Exception { name: "Holiday".into(), from: d(2026, 11, 26), to: d(2026, 11, 27), hours: vec![] });
    op(&mut dc, Op::CalPut { old: Some(c0.name.clone()), cal: c0 });
    for (r, f, v) in [(9, "cal", "Nights"), (1, "astart", "02-Nov-26"), (1, "afinish", "05-Nov-26"), (3, "astart", "03-Nov-26"), (3, "pct", "50")] {
        let uid = dc.p.tasks[r].uid;
        op(&mut dc, Op::Set { uid, field: f.into(), value: v.into() });
    }
    dc.p.status = Some(d(2026, 11, 4));
    op(&mut dc, Op::Baseline { uids: vec![], clear: false, n: 0 });
    dc
}

fn round_trip(p: &Project, ext: &str) -> Project {
    let dir = std::env::temp_dir().join(format!("datum-plan-cases-{}-{ext}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join(format!("plan.{ext}"));
    crate::io::export(p, &f, F, d(2026, 11, 4), &[]).unwrap();
    let crate::io::Imported::Plan(q, _) = crate::io::import(&f).unwrap() else { panic!("{ext}") };
    std::fs::remove_dir_all(dir).ok();
    *q
}

#[test]
fn exchange_files_keep_every_date_link_and_progress() {
    let dc = busy();
    let want = dates(&dc.p);
    for ext in ["xml", "xer"] {
        let q = round_trip(&dc.p, ext);
        let by_name = |p: &Project| { let mut v: Vec<String> = dates(p).into_iter().zip(&p.tasks).map(|(d, t)| format!("{} {d}", t.name)).collect(); v.sort(); v };
        assert_eq!(by_name(&q), by_name(&dc.p), "{ext}: dates");
        assert_eq!(q.tasks.iter().map(|t| &t.name).collect::<Vec<_>>(), dc.p.tasks.iter().map(|t| &t.name).collect::<Vec<_>>(), "{ext}: order");
        assert_eq!(dates(&q), want, "{ext}: dates");
        for (a, b) in dc.p.tasks.iter().zip(&q.tasks) {
            let links = |t: &Task| t.preds.iter().map(|l| (l.kind, l.lag)).collect::<Vec<_>>();
            assert_eq!(links(a), links(b), "{ext} {}: links", a.name);
            assert_eq!((a.constraint, a.cdate, a.deadline), (b.constraint, b.cdate, b.deadline), "{ext} {}: constraint", a.name);
            assert_eq!((a.act_start, a.act_finish, a.pct), (b.act_start, b.act_finish, b.pct), "{ext} {}: progress", a.name);
            assert_eq!((a.level, a.dur * (a.level > 0) as i64, a.elapsed, &a.cal), (b.level, b.dur * (b.level > 0) as i64, b.elapsed, &b.cal), "{ext} {}: outline, duration, calendar", a.name);
            // XER keeps baselines as separate projects, not on the activity.
            if ext == "xml" { assert_eq!(a.bls.get(&0).map(|x| (x.start, x.finish)), b.bls.get(&0).map(|x| (x.start, x.finish)), "{ext} {}: baseline", a.name) }
        }
        assert_eq!(q.status, dc.p.status, "{ext}: status date");
    }
}

#[test]
fn a_task_list_with_progress_starts_each_task_after_its_predecessors() {
    let dir = std::env::temp_dir().join(format!("datum-plan-csv-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("list.csv");
    std::fs::write(&f, "ID,Name,Duration,Predecessors,% Complete\n1,Dig,5d,,100\n2,Pour,5d,1,40\n3,Cure,3d,2FS-1d,0\n").unwrap();
    let crate::io::Imported::Table(t) = crate::io::import(&f).unwrap() else { panic!() };
    std::fs::remove_dir_all(dir).ok();
    let (p, bad) = crate::io::table::to_project(&t, &t.guess, "Q", d(2026, 11, 2), F).unwrap();
    assert_eq!(bad, 0);
    assert_eq!((p.tasks[0].act_start, p.tasks[0].act_finish), (Some(d(2026, 11, 2)), Some(d(2026, 11, 6))));
    // Pour began when Dig was done, not on the first day of the plan.
    assert_eq!((p.tasks[1].act_start, p.tasks[1].pct), (Some(d(2026, 11, 9)), 40));
    assert_eq!(p.tasks[2].preds[0].lag, -480);
}

#[test]
fn a_percent_lag_survives_changing_the_link_type_in_the_details_pane() {
    // The Predecessors tab shows a percent lag as "50%" and sends it back when the type changes.
    let mut dc = doc(&["4d||", "2d|1SS+50%|"]);
    let (a, b) = (dc.p.tasks[0].uid, dc.p.tasks[1].uid);
    op(&mut dc, Op::LinkSet { pred: a, succ: b, kind: Some(LinkType::FF), lag: "50%".into() });
    let l = dc.p.tasks[1].preds[0].clone();
    assert_eq!((l.kind, l.lag, l.pct), (LinkType::FF, 50, true));
    // An overlap by a share of the predecessor, typed with a minus sign as the column's tip shows.
    op(&mut dc, Op::LinkSet { pred: a, succ: b, kind: Some(LinkType::FS), lag: "\u{2212}25%".into() });
    let l = dc.p.tasks[1].preds[0].clone();
    assert_eq!((l.kind, l.lag, l.pct), (LinkType::FS, -25, true));
    assert_eq!(dates(&dc.p)[1], "Nov05 08:00 Nov06 17:00 0");
    op(&mut dc, Op::LinkSet { pred: a, succ: b, kind: Some(LinkType::FS), lag: "-1d".into() });
    assert_eq!((dc.p.tasks[1].preds[0].lag, dc.p.tasks[1].preds[0].pct), (-480, false));
}

#[test]
fn renaming_a_calendar_keeps_the_people_on_it() {
    let mut dc = doc(&["2d||res=Night crew"]);
    let mut cal = Calendar::standard("Nights");
    cal.week = [vec![(1320, 1440)], vec![(0, 360), (1320, 1440)], vec![(0, 360), (1320, 1440)], vec![(0, 360), (1320, 1440)], vec![(0, 360), (1320, 1440)], vec![(0, 360)], vec![]];
    op(&mut dc, Op::CalPut { old: None, cal: cal.clone() });
    let r = dc.p.resources[0].uid;
    op(&mut dc, Op::ResJson { uid: r, field: "cal".into(), value: serde_json::json!("Nights") });
    op(&mut dc, Op::ProjectSet { field: "opts".into(), value: serde_json::json!({ "res_cals": true }) });
    let before = dates(&dc.p);
    assert_eq!(&before[0][..11], "Nov02 22:00");
    cal.name = "Night shift".into();
    op(&mut dc, Op::CalPut { old: Some("Nights".into()), cal });
    assert_eq!(dc.p.resources[0].cal.as_deref(), Some("Night shift"));
    assert_eq!(dates(&dc.p), before);
}

#[test]
fn an_elapsed_lag_from_a_project_xml_file_keeps_its_length() {
    // Pour 5d, then Strip forms 7 elapsed days later (concrete curing): the file says LagFormat 8 (edays).
    let dc = doc(&["5d||", "2d|1FS+7d|"]);
    let dir = std::env::temp_dir().join(format!("datum-plan-elag-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("plan.xml");
    crate::io::export(&dc.p, &f, F, d(2026, 11, 2), &[]).unwrap();
    let text = std::fs::read_to_string(&f).unwrap().replace("<LinkLag>33600</LinkLag><LagFormat>7</LagFormat>", "<LinkLag>100800</LinkLag><LagFormat>8</LagFormat>");
    std::fs::write(&f, text).unwrap();
    let crate::io::Imported::Plan(q, notes) = crate::io::import(&f).unwrap() else { panic!() };
    std::fs::remove_dir_all(dir).ok();
    assert!(!notes.iter().any(|n| n.contains("elapsed")), "{notes:?}");
    // Fri 6 Nov 17:00 + 7 days round the clock = Fri 13 Nov 17:00: Strip forms starts Mon 16 Nov.
    let l = &q.tasks[1].preds[0];
    assert_eq!((l.lag, l.elapsed, l.pct), (7 * 1440, true, false));
    assert_eq!(&dates(&q)[1][..11], "Nov16 08:00");
    // Written back as it came: 7 edays.
    let s = crate::sched::schedule(&q).unwrap();
    assert!(crate::io::mspdi::write(&q, &s, &[0.0; 2]).contains("<LinkLag>100800</LinkLag><LagFormat>8</LagFormat>"));
}

#[test]
fn update_project_then_baseline_variance_and_undo() {
    let mut dc = doc(&["5d||", "5d|1|", "0|2|"]);
    op(&mut dc, Op::Baseline { uids: vec![], clear: false, n: 0 });
    op(&mut dc, Op::UpdateProject { through: "04-Nov-26".into(), reschedule: false });
    let t = &dc.p.tasks[0];
    assert_eq!((t.pct, t.act_start, dc.p.status), (60, Some(d(2026, 11, 2)), Some(d(2026, 11, 4))));
    assert_eq!(dates(&dc.p), ["Nov02 08:00 Nov06 17:00 0", "Nov09 08:00 Nov13 17:00 0", "Nov13 17:00 Nov13 17:00 0"]);
    // The crew says 4 more days on A (it slips a day): B and the milestone slip a day against the baseline.
    let uid = dc.p.tasks[0].uid;
    op(&mut dc, Op::Set { uid, field: "rem".into(), value: "3d".into() });
    let c = crate::fields::Calc::new(&dc.p, F, d(2026, 11, 4));
    assert_eq!((c.text(2, "finish"), c.text(2, "vfinish"), c.text(2, "b0finish")), ("16-Nov-26".into(), "1 day".into(), "13-Nov-26".into()));
    dc.undo();
    assert_eq!(dc.p.tasks[0].dur, 5 * 480);
    dc.undo();
    assert_eq!((dc.p.tasks[0].pct, dc.p.status), (0, None));
}

#[test]
fn levelling_one_crew_then_clearing_and_undo() {
    // Two 3d tasks for the same crew and a 1d task after both.
    let mut dc = doc(&["3d||res=Crew", "3d||res=Crew", "1d|1,2|"]);
    let before = dates(&dc.p);
    op(&mut dc, Op::Level { uids: vec![], clear: false });
    // Float stays logic-only (the first task could slip 3 days if the crew allowed it).
    assert_eq!(dates(&dc.p), ["Nov02 08:00 Nov04 17:00 3", "Nov05 08:00 Nov09 17:00 0", "Nov10 08:00 Nov10 17:00 0"]);
    op(&mut dc, Op::Level { uids: vec![], clear: true });
    assert_eq!(dates(&dc.p), before);
    dc.undo();
    assert_eq!(&dates(&dc.p)[1][..11], "Nov05 08:00");
    // Half-time on each: both fit side by side, nothing moves.
    let mut dc = doc(&["3d||res=Crew[50%]", "3d||res=Crew[50%]"]);
    op(&mut dc, Op::Level { uids: vec![], clear: false });
    assert_eq!(&dates(&dc.p)[1][..11], "Nov02 08:00");
}

#[test]
fn an_xer_activity_measured_by_duration_takes_its_progress_from_the_remaining_duration() {
    // In an XER file the physical % column is only the physical %: an activity measured by duration
    // shows its progress in the remaining duration (here 2 of 4 days left), with physical % at 0.
    let dc = doc(&["4d||astart=02-Nov-26; pct=50"]);
    let mut p = dc.p.clone();
    p.status = Some(d(2026, 11, 3));
    let dir = std::env::temp_dir().join(format!("datum-plan-xerpct-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("plan.xer");
    crate::io::export(&p, &f, F, d(2026, 11, 3), &[]).unwrap();
    let text = std::fs::read_to_string(&f).unwrap();
    assert!(text.contains("\t50\tCP_Drtn\t"));
    std::fs::write(&f, text.replace("\t50\tCP_Drtn\t", "\t0\tCP_Drtn\t")).unwrap();
    let crate::io::Imported::Plan(q, _) = crate::io::import(&f).unwrap() else { panic!() };
    std::fs::remove_dir_all(dir).ok();
    assert_eq!((q.tasks[0].pct, q.tasks[0].pct_type), (50, PctType::Duration));
    assert_eq!(dates(&q), dates(&p));
}

#[test]
fn typing_or_dragging_the_dates_of_a_task_under_way_moves_its_actual_dates() {
    // A started Mon 2 Nov (40 %), B finished Mon 2 – Tue 3. The Gantt sends "start" and "finish" when a bar is dragged.
    let mut dc = doc(&["5d||astart=02-Nov-26; pct=40", "2d||astart=02-Nov-26; afinish=03-Nov-26", "1d|2|"]);
    let (a, b) = (dc.p.tasks[0].uid, dc.p.tasks[1].uid);
    op(&mut dc, Op::Set { uid: a, field: "start".into(), value: "03-Nov-26".into() });
    assert_eq!((dc.p.tasks[0].act_start, dc.p.tasks[0].constraint), (Some(d(2026, 11, 3)), Constraint::ASAP));
    assert_eq!(&dates(&dc.p)[0][..11], "Nov03 08:00");
    op(&mut dc, Op::Set { uid: b, field: "finish".into(), value: "04-Nov-26".into() });
    assert_eq!((dc.p.tasks[1].act_finish, dc.p.tasks[1].dur), (Some(d(2026, 11, 4)), 3 * 480));
    assert_eq!(&dates(&dc.p)[2][..11], "Nov05 08:00");
    assert!(dc.apply(Op::Set { uid: b, field: "start".into(), value: "05-Nov-26".into() }, F).is_err());
}

#[test]
fn a_daily_meeting_falls_on_working_days() {
    let mut dc = Doc::new(Project::new("R", d(2026, 11, 2)));
    op(&mut dc, Op::Recurring { at: 0, name: "Stand-up".into(), dur: "15m".into(), every: "day".into(), interval: 1, count: 7, start: "02-Nov-26".into() });
    let days: Vec<String> = dates(&dc.p)[1..].iter().map(|x| x[..5].to_string()).collect();
    assert_eq!(days, ["Nov02", "Nov03", "Nov04", "Nov05", "Nov06", "Nov09", "Nov10"]);
    // Every second working day.
    let mut dc = Doc::new(Project::new("R", d(2026, 11, 2)));
    op(&mut dc, Op::Recurring { at: 0, name: "Inspection".into(), dur: "1h".into(), every: "day".into(), interval: 2, count: 4, start: "05-Nov-26".into() });
    let days: Vec<String> = dates(&dc.p)[1..].iter().map(|x| x[..5].to_string()).collect();
    assert_eq!(days, ["Nov05", "Nov09", "Nov11", "Nov13"]);
}

#[test]
fn a_csv_round_trip_keeps_the_outline_links_and_dates() {
    let mut dc = doc(&["0||", "4d||", "3d|2FS+2d|", "2d|2SS+1d|", "5d|4FF,3FS-1d|", "1d|5SF+2d|", "0|5|", "2ed|7|", "1w|1|"]);
    let u = uids(&dc, &[1, 2, 3, 4]);
    op(&mut dc, Op::Indent { uids: u });
    let dir = std::env::temp_dir().join(format!("datum-plan-csvrt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("plan.csv");
    crate::io::export(&dc.p, &f, F, d(2026, 11, 2), &[]).unwrap();
    let crate::io::Imported::Table(t) = crate::io::import(&f).unwrap() else { panic!() };
    std::fs::remove_dir_all(dir).ok();
    let (q, bad) = crate::io::table::to_project(&t, &t.guess, "Q", d(2026, 11, 2), F).unwrap();
    assert_eq!(bad, 0);
    assert_eq!(names(&Doc::new(q.clone())), names(&dc));
    let ends = |p: &Project| dates(p).into_iter().map(|x| x[..23].to_string()).collect::<Vec<_>>();
    assert_eq!(ends(&q), ends(&dc.p));
}

#[test]
fn typing_a_finish_date_on_a_milestone_moves_it() {
    let mut dc = doc(&["5d||", "0|1|"]);
    let uid = dc.p.tasks[1].uid;
    op(&mut dc, Op::Set { uid, field: "finish".into(), value: "11-Nov-26".into() });
    assert_eq!(dc.p.tasks[1].dur, 0);
    assert_eq!(dates(&dc.p)[1], "Nov11 17:00 Nov11 17:00 0");
}

#[test]
fn an_actual_duration_longer_than_planned_is_how_long_the_task_took() {
    let mut dc = doc(&["5d||", "1d|1|"]);
    let uid = dc.p.tasks[0].uid;
    op(&mut dc, Op::Set { uid, field: "adur".into(), value: "7d".into() });
    let t = &dc.p.tasks[0];
    assert_eq!((t.dur, t.pct, t.act_start, t.act_finish), (7 * 480, 100, Some(d(2026, 11, 2)), Some(d(2026, 11, 10))));
    assert_eq!(&dates(&dc.p)[1][..11], "Nov11 08:00");
}
