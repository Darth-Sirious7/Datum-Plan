//! Scheduling cases worked out by hand from the critical-path rules, one per rule.
//! Calendar: Mon–Fri 08:00–12:00 and 13:00–17:00 (8 h days); the project starts Mon 2 Nov 2026.
//! Each row is "duration | predecessors | field=value; …" (the fields and text the window uses);
//! each result is "early start, early finish, total float in days".
use crate::{cal::date_of, doc::{set, Doc, Op}, sched::schedule, *};
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

fn check(name: &str, rows: &[&str], tweak: impl FnOnce(&mut Project), want: &[&str]) {
    let got = dates(&build(rows, tweak));
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert_eq!(g, w, "{name}: task {}", i + 1);
    }
    assert_eq!(got.len(), want.len(), "{name}");
}
fn plain(_: &mut Project) {}

#[test]
fn a_textbook_network() {
    // A 5d; B 3d and C 4d after A; D 2d after B and C; E 6d after A; F 1d after D and E.
    check("network", &["5d||", "3d|1|", "4d|1|", "2d|2,3|", "6d|1|", "1d|4,5|"], plain, &[
        "Nov02 08:00 Nov06 17:00 0", "Nov09 08:00 Nov11 17:00 1", "Nov09 08:00 Nov12 17:00 0",
        "Nov13 08:00 Nov16 17:00 0", "Nov09 08:00 Nov16 17:00 0", "Nov17 08:00 Nov17 17:00 0"]);
    // Free float: B can slip a day before it delays D.
    let p = build(&["5d||", "3d|1|", "4d|1|", "2d|2,3|"], plain);
    let s = schedule(&p).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((s.ff[1], s.ff[2]), (480, 0));
}

#[test]
fn link_types_and_lags() {
    let two = |pred: &str, b: &str| ["5d||".to_string(), format!("{b}|{pred}|")];
    let run = |name: &str, pred: &str, b: &str, want: &str| {
        let r = two(pred, b);
        check(name, &[r[0].as_str(), r[1].as_str()], plain, &["Nov02 08:00 Nov06 17:00 0", want]);
    };
    run("FS", "1", "3d", "Nov09 08:00 Nov11 17:00 0");
    run("FS+2d", "1FS+2d", "3d", "Nov11 08:00 Nov13 17:00 0");
    run("FS lead", "1FS-1d", "3d", "Nov06 08:00 Nov10 17:00 0");
    run("FS+4h", "1FS+4h", "1d", "Nov09 13:00 Nov10 12:00 0");
    run("FS+50%", "1FS+50%", "1d", "Nov11 13:00 Nov12 12:00 0");
    run("SS+2d", "1SS+2d", "3d", "Nov04 08:00 Nov06 17:00 0");
    run("FF+1d", "1FF+1d", "3d", "Nov05 08:00 Nov09 17:00 0");
    run("SF+3d", "1SF+3d", "2d", "Nov03 08:00 Nov04 17:00 2");
}

#[test]
fn durations_and_units() {
    check("units", &["2w||", "12h||", "1.5d||", "90m||", "0||"], plain, &[
        "Nov02 08:00 Nov13 17:00 0", "Nov02 08:00 Nov03 12:00 8.50", "Nov02 08:00 Nov03 12:00 8.50", "Nov02 08:00 Nov02 09:30 9.81", "Nov02 08:00 Nov02 08:00 10"]);
    // A milestone sits at the end of its predecessor; the next task starts the next morning.
    check("milestone", &["5d||", "0|1|", "1d|2|"], plain, &["Nov02 08:00 Nov06 17:00 0", "Nov06 17:00 Nov06 17:00 0", "Nov09 08:00 Nov09 17:00 0"]);
}

