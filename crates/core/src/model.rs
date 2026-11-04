//! The plan model (file format 3). Instants are minutes since 1970-01-01 00:00 local time;
//! durations and lags are working minutes.
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkType { FS, SS, FF, SF }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Constraint { #[default] ASAP, ALAP, SNET, SNLT, FNET, FNLT, MSO, MFO }

pub const CONSTRAINTS: [(Constraint, &str); 8] = [
    (Constraint::ASAP, "As Soon As Possible"), (Constraint::ALAP, "As Late As Possible"),
    (Constraint::SNET, "Start No Earlier Than"), (Constraint::SNLT, "Start No Later Than"),
    (Constraint::FNET, "Finish No Earlier Than"), (Constraint::FNLT, "Finish No Later Than"),
    (Constraint::MSO, "Must Start On"), (Constraint::MFO, "Must Finish On"),
];

/// How a duration is shown: minutes, hours, days, weeks or months.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit { M, H, #[default] D, W, Mo }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResKind { #[default] Work, Material, Cost }

/// What stays fixed when work, units or duration change (task types).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType { #[default] FixedDuration, FixedUnits, FixedWork }

/// Activity types: task, resource dependent, level of effort, milestones, WBS summary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActType { #[default] Task, ResourceDep, Loe, StartMs, FinishMs, WbsSummary }

/// How % complete is measured (percent complete types).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PctType { #[default] Duration, Units, Physical }

/// When a fixed cost or expense is charged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Accrual { Start, #[default] Prorated, End }

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Step { pub name: String, pub weight: f64, pub done: bool }

/// A rule of credit: the steps of a kind of work and the share of progress each earns.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CreditRule { pub name: String, pub steps: Vec<(String, f64)> }

/// An entry in the constraint log: something a task needs before it can start.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LogItem { pub id: u32, pub uid: u32, pub kind: String, pub text: String, pub owner: String, pub need: Option<NaiveDate>, pub status: String }

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NoteTopic { pub topic: String, pub text: String }

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Expense { pub name: String, pub category: String, pub vendor: String, pub planned: f64, pub actual: f64, pub accrual: Accrual }

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DocRef { pub title: String, pub path: String, pub status: String, pub deliverable: bool }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExtLink {
    /// The other plan's file, as last saved (resolved next to this plan when the folder moves).
    pub file: String,
    /// The task in that plan (unique id).
    pub uid: u32,
    pub kind: LinkType,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub lag: i64,
    /// Last known: the other plan's name, the task's ID and name, and its start and finish (minutes).
    #[serde(default)]
    pub plan: String,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub start: i64,
    #[serde(default)]
    pub finish: i64,
}

/// A link from a task in this plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub pred: u32,
    pub kind: LinkType,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub lag: i64,
    /// The lag is a percentage of the predecessor's duration (`lag` holds the percent).
    #[serde(default, skip_serializing_if = "is_false")]
    pub pct: bool,
}

