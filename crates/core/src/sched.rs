//! Critical path method on working time. Each task runs on its own calendar (elapsed tasks on a
//! 24-hour clock, resource-dependent tasks on their resource's calendar). Lags use the calendar
//! chosen in Schedule Options. Summaries roll up; a link to or from a summary applies to every task
//! in it. Level-of-effort tasks stretch between their neighbours and never drive them.
use crate::cal::{at, date_of, WorkCal, DAY};
use chrono::NaiveDate;
use crate::model::{ActType, Calendar, Constraint as C, LagCal, LinkType, Project};
use std::collections::HashMap;

/// Instants per task index (minutes); floats in working minutes of the task's calendar.
pub struct Sched {
    pub es: Vec<i64>,
    pub ef: Vec<i64>,
    pub ls: Vec<i64>,
    pub lf: Vec<i64>,
    pub tf: Vec<i64>,
    pub ff: Vec<i64>,
    /// Where each task's remaining work starts: after the status date for a task under way, else its start.
    pub rs: Vec<i64>,
    pub summary: Vec<bool>,
    /// Inactive, or under an inactive summary.
    pub off: Vec<bool>,
    /// Level of effort or WBS summary activity.
    pub loe: Vec<bool>,
    /// Tasks whose length follows a task in another plan (`mirrored`).
    pub mirror: Vec<bool>,
    /// On the longest path.
    pub lp: Vec<bool>,
    /// Float path number (1 = most critical; 0 = not calculated).
    pub fpath: Vec<u8>,
    /// Index into `cals` for each task.
    pub ci: Vec<usize>,
    /// Index into `cals` for each resource: its own working time (with resource calendars on),
    /// else the project calendar. Capacity in workload and levelling uses it.
    pub rcal: Vec<usize>,
    pub cals: Vec<WorkCal>,
    pub edges: Vec<Edge>,
    /// Whether each edge drives its successor's dates.
    pub drive: Vec<bool>,
    pub start: i64,
    pub finish: i64,
}

#[derive(Clone, Copy)]
pub struct Edge { pub p: usize, pub s: usize, pub kind: LinkType, pub lag: i64, /** Elapsed lag: clock time, round the clock. */ pub el: bool, /** Comes from a link on the predecessor's summary. */ pub up: bool }

/// The other plan's task a task mirrors (SS and FF to it, no lag), as its last known start and finish.
pub fn mirrored(t: &crate::model::Task) -> Option<(i64, i64)> {
    let ss = t.ext.iter().find(|x| x.kind == LinkType::SS && x.lag == 0 && x.finish > x.start)?;
    t.ext.iter().any(|x| x.kind == LinkType::FF && x.lag == 0 && x.uid == ss.uid && x.file == ss.file).then_some((ss.start, ss.finish))
}

