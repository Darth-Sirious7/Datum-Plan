//! Network Diagram layout (network boxes): one box per task, left to right by logic,
//! each task one column after its latest predecessor, rows ordered to keep links short.
//! Optionally summary tasks get a box (over their first child), and "by WBS" puts each
//! top-level phase in its own horizontal band.
use crate::fields::Calc;
use serde::Serialize;

#[derive(Serialize)]
pub struct Node { pub uid: u32, pub col: usize, pub row: usize }

#[derive(Serialize)]
pub struct NetLink { pub from: u32, pub to: u32, pub kind: crate::model::LinkType, pub crit: bool, pub driving: bool, pub lag: String }

/// A WBS band: the phase name and the rows it covers.
#[derive(Serialize)]
pub struct Band { pub name: String, pub row: usize, pub rows: usize }

#[derive(Serialize)]
pub struct Net { pub nodes: Vec<Node>, pub links: Vec<NetLink>, pub cols: usize, pub rows: usize, pub bands: Vec<Band> }

/// Lays out the given rows (task indexes, already filtered) as a network.
pub fn layout(c: &Calc, rows: &[usize]) -> Net {
    layout_with(c, rows, &[], false)
}

/// As `layout`, also placing the summary tasks `sums`, and banding by top-level WBS when `wbs`.
pub fn layout_with(c: &Calc, rows: &[usize], sums: &[usize], wbs: bool) -> Net {
    let (p, s) = (c.p, &c.s);
    let n = p.tasks.len();
    let mut shown = vec![false; n];
    rows.iter().chain(sums).for_each(|&i| shown[i] = true);
    let edges: Vec<(usize, &crate::sched::Edge)> = s.edges.iter().enumerate().filter(|(_, e)| shown[e.p] && shown[e.s]).collect();
    let mut preds: Vec<Vec<usize>> = vec![vec![]; n];
    edges.iter().filter(|(_, e)| e.p != e.s && !p.is_summary(e.s) && !p.is_summary(e.p)).for_each(|(_, e)| preds[e.s].push(e.p));
    // Columns: longest chain of predecessors, in schedule order so predecessors come first.
    let mut order: Vec<usize> = rows.to_vec();
    order.sort_by_key(|&i| (s.es[i], s.ef[i], i));
    let mut col = vec![0usize; n];
    for _ in 0..2 {
        for &i in &order {
            col[i] = preds[i].iter().map(|&q| col[q] + 1).max().unwrap_or(0);
        }
    }
    // A summary sits over its first shown child.
    // Each task's top-level phase, in one pass (an open-summary stack, not a scan back per task).
    let mut open: Vec<usize> = vec![];
    let tops: Vec<usize> = (0..n).map(|i| { while open.last().is_some_and(|&j| p.tasks[j].level >= p.tasks[i].level) { open.pop(); } let t = open.first().copied().unwrap_or(i); open.push(i); t }).collect();
    let top = |i: usize| tops[i];
    for &sm in sums.iter().rev() {
        let kids: Vec<usize> = (sm + 1..n).take_while(|&k| p.tasks[k].level > p.tasks[sm].level).filter(|&k| shown[k]).collect();
        col[sm] = kids.iter().map(|&k| col[k]).min().unwrap_or(0);
    }
    let cols = rows.iter().chain(sums).map(|&i| col[i] + 1).max().unwrap_or(0);
    // Groups: one per top-level phase in WBS mode, otherwise one for everything.
    let mut groups: Vec<(usize, Vec<usize>)> = vec![];
    for &i in &order {
        let g = if wbs { top(i) } else { usize::MAX };
        match groups.iter_mut().find(|x| x.0 == g) { Some(x) => x.1.push(i), None => groups.push((g, vec![i])) }
    }
    if wbs { groups.sort_by_key(|x| x.0); }
    let mut row = vec![0usize; n];
    let (mut height, mut bands) = (0usize, vec![]);
    for (g, members) in groups {
        let base = height;
        let head: Vec<usize> = sums.iter().copied().filter(|&sm| (wbs && sm == g) || (!wbs && p.tasks[sm].level == 0)).collect();
        let mut used: Vec<Vec<bool>> = vec![vec![]; cols];
        let mut take = |c: usize, want: usize| -> usize { let mut r = want; while used[c].get(r).copied().unwrap_or(false) { r += 1; } if used[c].len() <= r { used[c].resize(r + 1, false); } used[c][r] = true; r };
        // Top-level summaries take the band's first row; the band's tasks go below them.
        let first = if head.is_empty() { 0 } else { 1 };
        for &sm in &head { row[sm] = base + take(col[sm], 0); }
        let mut by_col: Vec<Vec<usize>> = vec![vec![]; cols];
        members.iter().for_each(|&i| by_col[col[i]].push(i));
        for here in by_col {
            let mut here: Vec<(f64, i64, usize)> = here.into_iter().map(|i| {
                let pr: Vec<f64> = preds[i].iter().filter(|&&q| row[q] >= base).map(|&q| (row[q] - base) as f64).collect();
                (if pr.is_empty() { f64::MAX } else { pr.iter().sum::<f64>() / pr.len() as f64 }, s.es[i], i)
            }).collect();
            here.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
            for (want, _, i) in here { row[i] = base + take(col[i], if want == f64::MAX { first } else { (want.round() as usize).max(first) }); }
        }
        // Other summaries go to the first free row of their column in the band.
        for &sm in sums.iter().filter(|&&sm| !head.contains(&sm) && (!wbs || top(sm) == g)) { row[sm] = base + take(col[sm], first); }
        let rows_used = used.iter().map(Vec::len).max().unwrap_or(0).max(1);
        if wbs { bands.push(Band { name: if g < n { p.tasks[g].name.clone() } else { String::new() }, row: base, rows: rows_used }); }
        height = base + rows_used;
    }
    let all: Vec<usize> = sums.iter().chain(rows).copied().collect();
    Net {
        nodes: all.iter().map(|&i| Node { uid: p.tasks[i].uid, col: col[i], row: row[i] }).collect(),
        links: edges.iter().map(|&(k, e)| NetLink {
            from: p.tasks[e.p].uid, to: p.tasks[e.s].uid, kind: e.kind, crit: s.critical(p, e.p) && s.critical(p, e.s),
            driving: s.drive.get(k).copied().unwrap_or(false), lag: if e.lag == 0 { String::new() } else { crate::parse::fmt_lag(e.lag, p.day_min) },
        }).collect(),
        cols, rows: height, bands,
    }
}
