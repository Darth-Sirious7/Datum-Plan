//! Datum Plan desktop shell: owns the open plan, files, options and recovery, and talks to the window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod pdf;
mod update;

use datum_core::{check, doc::{self, Doc, Op}, fields::{self, Calc}, io, links, network, query::{self, Query}, report, templates, Project, Task};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::{Path, PathBuf}, sync::Mutex};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_dialog::DialogExt;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Options {
    /// chrono format for dates, such as %d-%b-%y.
    date_fmt: String,
    /// Minutes between recovery saves; 0 turns them off.
    autosave: u32,
    /// Offer beta (pre-release) versions in Check for updates.
    beta: bool,
    /// Defaults for new plans: currency, company and logo for reports.
    currency: datum_core::Currency,
    company: String,
    logo: String,
    /// The folder whose plans make up the workspace (portfolio).
    workspace: String,
    /// Ask for project details when a plan is created.
    setup_panel: bool,
    /// auto, light or dark.
    theme: String,
    /// Reopen the last plan at start, and the view a plan opens in.
    open_last: bool,
    start_view: String,
    /// Defaults for new plans: manual scheduling, week end day (0 = Mon … 6 = Sun), month close day (0 = last).
    manual_default: bool,
    week_end: u8,
    month_end: u8,
    /// Default paper and orientation for printing.
    paper: String,
    landscape: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { date_fmt: "%d-%b-%y".into(), autosave: 5, beta: false, currency: Default::default(), company: String::new(), logo: String::new(), workspace: String::new(), setup_panel: true, theme: "auto".into(), open_last: false, start_view: "gantt".into(), manual_default: false, week_end: 4, month_end: 0, paper: "A4".into(), landscape: true }
    }
}

struct App {
    /// The plan in the tab shown; the other tabs' plans wait in `others` (slot `cur` is empty).
    doc: Doc,
    others: Vec<Option<Doc>>,
    cur: usize,
    opts: Options,
    clip: Vec<Task>,
    table: Option<(io::table::Table, String)>,
    /// Subplan files read for the master's combined view, with their modified time.
    files: HashMap<PathBuf, (std::time::SystemTime, Project)>,
}

struct St(Mutex<App>);

impl St {
    /// The open plan. A command that panicked must not lock every later one out, so a poisoned lock is reused.
    fn lock(&self) -> std::sync::MutexGuard<'_, App> { self.0.lock().unwrap_or_else(|e| e.into_inner()) }
}
type R<T> = Result<T, String>;

/// Runs an action so that an internal error (a panic) becomes an error message instead of
/// closing the app. Edits work on a copy of the plan, so the plan is left as it was.
fn guard<T>(f: impl FnOnce() -> R<T>) -> R<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|_| {
        Err("Datum Plan met an internal error and stopped that action; your plan is as it was. Details are in crash.log; please report it (Help › Report an Issue).".into())
    })
}

/// What the window needs after every change: the project settings (everything except tasks),
/// its overall figures and the file state. Rows come separately through `query`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Out {
    /// The open plans, one per tab, and which one is shown.
    tabs: Vec<TabInfo>,
    tab: usize,
    project: serde_json::Value,
    summary: query::Summary,
    path: Option<String>,
    dirty: bool,
    undo: bool,
    redo: bool,
    clip: usize,
    rev: u64,
}

#[derive(Serialize)]
struct TabInfo {
    name: String, path: Option<String>, dirty: bool, moved: usize,
    /// "master", "sub" or "" (a plan on its own); the master's tab (-1 when it is not open), the
    /// depth under the master, and the project's (master's) name.
    role: &'static str, group: i64, level: u8, project: String,
}

/// Every open plan in tab order (the shown one is `a.doc`).
fn docs(a: &App) -> Vec<&Doc> {
    (0..a.others.len()).map(|k| if k == a.cur { &a.doc } else { a.others[k].as_ref().unwrap_or(&a.doc) }).collect()
}

fn tab_list(a: &App) -> Vec<TabInfo> {
    let moved = link_status(a);
    let ds = docs(a);
    ds.iter().enumerate().zip(moved).map(|((k, d), moved)| {
        let (role, group, level, project) = match place(&ds, k) {
            Place::Master => ("master", k as i64, 0, d.p.name.clone()),
            Place::Sub(m, level) => ("sub", m as i64, level, ds[m].p.name.clone()),
            Place::Orphan => ("sub", -1, 0, Path::new(links::file_name(&d.p.master)).file_stem().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()),
            Place::Alone => ("", -1, 0, String::new()),
        };
        TabInfo { name: d.p.name.clone(), path: d.path.as_ref().map(|p| p.display().to_string()), dirty: d.dirty, moved: moved.len(), role, group, level, project }
    }).collect()
}

/// A tab's place in a project: a master, a subplan of the open master in tab `m` (at a depth),
/// a subplan whose master is not open, or a plan on its own.
enum Place { Master, Sub(usize, u8), Orphan, Alone }

fn file_key(d: &Doc) -> Option<String> {
    d.path.as_ref().and_then(|p| p.file_name()).map(|f| f.to_string_lossy().to_lowercase())
}

fn place(ds: &[&Doc], k: usize) -> Place {
    if !ds[k].p.subplans.is_empty() { return Place::Master }
    if let Some(me) = file_key(ds[k]) {
        for (m, d) in ds.iter().enumerate().filter(|(m, _)| *m != k) {
            if let Some(sp) = d.p.subplans.iter().find(|s| links::file_name(&s.file).to_lowercase() == me) { return Place::Sub(m, sp.level) }
        }
    }
    if ds[k].p.master.is_empty() { Place::Alone } else { Place::Orphan }
}

/// Keeps each project's tabs together: the master, then its open subplans in the master's order.
fn regroup(a: &mut App) {
    let ds = docs(a);
    let mut key: Vec<(usize, usize, usize)> = (0..ds.len()).map(|k| match place(&ds, k) {
        Place::Sub(m, _) => {
            let me = file_key(ds[k]).unwrap_or_default();
            (m, 1 + ds[m].p.subplans.iter().position(|s| links::file_name(&s.file).to_lowercase() == me).unwrap_or(0), k)
        }
        _ => (k, 0, k),
    }).collect();
    key.sort();
    let order: Vec<usize> = key.into_iter().map(|x| x.2).collect();
    if order.iter().enumerate().all(|(i, &k)| i == k) { return }
    let cur = a.cur;
    let mut slots: Vec<Option<Doc>> = std::mem::take(&mut a.others);
    a.others = order.iter().map(|&k| slots[k].take()).collect();
    a.cur = order.iter().position(|&k| k == cur).unwrap_or(0);
}

/// For each open plan, its tasks whose links point at a task in another open plan that has
/// moved since the link last saw it (they need Update Links). Only open plans are scheduled.
fn link_status(a: &App) -> Vec<std::collections::HashSet<u32>> {
    let ds = docs(a);
    let key = |d: &Doc| d.path.as_ref().and_then(|p| p.file_name()).map(|f| f.to_string_lossy().to_lowercase());
    let keys: Vec<Option<String>> = ds.iter().map(|d| key(d)).collect();
    let mut now: HashMap<usize, HashMap<u32, (i64, i64)>> = HashMap::new();
    let mut out = vec![std::collections::HashSet::new(); ds.len()];
    for (k, d) in ds.iter().enumerate() {
        for t in &d.p.tasks {
            for x in &t.ext {
                let f = links::file_name(&x.file).to_lowercase();
                let Some(j) = keys.iter().position(|k2| k2.as_deref() == Some(f.as_str())) else { continue };
                let dates = now.entry(j).or_insert_with(|| datum_core::sched::schedule(&ds[j].p).map(|s| ds[j].p.tasks.iter().enumerate().map(|(i, t)| (t.uid, (s.es[i], s.ef[i]))).collect()).unwrap_or_default());
                if dates.get(&x.uid).is_some_and(|&d2| d2 != (x.start, x.finish)) { out[k].insert(t.uid); }
            }
        }
    }
    out
}

/// Shows a plan in a new tab, or in place of an empty untouched one.
fn add_tab(a: &mut App, d: Doc) {
    if a.doc.path.is_none() && !a.doc.dirty && a.doc.p.tasks.is_empty() {
        a.doc = d;
        return;
    }
    let old = std::mem::replace(&mut a.doc, d);
    a.others[a.cur] = Some(old);
    a.others.push(None);
    a.cur = a.others.len() - 1;
    regroup(a);
}

fn show_tab(a: &mut App, k: usize) {
    if k == a.cur || k >= a.others.len() { return }
    let Some(d) = a.others[k].take() else { return };
    let old = std::mem::replace(&mut a.doc, d);
    a.others[a.cur] = Some(old);
    a.cur = k;
}

#[tauri::command]
fn switch_tab(st: State<St>, k: usize) -> Out {
    let mut a = st.lock();
    show_tab(&mut a, k);
    out(&a)
}

/// Closes a tab (the window asks about unsaved changes first). The last tab becomes a new plan.
#[tauri::command]
fn close_tab(app: AppHandle, st: State<St>, k: usize) -> Out {
    let mut a = st.lock();
    if k >= a.others.len() { return out(&a) }
    if a.others.len() == 1 {
        a.doc = Doc::new(Project::new("Project1", today()));
        write_recovery(&app, &a);
        return out(&a);
    }
    if k == a.cur {
        let next = if k + 1 < a.others.len() { k + 1 } else { k - 1 };
        show_tab(&mut a, next);
    }
    a.others.remove(k);
    if a.cur > k { a.cur -= 1 }
    write_recovery(&app, &a);
    out(&a)
}

