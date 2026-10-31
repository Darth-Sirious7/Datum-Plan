//! Timephased data: Task Usage and Resource Usage grids, cash flow and S-curves, and the
//! period figures behind progress reports.
use crate::cal::{date_of, DAY};
use crate::fields::Calc;
use crate::load::buckets;
use crate::model::ResKind;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
pub struct UseRow {
    /// Task or resource uid; `of` is the parent row for assignment rows.
    pub uid: u32,
    pub of: Option<u32>,
    pub name: String,
    pub lvl: u8,
    /// Summary task or resource row (shown bold).
    pub sum: bool,
    pub total: f64,
    pub vals: Vec<f64>,
    pub over: Vec<bool>,
    /// Hours available per bucket (resource and trade rows).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cap: Vec<f64>,
}

#[derive(Serialize)]
pub struct Usage {
    pub buckets: Vec<i64>,
    pub unit: String,
    pub rows: Vec<UseRow>,
    /// Working hours of one person full time in each bucket (to show hours as people).
    pub person: Vec<f64>,
}

fn bucket_of(b: &[i64], d: i64) -> Option<usize> {
    if b.is_empty() || d < b[0] {
        return None;
    }
    Some(b.partition_point(|&x| x <= d) - 1)
}

/// Usage grid: `by` is "task" or "resource", `what` is "work" (hours) or "cost".
pub fn usage(c: &Calc, by: &str, what: &str, unit: &str) -> Usage {
    let (p, s, l) = (c.p, &c.s, &c.l);
    let b = buckets(s.start.div_euclid(DAY), (s.finish - 1).max(s.start).div_euclid(DAY), unit);
    let cost = what == "cost";
    let empty = || vec![0.0; b.len()];
    let mut rows = vec![];
    let rate = |k: usize, i: usize, d: i64| {
        let a = p.tasks[i].res.iter().find(|a| p.res_idx(a.res) == Some(k)).map_or(0, |a| a.table);
        p.resources[k].rate_on(a, date_of(d * DAY)).0
    };
    let pc = &s.cals[p.calendars.iter().position(|c| c.name == p.calendar).unwrap_or(0)];
    let tail = b.last().map_or(0, |&x| buckets(x, x + 400, unit).get(1).copied().unwrap_or(x + 1));
    let days_of = |j: usize| b[j]..b.get(j + 1).copied().unwrap_or(tail);
    let person: Vec<f64> = (0..b.len()).map(|j| days_of(j).map(|d| pc.day_minutes(date_of(d * DAY)) as f64 / 60.0).sum()).collect();
    if by == "resource" || by == "group" {
        let mut per: BTreeMap<(usize, usize), Vec<f64>> = BTreeMap::new();
        for (&(k, i, d), &m) in &l.assign_daily {
            if let Some(j) = bucket_of(&b, d) {
                per.entry((k, i)).or_insert_with(empty)[j] += if cost { m / 60.0 * rate(k, i, d) } else { m / 60.0 };
            }
        }
        let mut res_rows = vec![];
        for (k, r) in p.resources.iter().enumerate() {
            let kids: Vec<(&(usize, usize), &Vec<f64>)> = per.range((k, 0)..(k + 1, 0)).collect();
            let mut vals = empty();
            kids.iter().for_each(|(_, v)| v.iter().enumerate().for_each(|(j, x)| vals[j] += x));
            let over = b.iter().enumerate().map(|(j, &d0)| {
                let end = b.get(j + 1).copied().unwrap_or(d0 + 1);
                r.kind == ResKind::Work && (d0..end).any(|d| l.daily.get(&(k, d)).is_some_and(|&m| m > r.max_on(date_of(d * DAY)) * s.cals[s.rcal[k]].day_minutes(date_of(d * DAY)) as f64 + 0.5))
            }).collect();
            let cap = if r.kind == ResKind::Work { (0..b.len()).map(|j| days_of(j).map(|d| r.max_on(date_of(d * DAY)) * s.cals[s.rcal[k]].day_minutes(date_of(d * DAY)) as f64 / 60.0).sum()).collect() } else { vec![] };
            let kid_rows: Vec<UseRow> = kids.iter().map(|((_, i), v)| UseRow { uid: p.tasks[*i].uid, of: Some(r.uid), name: p.tasks[*i].name.clone(), lvl: 1, sum: false, total: v.iter().sum(), vals: (*v).clone(), over: vec![], cap: vec![] }).collect();
            res_rows.push((UseRow { uid: r.uid, of: None, name: r.name.clone(), lvl: 0, sum: true, total: vals.iter().sum(), vals, over, cap }, kid_rows));
        }
        if by == "resource" {
            for (r, kids) in res_rows {
                rows.push(r);
                rows.extend(kids);
            }
        } else {
            // Trades (resource groups) of work resources: the total against the trade's capacity.
            let mut groups: Vec<(String, Vec<UseRow>)> = vec![];
            for ((mut r, _), res) in res_rows.into_iter().zip(&p.resources).filter(|(_, x)| x.kind == ResKind::Work) {
                let g = if res.group.trim().is_empty() { "No trade".to_string() } else { res.group.trim().to_string() };
                (r.sum, r.lvl) = (false, 1);
                match groups.iter_mut().find(|x| x.0 == g) { Some(x) => x.1.push(r), None => groups.push((g, vec![r])) }
            }
            groups.sort_by(|a, b| (a.0 == "No trade").cmp(&(b.0 == "No trade")).then(a.0.cmp(&b.0)));
            for (k, (g, mut members)) in groups.into_iter().enumerate() {
                let (mut vals, mut cap) = (empty(), empty());
                for m in &members { (0..b.len()).for_each(|j| { vals[j] += m.vals[j]; cap[j] += m.cap.get(j).copied().unwrap_or(0.0) }) }
                let over = (0..b.len()).map(|j| vals[j] > cap[j] + 0.01).collect();
                let uid = k as u32 + 1;
                members.iter_mut().for_each(|m| m.of = Some(uid));
                rows.push(UseRow { uid, of: None, name: g, lvl: 0, sum: true, total: vals.iter().sum(), vals, over, cap });
                rows.extend(members);
            }
        }
    } else {
        let mut per: Vec<Vec<f64>> = vec![empty(); p.tasks.len()];
        for (&(i, d), &(m, cst)) in &l.task_daily {
            if let Some(j) = bucket_of(&b, d) {
                per[i][j] += if cost { cst } else { m / 60.0 };
            }
        }
        for i in (0..p.tasks.len()).rev().filter(|&i| s.summary[i]) {
            for j in p.subtree(i).skip(1).filter(|&j| !s.summary[j]) {
                let leaf = per[j].clone();
                per[i].iter_mut().zip(leaf).for_each(|(a, x)| *a += x);
            }
        }
        for (i, t) in p.tasks.iter().enumerate().filter(|&(i, _)| !s.off[i]) {
            rows.push(UseRow { uid: t.uid, of: None, name: t.name.clone(), lvl: t.level, sum: s.summary[i], total: per[i].iter().sum(), vals: per[i].clone(), over: vec![], cap: vec![] });
            if s.summary[i] {
                continue;
            }
            for a in &t.res {
                let Some(k) = p.res_idx(a.res) else { continue };
                let mut v = empty();
                for (&(_, _, d), &m) in l.assign_daily.range((k, i, i64::MIN)..=(k, i, i64::MAX)) {
                    if let Some(j) = bucket_of(&b, d) {
                        v[j] += if cost { m / 60.0 * rate(k, i, d) } else { m / 60.0 };
                    }
                }
                rows.push(UseRow { uid: p.resources[k].uid, of: Some(t.uid), name: p.resources[k].name.clone(), lvl: t.level + 1, sum: false, total: v.iter().sum(), vals: v, over: vec![], cap: vec![] });
            }
        }
    }
    Usage { buckets: b, unit: unit.into(), rows, person }
}

