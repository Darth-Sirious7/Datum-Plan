//! Checked cases: the scheduling engine and links between plans (0.5.12 audit).
use crate::{cal::date_of, doc::set, sched::schedule, *};
use chrono::NaiveDate;

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap_or_default() }

fn build(rows: &[&str], tweak: impl FnOnce(&mut Project)) -> Project {
    let mut p = Project::new("Case", d(2026, 11, 2));
    for i in 0..rows.len() {
        let t = p.new_task(&format!("T{}", i + 1), 0, 0);
        p.tasks.push(t);
    }
    tweak(&mut p);
    for (i, r) in rows.iter().enumerate() {
        let mut parts = r.split('|').map(str::trim);
        let (dur, preds, extra) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        set(&mut p, i, "dur", dur, "%d-%b-%y").unwrap_or_else(|e| panic!("row {}: {e}", i + 1));
        set(&mut p, i, "preds", preds, "%d-%b-%y").unwrap_or_else(|e| panic!("row {}: {e}", i + 1));
        for kv in extra.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            set(&mut p, i, k.trim(), v.trim(), "%d-%b-%y").unwrap_or_else(|e| panic!("row {} {k}: {e}", i + 1));
        }
    }
    p
}

fn at(t: i64) -> String { format!("{} {:02}:{:02}", date_of(t).format("%b%d"), t.rem_euclid(1440) / 60, t.rem_euclid(60)) }

fn dates(p: &Project) -> Vec<String> {
    let s = schedule(p).unwrap_or_else(|e| panic!("{e}"));
    (0..p.tasks.len()).map(|i| {
        let f = s.tf[i] as f64 / 480.0;
        format!("{} {} {}", at(s.es[i]), at(s.ef[i]), if f.fract() == 0.0 { format!("{f}") } else { format!("{f:.2}") })
    }).collect()
}

fn ext(kind: LinkType, lag: i64, start: i64, finish: i64) -> ExtLink {
    ExtLink { file: "Other.dplan".into(), uid: 7, kind, lag, plan: "Other".into(), code: String::new(), name: "X".into(), start, finish }
}

#[test]
fn summary_float_ignores_finished_children() {
    // Summary 1 holds A (done) and B (2d, free); C 10d sets the finish. B has 8d of float.
    let mut p = build(&["0||", "2d||astart=02-Nov-26; afinish=03-Nov-26", "2d||", "10d||"], |_| {});
    for t in &mut p.tasks[1..3] { t.level = 1 }
    let s = schedule(&p).unwrap();
    assert_eq!(s.tf[2], 8 * 480);
    assert_eq!(s.tf[0], 8 * 480, "summary float should come from its unfinished work");
    assert!(!s.critical(&p, 0), "summary is not critical");
}

#[test]
fn ext_lag_uses_lag_calendar() {
    // Same as cases.rs "lag on 24 h" but the predecessor is in another plan: it finishes Fri 6 Nov 17:00.
    let mut p = build(&["1d||"], |p| p.opts.lag_cal = LagCal::H24);
    let fri = crate::cal::at(d(2026, 11, 6)) + 17 * 60;
    p.tasks[0].ext.push(ext(LinkType::FS, 2 * 480, fri - 5 * 480, fri));
    assert_eq!(dates(&p)[0], "Nov09 08:00 Nov09 17:00 0");
}

#[test]
fn remaining_work_excludes_past_split_gaps() {
    // 5d split after 1d by a 2d pause: Mon 2 work, Tue–Wed pause, Thu 5 – Tue 10 work.
    // Status Wed 4: 1 of 5 days done (20 %); 4 days remain from Thu 5, finishing Tue 10.
    let mut p = build(&["5d||astart=02-Nov-26; pct=20"], |p| p.status = Some(d(2026, 11, 4)));
    p.tasks[0].splits = vec![(480, 960)];
    assert_eq!(dates(&p)[0], "Nov02 08:00 Nov10 17:00 0");
}

#[test]
fn two_people_start_when_the_first_one_can() {
    // Ahmed is off Mon 2 Nov; Bina works. 2d task for both: Bina starts Mon 2, Ahmed works Tue–Wed.
    let mut p = build(&["2d||res=Ahmed, Bina"], |_| {});
    let r = p.resources.iter_mut().find(|r| r.name == "Ahmed").unwrap();
    r.exceptions.push(Exception { name: "Leave".into(), from: d(2026, 11, 2), to: d(2026, 11, 2), hours: vec![] });
    p.opts.res_cals = true;
    assert_eq!(dates(&p)[0], "Nov02 08:00 Nov04 17:00 0");
}

#[test]
fn finish_link_counts_back_on_the_calendar_of_the_last_to_finish() {
    // T1 Mon 2 – Fri 6. T2 (2d, FF on T1) for Ahmed and Bina; Ahmed is off Wed 4 – Thu 5, so he
    // works Tue 3 and Fri 6 and the task still finishes with T1 (not on Mon 9).
    let mut p = build(&["5d", "2d|1FF|res=Ahmed, Bina"], |_| {});
    let r = p.resources.iter_mut().find(|r| r.name == "Ahmed").unwrap();
    r.exceptions.push(Exception { name: "Leave".into(), from: d(2026, 11, 4), to: d(2026, 11, 5), hours: vec![] });
    p.opts.res_cals = true;
    assert_eq!(dates(&p)[1], "Nov03 08:00 Nov06 17:00 0");
}