/// Closes every plan (the window asks about unsaved changes first) and leaves one new plan.
#[tauri::command]
fn close_all(app: AppHandle, st: State<St>) -> Out {
    let mut a = st.lock();
    a.doc = Doc::new(Project::new("Project1", today()));
    a.others = vec![None];
    a.cur = 0;
    write_recovery(&app, &a);
    out(&a)
}

fn out(a: &App) -> Out {
    let d = &a.doc;
    let mut project = serde_json::to_value(&d.p).unwrap_or_default();
    if let Some(m) = project.as_object_mut() {
        m.remove("tasks");
        if let Some(snaps) = m.get_mut("snapshots").and_then(|v| v.as_array_mut()) {
            snaps.iter_mut().filter_map(|x| x.as_object_mut()).for_each(|x| { x.remove("rows"); });
        }
    }
    Out {
        tabs: tab_list(a),
        tab: a.cur,
        project,
        summary: calc(a).summary(),
        path: d.path.as_ref().map(|p| p.display().to_string()),
        dirty: d.dirty,
        undo: d.can_undo(),
        redo: d.can_redo(),
        clip: a.clip.len(),
        rev: d.undo_len() as u64 * 1_000_003 + d.p.next_uid as u64,
    }
}

fn calc(a: &App) -> Calc<'_> {
    Calc::new(&a.doc.p, &a.opts.date_fmt, today())
}

/// A new plan gets the user's defaults for currency and branding.
fn fresh(a: &App, mut p: Project) -> Project {
    p.currency = a.opts.currency.clone();
    if p.company.is_empty() {
        p.company = a.opts.company.clone();
    }
    if p.logo.is_empty() {
        p.logo = a.opts.logo.clone();
    }
    p.setup_done = !a.opts.setup_panel;
    p.manual_default = a.opts.manual_default;
    p.periods.week_end = a.opts.week_end;
    p.periods.month_end = a.opts.month_end;
    p
}

fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

fn dir(app: &AppHandle, data: bool) -> Option<PathBuf> {
    let d = if data { app.path().app_data_dir() } else { app.path().app_config_dir() }.ok()?;
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

#[tauri::command]
fn state(st: State<St>) -> Out {
    out(&st.lock())
}

#[tauri::command]
fn apply(st: State<St>, op: Op) -> R<Out> {
    guard(|| {
        let mut a = st.lock();
        let fmt = a.opts.date_fmt.clone();
        a.doc.apply(op, &fmt)?;
        regroup(&mut a);
        Ok(out(&a))
    })
}

#[tauri::command]
fn history(st: State<St>, redo: bool) -> Out {
    let mut a = st.lock();
    if redo { a.doc.redo() } else { a.doc.undo() }
    out(&a)
}

/// Templates start next Monday so the first week is a full one.
fn next_monday() -> chrono::NaiveDate {
    use chrono::Datelike;
    let t = today();
    t + chrono::Days::new(7 - t.weekday().num_days_from_monday() as u64)
}

#[derive(Serialize)]
struct Tpl { id: &'static str, name: &'static str, desc: &'static str, color: &'static str, tasks: usize, days: i64, bars: Vec<(u8, f64, f64)> }

/// Template cards with a miniature Gantt: (0 task / 1 summary / 2 milestone, start, end) as 0–1 fractions.
#[tauri::command]
fn templates() -> Vec<Tpl> {
    templates::TEMPLATES
        .iter()
        .map(|t| {
            let p = templates::build(t, next_monday());
            let c = Calc::new(&p, "%d-%b-%y", today());
            let (s0, f0) = (c.s.start as f64 / 1440.0, c.s.finish as f64 / 1440.0);
            let span = (f0 - s0).max(1.0);
            let bars = (0..p.tasks.len()).map(|i| {
                let kind = if c.s.summary[i] { 1 } else if p.tasks[i].dur == 0 { 2 } else { 0 };
                (kind, (c.s.es[i] as f64 / 1440.0 - s0) / span, (c.s.ef[i] as f64 / 1440.0 - s0) / span)
            }).collect();
            Tpl { id: t.id, name: t.name, desc: t.desc, color: t.color, tasks: (0..p.tasks.len()).filter(|&i| !c.s.summary[i]).count(), days: (f0 - s0).ceil() as i64, bars }
        })
        .collect()
}

#[tauri::command]
fn new_plan(st: State<St>, template: Option<String>) -> Out {
    let p = match templates::TEMPLATES.iter().find(|t| Some(t.id) == template.as_deref()) {
        Some(t) => templates::build(t, next_monday()),
        None => Project::new("Project1", today()),
    };
    let mut a = st.lock();
    let p = fresh(&a, p);
    add_tab(&mut a, Doc::new(p));
    out(&a)
}

fn pick(app: &AppHandle, name: &str, exts: &[&str]) -> Option<PathBuf> {
    app.dialog().file().add_filter(name, exts).blocking_pick_file().and_then(|f| f.into_path().ok())
}

#[tauri::command]
async fn open(app: AppHandle, st: State<'_, St>, path: Option<String>) -> R<Option<Out>> {
    let Some(path) = path.map(PathBuf::from).or_else(|| pick(&app, "Datum Plan", &["dplan"])) else { return Ok(None) };
    backup_old(&path);
    // A plan that is already open is shown, not opened twice.
    {
        let mut a = st.lock();
        let same = |d: &Doc| d.path.as_ref().is_some_and(|x| x == &path || x.canonicalize().ok() == path.canonicalize().ok());
        if let Some(k) = docs(&a).iter().position(|d| same(d)) {
            show_tab(&mut a, k);
            return Ok(Some(out(&a)));
        }
    }
    let d = guard(|| Doc::open(&path))?;
    remember(&app, &path);
    let mut a = st.lock();
    add_tab(&mut a, d);
    Ok(Some(out(&a)))
}

/// Plans saved by an older version are copied to name.backup.dplan before they are opened,
/// because saving them again writes the newer format.
fn backup_old(path: &Path) {
    let old = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).and_then(|v| v["format"].as_u64()).is_some_and(|f| f < 3);
    let backup = path.with_extension("backup.dplan");
    if old && !backup.exists() {
        let _ = std::fs::copy(path, backup);
    }
}

