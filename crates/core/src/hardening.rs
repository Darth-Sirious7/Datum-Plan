//! Hardening tests: a failed edit changes nothing, random plans and damaged files never panic,
//! and rescheduling stays fast.
use crate::{doc::{Doc, Op}, io::{mspdi, xer}, load, sched::schedule, templates, *};
use chrono::NaiveDate;
use std::{panic, time::Instant};

/// A small deterministic random generator (no extra crate).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
    fn below(&mut self, n: u64) -> u64 { self.next() % n.max(1) }
}

fn start() -> NaiveDate { NaiveDate::from_ymd_opt(2026, 11, 2).unwrap_or_default() }
fn sample() -> Project { templates::build(&templates::TEMPLATES[0], start()) }

/// A plan of `n` leaf tasks with random durations, outline levels and links (loops allowed).
fn random_plan(r: &mut Rng, n: usize, loops: bool) -> Project {
    let mut p = Project::new("R", start());
    for i in 0..n {
        let t = p.new_task(&format!("T{i}"), 0, r.below(10) as i64 * 480);
        p.tasks.push(t);
    }
    let uids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    for i in 1..n {
        for _ in 0..r.below(3) {
            let j = if loops { r.below(n as u64) as usize } else { r.below(i as u64) as usize };
            let kind = ["FS", "SS", "FF", "SF"][r.below(4) as usize];
            let lag = r.below(5) as i64 - 2;
            if let Ok(mut l) = parse::preds(&format!("{}{kind}{lag:+}d", j + 1), &uids, 480) { p.tasks[i].preds.append(&mut l) }
        }
    }
    p
}

#[test]
fn a_failed_edit_changes_nothing() {
    let mut d = Doc::new(sample());
    let before = d.json();
    let (a, b) = (d.p.tasks[1].uid, d.p.tasks[2].uid);
    let bad = vec![
        Op::Delete { uids: vec![99_999] },
        Op::Set { uid: 99_999, field: "name".into(), value: "x".into() },
        Op::Set { uid: a, field: "duration".into(), value: "not a duration".into() },
        Op::Set { uid: a, field: "start".into(), value: "31-Feb-99999".into() },
        Op::Set { uid: a, field: "predecessors".into(), value: "99999".into() },
        Op::LinkSet { pred: a, succ: b, kind: Some(LinkType::FS), lag: "zz".into() },
        Op::Split { uid: 99_999, after: 1, gap: 1 },
        Op::CalDelete { name: "No such calendar".into() },
        Op::ResSet { uid: 99_999, field: "name".into(), value: "x".into() },
        Op::Sheet { k: Some(999), sheet: None },
        Op::SheetCells { k: 999, cells: vec![("A1".into(), "1".into())], fmt: None },
        Op::Style { cells: vec![(99_999, "name".into(), "b".into())] },
        Op::UpdateProject { through: "not a date".into(), reschedule: true },
        Op::Recurring { at: 0, name: "R".into(), dur: "1d".into(), every: "fortnight".into(), interval: 0, count: 0, start: "?".into() },
    ];
    for o in bad {
        let name = format!("{o:?}");
        let r = panic::catch_unwind(panic::AssertUnwindSafe(|| d.apply(o, "%d-%b-%y")));
        assert!(r.is_ok(), "panicked: {name}");
        if r.is_ok_and(|x| x.is_err()) { assert_eq!(d.json(), before, "a failed {name} changed the plan") }
        d = Doc::new(serde_json::from_str(&before).unwrap_or_default());
    }
    // A link that closes a loop is refused and leaves the plan as it was.
    let mut d = Doc::new(sample());
    let last = d.p.tasks.iter().rev().find(|t| !t.preds.is_empty()).map(|t| t.uid).unwrap_or(a);
    let first = d.p.tasks.iter().find(|t| t.preds.is_empty() && t.uid != last).map(|t| t.uid).unwrap_or(b);
    let json = d.json();
    let r = d.apply(Op::LinkSet { pred: last, succ: first, kind: Some(LinkType::FS), lag: String::new() }, "%d-%b-%y");
    if r.is_err() { assert_eq!(d.json(), json) }
}

