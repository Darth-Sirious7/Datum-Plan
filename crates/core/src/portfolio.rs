//! The portfolio: every plan of a workspace side by side (with what its modules say), and the
//! shared resource pool: each resource's work across all plans against its capacity, taken from
//! a pool plan when one is chosen (else the most any plan gives it). A plan can take its
//! resources' rates, units, availability and calendars from the pool.
use crate::{cal::{date_of, DAY}, fields::Calc, load::buckets, model::*};
use chrono::NaiveDate;
use serde::Serialize;
use std::collections::HashMap;

/// One plan's headline figures.
#[derive(Serialize, Debug, Default, Clone)]
pub struct Card {
    pub path: String,
    pub folder: String,
    pub name: String,
    pub manager: String,
    pub client: String,
    pub start: String,
    pub finish: String,
    /// Start and finish, and the baseline finish, in days since 1970.
    pub s: f64,
    pub f: f64,
    pub bf: Option<f64>,
    pub var_finish: String,
    pub pct: f64,
    pub cost: String,
    pub spi: Option<f64>,
    pub cpi: Option<f64>,
    pub tasks: usize,
    pub late: usize,
    pub critical: usize,
    pub error: Option<String>,
    pub modules: Vec<String>,
    /// Contract dates: late, at risk, damages at today's forecast.
    pub contract: Option<(usize, usize, f64)>,
    /// Documents: earned %, planned %, documents.
    pub documents: Option<(f64, f64, usize)>,
    /// Procurement: earned %, planned %, orders late for the plan, orders.
    pub procurement: Option<(f64, f64, usize, usize)>,
}

pub fn card(p: &Project, fmt: &str, today: NaiveDate, path: String, folder: String) -> Card {
    let c = Calc::new(p, fmt, today);
    let s = c.summary();
    let has = |m: &str| p.modules.iter().any(|x| x == m);
    let bf = p.tasks.iter().filter_map(|t| t.bls.get(&0).map(|b| b.finish)).max().map(|f| (f - 1).div_euclid(DAY) as f64 + 1.0);
    Card {
        path, folder, name: p.name.clone(), manager: p.manager.clone(), client: p.client.clone(), start: s.start, finish: s.finish, s: s.s, f: s.f, bf, var_finish: s.var_finish,
        pct: s.pct, cost: s.cost, spi: s.spi, cpi: s.cpi, tasks: s.tasks, late: s.late, critical: s.critical, error: s.error, modules: p.modules.clone(),
        contract: has("contract").then(|| { let o = crate::contract::status(&c); (o.late, o.at_risk, o.ld) }),
        documents: has("documents").then(|| { let o = crate::docreg::status(&c); (o.earned, o.planned, o.docs.len()) }),
        procurement: has("procurement").then(|| { let o = crate::procure::status(&c); (o.earned, o.planned, o.items.iter().filter(|x| x.rag == "red").count(), o.items.len()) }),
    }
}

#[derive(Serialize, Debug, Default, Clone)]
pub struct PoolRow {
    pub name: String,
    pub group: String,
    /// Hours per plan (in the order of `Pool::plans`) per bucket, their total, and the capacity.
    pub plans: Vec<Vec<f64>>,
    pub load: Vec<f64>,
    pub cap: Vec<f64>,
    /// Buckets with more work than capacity, and the highest load against capacity (1 = full).
    pub over: usize,
    pub peak: f64,
    /// The pool plan defines this resource.
    pub in_pool: bool,
}

#[derive(Serialize, Debug, Default, Clone)]
pub struct Pool {
    pub buckets: Vec<i64>,
    pub plans: Vec<String>,
    pub rows: Vec<PoolRow>,
}

fn key(n: &str) -> String { n.trim().to_lowercase() }

