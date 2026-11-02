//! An open plan: edit operations, undo/redo and .dplan files.
use crate::{cal::{at, date_of, DAY}, load, model::*, parse, sched};
use chrono::NaiveDate;
use serde::Deserialize;
use std::{collections::{HashMap, HashSet}, path::{Path, PathBuf}};

#[derive(Debug, Clone, Deserialize)]
pub enum Op {
    Insert { at: usize, name: String, ms: bool },
    /// New tasks from (field, value) pairs, such as pasted spreadsheet rows.
    InsertRows { at: usize, rows: Vec<Vec<(String, String)>> },
    QuickAdd { at: usize, text: String },
    Paste { at: usize, tasks: Vec<Task> },
    Delete { uids: Vec<u32> },
    Set { uid: u32, field: String, value: String },
    Indent { uids: Vec<u32> },
    Outdent { uids: Vec<u32> },
    Move { uids: Vec<u32>, up: bool },
    Link { uids: Vec<u32> },
    /// Resource leveling of the given tasks (all when empty), or clearing it.
    Level { uids: Vec<u32>, clear: bool },
    /// Splits a task: a pause of `gap` working minutes after `after` working minutes of work.
    Split { uid: u32, after: i64, gap: i64 },
    /// Saves current start/finish into interim plan n (custom start/finish n).
    Interim { n: u8 },
    /// Marks work complete through a date (Update Project).
    UpdateProject { through: String, reschedule: bool },
    /// Freezes the plan at the status date (period performance).
    Snapshot { label: String, comment: String },
    /// Sets a project-level value (setup, currency, branding, options) from JSON.
    ProjectSet { field: String, value: serde_json::Value },
    /// Replaces a custom field definition (empty alias and formula with no values removes it).
    FieldDef { def: CustomDef },
    /// Deletes a custom field: its definition and every task's value.
    DeleteField { key: String },
    CodeDef { old: Option<String>, def: Option<CodeDef> },
    /// Adds, changes or (when `risk` is None) removes a risk by id.
    Risk { id: u32, risk: Option<Risk> },
    /// Sets a task list (steps, notebook, expenses, docs) from JSON.
    TaskList { uid: u32, field: String, value: serde_json::Value },
    /// Sets a resource list or value (rate tables, availability, custom) from JSON.
    ResJson { uid: u32, field: String, value: serde_json::Value },
    /// Stores window settings (views, tables, filters, styles, reports) under a key.
    Ui { key: String, value: serde_json::Value },
    /// Adds, changes or (with no kind) removes the link from `pred` to `succ`.
    LinkSet { pred: u32, succ: u32, kind: Option<LinkType>, lag: String },
    Unlink { uids: Vec<u32> },
    MarkOnTrack { uids: Vec<u32> },
    /// Saves (or clears) the baseline of the given tasks, or of all tasks when empty.
    Baseline { uids: Vec<u32>, clear: bool, #[serde(default)] n: u8 },
    Project { name: String, start: String, status: String, calendar: String, day_hours: f64, manual_default: bool },
    CalPut { old: Option<String>, cal: Calendar },
    CalDelete { name: String },
    ResInsert { name: String },
    ResSet { uid: u32, field: String, value: String },
    ResDelete { uids: Vec<u32> },
    Columns { cols: Vec<(String, i32)> },
    /// Many cell edits as one step (paste, fill, clear a range). Cells that cannot take the value
    /// are skipped; it fails only when none could be set.
    Cells { cells: Vec<(u32, String, String)> },
    /// Inserts a recurring task: a summary with one occurrence every `interval` days, weeks or
    /// months from `start`, each held to its date by a Start No Earlier Than constraint.
    Recurring { at: usize, name: String, dur: String, every: String, interval: u32, count: u32, start: String },
    /// Renames a column (empty title restores the field's name).
    Title { key: String, title: String },
    /// Adds (k = None), replaces or (sheet = None) removes a spreadsheet tab.
    Sheet { k: Option<usize>, sheet: Option<SheetData> },
    /// Sets cell inputs on a spreadsheet tab (empty input clears the cell).
    SheetCells { k: usize, cells: Vec<(String, String)>, fmt: Option<String> },
    /// Sets the text formatting of task cells (column key, or "*" for the row); empty clears it.
    Style { cells: Vec<(u32, String, String)> },
    /// Sets the text formatting of spreadsheet cells.
    SheetStyle { k: usize, cells: Vec<(String, String)> },
    /// Sets a task's predecessors in other plans (links between plans).
    ExtSet { uid: u32, links: Vec<ExtLink> },
    /// Sets a master plan's subplans (order and hierarchy).
    Subplans { list: Vec<SubPlan> },
    /// Sets the master a subplan belongs to (its file; empty for none).
    Master { file: String },
    /// Inserts (`count` > 0) or deletes (`count` < 0) whole rows or columns of a spreadsheet tab
    /// at `at` (0-based); formulas on every tab follow, as in a spreadsheet.
    SheetInsert { k: usize, cols: bool, at: u32, count: i32 },
    /// Adds, replaces (`old`) or (rule None) removes a rule of credit.
    Credit { old: Option<String>, rule: Option<CreditRule> },
    /// Gives the tasks the steps of a rule of credit (from the plan or the built-in library).
    ApplyCredit { uids: Vec<u32>, rule: String },
    /// Adds, changes or (item None) removes a constraint-log entry by id (0 = new).
    Log { id: u32, item: Option<LogItem> },
    /// Saves the notes of a reporting period (replacing the same kind and end date).
    Note { note: PeriodNote },
    /// Adds (id 0), changes or removes (None) a picture in the plan's library.
    Picture { id: u32, pic: Option<Picture> },
    /// Replaces the pictures placed on sheet k.
    SheetImages { k: usize, images: Vec<SheetImage> },
    Batch(Vec<Op>),
}

pub struct Doc {
    pub p: Project,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    undo: Vec<Project>,
    redo: Vec<Project>,
}

const UNDO_LIMIT: usize = 200;

impl Doc {
    pub fn new(p: Project) -> Self {
        Self { p, path: None, dirty: false, undo: vec![], redo: vec![] }
    }

    pub fn can_undo(&self) -> bool { !self.undo.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo.is_empty() }
    pub fn undo_len(&self) -> usize { self.undo.len() + self.redo.len() * 7919 }

    /// Applies `op` to a copy and keeps it only if the plan still schedules.
    /// `fmt` is the date format the user types in.
    pub fn apply(&mut self, op: Op, fmt: &str) -> Result<(), String> {
        let mut q = self.p.clone();
        exec(&mut q, op, fmt)?;
        q.normalize();
        sched::schedule(&q)?;
        if q == self.p {
            return Ok(());
        }
        self.undo.push(std::mem::replace(&mut self.p, q));
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
        Ok(())
    }

