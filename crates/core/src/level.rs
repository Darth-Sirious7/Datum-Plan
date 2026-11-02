//! Resource leveling: delay lower-ranked tasks until no work resource is over its daily capacity.
use crate::cal::{date_of, DAY};
use crate::model::{Constraint, LevelOrder, Project, ResKind};
use crate::sched::{schedule, Sched};
use std::collections::{BTreeMap, HashSet};

fn movable(p: &Project, s: &Sched, i: usize) -> bool {
    let t = &p.tasks[i];
    !s.summary[i] && !s.off[i] && !t.manual && t.act_start.is_none() && !matches!(t.constraint, Constraint::MSO | Constraint::MFO)
}

/// Daily minutes per (resource index, day) and the tasks behind them.
fn loads(p: &Project, s: &Sched) -> BTreeMap<(i64, usize), (f64, Vec<usize>)> {
    let mut m: BTreeMap<(i64, usize), (f64, Vec<usize>)> = BTreeMap::new();
    for (i, t) in p.tasks.iter().enumerate().filter(|&(i, _)| !s.summary[i] && !s.off[i]) {
        let c = &s.cals[s.ci[i]];
        for a in t.res.iter().filter(|a| a.units > 0.0) {
            let Some(k) = p.res_idx(a.res).filter(|&k| p.resources[k].kind == ResKind::Work) else { continue };
            let mut d = s.es[i].div_euclid(DAY);
            while d * DAY < s.ef[i] {
                let mins = c.between(s.es[i].max(d * DAY), s.ef[i].min((d + 1) * DAY)) as f64 * a.units;
                if mins > 0.0 {
                    let e = m.entry((d, k)).or_default();
                    e.0 += mins;
                    e.1.push(i);
                }
                d += 1;
            }
        }
    }
    m
}

/// Levels the given tasks (all tasks when empty). Returns how many delays were added.
pub fn level(p: &mut Project, only: &[u32]) -> Result<usize, String> {
    let scope: HashSet<usize> = if only.is_empty() { (0..p.tasks.len()).collect() } else { only.iter().filter_map(|&u| p.idx(u)).collect() };
    scope.iter().for_each(|&i| p.tasks[i].level_delay = 0);
    let mut stuck: HashSet<(i64, usize)> = HashSet::new();
    let mut moves = 0;
    // Rank once, from the unleveled schedule, so tasks do not take turns being delayed.
    let s0 = schedule(p)?;
    let rank: Vec<(i64, i64, i64, i64)> = (0..p.tasks.len()).map(|i| {
        let pr = 1000 - p.priority(i) as i64;
        match p.opts.level_order {
            LevelOrder::IdOnly => (i as i64, 0, 0, 0),
            LevelOrder::Standard => (s0.tf[i], s0.es[i], pr, i as i64),
            LevelOrder::Priority => (pr, s0.tf[i], s0.es[i], i as i64),
        }
    }).collect();
    for _ in 0..5000 {
        let s = schedule(p)?;
        let cap = |d: i64, k: usize| p.resources[k].max_on(date_of(d * DAY)) * s.cals[s.rcal[k]].day_minutes(date_of(d * DAY)).max(1) as f64;
        let over: Vec<((i64, usize), Vec<usize>)> = loads(p, &s).into_iter().filter(|((d, k), (mins, _))| *mins > cap(*d, *k) + 0.5 && !stuck.contains(&(*d, *k))).map(|(x, (_, v))| (x, v)).collect();
        // Days in order, every resource on a day in one pass (a task moved for one is gone for the
        // rest); moves change only later days, so a pass ends after the first day with a move.
        if over.is_empty() { break }
        let (mut moved, mut done) = (HashSet::new(), None);
        for ((day, k), tasks) in over {
            if done.is_some_and(|d| d < day) { break }
            let mut cands: Vec<usize> = tasks.iter().copied().filter(|&i| scope.contains(&i) && movable(p, &s, i) && !moved.contains(&i)).collect();
            cands.sort_by_key(|&i| rank[i]);
            cands.dedup();
            // The highest-ranked task keeps its place, and so do the next ones while the day has room;
            // the rest move past this day, all in one go.
            let cap = cap(day, k);
            let on_day = |i: usize| s.cals[s.ci[i]].between(s.es[i].max(day * DAY), s.ef[i].min((day + 1) * DAY)) as f64
                * p.tasks[i].res.iter().filter(|a| p.res_idx(a.res) == Some(k)).map(|a| a.units).sum::<f64>();
            let mut used: f64 = tasks.iter().copied().collect::<HashSet<_>>().into_iter().filter(|i| !cands.contains(i) && !moved.contains(i)).map(on_day).sum();
            let mut delay = vec![];
            for (n, &i) in cands.iter().enumerate() {
                let c = &s.cals[s.ci[i]];
                let add = c.between(s.es[i], c.next_start((day + 1) * DAY)).max(60);
                if n == 0 || used + on_day(i) <= cap + 0.5 || (p.opts.level_within_slack && s.tf[i] < add) {
                    used += on_day(i);
                } else {
                    delay.push((i, add));
                }
            }
            if delay.is_empty() {
                stuck.insert((day, k));
            } else {
                done = Some(day);
            }
            moves += delay.len();
            for (i, add) in delay {
                p.tasks[i].level_delay += add;
                moved.insert(i);
            }
        }
    }
    Ok(moves)
}

/// Removes leveling delays from the given tasks (all when empty).
pub fn clear(p: &mut Project, only: &[u32]) {
    for t in p.tasks.iter_mut().filter(|t| only.is_empty() || only.contains(&t.uid)) {
        t.level_delay = 0;
    }
}

/// A task that levelling would move.
#[derive(serde::Serialize, Debug)]
pub struct Moved { pub uid: u32, pub name: String, pub start: i64, pub new_start: i64, pub finish: i64, pub new_finish: i64 }

#[derive(serde::Serialize, Debug, Default)]
pub struct Preview { pub moved: Vec<Moved>, pub finish: i64, pub new_finish: i64, pub over_before: usize, pub over_after: usize }

/// What levelling the given tasks (all when empty) would do, without changing the plan.
pub fn preview(p: &Project, only: &[u32]) -> Result<Preview, String> {
    let mut q = p.clone();
    level(&mut q, only)?;
    let (a, b) = (schedule(p)?, schedule(&q)?);
    // Days on which some work resource is booked over its capacity.
    let over = |p: &Project, s: &Sched| loads(p, s).iter()
        .filter(|((d, k), (m, _))| *m > p.resources[*k].max_on(date_of(d * DAY)) * s.cals[s.rcal[*k]].day_minutes(date_of(d * DAY)).max(1) as f64 + 0.5)
        .map(|((d, _), _)| *d).collect::<HashSet<_>>().len();
    let moved = (0..p.tasks.len()).filter(|&i| !a.summary[i] && (a.es[i] != b.es[i] || a.ef[i] != b.ef[i]))
        .map(|i| Moved { uid: p.tasks[i].uid, name: p.tasks[i].name.clone(), start: a.es[i], new_start: b.es[i], finish: a.ef[i], new_finish: b.ef[i] }).collect();
    Ok(Preview { moved, finish: a.finish, new_finish: b.finish, over_before: over(p, &a), over_after: over(&q, &b) })
}