#[test]
fn constraints_and_deadlines() {
    let ab = |extra: &str| ["5d||".to_string(), format!("3d|1|{extra}")];
    let run = |name: &str, extra: &str, want: [&str; 2]| {
        let r = ab(extra);
        check(name, &[r[0].as_str(), r[1].as_str()], plain, &want);
    };
    run("SNET", "ctype=Start No Earlier Than; cdate=16-Nov-26", ["Nov02 08:00 Nov06 17:00 5", "Nov16 08:00 Nov18 17:00 0"]);
    run("FNET", "ctype=Finish No Earlier Than; cdate=20-Nov-26", ["Nov02 08:00 Nov06 17:00 7", "Nov18 08:00 Nov20 17:00 0"]);
    run("MSO early", "ctype=Must Start On; cdate=04-Nov-26", ["Nov02 08:00 Nov06 17:00 -3", "Nov04 08:00 Nov06 17:00 0"]);
    run("MFO", "ctype=Must Finish On; cdate=13-Nov-26", ["Nov02 08:00 Nov06 17:00 2", "Nov11 08:00 Nov13 17:00 0"]);
    run("FNLT", "ctype=Finish No Later Than; cdate=10-Nov-26", ["Nov02 08:00 Nov06 17:00 -1", "Nov09 08:00 Nov11 17:00 -1"]);
    run("SNLT", "ctype=Start No Later Than; cdate=06-Nov-26", ["Nov02 08:00 Nov06 17:00 -1", "Nov09 08:00 Nov11 17:00 -1"]);
    check("deadline", &["5d||deadline=05-Nov-26", "3d|1|"], plain, &["Nov02 08:00 Nov06 17:00 -1", "Nov09 08:00 Nov11 17:00 0"]);
    // As late as possible: B has no successors, so it ends with the project.
    check("ALAP", &["5d||", "2d||ctype=As Late As Possible", "1d|1|"], plain, &[
        "Nov02 08:00 Nov06 17:00 0", "Nov06 08:00 Nov09 17:00 0", "Nov09 08:00 Nov09 17:00 0"]);
    // With a successor, it uses only its free float: it moves up to C's start and C is not delayed.
    check("ALAP with successor", &["5d||", "2d||ctype=As Late As Possible", "1d|1,2|"], plain, &[
        "Nov02 08:00 Nov06 17:00 0", "Nov05 08:00 Nov06 17:00 0", "Nov09 08:00 Nov09 17:00 0"]);
}

#[test]
fn calendars() {
    // A public holiday on Wed 4 Nov pushes the work a day.
    check("holiday", &["5d||", "1d|1|"], |p| p.calendars[0].exceptions.push(Exception { name: "Holiday".into(), from: d(2026, 11, 4), to: d(2026, 11, 4), hours: vec![] }), &[
        "Nov02 08:00 Nov09 17:00 0", "Nov10 08:00 Nov10 17:00 0"]);
    // A task on a Mon–Thu 10-hour calendar: 2 days of work (960 min) take 1.6 of its days.
    let four = |p: &mut Project| {
        let ten = vec![(420, 720), (750, 1050)];
        p.calendars.push(Calendar { name: "Four tens".into(), week: [ten.clone(), ten.clone(), ten.clone(), ten, vec![], vec![], vec![]], exceptions: vec![] });
    };
    check("task calendar", &["5d||", "2d|1|cal=Four tens"], four, &["Nov02 08:00 Nov06 17:00 0", "Nov09 07:00 Nov10 13:30 0"]);
    // Elapsed durations run round the clock, weekends included.
    check("elapsed", &["5d||", "2ed|1|"], plain, &["Nov02 08:00 Nov06 17:00 0", "Nov06 17:00 Nov08 17:00 0"]);
    // Lags on a 24-hour calendar: 2d of lag is 16 clock hours, so B starts Monday.
    check("lag on 24 h", &["5d||", "1d|1FS+2d|"], |p| p.opts.lag_cal = LagCal::H24, &["Nov02 08:00 Nov06 17:00 0", "Nov09 08:00 Nov09 17:00 0"]);
}