    pub fn undo(&mut self) {
        if let Some(p) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.p, p));
            self.dirty = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(p) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.p, p));
            self.dirty = true;
        }
    }

    pub fn open(path: &Path) -> Result<Self, String> {
        let text = read_limited(path)?;
        let p = Project::from_json(&text).map_err(|e| format!("This is not a Datum Plan file ({e})."))?;
        let mut d = Self::new(p);
        d.path = Some(path.into());
        Ok(d)
    }

    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        write_atomic(path, self.json().as_bytes())?;
        self.path = Some(path.into());
        self.dirty = false;
        Ok(())
    }

    pub fn json(&self) -> String {
        serde_json::to_string_pretty(&self.p).unwrap_or_default()
    }
}

/// Tasks (with their subtasks) to put on the clipboard; levels start at 0.
/// The largest file Datum Plan opens or imports (a 100,000-task plan is about 60 MB).
pub const MAX_FILE: u64 = 300 * 1024 * 1024;

/// Reads a text file of at most `MAX_FILE` bytes (invalid UTF-8 is replaced, not fatal).
pub fn read_limited(path: &Path) -> Result<String, String> {
    let len = std::fs::metadata(path).map_err(|e| format!("Could not open {}: {e}", path.display()))?.len();
    if len > MAX_FILE {
        return Err(format!("{} is {} MB; Datum Plan opens files up to {} MB.", path.display(), len / 1_048_576, MAX_FILE / 1_048_576));
    }
    std::fs::read(path).map(|b| String::from_utf8_lossy(&b).into_owned()).map_err(|e| format!("Could not open {}: {e}", path.display()))
}

/// Writes a file so that a crash or a full disk never leaves it half-written: the new content
/// goes to a temporary file next to it, which then replaces the old one in one step.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let fail = |e: std::io::Error| format!("Could not save {}: {e}", path.display());
    let tmp = path.with_extension(format!("{}.saving", path.extension().and_then(|e| e.to_str()).unwrap_or("tmp")));
    let mut f = std::fs::File::create(&tmp).map_err(fail)?;
    if let Err(e) = f.write_all(bytes).and_then(|_| f.sync_all()) {
        drop(f);
        let _ = std::fs::remove_file(&tmp);
        return Err(fail(e));
    }
    drop(f);
    std::fs::rename(&tmp, path).map_err(|e| { let _ = std::fs::remove_file(&tmp); fail(e) })
}

pub fn copy(p: &Project, uids: &[u32]) -> Vec<Task> {
    let mut keep = vec![false; p.tasks.len()];
    for i in idxs(p, uids) {
        p.subtree(i).for_each(|j| keep[j] = true);
    }
    let mut v: Vec<Task> = p.tasks.iter().zip(&keep).filter(|(_, k)| **k).map(|(t, _)| t.clone()).collect();
    let min = v.iter().map(|t| t.level).min().unwrap_or(0);
    v.iter_mut().for_each(|t| t.level -= min);
    v
}

fn idxs(p: &Project, uids: &[u32]) -> Vec<usize> {
    let mut v: Vec<usize> = uids.iter().filter_map(|&u| p.idx(u)).collect();
    v.sort_unstable();
    v.dedup();
    v
}

fn shift(p: &mut Project, uids: &[u32], by: i8) {
    let mut done = HashSet::new();
    for i in idxs(p, uids) {
        let t = &p.tasks[i];
        let ok = if by > 0 { i > 0 && t.level <= p.tasks[i - 1].level } else { t.level > 0 };
        if done.contains(&i) || !ok {
            continue;
        }
        for j in p.subtree(i) {
            done.insert(j);
            p.tasks[j].level = (p.tasks[j].level as i8 + by) as u8;
        }
    }
}

fn move_rows(p: &mut Project, uids: &[u32], up: bool) {
    let mut v = idxs(p, uids);
    if !up {
        v.reverse();
    }
    for uid in v.iter().map(|&i| p.tasks[i].uid).collect::<Vec<_>>() {
        let i = p.idx(uid).unwrap();
        let (r, lvl) = (p.subtree(i), p.tasks[i].level);
        if up {
            let mut k = i;
            while k > 0 && p.tasks[k - 1].level > lvl {
                k -= 1;
            }
            if k > 0 && p.tasks[k - 1].level == lvl {
                let k = p.subtree(k - 1).start;
                p.tasks[k..r.end].rotate_right(r.len());
            }
        } else if r.end < p.tasks.len() && p.tasks[r.end].level == lvl {
            let s = p.subtree(r.end);
            p.tasks[i..s.end].rotate_left(r.len());
        }
    }
}

fn level_at(p: &Project, at: usize) -> u8 {
    p.tasks.get(at).or_else(|| p.tasks.last()).map_or(0, |t| t.level)
}

fn yes(v: &str) -> bool {
    matches!(v.trim().to_lowercase().as_str(), "yes" | "y" | "true" | "1" | "on")
}

fn set_pct(p: &mut Project, i: usize, n: u8) -> Result<(), String> {
    let needs = (n > 0 && p.tasks[i].act_start.is_none()) || (n == 100 && p.tasks[i].act_finish.is_none());
    let (es, ef) = if needs { let s = sched::schedule(p)?; (s.es[i], s.ef[i]) } else { (0, 0) };
    let t = &mut p.tasks[i];
    t.pct = n.min(100);
    if n == 0 {
        (t.act_start, t.act_finish) = (None, None);
    } else {
        t.act_start.get_or_insert(date_of(es));
        t.act_finish = if n == 100 { Some(t.act_finish.unwrap_or(date_of((ef - 1).max(es)))) } else { None };
    }
    Ok(())
}