#[test]
fn random_plans_schedule_without_panics() {
    let mut r = Rng(0x2545_F491_4F6C_DD1D);
    for round in 0..1000 {
        let loops = round % 3 == 0;
        let n = 2 + r.below(40) as usize;
        let p = random_plan(&mut r, n, loops);
        let s = panic::catch_unwind(|| schedule(&p)).unwrap_or_else(|_| panic!("schedule panicked on round {round}"));
        let Ok(s) = s else { assert!(loops, "a plan without loops failed to schedule (round {round})"); continue };
        assert_eq!(Some(&s.es), schedule(&p).ok().as_ref().map(|x| &x.es), "same plan, same dates");
        for i in (0..p.tasks.len()).filter(|&i| !s.summary[i]) {
            assert!(s.es[i] <= s.ef[i] && s.ls[i] <= s.lf[i], "start after finish (round {round}, task {i})");
            assert!(s.tf[i] >= 0, "negative float with no constraints (round {round}, task {i})");
            // Compared in working time: 17:00 one day and 08:00 the next are the same moment.
            let c = &s.cals[s.ci[i]];
            assert!(c.between(s.es[i], s.ls[i]) >= 0 && c.between(s.ef[i], s.lf[i]) >= 0, "late dates before early dates (round {round}, task {i})");
        }
        let _ = load::compute(&p, &s);
        if round % 25 == 0 { exercise(&p) }
    }
}

/// Cuts and garbles a good file many ways; every reader must answer with Ok or an error, never panic.
fn survives(name: &str, good: &str, read: fn(&str) -> Result<Project, String>) {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    let b = good.as_bytes();
    let mut cases: Vec<String> = (0..60).map(|k| String::from_utf8_lossy(&b[..b.len() * k / 60]).into_owned()).collect();
    for _ in 0..3000 {
        let mut v = b.to_vec();
        for _ in 0..1 + r.below(8) {
            let i = r.below(v.len() as u64) as usize;
            v[i] = [b'<', b'>', b'/', b'\t', b'\n', b'"', b'-', b'9', 0, 0xFF][r.below(10) as usize];
        }
        cases.push(String::from_utf8_lossy(&v).into_owned());
    }
    cases.extend(["", "<", "</a>", "<a><b></a>", "%T\tTASK\n%F\ttask_id\n%R\t1", "\u{FEFF}", "<?xml?><Project><Tasks><Task><Duration>PT99999999999H</Duration></Task></Tasks></Project>"].map(String::from));
    for (k, c) in cases.iter().enumerate() {
        let res = panic::catch_unwind(|| read(c).map(|p| { let _ = schedule(&p); }));
        assert!(res.is_ok(), "{name} reader panicked on case {k}: {:?}", &c[..c.len().min(120)]);
    }
}

#[test]
fn damaged_files_never_panic() {
    let p = sample();
    let s = schedule(&p).unwrap_or_else(|e| panic!("{e}"));
    let cost = load::compute(&p, &s).cost;
    survives("XML", &mspdi::write(&p, &s, &cost), mspdi::read);
    survives("XER", &xer::write(&p, &s, start()), xer::read);
    survives("JSON", &serde_json::to_string(&p).unwrap_or_default(), |t| Project::from_json(t).map_err(|e| e.to_string()));
}

#[test]
fn rescheduling_stays_fast() {
    let mut r = Rng(7);
    let p = random_plan(&mut r, 2_000, false);
    let _ = schedule(&p);
    let t = Instant::now();
    for _ in 0..5 { let _ = schedule(&p); }
    let ms = t.elapsed().as_secs_f64() * 1000.0 / 5.0;
    // Debug builds (cargo test) are about 10–20× slower than the app.
    let limit = if cfg!(debug_assertions) { 400.0 } else { 20.0 };
    assert!(ms < limit, "2,000 tasks took {ms:.1} ms to schedule (limit {limit} ms)");
    eprintln!("2,000 tasks schedule in {ms:.2} ms");
}


