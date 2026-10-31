//! Schedule risk analysis (Monte Carlo): each run draws every task's duration from its range and
//! lets each open risk happen with its probability, then reschedules the plan with the full engine.
use crate::{cal::DAY, load, model::*, sched::schedule};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct SimOpts {
    pub iterations: u32,
    /// Range for tasks without their own: −low % to +high % of the duration.
    pub low: f64,
    pub high: f64,
    pub seed: u64,
}
impl Default for SimOpts {
    fn default() -> Self { Self { iterations: 1000, low: 10.0, high: 25.0, seed: 1 } }
}

#[derive(Serialize, Default, Debug)]
pub struct TaskRisk { pub uid: u32, pub name: String, pub value: f64 }

#[derive(Serialize, Default, Debug)]
pub struct SimResult {
    pub runs: usize,
    /// The plan's own finish (instant) and the chance of finishing by then.
    pub planned: i64,
    pub planned_chance: f64,
    /// Finish at P10, P50, P80, P90 (instants).
    pub p10: i64,
    pub p50: i64,
    pub p80: i64,
    pub p90: i64,
    /// Finishing day (days since 1970) and how many runs finished that day.
    pub days: Vec<(i64, usize)>,
    pub cost: f64,
    pub cost_p50: f64,
    pub cost_p80: f64,
    /// Share of runs (0–1) in which each task was critical, most critical first.
    pub criticality: Vec<TaskRisk>,
    /// Correlation of each task's duration with the finish, strongest first (the tornado).
    pub drivers: Vec<TaskRisk>,
    /// Risk register entries that were simulated: how often each happened.
    pub risks: Vec<TaskRisk>,
    pub notes: Vec<String>,
}

struct Rng(u64);
impl Rng {
    fn f(&mut self) -> f64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; (self.0 >> 11) as f64 / (1u64 << 53) as f64 }
}

/// A triangular draw between `a` and `b` with its peak at `m`.
fn tri(r: f64, a: f64, m: f64, b: f64) -> f64 {
    if b <= a { return m }
    let c = (m - a) / (b - a);
    if r < c { a + ((b - a) * (m - a) * r).sqrt() } else { b - ((b - a) * (b - m) * (1.0 - r)).sqrt() }
}

/// Probability of a risk scored 1–5.
pub fn chance(prob: u8) -> f64 { [0.0, 0.1, 0.3, 0.5, 0.7, 0.9][prob.min(5) as usize] }

fn pct(sorted: &[i64], q: f64) -> i64 { sorted[((sorted.len() - 1) as f64 * q).round() as usize] }

