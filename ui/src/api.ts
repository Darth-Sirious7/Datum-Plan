// Calls into the Rust shell and the shapes it returns.
declare global {
  interface Window { __TAURI__?: { core: { invoke: (cmd: string, args?: object) => Promise<any> }; event: { listen: (e: string, f: (x: { payload: any }) => void) => Promise<() => void> } } }
}

export const invoke = <T = any>(cmd: string, args?: object): Promise<T> =>
  window.__TAURI__ ? window.__TAURI__.core.invoke(cmd, args) : Promise.reject("Datum Plan must run inside its desktop window.");
export const listen = (e: string, f: (p: any) => void) => window.__TAURI__?.event.listen(e, x => f(x.payload));

export type Kind = "Text" | "Num" | "Int" | "Dur" | "Date" | "Fin" | "Cost" | "Flag" | "Pct" | "Work";
export type LinkType = "FS" | "SS" | "FF" | "SF";

export interface Currency { code: string; symbol: string; decimals: number; after: boolean }
export interface Calendar { name: string; week: [number, number][][]; exceptions: { name: string; from: string; to: string; hours: [number, number][] }[] }
export interface Rate { from: string | null; std: number; ovt: number; per_use: number }
export interface Resource {
  uid: number; name: string; kind: "Work" | "Material" | "Cost"; group: string; max: number; rate: number; per_use: number; label: string;
  initials: string; email: string; code: string; notes: string; cal: string | null; ovt: number; tables: Rate[][];
  avail: { from: string | null; to: string | null; max: number }[]; custom: Record<string, any>; country?: string; exceptions?: Calendar["exceptions"];
}
export interface Indicator { scope: number; test: string; value: string; icon: string }
export interface CustomDef { key: string; alias: string; lookup: { value: string; desc: string }[]; restrict: boolean; default: string; formula: string; rollup: string; indicators: Indicator[]; show_icons: boolean }
export interface CodeDef { name: string; scope: string; values: { value: string; desc: string; color: string }[] }
export interface Risk { id: number; name: string; cause: string; prob: number; impact: number; owner: string; response: string; status: string; tasks: number[]; kind: string; days: number }
export interface SchedOpts {
  retained: boolean; critical_float: number; longest_path: boolean; lag_cal: string; float_paths: number; open_ends_critical: boolean;
  level_within_slack: boolean; level_split: boolean; level_order: string; effort_default: boolean; task_type_default: string; from_finish: boolean; finish_date: string | null; res_cals?: boolean;
}
export interface Project {
  format: number; name: string; start: string; status: string | null; day_min: number; manual_default: boolean; calendar: string; calendars: Calendar[];
  resources: Resource[]; next_uid: number; columns: [string, number][]; code: string; description: string; client: string; manager: string; location: string;
  company: string; logo: string; currency: Currency; setup_done: boolean; id_prefix: string; id_start: number; id_step: number; opts: SchedOpts;
  fields: CustomDef[]; codes: CodeDef[]; risks: Risk[]; log: LogItem[]; credit: CreditRule[]; snapshots: { date: string | null; label: string; comment: string; finish: number; pct: number; cost: number }[]; ui: any;
  subplans?: SubPlan[]; master?: string;
  titles: Record<string, string>; sheets: SheetData[]; names?: DefName[]; pictures?: Picture[]; periods: { week_end: number; month_end: number; weight: string }; notes: PeriodNote[];
}
export interface Summary {
  name: string; start: string; finish: string; s: number; f: number; status: string; status_day: number | null; measured_at: string; dur: string; pct: number; work: string; cost: string;
  bl_finish: string; var_finish: string; pv: string; ev: string; ac: string; spi: number | null; cpi: number | null; tasks: number; done: number; late: number;
  critical: number; milestones: number; error: string | null; week: boolean[]; off_days: number[];
}
export interface SubPlan { file: string; name: string; level?: number }
export interface TabInfo { name: string; path: string | null; dirty: boolean; moved: number; role: "master" | "sub" | ""; group: number; level: number; project: string }
export interface Out { tabs: TabInfo[]; tab: number; project: Project; summary: Summary; path: string | null; dirty: boolean; undo: boolean; redo: boolean; clip: number; rev: number }