/// Adds random constraints, task calendars, splits and progress to a random plan.
fn decorate(r: &mut Rng, p: &mut Project) {
    let ten = vec![(420, 720), (750, 1050)];
    p.calendars.push(Calendar { name: "Four tens".into(), week: [ten.clone(), ten.clone(), ten.clone(), ten, vec![], vec![], vec![]], exceptions: vec![] });
    p.calendars[0].exceptions.push(Exception { name: "Holiday".into(), from: start() + chrono::Days::new(9), to: start() + chrono::Days::new(10), hours: vec![] });
    let day = |r: &mut Rng| start() + chrono::Days::new(r.below(40));
    for i in 0..p.tasks.len() {
        let t = &mut p.tasks[i];
        match r.below(12) {
            0 => (t.constraint, t.cdate) = (Constraint::SNET, Some(day(r))),
            1 => (t.constraint, t.cdate) = (Constraint::FNLT, Some(day(r))),
            2 => (t.constraint, t.cdate) = (Constraint::MSO, Some(day(r))),
            3 => t.constraint = Constraint::ALAP,
            4 => t.deadline = Some(day(r)),
            5 => t.cal = Some("Four tens".into()),
            6 => t.splits = vec![(t.dur / 2, 480)],
            7 => t.elapsed = true,
            _ => {}
        }
    }
}

#[test]
fn links_hold_and_the_forward_pass_is_tight() {
    let mut r = Rng(0xD1B5_4A32_D192_ED03);
    for round in 0..400 {
        let n = 2 + r.below(30) as usize;
        let mut p = random_plan(&mut r, n, false);
        let free = round % 2 == 0; // half the plans have constraints, calendars, splits and elapsed tasks
        if !free { decorate(&mut r, &mut p) }
        let Ok(s) = schedule(&p) else { panic!("round {round}: a plan without loops failed") };
        let start = s.cals[0].next_start(crate::cal::at(p.start));
        for i in 0..n {
            let (t, c) = (&p.tasks[i], &s.cals[s.ci[i]]);
            let plain = t.constraint == Constraint::ASAP && t.cdate.is_none();
            let mut tight = s.es[i] == start || !plain;
            for e in s.edges.iter().filter(|e| e.s == i) {
                let lc = &s.cals[s.ci[e.s]];
                let (from, to) = match e.kind {
                    LinkType::FS => (lc.add(s.ef[e.p], e.lag), s.es[i]),
                    LinkType::SS => (lc.add(s.es[e.p], e.lag), s.es[i]),
                    LinkType::FF => (lc.add(s.ef[e.p], e.lag), s.ef[i]),
                    LinkType::SF => (lc.add(s.es[e.p], e.lag), s.ef[i]),
                };
                let gap = c.between(from, to);
                if plain && gap < 0 {
                    let q = &p.tasks[e.p];
                    panic!("round {round}: task {} ({:?} cal {:?} el {} sp {:?} dur {}) starts before its {:?}{:+} link from {} ({:?} cal {:?} el {} sp {:?}) allows ({gap} min): from {} to {} pred es {} ef {}", i + 1, t.constraint, t.cal, t.elapsed, t.splits, t.dur, e.kind, e.lag, e.p + 1, q.constraint, q.cal, q.elapsed, q.splits, from, to, s.es[e.p], s.ef[e.p]);
                }
                tight |= gap == 0 || (to == s.es[i] && to <= start);
            }
            if free { assert!(tight, "round {round}: task {} could start earlier", i + 1) }
        }
    }
}

#[test]
fn row_order_does_not_change_the_dates() {
    let mut r = Rng(0x5851_F42D_4C95_7F2D);
    for round in 0..200 {
        let n = 2 + r.below(30) as usize;
        let mut p = random_plan(&mut r, n, false);
        decorate(&mut r, &mut p);
        let by_uid = |p: &Project| -> Vec<(u32, i64, i64, i64)> {
            let s = schedule(p).unwrap_or_else(|e| panic!("round {round}: {e}"));
            let mut v: Vec<_> = p.tasks.iter().enumerate().map(|(i, t)| (t.uid, s.es[i], s.ef[i], s.tf[i])).collect();
            v.sort();
            v
        };
        let a = by_uid(&p);
        p.tasks.reverse();
        assert_eq!(a, by_uid(&p), "round {round}: the dates depend on the row order");
    }
}