#[test]
fn summaries_inactive_and_splits() {
    // Summary 1 holds tasks 2 and 3; task 4 follows the summary.
    let mut p = build(&["0||", "5d||", "3d|2|", "2d|1|"], plain);
    for t in &mut p.tasks[1..3] { t.level = 1 }
    assert_eq!(dates(&p), ["Nov02 08:00 Nov11 17:00 0", "Nov02 08:00 Nov06 17:00 0", "Nov09 08:00 Nov11 17:00 0", "Nov12 08:00 Nov13 17:00 0"]);
    // An inactive task is still scheduled from its predecessors (as in MS Project) but drives nothing:
    // its successor starts as if it were not there, and it has no float of its own.
    check("inactive", &["5d||", "10d|1|active=No", "2d|2|"], plain, &["Nov02 08:00 Nov06 17:00 0", "Nov09 08:00 Nov20 17:00 0", "Nov02 08:00 Nov03 17:00 3"]);
    // A split: one day of work, a two-day pause, then the remaining four days.
    let mut doc = Doc::new(build(&["5d||", "1d|1|"], plain));
    let uid = doc.p.tasks[0].uid;
    doc.apply(Op::Split { uid, after: 480, gap: 960 }, "%d-%b-%y").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(dates(&doc.p), ["Nov02 08:00 Nov10 17:00 0", "Nov11 08:00 Nov11 17:00 0"]);
}

#[test]
fn progress_retained_logic_and_override() {
    let status = |p: &mut Project| p.status = Some(d(2026, 11, 4));
    // A is 40 % done at the status date (Wed 4 Nov): its 3 remaining days start Thursday.
    check("remaining", &["5d||astart=02-Nov-26; pct=40", "3d|1|", "2d||"], status, &[
        "Nov02 08:00 Nov09 17:00 0", "Nov10 08:00 Nov12 17:00 0", "Nov05 08:00 Nov06 17:00 4"]);
    // B started early, out of sequence. Retained logic keeps its rest after A; override lets it go on.
    let rows = ["5d||astart=02-Nov-26; pct=40", "4d|1|astart=03-Nov-26; pct=50"];
    check("retained", &rows, status, &["Nov02 08:00 Nov09 17:00 0", "Nov03 08:00 Nov11 17:00 0"]);
    check("override", &rows, |p| { status(p); p.opts.retained = false }, &["Nov02 08:00 Nov09 17:00 0", "Nov03 08:00 Nov06 17:00 1"]);
    // A finished task keeps its actual dates and its successor follows them.
    check("actuals", &["5d||astart=02-Nov-26; afinish=04-Nov-26", "3d|1|"], plain, &["Nov02 08:00 Nov04 17:00 0", "Nov05 08:00 Nov09 17:00 0"]);
}

#[test]
fn scheduling_from_the_finish() {
    check("from finish", &["5d||", "3d|1|"], |p| { p.opts.from_finish = true; p.opts.finish_date = Some(d(2026, 11, 20)) }, &[
        "Nov11 08:00 Nov17 17:00 0", "Nov18 08:00 Nov20 17:00 0"]);
}

#[test]
fn levelling_moves_the_second_task() {
    // Float stays a logic measure: after levelling, A could slip 5 days before it delays the finish.
    let mut doc = Doc::new(build(&["5d||res=Alice", "5d||res=Alice"], plain));
    doc.apply(Op::Level { uids: vec![], clear: false }, "%d-%b-%y").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(dates(&doc.p), ["Nov02 08:00 Nov06 17:00 5", "Nov09 08:00 Nov13 17:00 0"]);
}

#[test]
fn more_links_and_leads() {
    // A lead that would start B before the project is held at the project start.
    check("SS lead", &["5d||", "3d|1SS-1d|"], plain, &["Nov02 08:00 Nov06 17:00 0", "Nov02 08:00 Nov04 17:00 2"]);
    // Two kinds of link at once: the later one wins.
    check("mixed", &["3d||", "5d||", "2d|1,2SS+1d|"], plain, &["Nov02 08:00 Nov04 17:00 0", "Nov02 08:00 Nov06 17:00 0", "Nov05 08:00 Nov06 17:00 0"]);
    check("FF+4h", &["5d||", "1d|1FF+4h|"], plain, &["Nov02 08:00 Nov06 17:00 0", "Nov06 13:00 Nov09 12:00 0"]);
    check("SF", &["5d||", "2d|1SF|"], plain, &["Nov02 08:00 Nov06 17:00 0", "Nov02 08:00 Nov03 17:00 3"]);
    // A chain: each link type in turn.
    check("chain", &["2d||", "2d|1SS+1d|", "2d|2FF+1d|", "1d|3FS|"], plain, &[
        "Nov02 08:00 Nov03 17:00 0", "Nov03 08:00 Nov04 17:00 0", "Nov04 08:00 Nov05 17:00 0", "Nov06 08:00 Nov06 17:00 0"]);
}