pub(crate) fn set(p: &mut Project, i: usize, field: &str, value: &str, fmt: &str) -> Result<(), String> {
    let v = value.trim();
    let any_row = field.starts_with("c:") || field.starts_with("code:") || matches!(field, "name" | "notes" | "code" | "preds" | "active" | "timeline" | "deadline" | "priority" | "costaccount" | "responsible" | "hyperlink" | "style" | "fixedcost");
    if p.is_summary(i) && !any_row {
        return Err(format!("A summary task's {field} comes from the tasks under it."));
    }
    let date = |s: &str| -> Result<Option<NaiveDate>, String> { if s.is_empty() { Ok(None) } else { parse::date_in(s, fmt).map(Some) } };
    let day = p.day_min;
    let uid = p.tasks[i].uid;
    if let Some(key) = field.strip_prefix("c:") {
        let val = custom_value(p, key, value, fmt)?;
        match val { serde_json::Value::Null => { p.tasks[i].custom.remove(key); } v => { p.tasks[i].custom.insert(key.into(), v); } }
        return Ok(());
    }
    if let Some(code) = field.strip_prefix("code:") {
        if let Some(def) = p.codes.iter().find(|c| c.name == code) {
            if !v.is_empty() && !def.values.iter().any(|x| x.value == v) {
                return Err(format!("\"{v}\" is not a value of the {code} code. Add it in Project › Activity Codes."));
            }
        }
        if v.is_empty() { p.tasks[i].codes.remove(code); } else { p.tasks[i].codes.insert(code.into(), v.into()); }
        return Ok(());
    }
    match field {
        "name" => p.tasks[i].name = v.into(),
        "notes" => p.tasks[i].notes = value.into(),
        "code" => p.tasks[i].code = v.into(),
        "dur" => {
            let (m, u, e) = parse::dur_any(v, day)?;
            let t = &mut p.tasks[i];
            (t.dur, t.unit, t.elapsed) = (m, u, e);
        }
        "work" => {
            let (m, _) = parse::dur(if v.chars().all(|c| c.is_ascii_digit() || c == '.') { format!("{v}h") } else { v.to_string() }.as_str(), day)?;
            let units: f64 = work_units(p, i);
            if units <= 0.0 {
                return Err("Assign a work resource before entering work.".into());
            }
            let t = &mut p.tasks[i];
            if t.task_type == TaskType::FixedDuration && t.dur > 0 {
                let f = m as f64 / (t.dur as f64 * units);
                for a in &mut t.res { a.units *= f; }
            } else {
                t.dur = (m as f64 / units).round() as i64;
            }
        }
        "type" => p.tasks[i].task_type = match v.to_lowercase().replace(' ', "").as_str() {
            "fixedunits" => TaskType::FixedUnits, "fixedwork" => TaskType::FixedWork, "fixedduration" => TaskType::FixedDuration,
            _ => return Err("Task type is Fixed Duration, Fixed Units or Fixed Work.".into()),
        },
        "effort" => p.tasks[i].effort = yes(v),
        "ignore_rc" => p.tasks[i].ignore_rc = yes(v),
        "acttype" => {
            let t = &mut p.tasks[i];
            t.act_type = match v.to_lowercase().replace([' ', '-'], "").as_str() {
                "task" | "taskdependent" => ActType::Task, "resourcedependent" => ActType::ResourceDep, "levelofeffort" | "loe" => ActType::Loe,
                "startmilestone" => ActType::StartMs, "finishmilestone" => ActType::FinishMs, "wbssummary" => ActType::WbsSummary,
                _ => return Err("Activity type is Task Dependent, Resource Dependent, Level of Effort, Start Milestone, Finish Milestone or WBS Summary.".into()),
            };
            if matches!(t.act_type, ActType::StartMs | ActType::FinishMs) { t.dur = 0; }
        }
        "pcttype" => p.tasks[i].pct_type = match v.to_lowercase().as_str() { "units" => PctType::Units, "physical" => PctType::Physical, _ => PctType::Duration },
        "phys" => p.tasks[i].phys = v.trim_end_matches('%').trim().parse::<u8>().map_err(|_| "Enter a number from 0 to 100.")?.min(100),
        "priority" => p.tasks[i].priority = if v.is_empty() { None } else { Some(v.parse::<u16>().map_err(|_| "Priority is a number from 0 to 1000.")?.min(1000)) },
        "fixedcost" => { let unit = 10f64.powi(p.currency.decimals.min(4) as i32); p.tasks[i].fixed_cost = (money(v)? * unit).round() / unit }
        "accrual" => p.tasks[i].accrual = match v.to_lowercase().chars().next() { Some('s') => Accrual::Start, Some('e') => Accrual::End, _ => Accrual::Prorated },
        "costaccount" => p.tasks[i].cost_account = v.into(),
        "responsible" => p.tasks[i].responsible = v.into(),
        "hyperlink" => p.tasks[i].hyperlink = v.into(),
        "leveldelay" => p.tasks[i].level_delay = if v.is_empty() { 0 } else { parse::dur(v, day)?.0 },
        "qty" | "qtydone" => {
            let n: f64 = if v.is_empty() { 0.0 } else { v.replace(',', "").parse().map_err(|_| "Enter a quantity, like 120 or 37.5.")? };
            if !(0.0..1e12).contains(&n) { return Err("Enter a quantity of 0 or more.".into()) }
            let t = &mut p.tasks[i];
            if field == "qty" { t.qty = n } else { t.qty_done = n }
            // Installed quantities measure physical progress.
            if t.qty > 0.0 { (t.pct_type, t.phys) = (PctType::Physical, (t.qty_done / t.qty * 100.0).clamp(0.0, 100.0).round() as u8) }
        }
        "qtyunit" => p.tasks[i].qty_unit = v.into(),
        "dmin" | "dmax" => {
            let m = if v.is_empty() { 0 } else { parse::dur(v, day)?.0 };
            if field == "dmin" { p.tasks[i].dmin = m } else { p.tasks[i].dmax = m }
        }
        "style" => p.tasks[i].style = serde_json::from_str(value).unwrap_or(serde_json::Value::Null),
        "pct" => set_pct(p, i, v.trim_end_matches('%').trim().parse().map_err(|_| "Enter a number from 0 to 100.")?)?,
        "rem" => {
            let (m, _) = parse::dur(v, day)?;
            let d = p.tasks[i].dur.max(1);
            set_pct(p, i, (100 - (m.min(d) * 100 / d)) as u8)?;
        }
        "start" => {
            let d = date(v)?;
            let t = &mut p.tasks[i];
            if t.manual {
                t.mstart = d;
            } else if d.is_some() {
                (t.constraint, t.cdate) = (Constraint::SNET, d);
            } else if t.constraint == Constraint::SNET {
                (t.constraint, t.cdate) = (Constraint::ASAP, None);
            }
        }
        "finish" => {
            let d = date(v)?.ok_or("Type a finish date.")?;
            let s = sched::schedule(p)?;
            let c = &s.cals[s.ci[i]];
            p.tasks[i].dur = c.between(s.es[i], c.end_of(d)).max(0);
        }
        "preds" => {
            let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
            let links = parse::preds(v, &ids, day)?;
            if links.iter().any(|l| l.pred == uid) {
                return Err("A task cannot be its own predecessor.".into());
            }
            p.tasks[i].preds = links;
        }
        "res" => {
            let before = work_units(p, i);
            let res = parse::res(v, p)?;
            p.tasks[i].res = res;
            let after = work_units(p, i);
            // Fixed work, or effort-driven fixed units: the work stays and the duration changes.
            let t = &mut p.tasks[i];
            if before > 0.0 && after > 0.0 && (t.task_type == TaskType::FixedWork || (t.effort && t.task_type == TaskType::FixedUnits)) {
                t.dur = (t.dur as f64 * before / after).round() as i64;
            }
        }
        "ctype" => {
            let c = parse::constraint(v)?;
            if matches!(c, Constraint::ASAP | Constraint::ALAP) {
                p.tasks[i].cdate = None;
            } else if p.tasks[i].cdate.is_none() {
                let s = sched::schedule(p)?;
                let ends = matches!(c, Constraint::FNET | Constraint::FNLT | Constraint::MFO);
                p.tasks[i].cdate = Some(date_of(if ends { (s.ef[i] - 1).max(s.es[i]) } else { s.es[i] }));
            }
            p.tasks[i].constraint = c;
        }
        "cdate" => {
            let t = &mut p.tasks[i];
            t.cdate = date(v)?;
            if t.cdate.is_some() && matches!(t.constraint, Constraint::ASAP | Constraint::ALAP) {
                t.constraint = Constraint::SNET;
            } else if t.cdate.is_none() {
                t.constraint = Constraint::ASAP;
            }
        }
        "deadline" => p.tasks[i].deadline = date(v)?,
        "cal" => {
            if !v.is_empty() && !p.calendars.iter().any(|c| c.name == v) {
                return Err(format!("There is no calendar called \"{v}\". Add it in Project › Change Working Time."));
            }
            p.tasks[i].cal = Some(v.to_string()).filter(|c| !c.is_empty());
        }
        "mode" => {
            let manual = v.to_lowercase().starts_with('m');
            if manual && p.tasks[i].mstart.is_none() {
                let s = sched::schedule(p)?;
                p.tasks[i].mstart = Some(date_of(s.es[i]));
            }
            p.tasks[i].manual = manual;
        }
        "active" => p.tasks[i].inactive = !yes(v),
        "timeline" => p.tasks[i].timeline = yes(v),
        "astart" => {
            let d = date(v)?;
            let t = &mut p.tasks[i];
            t.act_start = d;
            if d.is_none() {
                (t.act_finish, t.pct) = (None, 0);
            }
        }
        "afinish" => {
            let d = date(v)?;
            if d.is_some() {
                if p.tasks[i].act_start.is_none() {
                    set_pct(p, i, 1)?;
                }
                p.tasks[i].pct = 100;
            } else if p.tasks[i].pct == 100 {
                p.tasks[i].pct = 99;
            }
            p.tasks[i].act_finish = d;
        }
        f => return Err(format!("{f} is calculated and cannot be edited.")),
    }
    Ok(())
}