/// Cash flow and S-curve data per bucket: baseline (planned value), current forecast and actual cost.
#[derive(Serialize)]
pub struct Curve { pub buckets: Vec<i64>, pub planned: Vec<f64>, pub forecast: Vec<f64>, pub actual: Vec<f64>, pub earned: Vec<f64> }

pub fn curve(c: &Calc, unit: &str) -> Curve {
    let (p, s, l) = (c.p, &c.s, &c.l);
    let leaves: Vec<usize> = (0..p.tasks.len()).filter(|&i| !s.summary[i] && !s.off[i]).collect();
    let lo = leaves.iter().filter_map(|&i| p.tasks[i].bls.get(&0).map(|b| b.start)).chain([s.start]).min().unwrap_or(s.start);
    let hi = leaves.iter().filter_map(|&i| p.tasks[i].bls.get(&0).map(|b| b.finish)).chain([s.finish]).max().unwrap_or(s.finish);
    let b = buckets(lo.div_euclid(DAY), (hi - 1).max(lo).div_euclid(DAY), unit);
    let z = vec![0.0; b.len()];
    let (mut planned, mut forecast, mut actual, mut earned) = (z.clone(), z.clone(), z.clone(), z);
    let dd = c.progress_at().div_euclid(DAY);
    for &i in &leaves {
        let t = &p.tasks[i];
        let cal = &s.cals[s.ci[i]];
        if let Some(bl) = t.bls.get(&0) {
            let tot = cal.between(bl.start, bl.finish).max(1) as f64;
            let mut d = bl.start.div_euclid(DAY);
            let done = (if t.pct_type == crate::model::PctType::Physical { t.phys } else { t.pct }) as f64 / 100.0;
            if bl.finish <= bl.start {
                if let Some(j) = bucket_of(&b, d) {
                    planned[j] += bl.cost;
                }
            }
            while d * DAY < bl.finish {
                let share = cal.between(bl.start.max(d * DAY), bl.finish.min((d + 1) * DAY)) as f64 / tot;
                if let Some(j) = bucket_of(&b, d) {
                    planned[j] += bl.cost * share;
                }
                d += 1;
            }
            // Earned value (budget × % complete) where the work was done: over the task's dates up to
            // the data date, or on the data date when none of its dates fall before it.
            let ev = bl.cost * done;
            if ev > 0.0 {
                let (a, z) = (s.es[i], s.ef[i].min(dd * DAY));
                let worked = if z > a { cal.between(a, z) as f64 } else { 0.0 };
                if worked > 0.0 {
                    let mut d = a.div_euclid(DAY);
                    while d * DAY < z {
                        if let Some(j) = bucket_of(&b, d) { earned[j] += ev * cal.between(a.max(d * DAY), z.min((d + 1) * DAY)) as f64 / worked }
                        d += 1;
                    }
                } else if let Some(j) = bucket_of(&b, (dd - 1).max(a.div_euclid(DAY))) {
                    earned[j] += ev;
                }
            }
        }
        let mut before = 0.0;
        for (&(_, d), &(_, cost)) in l.task_daily.range((i, i64::MIN)..=(i, i64::MAX)) {
            if let Some(j) = bucket_of(&b, d) {
                forecast[j] += cost;
                if d < dd {
                    before += cost;
                }
            }
        }
        // Actual cost is spread like the forecast up to the status date.
        let scale = if before > 0.0 { l.acost[i] / before } else { 0.0 };
        for (&(_, d), &(_, cost)) in l.task_daily.range((i, i64::MIN)..=(i, i64::MAX)) {
            if let Some(j) = bucket_of(&b, d).filter(|_| d < dd) {
                actual[j] += cost * scale;
            }
        }
    }
    Curve { buckets: b, planned, forecast, actual, earned }
}