#[test]
fn more_constraints_and_calendars() {
    let weekend = |p: &mut Project| p.start = d(2026, 10, 31);
    check("weekend start", &["1d||"], weekend, &["Nov02 08:00 Nov02 17:00 0"]);
    check("MSO late", &["5d||", "3d|1|ctype=Must Start On; cdate=16-Nov-26"], plain, &["Nov02 08:00 Nov06 17:00 5", "Nov16 08:00 Nov18 17:00 0"]);
    check("deadline met", &["5d||deadline=20-Nov-26"], plain, &["Nov02 08:00 Nov06 17:00 0"]);
    let half_day = |p: &mut Project| p.calendars[0].exceptions.push(Exception { name: "Half day".into(), from: d(2026, 11, 2), to: d(2026, 11, 2), hours: vec![(480, 720)] });
    check("half day", &["1d||"], half_day, &["Nov02 08:00 Nov03 12:00 0"]);
    let week_off = |p: &mut Project| p.calendars[0].exceptions.push(Exception { name: "Shutdown".into(), from: d(2026, 11, 9), to: d(2026, 11, 13), hours: vec![] });
    check("shutdown", &["5d||", "3d|1|"], week_off, &["Nov02 08:00 Nov06 17:00 0", "Nov16 08:00 Nov18 17:00 0"]);
    // Work on a not-started task waits for the status date.
    check("after status", &["5d||astart=02-Nov-26; pct=40", "1d|1|", "2d||"], |p| p.status = Some(d(2026, 11, 4)), &[
        "Nov02 08:00 Nov09 17:00 0", "Nov10 08:00 Nov10 17:00 0", "Nov05 08:00 Nov06 17:00 2"]);
}

#[test]
fn critical_tasks_and_man_hours() {
    let p = build(&["5d||", "3d|1|", "4d|1|", "2d|2,3|", "6d|1|", "1d|4,5|"], plain);
    let s = schedule(&p).unwrap_or_else(|e| panic!("{e}"));
    let crit: Vec<bool> = (0..p.tasks.len()).map(|i| s.critical(&p, i)).collect();
    assert_eq!(crit, [true, false, true, true, true, true]);
    // Two people full time on a 5-day task: 80 man-hours.
    let p = build(&["5d||res=Alice, Bob"], plain);
    let s = schedule(&p).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(crate::load::compute(&p, &s).work[0], 2.0 * 5.0 * 480.0);
}