#[test]
fn started_task_waits_for_unfinished_ext_pred_with_retained_logic() {
    // B started Mon 2, 50 % of 4d; status Tue 3. Its other-plan FS predecessor finishes Fri 6 17:00.
    // Retained logic: the remaining 2d wait for it → Mon 9 – Tue 10 (as an in-plan predecessor would).
    let mut p = build(&["4d||astart=02-Nov-26; pct=50"], |p| p.status = Some(d(2026, 11, 3)));
    let fri = crate::cal::at(d(2026, 11, 6)) + 17 * 60;
    p.tasks[0].ext.push(ext(LinkType::FS, 0, fri - 5 * 480, fri));
    // In-plan equivalent for comparison.
    let mut q = build(&["5d||", "4d|1|astart=02-Nov-26; pct=50"], |p| p.status = Some(d(2026, 11, 3)));
    q.tasks[0].dur = 5 * 480;
    let inplan = dates(&q)[1].clone();
    let _ = inplan;
    assert_eq!(dates(&p)[0], "Nov02 08:00 Nov10 17:00 0");
}

#[test]
fn ext_link_before_1970() {
    let mut p = Project::new("Old", d(1965, 3, 1));
    let t = p.new_task("A", 0, 480);
    p.tasks.push(t);
    let f = crate::cal::at(d(1965, 3, 12)) + 17 * 60;
    p.tasks[0].ext.push(ext(LinkType::FS, 0, f - 480, f));
    let s = schedule(&p).unwrap();
    assert!(s.es[0] > f, "the other plan's finish holds the task: {}", at(s.es[0]));
}

#[test]
fn perf_level_one_resource() {
    // 120 one-day tasks on one person, no links: levelling must put them one after another.
    let mut p = Project::new("Lv", d(2026, 11, 2));
    for i in 0..120 { let t = p.new_task(&format!("T{i}"), 0, 480); p.tasks.push(t); }
    for i in 0..120 { set(&mut p, i, "res", "Ahmed", "%d-%b-%y").unwrap(); }
    let t0 = std::time::Instant::now();
    let moves = crate::level::level(&mut p, &[]).unwrap();
    let el = t0.elapsed();
    let s = schedule(&p).unwrap();
    let mut days: Vec<i64> = (0..120).map(|i| s.es[i].div_euclid(1440)).collect();
    days.sort_unstable();
    let before = days.len();
    days.dedup();
    eprintln!("level moves {moves} in {el:?}; distinct days {} of {before}", days.len());
    assert_eq!(days.len(), 120, "every task on its own day");
    assert!(el.as_secs_f64() < 2.0, "{el:?}");
}

#[test]
fn a_long_plan_schedules_in_linear_time() {
    let mut p = Project::new("Big", d(2026, 11, 2));
    for i in 0..20_000u32 {
        let mut t = p.new_task(&format!("T{i}"), 0, 480);
        if i > 0 { t.preds.push(Link { pred: t.uid - 1, kind: LinkType::FS, lag: 0, pct: false }) }
        p.tasks.push(t);
    }
    let t0 = std::time::Instant::now();
    schedule(&p).unwrap();
    assert!(t0.elapsed().as_secs_f64() < 3.0, "{:?}", t0.elapsed());
}

fn one(name: &str, days: i64) -> Project {
    let mut p = Project::new(name, d(2026, 11, 2));
    let t = p.new_task("A", 0, days * 480);
    p.tasks.push(t);
    p
}

#[test]
fn update_all_reports_links_it_cannot_follow() {
    // Main waits for task 99 of Other, which was deleted. update_all says the dates settled in one
    // pass and the link keeps its stale finish: nothing tells the caller the link is broken.
    let other = one("Other", 5);
    let mut main = one("Main", 1);
    main.tasks[0].ext.push(ExtLink { file: "Other.dplan".into(), uid: 99, kind: LinkType::FS, lag: 0, plan: "Other".into(), code: String::new(), name: "Gone".into(), start: 1, finish: 2 });
    let mut plans = vec![("Main.dplan".to_string(), main), ("Other.dplan".to_string(), other)];
    let r = crate::links::update_all(&mut plans);
    assert!(r.as_ref().map_or(true, |x| !x.1.is_empty()), "update_all hides the broken link: {r:?}, finish still {}", plans[0].1.tasks[0].ext[0].finish);
}

#[test]
fn update_all_tells_same_named_files_apart() {
    // Two plans called Plan.dplan in different folders; Main links to the one in B (10d long).
    let (a, b) = (one("Plan in A", 2), one("Plan in B", 10));
    let mut main = one("Main", 1);
    let uid = b.tasks[0].uid;
    main.tasks[0].ext.push(ExtLink { file: "C:/B/Plan.dplan".into(), uid, kind: LinkType::FS, lag: 0, plan: String::new(), code: String::new(), name: String::new(), start: 0, finish: 0 });
    let mut plans = vec![("C:/A/Plan.dplan".to_string(), a), ("C:/B/Plan.dplan".to_string(), b), ("C:/Main.dplan".to_string(), main)];
    crate::links::update_all(&mut plans).unwrap();
    assert_eq!(plans[2].1.tasks[0].ext[0].plan, "Plan in B");
}