/// Work of every resource (matched by name) across the plans, by day, week or month.
pub fn pool(plans: &[(String, &Project)], pool: Option<&Project>, fmt: &str, today: NaiveDate, unit: &str) -> Pool {
    let calcs: Vec<Calc> = plans.iter().map(|(_, p)| Calc::new(p, fmt, today)).collect();
    let (Some(first), Some(last)) = (calcs.iter().map(|c| c.s.start.div_euclid(DAY)).min(), calcs.iter().map(|c| (c.s.finish - 1).div_euclid(DAY)).max()) else { return Pool::default() };
    let b = buckets(first, last, unit);
    let n = b.len();
    let end = |j: usize| b.get(j + 1).copied().unwrap_or(b[j] + match unit { "day" => 1, "week" => 7, _ => 31 });
    let mut rows: Vec<PoolRow> = vec![];
    let mut at: HashMap<String, usize> = HashMap::new();
    let cap_of = |c: &Calc, k: usize, r: &Resource| -> Vec<f64> {
        (0..n).map(|j| (b[j]..end(j)).map(|d| { let day = date_of(d * DAY); r.max_on(day) * c.s.cals[c.s.rcal[k]].day_minutes(day) as f64 / 60.0 }).sum()).collect()
    };
    for (pi, c) in calcs.iter().enumerate() {
        for (k, r) in c.p.resources.iter().enumerate().filter(|(_, r)| r.kind == ResKind::Work) {
            let i = *at.entry(key(&r.name)).or_insert_with(|| { rows.push(PoolRow { name: r.name.trim().to_string(), group: r.group.clone(), plans: vec![vec![0.0; n]; plans.len()], load: vec![0.0; n], cap: vec![0.0; n], ..Default::default() }); rows.len() - 1 });
            let row = &mut rows[i];
            for (j, v) in row.plans[pi].iter_mut().enumerate() { *v += (b[j]..end(j)).map(|d| c.l.daily.get(&(k, d)).copied().unwrap_or(0.0)).sum::<f64>() / 60.0 }
            for (j, v) in cap_of(c, k, r).into_iter().enumerate() { row.cap[j] = row.cap[j].max(v) }
            if row.group.is_empty() { row.group = r.group.clone() }
        }
    }
    if let Some(pp) = pool {
        let pc = Calc::new(pp, fmt, today);
        for (k, r) in pp.resources.iter().enumerate().filter(|(_, r)| r.kind == ResKind::Work) {
            let i = *at.entry(key(&r.name)).or_insert_with(|| { rows.push(PoolRow { name: r.name.trim().to_string(), group: r.group.clone(), plans: vec![vec![0.0; n]; plans.len()], load: vec![0.0; n], cap: vec![0.0; n], ..Default::default() }); rows.len() - 1 });
            rows[i].cap = cap_of(&pc, k, r);
            rows[i].in_pool = true;
        }
    }
    for r in &mut rows {
        for j in 0..n { r.load[j] = r.plans.iter().map(|p| p[j]).sum() }
        r.over = (0..n).filter(|&j| r.load[j] > r.cap[j] + 0.01).count();
        r.peak = (0..n).map(|j| if r.cap[j] > 0.0 { r.load[j] / r.cap[j] } else if r.load[j] > 0.0 { f64::INFINITY } else { 0.0 }).fold(0.0, f64::max);
        if !r.peak.is_finite() { r.peak = 99.0 }
    }
    rows.sort_by(|a, b| b.over.cmp(&a.over).then(b.peak.total_cmp(&a.peak)).then(a.name.cmp(&b.name)));
    Pool { buckets: b, plans: plans.iter().map(|(n, _)| n.clone()).collect(), rows }
}