/// Resource calendars: a person's leave and base calendar drive their tasks when the option is on.
#[test]
fn resource_calendars_drive_task_dates() {
    let leave = |p: &mut Project, who: &str, from: (u32, u32), to: (u32, u32)| {
        let r = p.resources.iter_mut().find(|r| r.name == who).unwrap_or_else(|| panic!("{who}"));
        r.exceptions.push(Exception { name: "Leave".into(), from: d(2026, from.0, from.1), to: d(2026, to.0, to.1), hours: vec![] });
    };
    // Ahmed is on leave Tue 3 – Wed 4 Nov. T1 5d is his; T2 1d follows it.
    let rows = ["5d||res=Ahmed", "1d|1|"];
    let mut p = build(&rows, plain);
    leave(&mut p, "Ahmed", (11, 3), (11, 4));
    let off = dates(&p);
    assert_eq!(off[0], "Nov02 08:00 Nov06 17:00 0", "option off: nothing changes");
    p.opts.res_cals = true;
    let on = dates(&p);
    // Works Mon 2, Thu 5, Fri 6, Mon 9, Tue 10; the successor follows on Wed 11.
    assert_eq!(on, ["Nov02 08:00 Nov10 17:00 0", "Nov11 08:00 Nov11 17:00 0"]);
    // Ignore Resource Calendar: the task keeps the project calendar.
    p.tasks[0].ignore_rc = true;
    assert_eq!(dates(&p)[0], "Nov02 08:00 Nov06 17:00 0");
    p.tasks[0].ignore_rc = false;

    // Two people on one task: the one who finishes last sets the finish (Bina has no leave).
    let mut p = build(&["5d||res=Ahmed, Bina"], plain);
    leave(&mut p, "Ahmed", (11, 3), (11, 4));
    p.opts.res_cals = true;
    assert_eq!(dates(&p)[0], "Nov02 08:00 Nov10 17:00 0");
    // …even when Bina's leave is the later one.
    leave(&mut p, "Bina", (11, 4), (11, 9));
    assert_eq!(dates(&p)[0], "Nov02 08:00 Nov12 17:00 0", "Bina: Mon 2, Tue 3, then Tue 10 – Thu 12");

    // A person on a six-day calendar works Saturdays.
    let mut p = build(&["6d||res=Chen"], |p| {
        let mut six = Calendar::standard("Six-day");
        six.week[5] = crate::model::STD_HOURS.to_vec();
        p.calendars.push(six);
    });
    p.resources.iter_mut().for_each(|r| r.cal = Some("Six-day".into()));
    assert_eq!(dates(&p)[0], "Nov02 08:00 Nov09 17:00 0", "option off: the project calendar");
    p.opts.res_cals = true;
    assert_eq!(dates(&p)[0], "Nov02 08:00 Nov07 17:00 0", "Mon–Sat");

    // A task calendar and a person's calendar: work only when both do (mornings only here).
    let mut p = build(&["5d||res=Dana; cal=Mornings"], |p| {
        let mut m = Calendar::standard("Mornings");
        for k in 0..5 { m.week[k] = vec![(480, 720)] }
        p.calendars.push(m);
    });
    p.opts.res_cals = true;
    leave(&mut p, "Dana", (11, 6), (11, 6));
    // 40 h of mornings is 10 mornings, and Dana is off Fri 6: Mon 2 … Fri 13, then Mon 16.
    assert_eq!(dates(&p)[0], "Nov02 08:00 Nov16 12:00 0");

    // Capacity follows the person: the week of Ahmed's leave holds 24 h, not 40.
    let mut p = build(&["5d||res=Ahmed"], plain);
    leave(&mut p, "Ahmed", (11, 3), (11, 4));
    p.opts.res_cals = true;
    let s = schedule(&p).unwrap_or_else(|e| panic!("{e}"));
    let w = crate::load::workload(&p, &s, true);
    assert_eq!(w.rows[0].cap[0], 24.0);
}

/// A plan with resource calendars keeps its dates through project XML and XER.
#[test]
fn resource_calendars_survive_exchange_files() {
    let mut p = build(&["5d||res=Ahmed", "1d|1|", "3d||res=Bina; ignore_rc=Yes"], plain);
    let r = p.resources.iter_mut().find(|r| r.name == "Ahmed").unwrap_or_else(|| panic!());
    r.exceptions.push(Exception { name: "Leave".into(), from: d(2026, 11, 3), to: d(2026, 11, 4), hours: vec![] });
    p.opts.res_cals = true;
    let want = dates(&p);
    assert_eq!(want[0], "Nov02 08:00 Nov10 17:00 0");
    for ext in ["xml", "xer"] {
        let f = std::env::temp_dir().join(format!("dp-rc-{}.{ext}", std::process::id()));
        crate::io::export(&p, &f, "%d-%b-%y", d(2026, 11, 1), &[]).unwrap_or_else(|e| panic!("{ext}: {e}"));
        let got = crate::io::import(&f);
        let _ = std::fs::remove_file(&f);
        let Ok(crate::io::Imported::Plan(q, _)) = got else { panic!("{ext} did not import") };
        // Rows may come back in another order (XER groups activities by WBS): compare by name.
        let (dq, dp) = (dates(&q), dates(&p));
        for (k, t) in p.tasks.iter().enumerate() {
            let j = q.tasks.iter().position(|x| x.name == t.name).unwrap_or_else(|| panic!("{ext}: {} is missing", t.name));
            assert_eq!(dq[j], dp[k], "{ext}: {}", t.name);
        }
    }
}