/// Total units of work resources on task `i`.
fn work_units(p: &Project, i: usize) -> f64 {
    p.tasks[i].res.iter().filter(|a| p.res_idx(a.res).is_some_and(|k| p.resources[k].kind == ResKind::Work)).map(|a| a.units).sum()
}

fn money(v: &str) -> Result<f64, String> {
    let t: String = v.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
    if t.is_empty() { Ok(0.0) } else { t.parse().map_err(|_| format!("\"{v}\" is not an amount.")) }
}

/// Checks and converts a custom field value for key `key` (text1, number2, flag3, date1, cost1,
/// duration1, start1, finish1, outline1). Empty clears it.
pub(crate) fn custom_value(p: &Project, key: &str, value: &str, fmt: &str) -> Result<serde_json::Value, String> {
    use serde_json::json;
    let v = value.trim();
    let def = p.fields.iter().find(|d| d.key == key);
    if let Some(d) = def {
        if !d.formula.is_empty() {
            return Err(format!("{} is calculated by a formula and cannot be typed in.", if d.alias.is_empty() { key } else { &d.alias }));
        }
        if d.restrict && !v.is_empty() && !d.lookup.iter().any(|x| x.value.eq_ignore_ascii_case(v)) {
            return Err(format!("\"{v}\" is not in the list for this field."));
        }
    }
    if v.is_empty() {
        return Ok(serde_json::Value::Null);
    }
    let kind = key.trim_start_matches('r').trim_end_matches(|c: char| c.is_ascii_digit());
    Ok(match kind {
        "number" => json!(v.replace(',', "").parse::<f64>().map_err(|_| format!("\"{v}\" is not a number."))?),
        "cost" => json!(money(v)?),
        "flag" => json!(yes(v)),
        "duration" => json!(parse::dur(v, p.day_min)?.0),
        "date" | "start" | "finish" => json!(parse::date_in(v, fmt)?.to_string()),
        _ => json!(v),
    })
}

fn insert_rows(p: &mut Project, at: usize, rows: Vec<Vec<(String, String)>>, fmt: &str) -> Result<(), String> {
    let at = at.min(p.tasks.len());
    let level = level_at(p, at);
    let mut later = vec![];
    let n = rows.len();
    for (k, row) in rows.into_iter().enumerate() {
        let t = p.new_task("", level, p.day_min);
        p.tasks.insert(at + k, t);
        for (f, v) in row {
            if f == "preds" { later.push((at + k, v)) } else if !v.trim().is_empty() || f == "name" { set(p, at + k, &f, &v, fmt)? }
        }
    }
    // Predecessor IDs are the ones the user saw before the rows went in.
    let shift_ids = |v: &str, n: usize| v.split([',', ';']).map(|tok| {
        let tok = tok.trim();
        let cut = tok.find(|c: char| !c.is_ascii_digit()).unwrap_or(tok.len());
        match tok[..cut].parse::<usize>() { Ok(id) if id > at => format!("{}{}", id + n, &tok[cut..]), _ => tok.to_string() }
    }).collect::<Vec<_>>().join(",");
    later.into_iter().try_for_each(|(i, v)| set(p, i, "preds", &shift_ids(&v, n), fmt))
}

fn res_set(p: &mut Project, uid: u32, field: &str, v: &str) -> Result<(), String> {
    let i = p.res_idx(uid).ok_or("That resource no longer exists.")?;
    let v = v.trim();
    let num = |s: &str| -> Result<f64, String> { s.trim_end_matches('%').replace(',', "").trim().parse::<f64>().map_err(|_| format!("\"{s}\" is not a number.")) };
    let r = &mut p.resources[i];
    match field {
        "name" if v.is_empty() => return Err("A resource needs a name.".into()),
        "name" => r.name = v.into(),
        "kind" => r.kind = match v.to_lowercase().chars().next() { Some('m') => ResKind::Material, Some('c') => ResKind::Cost, _ => ResKind::Work },
        "group" => r.group = v.into(),
        "max" => r.max = num(v)? / if v.ends_with('%') || num(v)? > 10.0 { 100.0 } else { 1.0 },
        "rate" => r.rate = num(v)?,
        "per_use" => r.per_use = num(v)?,
        "label" => r.label = v.into(),
        "initials" => r.initials = v.into(),
        "email" => r.email = v.into(),
        "country" => r.country = v.chars().take(60).collect(),
        "code" => r.code = v.into(),
        "notes" => r.notes = v.into(),
        "ovt" => r.ovt = num(v)?,
        f => return Err(format!("{f} cannot be edited.")),
    }
    let name = p.resources[i].name.clone();
    if p.resources.iter().filter(|r| r.name.eq_ignore_ascii_case(&name)).count() > 1 {
        return Err(format!("There is already a resource called \"{name}\"."));
    }
    Ok(())
}