/// One task's difference between two versions of a plan (Compare Projects).
#[derive(Serialize)]
pub struct Diff { pub uid: u32, pub name: String, pub change: &'static str, pub was: [String; 4], pub now: [String; 4], pub finish_var: String }

/// Compares `old` with the current plan by unique ID: start, finish, duration and cost.
pub fn compare(old: &Calc, new: &Calc) -> Vec<Diff> {
    let keys = ["start", "finish", "dur", "cost"];
    let pick = |c: &Calc, i: usize| keys.map(|k| c.text(i, k));
    let mut out = vec![];
    for (i, t) in new.p.tasks.iter().enumerate() {
        match old.p.idx(t.uid) {
            None => out.push(Diff { uid: t.uid, name: t.name.clone(), change: "Added", was: Default::default(), now: pick(new, i), finish_var: String::new() }),
            Some(j) => {
                let (a, b) = (pick(old, j), pick(new, i));
                if a != b || old.p.tasks[j].name != t.name {
                    let c = &new.s.cals[new.s.ci[i]];
                    let (x, y) = (old.s.ef[j], new.s.ef[i]);
                    let var = if y >= x { c.between(x, y) } else { -c.between(y, x) };
                    out.push(Diff { uid: t.uid, name: t.name.clone(), change: "Changed", was: a, now: b, finish_var: crate::parse::fmt_dur(var, crate::model::Unit::D, new.p.day_min) });
                }
            }
        }
    }
    for (j, t) in old.p.tasks.iter().enumerate().filter(|(_, t)| new.p.idx(t.uid).is_none()) {
        out.push(Diff { uid: t.uid, name: t.name.clone(), change: "Removed", was: pick(old, j), now: Default::default(), finish_var: String::new() });
    }
    out
}

// ---------- reporting periods, S-curves and period reports ----------

/// Period ends (inclusive days, as days since 1970-01-01) covering the project, weekly or monthly,
/// following the plan's cut-off settings.
pub fn period_ends(c: &Calc, kind: &str) -> Vec<i64> {
    use chrono::Datelike;
    let (p, s) = (c.p, &c.s);
    let first = s.start.div_euclid(DAY);
    let fin = p.tasks.iter().filter_map(|t| t.bls.get(&0).map(|b| b.finish)).chain([s.finish]).max().unwrap_or(s.finish);
    let last = fin.div_euclid(DAY).max(c.progress_at().div_euclid(DAY)) + if kind == "month" { 31 } else { 7 };
    let mut out = vec![];
    if kind == "month" {
        let d0 = date_of(first * DAY);
        let (mut y, mut m) = (d0.year(), d0.month());
        loop {
            let len = chrono::NaiveDate::from_ymd_opt(if m == 12 { y + 1 } else { y }, if m == 12 { 1 } else { m + 1 }, 1).unwrap().pred_opt().unwrap().day();
            let day = if p.periods.month_end == 0 { len } else { (p.periods.month_end as u32).min(len) };
            let d = crate::cal::at(chrono::NaiveDate::from_ymd_opt(y, m, day).unwrap()) / DAY;
            if d >= first {
                out.push(d);
            }
            if d > last || out.len() > 600 {
                break;
            }
            (y, m) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
        }
    } else {
        // First day on the chosen weekday at or after the start (day 0 = Thursday 1970-01-01).
        let mut d = first + (p.periods.week_end as i64 - (first + 3).rem_euclid(7)).rem_euclid(7);
        while d <= last && out.len() < 3000 {
            out.push(d);
            d += 7;
        }
    }
    out
}

/// Weight of each leaf task for progress: duration, cost, work or (physical) duration, and the
/// weighting actually used (a plan without costs or work falls back to duration).
pub(crate) fn weights(c: &Calc, how: &str) -> (Vec<f64>, String) {
    let (p, s) = (c.p, &c.s);
    let w: Vec<f64> = (0..p.tasks.len()).map(|i| {
        if s.summary[i] || s.off[i] {
            return 0.0;
        }
        match how {
            "cost" => p.tasks[i].bls.get(&0).map_or(c.l.cost[i], |b| b.cost),
            "work" => c.l.work[i],
            _ => p.tasks[i].dur.max(1) as f64,
        }
    }).collect();
    if how != "duration" && w.iter().sum::<f64>() <= 0.0 { weights(c, "duration") } else { (w, how.into()) }
}

/// Share (0–1) of a task's work done by instant `t` if it runs evenly from `a` to `b`.
pub(crate) fn share(cal: &crate::cal::WorkCal, a: i64, b: i64, t: i64) -> f64 {
    if t <= a { 0.0 } else if t >= b { 1.0 } else { cal.between(a, t) as f64 / cal.between(a, b).max(1) as f64 }
}

/// Each task's progress (0–1) as stored in the snapshots (period closes): (end of the snapshot
/// day, share done), oldest first. Empty for a task no snapshot recorded.
pub(crate) fn history(c: &Calc) -> Vec<Vec<(i64, f64)>> {
    let p = c.p;
    let mut h = vec![vec![]; p.tasks.len()];
    let dd = c.progress_at();
    let mut snaps: Vec<&crate::model::Snapshot> = p.snapshots.iter().filter(|s| s.date.is_some_and(|d| crate::cal::at(d) + DAY <= dd)).collect();
    snaps.sort_by_key(|s| s.date);
    for sn in snaps {
        let t = crate::cal::at(sn.date.unwrap_or_default()) + DAY;
        for r in &sn.rows {
            if let Some(i) = p.idx(r.uid) { h[i].push((t, r.pct as f64 / 100.0)) }
        }
    }
    h
}

/// Share done at `t` from the recorded points: straight lines between the actual start, each
/// stored period and the status date.
fn along(points: &[(i64, f64)], t: i64) -> f64 {
    match points.iter().position(|&(x, _)| x >= t) {
        None => points.last().map_or(0.0, |p| p.1),
        Some(0) => if t >= points[0].0 { points[0].1 } else { 0.0 },
        Some(k) => { let ((a, u), (b, v)) = (points[k - 1], points[k]); if b > a { u + (v - u) * (t - a) as f64 / (b - a) as f64 } else { v } }
    }
}

/// Progress of task `i` (0–1) at instant `t`: planned (baseline, or current dates without one),
/// actual (through the stored periods in `h`, else linear from the actual start to today's %, up
/// to the status date) and forecast.
pub(crate) fn progress_at(c: &Calc, i: usize, t: i64, h: &[Vec<(i64, f64)>]) -> (f64, f64, f64) {
    let (p, s) = (c.p, &c.s);
    let task = &p.tasks[i];
    let cal = &s.cals[s.ci[i]];
    let (bs, bf) = task.bls.get(&0).map_or((s.es[i], s.ef[i]), |b| (b.start, b.finish));
    let planned = share(cal, bs, bf, t);
    let dd = c.progress_at();
    let done = if task.pct_type == crate::model::PctType::Physical { task.phys } else { c.pct[i] } as f64 / 100.0;
    let a0 = task.act_start.map_or(s.es[i], crate::cal::at);
    let a1 = task.act_finish.map_or(dd, |d| crate::cal::at(d) + DAY).max(a0 + 1);
    let actual = if done == 0.0 && h[i].iter().all(|x| x.1 == 0.0) { 0.0 } else if h[i].is_empty() { done * share(cal, a0, a1, t.min(dd)) } else {
        let mut pts: Vec<(i64, f64)> = std::iter::once((a0.min(h[i][0].0), 0.0)).chain(h[i].iter().copied().filter(|x| x.0 < dd)).chain([(dd, done)]).collect();
        pts.dedup_by_key(|x| x.0);
        along(&pts, t.min(dd))
    };
    let forecast = if t <= dd { actual } else { done + (1.0 - done) * share(cal, dd.max(s.es[i]), s.ef[i].max(dd.max(s.es[i]) + 1), t) };
    (planned, actual, forecast)
}

#[derive(Serialize)]
pub struct SCurve {
    /// Bucket end days (days since 1970-01-01), and the cumulative % at each.
    pub days: Vec<i64>,
    pub planned: Vec<f64>,
    pub actual: Vec<Option<f64>>,
    pub forecast: Vec<f64>,
    /// The total weight (hours, money or days) that 100% stands for.
    pub total: f64,
    pub status_day: i64,
    pub name: String,
    /// The weighting used: duration, cost, work or physical.
    pub weight: String,
}

/// Cumulative progress S-curve for the project or one summary (`scope` uid), weighted by
/// `how` (duration, cost, work, physical), sampled at period ends ("week", "month") or days.
pub fn scurve(c: &Calc, unit: &str, how: &str, scope: Option<u32>) -> SCurve {
    let p = c.p;
    let (w, weight) = weights(c, how);
    let tasks: Vec<usize> = match scope.and_then(|u| p.idx(u)) {
        Some(k) => p.subtree(k).filter(|&i| w[i] > 0.0).collect(),
        None => (0..p.tasks.len()).filter(|&i| w[i] > 0.0).collect(),
    };
    let total: f64 = tasks.iter().map(|&i| w[i]).sum::<f64>().max(1e-9);
    // Daily points only for plans up to about ten years; longer ones use weeks.
    let fin = tasks.iter().map(|&i| p.tasks[i].bls.get(&0).map_or(c.s.ef[i], |b| b.finish.max(c.s.ef[i]))).max().unwrap_or(c.s.finish).max(c.s.finish).div_euclid(DAY);
    let long = fin - c.s.start.div_euclid(DAY) > 3660;
    let mut days = if unit == "day" && !long { (c.s.start.div_euclid(DAY)..=fin).collect() } else { period_ends(c, if unit == "day" { "week" } else { unit }) };
    let lo = tasks.iter().map(|&i| p.tasks[i].bls.get(&0).map_or(c.s.es[i], |b| b.start.min(c.s.es[i]))).min().unwrap_or(c.s.start).div_euclid(DAY);
    days.retain(|&d| d >= lo - 7);
    days.insert(0, lo - 1);
    let dd = c.progress_at();
    let h = history(c);
    let mut out = SCurve { days: vec![], planned: vec![], actual: vec![], forecast: vec![], total, status_day: dd / DAY - 1, name: scope.and_then(|u| p.idx(u)).map_or(p.name.clone(), |k| p.tasks[k].name.clone()), weight };
    for d in days {
        let t = (d + 1) * DAY;
        let (mut a, mut b, mut f) = (0.0, 0.0, 0.0);
        for &i in &tasks {
            let (x, y, z) = progress_at(c, i, t, &h);
            a += w[i] * x;
            b += w[i] * y;
            f += w[i] * z;
        }
        out.days.push(d);
        out.planned.push(a / total * 100.0);
        out.actual.push((t <= dd + DAY).then_some(b / total * 100.0));
        out.forecast.push(f / total * 100.0);
    }
    out
}

#[derive(Serialize, Default)]
pub struct PRow { pub uid: u32, pub code: String, pub name: String, pub start: String, pub finish: String, pub pct: u8, pub note: String }

#[derive(Serialize, Default)]
pub struct WbsRow { pub uid: u32, pub name: String, pub planned_prev: f64, pub planned: f64, pub actual_prev: f64, pub actual: f64 }

#[derive(Serialize, Default)]
pub struct PeriodReport {
    pub kind: String,
    /// This period and the previous one: first and last day (days since 1970-01-01).
    pub from: i64,
    pub to: i64,
    pub prev_from: i64,
    /// Cumulative % (weighted) at the end of the previous period and of this one.
    pub planned_prev: f64,
    pub planned: f64,
    pub actual_prev: f64,
    pub actual: f64,
    pub finish: String,
    pub baseline_finish: String,
    pub finish_var: String,
    pub spi: Option<f64>,
    pub cpi: Option<f64>,
    pub cost: String,
    pub critical: usize,
    pub late: usize,
    pub completed: Vec<PRow>,
    pub started: Vec<PRow>,
    pub in_progress: Vec<PRow>,
    pub slipped: Vec<PRow>,
    pub upcoming: Vec<PRow>,
    pub milestones: Vec<PRow>,
    pub wbs: Vec<WbsRow>,
    /// Hours per period: planned (current schedule) by period end.
    pub hours: Vec<(i64, f64)>,
}

/// The progress report for the period ending on day `end` (or the latest period at the status date).
pub fn period_report(c: &Calc, kind: &str, end: Option<i64>) -> PeriodReport {
    let (p, s) = (c.p, &c.s);
    let ends = period_ends(c, kind);
    let dd = c.progress_at() / DAY - 1;
    let to = end.unwrap_or_else(|| ends.iter().copied().filter(|&e| e >= dd).min().unwrap_or(dd));
    let k = ends.iter().position(|&e| e == to);
    let prev = k.and_then(|k| k.checked_sub(1)).map_or(to - if kind == "month" { 30 } else { 7 }, |k| ends[k]);
    let before = k.and_then(|k| k.checked_sub(2)).map_or(prev - (to - prev), |k| ends[k]);
    let (w, _) = weights(c, &p.periods.weight);
    let total: f64 = w.iter().sum::<f64>().max(1e-9);
    let h = history(c);
    let at = |d: i64, which: usize| (0..p.tasks.len()).filter(|&i| w[i] > 0.0).map(|i| { let v = progress_at(c, i, (d + 1) * DAY, &h); w[i] * [v.0, v.1, v.2][which] }).sum::<f64>() / total * 100.0;
    let row = |i: usize, note: &str| PRow {
        uid: p.tasks[i].uid, code: p.tasks[i].code.clone(), name: p.tasks[i].name.clone(), start: c.text(i, "start"), finish: c.text(i, "finish"), pct: c.pct[i], note: note.into(),
    };
    let leaves: Vec<usize> = (0..p.tasks.len()).filter(|&i| !s.summary[i] && !s.off[i]).collect();
    let within = |d: Option<chrono::NaiveDate>, a: i64, b: i64| d.is_some_and(|d| { let x = crate::cal::at(d) / DAY; x > a && x <= b });
    let day = |t: i64| t.div_euclid(DAY);
    let next = to + (to - prev);
    let mut r = PeriodReport {
        kind: kind.into(), from: prev + 1, to, prev_from: before + 1,
        planned_prev: at(prev, 0), planned: at(to, 0), actual_prev: at(prev, 1), actual: at(to.min(dd), 1),
        ..Default::default()
    };
    let sum = c.summary();
    r.finish = sum.finish.clone();
    r.baseline_finish = sum.bl_finish.clone();
    r.finish_var = sum.var_finish.clone();
    (r.spi, r.cpi, r.cost, r.critical, r.late) = (sum.spi, sum.cpi, sum.cost.clone(), sum.critical, sum.late);
    for &i in &leaves {
        let t = &p.tasks[i];
        if within(t.act_finish, prev, to) {
            r.completed.push(row(i, ""));
        } else if within(t.act_start, prev, to) {
            r.started.push(row(i, ""));
        } else if c.pct[i] > 0 && c.pct[i] < 100 {
            r.in_progress.push(row(i, ""));
        }
        let bf = t.bls.get(&0).map(|b| day(b.finish - 1));
        if c.pct[i] < 100 && bf.is_some_and(|f| f <= to) {
            r.slipped.push(row(i, &format!("Baseline finish {}", c.text(i, "b0finish"))));
        }
        if c.pct[i] < 100 && day(s.es[i]) > to.min(dd) && day(s.es[i]) <= next {
            r.upcoming.push(row(i, ""));
        }
        if t.dur == 0 {
            r.milestones.push(row(i, &c.text(i, "vfinish")));
        }
    }
    for i in (0..p.tasks.len()).filter(|&i| s.summary[i] && p.tasks[i].level == 0) {
        let sub: Vec<usize> = p.subtree(i).filter(|&j| w[j] > 0.0).collect();
        let tw: f64 = sub.iter().map(|&j| w[j]).sum::<f64>().max(1e-9);
        let pa = |d: i64, which: usize| sub.iter().map(|&j| { let v = progress_at(c, j, (d + 1) * DAY, &h); w[j] * [v.0, v.1][which] }).sum::<f64>() / tw * 100.0;
        r.wbs.push(WbsRow { uid: p.tasks[i].uid, name: p.tasks[i].name.clone(), planned_prev: pa(prev, 0), planned: pa(to, 0), actual_prev: pa(prev, 1), actual: pa(to.min(dd), 1) });
    }
    let mut last = i64::MIN;
    for &e in &ends {
        let h: f64 = c.l.task_daily.iter().filter(|((_, d), _)| *d > last && *d <= e).map(|(_, (m, _))| m / 60.0).sum();
        r.hours.push((e, h));
        last = e;
    }
    r
}