/// Work resources: units 1.0 = 100%. Material: quantity. Cost: amount.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assign {
    pub res: u32,
    pub units: f64,
    /// Cost rate table A–E (0–4).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub table: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Baseline { pub start: i64, pub finish: i64, pub dur: i64, #[serde(default)] pub cost: f64, /** Work minutes when saved (None in older files). */ #[serde(default, skip_serializing_if = "Option::is_none")] pub work: Option<f64> }

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Task {
    pub uid: u32,
    pub name: String,
    #[serde(skip_serializing_if = "is_zero_u8")]
    pub level: u8,
    pub dur: i64,
    #[serde(skip_serializing_if = "is_day")]
    pub unit: Unit,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub preds: Vec<Link>,
    /// Predecessors in other plans (links between plans), with their last known dates.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ext: Vec<ExtLink>,
    #[serde(skip_serializing_if = "is_zero_u8")]
    pub pct: u8,
    #[serde(skip_serializing_if = "is_asap")]
    pub constraint: Constraint,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cdate: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<NaiveDate>,
    #[serde(skip_serializing_if = "is_false")]
    pub manual: bool,
    /// Start of a manually scheduled task.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mstart: Option<NaiveDate>,
    #[serde(skip_serializing_if = "is_false")]
    pub inactive: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub act_start: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub act_finish: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub res: Vec<Assign>,
    /// Baselines 0–10.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub bls: BTreeMap<u8, Baseline>,
    #[serde(skip_serializing_if = "is_fixed_dur")]
    pub task_type: TaskType,
    #[serde(skip_serializing_if = "is_false")]
    pub effort: bool,
    #[serde(skip_serializing_if = "is_task_act")]
    pub act_type: ActType,
    #[serde(skip_serializing_if = "is_dur_pct")]
    pub pct_type: PctType,
    /// Physical % complete, used for earned value when the type is Physical.
    #[serde(skip_serializing_if = "is_zero_u8")]
    pub phys: u8,
    /// Elapsed duration: runs around the clock, ignoring calendars.
    #[serde(skip_serializing_if = "is_false")]
    pub elapsed: bool,
    /// With resource calendars on: this task follows its own (or the project) calendar only.
    #[serde(skip_serializing_if = "is_false")]
    pub ignore_rc: bool,
    /// Pauses in the work: (after working minutes, gap working minutes).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub splits: Vec<(i64, i64)>,
    /// Working minutes added by resource leveling.
    #[serde(skip_serializing_if = "is_zero")]
    pub level_delay: i64,
    /// Schedule risk: optimistic and pessimistic durations in working minutes (0 = use the plan's default range).
    #[serde(skip_serializing_if = "is_zero")]
    pub dmin: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub dmax: i64,
    /// Installed quantities: planned and installed so far, in `qty_unit` (m, t, m³, each…).
    /// When `qty` is set, the installed share is the task's physical % complete.
    #[serde(skip_serializing_if = "is_zero_f")]
    pub qty: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub qty_done: f64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub qty_unit: String,
    /// Leveling priority 0–1000 (500 when not set).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u16>,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub fixed_cost: f64,
    #[serde(skip_serializing_if = "is_prorated")]
    pub accrual: Accrual,
    /// Custom field values by key (text1, number3, flag2…).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub custom: BTreeMap<String, Value>,
    /// Activity code values by code name.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub codes: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notebook: Vec<NoteTopic>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub expenses: Vec<Expense>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<DocRef>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub hyperlink: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub cost_account: String,
    /// Responsible person or team (OBS).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub responsible: String,
    /// Text formatting: an object of column key ("*" = the whole row) → `b;i;u;s;c:#hex;f:#hex;z:12;a:c`.
    #[serde(skip_serializing_if = "Value::is_null")]
    pub style: Value,
    #[serde(skip_serializing_if = "is_false")]
    pub timeline: bool,
    /// Activity ID (as in XER files) or any short code.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub code: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub notes: String,
}

