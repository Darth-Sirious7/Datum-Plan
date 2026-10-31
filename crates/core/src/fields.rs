//! The field registry: every task and resource field with its name, type and value. Grid columns,
//! filters, groups, sorting, formulas, reports and exports all read fields through here.
use crate::cal::{at, date_of, DAY};
use crate::formula::{self, Ctx, Ex, F};
use crate::load::{self, Loads};
use crate::model::*;
use crate::{parse, sched};
use chrono::NaiveDate;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Value types, which decide formatting, editing and the tests a filter offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Kind { Text, Num, Int, Dur, Date, Fin, Cost, Flag, Pct, Work }

#[derive(Clone, Debug, Serialize)]
pub struct FieldDef {
    pub key: String,
    pub name: String,
    pub kind: Kind,
    pub group: &'static str,
    pub edit: bool,
}

const CUSTOM: [(&str, &str, Kind, usize); 9] = [
    ("text", "Text", Kind::Text, 30), ("number", "Number", Kind::Num, 20), ("flag", "Flag", Kind::Flag, 20), ("date", "Date", Kind::Date, 10),
    ("cost", "Cost", Kind::Cost, 10), ("duration", "Duration", Kind::Dur, 10), ("start", "Start", Kind::Date, 10), ("finish", "Finish", Kind::Fin, 10),
    ("outline", "Outline Code", Kind::Text, 10),
];

/// Built-in task fields (custom fields and activity codes are added per plan).
pub fn task_fields() -> &'static [FieldDef] {
    static F: OnceLock<Vec<FieldDef>> = OnceLock::new();
    F.get_or_init(|| {
        use Kind::*;
        let mut v: Vec<FieldDef> = vec![];
        let mut add = |key: &str, name: &str, kind: Kind, group: &'static str, edit: bool| v.push(FieldDef { key: key.into(), name: name.into(), kind, group, edit });
        for (k, n, kd, e) in [
            ("id", "ID", Int, false), ("uid", "Unique ID", Int, false), ("wbs", "WBS", Text, false), ("outline", "Outline Number", Text, false),
            ("level", "Outline Level", Int, false), ("code", "Activity ID", Text, true), ("name", "Task Name", Text, true), ("notes", "Notes", Text, true),
            ("hyperlink", "Hyperlink", Text, true), ("mode", "Task Mode", Text, true), ("active", "Active", Flag, true), ("summary", "Summary", Flag, false),
            ("milestone", "Milestone", Flag, false), ("critical", "Critical", Flag, false), ("late", "Late", Flag, false), ("timeline", "Display on Timeline", Flag, true),
            ("responsible", "Responsible", Text, true), ("indicators", "Indicators", Text, false),
        ] { add(k, n, kd, "General", e) }
        for (k, n, kd, e) in [
            ("dur", "Duration", Dur, true), ("start", "Start", Date, true), ("finish", "Finish", Fin, true), ("preds", "Predecessors", Text, true),
            ("succs", "Successors", Text, false), ("ctype", "Constraint Type", Text, true), ("cdate", "Constraint Date", Date, true), ("deadline", "Deadline", Fin, true),
            ("cal", "Task Calendar", Text, true), ("type", "Type", Text, true), ("effort", "Effort Driven", Flag, true), ("priority", "Priority", Int, true),
            ("es", "Early Start", Date, false), ("ef", "Early Finish", Fin, false), ("ls", "Late Start", Date, false), ("lf", "Late Finish", Fin, false),
            ("tf", "Total Slack", Dur, false), ("ff", "Free Slack", Dur, false), ("leveldelay", "Leveling Delay", Dur, true), ("acttype", "Activity Type", Text, true),
            ("pcttype", "% Complete Type", Text, true), ("lp", "Longest Path", Flag, false), ("fpath", "Float Path", Int, false), ("elapsed", "Elapsed Duration", Flag, false), ("ignore_rc", "Ignore Resource Calendar", Flag, true), ("ext", "External Predecessors", Text, false),
            ("split", "Split", Flag, false), ("project_finish_var", "Finish Slack", Dur, false),
            ("dmin", "Optimistic Duration", Dur, true), ("dmax", "Pessimistic Duration", Dur, true),
        ] { add(k, n, kd, "Schedule", e) }
        for (k, n, kd, e) in [
            ("pct", "% Complete", Pct, true), ("phys", "Physical % Complete", Pct, true), ("workpct", "% Work Complete", Pct, false), ("astart", "Actual Start", Date, true),
            ("afinish", "Actual Finish", Fin, true), ("adur", "Actual Duration", Dur, true), ("rem", "Remaining Duration", Dur, true), ("status", "Status", Text, false),
            ("qty", "Quantity", Num, true), ("qtydone", "Installed Quantity", Num, true), ("qtyunit", "Quantity Unit", Text, true), ("steps", "Steps", Int, false), ("plannedpct", "Baseline % Complete", Pct, false), ("pctvar", "% Complete Variance", Pct, false),
        ] { add(k, n, kd, "Progress", e) }
        for (k, n, kd, e) in [
            ("work", "Work", Work, true), ("awork", "Actual Work", Work, false), ("rwork", "Remaining Work", Work, false), ("cost", "Cost", Cost, false),
            ("acost", "Actual Cost", Cost, false), ("rcost", "Remaining Cost", Cost, false), ("fixedcost", "Fixed Cost", Cost, true), ("accrual", "Fixed Cost Accrual", Text, true),
            ("expenses", "Expenses", Cost, false), ("costaccount", "Cost Account", Text, true), ("res", "Resource Names", Text, true), ("initials", "Resource Initials", Text, false),
        ] { add(k, n, kd, "Work and cost", e) }
        for b in 0..=10u8 {
            let nm = if b == 0 { "Baseline".to_string() } else { format!("Baseline{b}") };
            for (k, n, kd) in [("start", "Start", Date), ("finish", "Finish", Fin), ("dur", "Duration", Dur), ("cost", "Cost", Cost), ("work", "Work", Work)] {
                add(&format!("b{b}{k}"), &format!("{nm} {n}"), kd, "Baselines", false);
            }
        }
        for (k, n, kd) in [("vstart", "Start Variance", Dur), ("vfinish", "Finish Variance", Dur), ("vdur", "Duration Variance", Dur), ("vcost", "Cost Variance", Cost), ("vwork", "Work Variance", Work)] {
            add(k, n, kd, "Variance", false);
        }
        for (k, n, kd) in [
            ("bcws", "Planned Value - PV (BCWS)", Cost), ("bcwp", "Earned Value - EV (BCWP)", Cost), ("acwp", "AC (ACWP)", Cost), ("sv", "SV", Cost), ("cv", "CV", Cost),
            ("spi", "SPI", Num), ("cpi", "CPI", Num), ("bac", "BAC", Cost), ("eac", "EAC", Cost), ("vac", "VAC", Cost), ("tcpi", "TCPI", Num),
            ("svpct", "SV%", Pct), ("cvpct", "CV%", Pct),
        ] { add(k, n, kd, "Earned value", false) }
        for (pre, nm, kd, count) in CUSTOM {
            for k in 1..=count { add(&format!("c:{pre}{k}"), &format!("{nm}{k}"), kd, "Custom", true) }
        }
        v
    })
}

