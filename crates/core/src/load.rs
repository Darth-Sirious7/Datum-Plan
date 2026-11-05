//! Resource loading and costs, day by day: work and cost per task, assignment and resource,
//! overallocation, and the timephased data behind usage views, cash flow and earned value.
use crate::cal::{date_of, DAY};
use crate::model::{Accrual, Project, ResKind};
use crate::sched::Sched;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct Loads {
    /// Work minutes, cost, actual work and actual cost per task (summaries roll up).
    pub work: Vec<f64>,
    pub cost: Vec<f64>,
    pub awork: Vec<f64>,
    pub acost: Vec<f64>,
    /// Per resource.
    pub res_work: Vec<f64>,
    pub res_cost: Vec<f64>,
    pub over: Vec<bool>,
    /// (resource, day) → working minutes assigned.
    pub daily: BTreeMap<(usize, i64), f64>,
    /// (resource, task, day) → working minutes of that assignment.
    pub assign_daily: BTreeMap<(usize, usize, i64), f64>,
    /// (task, day) → (work minutes, cost) for leaf tasks.
    pub task_daily: BTreeMap<(usize, i64), (f64, f64)>,
}

/// Spreads `amount` over the task's working days in proportion to working minutes.
fn spread(c: &crate::cal::WorkCal, es: i64, ef: i64, mut f: impl FnMut(i64, f64)) {
    let total = c.between(es, ef).max(1) as f64;
    if ef <= es {
        f(es.div_euclid(DAY), 1.0);
        return;
    }
    let mut d = es.div_euclid(DAY);
    while d * DAY < ef {
        let m = c.between(es.max(d * DAY), ef.min((d + 1) * DAY));
        if m > 0 {
            f(d, m as f64 / total);
        }
        d += 1;
    }
}

/// Spreads a task's amounts like `spread`, split at the status date when work is under way (`cut`
/// = the status date and where the remaining work starts): the share `done` over the actual start
/// up to the status date, the rest from the resume point to the finish.
fn phased(c: &crate::cal::WorkCal, es: i64, ef: i64, cut: Option<(i64, i64)>, done: f64, mut f: impl FnMut(i64, f64)) {
    let Some((dd, rs)) = cut else { return spread(c, es, ef, f) };
    if done > 0.0 {
        spread(c, es, dd.clamp(es, ef), |d, x| f(d, x * done));
    }
    spread(c, rs, ef, |d, x| f(d, x * (1.0 - done)));
}

pub fn compute(p: &Project, s: &Sched) -> Loads {
    let (n, m) = (p.tasks.len(), p.resources.len());
    let mut l = Loads { work: vec![0.0; n], cost: vec![0.0; n], awork: vec![0.0; n], acost: vec![0.0; n], res_work: vec![0.0; m], res_cost: vec![0.0; m], over: vec![false; m], ..Default::default() };
    for (i, t) in p.tasks.iter().enumerate().filter(|&(i, _)| !s.summary[i] && !s.off[i]) {
        let (c, es, ef) = (&s.cals[s.ci[i]], s.es[i], s.ef[i]);
        let done = t.pct as f64 / 100.0;
        let first = date_of(es);
        // Under way at the status date: work done before it, the rest after it.
        let dd = p.status.map(|d| crate::cal::at(d) + DAY);
        let cut = dd.filter(|_| t.act_start.is_some() && t.act_finish.is_none() && t.pct < 100).map(|dd| (dd, s.rs[i]));
        for a in &t.res {
            let Some(k) = p.res_idx(a.res) else { continue };
            let r = &p.resources[k];
            let (rate, per_use) = r.rate_on(a.table, first);
            let (w, cost) = match r.kind {
                ResKind::Work => (t.dur as f64 * a.units, 0.0),
                ResKind::Material => (0.0, a.units * rate),
                ResKind::Cost => (0.0, a.units),
            };
            let mut wcost = 0.0;
            phased(c, es, ef, cut, done, |d, share| {
                let mins = w * share;
                let day_cost = if r.kind == ResKind::Work { mins / 60.0 * r.rate_on(a.table, date_of(d * DAY)).0 } else { cost * share };
                wcost += day_cost;
                let e = l.task_daily.entry((i, d)).or_default();
                e.0 += mins;
                e.1 += day_cost;
                if mins > 0.0 {
                    *l.daily.entry((k, d)).or_default() += mins;
                    *l.assign_daily.entry((k, i, d)).or_default() += mins;
                }
            });
            l.task_daily.entry((i, es.div_euclid(DAY))).or_default().1 += per_use;
            let total = wcost + per_use;
            l.work[i] += w;
            l.cost[i] += total;
            l.awork[i] += w * done;
            l.acost[i] += total * done;
            l.res_work[k] += w;
            l.res_cost[k] += total;
        }
        // Fixed cost and expenses, charged by their accrual.
        let mut charge = |amount: f64, accrual: Accrual, actual: f64| {
            if amount == 0.0 && actual == 0.0 {
                return;
            }
            match accrual {
                Accrual::Start => l.task_daily.entry((i, es.div_euclid(DAY))).or_default().1 += amount,
                Accrual::End => l.task_daily.entry((i, (ef - 1).max(es).div_euclid(DAY))).or_default().1 += amount,
                Accrual::Prorated => phased(c, es, ef, cut, done, |d, share| l.task_daily.entry((i, d)).or_default().1 += amount * share),
            }
            l.cost[i] += amount;
            l.acost[i] += actual;
        };
        let fixed_actual = match t.accrual { Accrual::Start if t.pct > 0 => t.fixed_cost, Accrual::End if t.pct == 100 => t.fixed_cost, Accrual::Prorated => t.fixed_cost * done, _ => 0.0 };
        charge(t.fixed_cost, t.accrual, fixed_actual);
        for e in &t.expenses {
            charge(e.planned.max(e.actual), e.accrual, e.actual);
        }
    }
    for (&(k, day), &mins) in &l.daily {
        let d = date_of(day * DAY);
        let cap = p.resources[k].max_on(d) * s.cals[s.rcal[k]].day_minutes(d).max(0) as f64;
        if mins > cap + 0.5 {
            l.over[k] = true;
        }
    }
    // Money in whole minor units (cents): each task's cost is rounded once, so every total is
    // the exact sum of the amounts shown.
    let unit = 10f64.powi(p.currency.decimals.min(4) as i32);
    let minor = |v: f64| (v * unit).round() / unit;
    for i in (0..n).filter(|&i| !s.summary[i]) {
        (l.cost[i], l.acost[i]) = (minor(l.cost[i]), minor(l.acost[i]));
    }
    l.res_cost.iter_mut().for_each(|v| *v = minor(*v));
    for i in (0..n).filter(|&i| s.summary[i]) {
        for j in p.subtree(i).skip(1).filter(|&j| !s.summary[j]) {
            l.work[i] += l.work[j];
            l.cost[i] += l.cost[j];
            l.awork[i] += l.awork[j];
            l.acost[i] += l.acost[j];
        }
        (l.cost[i], l.acost[i]) = (minor(l.cost[i]), minor(l.acost[i]));
    }
    l
}