export interface Bar { s: number; f: number; ls: number; lf: number; bs: number | null; bf: number | null; dl: number | null; pct: number; segs: [number, number][]; links: [number, LinkType][] }
export interface Row {
  uid: number; id: number; lvl: number; kind: "task" | "summary" | "group" | "project" | "subplan"; ms: boolean; crit: boolean; late: boolean; manual: boolean; off: boolean; lp: boolean; ext?: string[]; ext_out?: string[]; ext_moved?: boolean; sub?: string; suid?: number;
  kids: boolean; open: boolean; hl: boolean; name: string; res: string; cells: string[]; icons?: [number, string][]; style?: Record<string, string>; bar: Bar;
}
export interface Col { key: string; name: string; kind: Kind; edit: boolean }
export interface Grid { cols: Col[]; rows: Row[]; shown: number; total: number }
export interface Rule { field: string; test: string; value: string; value2: string; or: boolean }
export interface Filter { name: string; rules: Rule[]; summaries: boolean; highlight: boolean }
export interface Query {
  cols: string[]; filter: Filter; group: string[]; sort: [string, boolean][]; flat: boolean; collapsed: number[]; level: number;
  hide_summaries: boolean; project_summary: boolean; baseline: number;
}
export interface FieldDef { key: string; name: string; kind: Kind; group: string; edit: boolean }
export interface FieldList { task: FieldDef[]; resource: FieldDef[]; filters: Filter[] }

export interface Rel { uid: number; id: number; code: string; name: string; kind: LinkType; lag: string; driving: boolean; tf: string }
export interface TaskInfo {
  task: any; fields: [string, string, string, string][]; preds: Rel[]; succs: Rel[];
  assigns: { res: number; name: string; kind: string; units: string; work: string; cost: string }[];
}
export interface Usage { buckets: number[]; unit: string; person: number[]; rows: { uid: number; of: number | null; name: string; lvl: number; sum: boolean; total: number; vals: number[]; over: boolean[]; cap?: number[] }[] }
export interface LogItem { id: number; uid: number; kind: string; text: string; owner: string; need: string | null; status: string }
export interface CreditRule { name: string; steps: [string, number][] }
export interface Manpower { buckets: number[]; unit: string; status_day: number; trades: { name: string; planned: number[]; actual: (number | null)[]; forecast: (number | null)[] }[] }
export interface EarnedSchedule { es: number; at: number; pd: number; spi_t: number; sv_t: number; ieac: number; planned_finish: number; forecast_finish: number }
export interface Lookahead { from: number; to: number; rows: { uid: number; code: string; name: string; es: number; ef: number; pct: number; res: string; crit: boolean; open: number; overdue: number }[] }
export interface TaskRisk { uid: number; name: string; value: number }
export interface SimResult { runs: number; planned: number; planned_chance: number; p10: number; p50: number; p80: number; p90: number; days: [number, number][]; cost: number; cost_p50: number; cost_p80: number; criticality: TaskRisk[]; drivers: TaskRisk[]; risks: TaskRisk[]; notes: string[] }
export interface LevelPreview { moved: { uid: number; name: string; start: number; new_start: number; finish: number; new_finish: number }[]; finish: number; new_finish: number; over_before: number; over_after: number }
/** An instant (minutes) as a short date: finishes at midnight belong to the day before. */
export const fmtAt = (t: number, fin = false) => fmtDay(Math.floor(((fin ? t - 1 : t)) / 1440));
export interface Curve { buckets: number[]; planned: number[]; forecast: number[]; actual: number[]; earned: number[]; status_day: number }
export interface Check { key: string; name: string; about: string; count: number; total: number; value: number; limit: string; pass: boolean; uids: number[] }
export interface Net { nodes: { uid: number; col: number; row: number }[]; links: { from: number; to: number; kind: LinkType; crit: boolean; driving: boolean; lag: string }[]; cols: number; rows: number; bands?: { name: string; row: number; rows: number }[] }
export interface Diff { uid: number; name: string; change: string; was: string[]; now: string[]; finish_var: string }
export interface Options { dateFmt: string; autosave: number; beta: boolean; currency: Currency; company: string; logo: string; workspace: string; setupPanel: boolean; theme: string;
  openLast: boolean; startView: string; manualDefault: boolean; weekEnd: number; monthEnd: number; paper: string; landscape: boolean }