pub fn schedule(p: &Project) -> Result<Sched, String> {
    let n = p.tasks.len();
    let o = &p.opts;
    let summary: Vec<bool> = (0..n).map(|i| p.is_summary(i)).collect();
    let mut off = vec![false; n];
    for i in 0..n {
        if p.tasks[i].inactive && !off[i] {
            p.subtree(i).for_each(|j| off[j] = true);
        }
    }
    let names: Vec<&str> = p.calendars.iter().map(|c| c.name.as_str()).collect();
    let mut cals: Vec<WorkCal> = p.calendars.iter().map(WorkCal::new).collect();
    let h24 = cals.len();
    cals.push(WorkCal::new(&Calendar::full("24 Hours")));
    let def = names.iter().position(|n| *n == p.calendar).unwrap_or(0);
    let cal_of = |name: Option<&str>| name.and_then(|c| names.iter().position(|n| *n == c));
    // Resource calendars: each person's own working time is their base calendar plus their leave.
    let rcal: Vec<usize> = p.resources.iter().map(|r| {
        let base = cal_of(r.cal.as_deref()).unwrap_or(def);
        if !o.res_cals || r.exceptions.is_empty() { return if o.res_cals { base } else { def } }
        let mut c = p.calendars[base].clone();
        c.exceptions.extend(r.exceptions.iter().cloned());
        cals.push(WorkCal::new(&c));
        cals.len() - 1
    }).collect();
    // The calendars a task may run on: one per assigned person (with the task's own calendar,
    // when it has one, working time must suit both). The task finishes when the last person does.
    let mut both: HashMap<(usize, usize), usize> = HashMap::new();
    let cand: Vec<Vec<usize>> = p.tasks.iter().map(|t| {
        if !o.res_cals || t.elapsed || t.ignore_rc { return vec![] }
        let tc = cal_of(t.cal.as_deref());
        let mut v: Vec<usize> = t.res.iter().filter(|a| a.units > 0.0).filter_map(|a| p.res_idx(a.res)).filter(|&k| p.resources[k].kind == crate::model::ResKind::Work).map(|k| match tc {
            None => rcal[k],
            Some(tc) => *both.entry((tc, rcal[k])).or_insert_with(|| { let x = cals[tc].intersect(&cals[rcal[k]]); cals.push(x); cals.len() - 1 }),
        }).collect();
        v.sort_unstable();
        v.dedup();
        v
    }).collect();
    let ci: Vec<std::cell::Cell<usize>> = p.tasks.iter().zip(&cand).map(|(t, cd)| {
        if t.elapsed {
            return h24.into();
        }
        if let Some(&k) = cd.first() {
            return k.into();
        }
        let res_cal = || t.res.first().and_then(|a| p.res_idx(a.res)).and_then(|k| cal_of(p.resources[k].cal.as_deref()));
        (if t.act_type == ActType::ResourceDep { res_cal() } else { None }).or_else(|| cal_of(t.cal.as_deref())).unwrap_or(def).into()
    }).collect();
    let ix: HashMap<u32, usize> = p.tasks.iter().enumerate().map(|(i, t)| (t.uid, i)).collect();
    let ms_type = |i: usize| matches!(p.tasks[i].act_type, ActType::StartMs | ActType::FinishMs);
    // A task that starts and finishes with one task of another plan (SS and FF, no lag) mirrors it:
    // it lasts as long as that task does, as last known.
    let mirror: Vec<bool> = p.tasks.iter().map(|t| mirrored(t).is_some()).collect();
    let dur: Vec<i64> = p.tasks.iter().enumerate().map(|(i, t)| if ms_type(i) { 0 } else if let Some((a, b)) = mirrored(t) { cals[ci[i].get()].between(a, b) } else { t.dur }).collect();
    // Splits add pauses inside the work.
    let span: Vec<i64> = p.tasks.iter().enumerate().map(|(i, t)| dur[i] + t.splits.iter().filter(|(a, _)| *a < dur[i]).map(|(_, g)| g.max(&0)).sum::<i64>()).collect();
    // Work still to do at `pct` % complete, with the pauses that come after the work done.
    let rest = |i: usize, pct: u8| { let done = dur[i] * pct.min(100) as i64 / 100; dur[i] - done + p.tasks[i].splits.iter().filter(|(a, _)| *a > done && *a < dur[i]).map(|(_, g)| g.max(&0)).sum::<i64>() };
    let loe: Vec<bool> = p.tasks.iter().map(|t| matches!(t.act_type, ActType::Loe | ActType::WbsSummary)).collect();
    let leaf = |i: usize| !summary[i] && !off[i];
    // Each task's summary (the row above it at a lower outline level).
    let mut open: Vec<usize> = vec![];
    let parent: Vec<Option<usize>> = (0..n).map(|i| {
        while open.last().is_some_and(|&j| p.tasks[j].level >= p.tasks[i].level) { open.pop(); }
        let up = open.last().copied();
        open.push(i);
        up
    }).collect();
    // An inactive summary stands for its inactive tasks, an active one for its active tasks.
    let leaves_of = |i: usize| -> Vec<usize> { if summary[i] { p.subtree(i).filter(|&j| !summary[j] && off[j] == off[i]).collect() } else { vec![i] } };
    // Inactive tasks are still scheduled from their predecessors (as in MS Project) but drive nothing.
    let node = |i: usize| !summary[i];

    let mut edges: Vec<Edge> = vec![];
    let (mut inc, mut out) = (vec![vec![]; n], vec![vec![]; n]);
    for (s, t) in p.tasks.iter().enumerate() {
        for l in &t.preds {
            let Some(&q) = ix.get(&l.pred) else { continue };
            if off[q] && !off[s] {
                continue;
            }
            if (summary[q] && p.subtree(q).contains(&s)) || (summary[s] && p.subtree(s).contains(&q)) {
                return Err(format!("Task {} is linked to its own summary task. Link tasks inside the summary instead.", s + 1));
            }
            // A start link (SS, SF) from a summary starts with the summary: from the tasks that open it
            // (those with no predecessor inside it), not from its last task to start.
            let mut from = leaves_of(q);
            if summary[q] && matches!(l.kind, LinkType::SS | LinkType::SF) {
                let inside = p.subtree(q);
                let open: Vec<usize> = from.iter().copied().filter(|&a| !p.tasks[a].preds.iter().any(|x| ix.get(&x.pred).is_some_and(|j| inside.contains(j)))).collect();
                if !open.is_empty() { from = open }
            }
            for &a in &from {
                for &b in &leaves_of(s) {
                    let lag = if l.pct { dur[a] * l.lag / 100 } else { l.lag };
                    inc[b].push(edges.len());
                    out[a].push(edges.len());
                    edges.push(Edge { p: a, s: b, kind: l.kind, lag, el: l.elapsed && !l.pct, up: summary[q] });
                }
            }
        }
    }

    // Topological order (Kahn), stable by row.
    let mut indeg: Vec<usize> = inc.iter().map(Vec::len).collect();
    let mut order: Vec<usize> = (0..n).filter(|&i| node(i) && indeg[i] == 0).collect();
    let mut k = 0;
    while k < order.len() {
        for &e in &out[order[k]] {
            let s = edges[e].s;
            indeg[s] -= 1;
            if indeg[s] == 0 {
                order.push(s);
            }
        }
        k += 1;
    }
    if order.len() < (0..n).filter(|&i| node(i)).count() {
        let i = (0..n).find(|&i| node(i) && indeg[i] > 0).unwrap();
        return Err(format!("Circular link: task {} \"{}\" ends up depending on itself.", i + 1, p.tasks[i].name));
    }

    // The calendar a lag is counted on (an elapsed lag on the clock).
    let lag_cal = |e: &Edge| if e.el { h24 } else { match o.lag_cal { LagCal::Successor => ci[e.s].get(), LagCal::Predecessor => ci[e.p].get(), LagCal::H24 => h24, LagCal::Project => def } };
    let pc = &cals[def];
    // Predecessors in other plans hold a task like links, from their last known dates (0 = never
    // read); links on a summary hold every task under it. Lags count on the lag calendar (the other
    // plan's calendars are not known here: the project calendar stands in for the predecessor's);
    // an elapsed lag on the clock.
    let ext_need = |i: usize, c: &WorkCal, sp: i64| std::iter::successors(Some(i), |&j| parent[j]).flat_map(|j| p.tasks[j].ext.iter()).filter(|x| x.finish != 0).map(|x| {
        let lc = &cals[if x.elapsed { h24 } else { match o.lag_cal { LagCal::Successor => ci[i].get(), LagCal::H24 => h24, LagCal::Predecessor | LagCal::Project => def } }];
        let y = match x.kind { LinkType::FS | LinkType::FF => lc.add(x.finish, x.lag), LinkType::SS | LinkType::SF => lc.add(x.start, x.lag) };
        match x.kind { LinkType::FS | LinkType::SS => y, _ => c.sub(y, sp) }
    }).max();
    let start = pc.next_start(at(p.start));
    // Microsoft Project style: the status date measures progress but does not move work.
    let data = if o.msp_status { None } else { p.status.map(|d| at(d) + DAY) };
    let (mut es, mut ef) = (vec![start; n], vec![start; n]);
    let mut fixed = vec![false; n];
    let mut rs = vec![i64::MIN; n];
    // The earliest start (or finish, for FF/SF) a link allows.
    let need = |e: &Edge, es: &[i64], ef: &[i64]| -> i64 {
        let lc = &cals[lag_cal(e)];
        match e.kind { LinkType::FS | LinkType::FF => lc.add(ef[e.p], e.lag), LinkType::SS | LinkType::SF => lc.add(es[e.p], e.lag) }
    };
    // With several people on a task, the one who would finish last sets the task's calendar.
    let pick = |i: usize, v: i64, sp: i64| {
        if cand[i].len() > 1 {
            let k = cand[i].iter().copied().max_by_key(|&k| (cals[k].add(cals[k].next_start(v), sp), std::cmp::Reverse(k))).unwrap_or(ci[i].get());
            ci[i].set(k);
        }
    };
    for &i in &order {
        let (t, d, sp) = (&p.tasks[i], dur[i], span[i]);
        if let Some(a) = t.act_start { pick(i, at(a), sp) }
        let c = &cals[ci[i].get()];
        if let Some(a) = t.act_start {
            // Actual dates stand as recorded, also on a day off (at the usual shift times then).
            let usual = |d: NaiveDate| { let w = c.day(date_of(c.next_start(at(d)))); (w.first().map_or(480, |x| x.0), w.last().map_or(1020, |x| x.1)) };
            es[i] = if c.is_work_day(a) { c.next_start(at(a)) } else { at(a) + usual(a).0 };
            ef[i] = match (t.act_finish, data) {
                (Some(f), _) => {
                    fixed[i] = true;
                    (if c.is_work_day(f) { c.end_of(f) } else { at(f) + usual(f).1 }).max(es[i])
                }
                (None, Some(dd)) if t.pct < 100 => {
                    let rem = rest(i, t.pct);
                    let mut from = dd.max(es[i]);
                    // Retained logic: unfinished predecessors still hold back the remaining work.
                    if o.retained {
                        for &e in inc[i].iter().filter(|&&e| !fixed[edges[e].p] && !loe[edges[e].p]) {
                            let x = need(&edges[e], &es, &ef);
                            from = from.max(match edges[e].kind { LinkType::FS | LinkType::SS => x, _ => c.sub(x, rem) });
                        }
                        from = from.max(ext_need(i, c, rem).unwrap_or(from));
                    }
                    rs[i] = c.next_start(from);
                    c.add(rs[i], rem)
                }
                _ => c.add(es[i], sp),
            };
            continue;
        }
        // The earliest start on calendar `c`: finish links and constraints count back on it.
        // A constraint's moment: the time it was given with (from Project), else the start or end of its working day.
        let at_time = |cd: NaiveDate| t.ctime.filter(|(d, _)| *d == cd).map(|(_, m)| at(cd) + m as i64);
        let want = |c: &WorkCal| {
            // No task starts before the plan start, unless a start-no-earlier-than says so or a predecessor
            // already starts before it (as in MS Project; a lead alone does not pull a task out of the plan).
            let first_pred = inc[i].iter().filter(|&&e| !loe[edges[e].p]).map(|&e| es[edges[e].p]).min();
            let base = match (t.constraint, t.cdate) {
                (C::SNET, Some(cd)) => start.min(at_time(cd).unwrap_or(at(cd))),
                _ if !(t.manual && t.mstart.is_some()) => first_pred.map_or(start, |f| start.min(f)),
                _ => start,
            };
            let mut v = match data { Some(dd) if t.pct == 0 => dd.max(base), _ => base };
            if !(t.manual && t.mstart.is_some()) {
                for &e in inc[i].iter().filter(|&&e| !loe[edges[e].p]) {
                    let x = need(&edges[e], &es, &ef);
                    v = v.max(match edges[e].kind { LinkType::FS | LinkType::SS => x, _ => c.sub(x, sp) });
                }
                v = v.max(ext_need(i, c, sp).unwrap_or(v));
            }
            if let Some(cd) = t.cdate {
                let (cs, ce) = (at_time(cd).unwrap_or(at(cd)), at_time(cd).unwrap_or(c.end_of(cd)));
                match t.constraint {
                    C::SNET => v = v.max(cs),
                    C::FNET => v = v.max(c.sub(ce, sp)),
                    // A milestone may sit anywhere on its day (at the end of it after a finish).
                    C::MSO if sp == 0 => v = v.clamp(cs, ce.max(cs)),
                    C::MSO => v = cs,
                    C::MFO => v = c.sub(ce, sp),
                    _ => {}
                }
            }
            if let (true, Some(m)) = (t.manual, t.mstart) {
                // A manual milestone sits at the end of its day, as MS Project places it (its successors start the next day).
                v = if sp == 0 { c.end_of(m).max(at(m)) } else { at(m) };
            }
            v
        };
        let v = want(c);
        pick(i, v, sp);
        let c = &cals[ci[i].get()];
        // When another person's calendar sets the finish, count back on that calendar.
        let v = if cand[i].len() > 1 { want(c) } else { v };
        es[i] = if d > 0 || t.act_type == ActType::StartMs || !c.in_or_end(v) { c.next_start(v) } else { v };
        if t.level_delay > 0 {
            es[i] = c.next_start(c.add(es[i], t.level_delay));
        }
        ef[i] = c.add(es[i], sp);
        // With several people, the task starts when the first of them can (and ends with the last).
        if cand[i].len() > 1 && t.level_delay == 0 {
            es[i] = cand[i].iter().map(|&k| cals[k].next_start(v)).min().map_or(es[i], |x| x.min(es[i]));
        }
    }
    // Level of effort: from its own start to the latest point its successors need.
    for &i in order.iter().filter(|&&i| loe[i] && !fixed[i]) {
        let reach = out[i].iter().map(|&e| {
            let Edge { s, kind, lag, .. } = edges[e];
            let lc = &cals[lag_cal(&edges[e])];
            // A start the successor needs means the effort ends at the end of the working time before it.
            match kind { LinkType::FS => cals[ci[i].get()].prev_end(lc.sub(es[s], lag)), LinkType::SS => cals[ci[i].get()].prev_end(es[s]), LinkType::FF | LinkType::SF => lc.sub(ef[s], lag) }
        }).max();
        if let Some(r) = reach {
            ef[i] = r.max(es[i]);
        }
    }
    let mut finish = order.iter().filter(|&&i| !off[i]).map(|&i| ef[i]).max().unwrap_or(start);
    let back_from = match (o.from_finish, o.finish_date) { (true, Some(f)) => pc.end_of(f), _ => finish };

    // Which links drive their successor (for the longest path and the Inspect pane).
    let drive: Vec<bool> = edges.iter().map(|e| {
        if loe[e.p] || fixed[e.s] {
            return false;
        }
        let c = &cals[ci[e.s].get()];
        let x = need(e, &es, &ef);
        match e.kind { LinkType::FS | LinkType::SS => c.next_start(x) >= es[e.s] || x == es[e.s], _ => x >= ef[e.s] }
    }).collect();

    let (mut ls, mut lf, mut ff) = (es.clone(), ef.clone(), vec![0i64; n]);
    for &i in order.iter().rev() {
        let (t, c, sp) = (&p.tasks[i], &cals[ci[i].get()], span[i]);
        // With several critical paths the end of each chain is its own finish; otherwise the end of the plan.
        // (Inactive successors do not count, nor a manual task's start links: such a task ends its chain.)
        let ends = out[i].iter().all(|&e| (off[edges[e].s] && !off[i]) || (p.tasks[i].manual && matches!(edges[e].kind, LinkType::SS | LinkType::SF)));
        let mut v = if o.multi_critical && ends && !fixed[i] { ef[i] } else { back_from };
        let mut free = c.between(ef[i], v);
        for &e in &out[i] {
            let Edge { s, kind, lag, .. } = edges[e];
            // Finished work, and (with progress override) work already under way, no longer waits for this task.
            if fixed[s] || (!o.retained && p.tasks[s].act_start.is_some()) || (off[s] && !off[i]) {
                continue;
            }
            // A start link no longer limits a task that has already started (its start is a fact), as in MS Project.
            if t.act_start.is_some() && matches!(kind, LinkType::SS | LinkType::SF) {
                continue;
            }
            let lc = &cals[lag_cal(&edges[e])];
            // A manual task's late dates come from its finish links only: a start link still counts in its free
            // float but not in its late finish (as MS Project computes them; checked on a manual milestone).
            if !(t.manual && matches!(kind, LinkType::SS | LinkType::SF)) {
            v = v.min(match kind {
                LinkType::FS => lc.sub(ls[s], lag),
                LinkType::SS => c.add(lc.sub(ls[s], lag), sp),
                LinkType::FF => lc.sub(lf[s], lag),
                LinkType::SF => c.add(lc.sub(lf[s], lag), sp),
            });
            }
            // A link on its summary holds the task but is not its own: it does not count in its free float.
            if edges[e].up {
                continue;
            }
            free = free.min(match kind {
                LinkType::FS => c.between(lc.add(ef[i], lag), es[s]),
                LinkType::SS => c.between(lc.add(es[i], lag), es[s]),
                LinkType::FF => c.between(lc.add(ef[i], lag), ef[s]),
                LinkType::SF => c.between(lc.add(es[i], lag), ef[s]),
            });
        }
        if let Some(cd) = t.cdate {
            let tm = t.ctime.filter(|(d, _)| *d == cd).map(|(_, m)| at(cd) + m as i64);
            let (cs, ce) = (tm.unwrap_or(at(cd)), tm.unwrap_or(c.end_of(cd)));
            match t.constraint {
                // Dates are whole days: a milestone starts "on" its day up to the end of it.
                C::SNLT | C::MSO if sp == 0 => {
                    let a = c.next_start(cs);
                    v = if t.constraint == C::MSO { v.clamp(a, ce.max(a)) } else { v.min(ce.max(a)) };
                }
                C::SNLT => v = v.min(c.add(c.next_start(cs), sp)),
                C::FNLT => v = v.min(ce),
                C::MSO => v = c.add(c.next_start(cs), sp),
                C::MFO => v = ce,
                _ => {}
            }
        }
        if let Some(dl) = t.deadline {
            v = v.min(c.end_of(dl));
        }
        if fixed[i] {
            continue;
        }
        if loe[i] {
            (ls[i], lf[i]) = (es[i], ef[i]);
            continue;
        }
        // A started task's late start is that of its remaining work, which is what its predecessors must meet.
        let rest = if t.act_start.is_some() { rest(i, t.pct) } else { sp };
        lf[i] = if sp > 0 { c.prev_end(v) } else { v };
        ls[i] = c.sub(lf[i], rest);
        ff[i] = free.max(0);
        let backward = o.from_finish && o.finish_date.is_some() && !t.manual && t.act_start.is_none();
        if backward {
            (es[i], ef[i]) = (ls[i], lf[i]);
            ff[i] = 0;
        } else if t.constraint == C::ALAP && t.act_start.is_none() && ff[i] > 0 {
            // As late as possible without delaying any successor: the largest shift (at most its free
            // float) that every link still allows, by the same rule as the forward pass.
            let fits = |sh: i64| {
                let e0 = c.next_start(c.add(es[i], sh));
                let f0 = c.add(e0, sp);
                f0 <= back_from && out[i].iter().all(|&e| {
                    let Edge { s, kind, lag, .. } = edges[e];
                    let x = cals[lag_cal(&edges[e])].add(if matches!(kind, LinkType::FS | LinkType::FF) { f0 } else { e0 }, lag);
                    if matches!(kind, LinkType::FS | LinkType::SS) { cals[ci[s].get()].next_start(x) <= es[s] } else { x <= ef[s] }
                })
            };
            let (mut lo, mut hi) = (0, ff[i]);
            while lo < hi {
                let mid = (lo + hi + 1) / 2;
                if fits(mid) { lo = mid } else { hi = mid - 1 }
            }
            if lo > 0 {
                es[i] = c.next_start(c.add(es[i], lo));
                ef[i] = c.add(es[i], sp);
            }
            ff[i] = 0;
        }
    }
    let mut tf: Vec<i64> = (0..n).map(|i| if leaf(i) && !fixed[i] && !loe[i] { cals[ci[i].get()].between(ef[i], lf[i]) } else { 0 }).collect();
    for i in (0..n).filter(|&i| leaf(i)) {
        // Free float never exceeds total float; a task under way can no longer start late (MS Project shows its start).
        ff[i] = ff[i].min(tf[i].max(0));
        if p.tasks[i].act_start.is_some() && !fixed[i] {
            ls[i] = es[i];
        }
    }
    let mut start = start;
    if o.from_finish && o.finish_date.is_some() {
        start = order.iter().filter(|&&i| !off[i]).map(|&i| es[i]).min().unwrap_or(start);
        finish = back_from;
    }

    // Longest path: back from the tasks that finish last, through driving links.
    let mut lp = vec![false; n];
    let mut stack: Vec<usize> = order.iter().copied().filter(|&i| ef[i] >= finish && !loe[i] && !off[i]).collect();
    while let Some(i) = stack.pop() {
        if std::mem::replace(&mut lp[i], true) {
            continue;
        }
        stack.extend(inc[i].iter().filter(|&&e| drive[e]).map(|&e| edges[e].p));
    }
    // Float paths: 1 is the longest path, then tasks grouped by total float, most critical first.
    let mut fpath = vec![0u8; n];
    if o.float_paths > 0 {
        let mut rest: Vec<usize> = order.iter().copied().filter(|&i| !lp[i] && !fixed[i] && !loe[i] && !off[i]).collect();
        rest.sort_by_key(|&i| (tf[i], es[i]));
        order.iter().filter(|&&i| lp[i]).for_each(|&i| fpath[i] = 1);
        let (mut path, mut last) = (1u8, None);
        for i in rest {
            if last != Some(tf[i]) {
                path += 1;
                last = Some(tf[i]);
            }
            if path > o.float_paths {
                break;
            }
            fpath[i] = path;
        }
    }

    for i in (0..n).filter(|&i| summary[i]) {
        let kids: Vec<usize> = p.subtree(i).filter(|&j| !summary[j] && off[j] == off[i]).collect();
        if kids.is_empty() {
            continue;
        }
        es[i] = kids.iter().map(|&j| es[j]).min().unwrap();
        ef[i] = kids.iter().map(|&j| ef[j]).max().unwrap();
        lf[i] = kids.iter().map(|&j| lf[j]).max().unwrap();
        // As MS Project shows a summary: the float of its unfinished tasks that finish last, its free float up to its own successors
        // (below), and a late start that is its start once under way, else its start moved by that float.
        let open: Vec<usize> = kids.iter().copied().filter(|&j| !fixed[j] && !loe[j]).collect();
        let last = open.iter().map(|&j| ef[j]).max();
        // An elapsed task's float is on the clock: the summary takes it in whole days of the plan (as MS Project does).
        let plan_days = |j: usize| if p.tasks[j].elapsed { tf[j].div_euclid(DAY) * p.day_min } else { tf[j] };
        tf[i] = open.iter().filter(|&&j| Some(ef[j]) == last).map(|&j| plan_days(j)).min().unwrap_or(0);
        ff[i] = tf[i];
        ls[i] = if kids.iter().any(|&j| p.tasks[j].act_start.is_some()) || tf[i] <= 0 { es[i] } else { pc.add(es[i], tf[i]) };
        lp[i] = kids.iter().any(|&j| lp[j]);
        fpath[i] = kids.iter().map(|&j| fpath[j]).filter(|&f| f > 0).min().unwrap_or(0);
    }
    for (s, t) in p.tasks.iter().enumerate().filter(|&(s, _)| !off[s]) {
        for l in &t.preds {
            let Some(&q) = ix.get(&l.pred) else { continue };
            if !summary[q] || off[q] {
                continue;
            }
            let gap = match l.kind {
                LinkType::FS => pc.between(pc.add(ef[q], l.lag), es[s]),
                LinkType::SS => pc.between(pc.add(es[q], l.lag), es[s]),
                LinkType::FF => pc.between(pc.add(ef[q], l.lag), ef[s]),
                LinkType::SF => pc.between(pc.add(es[q], l.lag), ef[s]),
            };
            ff[q] = ff[q].min(gap.max(0));
        }
    }
    let ci = ci.iter().map(std::cell::Cell::get).collect();
    // Remaining work starts at the start, except for work under way at the status date.
    for i in 0..n { if rs[i] == i64::MIN || summary[i] { rs[i] = es[i] } }
    Ok(Sched { es, ef, ls, lf, tf, ff, rs, summary, off, loe, mirror, lp, fpath, ci, rcal, cals, edges, drive, start, finish })
}

impl Sched {
    /// Is task `i` critical under the plan's options?
    pub fn critical(&self, p: &Project, i: usize) -> bool {
        if self.off[i] || self.loe[i] {
            return false;
        }
        // A finished task is not critical, and neither is a summary whose tasks are all finished (as in MS Project).
        let done = if self.summary[i] {
            let mut leaves = p.subtree(i).filter(|&j| !self.summary[j] && !self.off[j]).peekable();
            leaves.peek().is_some() && leaves.all(|j| p.tasks[j].pct >= 100)
        } else {
            p.tasks[i].pct >= 100
        };
        if done {
            return false;
        }
        if p.opts.longest_path { self.lp[i] } else { self.tf[i] <= p.opts.critical_float }
    }
}