/// Working intervals of one day, in minutes from midnight.
pub type Hours = Vec<(u16, u16)>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Exception {
    pub name: String,
    pub from: NaiveDate,
    pub to: NaiveDate,
    /// Empty means not working.
    #[serde(default)]
    pub hours: Hours,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Calendar {
    pub name: String,
    /// Monday first.
    pub week: [Hours; 7],
    #[serde(default)]
    pub exceptions: Vec<Exception>,
}

/// The longest span (duration, lag, delay) a plan may hold: about 80 years of 8-hour days.
pub const MAX_SPAN: i64 = 10_000_000;
/// Minutes in a day.
const DAY_MAX: i64 = 1440;

pub const STD_HOURS: [(u16, u16); 2] = [(480, 720), (780, 1020)];

impl Calendar {
    pub fn standard(name: &str) -> Self {
        let d = STD_HOURS.to_vec();
        Self { name: name.into(), week: [d.clone(), d.clone(), d.clone(), d.clone(), d, vec![], vec![]], exceptions: vec![] }
    }
    pub fn full(name: &str) -> Self {
        let d = vec![(0, 1440)];
        Self { name: name.into(), week: std::array::from_fn(|_| d.clone()), exceptions: vec![] }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Resource {
    pub uid: u32,
    pub name: String,
    pub kind: ResKind,
    pub group: String,
    /// 1.0 = one full-time person.
    pub max: f64,
    /// Per hour for work, per unit for material.
    pub rate: f64,
    pub per_use: f64,
    /// Material unit, such as m3 or t.
    pub label: String,
    pub initials: String,
    pub email: String,
    pub code: String,
    pub notes: String,
    /// Resource calendar; the project calendar when empty.
    pub cal: Option<String>,
    /// Overtime rate per hour.
    pub ovt: f64,
    /// Cost rate tables A–E; table A falls back to `rate`/`per_use` when empty.
    pub tables: Vec<Vec<Rate>>,
    /// Availability over time (max units between dates).
    pub avail: Vec<Avail>,
    pub custom: BTreeMap<String, Value>,
    /// The resource's own working time on top of its base calendar: leave, holidays, part days.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exceptions: Vec<Exception>,
    /// Where the resource (a person, crew or vendor) comes from: an ISO code such as DE, or a name.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub country: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rate { pub from: Option<NaiveDate>, pub std: f64, pub ovt: f64, pub per_use: f64 }

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Avail { pub from: Option<NaiveDate>, pub to: Option<NaiveDate>, pub max: f64 }

impl Default for Resource {
    fn default() -> Self {
        Self {
            uid: 0, name: String::new(), kind: ResKind::Work, group: String::new(), max: 1.0, rate: 0.0, per_use: 0.0, label: String::new(),
            initials: String::new(), email: String::new(), code: String::new(), notes: String::new(), cal: None, ovt: 0.0, tables: vec![], avail: vec![], custom: BTreeMap::new(), country: String::new(), exceptions: vec![],
        }
    }
}

impl Resource {
    /// (standard rate per hour, cost per use) of table `t` on day `d`.
    pub fn rate_on(&self, t: u8, d: NaiveDate) -> (f64, f64) {
        match self.tables.get(t as usize).filter(|v| !v.is_empty()) {
            Some(v) => v.iter().rfind(|r| r.from.is_none_or(|f| f <= d)).or(v.first()).map_or((self.rate, self.per_use), |r| (r.std, r.per_use)),
            None => (self.rate, self.per_use),
        }
    }
    /// Max units on day `d`.
    pub fn max_on(&self, d: NaiveDate) -> f64 {
        self.avail.iter().find(|a| a.from.is_none_or(|f| f <= d) && a.to.is_none_or(|t| d <= t)).map_or(self.max, |a| a.max)
    }
}

/// Currency of costs: ISO code, symbol, decimals and whether the symbol follows the number.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Currency { pub code: String, pub symbol: String, pub decimals: u8, pub after: bool }

impl Default for Currency {
    fn default() -> Self { Self { code: "USD".into(), symbol: "$".into(), decimals: 2, after: false } }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LagCal { #[default] Successor, Predecessor, H24, Project }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LevelOrder { IdOnly, #[default] Standard, Priority }

/// Scheduling policy (Schedule Options). Defaults are the Datum style.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchedOpts {
    /// Retained logic (true) or progress override (false) for out-of-sequence progress.
    pub retained: bool,
    /// Critical when total float ≤ this many working minutes…
    pub critical_float: i64,
    /// …or when on the longest path.
    pub longest_path: bool,
    pub lag_cal: LagCal,
    /// Number of float paths to calculate (0 = off).
    pub float_paths: u8,
    /// Open-ended tasks are shown as critical (diagnostic).
    pub open_ends_critical: bool,
    pub level_within_slack: bool,
    pub level_split: bool,
    pub level_order: LevelOrder,
    /// New tasks are effort-driven.
    pub effort_default: bool,
    pub task_type_default: TaskType,
    /// Schedule backward from `finish_date`.
    pub from_finish: bool,
    pub finish_date: Option<NaiveDate>,
    /// Resource calendars drive task dates: each assigned person works on their own calendar
    /// (base calendar plus their leave) and the task finishes when the last of them does.
    pub res_cals: bool,
}

impl Default for SchedOpts {
    fn default() -> Self {
        Self {
            retained: true, critical_float: 0, longest_path: false, lag_cal: LagCal::Successor, float_paths: 0, open_ends_critical: false,
            level_within_slack: false, level_split: false, level_order: LevelOrder::Standard, effort_default: false,
            task_type_default: TaskType::FixedDuration, from_finish: false, finish_date: None, res_cals: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LookupItem { pub value: String, pub desc: String }

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IndicatorRule {
    /// 0 = tasks, 1 = summary rows, 2 = project summary.
    pub scope: u8,
    pub test: String,
    pub value: String,
    pub icon: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rollup { #[default] None, Max, Min, Sum, Average, Count, Formula }

/// A custom field definition (Custom Fields dialog).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomDef {
    /// Field key, e.g. text1, number3, flag2, rtext1 for resources.
    pub key: String,
    pub alias: String,
    pub lookup: Vec<LookupItem>,
    pub restrict: bool,
    pub default: String,
    pub formula: String,
    pub rollup: Rollup,
    pub indicators: Vec<IndicatorRule>,
    /// Show indicators instead of the value.
    pub show_icons: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CodeValue { pub value: String, pub desc: String, pub color: String }

/// An activity code: a classification with a list of values.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CodeDef { pub name: String, pub scope: String, pub values: Vec<CodeValue> }

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Risk {
    pub id: u32,
    pub name: String,
    pub cause: String,
    /// 1–5.
    pub prob: u8,
    pub impact: u8,
    pub owner: String,
    pub response: String,
    pub status: String,
    pub tasks: Vec<u32>,
    pub kind: String,
    /// Schedule impact in working days on each linked task when the risk occurs (schedule risk analysis).
    pub days: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SnapRow { pub uid: u32, pub start: i64, pub finish: i64, pub pct: u8, pub cost: f64 }

/// A spreadsheet tab in the plan: typed cell inputs by A1 address, number formats, column widths.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SheetData { pub name: String, pub cells: BTreeMap<String, String>, pub fmt: BTreeMap<String, String>, pub widths: BTreeMap<u32, u16>, #[serde(default, skip_serializing_if = "BTreeMap::is_empty")] pub style: BTreeMap<String, String>,
    /// Pictures placed on the sheet: a library picture, its top-left cell and its size in px.
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub images: Vec<SheetImage>,
    /// Merged ranges ("A1:C2"); the top-left cell's value and style show across the range.
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub merges: Vec<String>,
    /// Row heights in px (0-based row), where not the default.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")] pub heights: BTreeMap<u32, u16>,
    /// The tab's colour ("#RRGGBB", empty for none) and whether the tab is hidden.
    #[serde(default, skip_serializing_if = "String::is_empty")] pub color: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")] pub hidden: bool,
    /// Hidden rows and columns (0-based).
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub hide_rows: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub hide_cols: Vec<u32>,
    /// Conditional formatting: rules over ranges, the first that applies to a cell wins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub cond: Vec<CondRule>,
    /// The sheet's own page setup for printing.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub page: Option<Box<SheetPage>> }

/// A range "A1:C2" as (row0, col0, row1, col1), 0-based and ordered.
pub fn sheet_range(r: &str) -> Option<(u32, u32, u32, u32)> {
    let (a, b) = r.split_once(':').unwrap_or((r, r));
    let (a, b) = (gridcraft_core::CellRef::parse(a.trim())?, gridcraft_core::CellRef::parse(b.trim())?);
    Some((a.row.min(b.row), a.col.min(b.col), a.row.max(b.row), a.col.max(b.col)))
}

/// A picture on a sheet, anchored at a cell ("B2") like a picture over a spreadsheet's cells.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SheetImage { pub pic: u32, pub cell: String, pub w: u16, pub h: u16,
    /// Placed in its cell (or the merged range it starts): drawn to fit inside, centred, keeping its shape.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")] pub fit: bool }

/// A conditional formatting rule. `op`: gt, ge, lt, le, eq, ne, between, notbetween (values `a`, `b`);
/// contains, notcontains, begins, ends (text `a`); blank, notblank, dup, unique; top, bottom (`a` items);
/// above, below (the average); date (`a`: yesterday, today, tomorrow, last7, next7, lastweek, thisweek,
/// nextweek, lastmonth, thismonth, nextmonth, past, future); bar (data bars, colour `style`); scale (colour scale from `a` to `b`,
/// through `style` when set). `style` is the look a matching cell takes (the window's style string).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CondRule { pub range: String, pub op: String, pub a: String, pub b: String, pub style: String }
pub const COND_OPS: [&str; 23] = ["date", "gt", "ge", "lt", "le", "eq", "ne", "between", "notbetween", "contains", "notcontains", "begins", "ends", "blank", "notblank", "dup", "unique", "top", "bottom", "above", "below", "bar", "scale"];

/// How a sheet prints, where it differs from the plan's page setup. Empty text and None follow the plan.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SheetPage {
    /// "A4", "A3", "A5", "Letter", "Legal", "Tabloid"; empty: the plan's paper.
    pub paper: String,
    /// "portrait" or "landscape"; empty: the plan's.
    pub orient: String,
    /// Margins in mm: top, right, bottom, left.
    pub margins: Option<[f32; 4]>,
    /// Scale in % (0: 100 %), or fit to so many pages wide / tall (0: no limit).
    pub scale: u16, pub fit_w: u8, pub fit_h: u8,
    /// The print area ("A1:H40", empty: everything used) and the rows printed at the top of every page ("1:2").
    pub area: String, pub titles: String,
    /// Leave out the plan's header and footer; leave out cell gridlines; print row numbers and column letters; centre on the paper.
    pub no_hf: bool, pub no_grid: bool, pub headings: bool, pub center: bool,
    /// Rows that start a new page (0-based).
    pub breaks: Vec<u32>,
    /// Columns that start a new page (0-based).
    pub col_breaks: Vec<u32>,
}

/// A picture kept in the plan file: a logo of a party to the project (the client, our company, a
/// vendor or sub-vendor) or any image used on dashboards, sheets and printouts. `data` is a data URL.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Picture { pub id: u32, pub name: String, pub role: String, pub data: String }

/// Pictures a plan may keep, and the size of each (data URL characters, about 4 MB of image).
pub const MAX_PICTURES: usize = 200;
pub const MAX_PICTURE_LEN: usize = 6_000_000;

/// Reporting periods: the weekday a week ends on (0 = Monday … 6 = Sunday), the day of the month
/// the month closes (0 = last day), and how progress is weighted on S-curves.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PeriodOpts { pub week_end: u8, pub month_end: u8, pub weight: String }

impl Default for PeriodOpts {
    fn default() -> Self { Self { week_end: 4, month_end: 0, weight: "duration".into() } }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Photo { pub data: String, pub caption: String }

/// The narrative of a progress report for one period.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PeriodNote { pub kind: String, pub end: Option<NaiveDate>, pub summary: String, pub progress: String, pub issues: String, pub actions: String, pub look_ahead: String, pub photos: Vec<Photo> }

/// A frozen copy of the plan at a reporting date (stored period performance).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Snapshot { pub date: Option<NaiveDate>, pub label: String, pub comment: String, pub finish: i64, pub pct: f64, pub cost: f64, pub rows: Vec<SnapRow> }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub format: u32,
    pub name: String,
    pub start: NaiveDate,
    /// Status (data) date: progress is reported up to the end of this day.
    pub status: Option<NaiveDate>,
    /// Working minutes in a "day" when durations are typed or shown in days.
    pub day_min: i64,
    /// New tasks are manually scheduled.
    pub manual_default: bool,
    /// Name of the project calendar.
    pub calendar: String,
    pub calendars: Vec<Calendar>,
    pub tasks: Vec<Task>,
    pub resources: Vec<Resource>,
    pub next_uid: u32,
    /// Grid columns: field and width (a negative width is a hidden column).
    pub columns: Vec<(String, i32)>,
    // Project Setup.
    pub code: String,
    pub description: String,
    pub client: String,
    pub manager: String,
    pub location: String,
    pub company: String,
    /// Logo as a data URL, used on reports and printouts.
    pub logo: String,
    pub currency: Currency,
    pub setup_done: bool,
    /// Activity ID numbering: prefix, first number, step.
    pub id_prefix: String,
    pub id_start: u32,
    pub id_step: u32,
    pub opts: SchedOpts,
    pub fields: Vec<CustomDef>,
    pub codes: Vec<CodeDef>,
    pub risks: Vec<Risk>,
    pub snapshots: Vec<Snapshot>,
    /// Rules of credit: named sets of weighted steps that measure physical progress.
    pub credit: Vec<CreditRule>,
    /// Constraint log for the look-ahead: what must be in place before a task can start.
    pub log: Vec<LogItem>,
    /// Column titles chosen by the user (field key → title).
    pub titles: BTreeMap<String, String>,
    pub sheets: Vec<SheetData>,
    /// The plan's picture library (logos of the parties, images for dashboards and sheets).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pictures: Vec<Picture>,
    pub periods: PeriodOpts,
    pub notes: Vec<PeriodNote>,
    /// Settings owned by the window: saved views, tables, filters, groups, bar and text styles,
    /// page setup, reports, timeline. The engine stores them but does not read them.
    pub ui: Value,
    /// A master plan: its subplans, in order, with their place in the hierarchy. Dates still come
    /// from links between the plans; the list says which plans make up the project.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subplans: Vec<SubPlan>,
    /// A subplan: its master's file (empty for a plan on its own or a master).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub master: String,
}

/// One subplan of a master plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SubPlan {
    /// The subplan's file, as last saved (found next to the master when the folder moves).
    pub file: String,
    /// Its name when last seen.
    pub name: String,
    /// Depth under the master: 0 = directly under it, 1 = under the subplan above, …
    #[serde(skip_serializing_if = "is_zero_u8")]
    pub level: u8,
}

impl Default for Project {
    fn default() -> Self {
        Self::new("Project1", NaiveDate::from_ymd_opt(2026, 1, 5).unwrap())
    }
}

pub const DEFAULT_COLUMNS: [(&str, i32); 8] =
    [("mode", 34), ("name", 260), ("dur", 74), ("start", 92), ("finish", 92), ("preds", 90), ("res", 140), ("pct", 64)];

fn is_zero(v: &i64) -> bool { *v == 0 }
fn is_zero_u8(v: &u8) -> bool { *v == 0 }
fn is_false(v: &bool) -> bool { !*v }
fn is_day(u: &Unit) -> bool { *u == Unit::D }
fn is_asap(c: &Constraint) -> bool { *c == Constraint::ASAP }
fn is_zero_f(v: &f64) -> bool { *v == 0.0 }
fn is_fixed_dur(v: &TaskType) -> bool { *v == TaskType::FixedDuration }
fn is_task_act(v: &ActType) -> bool { *v == ActType::Task }
fn is_dur_pct(v: &PctType) -> bool { *v == PctType::Duration }
fn is_prorated(v: &Accrual) -> bool { *v == Accrual::Prorated }

impl Project {
    pub fn new(name: &str, start: NaiveDate) -> Self {
        Self {
            format: 3, name: name.into(), start, status: None, day_min: 480, manual_default: false,
            calendar: "Standard".into(), calendars: vec![Calendar::standard("Standard")], tasks: vec![], resources: vec![],
            next_uid: 1, columns: DEFAULT_COLUMNS.iter().map(|(f, w)| (f.to_string(), *w)).collect(),
            code: String::new(), description: String::new(), client: String::new(), manager: String::new(), location: String::new(),
            company: String::new(), logo: String::new(), currency: Currency::default(), setup_done: false,
            id_prefix: "A".into(), id_start: 1000, id_step: 10, opts: SchedOpts::default(),
            fields: vec![], codes: vec![], risks: vec![], snapshots: vec![], credit: vec![], log: vec![], titles: BTreeMap::new(), sheets: vec![], pictures: vec![], periods: PeriodOpts::default(), notes: vec![], ui: Value::Null, subplans: vec![], master: String::new(),
        }
    }

    /// Reads a .dplan file, upgrading 0.1 files (whole working days, one Mon–Fri calendar).
    pub fn from_json(text: &str) -> Result<Self, String> {
        let mut v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if v["format"].as_u64().unwrap_or(1) < 2 {
            let c = v["calendar"].take();
            let day = |on: bool| if on { json!(STD_HOURS) } else { json!([]) };
            let week: Vec<Value> = (0..7).map(|i| day(c["week"][i].as_bool().unwrap_or(i < 5))).collect();
            let exc: Vec<Value> = c["holidays"].as_array().into_iter().flatten().map(|d| json!({"name": "Holiday", "from": d, "to": d})).collect();
            v["calendars"] = json!([{ "name": "Standard", "week": week, "exceptions": exc }]);
            v["calendar"] = json!("Standard");
            for t in v.get_mut("tasks").and_then(Value::as_array_mut).into_iter().flatten() {
                t["dur"] = json!(t["dur"].as_i64().unwrap_or(0) * 480);
                for l in t.get_mut("preds").and_then(Value::as_array_mut).into_iter().flatten() {
                    l["lag"] = json!(l["lag"].as_i64().unwrap_or(0) * 480);
                }
                if let Some(d) = t["snet"].as_str().map(String::from) {
                    t["constraint"] = json!("SNET");
                    t["cdate"] = json!(d);
                }
            }
            v["format"] = json!(2);
        }
        if v["format"].as_u64().unwrap_or(2) < 3 {
            // 0.2 files kept one baseline per task in `bl`.
            for t in v.get_mut("tasks").and_then(Value::as_array_mut).into_iter().flatten() {
                if let Some(b) = t.as_object_mut().and_then(|o| o.remove("bl")) {
                    t["bls"] = json!({ "0": b });
                }
            }
            v["format"] = json!(3);
        }
        let mut p: Project = serde_json::from_value(v).map_err(|e| e.to_string())?;
        p.normalize();
        Ok(p)
    }

    pub fn is_summary(&self, i: usize) -> bool {
        self.tasks.get(i + 1).is_some_and(|n| n.level > self.tasks[i].level)
    }

    /// Task `i` and all its descendants.
    pub fn subtree(&self, i: usize) -> Range<usize> {
        let l = self.tasks[i].level;
        let mut j = i + 1;
        while j < self.tasks.len() && self.tasks[j].level > l {
            j += 1;
        }
        i..j
    }

    pub fn idx(&self, uid: u32) -> Option<usize> {
        self.tasks.iter().position(|t| t.uid == uid)
    }

    pub fn res_idx(&self, uid: u32) -> Option<usize> {
        self.resources.iter().position(|r| r.uid == uid)
    }

    pub fn new_task(&mut self, name: &str, level: u8, dur: i64) -> Task {
        let uid = self.next_uid;
        self.next_uid += 1;
        let code = self.next_code();
        Task { uid, name: name.into(), level, dur, manual: self.manual_default, code, task_type: self.opts.task_type_default, effort: self.opts.effort_default, ..Default::default() }
    }

    /// The next Activity ID (A1000, A1010…), or empty when numbering is off.
    pub fn next_code(&self) -> String {
        if self.id_prefix.is_empty() && self.id_step == 0 {
            return String::new();
        }
        let n = self.tasks.iter().filter_map(|t| t.code.strip_prefix(self.id_prefix.as_str())?.parse::<u32>().ok()).max();
        // Past the last number there is no ID to give.
        n.map_or(Some(self.id_start), |n| n.checked_add(self.id_step.max(1))).map_or(String::new(), |n| format!("{}{n}", self.id_prefix))
    }

    /// Effective leveling priority.
    pub fn priority(&self, i: usize) -> u16 {
        self.tasks[i].priority.unwrap_or(500)
    }

    /// Finds a resource by name, adding a work resource if there is none.
    pub fn resource(&mut self, name: &str) -> u32 {
        if let Some(r) = self.resources.iter().find(|r| r.name.eq_ignore_ascii_case(name)) {
            return r.uid;
        }
        let uid = self.resources.iter().map(|r| r.uid).max().unwrap_or(0) + 1;
        self.resources.push(Resource { uid, name: name.into(), ..Default::default() });
        uid
    }

    pub fn cal(&self, name: Option<&str>) -> &Calendar {
        let n = name.unwrap_or(&self.calendar);
        self.calendars.iter().find(|c| c.name == n).or_else(|| self.calendars.iter().find(|c| c.name == self.calendar)).unwrap_or(&self.calendars[0])
    }

    /// Repairs anything an edit or a hand-made file can leave inconsistent.
    pub fn normalize(&mut self) {
        // Pictures: images only, within the size limits, unique ids; sheet pictures point at one.
        self.pictures.retain(|x| x.data.starts_with("data:image/") && x.data.len() <= MAX_PICTURE_LEN);
        self.pictures.truncate(MAX_PICTURES);
        let mut seen = std::collections::HashSet::new();
        self.pictures.retain(|x| x.id > 0 && seen.insert(x.id));
        for s in &mut self.sheets {
            s.images.retain(|i| seen.contains(&i.pic));
            s.images.truncate(500);
            s.images.iter_mut().for_each(|i| (i.w, i.h) = (i.w.clamp(8, 4000), i.h.clamp(8, 4000)));
            // Merges: real ranges of more than one cell that do not overlap one another.
            let mut kept: Vec<(u32, u32, u32, u32)> = vec![];
            s.merges = std::mem::take(&mut s.merges).into_iter().filter_map(|m| {
                let g = sheet_range(&m).filter(|g| (g.0, g.1) != (g.2, g.3) && g.2 - g.0 < 10_000 && g.3 - g.1 < 1_000)?;
                if kept.len() >= 5_000 || kept.iter().any(|k| g.0 <= k.2 && k.0 <= g.2 && g.1 <= k.3 && k.1 <= g.3) { return None }
                kept.push(g);
                Some(format!("{}{}:{}{}", crate::sheet::col_name(g.1), g.0 + 1, crate::sheet::col_name(g.3), g.2 + 1))
            }).collect();
            s.heights.retain(|_, h| *h != 0);
            s.heights.values_mut().for_each(|h| *h = (*h).clamp(6, 600));
            for v in [&mut s.hide_rows, &mut s.hide_cols] { v.sort_unstable(); v.dedup(); v.retain(|x| *x < 1_048_576); v.truncate(20_000); }
            s.cond.retain(|c| COND_OPS.contains(&c.op.as_str()) && sheet_range(&c.range).is_some());
            s.cond.truncate(500);
            for c in &mut s.cond { c.a.truncate(500); c.b.truncate(500); c.style.truncate(500); }
            if let Some(pg) = &mut s.page {
                pg.scale = if pg.scale == 0 { 0 } else { pg.scale.clamp(10, 400) };
                pg.fit_w = pg.fit_w.min(50); pg.fit_h = pg.fit_h.min(50);
                if let Some(m) = &mut pg.margins { m.iter_mut().for_each(|x| *x = if x.is_finite() { x.clamp(0.0, 100.0) } else { 10.0 }); }
                if sheet_range(&pg.area).is_none() { pg.area.clear() }
                if !pg.titles.is_empty() && !pg.titles.split(':').all(|x| x.trim().parse::<u32>().is_ok_and(|n| n > 0)) { pg.titles.clear() }
                for b in [&mut pg.breaks, &mut pg.col_breaks] { b.sort_unstable(); b.dedup(); b.retain(|r| *r > 0); b.truncate(1000); }
                pg.paper.truncate(20); pg.orient.truncate(20);
            }
            if !(s.color.len() == 7 && s.color.starts_with('#') && s.color[1..].chars().all(|c| c.is_ascii_hexdigit())) { s.color.clear() }
        }
        // At least one tab stays visible.
        if !self.sheets.is_empty() && self.sheets.iter().all(|s| s.hidden) { self.sheets[0].hidden = false }
        if self.calendars.is_empty() {
            self.calendars.push(Calendar::standard("Standard"));
        }
        if !self.calendars.iter().any(|c| c.name == self.calendar) {
            self.calendar = self.calendars[0].name.clone();
        }
        if !(1..=DAY_MAX).contains(&self.day_min) {
            self.day_min = 480;
        }
        // Damaged or hand-made files: keep every span within MAX_SPAN working minutes and every
        // number finite, so the scheduler never meets values it cannot represent.
        let span = |v: i64| v.clamp(-MAX_SPAN, MAX_SPAN);
        let fin = |v: f64| if v.is_finite() { v } else { 0.0 };
        let mut prev: i32 = -1;
        // People work at most 1000 %; a material quantity or a cost amount can be far larger.
        let work: std::collections::HashSet<u32> = self.resources.iter().filter(|r| r.kind == ResKind::Work).map(|r| r.uid).collect();
        for t in &mut self.tasks {
            t.level = (t.level as i32).min(prev + 1).max(0) as u8;
            prev = t.level as i32;
            t.dur = span(t.dur).max(0);
            t.pct = t.pct.min(100);
            t.phys = t.phys.min(100);
            t.level_delay = span(t.level_delay).max(0);
            (t.dmin, t.dmax) = (span(t.dmin).max(0), span(t.dmax).max(0));
            t.splits.retain(|&(a, g)| a > 0 && g > 0);
            t.splits.sort_unstable();
            t.splits.dedup_by_key(|x| x.0);
            t.splits.iter_mut().for_each(|x| *x = (span(x.0), span(x.1)));
            t.splits.truncate(100);
            t.preds.iter_mut().for_each(|l| l.lag = if l.pct { l.lag.clamp(-10_000, 10_000) } else { span(l.lag) });
            (t.qty, t.qty_done, t.fixed_cost) = (fin(t.qty).max(0.0), fin(t.qty_done).max(0.0), fin(t.fixed_cost));
            t.res.iter_mut().for_each(|a| a.units = fin(a.units).clamp(0.0, if work.contains(&a.res) { 1000.0 } else { 1e12 }));
            // Sums of huge amounts overflow; a NaN or infinity would be saved as null and the file could not be opened.
            t.bls.values_mut().for_each(|b| (b.cost, b.work) = (fin(b.cost), b.work.map(fin)));
            t.expenses.iter_mut().for_each(|e| (e.planned, e.actual) = (fin(e.planned), fin(e.actual)));
            t.steps.iter_mut().for_each(|s| s.weight = fin(s.weight));
        }
        for s in &mut self.snapshots {
            (s.pct, s.cost) = (fin(s.pct), fin(s.cost));
            s.rows.iter_mut().for_each(|r| r.cost = fin(r.cost));
        }
        self.risks.iter_mut().for_each(|r| r.days = fin(r.days));
        self.credit.iter_mut().flat_map(|r| &mut r.steps).for_each(|s| s.1 = fin(s.1));
        self.next_uid = self.next_uid.max(self.tasks.iter().map(|t| t.uid.saturating_add(1)).max().unwrap_or(1));
        let uids: std::collections::HashSet<u32> = self.tasks.iter().map(|t| t.uid).collect();
        let res: std::collections::HashSet<u32> = self.resources.iter().map(|r| r.uid).collect();
        let cals: Vec<String> = self.calendars.iter().map(|c| c.name.clone()).collect();
        // A task cannot be linked to its own summary tasks.
        let mut stack: Vec<(u8, u32)> = vec![];
        let anc: std::collections::HashMap<u32, Vec<u32>> = self.tasks.iter().map(|t| {
            stack.retain(|&(l, _)| l < t.level);
            let a = stack.iter().map(|&(_, u)| u).collect();
            stack.push((t.level, t.uid));
            (t.uid, a)
        }).collect();
        for t in &mut self.tasks {
            let me = t.uid;
            let mut seen = std::collections::HashSet::new();
            t.preds.retain(|l| l.pred != me && uids.contains(&l.pred) && !anc[&me].contains(&l.pred) && !anc[&l.pred].contains(&me) && seen.insert(l.pred));
            t.res.retain(|a| res.contains(&a.res));
            if t.cal.as_ref().is_some_and(|c| !cals.contains(c)) {
                t.cal = None;
            }
        }
        if self.columns.is_empty() {
            self.columns = Project::new("", self.start).columns;
        }
        // Links to other plans: a sensible number, lags and dates inside the calendar's range.
        let (lo, hi) = (crate::cal::at(crate::cal::lo()), crate::cal::at(crate::cal::hi()));
        for t in &mut self.tasks {
            t.ext.truncate(200);
            for x in &mut t.ext {
                x.lag = x.lag.clamp(-MAX_SPAN, MAX_SPAN);
                (x.start, x.finish) = (x.start.clamp(0, hi), x.finish.clamp(0, hi));
                if x.start < lo { x.start = 0 }
                if x.finish < lo { x.finish = 0 }
            }
        }
        // Subplans: a sensible number, each file once, a hierarchy without jumps.
        self.subplans.truncate(500);
        let mut seen = std::collections::HashSet::new();
        self.subplans.retain(|s| !s.file.is_empty() && seen.insert(crate::links::file_name(&s.file).to_lowercase()));
        let mut prev = None::<u8>;
        for s in &mut self.subplans {
            s.level = s.level.min(prev.map_or(0, |p| p + 1)).min(8);
            prev = Some(s.level);
        }
        for r in &mut self.resources {
            (r.max, r.rate, r.per_use, r.ovt) = (fin(r.max).clamp(0.0, 1e6), fin(r.rate), fin(r.per_use), fin(r.ovt));
            r.tables.iter_mut().flatten().for_each(|x| (x.std, x.ovt, x.per_use) = (fin(x.std), fin(x.ovt), fin(x.per_use)));
            r.avail.iter_mut().for_each(|a| a.max = fin(a.max).clamp(0.0, 1e6));
            if r.cal.as_ref().is_some_and(|c| !cals.contains(c)) {
                r.cal = None;
            }
            // A person's leave: whole ranges the right way round, no more than a few hundred of them.
            r.exceptions.retain(|e| e.to >= e.from);
            r.exceptions.truncate(500);
        }
        if self.currency.code.is_empty() {
            self.currency = Currency::default();
        }
    }
}