fn exec(p: &mut Project, op: Op, fmt: &str) -> Result<(), String> {
    match op {
        Op::Insert { at, name, ms } => {
            let at = at.min(p.tasks.len());
            let t = p.new_task(name.trim(), level_at(p, at), if ms { 0 } else { p.day_min });
            p.tasks.insert(at, t);
        }
        Op::InsertRows { at, rows } => insert_rows(p, at, rows, fmt)?,
        Op::QuickAdd { at, text } => {
            let row = parse::quick(&text);
            if row[0].1.is_empty() {
                return Err("Start with the task name, like: Pour slab 5d after 4 @Crew.".into());
            }
            insert_rows(p, at, vec![row], fmt)?;
        }
        Op::Paste { at, mut tasks } => {
            let at = at.min(p.tasks.len());
            let base = level_at(p, at);
            let map: HashMap<u32, u32> = tasks.iter().enumerate().map(|(k, t)| (t.uid, p.next_uid + k as u32)).collect();
            p.next_uid += tasks.len() as u32;
            for t in &mut tasks {
                t.uid = map[&t.uid];
                t.level += base;
                t.preds.retain_mut(|l| map.get(&l.pred).map(|&u| l.pred = u).is_some());
            }
            // Pasted copies get fresh Activity IDs.
            let mut taken: HashSet<String> = p.tasks.iter().map(|t| t.code.clone()).collect();
            let mut next = p.next_code();
            for t in tasks.iter_mut().filter(|t| !t.code.is_empty()) {
                if taken.contains(&t.code) {
                    while taken.contains(&next) {
                        let n: u32 = next.trim_start_matches(p.id_prefix.as_str()).parse().unwrap_or(p.id_start);
                        next = format!("{}{}", p.id_prefix, n + p.id_step.max(1));
                    }
                    t.code = next.clone();
                }
                taken.insert(t.code.clone());
            }
            p.tasks.splice(at..at, tasks);
        }
        Op::Delete { uids } => {
            let gone: HashSet<usize> = idxs(p, &uids).into_iter().flat_map(|i| p.subtree(i)).collect();
            let mut i = 0;
            p.tasks.retain(|_| (!gone.contains(&i), i += 1).0);
        }
        Op::Set { uid, field, value } => {
            let i = p.idx(uid).ok_or("That task no longer exists.")?;
            set(p, i, &field, &value, fmt)?;
        }
        Op::Indent { uids } => shift(p, &uids, 1),
        Op::Outdent { uids } => shift(p, &uids, -1),
        Op::Move { uids, up } => move_rows(p, &uids, up),
        Op::Link { uids } => {
            let v = idxs(p, &uids);
            if v.len() < 2 {
                return Err("Select two or more tasks to link.".into());
            }
            for w in v.windows(2) {
                let pred = p.tasks[w[0]].uid;
                let t = &mut p.tasks[w[1]];
                if !t.preds.iter().any(|l| l.pred == pred) {
                    t.preds.push(Link { pred, kind: LinkType::FS, lag: 0, pct: false });
                }
            }
        }
        Op::LinkSet { pred, succ, kind, lag } => {
            let i = p.idx(succ).ok_or("That task no longer exists.")?;
            p.idx(pred).ok_or("There is no such predecessor.")?;
            if pred == succ {
                return Err("A task cannot be its own predecessor.".into());
            }
            let lag = if lag.trim().is_empty() { 0 } else { let l = lag.trim(); let neg = l.starts_with('-'); let m = parse::dur(l.trim_start_matches(['+', '-']), p.day_min)?.0; if neg { -m } else { m } };
            let t = &mut p.tasks[i];
            let at = t.preds.iter().position(|l| l.pred == pred);
            match (kind, at) {
                (Some(kind), Some(k)) => t.preds[k] = Link { pred, kind, lag, pct: false },
                (Some(kind), None) => t.preds.push(Link { pred, kind, lag, pct: false }),
                (None, Some(k)) => { t.preds.remove(k); }
                (None, None) => {}
            }
        }
        Op::Level { uids, clear } => {
            if clear { crate::level::clear(p, &uids) } else { crate::level::level(p, &uids)?; }
        }
        Op::Split { uid, after, gap } => {
            let i = p.idx(uid).ok_or("That task no longer exists.")?;
            let t = &mut p.tasks[i];
            if after <= 0 || after >= t.dur || gap <= 0 {
                return Err("Split inside the task's work, with a pause of at least a minute.".into());
            }
            t.splits.retain(|(a, _)| *a != after);
            t.splits.push((after, gap));
            t.splits.sort_unstable();
        }
        Op::Interim { n } => {
            let s = sched::schedule(p)?;
            let n = n.clamp(1, 10);
            for (i, t) in p.tasks.iter_mut().enumerate() {
                t.custom.insert(format!("start{n}"), serde_json::json!(date_of(s.es[i]).to_string()));
                t.custom.insert(format!("finish{n}"), serde_json::json!(date_of((s.ef[i] - 1).max(s.es[i])).to_string()));
            }
        }
        Op::UpdateProject { through, reschedule } => {
            let d = parse::date_in(&through, fmt)?;
            let s = sched::schedule(p)?;
            let dd = at(d) + DAY;
            for i in (0..p.tasks.len()).filter(|&i| !s.summary[i] && !s.off[i]) {
                if reschedule {
                    if p.tasks[i].act_start.is_none() && s.es[i] < dd && p.tasks[i].pct == 0 {
                        (p.tasks[i].constraint, p.tasks[i].cdate) = (Constraint::SNET, Some(d.succ_opt().unwrap()));
                    }
                    continue;
                }
                let c = &s.cals[s.ci[i]];
                let done = if dd <= s.es[i] { 0 } else if dd >= s.ef[i] { 100 } else { c.between(s.es[i], dd) * 100 / c.between(s.es[i], s.ef[i]).max(1) };
                if done as u8 > p.tasks[i].pct {
                    set_pct(p, i, done as u8)?;
                }
            }
            p.status = Some(d);
        }
        Op::Snapshot { label, comment } => {
            let s = sched::schedule(p)?;
            let cost = load::compute(p, &s).cost;
            let rows = p.tasks.iter().enumerate().map(|(i, t)| SnapRow { uid: t.uid, start: s.es[i], finish: s.ef[i], pct: t.pct, cost: cost[i] }).collect();
            let leaf: Vec<usize> = (0..p.tasks.len()).filter(|&i| !s.summary[i] && !s.off[i]).collect();
            let w: i64 = leaf.iter().map(|&i| p.tasks[i].dur).sum();
            let pct = if w > 0 { leaf.iter().map(|&i| p.tasks[i].dur * p.tasks[i].pct as i64).sum::<i64>() as f64 / w as f64 } else { 0.0 };
            let total = leaf.iter().map(|&i| cost[i]).sum();
            p.snapshots.push(Snapshot { date: Some(p.status.unwrap_or_else(|| date_of(s.start))), label, comment, finish: s.finish, pct, cost: total, rows });
        }
        Op::ProjectSet { field, value } => {
            let mut v = serde_json::to_value(&*p).map_err(|e| e.to_string())?;
            if v.get(&field).is_none() || matches!(field.as_str(), "tasks" | "resources" | "format" | "next_uid") {
                return Err(format!("{field} cannot be set here."));
            }
            v[&field] = value;
            *p = serde_json::from_value(v).map_err(|e| format!("That value does not fit {field}: {e}"))?;
        }
        Op::FieldDef { def } => {
            if crate::sheet::is_excel(&def.formula) {
                let c = crate::fields::Calc::new(p, fmt, p.start);
                crate::sheet::check(&def.formula, |n| c.key_of(n).is_some())?;
            } else if !def.formula.trim().is_empty() {
                crate::formula::parse(&def.formula).map_err(|e| format!("Formula: {e}"))?;
            }
            p.fields.retain(|d| d.key != def.key);
            if !(def.alias.is_empty() && def.formula.is_empty() && def.lookup.is_empty() && def.indicators.is_empty()) {
                p.fields.push(def);
            }
        }
        Op::DeleteField { key } => {
            p.fields.retain(|d| d.key != key);
            p.tasks.iter_mut().for_each(|t| { t.custom.remove(&key); });
        }
        Op::CodeDef { old, def } => {
            if let Some(o) = &old {
                p.codes.retain(|c| &c.name != o);
            }
            if let Some(d) = def {
                if d.name.trim().is_empty() {
                    return Err("An activity code needs a name.".into());
                }
                if let Some(o) = old.filter(|o| *o != d.name) {
                    for t in &mut p.tasks {
                        if let Some(v) = t.codes.remove(&o) { t.codes.insert(d.name.clone(), v); }
                    }
                }
                p.codes.retain(|c| c.name != d.name);
                p.codes.push(d);
            }
        }
        Op::Credit { old, rule } => {
            if let Some(o) = &old { p.credit.retain(|r| &r.name != o) }
            if let Some(r) = rule {
                if r.name.trim().is_empty() || r.steps.iter().map(|x| x.1).sum::<f64>() <= 0.0 { return Err("A rule needs a name and steps whose weights add up to more than 0.".into()) }
                p.credit.retain(|x| x.name != r.name);
                p.credit.push(r);
            }
        }
        Op::ApplyCredit { uids, rule } => {
            let r = p.credit.iter().find(|r| r.name == rule).cloned().or_else(|| crate::progress::credit_library().into_iter().find(|r| r.name == rule)).ok_or(format!("There is no rule of credit called \"{rule}\"."))?;
            let idx: Vec<usize> = uids.iter().filter_map(|u| p.idx(*u)).filter(|&i| !p.is_summary(i)).collect();
            if idx.is_empty() { return Err("Select the tasks to apply the rule to.".into()) }
            for i in idx {
                let t = &mut p.tasks[i];
                t.steps = r.steps.iter().map(|(n, w)| Step { name: n.clone(), weight: *w, done: false }).collect();
                (t.pct_type, t.phys) = (PctType::Physical, 0);
            }
        }
        Op::Log { id, item } => {
            p.log.retain(|x| x.id != id || id == 0);
            if let Some(mut x) = item {
                if p.idx(x.uid).is_none() { return Err("Choose the task this constraint holds up.".into()) }
                x.id = if id == 0 { p.log.iter().map(|x| x.id).max().unwrap_or(0) + 1 } else { id };
                if x.status.is_empty() { x.status = "Open".into() }
                p.log.push(x);
                p.log.sort_by_key(|x| x.id);
            }
        }
        Op::Risk { id, risk } => {
            p.risks.retain(|r| r.id != id);
            if let Some(mut r) = risk {
                r.id = if id == 0 { p.risks.iter().map(|r| r.id).max().unwrap_or(0) + 1 } else { id };
                p.risks.push(r);
                p.risks.sort_by_key(|r| r.id);
            }
        }
        Op::TaskList { uid, field, value } => {
            let i = p.idx(uid).ok_or("That task no longer exists.")?;
            let t = &mut p.tasks[i];
            let e = |x: serde_json::Error| x.to_string();
            match field.as_str() {
                "steps" => t.steps = serde_json::from_value(value).map_err(e)?,
                "notebook" => t.notebook = serde_json::from_value(value).map_err(e)?,
                "expenses" => t.expenses = serde_json::from_value(value).map_err(e)?,
                "docs" => t.docs = serde_json::from_value(value).map_err(e)?,
                "splits" => t.splits = serde_json::from_value(value).map_err(e)?,
                f => return Err(format!("{f} is not a task list.")),
            }
            // Weighted steps measure physical progress.
            let w: f64 = t.steps.iter().map(|s| s.weight.max(0.0)).sum();
            if field == "steps" && w > 0.0 {
                t.phys = (t.steps.iter().filter(|s| s.done).map(|s| s.weight.max(0.0)).sum::<f64>() * 100.0 / w).round() as u8;
            }
        }
        Op::ResJson { uid, field, value } => {
            let k = p.res_idx(uid).ok_or("That resource no longer exists.")?;
            let r = &mut p.resources[k];
            let e = |x: serde_json::Error| x.to_string();
            match field.as_str() {
                "tables" => r.tables = serde_json::from_value(value).map_err(e)?,
                "avail" => r.avail = serde_json::from_value(value).map_err(e)?,
                "custom" => r.custom = serde_json::from_value(value).map_err(e)?,
                "cal" => r.cal = value.as_str().filter(|s| !s.is_empty()).map(String::from),
                "exceptions" => r.exceptions = serde_json::from_value(value).map_err(e)?,
                f => return Err(format!("{f} cannot be set here.")),
            }
        }
        Op::Ui { key, value } => {
            if !p.ui.is_object() {
                p.ui = serde_json::json!({});
            }
            p.ui[&key] = value;
        }
        Op::Unlink { uids } => {
            let set: HashSet<u32> = uids.iter().copied().collect();
            for t in &mut p.tasks {
                let me = set.contains(&t.uid);
                t.preds.retain(|l| if set.len() == 1 { !me && !set.contains(&l.pred) } else { !(me && set.contains(&l.pred)) });
            }
        }
        Op::MarkOnTrack { uids } => {
            let dd = at(p.status.ok_or("Set a status date in Project › Project Information first.")?) + DAY;
            let s = sched::schedule(p)?;
            for i in idxs(p, &uids).into_iter().filter(|&i| !s.summary[i]) {
                let c = &s.cals[s.ci[i]];
                let n = if dd <= s.es[i] { 0 } else if dd >= s.ef[i] { 100 } else { c.between(s.es[i], dd) * 100 / c.between(s.es[i], s.ef[i]).max(1) };
                set_pct(p, i, n as u8)?;
            }
        }
        Op::Baseline { uids, clear, n } => {
            let s = sched::schedule(p)?;
            let l = load::compute(p, &s);
            let all = uids.is_empty();
            let pick: HashSet<u32> = uids.into_iter().collect();
            for (i, t) in p.tasks.iter_mut().enumerate().filter(|(_, t)| all || pick.contains(&t.uid)) {
                if clear { t.bls.remove(&n); } else { t.bls.insert(n.min(10), Baseline { start: s.es[i], finish: s.ef[i], dur: if s.summary[i] { s.cals[s.ci[i]].between(s.es[i], s.ef[i]) } else { t.dur }, cost: l.cost[i], work: Some(l.work[i]) }); }
            }
        }
        Op::Project { name, start, status, calendar, day_hours, manual_default } => {
            if !(0.5..=24.0).contains(&day_hours) {
                return Err("Hours per day must be between 0.5 and 24.".into());
            }
            p.name = name.trim().into();
            p.start = parse::date_in(&start, fmt)?;
            p.status = if status.trim().is_empty() { None } else { Some(parse::date_in(&status, fmt)?) };
            if !p.calendars.iter().any(|c| c.name == calendar) {
                return Err(format!("There is no calendar called \"{calendar}\"."));
            }
            p.calendar = calendar;
            p.day_min = (day_hours * 60.0).round() as i64;
            p.manual_default = manual_default;
        }
        Op::CalPut { old, mut cal } => {
            cal.name = cal.name.trim().into();
            if cal.name.is_empty() {
                return Err("A calendar needs a name.".into());
            }
            for h in cal.week.iter_mut().chain(cal.exceptions.iter_mut().map(|e| &mut e.hours)) {
                h.retain(|(s, e)| s < e && *e <= 1440);
            }
            if cal.week.iter().all(Vec::is_empty) {
                return Err("A calendar needs at least one working day.".into());
            }
            if cal.exceptions.iter().any(|e| e.to < e.from) {
                return Err("An exception ends before it starts.".into());
            }
            let old = old.unwrap_or_else(|| cal.name.clone());
            if cal.name != old && p.calendars.iter().any(|c| c.name == cal.name) {
                return Err(format!("There is already a calendar called \"{}\".", cal.name));
            }
            if p.calendar == old {
                p.calendar = cal.name.clone();
            }
            for t in p.tasks.iter_mut().filter(|t| t.cal.as_deref() == Some(old.as_str())) {
                t.cal = Some(cal.name.clone());
            }
            match p.calendars.iter().position(|c| c.name == old) {
                Some(k) => p.calendars[k] = cal,
                None => p.calendars.push(cal),
            }
        }
        Op::CalDelete { name } => {
            if name == p.calendar {
                return Err("The project calendar cannot be deleted. Choose another project calendar first.".into());
            }
            p.calendars.retain(|c| c.name != name);
        }
        Op::ResInsert { name } => {
            let name = name.trim();
            if p.resources.iter().any(|r| r.name.eq_ignore_ascii_case(name)) {
                return Err(format!("There is already a resource called \"{name}\"."));
            }
            p.resource(if name.is_empty() { "New resource" } else { name });
        }
        Op::ResSet { uid, field, value } => res_set(p, uid, &field, &value)?,
        Op::ResDelete { uids } => p.resources.retain(|r| !uids.contains(&r.uid)),
        Op::Columns { cols } => p.columns = cols,
        Op::Batch(ops) => ops.into_iter().try_for_each(|o| exec(p, o, fmt))?,
        Op::Cells { cells } => {
            let (mut done, mut first) = (0, None);
            for (uid, field, value) in cells {
                let Some(i) = p.idx(uid) else { continue };
                match set(p, i, &field, &value, fmt) {
                    Ok(()) => done += 1,
                    Err(e) => { first.get_or_insert(e); }
                }
            }
            if done == 0 {
                if let Some(e) = first {
                    return Err(e);
                }
            }
        }
        Op::Recurring { at, name, dur, every, interval, count, start } => {
            if count == 0 || count > 500 {
                return Err("Choose between 1 and 500 occurrences.".into());
            }
            let at = at.min(p.tasks.len());
            let lv = level_at(p, at);
            let (d, unit) = parse::dur(&dur, p.day_min)?;
            let first = parse::date_in(&start, fmt).or_else(|_| parse::date(&start))?;
            let t = p.new_task(name.trim(), lv, d);
            p.tasks.insert(at, t);
            for k in 0..count {
                let n = interval.max(1) * k;
                let day = match every.as_str() {
                    "month" => first.checked_add_months(chrono::Months::new(n)).ok_or("Date out of range.")?,
                    "week" => first + chrono::Days::new(7 * n as u64),
                    _ => first + chrono::Days::new(n as u64),
                };
                let mut t = p.new_task(&format!("{} {}", name.trim(), k + 1), lv + 1, d);
                t.unit = unit;
                (t.constraint, t.cdate) = (Constraint::SNET, Some(day));
                p.tasks.insert(at + 1 + k as usize, t);
            }
        }
        Op::Title { key, title } => {
            if title.trim().is_empty() { p.titles.remove(&key); } else { p.titles.insert(key, title.trim().into()); }
        }
        Op::Sheet { k, sheet } => match (k, sheet) {
            (None, Some(s)) => {
                if p.sheets.iter().any(|x| x.name.eq_ignore_ascii_case(&s.name)) || ["plan", "resources"].contains(&s.name.to_lowercase().as_str()) { return Err(format!("There is already a sheet called {}.", s.name)); }
                p.sheets.push(s)
            }
            (Some(k), Some(s)) if k < p.sheets.len() => {
                let old = p.sheets[k].name.clone();
                if !old.eq_ignore_ascii_case(&s.name) {
                    if p.sheets.iter().any(|x| x.name.eq_ignore_ascii_case(&s.name)) || ["plan", "resources"].contains(&s.name.to_lowercase().as_str()) { return Err(format!("There is already a sheet called {}.", s.name)); }
                    rewrite_sheets(p, |f, _| crate::sheet::rename(f, &old, &s.name, &[]));
                    // A rename keeps the cells, now pointing at the new name.
                    p.sheets[k] = SheetData { cells: std::mem::take(&mut p.sheets[k].cells), images: std::mem::take(&mut p.sheets[k].images), ..s };
                } else {
                    p.sheets[k] = s;
                }
            }
            (Some(k), None) if k < p.sheets.len() => {
                let order: Vec<String> = ["Plan", "Resources"].iter().map(|x| x.to_string()).chain(p.sheets.iter().map(|x| x.name.clone())).collect();
                let old = p.sheets.remove(k).name;
                rewrite_sheets(p, |f, _| crate::sheet::rename(f, &old, "", &order));
            }
            _ => return Err("That sheet does not exist.".into()),
        },
        Op::ExtSet { uid, links } => {
            let i = p.idx(uid).ok_or("That task no longer exists.")?;
            if links.len() > 200 { return Err("A task can have at most 200 links to other plans.".into()) }
            p.tasks[i].ext = links;
            p.normalize();
        }
        Op::Subplans { list } => {
            if list.len() > 500 { return Err("A master plan can have at most 500 subplans.".into()) }
            p.subplans = list;
            p.normalize();
        }
        Op::Master { file } => p.master = file.chars().take(1000).collect(),
        Op::SheetInsert { k, cols, at, count } => {
            use gridcraft_formula::adjust::{Axis, Edit};
            let target = p.sheets.get(k).ok_or("That sheet does not exist.")?.name.clone();
            let n = count.unsigned_abs().min(100_000);
            let axis = if cols { Axis::Cols } else { Axis::Rows };
            let edit = if count > 0 { Edit::Insert { axis, at, count: n } } else { Edit::Delete { axis, at, count: n } };
            // An insert must not push anything past the last row or column: it would be lost.
            let last = if cols { 16_383 } else { 1_048_575 };
            let s = &p.sheets[k];
            let used = s.cells.keys().chain(s.fmt.keys()).chain(s.style.keys()).filter_map(|a| gridcraft_core::CellRef::parse(a)).map(|r| if cols { r.col } else { r.row });
            if count > 0 && used.chain(s.widths.keys().copied().filter(|_| cols)).any(|pos| pos >= at && pos.saturating_add(n) > last) {
                return Err(if cols { "Inserting these columns would push cells past the last column (XFD). Clear the cells at the right first." } else { "Inserting these rows would push cells past the last row (1,048,576). Clear the cells at the bottom first." }.into());
            }
            rewrite_sheets(p, |f, host| crate::sheet::adjust(f, host, &target, &edit));
            let s = &mut p.sheets[k];
            // Moves every keyed map of the tab: cells, formats, styles (and column widths).
            let shift = |a1: &str| -> Option<String> {
                let r = gridcraft_core::CellRef::parse(a1)?;
                let (pos, other) = if cols { (r.col, r.row) } else { (r.row, r.col) };
                let pos = if pos < at { pos } else if count > 0 { pos.saturating_add(n) } else if pos < at.saturating_add(n) { return None } else { pos - n };
                let (row, col) = if cols { (other, pos) } else { (pos, other) };
                Some(format!("{}{}", crate::sheet::col_name(col), row + 1))
            };
            for m in [&mut s.cells, &mut s.fmt, &mut s.style] {
                *m = std::mem::take(m).into_iter().filter_map(|(a1, v)| shift(&a1).map(|x| (x, v))).collect();
            }
            if cols {
                s.widths = std::mem::take(&mut s.widths).into_iter().filter_map(|(c, w)| if c < at { Some((c, w)) } else if count > 0 { Some((c.saturating_add(n), w)) } else if c < at.saturating_add(n) { None } else { Some((c - n, w)) }).collect();
            }
        }
        Op::SheetCells { k, cells, fmt } => {
            let s = p.sheets.get_mut(k).ok_or("That sheet does not exist.")?;
            for (a1, v) in cells {
                let a1 = a1.to_uppercase();
                if let Some(f) = &fmt {
                    if f.is_empty() { s.fmt.remove(&a1); } else { s.fmt.insert(a1.clone(), f.clone()); }
                    continue;
                }
                if v.is_empty() { s.cells.remove(&a1); } else { s.cells.insert(a1, v); }
            }
        }
        Op::Style { cells } => for (uid, key, v) in cells {
            let i = p.idx(uid).ok_or("That task no longer exists.")?;
            let st = &mut p.tasks[i].style;
            if !st.is_object() { *st = serde_json::json!({}); }
            let m = st.as_object_mut().unwrap();
            if v.is_empty() { m.remove(&key); } else { m.insert(key, v.into()); }
            if m.is_empty() { *st = serde_json::Value::Null; }
        },
        Op::SheetStyle { k, cells } => {
            let s = p.sheets.get_mut(k).ok_or("That sheet does not exist.")?;
            for (a1, v) in cells { if v.is_empty() { s.style.remove(&a1.to_uppercase()); } else { s.style.insert(a1.to_uppercase(), v); } }
        }
        Op::Note { note } => {
            p.notes.retain(|n| !(n.kind == note.kind && n.end == note.end));
            p.notes.push(note);
        }
        Op::Picture { id, pic } => match pic {
            None => {
                p.pictures.retain(|x| x.id != id);
                p.sheets.iter_mut().for_each(|s| s.images.retain(|i| i.pic != id));
            }
            Some(mut x) => {
                if !x.data.starts_with("data:image/") { return Err("Choose a picture (PNG, JPEG, GIF or SVG).".into()) }
                if x.data.len() > MAX_PICTURE_LEN { return Err("That picture is too large; choose one under 4 MB.".into()) }
                x.name = x.name.trim().chars().take(120).collect();
                x.role = x.role.trim().chars().take(60).collect();
                match p.pictures.iter_mut().find(|y| y.id == id && id > 0) {
                    Some(y) => { x.id = id; *y = x }
                    None => {
                        if p.pictures.len() >= MAX_PICTURES { return Err(format!("A plan keeps up to {MAX_PICTURES} pictures.")) }
                        x.id = p.pictures.iter().map(|y| y.id).max().unwrap_or(0) + 1;
                        p.pictures.push(x);
                    }
                }
            }
        },
        Op::SheetImages { k, images } => {
            let ids: Vec<u32> = p.pictures.iter().map(|x| x.id).collect();
            let s = p.sheets.get_mut(k).ok_or("There is no such sheet.")?;
            s.images = images.into_iter().filter(|i| ids.contains(&i.pic)).take(500).map(|mut i| { (i.w, i.h) = (i.w.clamp(8, 4000), i.h.clamp(8, 4000)); i }).collect();
        }
    }
    Ok(())
}

/// Rewrites every formula on every spreadsheet tab: `f(formula, host sheet name)`.
fn rewrite_sheets(p: &mut Project, f: impl Fn(&str, &str) -> String) {
    for s in &mut p.sheets {
        let host = s.name.clone();
        for v in s.cells.values_mut().filter(|v| v.starts_with('=')) { *v = f(v, &host); }
    }
    // Dashboard tiles hold formulas too (ui.dashboards[].tiles[]); they live on no sheet of their own.
    let tiles = p.ui.get_mut("dashboards").and_then(|d| d.as_array_mut()).into_iter().flatten().filter_map(|d| d.get_mut("tiles")?.as_array_mut()).flatten();
    for t in tiles.filter_map(|t| t.as_object_mut()) {
        for key in ["formula", "sub", "src"] {
            if let Some(serde_json::Value::String(v)) = t.get_mut(key).filter(|v| v.as_str().is_some_and(|x| x.starts_with('='))) { *v = f(v, "__dashboard") }
        }
    }
}