#[test]
fn random_edits_never_break_the_plan() {
    let mut r = Rng(0xA076_1D64_78BD_642F);
    let words = ["5d", "0", "2w", "-3", "abc", "1,2", "3SS+2d", "99", "1FS-200d", "Yes", "No", "100", "50", "", "15-Nov-26", "31-Feb-26",
        "Start No Earlier Than", "Must Finish On", "As Late As Possible", "Alice", "Alice[50%], Bob", "Four tens", "1e9", "∞", "\u{0}", "Fixed Work"];
    let fields = ["name", "dur", "preds", "pct", "ctype", "cdate", "deadline", "res", "cal", "astart", "afinish", "active", "mode", "type", "work", "leveldelay", "start", "finish", "notes", "fixedcost", "qty", "qtydone", "dmin", "dmax"];
    for round in 0..30 {
        let mut d = Doc::new(sample());
        for step in 0..60 {
            let uids: Vec<u32> = d.p.tasks.iter().map(|t| t.uid).collect();
            let uid = |r: &mut Rng| if uids.is_empty() || r.below(20) == 0 { 99_999 } else { uids[r.below(uids.len() as u64) as usize] };
            let some = |r: &mut Rng| (0..1 + r.below(3)).map(|_| uid(r)).collect::<Vec<u32>>();
            let o = match r.below(12) {
                0 => Op::Insert { at: r.below(uids.len() as u64 + 3) as usize, name: "New".into(), ms: r.below(2) == 0 },
                1 => Op::Delete { uids: some(&mut r) },
                2 => Op::Indent { uids: some(&mut r) },
                3 => Op::Outdent { uids: some(&mut r) },
                4 => Op::Move { uids: some(&mut r), up: r.below(2) == 0 },
                5 => Op::LinkSet { pred: uid(&mut r), succ: uid(&mut r), kind: Some([LinkType::FS, LinkType::SS, LinkType::FF, LinkType::SF][r.below(4) as usize]), lag: words[r.below(words.len() as u64) as usize].into() },
                6 => Op::Level { uids: vec![], clear: r.below(2) == 0 },
                7 => Op::Split { uid: uid(&mut r), after: r.below(3000) as i64, gap: r.below(2000) as i64 - 200 },
                _ => Op::Set { uid: uid(&mut r), field: fields[r.below(fields.len() as u64) as usize].into(), value: words[r.below(words.len() as u64) as usize].into() },
            };
            let before = d.json();
            let name = format!("{o:?}");
            let res = panic::catch_unwind(panic::AssertUnwindSafe(|| d.apply(o, "%d-%b-%y")));
            let Ok(res) = res else { panic!("round {round} step {step}: {name} panicked") };
            if res.is_err() { assert_eq!(d.json(), before, "round {round} step {step}: a failed {name} changed the plan") }
            assert!(schedule(&d.p).is_ok(), "round {round} step {step}: {name} left a plan that does not schedule");
            if r.below(8) == 0 { d.undo() }
            if r.below(16) == 0 { d.redo() }
            let _ = crate::fields::Calc::new(&d.p, "%d-%b-%y", start());
        }
    }
}

