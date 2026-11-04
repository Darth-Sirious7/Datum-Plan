//! Progress history: manpower planned against work done by trade, earned schedule, rules of
//! credit, and the look-ahead with its constraint log.
use crate::{cal::{date_of, DAY}, fields::Calc, load::buckets, model::*, report::{history, progress_at, share, weights}};
use serde::Serialize;
use std::collections::BTreeMap;

/// Built-in rules of credit for common kinds of work (weights in %).
pub fn credit_library() -> Vec<CreditRule> {
    let r = |name: &str, steps: &[(&str, f64)]| CreditRule { name: name.into(), steps: steps.iter().map(|(n, w)| (n.to_string(), *w)).collect() };
    vec![
        r("Concrete", &[("Formwork", 25.0), ("Reinforcement", 25.0), ("Pour", 40.0), ("Strip and cure", 10.0)]),
        r("Steel erection", &[("Delivered", 10.0), ("Erected", 50.0), ("Bolted and aligned", 30.0), ("Inspected", 10.0)]),
        r("Piping", &[("Spools received", 10.0), ("Erected", 40.0), ("Welded", 30.0), ("Tested", 15.0), ("Insulated", 5.0)]),
        r("Cable", &[("Pulled", 60.0), ("Terminated", 30.0), ("Tested", 10.0)]),
        r("Equipment", &[("Set", 40.0), ("Aligned", 25.0), ("Connected", 25.0), ("Commissioned", 10.0)]),
        r("Document", &[("Drafted", 50.0), ("Checked", 25.0), ("Approved and issued", 25.0)]),
        r("Software feature", &[("Designed", 20.0), ("Built", 50.0), ("Reviewed", 15.0), ("Tested", 15.0)]),
    ]
}

#[derive(Serialize, Debug, Default)]
pub struct Trade { pub name: String, pub planned: Vec<f64>, pub actual: Vec<Option<f64>>, pub forecast: Vec<Option<f64>> }

/// People per period by trade: planned (baseline, or current dates without one), work done
/// (earned hours from progress) and forecast for the rest.
#[derive(Serialize, Debug, Default)]
pub struct Manpower { pub buckets: Vec<i64>, pub unit: String, pub status_day: i64, pub trades: Vec<Trade> }

/// Minutes per period: planned, done, forecast.
type Series = (Vec<f64>, Vec<f64>, Vec<f64>);

pub fn manpower(c: &Calc, unit: &str) -> Manpower {
    let (p, s, l) = (c.p, &c.s, &c.l);
    let n = p.tasks.len();
    let bl = |i: usize| p.tasks[i].bls.get(&0).map_or((s.es[i], s.ef[i]), |b| (b.start, b.finish));
    let leaves: Vec<usize> = (0..n).filter(|&i| !s.summary[i] && !s.off[i]).collect();
    let lo = leaves.iter().map(|&i| bl(i).0.min(s.es[i])).min().unwrap_or(s.start).div_euclid(DAY);
    let hi = leaves.iter().map(|&i| bl(i).1.max(s.ef[i])).max().unwrap_or(s.finish).div_euclid(DAY);
    let b = buckets(lo, hi.max(lo), unit);
    let mut ends: Vec<i64> = b.iter().skip(1).copied().collect();
    ends.push(b.last().map_or(lo + 1, |&x| buckets(x, x + 400, unit).get(1).copied().unwrap_or(x + 1)));
    let pc = &s.cals[p.calendars.iter().position(|x| x.name == p.calendar).unwrap_or(0)];
    let person: Vec<f64> = b.iter().zip(&ends).map(|(&a, &e)| (a..e).map(|d| pc.day_minutes(date_of(d * DAY)) as f64).sum::<f64>().max(1.0)).collect();
    // Minutes of work per (task, trade).
    let trade = |k: usize| { let g = p.resources[k].group.trim(); if g.is_empty() { "No trade".to_string() } else { g.to_string() } };
    let mut work: BTreeMap<(usize, String), f64> = BTreeMap::new();
    for (&(k, i, _), &m) in &l.assign_daily {
        if p.resources[k].kind == ResKind::Work { *work.entry((i, trade(k))).or_default() += m }
    }
    let h = history(c);
    let dd = c.progress_at();
    let mut names: Vec<String> = work.keys().map(|x| x.1.clone()).collect();
    names.sort();
    names.dedup();
    names.sort_by_key(|x| x == "No trade");
    let nb = b.len();
    let mut rows: BTreeMap<String, Series> = names.iter().map(|x| (x.clone(), (vec![0.0; nb], vec![0.0; nb], vec![0.0; nb]))).collect();
    for ((i, t), &m) in &work {
        let (bs, bf) = bl(*i);
        let cal = &s.cals[s.ci[*i]];
        let at = |d: i64| progress_at(c, *i, d * DAY, &h);
        let row = rows.get_mut(t).expect("every trade has a row");
        for j in 0..nb {
            let (a, e) = (b[j], ends[j]);
            row.0[j] += m * (share(cal, bs, bf, e * DAY) - share(cal, bs, bf, a * DAY));
            let (x, y) = (at(a), at(e));
            row.1[j] += m * (y.1 - x.1);
            row.2[j] += m * (y.2 - x.2);
        }
    }
    let first_future = b.iter().position(|&d| d * DAY >= dd).unwrap_or(nb);
    let mk = |name: String, v: &Series| Trade {
        name,
        planned: (0..nb).map(|j| v.0[j] / person[j]).collect(),
        actual: (0..nb).map(|j| (j < first_future).then(|| v.1[j] / person[j])).collect(),
        forecast: (0..nb).map(|j| (j + 1 >= first_future).then(|| v.2[j] / person[j])).collect(),
    };
    let mut total = (vec![0.0; nb], vec![0.0; nb], vec![0.0; nb]);
    for v in rows.values() { for j in 0..nb { total.0[j] += v.0[j]; total.1[j] += v.1[j]; total.2[j] += v.2[j] } }
    let mut trades = vec![mk("All trades".into(), &total)];
    trades.extend(names.into_iter().map(|x| { let v = &rows[&x]; mk(x, v) }));
    Manpower { buckets: b, unit: unit.into(), status_day: dd / DAY - 1, trades }
}

