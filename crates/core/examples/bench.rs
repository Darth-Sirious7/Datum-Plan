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
    let checks = check::run(&c, &Default::default());
    let t5 = Instant::now();
    println!("{n} tasks, {} edges: schedule {:?}, calc {:?}, grid {:?}, json {:?} ({} KB), checks {:?} ({})", s.edges.len(), t1 - t0, t2 - t1, t3 - t2, t4 - t3, json.len() / 1024, t5 - t4, checks.len());
}