#[tauri::command]
async fn save(app: AppHandle, st: State<'_, St>, as_new: bool) -> R<Option<Out>> {
    let (known, name) = {
        let a = st.lock();
        (a.doc.path.clone().filter(|_| !as_new), a.doc.p.name.clone())
    };
    let path = match known {
        Some(p) => p,
        None => match app.dialog().file().add_filter("Datum Plan", &["dplan"]).set_file_name(format!("{name}.dplan")).blocking_save_file() {
            Some(f) => f.into_path().map_err(|e| e.to_string())?,
            None => return Ok(None),
        },
    };
    let mut a = st.lock();
    a.doc.save(&path)?;
    remember(&app, &path);
    write_recovery(&app, &a);
    Ok(Some(out(&a)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Imported {
    plan: Option<Out>,
    /// The import report: what was changed or left out, and how the dates compare with the file.
    notes: Vec<String>,
    table: Option<io::table::Table>,
    fields: Vec<(&'static str, &'static str)>,
}

/// Opens an exchange file. Plans load straight away; spreadsheets come back for column mapping.
#[tauri::command]
async fn import(app: AppHandle, st: State<'_, St>) -> R<Option<Imported>> {
    let Some(path) = pick(&app, "Project files", &["xml", "xer", "xlsx", "xls", "csv", "json", "mpp"]) else { return Ok(None) };
    let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let fields = io::table::FIELDS.iter().map(|f| (f.0, f.1)).collect();
    let mut a = st.lock();
    match guard(|| io::import(&path))? {
        io::Imported::Plan(p, notes) => {
            let p = fresh(&a, *p);
            let mut d = Doc::new(p);
            d.dirty = true;
            add_tab(&mut a, d);
            Ok(Some(Imported { plan: Some(out(&a)), notes, table: None, fields }))
        }
        io::Imported::Table(t) => {
            let shown = io::table::Table { headers: t.headers.clone(), rows: t.rows.iter().take(8).cloned().collect(), guess: t.guess.clone() };
            a.table = Some((t, name));
            Ok(Some(Imported { plan: None, notes: vec![], table: Some(shown), fields }))
        }
    }
}

#[derive(Serialize)]
struct TableResult { plan: Out, skipped: usize }

#[tauri::command]
fn import_table(st: State<St>, map: Vec<String>) -> R<TableResult> {
    let mut a = st.lock();
    let (t, name) = a.table.take().ok_or("Choose a file to import first.")?;
    let (p, skipped) = io::table::to_project(&t, &map, &name, today(), &a.opts.date_fmt).inspect_err(|_| a.table = Some((io::table::Table { headers: t.headers.clone(), rows: t.rows.clone(), guess: t.guess.clone() }, name.clone())))?;
    let p = fresh(&a, p);
    let mut d = Doc::new(p);
    d.dirty = true;
    add_tab(&mut a, d);
    Ok(TableResult { plan: out(&a), skipped })
}

#[tauri::command]
async fn export(app: AppHandle, st: State<'_, St>, kind: String, cols: Vec<String>) -> R<Option<String>> {
    let (label, ext) = match kind.as_str() {
        "xlsx" => ("Excel workbook", "xlsx"),
        "csv" => ("CSV", "csv"),
        "xml" => ("Project XML", "xml"),
        "xer" => ("XER", "xer"),
        "pmxml" => ("PMXML", "pmxml"),
        "mpx" => ("MPX", "mpx"),
        "json" => ("JSON", "json"),
        "html" => ("Web page", "html"),
        "ics" => ("iCalendar", "ics"),
        k => return Err(format!("Unknown export type {k}.")),
    };
    let name = st.lock().doc.p.name.clone();
    let Some(path) = app.dialog().file().add_filter(label, &[ext]).set_file_name(format!("{name}.{ext}")).blocking_save_file().and_then(|f| f.into_path().ok()) else { return Ok(None) };
    let a = st.lock();
    io::export(&a.doc.p, &path, &a.opts.date_fmt, today(), &cols)?;
    Ok(Some(std::iter::once(path.display().to_string()).chain(io::export_notes(&a.doc.p, &path)).collect::<Vec<_>>().join(". ")))
}

/// Brings the sheets of a workbook (.xlsx, .xlsm, .xls, .xlsb, .ods) in as sheet tabs: one step to undo.
#[tauri::command]
async fn import_workbook(app: AppHandle, st: State<'_, St>) -> R<Option<(Out, Vec<String>)>> {
    let Some(path) = pick(&app, "Workbooks", &["xlsx", "xlsm", "xls", "xlsb", "ods"]) else { return Ok(None) };
    if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) > 100_000_000 { return Err("That workbook is larger than 100 MB.".into()) }
    let got = guard(|| io::sheetbook::import(&path))?;
    let names: Vec<String> = got.sheets.iter().map(|s| s.name.clone()).collect();
    let mut a = st.lock();
    let fmt = a.opts.date_fmt.clone();
    let add = Op::AddSheets { sheets: got.sheets, pictures: got.pictures, freeze: got.freeze };
    // The workbook's defined names join the plan's (a name the plan has keeps its own meaning).
    let op = if got.names.is_empty() { add } else { Op::Batch(vec![add, Op::Names { names: datum_core::sheetx::merge_names(&a.doc.p.names, got.names) }]) };
    a.doc.apply(op, &fmt)?;
    regroup(&mut a);
    Ok(Some((out(&a), names)))
}

/// Moves or copies sheet `k` of the plan shown into the open plan in tab `to` (a master's sheet into
/// one of its subplans): it lands as that plan's last sheet, with its pictures and panes. Each plan
/// undoes its own part. Says how many formulas read other sheets of this plan.
#[tauri::command]
fn sheet_to_plan(st: State<St>, k: usize, to: usize, copy: bool) -> R<(Out, String)> {
    let mut a = st.lock();
    if to == a.cur || to >= a.others.len() { return Err("Choose another open plan.".into()) }
    let (op, reads) = datum_core::sheetmove::package(&a.doc.p, k).ok_or("That sheet does not exist.")?;
    let (name, dest) = (a.doc.p.sheets[k].name.clone(), docs(&a)[to].p.name.clone());
    apply_at(&mut a, to, op)?;
    if !copy { let fmt = a.opts.date_fmt.clone(); a.doc.apply(Op::Sheet { k: Some(k), sheet: None }, &fmt)?; }
    let note = if reads > 0 { format!(" {reads} formula{} read other sheets of this plan: in {dest} they read its sheets of the same name, or show #REF!.", if reads > 1 { "s" } else { "" }) } else { String::new() };
    Ok((out(&a), format!("{name} {} {dest}; open its tab to see it.{note}", if copy { "copied to" } else { "moved to" })))
}

/// Saves the plan's sheets as a workbook to share (the plan data their formulas read on hidden sheets).
#[tauri::command]
async fn export_sheets(app: AppHandle, st: State<'_, St>) -> R<Option<String>> {
    let name = st.lock().doc.p.name.clone();
    let Some(path) = app.dialog().file().add_filter("Excel workbook", &["xlsx"]).set_file_name(format!("{name} - sheets.xlsx")).blocking_save_file().and_then(|f| f.into_path().ok()) else { return Ok(None) };
    let a = st.lock();
    let c = datum_core::fields::Calc::new(&a.doc.p, &a.opts.date_fmt, today());
    io::table::write_sheets_xlsx(&c, &path)?;
    Ok(Some(path.display().to_string()))
}

/// Saves a picture or a report the window made (SVG, PNG data URL or HTML text).
#[tauri::command]
async fn save_file(app: AppHandle, name: String, data: String) -> R<Option<String>> {
    let ext = name.rsplit('.').next().unwrap_or("txt").to_string();
    let Some(path) = app.dialog().file().add_filter(ext.to_uppercase(), &[&ext]).set_file_name(&name).blocking_save_file().and_then(|f| f.into_path().ok()) else { return Ok(None) };
    let bytes = match data.strip_prefix("data:").and_then(|d| d.split_once(";base64,")) {
        Some((_, b64)) => base64_decode(b64)?,
        None => data.into_bytes(),
    };
    doc::write_atomic(&path, &bytes)?;
    Ok(Some(path.display().to_string()))
}

fn base64_decode(s: &str) -> R<Vec<u8>> {
    let val = |c: u8| match c { b'A'..=b'Z' => Some(c - b'A'), b'a'..=b'z' => Some(c - b'a' + 26), b'0'..=b'9' => Some(c - b'0' + 52), b'+' => Some(62), b'/' => Some(63), _ => None };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        acc = acc << 6 | val(c).ok_or("The picture data is damaged.")? as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

/// Reads a picture for the report logo and returns it as a data URL.
#[tauri::command]
async fn pick_logo(app: AppHandle) -> R<Option<String>> {
    let Some(path) = pick(&app, "Pictures", &["png", "jpg", "jpeg", "svg", "gif"]) else { return Ok(None) };
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    if bytes.len() > 1_500_000 {
        return Err("Choose a picture smaller than 1.5 MB.".into());
    }
    let mime = match path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase().as_str() { "svg" => "image/svg+xml", "jpg" | "jpeg" => "image/jpeg", "gif" => "image/gif", _ => "image/png" };
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut b64 = String::with_capacity(bytes.len() * 4 / 3 + 4);
    for ch in bytes.chunks(3) {
        let n = (ch[0] as u32) << 16 | (*ch.get(1).unwrap_or(&0) as u32) << 8 | *ch.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            b64.push(if k <= ch.len() { T[(n >> (18 - 6 * k) & 63) as usize] as char } else { '=' });
        }
    }
    Ok(Some(format!("data:{mime};base64,{b64}")))
}

#[tauri::command]
fn copy(st: State<St>, uids: Vec<u32>) -> usize {
    let mut a = st.lock();
    a.clip = doc::copy(&a.doc.p, &uids);
    a.clip.len()
}

#[tauri::command]
fn paste(st: State<St>, at: usize) -> R<Out> {
    let mut a = st.lock();
    let (tasks, fmt) = (a.clip.clone(), a.opts.date_fmt.clone());
    a.doc.apply(Op::Paste { at, tasks }, &fmt)?;
    Ok(out(&a))
}

/// The rows a view shows: filtered, grouped, sorted, with cells and bars.
#[tauri::command]
fn query(st: State<St>, q: Query, subs: Option<SubView>) -> query::Grid {
    let mut a = st.lock();
    let mut g = calc(&a).grid(&q);
    if let Some(v) = subs.filter(|v| v.show && !a.doc.p.subplans.is_empty()) { combined(&mut a, &q, &v, &mut g) }
    let by = waited_on(&a);
    let moved = link_status(&a).swap_remove(a.cur);
    for r in &mut g.rows {
        if let Some(v) = by.get(&r.uid) { r.ext_out = v.iter().map(|x| x.1.clone()).collect() }
        r.ext_moved = moved.contains(&r.uid);
    }
    g
}

/// The master's combined view: which subplans to show and which are expanded (their files).
#[derive(Deserialize, Default)]
struct SubView { show: bool, open: Vec<String> }

/// Adds the master's subplans under its own rows: each as a summary row (dates, progress, the
/// columns shown) and, when expanded, its tasks, read-only and without links drawn.
fn combined(a: &mut App, q: &Query, v: &SubView, g: &mut query::Grid) {
    let here = a.doc.path.clone();
    let list = a.doc.p.subplans.clone();
    let opened: std::collections::HashSet<String> = v.open.iter().map(|f| links::file_name(f).to_lowercase()).collect();
    for sp in &list {
        let key = links::file_name(&sp.file).to_lowercase();
        let p = match tab_of(a, &sp.file) {
            Some(k) => Some(docs(a)[k].p.clone()),
            None => sub_path(here.as_deref(), &sp.file).and_then(|path| read_cached(a, &path)),
        };
        let Some(p) = p else {
            g.rows.push(query::GRow { kind: "subplan", lvl: sp.level, name: format!("{} (file not found)", sp.name), sub: sp.file.clone(), ..Default::default() });
            continue;
        };
        let c = Calc::new(&p, &a.opts.date_fmt, today());
        let sq = Query { cols: q.cols.clone(), project_summary: true, baseline: q.baseline, ..Query::default() };
        let mut rows = c.grid(&sq).rows.into_iter();
        let Some(mut head) = rows.next() else { continue };
        let open = opened.contains(&key);
        (head.kind, head.lvl, head.name, head.sub, head.open, head.kids) = ("subplan", sp.level, p.name.clone(), sp.file.clone(), open, true);
        g.rows.push(head);
        if !open { continue }
        for mut r in rows {
            (r.suid, r.uid, r.sub) = (r.uid, 0, sp.file.clone());
            r.lvl = r.lvl.saturating_add(sp.level + 1);
            r.bar.links.clear();
            r.ext_out.clear();
            g.rows.push(r);
        }
    }
}

/// A plan file read for the combined view, kept until the file changes.
fn read_cached(a: &mut App, path: &Path) -> Option<Project> {
    let when = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    if let Some((t, p)) = a.files.get(path) { if *t == when { return Some(p.clone()) } }
    let p = Doc::open(path).ok()?.p;
    a.files.insert(path.to_path_buf(), (when, p.clone()));
    Some(p)
}

/// Tasks of the shown plan that tasks in the other open plans wait for: uid → (the other plan's
/// tab, "Main schedule › A1040 Engineering complete (FS)").
fn waited_on(a: &App) -> HashMap<u32, Vec<(usize, String, u32)>> {
    let mut by: HashMap<u32, Vec<(usize, String, u32)>> = HashMap::new();
    let Some(me) = a.doc.path.as_ref().and_then(|p| p.file_name()).map(|f| f.to_string_lossy().to_lowercase()) else { return by };
    for (k, d) in docs(a).into_iter().enumerate().filter(|&(k, _)| k != a.cur) {
        for t in &d.p.tasks {
            for x in t.ext.iter().filter(|x| links::file_name(&x.file).to_lowercase() == me) {
                let code = if t.code.is_empty() { String::new() } else { format!("{} ", t.code) };
                by.entry(x.uid).or_default().push((k, format!("{} › {code}{} ({:?})", d.p.name, t.name, x.kind), t.uid));
            }
        }
    }
    by
}

#[derive(Serialize)]
struct LinkUser { uid: u32, task: String, tab: usize, by: String }

/// For the Linked Plans panel: which tasks of this plan other open plans wait for.
#[tauri::command]
fn link_users(st: State<St>) -> Vec<LinkUser> {
    let a = st.lock();
    let mut v: Vec<LinkUser> = waited_on(&a).into_iter().flat_map(|(uid, xs)| {
        let task = a.doc.p.idx(uid).map(|i| { let t = &a.doc.p.tasks[i]; if t.code.is_empty() { t.name.clone() } else { format!("{} {}", t.code, t.name) } }).unwrap_or_default();
        xs.into_iter().map(move |(tab, by, _)| LinkUser { uid, task: task.clone(), tab, by })
    }).collect();
    v.sort_by_key(|x| (a.doc.p.idx(x.uid), x.tab));
    v
}

#[tauri::command]
fn resources(st: State<St>, cols: Vec<String>) -> Vec<(u32, bool, Vec<String>)> {
    calc(&st.lock()).res_grid(&cols)
}

#[derive(Serialize)]
struct FieldList { task: &'static [fields::FieldDef], resource: &'static [fields::FieldDef], filters: Vec<query::Filter> }

#[tauri::command]
fn field_list() -> FieldList {
    FieldList { task: fields::task_fields(), resource: fields::resource_fields(), filters: query::builtin_filters() }
}

#[derive(Serialize)]
struct TaskInfo { task: Task, fields: Vec<(String, String, &'static str, String)>, preds: Vec<Rel>, succs: Vec<Rel>, assigns: Vec<AssignRow> }

#[derive(Serialize)]
struct Rel { uid: u32, id: usize, code: String, name: String, kind: datum_core::LinkType, lag: String, driving: bool, tf: String }

#[derive(Serialize)]
struct AssignRow { res: u32, name: String, kind: String, units: String, work: String, cost: String }

/// Everything about one task, for Task Information, Inspect and the details tabs.
#[tauri::command]
fn task_info(st: State<St>, uid: u32) -> Option<TaskInfo> {
    let a = st.lock();
    let c = calc(&a);
    let p = c.p;
    let i = p.idx(uid)?;
    let t = &p.tasks[i];
    let rel = |k: usize, l: &datum_core::Link, driving: bool| Rel {
        uid: p.tasks[k].uid, id: k + 1, code: p.tasks[k].code.clone(), name: p.tasks[k].name.clone(), kind: l.kind,
        lag: if l.pct { format!("{}%", l.lag) } else { datum_core::parse::fmt_lag_el(l.lag, p.day_min, l.elapsed) }, driving, tf: c.text(k, "tf"),
    };
    let drives = |a: usize, b: usize| c.s.edges.iter().enumerate().any(|(e, x)| x.p == a && x.s == b && c.s.drive[e]);
    let preds = t.preds.iter().filter_map(|l| p.idx(l.pred).map(|k| rel(k, l, drives(k, i)))).collect();
    let succs = p.tasks.iter().enumerate().flat_map(|(k, s)| s.preds.iter().filter(|l| l.pred == uid).map(move |l| (k, l))).map(|(k, l)| rel(k, l, drives(i, k))).collect();
    let assigns = t.res.iter().filter_map(|x| {
        let k = p.res_idx(x.res)?;
        let r = &p.resources[k];
        let w: f64 = c.l.assign_daily.range((k, i, i64::MIN)..=(k, i, i64::MAX)).map(|(_, m)| m).sum();
        let cost = match r.kind {
            datum_core::ResKind::Work => w / 60.0 * r.rate_on(x.table, p.start).0 + r.per_use,
            datum_core::ResKind::Material => x.units * r.rate + r.per_use,
            datum_core::ResKind::Cost => x.units + r.per_use,
        };
        let units = if r.kind == datum_core::ResKind::Work { format!("{}%", (x.units * 100.0).round()) } else { format!("{}", x.units) };
        Some(AssignRow { res: r.uid, name: r.name.clone(), kind: format!("{:?}", r.kind), units, work: format!("{} hrs", (w / 6.0).round() / 10.0), cost: fields::money(cost, &p.currency) })
    }).collect();
    Some(TaskInfo { task: t.clone(), fields: c.all_fields(i), preds, succs, assigns })
}

#[tauri::command]
fn usage(st: State<St>, by: String, what: String, unit: String) -> report::Usage {
    report::usage(&calc(&st.lock()), &by, &what, &unit)
}

#[tauri::command]
fn curve(st: State<St>, unit: String) -> report::Curve {
    report::curve(&calc(&st.lock()), &unit)
}

#[tauri::command]
/// Runs off the window's thread: a big plan takes a few seconds.
async fn risk_run(st: State<'_, St>, opts: datum_core::risk::SimOpts) -> R<datum_core::risk::SimResult> {
    let p = st.lock().doc.p.clone();
    guard(|| datum_core::risk::simulate(&p, &opts))
}

#[tauri::command]
fn manpower(st: State<St>, unit: String) -> datum_core::progress::Manpower {
    datum_core::progress::manpower(&calc(&st.lock()), &unit)
}

#[tauri::command]
fn earned_schedule(st: State<St>) -> Option<datum_core::progress::EarnedSchedule> {
    datum_core::progress::earned_schedule(&calc(&st.lock()))
}

#[tauri::command]
fn lookahead(st: State<St>, weeks: u32) -> datum_core::progress::Lookahead {
    datum_core::progress::lookahead(&calc(&st.lock()), weeks)
}

#[tauri::command]
fn credit_library() -> Vec<datum_core::CreditRule> {
    datum_core::progress::credit_library()
}

#[tauri::command]
fn level_preview(st: State<St>, uids: Vec<u32>) -> R<datum_core::level::Preview> {
    guard(|| datum_core::level::preview(&st.lock().doc.p, &uids))
}

#[tauri::command]
fn checks(st: State<St>, limits: check::Limits) -> Vec<check::Check> {
    check::run(&calc(&st.lock()), &limits)
}

/// Network Diagram of the tasks the query shows (summaries left out).
#[tauri::command]
fn network(st: State<St>, q: Query, summaries: Option<bool>, wbs: Option<bool>) -> network::Net {
    let a = st.lock();
    let c = calc(&a);
    let g = c.grid(&q);
    let pick = |k: &str| g.rows.iter().filter(|r| r.kind == k).filter_map(|r| c.p.idx(r.uid)).collect::<Vec<usize>>();
    let sums = if summaries.unwrap_or(false) { pick("summary") } else { vec![] };
    network::layout_with(&c, &pick("task"), &sums, wbs.unwrap_or(false))
}

/// Compares the open plan with another plan file.
#[tauri::command]
async fn compare(app: AppHandle, st: State<'_, St>) -> R<Option<(String, Vec<report::Diff>)>> {
    let Some(path) = pick(&app, "Datum Plan", &["dplan", "xml", "xer"]) else { return Ok(None) };
    let old = match io::import(&path)? {
        io::Imported::Plan(p, _) => *p,
        io::Imported::Table(_) => return Err("Compare needs a plan file (.dplan, .xml or .xer).".into()),
    };
    let a = st.lock();
    let fmt = a.opts.date_fmt.clone();
    let diffs = report::compare(&Calc::new(&old, &fmt, today()), &calc(&a));
    Ok(Some((path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), diffs)))
}

#[derive(Serialize)]
struct PlanCard { path: String, folder: String, name: String, start: String, finish: String, pct: f64, cost: String, spi: Option<f64>, cpi: Option<f64>, late: usize, tasks: usize, manager: String, error: Option<String> }

/// Every plan in the workspace folder with its headline figures (portfolio view).
#[tauri::command]
async fn workspace(app: AppHandle, st: State<'_, St>, choose: bool) -> R<(String, Vec<PlanCard>)> {
    let mut folder = st.lock().opts.workspace.clone();
    if choose || folder.is_empty() {
        let Some(d) = app.dialog().file().blocking_pick_folder().and_then(|f| f.into_path().ok()) else { return Ok((folder, vec![])) };
        folder = d.display().to_string();
        let mut a = st.lock();
        a.opts.workspace = folder.clone();
        if let Some(f) = options_file(&app) {
            let _ = doc::write_atomic(&f, serde_json::to_string(&a.opts).unwrap_or_default().as_bytes());
        }
    }
    let fmt = st.lock().opts.date_fmt.clone();
    // Subfolders are the EPS: each level of folders is a level of the portfolio tree.
    fn walk(dir: &Path, depth: u8, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for p in rd.filter_map(|e| e.ok().map(|e| e.path())) {
            if p.is_dir() && depth < 4 { walk(&p, depth + 1, out) }
            else if p.extension().is_some_and(|x| x == "dplan") && !p.to_string_lossy().ends_with(".backup.dplan") { out.push(p) }
        }
    }
    std::fs::read_dir(&folder).map_err(|e| format!("Could not read {folder}: {e}"))?;
    let mut files = vec![];
    walk(Path::new(&folder), 0, &mut files);
    files.sort();
    let rel = |f: &Path| f.parent().and_then(|d| d.strip_prefix(&folder).ok()).map(|d| d.display().to_string().replace(['\\', '/'], " › ")).unwrap_or_default();
    let cards = files.iter().take(200).map(|f| {
        let path = f.display().to_string();
        match Doc::open(f) {
            Ok(d) => {
                let c = Calc::new(&d.p, &fmt, today());
                let s = c.summary();
                PlanCard { path, folder: rel(f), name: d.p.name.clone(), start: s.start, finish: s.finish, pct: s.pct, cost: s.cost, spi: s.spi, cpi: s.cpi, late: s.late, tasks: s.tasks, manager: d.p.manager.clone(), error: s.error }
            }
            Err(e) => PlanCard { path, folder: rel(f), name: f.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), start: String::new(), finish: String::new(), pct: 0.0, cost: String::new(), spi: None, cpi: None, late: 0, tasks: 0, manager: String::new(), error: Some(e) },
        }
    }).collect();
    Ok((folder, cards))
}

#[tauri::command]
fn sheet_view(st: State<St>, k: usize) -> Option<datum_core::sheet::SheetOut> {
    datum_core::sheet::sheet_view(&calc(&st.lock()), k)
}

/// A value typed into a sheet cell against the cell's data validation: None when it may go in,
/// otherwise the message and whether it is only a warning.
#[tauri::command]
fn sheet_check(st: State<St>, k: usize, cell: String, input: String) -> Option<(String, bool)> {
    datum_core::sheet::check_input(&calc(&st.lock()), k, &cell, &input)
}

/// What a sheet formula can name: functions (with signatures), the plan's names and table columns.
#[derive(serde::Serialize)]
struct SheetCatalog { functions: Vec<(&'static str, &'static str, &'static str, String)>, names: Vec<(String, String)>, tasks: Vec<String>, resources: Vec<&'static str> }

#[tauri::command]
fn sheet_catalog(st: State<St>) -> SheetCatalog {
    let a = st.lock();
    let c = calc(&a);
    SheetCatalog {
        functions: datum_core::sheet::functions(),
        names: datum_core::sheet::project_names(&c).into_iter().map(|(n, _, d)| (n.to_string(), d.to_string()))
            .chain(a.doc.p.names.iter().map(|n| (n.name.clone(), if n.comment.is_empty() { n.formula.clone() } else { format!("{} ({})", n.comment, n.formula) }))).collect(),
        tasks: datum_core::sheet::task_columns(&c).into_iter().map(|x| x.1).collect(),
        resources: datum_core::sheet::RES_COLS.iter().map(|x| x.1).collect(),
    }
}

/// Dashboard tiles: each formula's result as a table of shown values (see `sheet::eval`).
#[tauri::command]
fn dash_eval(st: State<St>, items: Vec<(String, String)>) -> Vec<Vec<Vec<datum_core::sheet::Shown>>> {
    datum_core::sheet::eval(&calc(&st.lock()), &items[..items.len().min(200)])
}

/// Formulas moved by rows and columns, for fill and paste (relative references follow).
#[tauri::command]
fn formula_shift(items: Vec<(String, i64, i64)>) -> Vec<String> {
    items.iter().take(200_000).map(|(f, dr, dc)| datum_core::sheet::shift(f, *dr, *dc)).collect()
}

/// Saves every tab with unsaved changes (asking for a file name where a plan has none).
/// Returns None when a file name was not given.
#[tauri::command]
async fn save_all(app: AppHandle, st: State<'_, St>) -> R<Option<Out>> {
    let n = st.lock().others.len();
    for k in 0..n {
        let (dirty, path, name) = {
            let mut a = st.lock();
            show_tab(&mut a, k);
            (a.doc.dirty, a.doc.path.clone(), a.doc.p.name.clone())
        };
        if !dirty { continue }
        let path = match path {
            Some(p) => p,
            None => match app.dialog().file().add_filter("Datum Plan", &["dplan"]).set_file_name(format!("{name}.dplan")).blocking_save_file() {
                Some(f) => f.into_path().map_err(|e| e.to_string())?,
                None => return Ok(None),
            },
        };
        st.lock().doc.save(&path)?;
        remember(&app, &path);
    }
    let a = st.lock();
    write_recovery(&app, &a);
    Ok(Some(out(&a)))
}

/// A plan another plan links to: an open tab with that file, or the file read from disk
/// (looked for where it was, and next to this plan when the folder moved).
fn linked_plan(a: &App, file: &str, here: Option<&Path>) -> Option<(String, Project)> {
    let name = links::file_name(file).to_lowercase();
    // The tab with that exact file first; a tab of the same name only when none matches (the folder moved).
    let exact = |d: &&Doc| d.path.as_ref().is_some_and(|p| p.display().to_string().eq_ignore_ascii_case(file));
    let named = |d: &&Doc| d.path.as_ref().is_some_and(|p| p.file_name().is_some_and(|f| f.to_string_lossy().to_lowercase() == name));
    if let Some(d) = docs(a).into_iter().find(exact).or_else(|| docs(a).into_iter().find(named)) {
        return Some((name, d.p.clone()));
    }
    sub_path(here, file).and_then(|p| Doc::open(&p).ok()).map(|d| (name, d.p))
}

// ---------- a project: a master plan and its subplans ----------

/// Applies an edit to the plan in tab `k` (shown or not).
fn apply_at(a: &mut App, k: usize, op: Op) -> R<()> {
    let fmt = a.opts.date_fmt.clone();
    let d = if k == a.cur { &mut a.doc } else { a.others.get_mut(k).and_then(Option::as_mut).ok_or("That plan is not open.")? };
    d.apply(op, &fmt)
}

/// Saves the plan in tab `k` after a project change when it had no other unsaved changes, so the
/// master's list and a subplan's note of its master are not lost (other edits are never saved unasked).
fn save_if_clean(a: &mut App, k: usize, was_clean: bool) -> R<()> {
    let d = if k == a.cur { &mut a.doc } else { a.others.get_mut(k).and_then(Option::as_mut).ok_or("That plan is not open.")? };
    match (was_clean, d.path.clone()) { (true, Some(p)) => d.save(&p), _ => Ok(()) }
}

/// The tab showing a file, if it is open.
fn tab_of(a: &App, file: &str) -> Option<usize> {
    let name = links::file_name(file).to_lowercase();
    docs(a).iter().position(|d| file_key(d).as_deref() == Some(name.as_str()))
}

/// A plan file another plan names: where it was saved, or next to the master when the folder moved.
/// A network path (\\server\share) is only tried on the master's own server: a plan from someone
/// else must not make Windows connect, and sign in, to a server it names.
fn sub_path(master: Option<&Path>, file: &str) -> Option<PathBuf> {
    use std::path::{Component, Prefix};
    let server = |p: &Path| match p.components().next() { Some(Component::Prefix(x)) => match x.kind() { Prefix::UNC(s, _) | Prefix::VerbatimUNC(s, _) => Some(s.to_ascii_lowercase()), Prefix::Disk(_) | Prefix::VerbatimDisk(_) => None, _ => Some(Default::default()) }, _ => None };
    let saved = PathBuf::from(file);
    let saved = (server(&saved).is_none() || server(&saved) == master.and_then(server)).then_some(saved);
    let near = master.and_then(Path::parent).map(|d| d.join(links::file_name(file)));
    [saved, near].into_iter().flatten().find(|p| p.is_file())
}

/// Project › Subplans › New Subplan: a new plan saved next to the master (same calendars and
/// currency), added to the master's list at position `at` and depth `level`, and shown in a tab.
#[tauri::command]
fn new_subplan(app: AppHandle, st: State<St>, name: String, level: u8, at: usize) -> R<Out> {
    let mut a = st.lock();
    let here = a.doc.path.clone().ok_or("Save the master plan first, so its subplans can be saved next to it.")?;
    let clean = !a.doc.dirty;
    let folder = here.parent().ok_or("The master plan's folder was not found.")?.to_path_buf();
    let name = name.trim().chars().filter(|c| !"\\/:*?\"<>|".contains(*c)).collect::<String>();
    let name = if name.is_empty() { "Subplan".to_string() } else { name };
    let mut path = folder.join(format!("{name}.dplan"));
    let mut n = 2;
    while path.exists() { path = folder.join(format!("{name} ({n}).dplan")); n += 1; }
    let m = &a.doc.p;
    let mut p = fresh(&a, Project::new(&name, m.start));
    (p.calendars, p.calendar, p.currency, p.day_min) = (m.calendars.clone(), m.calendar.clone(), m.currency.clone(), m.day_min);
    p.master = links::file_name(&here.display().to_string()).to_string();
    p.setup_done = true;
    p.normalize();
    let mut d = Doc::new(p);
    d.save(&path)?;
    remember(&app, &path);
    let mut list = a.doc.p.subplans.clone();
    list.insert(at.min(list.len()), datum_core::SubPlan { file: path.display().to_string(), name, level });
    let k = a.cur;
    apply_at(&mut a, k, Op::Subplans { list })?;
    save_if_clean(&mut a, k, clean)?;
    add_tab(&mut a, d);
    Ok(out(&a))
}

/// Project › Subplans › Add Existing: a saved plan becomes a subplan of the shown master.
#[tauri::command]
async fn add_subplan(app: AppHandle, st: State<'_, St>) -> R<Option<Out>> {
    if st.lock().doc.path.is_none() { return Err("Save the master plan first.".into()) }
    let Some(path) = pick(&app, "Datum Plan", &["dplan"]) else { return Ok(None) };
    let mut a = st.lock();
    let here = a.doc.path.clone().unwrap_or_default();
    if path == here { return Err("A plan cannot be its own subplan.".into()) }
    let master = links::file_name(&here.display().to_string()).to_string();
    let k = match tab_of(&a, &path.display().to_string()) {
        Some(k) => k,
        None => {
            let d = Doc::open(&path)?;
            add_tab(&mut a, d);
            let k = a.cur;
            if let Some(m) = tab_of(&a, &here.display().to_string()) { show_tab(&mut a, m) }
            k
        }
    };
    let sub = docs(&a)[k];
    if !sub.p.subplans.is_empty() { return Err(format!("“{}” is a master plan itself; a project has one master.", sub.p.name)) }
    let item = datum_core::SubPlan { file: path.display().to_string(), name: sub.p.name.clone(), level: 0 };
    let sub_clean = !sub.dirty;
    apply_at(&mut a, k, Op::Master { file: master })?;
    save_if_clean(&mut a, k, sub_clean)?;
    let mut list = a.doc.p.subplans.clone();
    list.push(item);
    let m = a.cur;
    let clean = !a.doc.dirty;
    apply_at(&mut a, m, Op::Subplans { list })?;
    save_if_clean(&mut a, m, clean)?;
    regroup(&mut a);
    remember(&app, &path);
    Ok(Some(out(&a)))
}

/// Takes a plan out of the shown master's project (its file stays as it is; an open one forgets its master).
#[tauri::command]
fn remove_subplan(st: State<St>, file: String) -> R<Out> {
    let mut a = st.lock();
    let key = links::file_name(&file).to_lowercase();
    let list: Vec<_> = a.doc.p.subplans.iter().filter(|s| links::file_name(&s.file).to_lowercase() != key).cloned().collect();
    let m = a.cur;
    apply_at(&mut a, m, Op::Subplans { list })?;
    if let Some(k) = tab_of(&a, &file).filter(|&k| k != m) { apply_at(&mut a, k, Op::Master { file: String::new() })? }
    regroup(&mut a);
    Ok(out(&a))
}

/// Opens every subplan of the shown master that is not open yet (the master stays shown); says which were not found.
#[tauri::command]
fn open_subplans(app: AppHandle, st: State<St>) -> (Out, Vec<String>) {
    let mut a = st.lock();
    let here = a.doc.path.clone();
    let list = a.doc.p.subplans.clone();
    let mut missing = vec![];
    for sp in &list {
        if tab_of(&a, &sp.file).is_some() { continue }
        match sub_path(here.as_deref(), &sp.file).map(|p| Doc::open(&p).map(|d| (p, d))) {
            Some(Ok((p, d))) => { remember(&app, &p); add_tab(&mut a, d) }
            _ => missing.push(sp.name.clone()),
        }
    }
    if let Some(m) = here.as_ref().and_then(|h| tab_of(&a, &h.display().to_string())) { show_tab(&mut a, m) }
    (out(&a), missing)
}

/// Closes several tabs at once (the window asks about unsaved changes first): a project's tabs.
#[tauri::command]
fn close_tabs(app: AppHandle, st: State<St>, ks: Vec<usize>) -> Out {
    let mut a = st.lock();
    let mut ks = ks;
    ks.sort_unstable();
    ks.dedup();
    for &k in ks.iter().rev() {
        if k >= a.others.len() { continue }
        if a.others.len() == 1 { a.doc = Doc::new(Project::new("Project1", today())); break }
        if k == a.cur { let next = if k + 1 < a.others.len() { k + 1 } else { k - 1 }; show_tab(&mut a, next) }
        a.others.remove(k);
        if a.cur > k { a.cur -= 1 }
    }
    write_recovery(&app, &a);
    out(&a)
}

/// The shown master's subplans with what their files say now: found, open, dates and progress.
#[derive(Serialize)]
struct SubRow { file: String, name: String, level: u8, found: bool, open: bool, tab: i64, start: i64, finish: i64, pct: f64, tasks: usize }

#[tauri::command]
fn subplan_rows(st: State<St>) -> Vec<SubRow> {
    let a = st.lock();
    let here = a.doc.path.clone();
    a.doc.p.subplans.iter().map(|sp| {
        let tab = tab_of(&a, &sp.file);
        let p = match tab { Some(k) => Some(docs(&a)[k].p.clone()), None => sub_path(here.as_deref(), &sp.file).and_then(|p| Doc::open(&p).ok()).map(|d| d.p) };
        let mut row = SubRow { file: sp.file.clone(), name: sp.name.clone(), level: sp.level, found: p.is_some(), open: tab.is_some(), tab: tab.map_or(-1, |k| k as i64), start: 0, finish: 0, pct: 0.0, tasks: 0 };
        if let Some(p) = p {
            row.name = p.name.clone();
            if let Ok(s) = datum_core::sched::schedule(&p) {
                let leaf: Vec<usize> = (0..p.tasks.len()).filter(|&i| !s.summary[i] && !s.off[i]).collect();
                let w = |i: usize| (s.ef[i] - s.es[i]).max(1) as f64;
                let total: f64 = leaf.iter().map(|&i| w(i)).sum();
                (row.start, row.finish, row.tasks) = (s.start, s.finish, leaf.len());
                row.pct = if total > 0.0 { leaf.iter().map(|&i| w(i) * p.tasks[i].pct as f64).sum::<f64>() / total } else { 0.0 };
            }
        }
        row
    }).collect()
}

/// Where a task's links lead: the tasks it waits for in other plans ("in") and the tasks in other
/// open plans that wait for it ("out"), for "Go to" in the window.
#[derive(Serialize)]
struct LinkGo { dir: &'static str, label: String, file: String, uid: u32, moved: bool }

#[tauri::command]
fn link_go(st: State<St>, uid: u32) -> Vec<LinkGo> {
    let a = st.lock();
    let Some(i) = a.doc.p.idx(uid) else { return vec![] };
    let moved = link_status(&a).swap_remove(a.cur).contains(&uid);
    let mut v: Vec<LinkGo> = a.doc.p.tasks[i].ext.iter().map(|x| LinkGo { dir: "in", label: links::label(x, a.doc.p.day_min), file: x.file.clone(), uid: x.uid, moved }).collect();
    let ds = docs(&a);
    for (k, label, other) in waited_on(&a).remove(&uid).unwrap_or_default() {
        let file = ds[k].path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        v.push(LinkGo { dir: "out", label, file, uid: other, moved: false });
    }
    v
}

/// File › Print › Save as PDF: asks where, then writes the pages the window has laid out for
/// printing straight to that file (Windows); returns the path, None when cancelled.
#[tauri::command]
async fn save_pdf(app: AppHandle, window: tauri::WebviewWindow, name: String, w_mm: f64, h_mm: f64, margins: [f64; 4]) -> R<Option<String>> {
    if cfg!(not(windows)) { return Err("native-pdf-unavailable".into()) }
    let name: String = name.chars().filter(|c| !"\\/:*?\"<>|".contains(*c)).collect();
    let Some(path) = app.dialog().file().add_filter("PDF", &["pdf"]).set_file_name(format!("{}.pdf", name.trim())).blocking_save_file().and_then(|f| f.into_path().ok()) else { return Ok(None) };
    pdf::write(&window, &path, w_mm, h_mm, margins.map(|m| m.clamp(0.0, 60.0))).await?;
    Ok(Some(path.display().to_string()))
}

/// Shows the plan a link points at (its tab, or opened from its file next to this plan); the
/// window then selects the task.
#[tauri::command]
fn goto_link(app: AppHandle, st: State<St>, file: String) -> R<Out> {
    let mut a = st.lock();
    let name = links::file_name(&file).to_lowercase();
    let same = |d: &Doc| d.path.as_ref().is_some_and(|p| p.file_name().is_some_and(|f| f.to_string_lossy().to_lowercase() == name));
    if let Some(k) = docs(&a).iter().position(|d| same(d)) {
        show_tab(&mut a, k);
        return Ok(out(&a));
    }
    let path = sub_path(a.doc.path.as_deref(), &file).ok_or_else(|| format!("“{}” was not found, where it was or next to this plan.", links::file_name(&file)))?;
    let d = Doc::open(&path)?;
    remember(&app, &path);
    add_tab(&mut a, d);
    Ok(out(&a))
}

/// Link to Plan › Browse: a saved plan that is not open, read (not opened) to pick a task.
#[tauri::command]
async fn link_target_file(app: AppHandle) -> R<Option<Target>> {
    let Some(path) = pick(&app, "Datum Plan", &["dplan"]) else { return Ok(None) };
    let d = Doc::open(&path)?;
    Ok(Some(target(-1, &d)))
}

fn target(tab: i64, d: &Doc) -> Target {
    let s = datum_core::sched::schedule(&d.p).ok();
    let tasks = d.p.tasks.iter().enumerate().map(|(i, t)| (t.uid, t.code.clone(), t.name.clone(), t.level, s.as_ref().is_some_and(|s| s.summary[i]), s.as_ref().map_or(0, |s| s.es[i]), s.as_ref().map_or(0, |s| s.ef[i]))).collect();
    Target { tab, name: d.p.name.clone(), path: d.path.as_ref().map(|p| p.display().to_string()), tasks }
}

/// The other open plans and their tasks, to link to (plans must be saved, so the link knows the file).
#[derive(Serialize)]
struct Target { tab: i64, name: String, path: Option<String>, tasks: Vec<(u32, String, String, u8, bool, i64, i64)> }

#[tauri::command]
fn link_targets(st: State<St>) -> Vec<Target> {
    let a = st.lock();
    docs(&a).into_iter().enumerate().filter(|(k, _)| *k != a.cur).map(|(tab, d)| target(tab as i64, d)).collect()
}

/// Every link of the shown plan to other plans, and whether the other plan has moved since.
#[tauri::command]
fn link_states(st: State<St>) -> Vec<links::LinkState> {
    let a = st.lock();
    let here = a.doc.path.clone();
    let mut cache: HashMap<String, Option<(Project, Option<datum_core::sched::Sched>)>> = HashMap::new();
    links::states(&a.doc.p, &mut |x| {
        let e = cache.entry(x.file.clone()).or_insert_with(|| linked_plan(&a, &x.file, here.as_deref()).map(|(_, p)| { let s = datum_core::sched::schedule(&p).ok(); (p, s) }));
        let (p, s) = e.as_ref().ok_or("the file was not found")?;
        let i = p.idx(x.uid).ok_or("the task is no longer in that plan")?;
        let s = s.as_ref().ok_or("that plan cannot be scheduled")?;
        Ok(links::Found { plan: p.name.clone(), code: p.tasks[i].code.clone(), name: p.tasks[i].name.clone(), start: s.es[i], finish: s.ef[i] })
    })
}

/// Brings links up to date: the shown plan (`all` false) or every open plan. Plans that are not
/// open are read from their files (and left unchanged). Plans may link both ways; the dates are
/// passed round until they settle.
#[tauri::command]
fn update_links(st: State<St>, all: bool) -> R<(Out, String)> {
    let mut a = st.lock();
    // Plans are told apart by their full path (two folders may hold plans of the same name).
    let open: Vec<(String, Project, Option<PathBuf>)> = docs(&a).iter().map(|d| (d.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(), d.p.clone(), d.path.clone())).collect();
    let mut set: Vec<(String, Project)> = open.iter().map(|(f, p, _)| (f.clone(), p.clone())).collect();
    // Closed plans that the open ones link to.
    for (_, p, here) in &open {
        for x in p.tasks.iter().flat_map(|t| &t.ext) {
            if !set.iter().any(|(f, _)| f.eq_ignore_ascii_case(&x.file)) {
                if let Some((_, p)) = linked_plan(&a, &x.file, here.as_deref()) { set.push((x.file.clone(), p)) }
            }
        }
    }
    let (passes, missing) = links::update_all(&mut set)?;
    let mut changed = 0;
    for k in 0..open.len() {
        if !all && k != a.cur { continue }
        let new = &set[k].1;
        let ops: Vec<Op> = new.tasks.iter().zip(&open[k].1.tasks).filter(|(n, o)| n.ext != o.ext).map(|(n, _)| Op::ExtSet { uid: n.uid, links: n.ext.clone() }).collect();
        if ops.is_empty() { continue }
        changed += ops.len();
        let fmt = a.opts.date_fmt.clone();
        let d = if k == a.cur { &mut a.doc } else { match a.others[k].as_mut() { Some(d) => d, None => continue } };
        d.apply(Op::Batch(ops), &fmt)?;
    }
    let total: usize = open.iter().enumerate().filter(|(k, _)| all || *k == a.cur).map(|(_, (_, p, _))| p.tasks.iter().map(|t| t.ext.len()).sum::<usize>()).sum();
    let msg = if total == 0 { "This plan has no links to other plans yet.".to_string() }
        else if changed == 0 { format!("All {total} links are up to date.") }
        else { format!("Updated {changed} {} ({passes} {}).", if changed == 1 { "task's links" } else { "tasks' links" }, if passes == 1 { "pass" } else { "passes" }) };
    let msg = if missing.is_empty() { msg } else { format!("{msg} Not found: {}.", missing.join("; ")) };
    Ok((out(&a), msg))
}

/// File › New › Tutorial: a small fit-out that uses every view, with a note on each (unsaved, in a new tab).
#[tauri::command]
fn tutorial_example(st: State<St>) -> Out {
    let mut a = st.lock();
    let mut p = fresh(&a, datum_core::tutorial::tutorial(today()));
    p.setup_done = true;
    add_tab(&mut a, Doc::new(p));
    out(&a)
}

/// File › New › Example: a main schedule and three linked subplans, saved in a new folder and opened in tabs.
#[tauri::command]
async fn linked_example(app: AppHandle, st: State<'_, St>) -> R<Option<Out>> {
    let Some(base) = app.dialog().file().blocking_pick_folder().and_then(|f| f.into_path().ok()) else { return Ok(None) };
    let mut folder = base.join("Compressor Station Expansion");
    let mut n = 2;
    while folder.exists() { folder = base.join(format!("Compressor Station Expansion ({n})")); n += 1; }
    std::fs::create_dir_all(&folder).map_err(|e| format!("Could not make the folder: {e}"))?;
    let plans = datum_core::templates::linked_example(next_monday());
    let mut a = st.lock();
    let mut first = None;
    for (f, p) in plans {
        let path = folder.join(&f);
        let p = fresh(&a, p);
        let mut d = Doc::new(p);
        d.save(&path)?;
        remember(&app, &path);
        add_tab(&mut a, d);
        first.get_or_insert(a.cur);
    }
    if let Some(k) = first { show_tab(&mut a, k) }
    Ok(Some(out(&a)))
}

#[tauri::command]
fn scurve(st: State<St>, unit: String, how: String, scope: Option<u32>) -> report::SCurve {
    report::scurve(&calc(&st.lock()), &unit, &how, scope)
}

#[tauri::command]
fn periods(st: State<St>, kind: String) -> Vec<i64> {
    report::period_ends(&calc(&st.lock()), &kind)
}

#[tauri::command]
fn period_report(st: State<St>, kind: String, end: Option<i64>) -> report::PeriodReport {
    report::period_report(&calc(&st.lock()), &kind, end)
}

/// The full plan of a snapshot, for trend charts (rows are left out of `state`).
#[tauri::command]
fn snapshot_rows(st: State<St>, k: usize) -> Option<datum_core::Snapshot> {
    st.lock().doc.p.snapshots.get(k).cloned()
}

fn options_file(app: &AppHandle) -> Option<PathBuf> {
    dir(app, false).map(|d| d.join("options.json"))
}

#[tauri::command]
fn options(st: State<St>) -> Options {
    st.lock().opts.clone()
}

#[tauri::command]
fn set_options(app: AppHandle, st: State<St>, opts: Options) -> Out {
    if let Some(f) = options_file(&app) {
        let _ = doc::write_atomic(&f, serde_json::to_string(&opts).unwrap_or_default().as_bytes());
    }
    let mut a = st.lock();
    a.opts = opts;
    out(&a)
}

// ---------- recovery: a copy of unsaved work, kept until it is saved or discarded ----------
#[derive(Serialize, Deserialize)]
struct Recovery {
    name: String, path: Option<String>, saved: String,
    /// Every unsaved tab: its recovery file and the plan's own file (if it had one).
    #[serde(default)]
    items: Vec<(String, Option<String>)>,
}

fn recovery_files(app: &AppHandle) -> Option<(PathBuf, PathBuf)> {
    dir(app, true).map(|d| (d.join("recovery.dplan"), d.join("recovery.json")))
}

fn clear_recovery(app: &AppHandle) {
    if let Some((a, b)) = recovery_files(app) {
        let _ = std::fs::remove_file(a);
        let _ = std::fs::remove_file(b);
    }
    if let Some(d) = dir(app, true) {
        for k in 1..64 { let _ = std::fs::remove_file(d.join(format!("recovery-{k}.dplan"))); }
    }
}

#[tauri::command]
fn autosave(app: AppHandle, st: State<St>) {
    write_recovery(&app, &st.lock());
}

/// Keeps the recovery copies true to the open plans: one per tab with unsaved changes (so saving
/// one tab never drops the others' copies), none when everything is saved.
fn write_recovery(app: &AppHandle, a: &App) {
    // Every unsaved tab: recovery.dplan, recovery-1.dplan, … The new copies are written before the
    // old extras are removed, so a crash in between never leaves no recovery at all.
    let dirty: Vec<&Doc> = docs(a).into_iter().filter(|d| d.dirty).collect();
    if dirty.is_empty() { return clear_recovery(app) }
    if let Some(d) = dir(app, true) {
        for k in dirty.len().min(63)..64 { let _ = std::fs::remove_file(d.join(format!("recovery-{k}.dplan"))); }
    }
    if let Some((plan, meta)) = recovery_files(app) {
        let mut items = vec![];
        for (k, d) in dirty.iter().enumerate().take(63) {
            let f = if k == 0 { plan.clone() } else { plan.with_file_name(format!("recovery-{k}.dplan")) };
            let _ = doc::write_atomic(&f, d.json().as_bytes());
            items.push((f.display().to_string(), d.path.as_ref().map(|p| p.display().to_string())));
        }
        let info = Recovery { name: dirty.iter().map(|d| d.p.name.as_str()).collect::<Vec<_>>().join(", "), path: dirty[0].path.as_ref().map(|p| p.display().to_string()), saved: chrono::Local::now().format("%d-%b-%y %H:%M").to_string(), items };
        let _ = doc::write_atomic(&meta, serde_json::to_string(&info).unwrap_or_default().as_bytes());
    }
}

#[tauri::command]
fn recovery(app: AppHandle) -> Option<Recovery> {
    let (plan, meta) = recovery_files(&app)?;
    plan.exists().then_some(())?;
    serde_json::from_str(&std::fs::read_to_string(meta).ok()?).ok()
}

#[tauri::command]
fn recover(app: AppHandle, st: State<St>, keep: bool) -> R<Option<Out>> {
    let info = recovery(app.clone());
    let files = recovery_files(&app);
    if !keep {
        clear_recovery(&app);
        return Ok(None);
    }
    let (plan, _) = files.ok_or("No recovered plan was found.")?;
    let info = info.unwrap_or(Recovery { name: String::new(), path: None, saved: String::new(), items: vec![] });
    let items = if info.items.is_empty() { vec![(plan.display().to_string(), info.path.clone())] } else { info.items };
    let mut a = st.lock();
    for (f, path) in items {
        let Ok(mut d) = Doc::open(Path::new(&f)) else { continue };
        d.path = path.map(PathBuf::from);
        d.dirty = true;
        add_tab(&mut a, d);
    }
    Ok(Some(out(&a)))
}

#[derive(Serialize)]
struct Recent { path: String, name: String, modified: String }

fn recent_paths(app: &AppHandle) -> Vec<String> {
    options_file(app).map(|f| f.with_file_name("recent.json")).and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn remember(app: &AppHandle, path: &Path) {
    let p = path.display().to_string();
    let mut v = recent_paths(app);
    v.retain(|x| *x != p);
    v.insert(0, p);
    v.truncate(15);
    if let Some(f) = options_file(app).map(|f| f.with_file_name("recent.json")) {
        let _ = doc::write_atomic(&f, serde_json::to_string(&v).unwrap_or_default().as_bytes());
    }
}

#[tauri::command]
fn recent(app: AppHandle) -> Vec<Recent> {
    recent_paths(&app)
        .into_iter()
        .filter_map(|p| {
            let m = std::fs::metadata(&p).ok()?.modified().ok()?;
            let modified = chrono::DateTime::<chrono::Local>::from(m).format("%d-%b-%y %H:%M").to_string();
            let name = Path::new(&p).file_stem()?.to_string_lossy().into_owned();
            Some(Recent { path: p, name, modified })
        })
        .collect()
}

/// A .dplan path passed on the command line (double-clicked in Explorer).
#[tauri::command]
fn startup_file(app: AppHandle, st: State<St>) -> Option<String> {
    let last = || if st.lock().opts.open_last { recent_paths(&app).into_iter().find(|p| Path::new(p).exists()) } else { None };
    std::env::args().skip(1).find(|a| a.to_lowercase().ends_with(".dplan")).or_else(last)
}

/// Only the project's own GitHub pages, with plain URL characters.
fn allowed_url(url: &str) -> bool {
    const HOME: &str = "https://github.com/Edgar-Mendonca/Datum-Plan";
    let rest = url.strip_prefix(HOME).unwrap_or("x");
    (rest.is_empty() || rest.starts_with(['/', '?', '#'])) && rest.len() < 300 && rest.chars().all(|c| c.is_ascii_alphanumeric() || "-._~/?#&=%+".contains(c))
}

/// Chart › Open in Datum Plot: the chart's values saved as a Datum Plot project in
/// Documents\Datum Plan\To Datum Plot and opened there. A one-way copy: nothing comes back to the plan.
#[tauri::command]
fn open_in_plot(app: AppHandle, st: State<St>, chart: datum_core::io::dplot::ChartData) -> R<String> {
    let (plan, company) = { let a = st.lock(); (a.doc.p.name.clone(), a.doc.p.company.clone()) };
    let json = datum_core::io::dplot::project(&chart, &plan, &company, &today().format("%Y-%m-%d").to_string());
    let dir = app.path().document_dir().map_err(|e| e.to_string())?.join("Datum Plan").join("To Datum Plot");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not make the folder {}: {e}", dir.display()))?;
    let clean = |s: &str| s.chars().map(|c| if c.is_alphanumeric() || " -_().".contains(c) { c } else { ' ' }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
    let base = clean(&format!("{plan} - {}", chart.title)).chars().take(120).collect::<String>();
    let mut path = dir.join(format!("{base}.dplot"));
    for n in 2..1000 { if !path.exists() { break } path = dir.join(format!("{base} ({n}).dplot")) }
    doc::write_atomic(&path, json.to_string().as_bytes())?;
    if !plot_installed() {
        return Ok(format!("Datum Plot is not installed, so the chart's values were saved as {}. Install Datum Plot (Datum Home or the Datum website) and open that file.", path.display()));
    }
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(&path).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(&path).spawn();
    r.map_err(|e| e.to_string())?;
    Ok("Opening a copy in Datum Plot. Changes made there do not change this plan.".into())
}

/// Whether Datum Plot is installed: Windows knows which program opens .dplot files.
fn plot_installed() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        ["HKCU\\Software\\Classes\\.dplot", "HKLM\\Software\\Classes\\.dplot"].iter().any(|k| std::process::Command::new("reg").args(["query", k]).creation_flags(0x0800_0000).output().is_ok_and(|o| o.status.success()))
    }
    #[cfg(not(windows))]
    { true }
}

/// Opens one of the project's GitHub pages in the browser (only those, so the window cannot open anything else).
#[tauri::command]
fn open_url(url: String) -> R<()> {
    if !allowed_url(&url) {
        return Err("Only the Datum Plan GitHub pages can be opened.".into());
    }
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(&url).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(&url).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

/// Opens a link from a sheet cell in the browser, after the window has shown the address and asked:
/// only web and mail addresses (http, https, mailto), never files, scripts or other programs.
#[tauri::command]
fn open_link(url: String) -> R<()> {
    let Some(url) = datum_core::sheetx::link(&url) else { return Err("Only web (http, https) and mail (mailto) links can be opened.".into()) };
    #[cfg(windows)]
    let r = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", &url]).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(&url).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[tauri::command]
fn quit(app: AppHandle) {
    clear_recovery(&app);
    app.exit(0);
}

#[tauri::command]
async fn check_update(st: State<'_, St>) -> R<update::Info> {
    let beta = st.lock().opts.beta;
    tauri::async_runtime::spawn_blocking(move || update::check(beta)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn guide(app: AppHandle, lang: String) -> R<String> {
    tauri::async_runtime::spawn_blocking(move || update::guide(&app, &lang).map(|p| p.display().to_string())).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn install_update(app: AppHandle, url: String, sums: Option<String>) -> R<()> {
    tauri::async_runtime::spawn_blocking(move || update::install(&app, &url, sums.as_deref())).await.map_err(|e| e.to_string())?
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Anything that ever panics is written to crash.log (version, place, message) for an issue report.
            if let Some(log) = dir(app.handle(), true).map(|d| d.join("crash.log")) {
                let default = std::panic::take_hook();
                std::panic::set_hook(Box::new(move |info| {
                    use std::io::Write;
                    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&log) {
                        let at = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
                        let what = info.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_default();
                        let _ = writeln!(f, "{} Datum Plan {} panicked at {at}: {what}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), env!("CARGO_PKG_VERSION"));
                    }
                    default(info);
                }));
            }
            let opts = options_file(app.handle()).and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
            app.manage(St(Mutex::new(App { doc: Doc::new(Project::new("Project1", today())), others: vec![None], cur: 0, opts, clip: vec![], table: None, files: HashMap::new() })));
            Ok(())
        })
        .on_window_event(|w, e| {
            if let WindowEvent::CloseRequested { api, .. } = e {
                if docs(&w.state::<St>().lock()).iter().any(|d| d.dirty) {
                    api.prevent_close();
                    let _ = w.emit("close-requested", ());
                } else {
                    clear_recovery(w.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            state, apply, history, new_plan, templates, open, save, import, import_table, export, save_file, pick_logo, import_workbook, export_sheets, sheet_to_plan, copy, paste,
            query, resources, field_list, task_info, usage, curve, checks, risk_run, level_preview, manpower, earned_schedule, lookahead, credit_library, network, compare, workspace, snapshot_rows, sheet_view, sheet_check, sheet_catalog, formula_shift, dash_eval, switch_tab, close_tab, close_all, save_pdf, close_tabs, new_subplan, add_subplan, remove_subplan, open_subplans, subplan_rows, link_go, goto_link, link_target_file, save_all, link_targets, link_states, link_users, update_links, linked_example, tutorial_example, open_in_plot, scurve, periods, period_report,
            options, set_options, autosave, recovery, recover, recent, startup_file, version, open_url, open_link, quit, check_update, install_update, guide
        ])
        .run(tauri::generate_context!())
        .expect("Datum Plan failed to start");
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_the_projects_own_pages_open() {
        for ok in ["https://github.com/Edgar-Mendonca/Datum-Plan", "https://github.com/Edgar-Mendonca/Datum-Plan/issues/new", "https://github.com/Edgar-Mendonca/Datum-Plan/releases"] {
            assert!(super::allowed_url(ok), "{ok}");
        }
        for bad in ["https://github.com/Edgar-Mendonca/Datum-PlanX", "https://evil.com/Edgar-Mendonca/Datum-Plan", "https://github.com/Edgar-Mendonca/Datum-Plan/\"&calc.exe", "https://github.com/Edgar-Mendonca/Datum-Plan/ x", "file:///C:/Windows", "javascript:alert(1)"] {
            assert!(!super::allowed_url(bad), "{bad}");
        }
    }
}
