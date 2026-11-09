//! Check Schedule: the DCMA 14-point assessment plus further schedule checks, each with its
//! threshold and the tasks behind it.
use crate::cal::{at, DAY};
use crate::fields::Calc;
use crate::model::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Limits {
    /// Percent of tasks allowed to fail a ratio check.
    pub pct: f64,
    /// Float, duration and lag considered large, in working days.
    pub high_float: f64,
    pub high_dur: f64,
    pub long_lag: f64,
    /// Minimum index (BEI, CPLI).
    pub index: f64,
}

impl Default for Limits {
    fn default() -> Self { Self { pct: 5.0, high_float: 44.0, high_dur: 44.0, long_lag: 20.0, index: 0.95 } }
}

#[derive(Serialize)]
pub struct Check {
    pub key: &'static str,
    pub name: &'static str,
    pub about: &'static str,
    pub count: usize,
    pub total: usize,
    /// The measured value: a percent of tasks, or an index.
    pub value: f64,
    pub limit: String,
    pub pass: bool,
    pub uids: Vec<u32>,
}

pub fn run(c: &Calc, lim: &Limits) -> Vec<Check> {
    let (p, s) = (c.p, &c.s);
    let n = p.tasks.len();
    let day = p.day_min as f64;
    let dd = c.status_at();
    // Incomplete, active, normal tasks are what the DCMA counts.
    let all: Vec<usize> = (0..n).filter(|&i| !s.summary[i] && !s.off[i] && !matches!(p.tasks[i].act_type, ActType::Loe | ActType::WbsSummary)).collect();
    let open: Vec<usize> = all.iter().copied().filter(|&i| c.pct[i] < 100).collect();
    let (mut has_succ, mut has_pred) = (vec![false; n], vec![false; n]);
    for e in &s.edges {
        has_succ[e.p] = true;
        has_pred[e.s] = true;
    }
    let links: Vec<&crate::sched::Edge> = s.edges.iter().filter(|e| c.pct[e.s] < 100).collect();
    let uid = |v: &[usize]| v.iter().map(|&i| p.tasks[i].uid).collect::<Vec<u32>>();
    let mut out = vec![];
    let first = s.start;
    let last = s.finish;
    let ends = |i: usize| (!has_pred[i] && s.es[i] > first) || (!has_succ[i] && s.ef[i] < last);
    ratio(&mut out, p, "logic", "1. Missing logic", "Incomplete tasks without a predecessor or a successor (the project start and finish excepted).", &open,
        open.iter().copied().filter(|&i| ends(i)).collect(), lim.pct);
    let lead_tasks: Vec<usize> = links.iter().filter(|e| e.lag < 0).map(|e| e.s).collect();
    ratio(&mut out, p, "leads", "2. Leads", "Links with a negative lag. None are allowed.", &open, dedup(lead_tasks), 0.0);
    ratio(&mut out, p, "lags", "3. Lags", "Links with a positive lag.", &open, dedup(links.iter().filter(|e| e.lag > 0).map(|e| e.s).collect()), lim.pct);
    let fs = links.iter().filter(|e| e.kind == LinkType::FS).count();
    let non_fs = dedup(links.iter().filter(|e| e.kind != LinkType::FS).map(|e| e.s).collect());
    let v = if links.is_empty() { 100.0 } else { fs as f64 * 100.0 / links.len() as f64 };
    out.push(Check { key: "rel", name: "4. Relationship types", about: "Finish-to-start links as a share of all links. At least 90% should be FS.", count: links.len() - fs, total: links.len(), value: v, limit: "≥ 90% FS".into(), pass: v >= 90.0, uids: uid(&non_fs) });
    ratio(&mut out, p, "hard", "5. Hard constraints", "Must Start On, Must Finish On, Start No Later Than and Finish No Later Than.", &open,
        open.iter().copied().filter(|&i| matches!(p.tasks[i].constraint, Constraint::MSO | Constraint::MFO | Constraint::SNLT | Constraint::FNLT)).collect(), lim.pct);
    ratio(&mut out, p, "float", "6. High float", "Total float above the limit.", &open, open.iter().copied().filter(|&i| s.tf[i] as f64 > lim.high_float * day).collect(), lim.pct);
    ratio(&mut out, p, "negative", "7. Negative float", "Total float below zero. None are allowed.", &open, open.iter().copied().filter(|&i| s.tf[i] < 0).collect(), 0.0);
    ratio(&mut out, p, "duration", "8. High duration", "Remaining durations above the limit.", &open, open.iter().copied().filter(|&i| p.tasks[i].dur as f64 * (100 - c.pct[i]) as f64 / 100.0 > lim.high_dur * day).collect(), lim.pct);
    let invalid: Vec<usize> = all.iter().copied().filter(|&i| {
        let t = &p.tasks[i];
        let future = |d: Option<chrono::NaiveDate>| d.is_some_and(|d| at(d) >= dd);
        future(t.act_start) || future(t.act_finish) || (p.status.is_some() && !p.opts.msp_status && c.pct[i] == 0 && t.act_start.is_none() && !t.manual && s.ef[i] <= dd - DAY)
    }).collect();
    ratio(&mut out, p, "dates", "9. Invalid dates", if p.opts.msp_status { "Actual dates after the status date." } else { "Actual dates after the status date, or unstarted work left before it." }, &all, invalid, 0.0);
    ratio(&mut out, p, "resources", "10. Resources", "Tasks with duration but no resource or cost assigned.", &open,
        open.iter().copied().filter(|&i| p.tasks[i].dur > 0 && p.tasks[i].res.is_empty() && p.tasks[i].fixed_cost == 0.0).collect(), lim.pct);
    let base: Vec<usize> = all.iter().copied().filter(|&i| p.tasks[i].bls.get(&0).is_some_and(|b| b.finish <= dd)).collect();
    ratio(&mut out, p, "missed", "11. Missed tasks", "Tasks due by the status date in the baseline that finished late or are not finished.", &base,
        base.iter().copied().filter(|&i| c.pct[i] < 100 || s.ef[i] > p.tasks[i].bls[&0].finish).collect(), lim.pct);
    // 12: the critical path test. Adding a delay to the critical path must move the finish.
    let crit: Vec<usize> = open.iter().copied().filter(|&i| s.critical(p, i)).collect();
    let cp_ok = crit.first().is_none_or(|&i| {
        let mut q = p.clone();
        q.tasks[i].dur += 600 * p.day_min;
        crate::sched::schedule(&q).is_ok_and(|s2| s2.finish > s.finish)
    });
    out.push(Check { key: "cpt", name: "12. Critical path test", about: "A 600-day delay on the first critical task must push the project finish by the same amount.", count: usize::from(!cp_ok), total: 1, value: if cp_ok { 100.0 } else { 0.0 }, limit: "Pass".into(), pass: cp_ok, uids: crit.first().map(|&i| vec![p.tasks[i].uid]).unwrap_or_default() });
    // 13: CPLI = (critical path length + total float) / critical path length.
    let pc = &s.cals[p.calendars.iter().position(|c| c.name == p.calendar).unwrap_or(0)];
    let cpl = pc.between(dd.max(s.start), s.finish) as f64;
    let ptf = crit.iter().map(|&i| s.tf[i]).min().unwrap_or(0) as f64;
    let cpli = if cpl > 0.0 { (cpl + ptf) / cpl } else { 1.0 };
    out.push(Check { key: "cpli", name: "13. CPLI", about: "Critical path length index: (remaining critical path + total float) ÷ remaining critical path.", count: 0, total: 0, value: cpli, limit: format!("≥ {}", lim.index), pass: cpli >= lim.index, uids: vec![] });
    // 14: BEI = tasks finished ÷ tasks the baseline says should have finished.
    let due = base.len();
    let done = all.iter().filter(|&&i| c.pct[i] == 100).count();
    let bei = if due > 0 { done as f64 / due as f64 } else { 1.0 };
    out.push(Check { key: "bei", name: "14. BEI", about: "Baseline execution index: tasks finished ÷ tasks due by the status date in the baseline.", count: done, total: due, value: bei, limit: format!("≥ {}", lim.index), pass: bei >= lim.index, uids: vec![] });
    // Checks that the DCMA list does not cover. An elapsed lag counts as its share of a working week (7 elapsed days are 5 days).
    ratio(&mut out, p, "longlag", "Long lags", "Lags longer than the limit.", &open, dedup(links.iter().filter(|e| e.lag as f64 * if e.el { 5.0 * day / (7.0 * 1440.0) } else { 1.0 } > lim.long_lag * day).map(|e| e.s).collect()), 0.0);
    ratio(&mut out, p, "sf", "Start-to-finish links", "SF links are rarely right.", &open, dedup(links.iter().filter(|e| e.kind == LinkType::SF).map(|e| e.s).collect()), 0.0);
    ratio(&mut out, p, "soft", "Soft constraints", "Start/Finish No Earlier Than and As Late As Possible.", &open,
        open.iter().copied().filter(|&i| matches!(p.tasks[i].constraint, Constraint::SNET | Constraint::FNET | Constraint::ALAP)).collect(), lim.pct);
    ratio(&mut out, p, "manual", "Manually scheduled", "Tasks that the scheduler does not move.", &open, open.iter().copied().filter(|&i| p.tasks[i].manual).collect(), 0.0);
    ratio(&mut out, p, "sumlinks", "Summary links", "Links on summary tasks hide the real logic.", &all,
        (0..n).filter(|&i| s.summary[i] && !p.tasks[i].preds.is_empty()).collect(), 0.0);
    ratio(&mut out, p, "oos", "Out of sequence", "Started before a finish-to-start predecessor finished.", &all,
        dedup(s.edges.iter().filter(|e| e.kind == LinkType::FS && c.pct[e.s] > 0 && c.pct[e.p] < 100).map(|e| e.s).collect()), 0.0);
    ratio(&mut out, p, "deadline", "Missed deadlines", "Tasks forecast to finish after their deadline.", &all,
        all.iter().copied().filter(|&i| p.tasks[i].deadline.is_some_and(|d| s.ef[i] > at(d) + DAY)).collect(), 0.0);
    out
}

#[allow(clippy::too_many_arguments)]
fn ratio(out: &mut Vec<Check>, p: &Project, key: &'static str, name: &'static str, about: &'static str, base: &[usize], bad: Vec<usize>, limit: f64) {
    let value = if base.is_empty() { 0.0 } else { bad.len() as f64 * 100.0 / base.len() as f64 };
    let uids = bad.iter().map(|&i| p.tasks[i].uid).collect();
    out.push(Check { key, name, about, count: bad.len(), total: base.len(), value, limit: format!("≤ {limit}%"), pass: value <= limit + 1e-9, uids });
}

fn dedup(mut v: Vec<usize>) -> Vec<usize> {
    v.sort_unstable();
    v.dedup();
    v
}