/// Resource fields (Resource Sheet, Resource Usage).
pub fn resource_fields() -> &'static [FieldDef] {
    static F: OnceLock<Vec<FieldDef>> = OnceLock::new();
    F.get_or_init(|| {
        use Kind::*;
        let mut v: Vec<FieldDef> = [
            ("id", "ID", Int, false), ("name", "Resource Name", Text, true), ("kind", "Type", Text, true), ("initials", "Initials", Text, true),
            ("group", "Group", Text, true), ("max", "Max Units", Pct, true), ("rate", "Std. Rate", Cost, true), ("ovt", "Ovt. Rate", Cost, true),
            ("per_use", "Cost/Use", Cost, true), ("label", "Material Label", Text, true), ("code", "Code", Text, true), ("email", "Email Address", Text, true), ("country", "Country", Text, true),
            ("cal", "Base Calendar", Text, true), ("work", "Work", Work, false), ("cost", "Cost", Cost, false), ("over", "Overallocated", Flag, false),
            ("notes", "Notes", Text, true),
        ].into_iter().map(|(k, n, kd, e)| FieldDef { key: k.into(), name: n.into(), kind: kd, group: "Resource", edit: e }).collect();
        for (pre, nm, kd, count) in CUSTOM {
            for k in 1..=count { v.push(FieldDef { key: format!("c:r{pre}{k}"), name: format!("{nm}{k}"), kind: kd, group: "Custom", edit: true }) }
        }
        v
    })
}

/// Earned value per task (summaries sum their leaves).
#[derive(Default)]
pub struct Ev { pub pv: Vec<f64>, pub ev: Vec<f64>, pub ac: Vec<f64>, pub bac: Vec<f64> }

/// Everything needed to read any field of any row.
pub struct Calc<'a> {
    pub p: &'a Project,
    pub s: sched::Sched,
    pub l: Loads,
    pub ev: Ev,
    pub error: Option<String>,
    pub fmt: String,
    pub today: NaiveDate,
    pub pct: Vec<u8>,
    pub wbs: Vec<String>,
    pub id_of: HashMap<u32, usize>,
    pub succs: Vec<Vec<String>>,
    defs: HashMap<String, (CustomDef, Option<Ex>)>,
    names: HashMap<String, String>,
    /// Results of Excel-style formula columns, per field key and task.
    xl: HashMap<String, Vec<F>>,
}

