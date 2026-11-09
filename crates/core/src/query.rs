//! What a view shows: the visible rows after filter, group, sort and outline, their cells for the
//! chosen columns, and the bar geometry the Gantt, timeline and network diagram draw.
use crate::cal::{at, date_of, DAY};
use crate::fields::{self, res_text, task_fields, Calc, Kind};
use crate::formula::F;
use crate::model::*;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Rule { pub field: String, pub test: String, pub value: String, pub value2: String, pub or: bool }

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Filter {
    pub name: String,
    pub rules: Vec<Rule>,
    /// Show the summary rows of matching tasks.
    pub summaries: bool,
    /// Highlight matches instead of hiding the rest.
    pub highlight: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Query {
    pub cols: Vec<String>,
    pub filter: Filter,
    /// Group by these fields, outermost first.
    pub group: Vec<String>,
    /// Sort keys: field and descending.
    pub sort: Vec<(String, bool)>,
    /// Ignore the outline (sort and show every task flat).
    pub flat: bool,
    pub collapsed: Vec<u32>,
    /// Show outline levels up to this one (0 = all).
    pub level: u8,
    pub hide_summaries: bool,
    pub project_summary: bool,
    /// Baseline drawn on the Gantt.
    pub baseline: u8,
}

#[derive(Serialize, Default)]
pub struct Bar {
    /// Days since 1970-01-01 (fractions are times of day), sent with 5 decimals (under a second).
    #[serde(serialize_with = "r5")]
    pub s: f64,
    #[serde(serialize_with = "r5")]
    pub f: f64,
    #[serde(serialize_with = "r5")]
    pub ls: f64,
    #[serde(serialize_with = "r5")]
    pub lf: f64,
    #[serde(serialize_with = "r5o")]
    pub bs: Option<f64>,
    #[serde(serialize_with = "r5o")]
    pub bf: Option<f64>,
    #[serde(serialize_with = "r5o")]
    pub dl: Option<f64>,
    pub pct: u8,
    pub segs: Vec<(f64, f64)>,
    pub links: Vec<(u32, LinkType)>,
}

fn r5<S: serde::Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> { s.serialize_f64((v * 1e5).round() / 1e5) }
fn r5o<S: serde::Serializer>(v: &Option<f64>, s: S) -> Result<S::Ok, S::Error> { match v { Some(v) => s.serialize_some(&((v * 1e5).round() / 1e5)), None => s.serialize_none() } }

#[derive(Serialize, Default)]
pub struct GRow {
    /// 0 for group headers and the project summary.
    pub uid: u32,
    pub id: usize,
    pub lvl: u8,
    /// task, summary, group, project.
    pub kind: &'static str,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ms: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub crit: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub late: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub manual: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub off: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub lp: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub kids: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub open: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub hl: bool,
    /// Name and resources for bar labels and tips.
    pub name: String,
    pub res: String,
    pub cells: Vec<String>,
    /// Indicator icons per column, where a custom field shows icons.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub icons: Vec<(usize, String)>,
    /// Text formatting by column key ("*" = row).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub style: BTreeMap<String, String>,
    pub bar: Bar,
    /// Tasks in other plans this task waits for, and (filled in by the window's shell from the
    /// other open plans) tasks in other plans that wait for this one.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ext: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ext_out: Vec<String>,
    /// A task it waits for in another open plan has moved since (Update Links).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ext_moved: bool,
    /// In a master's combined view: the subplan (file) a read-only row comes from, and the
    /// task's uid in that plan (rows from a subplan have `uid` 0, so nothing edits them here).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub sub: String,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub suid: u32,
}

fn is_zero_u32(v: &u32) -> bool { *v == 0 }

#[derive(Serialize)]
pub struct Col { pub key: String, pub name: String, pub kind: Kind, pub edit: bool }

#[derive(Serialize)]
pub struct Grid { pub cols: Vec<Col>, pub rows: Vec<GRow>, pub shown: usize, pub total: usize }

fn day(x: i64) -> f64 { x as f64 / DAY as f64 }

fn cmp_f(a: &F, b: &F) -> Ordering {
    match (a, b) {
        (F::Null, F::Null) => Ordering::Equal,
        (F::Null, _) => Ordering::Greater,
        (_, F::Null) => Ordering::Less,
        (F::Str(x), F::Str(y)) => x.to_lowercase().cmp(&y.to_lowercase()),
        (F::Date(x), F::Date(y)) => x.cmp(y),
        (F::Bool(x), F::Bool(y)) => x.cmp(y),
        _ => a.num().partial_cmp(&b.num()).unwrap_or(Ordering::Equal),
    }
}

impl Calc<'_> {
    /// Does row `i` pass the filter's rules?
    pub fn matches(&self, i: usize, f: &Filter) -> bool {
        let mut acc: Option<bool> = None;
        for r in &f.rules {
            let Some(k) = self.key_of(&r.field) else { continue };
            let v = self.val(i, &k);
            let ok = fields::test(&v, self.kind(&k), &r.test, &r.value, &r.value2, self);
            acc = Some(match acc { None => ok, Some(a) if r.or => a || ok, Some(a) => a && ok });
        }
        acc.unwrap_or(true)
    }

    fn sort_cmp(&self, a: usize, b: usize, keys: &[(String, bool)]) -> Ordering {
        for (k, desc) in keys {
            let Some(k) = self.key_of(k) else { continue };
            let o = cmp_f(&self.val(a, &k), &self.val(b, &k));
            if o != Ordering::Equal {
                return if *desc { o.reverse() } else { o };
            }
        }
        a.cmp(&b)
    }

    pub fn bar(&self, i: usize, baseline: u8) -> Bar {
        let (p, s) = (self.p, &self.s);
        let t = &p.tasks[i];
        let b = t.bls.get(&baseline);
        let mut segs = vec![];
        if !t.splits.is_empty() && !s.summary[i] {
            let c = &s.cals[s.ci[i]];
            let (mut pos, mut done) = (s.es[i], 0);
            for &(a, g) in t.splits.iter().filter(|(a, _)| *a > 0 && *a < t.dur) {
                let end = c.add(pos, a - done);
                segs.push((day(pos), day(end)));
                pos = c.next_start(c.add(end, g.max(0)));
                done = a;
            }
            segs.push((day(pos), day(s.ef[i])));
        }
        Bar {
            s: day(s.es[i]), f: day(s.ef[i]), ls: day(s.ls[i]), lf: day(s.lf[i]),
            bs: b.map(|b| day(b.start)), bf: b.map(|b| day(b.finish)),
            dl: t.deadline.map(|d| day(at(d) + DAY)), pct: self.pct[i], segs,
            links: t.preds.iter().map(|l| (l.pred, l.kind)).collect(),
        }
    }

    fn row(&self, i: usize, cols: &[String], q: &Query, hl: bool) -> GRow {
        let (p, s) = (self.p, &self.s);
        let t = &p.tasks[i];
        let mut icons = vec![];
        let cells = cols.iter().enumerate().map(|(c, k)| {
            if let Some(ic) = self.icon(i, k) {
                icons.push((c, ic));
            }
            self.text(i, k)
        }).collect();
        GRow {
            uid: t.uid, id: i + 1, lvl: t.level, kind: if s.summary[i] { "summary" } else { "task" },
            ms: !s.summary[i] && (t.dur == 0 || matches!(t.act_type, ActType::StartMs | ActType::FinishMs)),
            crit: s.critical(p, i), late: self.val(i, "late").truth(), manual: t.manual, off: s.off[i], lp: s.lp[i],
            kids: s.summary[i], open: !q.collapsed.contains(&t.uid), hl, name: t.name.clone(), res: self.text(i, "res"), cells, icons, style: t.style.as_object().map(|m| m.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect()).unwrap_or_default(), bar: self.bar(i, q.baseline),
            ext: t.ext.iter().map(|x| crate::links::label(x, p.day_min)).collect(), ext_out: vec![], ext_moved: false, sub: String::new(), suid: 0,
        }
    }

    /// The project summary row (Task 0).
    fn project_row(&self, cols: &[String]) -> GRow {
        let (p, s) = (self.p, &self.s);
        let top: Vec<usize> = (0..p.tasks.len()).filter(|&i| p.tasks[i].level == 0 && !s.off[i]).collect();
        let sum = |f: &dyn Fn(usize) -> f64| top.iter().map(|&i| f(i)).sum::<f64>();
        let pc = &s.cals[p.calendars.iter().position(|c| c.name == p.calendar).unwrap_or(0)];
        let dur: f64 = top.iter().map(|&i| self.p.tasks[i].dur as f64).sum();
        let pct = if dur > 0.0 { top.iter().map(|&i| self.pct[i] as f64 * p.tasks[i].dur as f64).sum::<f64>() / dur } else { 0.0 };
        let bls: Vec<&Baseline> = top.iter().filter_map(|&i| p.tasks[i].bls.get(&0)).collect();
        let cells = cols.iter().map(|k| {
            let v = match k.as_str() {
                "name" => F::Str(p.name.clone()),
                "id" => F::Num(0.0),
                "start" | "es" => F::Date(s.start),
                "finish" | "ef" => F::Date(s.finish),
                "dur" => F::Num(pc.between(s.start, s.finish) as f64),
                "pct" => F::Num(pct.round()),
                "b0start" => bls.iter().map(|b| b.start).min().map_or(F::Null, F::Date),
                "b0finish" => bls.iter().map(|b| b.finish).max().map_or(F::Null, F::Date),
                "b0cost" => F::Num(bls.iter().map(|b| b.cost).sum()),
                k if matches!(self.kind(k), Kind::Cost | Kind::Work) && !k.starts_with("c:") => F::Num(sum(&|i| self.val(i, k).num())),
                _ => F::Null,
            };
            match (&v, top.first()) {
                (F::Num(_) | F::Date(_), Some(&i)) if !matches!(self.kind(k), Kind::Dur) => self.format(i, k, &v),
                (F::Num(m), _) if self.kind(k) == Kind::Dur => crate::parse::fmt_dur(*m as i64, Unit::D, p.day_min),
                (F::Str(x), _) => x.clone(),
                _ => String::new(),
            }
        }).collect();
        GRow { kind: "project", kids: true, open: true, name: p.name.clone(), cells, bar: Bar { s: day(s.start), f: day(s.finish), ls: day(s.start), lf: day(s.finish), pct: pct.round() as u8, ..Bar::default() }, ..GRow::default() }
    }

    /// A group header with rolled-up values for the rows under it.
    fn group_row(&self, label: String, depth: u8, rows: &[usize], cols: &[String]) -> GRow {
        let (s, e) = (rows.iter().map(|&i| self.s.es[i]).min().unwrap_or(0), rows.iter().map(|&i| self.s.ef[i]).max().unwrap_or(0));
        let first = rows[0];
        let cells = cols.iter().enumerate().map(|(c, k)| match self.kind(k) {
            _ if c == cols.iter().position(|k| k == "name").unwrap_or(0) => label.clone(),
            Kind::Date if matches!(k.as_str(), "start" | "es") => self.format(first, k, &F::Date(s)),
            Kind::Fin if matches!(k.as_str(), "finish" | "ef") => self.format(first, k, &F::Date(e)),
            Kind::Cost | Kind::Work => self.format(first, k, &F::Num(rows.iter().map(|&i| self.val(i, k).num()).sum())),
            _ => String::new(),
        }).collect();
        GRow { kind: "group", lvl: depth, kids: true, open: true, name: label.clone(), cells, bar: Bar { s: day(s), f: day(e), ls: day(s), lf: day(e), ..Bar::default() }, ..GRow::default() }
    }

    pub fn grid(&self, q: &Query) -> Grid {
        let p = self.p;
        let n = p.tasks.len();
        let cols: Vec<String> = q.cols.iter().filter_map(|c| self.key_of(c)).collect();
        let hit: Vec<bool> = (0..n).map(|i| self.matches(i, &q.filter)).collect();
        let mut rows = vec![];
        if q.project_summary {
            rows.push(self.project_row(&cols));
        }
        let shown_hit = |i: usize| q.filter.highlight || q.filter.rules.is_empty() || hit[i];
        if !q.group.is_empty() {
            let mut items: Vec<usize> = (0..n).filter(|&i| !self.s.summary[i] && shown_hit(i)).collect();
            let keys: Vec<String> = q.group.iter().filter_map(|g| self.key_of(g)).collect();
            items.sort_by(|&a, &b| {
                for k in &keys {
                    let o = cmp_f(&self.val(a, k), &self.val(b, k));
                    if o != Ordering::Equal {
                        return o;
                    }
                }
                self.sort_cmp(a, b, &q.sort)
            });
            let mut open: Vec<String> = vec![];
            for (j, &i) in items.iter().enumerate() {
                for (d, k) in keys.iter().enumerate() {
                    let v = self.text(i, k);
                    if open.get(d) != Some(&v) {
                        open.truncate(d);
                        open.push(v.clone());
                        let same: Vec<usize> = items[j..].iter().copied().take_while(|&x| keys[..=d].iter().zip(&open).all(|(k, v)| &self.text(x, k) == v)).collect();
                        let label = format!("{}: {}", self.name(k), if v.is_empty() { "(none)".into() } else { v });
                        rows.push(self.group_row(label, d as u8, &same, &cols));
                    }
                }
                let mut r = self.row(i, &cols, q, q.filter.highlight && hit[i] && !q.filter.rules.is_empty());
                r.lvl = keys.len() as u8;
                rows.push(r);
            }
        } else if q.flat {
            let mut items: Vec<usize> = (0..n).filter(|&i| shown_hit(i) && !(q.hide_summaries && self.s.summary[i])).collect();
            items.sort_by(|&a, &b| self.sort_cmp(a, b, &q.sort));
            rows.extend(items.into_iter().map(|i| {
                let mut r = self.row(i, &cols, q, q.filter.highlight && hit[i] && !q.filter.rules.is_empty());
                r.lvl = 0;
                r
            }));
        } else {
            // Children of each row (and of the root, at index n), sorted within their parent.
            let mut kids: Vec<Vec<usize>> = vec![vec![]; n + 1];
            let mut stack: Vec<usize> = vec![];
            for i in 0..n {
                let lv = p.tasks[i].level as usize;
                stack.truncate(lv);
                kids[stack.last().copied().unwrap_or(n)].push(i);
                stack.push(i);
            }
            if !q.sort.is_empty() {
                kids.iter_mut().for_each(|k| k.sort_by(|&a, &b| self.sort_cmp(a, b, &q.sort)));
            }
            // A summary stays when the filter keeps any task under it.
            let mut keep = vec![false; n];
            for i in (0..n).rev() {
                keep[i] = shown_hit(i) || (q.filter.summaries && self.s.summary[i] && kids[i].iter().any(|&k| keep[k]));
            }
            let mut todo: Vec<usize> = kids[n].iter().rev().copied().collect();
            while let Some(i) = todo.pop() {
                let t = &p.tasks[i];
                let deep = q.level > 0 && t.level >= q.level;
                if keep[i] && !deep && !(q.hide_summaries && self.s.summary[i]) {
                    rows.push(self.row(i, &cols, q, q.filter.highlight && hit[i] && !q.filter.rules.is_empty()));
                }
                let open = !q.collapsed.contains(&t.uid) && !(q.level > 0 && t.level + 1 >= q.level);
                if open || (q.hide_summaries && self.s.summary[i]) {
                    todo.extend(kids[i].iter().rev());
                } else if let Some(r) = rows.last_mut().filter(|r| r.uid == t.uid) {
                    r.open = false;
                }
            }
        }
        let shown = rows.iter().filter(|r| r.uid > 0).count();
        let cols = cols.iter().map(|k| Col {
            key: k.clone(), name: self.name(k), kind: self.kind(k),
            edit: k.starts_with("c:") || k.starts_with("code:") || task_fields().iter().any(|f| &f.key == k && f.edit),
        }).collect();
        Grid { cols, rows, shown, total: n }
    }

    /// Resource Sheet rows for the given columns.
    pub fn res_grid(&self, cols: &[String]) -> Vec<(u32, bool, Vec<String>)> {
        (0..self.p.resources.len()).map(|k| (self.p.resources[k].uid, self.l.over[k], cols.iter().map(|c| res_text(self, k, c)).collect())).collect()
    }

    /// Every field of one task as (key, name, group, text, editable), for Task Information and Inspect.
    pub fn all_fields(&self, i: usize) -> Vec<(String, String, &'static str, String)> {
        let mut v: Vec<(String, String, &'static str, String)> = task_fields().iter().map(|f| (f.key.clone(), self.name(&f.key), f.group, self.text(i, &f.key))).collect();
        v.extend(self.p.codes.iter().map(|c| (format!("code:{}", c.name), c.name.clone(), "Activity codes", self.text(i, &format!("code:{}", c.name)))));
        v
    }
}

/// Built-in filters (rules written in the same form as user filters).
pub fn builtin_filters() -> Vec<Filter> {
    let f = |name: &str, rules: &[(&str, &str, &str)]| Filter {
        name: name.into(), summaries: true, highlight: false,
        rules: rules.iter().map(|(fl, t, v)| Rule { field: fl.to_string(), test: t.to_string(), value: v.to_string(), ..Rule::default() }).collect(),
    };
    vec![
        f("All Tasks", &[]),
        f("Active Tasks", &[("active", "equals", "Yes")]),
        f("Completed Tasks", &[("pct", "equals", "100")]),
        f("Critical", &[("critical", "equals", "Yes")]),
        f("Incomplete Tasks", &[("pct", "is less than", "100")]),
        f("In Progress Tasks", &[("pct", "is greater than", "0"), ("pct", "is less than", "100")]),
        f("Late Tasks", &[("late", "equals", "Yes")]),
        f("Longest Path", &[("lp", "equals", "Yes")]),
        f("Manually Scheduled Tasks", &[("mode", "equals", "Manually Scheduled")]),
        f("Milestones", &[("milestone", "equals", "Yes")]),
        f("Summary Tasks", &[("summary", "equals", "Yes")]),
        f("Tasks With Deadlines", &[("deadline", "does not equal", "")]),
        f("Tasks With Estimated Durations", &[("dur", "contains", "?")]),
        f("Tasks Without Predecessors", &[("preds", "equals", "")]),
        f("Unstarted Tasks", &[("pct", "equals", "0")]),
        f("Work Overbudget", &[("vwork", "is greater than", "0")]),
        f("Cost Overbudget", &[("vcost", "is greater than", "0")]),
        f("Slipped Tasks", &[("vfinish", "is greater than", "0")]),
        f("Negative Float", &[("tf", "is less than", "0")]),
        f("Constrained Tasks", &[("ctype", "does not equal", "As Soon As Possible")]),
    ]
}

/// The project's overall figures for the status bar, dashboards and reports.
#[derive(Serialize)]
pub struct Summary {
    pub name: String,
    pub start: String,
    pub finish: String,
    pub s: f64,
    pub f: f64,
    pub status: String,
    pub status_day: Option<f64>,
    /// Without a status date: the day progress is measured at (today or the latest progress).
    pub measured_at: String,
    pub dur: String,
    pub pct: f64,
    pub work: String,
    pub cost: String,
    pub bl_start: String,
    pub bl_finish: String,
    pub var_finish: String,
    pub pv: String,
    pub ev: String,
    pub ac: String,
    pub spi: Option<f64>,
    pub cpi: Option<f64>,
    pub tasks: usize,
    pub done: usize,
    pub late: usize,
    pub critical: usize,
    pub milestones: usize,
    pub error: Option<String>,
    /// Working weekdays of the project calendar (Monday first) and its whole-day exceptions.
    pub week: [bool; 7],
    pub off_days: Vec<i64>,
}

impl Calc<'_> {
    pub fn summary(&self) -> Summary {
        let (p, s) = (self.p, &self.s);
        let df = |t: i64| date_of(t).format(&self.fmt).to_string();
        let leaves: Vec<usize> = (0..p.tasks.len()).filter(|&i| !s.summary[i] && !s.off[i]).collect();
        let top: Vec<usize> = (0..p.tasks.len()).filter(|&i| p.tasks[i].level == 0 && !s.off[i]).collect();
        let tot = |v: &[f64]| top.iter().map(|&i| v[i]).sum::<f64>();
        let (pv, ev, ac) = (tot(&self.ev.pv), tot(&self.ev.ev), tot(&self.ev.ac));
        let pc = p.calendars.iter().position(|c| c.name == p.calendar).unwrap_or(0);
        let cal = &p.calendars[pc];
        let dur: f64 = leaves.iter().map(|&i| p.tasks[i].dur as f64).sum();
        let bl_end = p.tasks.iter().filter_map(|t| t.bls.get(&0).map(|b| b.finish)).max();
        let bl_begin = p.tasks.iter().filter_map(|t| t.bls.get(&0).map(|b| b.start)).min();
        let mut off_days: Vec<i64> = cal.exceptions.iter().filter(|e| e.hours.is_empty()).flat_map(|e| {
            let (a, b) = (at(e.from) / DAY, at(e.to) / DAY);
            a..=b.min(a + 3700)
        }).collect();
        off_days.sort_unstable();
        let cur = &p.currency;
        let between = |a: i64, b: i64| if b >= a { s.cals[pc].between(a, b) } else { -s.cals[pc].between(b, a) };
        Summary {
            name: p.name.clone(), start: df(s.start), finish: df((s.finish - 1).max(s.start)), s: day(s.start), f: day(s.finish),
            status: p.status.map(|d| d.format(&self.fmt).to_string()).unwrap_or_default(),
            status_day: p.status.map(|d| day(at(d) + DAY)),
            measured_at: if p.status.is_some() { String::new() } else { self.progress_date().format(&self.fmt).to_string() },
            dur: crate::parse::fmt_dur(s.cals[pc].between(s.start, s.finish), Unit::D, p.day_min),
            // Weighted by duration; a plan of milestones only takes the plain average (as summary rows do).
            pct: if dur > 0.0 { leaves.iter().map(|&i| p.tasks[i].pct as f64 * p.tasks[i].dur as f64).sum::<f64>() / dur }
                else if leaves.is_empty() { 0.0 } else { leaves.iter().map(|&i| p.tasks[i].pct as f64).sum::<f64>() / leaves.len() as f64 },
            work: format!("{} hrs", (tot(&self.l.work) / 60.0).round() + 0.0), cost: fields::money(tot(&self.l.cost), cur),
            bl_start: bl_begin.map(df).unwrap_or_default(),
            bl_finish: bl_end.map(|b| df(b - 1)).unwrap_or_default(),
            var_finish: bl_end.map(|b| crate::parse::fmt_dur(between(b, s.finish), Unit::D, p.day_min)).unwrap_or_default(),
            pv: fields::money(pv, cur), ev: fields::money(ev, cur), ac: fields::money(ac, cur),
            spi: (pv > 0.0).then(|| ev / pv), cpi: (ac > 0.0).then(|| ev / ac),
            tasks: leaves.len(), done: leaves.iter().filter(|&&i| p.tasks[i].pct == 100).count(),
            late: leaves.iter().filter(|&&i| self.val(i, "late").truth()).count(),
            critical: leaves.iter().filter(|&&i| s.critical(p, i)).count(),
            milestones: leaves.iter().filter(|&&i| p.tasks[i].dur == 0).count(),
            error: self.error.clone(), week: std::array::from_fn(|k| !cal.week[k].is_empty()), off_days,
        }
    }
}