pub fn simulate(p: &Project, o: &SimOpts) -> Result<SimResult, String> {
    let base = schedule(p)?;
    let n = p.tasks.len();
    let cost0 = load::compute(p, &base).cost;
    let leaves: Vec<usize> = (0..n).filter(|&i| !base.summary[i] && !base.off[i] && p.tasks[i].act_finish.is_none()).collect();
    let range = |i: usize| {
        let t = &p.tasks[i];
        let d = t.dur as f64;
        let lo = if t.dmin > 0 { (t.dmin as f64).min(d) } else { d * (1.0 - o.low.clamp(0.0, 100.0) / 100.0) };
        let hi = if t.dmax > 0 { (t.dmax as f64).max(d) } else { d * (1.0 + o.high.max(0.0) / 100.0) };
        (lo, d, hi)
    };
    let risks: Vec<&Risk> = p.risks.iter().filter(|r| r.status != "Closed" && r.days != 0.0 && !r.tasks.is_empty()).collect();
    let runs = o.iterations.clamp(10, 10_000) as usize;
    let mut q = p.clone();
    let mut rng = Rng(o.seed.max(1).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let (mut fin, mut costs) = (Vec::with_capacity(runs), Vec::with_capacity(runs));
    let mut durs = vec![Vec::with_capacity(runs); leaves.len()];
    let (mut crit, mut hits) = (vec![0usize; n], vec![0usize; risks.len()]);
    let mut failed = 0;
    for _ in 0..runs {
        let mut add = vec![0i64; n];
        for (k, r) in risks.iter().enumerate() {
            if rng.f() < chance(r.prob) {
                hits[k] += 1;
                let m = (r.days.abs() * p.day_min as f64).round() as i64 * if r.kind == "Opportunity" { -1 } else { 1 };
                // A risk on a phase delays its last task.
                r.tasks.iter().filter_map(|u| p.idx(*u)).map(|i| if base.summary[i] { p.subtree(i).filter(|&j| !base.summary[j]).max_by_key(|&j| base.ef[j]).unwrap_or(i) } else { i }).for_each(|i| add[i] += m);
            }
        }
        let mut cost = 0.0;
        let mut drawn = Vec::with_capacity(leaves.len());
        for &i in &leaves {
            let (a, m, b) = range(i);
            let d = ((tri(rng.f(), a, m, b).round() as i64) + add[i]).max(0);
            q.tasks[i].dur = d;
            drawn.push(d as f64);
            let fixed = p.tasks[i].fixed_cost;
            cost += fixed + (cost0[i] - fixed) * if m > 0.0 { d as f64 / m } else { 1.0 };
        }
        let Ok(s) = schedule(&q) else { failed += 1; continue };
        durs.iter_mut().zip(drawn).for_each(|(v, d)| v.push(d));
        fin.push(s.finish);
        costs.push(cost + (0..n).filter(|i| !leaves.contains(i) && !base.summary[*i]).map(|i| cost0[i]).sum::<f64>());
        (0..n).filter(|&i| s.critical(&q, i) && !s.summary[i]).for_each(|i| crit[i] += 1);
    }
    if fin.is_empty() {
        return Err("No run could be scheduled. Check the plan for errors first.".into());
    }
    let mut res = SimResult { runs: fin.len(), planned: base.finish, notes: vec![], ..Default::default() };
    if failed > 0 { res.notes.push(format!("{failed} runs could not be scheduled and were left out.")) }
    if leaves.iter().all(|&i| p.tasks[i].dur == 0) { res.notes.push("No unfinished tasks with a duration: nothing to vary.".into()) }
    let ranged = leaves.iter().filter(|&&i| p.tasks[i].dmin > 0 || p.tasks[i].dmax > 0).count();
    res.notes.push(format!("{ranged} of {} tasks have their own optimistic and pessimistic durations; the rest use −{}% to +{}%.", leaves.len(), o.low, o.high));
    // Drivers: correlation of each task's duration with the finish.
    let fx: Vec<f64> = fin.iter().map(|&f| f as f64).collect();
    let corr = |v: &[f64]| {
        let (mx, my) = (v.iter().sum::<f64>() / v.len() as f64, fx.iter().sum::<f64>() / fx.len() as f64);
        let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
        for (x, y) in v.iter().zip(&fx) { sxy += (x - mx) * (y - my); sxx += (x - mx).powi(2); syy += (y - my).powi(2) }
        if sxx > 0.0 && syy > 0.0 { sxy / (sxx * syy).sqrt() } else { 0.0 }
    };
    res.drivers = leaves.iter().zip(&durs).map(|(&i, v)| TaskRisk { uid: p.tasks[i].uid, name: p.tasks[i].name.clone(), value: corr(v) }).filter(|t| t.value.abs() > 0.05).collect();
    res.drivers.sort_by(|a, b| b.value.abs().total_cmp(&a.value.abs()));
    res.drivers.truncate(10);
    let runs_f = res.runs as f64;
    res.criticality = (0..n).filter(|&i| crit[i] > 0).map(|i| TaskRisk { uid: p.tasks[i].uid, name: p.tasks[i].name.clone(), value: crit[i] as f64 / runs_f }).collect();
    res.criticality.sort_by(|a, b| b.value.total_cmp(&a.value));
    res.criticality.truncate(15);
    res.risks = risks.iter().zip(&hits).map(|(r, &h)| TaskRisk { uid: r.id, name: r.name.clone(), value: h as f64 / runs as f64 }).collect();
    res.planned_chance = fin.iter().filter(|&&f| f <= base.finish).count() as f64 / runs_f;
    fin.sort_unstable();
    (res.p10, res.p50, res.p80, res.p90) = (pct(&fin, 0.1), pct(&fin, 0.5), pct(&fin, 0.8), pct(&fin, 0.9));
    for &f in &fin {
        let d = (f - 1).div_euclid(DAY);
        match res.days.last_mut() { Some((x, c)) if *x == d => *c += 1, _ => res.days.push((d, 1)) }
    }
    costs.sort_by(f64::total_cmp);
    res.cost = (0..n).filter(|&i| !base.summary[i] && !base.off[i]).map(|i| cost0[i]).sum();
    let cp = |q: f64| costs[((costs.len() - 1) as f64 * q).round() as usize];
    (res.cost_p50, res.cost_p80) = (cp(0.5), cp(0.8));
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cal::at, parse};
    use chrono::NaiveDate;

    fn plan(rows: &[(i64, &str)]) -> Project {
        let mut p = Project::new("R", NaiveDate::from_ymd_opt(2026, 11, 2).unwrap_or_default());
        for (i, &(d, _)) in rows.iter().enumerate() {
            let t = p.new_task(&format!("T{}", i + 1), 0, d * 480);
            p.tasks.push(t);
        }
        let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
        for (t, &(_, pr)) in p.tasks.iter_mut().zip(rows) { t.preds = parse::preds(pr, &ids, 480).unwrap_or_default() }
        p
    }
    fn day(y: u32, m: u32, d: u32) -> i64 { at(NaiveDate::from_ymd_opt(y as i32, m, d).unwrap_or_default()) }
    const NO_RANGE: SimOpts = SimOpts { iterations: 200, low: 0.0, high: 0.0, seed: 1 };

    #[test]
    fn without_uncertainty_every_run_is_the_plan() {
        let p = plan(&[(5, ""), (3, "1")]);
        let r = simulate(&p, &NO_RANGE).unwrap_or_default();
        assert_eq!((r.p10, r.p50, r.p90, r.planned_chance), (r.planned, r.planned, r.planned, 1.0));
        assert_eq!(r.planned, day(2026, 11, 11) + 17 * 60);
        assert!(r.drivers.is_empty());
        assert_eq!(r.criticality.iter().map(|t| t.value).collect::<Vec<_>>(), [1.0, 1.0]);
    }

    #[test]
    fn ranges_skew_the_finish_and_the_long_task_drives_it() {
        let p = plan(&[(10, ""), (2, ""), (3, "1")]);
        let r = simulate(&p, &SimOpts::default()).unwrap_or_default();
        assert!(r.p10 <= r.p50 && r.p50 <= r.p80 && r.p80 <= r.p90);
        assert!(r.p50 > r.planned && r.planned_chance < 0.5, "a range skewed late (−10 %, +25 %) makes the plan's own date unlikely: {}", r.planned_chance);
        assert_eq!(r.drivers[0].name, "T1");
        assert!(r.drivers[0].value > 0.8);
        let c = |name: &str| r.criticality.iter().find(|t| t.name == name).map_or(0.0, |t| t.value);
        assert_eq!((c("T1"), c("T2")), (1.0, 0.0));
        assert_eq!(r.days.iter().map(|d| d.1).sum::<usize>(), r.runs);
        // The same seed gives the same answer.
        assert_eq!(simulate(&p, &SimOpts::default()).map(|x| x.p80).unwrap_or_default(), r.p80);
    }

    #[test]
    fn own_ranges_and_register_risks() {
        let mut p = plan(&[(10, "")]);
        (p.tasks[0].dmin, p.tasks[0].dmax) = (8 * 480, 20 * 480);
        let r = simulate(&p, &NO_RANGE).unwrap_or_default();
        assert!(r.p10 >= day(2026, 11, 11) && r.p90 <= day(2026, 11, 27) + DAY, "{} {}", r.p10, r.p90);
        // A risk scored 4 (70 %) adding 5 days: most runs finish a week late.
        let mut p = plan(&[(5, "")]);
        p.risks.push(Risk { id: 1, name: "Late permit".into(), prob: 4, impact: 3, status: "Open".into(), tasks: vec![p.tasks[0].uid], kind: "Threat".into(), days: 5.0, ..Default::default() });
        let r = simulate(&p, &NO_RANGE).unwrap_or_default();
        assert_eq!((r.p10, r.p50), (r.planned, day(2026, 11, 13) + 17 * 60));
        assert!((r.risks[0].value - 0.7).abs() < 0.07, "{}", r.risks[0].value);
        // An opportunity takes days off; a closed risk is left out.
        (p.risks[0].kind, p.risks[0].days) = ("Opportunity".into(), 2.0);
        assert_eq!(simulate(&p, &NO_RANGE).map(|x| x.p50).unwrap_or_default(), day(2026, 11, 4) + 17 * 60);
        p.risks[0].status = "Closed".into();
        assert!(simulate(&p, &NO_RANGE).map(|x| x.risks.is_empty()).unwrap_or(false));
    }

    #[test]
    fn cost_moves_with_duration() {
        let mut p = plan(&[(5, "")]);
        p.resources.push(Resource { uid: 1, name: "Fitter".into(), rate: 50.0, max: 1.0, ..Default::default() });
        p.tasks[0].res = vec![Assign { res: 1, units: 1.0, table: 0 }];
        p.tasks[0].fixed_cost = 1000.0;
        let r = simulate(&p, &SimOpts { iterations: 300, ..SimOpts::default() }).unwrap_or_default();
        assert_eq!(r.cost, 1000.0 + 40.0 * 50.0);
        assert!(r.cost_p50 > r.cost && r.cost_p80 >= r.cost_p50 && r.cost_p80 < 1000.0 + 50.0 * 40.0 * 1.25);
    }
}