// ---------- found by the engine audit (0.5.11) ----------

/// A one-row plan with custom fields defined by formulas; the text of each field on that row.
fn formula_texts(defs: &[(&str, &str)]) -> Vec<String> {
    let mut doc = crate::doc::Doc::new(build(&["1d"], |_| {}));
    for (k, f) in defs {
        doc.apply(crate::doc::Op::FieldDef { def: crate::model::CustomDef { key: k.to_string(), formula: f.to_string(), ..Default::default() } }, "%d-%b-%y").unwrap();
    }
    let c = crate::fields::Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 2));
    defs.iter().map(|(k, _)| c.text(0, &format!("c:{k}"))).collect()
}

#[test]
fn formulas_count_months_and_years_like_vba() {
    assert_eq!(formula_texts(&[("date1", "DateAdd(\"m\", 1, #2027-01-01#)"), ("number1", "DateDiff(\"m\", #2026-11-30#, #2026-12-01#)"), ("number2", "DateDiff(\"yyyy\", #2026-12-31#, #2027-01-01#)"), ("date2", "DateAdd(\"m\", 1, #2027-01-31#)"), ("number3", "DateDiff(\"d\", #2026-11-30#, #2026-12-01#)")]),
        ["01-Feb-27", "1", "1", "28-Feb-27", "1"]);
}

#[test]
fn a_finish_field_from_a_formula_shows_its_own_day() {
    assert_eq!(formula_texts(&[("finish1", "=DATE(2026,11,10)"), ("finish2", "#2026-11-10#"), ("date1", "=DATE(2026,11,10)")]), ["10-Nov-26", "10-Nov-26", "10-Nov-26"]);
}

#[test]
fn a_plan_of_milestones_only_reports_its_progress() {
    let mut p = build(&["0d", "0d", "0d"], |_| {});
    p.tasks[1].level = 1; p.tasks[2].level = 1;
    for k in 1..3 { p.tasks[k].pct = 100; p.tasks[k].act_start = Some(d(2026, 11, 2)); p.tasks[k].act_finish = Some(d(2026, 11, 2)); }
    let c = crate::fields::Calc::new(&p, "%d-%b-%y", d(2026, 11, 9));
    assert_eq!(c.summary().pct, 100.0);
}

/// A 10-day task with a 1,000 budget, baselined, at a % complete on Friday of its first week.
fn progressed(pct: u8, physical: Option<u8>) -> crate::fields::Calc<'static> {
    let mut doc = Doc::new(build(&["10d"], |_| {}));
    let u = doc.p.tasks[0].uid;
    doc.apply(Op::Set { uid: u, field: "fixedcost".into(), value: "1000".into() }, "%d-%b-%y").unwrap();
    doc.apply(Op::Baseline { uids: vec![], clear: false, n: 0 }, "%d-%b-%y").unwrap();
    doc.apply(Op::Set { uid: u, field: "pct".into(), value: pct.to_string() }, "%d-%b-%y").unwrap();
    doc.apply(Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-11-06") }, "%d-%b-%y").unwrap();
    if let Some(ph) = physical { doc.p.tasks[0].pct_type = PctType::Physical; doc.p.tasks[0].phys = ph; }
    let p: &'static Project = Box::leak(Box::new(doc.p));
    crate::fields::Calc::new(p, "%d-%b-%y", d(2026, 11, 6))
}

#[test]
fn the_earned_line_of_the_cost_curve_adds_up_to_earned_value() {
    for (pct, phys, ev) in [(50, None, 500.0), (50, Some(80), 800.0)] {
        let c = progressed(pct, phys);
        let total: f64 = crate::report::curve(&c, "day").earned.iter().sum();
        assert!((total - ev).abs() < 0.01 && (c.val(0, "bcwp").num() - ev).abs() < 0.01, "{pct}% {phys:?}: curve {total}, EV {}", c.val(0, "bcwp").num());
    }
}