pub fn money(v: f64, c: &Currency) -> String {
    let s = format!("{:.*}", c.decimals as usize, v.abs());
    let (int, dec) = match s.find('.') { Some(k) => s.split_at(k), None => (s.as_str(), "") };
    let mut out = String::new();
    for (k, ch) in int.chars().enumerate() {
        if k > 0 && (int.len() - k) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    let sign = if v < -1e-9 { "-" } else { "" };
    if c.after { format!("{sign}{out}{dec} {}", c.symbol) } else { format!("{sign}{}{out}{dec}", c.symbol) }
}

fn num(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

impl<'a> Calc<'a> {
    pub fn new(p: &'a Project, fmt: &str, today: NaiveDate) -> Self {
        let (s, error) = match sched::schedule(p) {
            Ok(s) => (s, None),
            Err(e) => {
                let mut q = p.clone();
                q.tasks.iter_mut().for_each(|t| t.preds.clear());
                (sched::schedule(&q).expect("a plan without links always schedules"), Some(e))
            }
        };
        let l = load::compute(p, &s);
        let n = p.tasks.len();
        let id_of: HashMap<u32, usize> = p.tasks.iter().enumerate().map(|(i, t)| (t.uid, i + 1)).collect();
        let mut succs: Vec<Vec<String>> = vec![vec![]; n];
        for (i, t) in p.tasks.iter().enumerate() {
            for lk in &t.preds {
                if let Some(&k) = id_of.get(&lk.pred) {
                    succs[k - 1].push(parse::fmt_link(i + 1, lk, p.day_min));
                }
            }
        }
        let mut stack: Vec<u32> = vec![];
        let wbs = p.tasks.iter().map(|t| {
            let lv = t.level as usize;
            stack.truncate(lv + 1);
            stack.resize(lv + 1, 0);
            stack[lv] += 1;
            stack.iter().map(u32::to_string).collect::<Vec<_>>().join(".")
        }).collect();
        let pct = (0..n).map(|i| {
            if !s.summary[i] {
                return p.tasks[i].pct;
            }
            let kids: Vec<&Task> = p.subtree(i).skip(1).filter(|&j| !s.summary[j] && !s.off[j]).map(|j| &p.tasks[j]).collect();
            let w: i64 = kids.iter().map(|k| k.dur).sum();
            if w > 0 { (kids.iter().map(|k| k.pct as f64 * k.dur as f64).sum::<f64>() / w as f64).round() as u8 } else { (kids.iter().map(|k| k.pct as f64).sum::<f64>() / kids.len().max(1) as f64).round() as u8 }
        }).collect();
        let defs = p.fields.iter().map(|d| (d.key.clone(), (d.clone(), formula::parse(&d.formula).ok().filter(|_| !d.formula.trim().is_empty())))).collect();
        let mut names: HashMap<String, String> = task_fields().iter().map(|f| (f.name.to_lowercase(), f.key.clone())).collect();
        for (k, a) in [("planned % complete", "plannedpct"), ("percent complete", "pct"), ("total slack", "tf"), ("free slack", "ff"), ("total float", "tf"), ("free float", "ff"), ("name", "name"), ("resource names", "res")] {
            names.insert(k.into(), a.into());
        }
        for d in &p.fields {
            if !d.alias.is_empty() {
                names.insert(d.alias.to_lowercase(), format!("c:{}", d.key));
            }
        }
        for c in &p.codes {
            names.insert(c.name.to_lowercase(), format!("code:{}", c.name));
        }
        for (k, t) in &p.titles {
            names.insert(t.to_lowercase(), k.clone());
        }
        let mut c = Calc { p, s, l, ev: Ev::default(), error, fmt: fmt.into(), today, pct, wbs, id_of, succs, defs, names, xl: HashMap::new() };
        c.ev = c.earned_value();
        let excel: Vec<(String, String)> = p.fields.iter().filter(|d| crate::sheet::is_excel(&d.formula) && !d.key.starts_with('r')).map(|d| (d.key.clone(), d.formula.clone())).collect();
        if !excel.is_empty() {
            c.xl = crate::sheet::task_formulas(&c, &excel);
        }
        c
    }

    /// The status (data) instant: end of the status date, or now.
    pub fn status_at(&self) -> i64 {
        at(self.p.status.unwrap_or(self.today)) + DAY
    }

    /// The end of the day progress reports (Dashboard, period reports, S-curves) are measured at.
    pub fn progress_at(&self) -> i64 {
        at(self.progress_date()) + DAY
    }

    /// The day progress is measured at: the status date, or without one the later of today and the
    /// latest progress recorded (so a plan updated ahead of time does not show 0 % done).
    pub fn progress_date(&self) -> NaiveDate {
        let (p, s) = (self.p, &self.s);
        p.status.unwrap_or_else(|| {
            let done = (0..p.tasks.len()).filter(|&i| self.pct[i] > 0 && !s.summary[i]).map(|i| {
                let t = &p.tasks[i];
                t.act_finish.or(if self.pct[i] < 100 { None } else { Some(date_of((s.ef[i] - 1).max(s.es[i]))) })
                    .unwrap_or_else(|| date_of(s.es[i] + (s.ef[i] - s.es[i]) * self.pct[i] as i64 / 100)).max(t.act_start.unwrap_or_default())
            });
            done.fold(self.today, NaiveDate::max)
        })
    }

    fn earned_value(&self) -> Ev {
        let (p, s) = (self.p, &self.s);
        let n = p.tasks.len();
        let mut e = Ev { pv: vec![0.0; n], ev: vec![0.0; n], ac: vec![0.0; n], bac: vec![0.0; n] };
        let dd = self.progress_at();
        for i in (0..n).filter(|&i| !s.summary[i] && !s.off[i]) {
            let t = &p.tasks[i];
            let Some(b) = t.bls.get(&0) else { continue };
            let c = &s.cals[s.ci[i]];
            let frac = if dd <= b.start { 0.0 } else if dd >= b.finish { 1.0 } else { c.between(b.start, dd) as f64 / c.between(b.start, b.finish).max(1) as f64 };
            let done = if t.pct_type == PctType::Physical { t.phys } else { t.pct } as f64 / 100.0;
            e.bac[i] = b.cost;
            e.pv[i] = b.cost * frac;
            e.ev[i] = b.cost * done;
            e.ac[i] = self.l.acost[i];
        }
        for i in (0..n).filter(|&i| s.summary[i]) {
            for j in p.subtree(i).skip(1).filter(|&j| !s.summary[j]) {
                e.pv[i] += e.pv[j];
                e.ev[i] += e.ev[j];
                e.ac[i] += e.ac[j];
                e.bac[i] += e.bac[j];
            }
        }
        e
    }

    /// Looks a field up by key, display name or alias.
    pub fn key_of(&self, name: &str) -> Option<String> {
        let n = name.trim().to_lowercase();
        if task_fields().iter().any(|f| f.key == n) || n.starts_with("c:") || n.starts_with("code:") {
            return Some(n);
        }
        self.names.get(&n).cloned()
    }

    pub fn kind(&self, key: &str) -> Kind {
        if key.starts_with("code:") {
            return Kind::Text;
        }
        task_fields().iter().find(|f| f.key == key).map_or(Kind::Text, |f| f.kind)
    }

    /// The display name (custom field alias where set).
    pub fn name(&self, key: &str) -> String {
        if let Some(t) = self.p.titles.get(key) {
            return t.clone();
        }
        if let Some((d, _)) = key.strip_prefix("c:").and_then(|k| self.defs.get(k)) {
            if !d.alias.is_empty() {
                return d.alias.clone();
            }
        }
        key.strip_prefix("code:").map(String::from).unwrap_or_else(|| task_fields().iter().find(|f| f.key == key).map_or(key.to_string(), |f| f.name.clone()))
    }

    fn dur_between(&self, i: usize, a: i64, b: i64) -> i64 {
        let c = &self.s.cals[self.s.ci[i]];
        if b >= a { c.between(a, b) } else { -c.between(b, a) }
    }

    /// The value of field `key` for task index `i`.
    pub fn val(&self, i: usize, key: &str) -> F {
        let (p, s, l) = (self.p, &self.s, &self.l);
        let t = &p.tasks[i];
        let leaf = !s.summary[i];
        let d = |x: i64| F::Date(x);
        let opt = |x: Option<NaiveDate>| x.map_or(F::Null, |d| F::Date(at(d)));
        let optf = |x: Option<NaiveDate>| x.map_or(F::Null, |d| F::Date(at(d) + 17 * 60));
        let txt = |x: &str| F::Str(x.into());
        let dur = if s.summary[i] || s.loe[i] || s.mirror[i] { s.cals[s.ci[i]].between(s.es[i], s.ef[i]) } else { t.dur };
        if let Some(k) = key.strip_prefix("c:") {
            return self.custom(i, k);
        }
        if let Some(c) = key.strip_prefix("code:") {
            return t.codes.get(c).map_or(F::Null, |v| txt(v));
        }
        if let Some(rest) = key.strip_prefix('b').filter(|r| r.starts_with(|c: char| c.is_ascii_digit())) {
            let cut = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
            let n: u8 = rest[..cut].parse().unwrap_or(0);
            let Some(b) = t.bls.get(&n) else { return F::Null };
            return match &rest[cut..] { "start" => d(b.start), "finish" => d(b.finish), "dur" => F::Num(b.dur as f64), "cost" => F::Num(b.cost), "work" => F::Num(b.work.unwrap_or(l.work[i] * b.dur as f64 / dur.max(1) as f64) / 60.0), _ => F::Null };
        }
        match key {
            "id" => F::Num(i as f64 + 1.0),
            "uid" => F::Num(t.uid as f64),
            "wbs" | "outline" => txt(&self.wbs[i]),
            "level" => F::Num(t.level as f64 + 1.0),
            "code" => txt(&t.code),
            "name" => txt(&t.name),
            "notes" => txt(&t.notes),
            "hyperlink" => txt(&t.hyperlink),
            "mode" => txt(if t.manual { "Manually Scheduled" } else { "Auto Scheduled" }),
            "active" => F::Bool(!s.off[i]),
            "summary" => F::Bool(s.summary[i]),
            "milestone" => F::Bool(leaf && t.dur == 0),
            "critical" => F::Bool(s.critical(p, i)),
            "late" => F::Bool(leaf && !s.off[i] && self.pct[i] < 100 && date_of((s.ef[i] - 1).max(s.es[i])) <= p.status.unwrap_or(self.today.pred_opt().unwrap())),
            "timeline" => F::Bool(t.timeline),
            "responsible" => txt(&t.responsible),
            "indicators" => txt(""),
            "dur" => F::Num(dur as f64),
            "start" | "es" => d(s.es[i]),
            "finish" | "ef" => d(s.ef[i]),
            "ls" => d(s.ls[i]),
            "lf" => d(s.lf[i]),
            "preds" => txt(&parse::fmt_preds(&t.preds, &self.id_of, p.day_min)),
            "succs" => txt(&self.succs[i].join(", ")),
            "ctype" => txt(CONSTRAINTS.iter().find(|(k, _)| *k == t.constraint).map_or("", |(_, n)| n)),
            "cdate" => opt(t.cdate),
            "deadline" => optf(t.deadline),
            "cal" => txt(t.cal.as_deref().unwrap_or("")),
            "type" => txt(match t.task_type { TaskType::FixedDuration => "Fixed Duration", TaskType::FixedUnits => "Fixed Units", TaskType::FixedWork => "Fixed Work" }),
            "effort" => F::Bool(t.effort),
            "priority" => F::Num(p.priority(i) as f64),
            "tf" => F::Num(s.tf[i] as f64),
            "ff" => F::Num(s.ff[i] as f64),
            "leveldelay" => F::Num(t.level_delay as f64),
            "dmin" | "dmax" => match if key == "dmin" { t.dmin } else { t.dmax } { 0 => F::Null, m => F::Num(m as f64) },
            "acttype" => txt(match t.act_type { ActType::Task => "Task Dependent", ActType::ResourceDep => "Resource Dependent", ActType::Loe => "Level of Effort", ActType::StartMs => "Start Milestone", ActType::FinishMs => "Finish Milestone", ActType::WbsSummary => "WBS Summary" }),
            "pcttype" => txt(match t.pct_type { PctType::Duration => "Duration", PctType::Units => "Units", PctType::Physical => "Physical" }),
            "lp" => F::Bool(s.lp[i]),
            "fpath" => if s.fpath[i] > 0 { F::Num(s.fpath[i] as f64) } else { F::Null },
            "elapsed" => F::Bool(t.elapsed),
            "ignore_rc" => F::Bool(t.ignore_rc),
            "ext" => txt(&t.ext.iter().map(|x| crate::links::label(x, p.day_min)).collect::<Vec<_>>().join("; ")),
            "split" => F::Bool(!t.splits.is_empty()),
            "project_finish_var" => F::Num(self.dur_between(i, s.ef[i], s.finish) as f64),
            "pct" => F::Num(self.pct[i] as f64),
            "phys" => F::Num(t.phys as f64),
            "workpct" => F::Num(if l.work[i] > 0.0 { (l.awork[i] / l.work[i] * 100.0).round() } else { self.pct[i] as f64 }),
            "astart" => opt(t.act_start),
            "afinish" => optf(t.act_finish),
            "adur" => F::Num((dur * self.pct[i] as i64 / 100) as f64),
            "rem" => F::Num((dur * (100 - self.pct[i] as i64) / 100) as f64),
            "status" => txt(if self.pct[i] == 100 { "Complete" } else if s.es[i] >= self.status_at() { "Future Task" } else if self.val(i, "late").truth() { "Late" } else { "On Schedule" }),
            "steps" => F::Num(t.steps.len() as f64),
            "qty" => if t.qty > 0.0 { F::Num(t.qty) } else { F::Null },
            "qtydone" => if t.qty > 0.0 || t.qty_done > 0.0 { F::Num(t.qty_done) } else { F::Null },
            "qtyunit" => txt(&t.qty_unit),
            "plannedpct" | "pctvar" => {
                // Where the baseline (or, without one, the current plan) says the task should be at the status date.
                let (a, b) = t.bls.get(&0).map_or((s.es[i], s.ef[i]), |b| (b.start, b.finish));
                let (dd, c) = (self.progress_at(), &s.cals[s.ci[i]]);
                let planned = if dd <= a { 0.0 } else if dd >= b { 100.0 } else { (c.between(a, dd) as f64 / c.between(a, b).max(1) as f64 * 100.0).round() };
                F::Num(if key == "pctvar" { self.pct[i] as f64 - planned } else { planned })
            }
            "work" => F::Num(l.work[i] / 60.0),
            "awork" => F::Num(l.awork[i] / 60.0),
            "rwork" => F::Num((l.work[i] - l.awork[i]) / 60.0),
            "cost" => F::Num(l.cost[i]),
            "acost" => F::Num(l.acost[i]),
            "rcost" => F::Num(l.cost[i] - l.acost[i]),
            "fixedcost" => F::Num(t.fixed_cost),
            "accrual" => txt(match t.accrual { Accrual::Start => "Start", Accrual::Prorated => "Prorated", Accrual::End => "End" }),
            "expenses" => F::Num(t.expenses.iter().map(|e| e.planned.max(e.actual)).sum()),
            "costaccount" => txt(&t.cost_account),
            "res" => txt(&parse::fmt_res(&t.res, p)),
            "initials" => txt(&t.res.iter().filter_map(|a| p.res_idx(a.res)).map(|k| { let r = &p.resources[k]; if r.initials.is_empty() { r.name.chars().take(1).collect() } else { r.initials.clone() } }).collect::<Vec<String>>().join(",")),
            "vstart" | "vfinish" | "vdur" | "vcost" | "vwork" => {
                let Some(b) = t.bls.get(&0) else { return F::Null };
                F::Num(match key {
                    "vstart" => self.dur_between(i, b.start, s.es[i]) as f64,
                    "vfinish" => self.dur_between(i, b.finish, s.ef[i]) as f64,
                    "vdur" => (dur - b.dur) as f64,
                    "vcost" => l.cost[i] - b.cost,
                    _ => (l.work[i] - b.work.unwrap_or(l.work[i] * b.dur as f64 / dur.max(1) as f64)) / 60.0,
                })
            }
            "bcws" => F::Num(self.ev.pv[i]),
            "bcwp" => F::Num(self.ev.ev[i]),
            "acwp" => F::Num(self.ev.ac[i]),
            "sv" => F::Num(self.ev.ev[i] - self.ev.pv[i]),
            "cv" => F::Num(self.ev.ev[i] - self.ev.ac[i]),
            "spi" => if self.ev.pv[i] > 0.0 { F::Num(self.ev.ev[i] / self.ev.pv[i]) } else { F::Null },
            "cpi" => if self.ev.ac[i] > 0.0 { F::Num(self.ev.ev[i] / self.ev.ac[i]) } else { F::Null },
            "bac" => F::Num(self.ev.bac[i]),
            "eac" => F::Num(self.eac(i)),
            "vac" => F::Num(self.ev.bac[i] - self.eac(i)),
            "tcpi" => { let r = self.ev.bac[i] - self.ev.ac[i]; if r.abs() > 1e-9 { F::Num((self.ev.bac[i] - self.ev.ev[i]) / r) } else { F::Null } }
            "svpct" => if self.ev.pv[i] > 0.0 { F::Num((self.ev.ev[i] - self.ev.pv[i]) / self.ev.pv[i] * 100.0) } else { F::Null },
            "cvpct" => if self.ev.ev[i] > 0.0 { F::Num((self.ev.ev[i] - self.ev.ac[i]) / self.ev.ev[i] * 100.0) } else { F::Null },
            _ => F::Null,
        }
    }

    fn eac(&self, i: usize) -> f64 {
        let (bac, ev, ac) = (self.ev.bac[i], self.ev.ev[i], self.ev.ac[i]);
        let cpi = if ac > 0.0 { ev / ac } else { 1.0 };
        if bac == 0.0 { self.l.cost[i] } else { ac + (bac - ev) / if cpi > 0.0 { cpi } else { 1.0 } }
    }

    fn custom(&self, i: usize, key: &str) -> F {
        let t = &self.p.tasks[i];
        let def = self.defs.get(key);
        let kind = self.kind(&format!("c:{key}"));
        let summary = self.s.summary[i];
        if let Some((d, _)) = def.filter(|(d, _)| summary && !matches!(d.rollup, Rollup::None | Rollup::Formula)) {
            let vals: Vec<f64> = self.p.subtree(i).skip(1).filter(|&j| !self.s.summary[j]).map(|j| self.custom(j, key)).filter(|v| *v != F::Null).map(|v| v.num()).collect();
            if vals.is_empty() {
                return F::Null;
            }
            let x = match d.rollup {
                Rollup::Max => vals.iter().cloned().fold(f64::MIN, f64::max),
                Rollup::Min => vals.iter().cloned().fold(f64::MAX, f64::min),
                Rollup::Sum => vals.iter().sum(),
                Rollup::Average => vals.iter().sum::<f64>() / vals.len() as f64,
                _ => vals.len() as f64,
            };
            return if matches!(kind, Kind::Date | Kind::Fin) { F::Date(x as i64 * 1440) } else { F::Num(x) };
        }
        // A finish field holds the end of its day; a formula giving a date only means that day.
        let fin = |v: F| match v { F::Date(d) if kind == Kind::Fin && d.rem_euclid(crate::cal::DAY) == 0 => F::Date(d + 17 * 60), v => v };
        if let Some(v) = self.xl.get(key) {
            return fin(v.get(i).cloned().unwrap_or(F::Null));
        }
        if let Some((_, Some(ex))) = def {
            // Summary rows use the formula too unless a rollup is chosen (handled above).
            return fin(formula::eval(ex, &Row { c: self, i }).unwrap_or_else(|e| F::Str(format!("#ERROR: {e}"))));
        }
        match t.custom.get(key) {
            None | Some(serde_json::Value::Null) => def.filter(|(d, _)| !d.default.is_empty()).map_or(F::Null, |(d, _)| F::Str(d.default.clone())),
            Some(serde_json::Value::Bool(b)) => F::Bool(*b),
            Some(serde_json::Value::Number(n)) => F::Num(n.as_f64().unwrap_or(0.0)),
            Some(serde_json::Value::String(s)) if matches!(kind, Kind::Date | Kind::Fin) => NaiveDate::parse_from_str(s, "%Y-%m-%d").map_or(F::Null, |d| F::Date(at(d) + if kind == Kind::Fin { 17 * 60 } else { 0 })),
            Some(v) => F::Str(v.as_str().map(String::from).unwrap_or_else(|| v.to_string())),
        }
    }

    /// The text shown in a cell.
    pub fn text(&self, i: usize, key: &str) -> String {
        let v = self.val(i, key);
        self.format(i, key, &v)
    }

    pub fn format(&self, i: usize, key: &str, v: &F) -> String {
        let p = self.p;
        let t = &p.tasks[i];
        match (self.kind(key), v) {
            (_, F::Null) => String::new(),
            (_, F::Str(s)) => s.clone(),
            (Kind::Flag, x) => if x.truth() { "Yes".into() } else { "No".into() },
            (Kind::Dur, x) => {
                let m = x.num().round() as i64;
                if matches!(key, "dur" | "rem" | "adur") && t.elapsed { parse::fmt_edur(m, t.unit) }
                else { parse::fmt_dur(m, if matches!(key, "dur" | "rem" | "adur") && !self.s.summary[i] { t.unit } else { Unit::D }, p.day_min) }
            }
            (Kind::Date, F::Date(d)) => date_of(*d).format(&self.fmt).to_string(),
            (Kind::Fin, F::Date(d)) => {
                let ms = !self.s.summary[i] && t.dur == 0 && matches!(key, "finish" | "ef");
                date_of(if ms || d.rem_euclid(DAY) != 0 { *d } else { d - 1 }).format(&self.fmt).to_string()
            }
            (Kind::Cost, x) => money(x.num(), &p.currency),
            (Kind::Work, x) => format!("{} hrs", num(x.num())),
            (Kind::Pct, x) => format!("{}%", x.num().round()),
            (Kind::Int, x) => format!("{}", x.num().round() as i64),
            (Kind::Num, x) => num(x.num()),
            (_, x) => x.to_string(),
        }
    }

    /// Is the value calculated rather than typed? (Shown differently in the grid.)
    pub fn calculated(&self, i: usize, key: &str) -> bool {
        if self.s.summary[i] {
            return !matches!(key, "name" | "notes" | "code" | "hyperlink" | "responsible" | "costaccount" | "priority");
        }
        if let Some(k) = key.strip_prefix("c:") {
            return self.defs.get(k).is_some_and(|(d, _)| !d.formula.is_empty());
        }
        !task_fields().iter().find(|f| f.key == key).is_some_and(|f| f.edit)
    }

    /// The indicator icon a custom field shows for row `i`, if any rule matches.
    pub fn icon(&self, i: usize, key: &str) -> Option<String> {
        let (d, _) = self.defs.get(key.strip_prefix("c:")?)?;
        let scope = if self.s.summary[i] { 1 } else { 0 };
        let v = self.val(i, key);
        let kind = self.kind(key);
        d.indicators.iter().filter(|r| r.scope == scope).find(|r| {
            let target = if r.value.starts_with('[') { self.key_of(r.value.trim_matches(['[', ']'])).map(|k| self.text(i, &k)).unwrap_or_default() } else { r.value.clone() };
            test(&v, kind, &r.test, &target, "", self)
        }).map(|r| r.icon.clone())
    }

    /// A row-level formula context.
    pub fn ctx(&self, i: usize) -> impl Ctx + '_ { Row { c: self, i } }
}

struct Row<'a, 'b> { c: &'b Calc<'a>, i: usize }

impl Ctx for Row<'_, '_> {
    fn field(&self, name: &str) -> Option<F> {
        let k = self.c.key_of(name)?;
        Some(self.c.val(self.i, &k))
    }
    fn now(&self) -> i64 { at(self.c.today) }
    fn work_between(&self, a: i64, b: i64) -> i64 { self.c.dur_between(self.i, a, b) }
    fn work_add(&self, a: i64, m: i64) -> i64 { self.c.s.cals[self.c.s.ci[self.i]].add(a, m) }
}

/// Parses filter text into a value of the field's kind.
pub fn parse_value(text: &str, kind: Kind, c: &Calc) -> F {
    let t = text.trim();
    if t.is_empty() {
        return F::Null;
    }
    match kind {
        Kind::Date | Kind::Fin => parse::date_in(t, &c.fmt).map_or(F::Str(t.into()), |d| F::Date(at(d))),
        Kind::Dur => parse::dur(t, c.p.day_min).map_or(F::Str(t.into()), |x| F::Num(x.0 as f64)),
        Kind::Flag => F::Bool(matches!(t.to_lowercase().as_str(), "yes" | "true" | "1")),
        Kind::Num | Kind::Int | Kind::Cost | Kind::Pct | Kind::Work => t.trim_end_matches('%').replace([',', '$'], "").parse().map_or(F::Str(t.into()), F::Num),
        Kind::Text => F::Str(t.into()),
    }
}

/// A filter or indicator test: equals, does not equal, is greater than (or equal to), is less than
/// (or equal to), is within / is not within (value and value2), contains, does not contain,
/// contains exactly, is any value.
pub fn test(v: &F, kind: Kind, test: &str, a: &str, b: &str, c: &Calc) -> bool {
    let t = test.to_lowercase();
    let text = |x: &F| match x { F::Date(d) => date_of(*d).to_string(), F::Bool(b) => if *b { "yes".into() } else { "no".into() }, x => x.to_string().to_lowercase() };
    let cmp = |x: &F, y: &F| -> std::cmp::Ordering {
        match (x, y) {
            (F::Date(p), F::Date(q)) => date_of(*p).cmp(&date_of(*q)),
            (F::Str(p), F::Str(q)) => p.to_lowercase().cmp(&q.to_lowercase()),
            _ => x.num().partial_cmp(&y.num()).unwrap_or(std::cmp::Ordering::Equal),
        }
    };
    if t.contains("any value") {
        return true;
    }
    if t.contains("contain") {
        let (hay, needle) = (text(v), a.to_lowercase());
        return if t.contains("exactly") { hay.split([',', ';']).any(|x| x.trim() == needle) } else if t.contains("not") { !hay.contains(&needle) } else { hay.contains(&needle) };
    }
    let x = parse_value(a, kind, c);
    if t.contains("within") {
        let y = parse_value(b, kind, c);
        let inside = *v != F::Null && cmp(v, &x) != std::cmp::Ordering::Less && cmp(v, &y) != std::cmp::Ordering::Greater;
        return if t.contains("not") { !inside } else { inside };
    }
    use std::cmp::Ordering::*;
    let o = if *v == F::Null && x == F::Null { Equal } else if *v == F::Null || x == F::Null { if *v == F::Null { Less } else { Greater } } else { cmp(v, &x) };
    let eq = if matches!(kind, Kind::Text) { text(v) == a.trim().to_lowercase() } else { o == Equal };
    match t.as_str() {
        x if x.contains("not equal") => !eq,
        x if x.contains("greater") && x.contains("equal") => o != Less,
        x if x.contains("greater") => o == Greater,
        x if x.contains("less") && x.contains("equal") => o != Greater,
        x if x.contains("less") => o == Less,
        _ => eq,
    }
}

/// Resource field values for the Resource Sheet and usage views.
pub fn res_text(c: &Calc, k: usize, key: &str) -> String {
    let p = c.p;
    let r = &p.resources[k];
    let cur = &p.currency;
    match key {
        "id" => (k + 1).to_string(),
        "name" => r.name.clone(),
        "kind" => format!("{:?}", r.kind),
        "initials" => r.initials.clone(),
        "group" => r.group.clone(),
        "max" => if r.kind == ResKind::Work { format!("{}%", (r.max * 100.0).round()) } else { String::new() },
        "rate" => format!("{}{}", money(r.rate, cur), if r.kind == ResKind::Work { "/hr" } else { "" }),
        "ovt" => if r.kind == ResKind::Work { format!("{}/hr", money(r.ovt, cur)) } else { String::new() },
        "per_use" => money(r.per_use, cur),
        "label" => r.label.clone(),
        "code" => r.code.clone(),
        "email" => r.email.clone(),
        "country" => r.country.clone(),
        "cal" => r.cal.clone().unwrap_or_default(),
        "work" => format!("{} hrs", num(c.l.res_work[k] / 60.0)),
        "cost" => money(c.l.res_cost[k], cur),
        "over" => if c.l.over[k] { "Yes".into() } else { "No".into() },
        "notes" => r.notes.clone(),
        k if k.starts_with("c:") => r.custom.get(&k[2..]).map(|v| v.as_str().map(String::from).unwrap_or_else(|| v.to_string())).unwrap_or_default(),
        _ => String::new(),
    }
}