#[test]
fn damaged_spreadsheets_never_panic() {
    use crate::io::table;
    let dir = std::env::temp_dir().join(format!("datum-hardening-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let p = sample();
    let c = crate::fields::Calc::new(&p, "%d-%b-%y", start());
    let cols: Vec<String> = ["name", "dur", "start", "finish", "preds", "res", "pct"].map(String::from).to_vec();
    let (csv, xlsx) = (dir.join("good.csv"), dir.join("good.xlsx"));
    table::write_csv(&c, &cols, &csv).unwrap_or_else(|e| panic!("{e}"));
    table::write_xlsx(&c, &cols, &xlsx).unwrap_or_else(|e| panic!("{e}"));
    let mut r = Rng(0xBF58_476D_1CE4_E5B9);
    for (k, good) in [csv, xlsx].iter().enumerate() {
        let b = std::fs::read(good).unwrap_or_default();
        for case in 0..300 {
            let mut v = if case < 30 { b[..b.len() * case / 30].to_vec() } else { b.clone() };
            for _ in 0..if case < 30 { 0 } else { 1 + r.below(6) } {
                let i = r.below(v.len().max(1) as u64) as usize;
                if let Some(x) = v.get_mut(i) { *x = [b',', b'"', b'\n', b'\r', b'0', b'-', 0, 0xFF, b'='][r.below(9) as usize] }
            }
            let f = dir.join(format!("bad{k}_{case}.{}", if k == 0 { "csv" } else { "xlsx" }));
            let _ = std::fs::write(&f, &v);
            let res = panic::catch_unwind(|| table::read(&f).map(|t| table::to_project(&t, &cols, "T", start(), "%d-%b-%y").map(|(p, _)| { let _ = schedule(&p); })));
            assert!(res.is_ok(), "{} case {case} panicked", f.display());
            let _ = std::fs::remove_file(&f);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Runs every engine function the window or a script can reach on a plan: none may panic.
fn exercise(p: &Project) {
    use crate::{check, network, progress, query::Query, report, risk};
    let Ok(s) = schedule(p) else { return };
    let _ = load::compute(p, &s);
    let c = crate::fields::Calc::new(p, "%d-%b-%y", start());
    let _ = c.summary();
    let q = Query { cols: ["code", "name", "dur", "start", "finish", "preds", "res", "tf", "cost", "work", "pct"].map(String::from).to_vec(), ..Query::default() };
    let _ = c.grid(&q);
    for unit in ["day", "week", "month"] {
        let _ = report::usage(&c, "resource", "work", unit);
        let _ = report::usage(&c, "group", "cost", unit);
        let _ = report::usage(&c, "task", "work", unit);
        let _ = report::scurve(&c, unit, "duration", None);
        let _ = report::curve(&c, unit);
        let _ = progress::manpower(&c, unit);
    }
    let _ = report::period_report(&c, "week", None);
    let _ = progress::earned_schedule(&c);
    let _ = progress::lookahead(&c, 3);
    let _ = check::run(&c, &check::Limits::default());
    let rows: Vec<usize> = (0..p.tasks.len()).collect();
    let _ = network::layout(&c, &rows);
    let _ = crate::level::preview(p, &[]);
    let _ = risk::simulate(p, &risk::SimOpts { iterations: 20, ..Default::default() });
    let cost = load::compute(p, &s).cost;
    let _ = mspdi::write(p, &s, &cost);
    let _ = xer::write(p, &s, start());
    let cols: Vec<String> = q.cols.clone();
    let _ = (crate::io::formats::json(&c, &cols), crate::io::formats::html(&c, &cols), crate::io::formats::ics(&c), crate::io::formats::mpx(&c), crate::io::formats::pmxml(&c));
}

#[test]
/// Values no plan should hold, as a damaged or hand-made file could: nothing may panic or hang.
fn hostile_values_never_panic() {
    let run = |name: &str, f: &dyn Fn(&mut Project)| {
        let mut p = sample();
        f(&mut p);
        // As loaded (no clamping): the scheduler alone must not panic.
        assert!(panic::catch_unwind(panic::AssertUnwindSafe(|| { let _ = schedule(&p); })).is_ok(), "{name}: the raw plan panicked");
        // As every way into the app leaves it (opening, importing, editing): everything must work, quickly.
        p.normalize();
        let t = Instant::now();
        let r = panic::catch_unwind(panic::AssertUnwindSafe(|| exercise(&p)));
        assert!(r.is_ok(), "{name}: panicked");
        assert!(t.elapsed().as_secs() < 60, "{name}: took {} s", t.elapsed().as_secs());
    };
    run("no working time", &|p| { for c in &mut p.calendars { c.week = Default::default() } });
    run("huge duration", &|p| p.tasks[1].dur = 1_000_000_000_000);
    run("year 9999 start", &|p| p.start = NaiveDate::from_ymd_opt(9999, 12, 20).unwrap_or_default());
    run("year 1 start", &|p| p.start = NaiveDate::from_ymd_opt(1, 1, 1).unwrap_or_default());
    run("constraint far future", &|p| { p.tasks[1].constraint = Constraint::MSO; p.tasks[1].cdate = NaiveDate::from_ymd_opt(9000, 1, 1) });
    run("huge lag", &|p| { if let Some(t) = p.tasks.iter_mut().find(|t| !t.preds.is_empty()) { t.preds[0].lag = i64::MAX / 4 } });
    run("negative huge lag", &|p| { if let Some(t) = p.tasks.iter_mut().find(|t| !t.preds.is_empty()) { t.preds[0].lag = i64::MIN / 4 } });
    run("next_uid max", &|p| p.next_uid = u32::MAX);
    run("outline level 255", &|p| p.tasks[3].level = 255);
    run("status year 9999", &|p| p.status = NaiveDate::from_ymd_opt(9999, 1, 1));
    run("weird hours", &|p| p.calendars[0].week[0] = vec![(2000, 100), (900, 900), (0, 1440)]);
    run("splits huge", &|p| p.tasks[1].splits = vec![(1, i64::MAX / 4)]);
    run("level delay huge", &|p| p.tasks[1].level_delay = i64::MAX / 4);
    run("units huge", &|p| { if let Some(t) = p.tasks.iter_mut().find(|t| !t.res.is_empty()) { t.res[0].units = 1e300 } });
    run("zero-length day", &|p| p.day_min = 0);
    run("day longer than a day", &|p| p.day_min = 100_000);
    run("nan costs", &|p| { p.tasks[1].fixed_cost = f64::NAN; p.tasks[1].qty = f64::INFINITY });
    run("every task linked to itself", &|p| { for t in &mut p.tasks { let u = t.uid; t.preds.push(Link { pred: u, kind: LinkType::FS, lag: 0, pct: false }) } });
    run("exception span", &|p| p.calendars[0].exceptions.push(Exception { name: "x".into(), from: NaiveDate::from_ymd_opt(1, 1, 1).unwrap_or_default(), to: NaiveDate::from_ymd_opt(9999, 1, 1).unwrap_or_default(), hours: vec![] }));
}

#[test]
fn hostile_plan_files_open_safely() {
    // The same values as a .dplan file: opening it clamps them, and the plan schedules.
    let mut v = serde_json::to_value(sample()).unwrap_or_default();
    v["tasks"][1]["dur"] = serde_json::json!(9_000_000_000_000_i64);
    v["tasks"][1]["level_delay"] = serde_json::json!(-5);
    v["tasks"][2]["splits"] = serde_json::json!([[1, 9_000_000_000_000_i64], [0, 0]]);
    v["next_uid"] = serde_json::json!(u32::MAX);
    v["day_min"] = serde_json::json!(0);
    let p = Project::from_json(&v.to_string()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((p.tasks[1].dur, p.tasks[1].level_delay, p.tasks[2].splits.len(), p.day_min), (MAX_SPAN, 0, 1, 480));
    assert!(schedule(&p).is_ok());
}

#[test]
fn hostile_sheet_formulas() {
    let cases: Vec<(&str, String)> = vec![
        ("whole sheet sum", "=SUM(A1:XFD1048576)".into()),
        ("circular", "=B1+1".into()),
        ("self", "=A1".into()),
        ("deep nesting", format!("={}1{}", "(".repeat(5000), ")".repeat(5000))),
        ("long text", format!("=\"{}\"", "x".repeat(200_000))),
        ("huge number", "=10^308*10".into()),
        ("repeat", "=REPT(\"x\",100000000)".into()),
        ("sequence", "=SEQUENCE(100000000)".into()),
        ("bad address", "=ZZZZZZZZ99999999999".into()),
        ("plan range", "=SUM(Plan!A1:XFD1048576)".into()),
        ("indirect", "=INDIRECT(\"A1\")".into()),
    ];
    for (name, f) in cases {
        let mut d = Doc::new(sample());
        let _ = d.apply(Op::Sheet { k: None, sheet: Some(SheetData { name: "S".into(), ..Default::default() }) }, "%d-%b-%y");
        let t = Instant::now();
        let r = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            let _ = d.apply(Op::SheetCells { k: 0, cells: vec![("A1".into(), f.clone()), ("B1".into(), "=A1".into())], fmt: None }, "%d-%b-%y");
            let c = crate::fields::Calc::new(&d.p, "%d-%b-%y", start());
            let v = crate::sheet::sheet_view(&c, 0);
            assert!(v.is_some(), "{name}: no sheet");
        }));
        assert!(r.is_ok(), "{name}: panicked");
        assert!(t.elapsed().as_secs() < 10, "{name}: took {} s", t.elapsed().as_secs());
    }
}

#[test]
fn extreme_edits_never_panic() {
    let base = sample();
    let (a, b) = (base.tasks[1].uid, base.tasks[2].uid);
    let mut evil = base.tasks[1].clone();
    (evil.uid, evil.dur, evil.level, evil.preds) = (u32::MAX, i64::MAX, 250, vec![Link { pred: u32::MAX, kind: LinkType::SF, lag: i64::MIN, pct: false }]);
    let ops = vec![
        Op::Insert { at: usize::MAX, name: "x".repeat(100_000), ms: false },
        Op::InsertRows { at: usize::MAX, rows: vec![vec![("dur".into(), "99999999d".into()), ("name".into(), "x".into())]; 2000] },
        Op::Paste { at: 3, tasks: vec![evil.clone(), evil] },
        Op::Recurring { at: 0, name: "R".into(), dur: "1d".into(), every: "month".into(), interval: u32::MAX, count: 500, start: "05-Nov-26".into() },
        Op::Split { uid: a, after: i64::MAX / 2, gap: i64::MAX / 2 },
        Op::Split { uid: a, after: 1, gap: i64::MAX },
        Op::LinkSet { pred: a, succ: b, kind: Some(LinkType::FS), lag: "99999999999d".into() },
        Op::LinkSet { pred: a, succ: b, kind: Some(LinkType::SS), lag: "-1e300%".into() },
        Op::Set { uid: a, field: "leveldelay".into(), value: "9e99d".into() },
        Op::Set { uid: a, field: "dmax".into(), value: "99999999999999d".into() },
        Op::Set { uid: a, field: "cdate".into(), value: "31-Dec-9999".into() },
        Op::Set { uid: a, field: "pct".into(), value: "255".into() },
        Op::Set { uid: a, field: "qty".into(), value: "1e308".into() },
        Op::Columns { cols: vec![("name".into(), i32::MIN), ("nonsense".into(), i32::MAX)] },
        Op::Cells { cells: vec![(a, "dur".into(), "1e18d".into()); 1000] },
        Op::Baseline { uids: vec![], clear: false, n: 250 },
        Op::Interim { n: 255 },
        Op::UpdateProject { through: "01-Jan-2600".into(), reschedule: true },
        Op::Snapshot { label: "x".repeat(10_000), comment: String::new() },
        Op::Level { uids: vec![u32::MAX], clear: false },
        Op::Log { id: u32::MAX, item: Some(LogItem { uid: a, need: NaiveDate::from_ymd_opt(1, 1, 1), ..Default::default() }) },
        Op::ApplyCredit { uids: vec![a, b], rule: "Cable".into() },
        Op::Credit { old: None, rule: Some(CreditRule { name: "Nan".into(), steps: vec![("x".into(), f64::NAN)] }) },
    ];
    for o in ops {
        let mut d = Doc::new(sample());
        let name = format!("{o:?}").chars().take(60).collect::<String>();
        let before = d.json();
        let t = Instant::now();
        let r = panic::catch_unwind(panic::AssertUnwindSafe(|| d.apply(o, "%d-%b-%y")));
        let Ok(r) = r else { panic!("{name}: panicked") };
        if r.is_err() { assert_eq!(d.json(), before, "{name}: failed but changed the plan") }
        assert!(schedule(&d.p).is_ok(), "{name}: left a plan that does not schedule");
        exercise(&d.p);
        assert!(t.elapsed().as_secs() < 60, "{name}: took {} s", t.elapsed().as_secs());
    }
}

#[test]
fn hostile_import_files_are_refused_not_crashing() {
    let dir = std::env::temp_dir().join(format!("dp_hostile_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // 200,000 nested tags: refused, not a stack overflow on a small stack.
    let n = 200_000;
    let xml = format!("<Project><Name>x</Name>{}{}</Project>", "<a>".repeat(n), "</a>".repeat(n));
    let p = dir.join("deep.xml");
    std::fs::write(&p, xml).unwrap();
    let ok = std::thread::Builder::new().stack_size(2 << 20).spawn(move || crate::io::import(&p).is_err()).unwrap().join().unwrap();
    assert!(ok);
    // Two cells far apart in a tiny workbook: read cell by cell, not as a 17-billion-cell box.
    let mut wb = rust_xlsxwriter::Workbook::new();
    let ws = wb.add_worksheet();
    ws.write_string(0, 0, "Name").unwrap();
    ws.write_string(1_048_575, 16_383, "far").unwrap();
    let x = dir.join("sparse.xlsx");
    wb.save(&x).unwrap();
    let t = crate::io::table::read(&x).unwrap();
    assert_eq!(t.headers, vec!["Name".to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}
