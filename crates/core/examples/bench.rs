//! cargo run --release -p datum-core --example bench -- 20000
use datum_core::{fields::Calc, query::Query, *};
use std::time::Instant;

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(20000);
    let mut p = Project::new("Bench", chrono::NaiveDate::from_ymd_opt(2026, 11, 2).unwrap());
    for i in 0..n {
        let lvl = if i % 50 == 0 { 0 } else { 1 };
        let mut t = p.new_task(&format!("Task {i}"), lvl, if lvl == 0 { 0 } else { 480 * (1 + i as i64 % 5) });
        if lvl == 1 && i % 50 > 1 {
            t.preds.push(Link { pred: t.uid - 1, kind: LinkType::FS, lag: 0, pct: false, elapsed: false });
        }
        p.tasks.push(t);
    }
    p.normalize();
    let t0 = Instant::now();
    let s = sched::schedule(&p).unwrap();
    let t1 = Instant::now();
    let c = Calc::new(&p, "%d-%b-%y", chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap());
    let t2 = Instant::now();
    let g = c.grid(&Query { cols: ["id", "name", "dur", "start", "finish", "preds", "res", "pct", "cost"].map(String::from).to_vec(), ..Query::default() });
    let t3 = Instant::now();
    let json = serde_json::to_string(&g).unwrap();
    let t4 = Instant::now();
    if std::env::var("ROW").is_ok() { println!("{}", serde_json::to_string(&g.rows[5]).unwrap()); }
    let checks = check::run(&c, &Default::default());
    let t5 = Instant::now();
    let size = serde_json::to_vec(&p).map(|v| v.len() / 1024).unwrap_or(0);
    let tv = Instant::now();
    let v = serde_json::to_value(&p).unwrap();
    println!("plan as JSON: {size} KB; to_value {:?}", tv.elapsed());
    drop(v);
    println!("{n} tasks, {} edges: schedule {:?}, calc {:?}, grid {:?}, json {:?} ({} KB), checks {:?} ({})", s.edges.len(), t1 - t0, t2 - t1, t3 - t2, t4 - t3, json.len() / 1024, t5 - t4, checks.len());
    edit_cycle(p);
}

/// The edit cycle at size: an edit through `Doc::apply` (copy, normalize, schedule, undo step),
/// saving and opening the file.
fn rss() -> usize { std::fs::read_to_string("/proc/self/statm").ok().and_then(|s| s.split_whitespace().nth(1)?.parse::<usize>().ok()).unwrap_or(0) * 4096 / 1024 / 1024 }
fn edit_cycle(p: Project) {
    let before = rss();
    let copies: Vec<Project> = (0..5).map(|_| p.clone()).collect();
    println!("memory per copy of the plan: {} MB", (rss() - before) / 5);
    drop(copies);
    let mut d = doc::Doc::new(p);
    let uid = d.p.tasks[d.p.tasks.len() / 2 + 1].uid;
    for k in 0..3 {
        let t = Instant::now();
        d.apply(doc::Op::Set { uid, field: "dur".into(), value: format!("{}d", 3 + k) }, "%d-%b-%y").unwrap();
        println!("edit {k}: {:?}", t.elapsed());
    }
    let dir = std::env::temp_dir().join("datum-bench.dplan");
    let t = Instant::now();
    d.save(&dir).unwrap();
    let t1 = Instant::now();
    let e = doc::Doc::open(&dir).unwrap();
    println!("save {:?} ({} KB), open {:?} ({} tasks)", t1 - t, std::fs::metadata(&dir).map(|m| m.len() / 1024).unwrap_or(0), t1.elapsed(), e.p.tasks.len());
}