#[derive(Serialize)]
pub struct Workload {
    /// First day of each bucket (days since 1970-01-01).
    pub buckets: Vec<i64>,
    pub rows: Vec<WorkRow>,
}

#[derive(Serialize)]
pub struct WorkRow {
    pub uid: u32,
    pub name: String,
    /// Hours assigned and hours available per bucket.
    pub load: Vec<f64>,
    pub cap: Vec<f64>,
}

/// Bucket starts from `first` to `last` (days) for day, week (Monday), month or quarter.
pub fn buckets(first: i64, last: i64, unit: &str) -> Vec<i64> {
    use chrono::Datelike;
    let mut out = vec![];
    let mut d = match unit {
        "week" => first - (first + 3).rem_euclid(7),
        "month" | "quarter" => crate::cal::at(date_of(first * DAY).with_day(1).unwrap()) / DAY,
        _ => first,
    };
    if unit == "quarter" {
        let x = date_of(d * DAY);
        d = crate::cal::at(chrono::NaiveDate::from_ymd_opt(x.year(), (x.month0() / 3) * 3 + 1, 1).unwrap()) / DAY;
    }
    while d <= last && out.len() < 5000 {
        out.push(d);
        d = match unit {
            "week" => d + 7,
            "month" | "quarter" => {
                let x = date_of(d * DAY);
                let months = if unit == "month" { 1 } else { 3 };
                crate::cal::at(x.checked_add_months(chrono::Months::new(months)).unwrap()) / DAY
            }
            _ => d + 1,
        };
    }
    out
}

/// Hours per resource per week (or day) across the project.
pub fn workload(p: &Project, s: &Sched, weekly: bool) -> Workload {
    let l = compute(p, s);
    let b = buckets(s.start.div_euclid(DAY), s.finish.div_euclid(DAY), if weekly { "week" } else { "day" });
    let rows = p.resources.iter().enumerate().filter(|(_, r)| r.kind == ResKind::Work).map(|(k, r)| {
        let (mut load, mut cap) = (vec![0.0; b.len()], vec![0.0; b.len()]);
        for (j, &d0) in b.iter().enumerate() {
            let end = b.get(j + 1).copied().unwrap_or(d0 + if weekly { 7 } else { 1 });
            for d in d0..end {
                load[j] += l.daily.get(&(k, d)).copied().unwrap_or(0.0) / 60.0;
                cap[j] += r.max_on(date_of(d * DAY)) * s.cals[s.rcal[k]].day_minutes(date_of(d * DAY)) as f64 / 60.0;
            }
        }
        WorkRow { uid: r.uid, name: r.name.clone(), load, cap }
    }).collect();
    Workload { buckets: b, rows }
}
