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