/** Days since 1970-01-01 (local) to a Date at local midnight + fraction. */
export const dayDate = (d: number) => { const x = new Date(1970, 0, 1); x.setDate(x.getDate() + Math.floor(d)); x.setMinutes(Math.round((d - Math.floor(d)) * 1440)); return x; };
export const dateDay = (x: Date) => Math.round((Date.UTC(x.getFullYear(), x.getMonth(), x.getDate()) - Date.UTC(1970, 0, 1)) / 864e5);
const MON = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
export const fmtDay = (d: number, style: "short" | "long" | "month" = "short") => {
  const x = dayDate(d);
  if (style === "month") return `${MON[x.getMonth()]} ${x.getFullYear()}`;
  return style === "long" ? `${x.getDate()} ${MON[x.getMonth()]} ${x.getFullYear()}` : `${x.getDate()} ${MON[x.getMonth()]}`;
};
export const iso = (d: number) => { const x = dayDate(d); return `${x.getFullYear()}-${String(x.getMonth() + 1).padStart(2, "0")}-${String(x.getDate()).padStart(2, "0")}`; };
export const money = (v: number, c: Currency) => {
  const s = Math.abs(v).toLocaleString("en-US", { minimumFractionDigits: c.decimals, maximumFractionDigits: c.decimals });
  return (v < 0 ? "-" : "") + (c.after ? `${s} ${c.symbol}` : `${c.symbol}${s}`);
};

export interface SheetData { name: string; cells: Record<string, string>; fmt: Record<string, string>; widths: Record<string, number>; style?: Record<string, string>; images?: SheetImage[]; merges?: string[]; heights?: Record<string, number>; color?: string; hidden?: boolean; page?: SheetPage; cond?: CondRule[]; hide_rows?: number[]; hide_cols?: number[]; notes?: Record<string, string>; filter?: SheetFilter | null; valid?: Validation[] }
/** AutoFilter: the range (headings in its first row) and each filtered column (0-based on the sheet). */
export interface SheetFilter { range: string; cols: ColFilter[] }
export interface ColFilter { col: number; values?: string[] | null; op: string; a: string; b: string }
/** A drop-down list on a range: typed values or cells (src). */
/** Data validation: kind "" is a drop-down list (list or src); "whole", "decimal", "date", "time", "len" compare with a and b by op; "any" only shows the prompt. */
export interface Validation { range: string; list: string[]; src: string; warn?: boolean; kind?: string; op?: string; a?: string; b?: string; prompt?: string; error?: string }
/** A defined name: what every sheet formula can call it by, and what it refers to (=Prices!$B$2:$B$20, =0.05). */
export interface DefName { name: string; formula: string; comment?: string }
/** A conditional formatting rule (see condfmt.tsx). */
export interface CondRule { range: string; op: string; a: string; b: string; style: string }
/** A sheet's own page setup (empty text and missing values follow the plan's). */
export interface SheetPage { paper?: string; orient?: string; margins?: number[] | null; scale?: number; fit_w?: number; fit_h?: number; area?: string; titles?: string; no_hf?: boolean; no_grid?: boolean; headings?: boolean; center?: boolean; breaks?: number[]; col_breaks?: number[] }
/** A picture in the plan's library (a party's logo or any image), as a data URL. */
export interface Picture { id: number; name: string; role: string; data: string }
/** A library picture placed on a sheet at a cell, with its size in px. */
export interface SheetImage { pic: number; cell: string; w: number; h: number; fit?: boolean }
export interface Photo { data: string; caption: string }
export interface PeriodNote { kind: string; end: string | null; summary: string; progress: string; issues: string; actions: string; look_ahead: string; photos: Photo[] }
export interface SheetOut { name: string; rows: number; cols: number; cells: [string, string, string, boolean, number | null][]; filtered?: number[]; lists?: string[][]; links?: [string, string][]; invalid?: string[] }
export interface SCurve { days: number[]; planned: number[]; actual: (number | null)[]; forecast: number[]; total: number; status_day: number; name: string; weight: string }
export interface PRow { uid: number; code: string; name: string; start: string; finish: string; pct: number; note: string }
export interface PeriodReport {
  kind: string; from: number; to: number; prev_from: number; planned_prev: number; planned: number; actual_prev: number; actual: number;
  finish: string; baseline_finish: string; finish_var: string; spi: number | null; cpi: number | null; cost: string; critical: number; late: number;
  completed: PRow[]; started: PRow[]; in_progress: PRow[]; slipped: PRow[]; upcoming: PRow[]; milestones: PRow[];
  wbs: { uid: number; name: string; planned_prev: number; planned: number; actual_prev: number; actual: number }[]; hours: [number, number][];
}