#[test]
fn earned_schedule_counts_working_time_within_a_day() {
    for pct in [30u8, 25, 22] {
        let e = crate::progress::earned_schedule(&progressed(pct, None)).unwrap();
        assert!((e.es - pct as f64 / 10.0).abs() < 0.01, "{pct}%: ES {}", e.es);
    }
}

#[test]
fn a_level_of_effort_ends_the_day_before_its_successor_starts() {
    // A 2d; LOE from A's start; C after A and the LOE: the LOE spans Mon–Tue (2 days).
    let mut p = build(&["2d", "1d|1SS", "3d|1,2"], |_| {});
    p.tasks[1].act_type = ActType::Loe;
    let s = schedule(&p).unwrap();
    assert_eq!((at(s.es[1]), at(s.ef[1])), ("Nov02 08:00".into(), "Nov03 17:00".into()));
    let c = crate::fields::Calc::new(&p, "%d-%b-%y", d(2026, 11, 2));
    assert_eq!(c.text(1, "dur"), "2 days");
}

#[test]
fn a_start_link_from_a_summary_follows_the_summary_start() {
    // Summary {A 2d, B 3d after A}; C starts with the summary (SS), on Monday.
    let mut p = build(&["0d", "2d", "3d|2", "1d"], |_| {});
    p.tasks[1].level = 1; p.tasks[2].level = 1;
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    p.tasks[3].preds = crate::parse::preds("1SS", &ids, 480).unwrap();
    let s = schedule(&p).unwrap();
    assert_eq!(at(s.es[3]), "Nov02 08:00");
}

#[test]
fn actual_dates_on_a_day_off_stand_as_recorded() {
    let p = build(&["1d||astart=2026-11-07;afinish=2026-11-07;pct=100"], |p| p.status = Some(d(2026, 11, 9)));
    let s = schedule(&p).unwrap();
    assert_eq!((date_of(s.es[0]), date_of(s.ef[0] - 1)), (d(2026, 11, 7), d(2026, 11, 7)));
}

#[test]
fn a_link_to_another_plan_on_a_summary_holds_the_tasks_under_it() {
    let mut p = build(&["0d", "2d"], |_| {});
    p.tasks[1].level = 1;
    let fin = crate::cal::at(d(2026, 11, 13)) + 17 * 60;
    p.tasks[0].ext.push(ExtLink { file: "X.dplan".into(), uid: 5, kind: LinkType::FS, lag: 0, elapsed: false, plan: "X".into(), code: String::new(), name: "x".into(), start: fin - 5 * crate::cal::DAY, finish: fin });
    let s = schedule(&p).unwrap();
    assert_eq!(at(s.es[1]), "Nov16 08:00");
}

#[test]
fn a_row_that_mirrors_a_task_in_another_plan_takes_its_length() {
    // SS and FF to the same task of another plan: the row runs Mon 2 – Fri 13 Nov (10 days), whatever its own duration says.
    let mut p = build(&["3d"], |_| {});
    let (a, b) = (crate::cal::at(d(2026, 11, 2)) + 8 * 60, crate::cal::at(d(2026, 11, 13)) + 17 * 60);
    for kind in [LinkType::SS, LinkType::FF] {
        p.tasks[0].ext.push(ExtLink { file: "Eng.dplan".into(), uid: 7, kind, lag: 0, elapsed: false, plan: "Eng".into(), code: String::new(), name: "Engineering".into(), start: a, finish: b });
    }
    let s = schedule(&p).unwrap();
    assert_eq!((at(s.es[0]), at(s.ef[0])), ("Nov02 08:00".into(), "Nov13 17:00".into()));
    assert_eq!(crate::fields::Calc::new(&p, "%d-%b-%y", d(2026, 11, 2)).text(0, "dur"), "10 days");
    // The other task grows by a week: so does the row, from the same start.
    for x in &mut p.tasks[0].ext { x.finish = b + 7 * crate::cal::DAY }
    let s = schedule(&p).unwrap();
    assert_eq!((at(s.es[0]), at(s.ef[0])), ("Nov02 08:00".into(), "Nov20 17:00".into()));
}