/// Earned schedule: the time at which the plan expected the progress made so far.
#[derive(Serialize, Debug, Default, PartialEq)]
pub struct EarnedSchedule {
    /// In working days from the planned start: earned schedule, actual time, planned duration.
    pub es: f64,
    pub at: f64,
    pub pd: f64,
    /// SPI(t) = ES / AT; SV(t) = ES − AT (working days); IEAC(t) = PD / SPI(t).
    pub spi_t: f64,
    pub sv_t: f64,
    pub ieac: f64,
    /// Planned and forecast finish (instants).
    pub planned_finish: i64,
    pub forecast_finish: i64,
}

pub fn earned_schedule(c: &Calc) -> Option<EarnedSchedule> {
    let (p, s) = (c.p, &c.s);
    let (w, _) = weights(c, &p.periods.weight);
    let tasks: Vec<usize> = (0..p.tasks.len()).filter(|&i| w[i] > 0.0).collect();
    let total: f64 = tasks.iter().map(|&i| w[i]).sum::<f64>();
    if total <= 0.0 { return None }
    let h = history(c);
    let dd = c.progress_at();
    let pct = |t: i64, k: usize| tasks.iter().map(|&i| { let v = progress_at(c, i, t, &h); w[i] * if k == 0 { v.0 } else { v.1 } }).sum::<f64>() / total;
    let ev = pct(dd, 1);
    let bl = |i: usize| p.tasks[i].bls.get(&0).map_or((s.es[i], s.ef[i]), |b| (b.start, b.finish));
    let (start, fin) = (tasks.iter().map(|&i| bl(i).0).min()?, tasks.iter().map(|&i| bl(i).1).max()?);
    if ev <= 0.0 || dd <= start { return None }
    let pc = &s.cals[p.calendars.iter().position(|x| x.name == p.calendar).unwrap_or(0)];
    // The day the planned curve reaches the progress made, interpolated over that day's working time.
    let (mut d, mut prev) = (start.div_euclid(DAY), 0.0);
    let es_t = loop {
        let v = pct((d + 1) * DAY, 0);
        if v >= ev - 1e-9 || (d + 1) * DAY >= fin {
            let frac = ((ev - prev) / (v - prev).max(1e-12)).clamp(0.0, 1.0);
            let work = pc.between(d * DAY, (d + 1) * DAY);
            break if work > 0 { pc.add(pc.next_start(d * DAY), (frac * work as f64) as i64) } else { d * DAY + (frac * DAY as f64) as i64 };
        }
        (prev, d) = (v, d + 1);
    };
    let days = |a: i64, b: i64| pc.between(a, b) as f64 / p.day_min.max(1) as f64;
    // Actual time stops when all the work is done (a plan finished on time stays on time).
    let end = if ev >= 1.0 - 1e-9 { tasks.iter().map(|&i| s.ef[i]).max().unwrap_or(dd).min(dd) } else { dd };
    let (es, at, pd) = (days(start, es_t.max(start)), days(start, end), days(start, fin));
    if at <= 0.0 { return None }
    let spi_t = es / at;
    let ieac = if spi_t > 0.0 { pd / spi_t } else { f64::INFINITY };
    let forecast_finish = if ieac.is_finite() { pc.add(start, (ieac * p.day_min as f64).round() as i64) } else { fin };
    Some(EarnedSchedule { es, at, pd, spi_t, sv_t: es - at, ieac, planned_finish: fin, forecast_finish })
}

#[derive(Serialize, Debug, Default)]
pub struct LookRow { pub uid: u32, pub code: String, pub name: String, pub es: i64, pub ef: i64, pub pct: u8, pub res: String, pub crit: bool, pub open: usize, pub overdue: usize }

/// The tasks to do between the status date and `weeks` weeks later, and whether each is ready
/// (no open constraints in the log).
#[derive(Serialize, Debug, Default)]
pub struct Lookahead { pub from: i64, pub to: i64, pub rows: Vec<LookRow> }

pub fn lookahead(c: &Calc, weeks: u32) -> Lookahead {
    let (p, s) = (c.p, &c.s);
    let from = c.progress_at().div_euclid(DAY);
    let to = from + weeks.clamp(1, 12) as i64 * 7;
    let mut rows: Vec<LookRow> = (0..p.tasks.len())
        .filter(|&i| !s.summary[i] && !s.off[i] && c.pct[i] < 100 && s.es[i] < to * DAY && s.ef[i] > from * DAY)
        .map(|i| {
            let t = &p.tasks[i];
            let open: Vec<&LogItem> = p.log.iter().filter(|x| x.uid == t.uid && x.status != "Cleared").collect();
            LookRow {
                uid: t.uid, code: t.code.clone(), name: t.name.clone(), es: s.es[i], ef: s.ef[i], pct: c.pct[i], res: c.text(i, "res"), crit: s.critical(p, i),
                open: open.len(), overdue: open.iter().filter(|x| x.need.is_some_and(|d| crate::cal::at(d) / DAY < from)).count(),
            }
        })
        .collect();
    rows.sort_by_key(|r| (r.es, r.ef));
    Lookahead { from, to, rows }
}