/// A plan takes the pool's resources: rates, units, availability, calendar and group for the ones
/// with the same name (and, with `add`, the pool's other resources). Returns (updated, added).
pub fn take(p: &mut Project, from: &[Resource], cals: &[Calendar], add: bool) -> (usize, usize) {
    let (mut up, mut new) = (0, 0);
    for r in from {
        let here = p.resources.iter().position(|x| key(&x.name) == key(&r.name));
        if here.is_none() && !add { continue }
        if let Some(c) = r.cal.as_ref().and_then(|n| cals.iter().find(|c| &c.name == n)) {
            if !p.calendars.iter().any(|x| x.name == c.name) { p.calendars.push(c.clone()) }
        }
        match here {
            Some(i) => {
                let x = &mut p.resources[i];
                (x.kind, x.group, x.max, x.rate, x.per_use, x.label, x.ovt, x.tables, x.avail, x.cal, x.exceptions) =
                    (r.kind, r.group.clone(), r.max, r.rate, r.per_use, r.label.clone(), r.ovt, r.tables.clone(), r.avail.clone(), r.cal.clone(), r.exceptions.clone());
                up += 1;
            }
            None => {
                let uid = p.resources.iter().map(|x| x.uid).max().unwrap_or(0) + 1;
                p.resources.push(Resource { uid, ..r.clone() });
                new += 1;
            }
        }
    }
    (up, new)
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::doc::{Doc, Op};
    const F: &str = "%d/%m/%Y";
    fn d(y: i32, m: u32, dd: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, dd).unwrap() }
    /// A plan with one 10-day task worked full time by `who`.
    fn plan(name: &str, who: &str, start: NaiveDate) -> Doc {
        let mut doc = Doc::new(Project::new(name, start));
        let row = vec![("name".to_string(), "Design".to_string()), ("dur".to_string(), "10d".to_string()), ("res".to_string(), who.to_string())];
        doc.apply(Op::InsertRows { at: 0, rows: vec![row] }, F).unwrap();
        doc
    }

    #[test]
    fn a_resource_shared_by_two_plans_is_over_its_capacity_where_they_overlap() {
        let (a, b) = (plan("Unit A", "Welder", d(2026, 11, 2)), plan("Unit B", "welder ", d(2026, 11, 9)));
        let o = pool(&[("Unit A".into(), &a.p), ("Unit B".into(), &b.p)], None, F, d(2026, 11, 1), "week");
        assert_eq!(o.rows.len(), 1, "matched by name, whatever the case and spaces");
        let r = &o.rows[0];
        assert_eq!(o.buckets.len(), 3);
        assert_eq!(r.plans[0][0], 40.0, "A alone in its first week");
        assert_eq!((r.load[1], r.cap[1]), (80.0, 40.0), "both plans in the second week: twice the capacity");
        assert_eq!((r.over, r.peak), (1, 2.0), "over in the one week they overlap");
        // The pool plan gives the welder two people: no longer over.
        let mut pp = Doc::new(Project::new("Pool", d(2026, 1, 5)));
        pp.apply(Op::ResInsert { name: "Welder".into() }, F).unwrap();
        let uid = pp.p.resources[0].uid;
        pp.apply(Op::ResSet { uid, field: "max".into(), value: "200%".into() }, F).unwrap();
        let o = pool(&[("Unit A".into(), &a.p), ("Unit B".into(), &b.p)], Some(&pp.p), F, d(2026, 11, 1), "week");
        assert_eq!((o.rows[0].cap[1], o.rows[0].over, o.rows[0].in_pool), (80.0, 0, true));
    }

    #[test]
    fn a_plan_takes_rates_and_units_from_the_pool() {
        let mut a = plan("Unit A", "Welder", d(2026, 11, 2));
        let mut cal = Calendar::standard("Shop");
        cal.week[5] = cal.week[0].clone();
        let pool_res = vec![Resource { name: "WELDER".into(), rate: 55.0, max: 3.0, cal: Some("Shop".into()), group: "Fabrication".into(), ..Default::default() }, Resource { name: "Fitter".into(), rate: 40.0, ..Default::default() }];
        let mut p = a.p.clone();
        assert_eq!(take(&mut p, &pool_res, std::slice::from_ref(&cal), false), (1, 0));
        let w = &p.resources[0];
        assert_eq!((w.name.as_str(), w.rate, w.max, w.cal.as_deref(), w.group.as_str()), ("Welder", 55.0, 3.0, Some("Shop"), "Fabrication"), "the plan keeps its own name and id");
        assert!(p.calendars.iter().any(|c| c.name == "Shop"), "the calendar comes along");
        assert_eq!(take(&mut p, &pool_res, &[cal], true).1, 1, "with add, the pool's other resources join");
        a.apply(Op::PoolTake { from: pool_res, cals: vec![], add: false }, F).unwrap();
        assert_eq!(a.p.resources[0].rate, 55.0);
    }
}
