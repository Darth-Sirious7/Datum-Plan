// Window state: the open plan, the current view and its table, filter, group and sort, selection.
import { createSignal, createEffect, createMemo, createRoot, on, batch, JSX } from "solid-js";
import { invoke, Out, Grid, Query, Filter, FieldList, Options } from "./api";

export const [out, setOutRaw] = createSignal<Out | null>(null);
export const [tick, setTick] = createSignal(0);
export const setOut = (o: Out | null | undefined) => { if (o) batch(() => { setOutRaw(o); setTick(t => t + 1); }); };
export const P = () => out()!.project;
export const [fields, setFields] = createSignal<FieldList>({ task: [], resource: [], filters: [] });
/** auto, light or dark; auto follows Windows. */
export const [theme, setThemeRaw] = createSignal("auto");
const darkMq = typeof matchMedia === "function" ? matchMedia("(prefers-color-scheme: dark)") : null;
export const applyTheme = () => { document.documentElement.dataset.theme = theme() === "auto" ? (darkMq?.matches ? "dark" : "light") : theme(); setThemeTick(t => t + 1); };
export const [themeTick, setThemeTick] = createSignal(0);
darkMq?.addEventListener("change", () => theme() === "auto" && applyTheme());
export const setTheme = (t: string) => { setThemeRaw(t); applyTheme(); invoke("set_options", { opts: { ...opts(), theme: t } }).then(o => setOut(o as Out)).catch(() => {}); setOpts({ ...opts(), theme: t }); };
export const [opts, setOpts] = createSignal<Options>({ dateFmt: "%d-%b-%y", autosave: 5, beta: false, currency: { code: "USD", symbol: "$", decimals: 2, after: false }, company: "", logo: "", workspace: "", setupPanel: true, theme: "auto", openLast: false, startView: "gantt", manualDefault: false, weekEnd: 4, monthEnd: 0, paper: "A4", landscape: true });

export type View = "setup" | "gantt" | "tracking" | "network" | "calendar" | "taskusage" | "ressheet" | "resusage" | "resgraph" | "team" | "dashboard" | "reports" | "portfolio" | "risks" | "simrisk" | "checks" | "lookahead" | "manpower" | "timeline" | "progress" | "scurve" | "sheets" | "update" | "relations" | "builder";
export const [view, setView] = createSignal<View>("gantt");
export const [backstage, setBackstage] = createSignal<string | null>("home");
export const [tab, setTab] = createSignal("task");

// ---------- tables (column sets), as in MS Project ----------
type Cols = [string, number][];
const N: [string, number] = ["name", 260];
export const TABLES: Record<string, Cols> = {
  Entry: [["mode", 34], N, ["dur", 74], ["start", 92], ["finish", 92], ["preds", 90], ["res", 140]],
  Schedule: [N, ["start", 92], ["finish", 92], ["ls", 92], ["lf", 92], ["ff", 70], ["tf", 70]],
  Tracking: [N, ["astart", 92], ["afinish", 92], ["pct", 64], ["phys", 64], ["adur", 74], ["rem", 74], ["acost", 92], ["awork", 80]],
  Cost: [N, ["fixedcost", 90], ["accrual", 80], ["cost", 100], ["b0cost", 100], ["vcost", 90], ["acost", 92], ["rcost", 92]],
  Work: [N, ["work", 80], ["b0work", 80], ["vwork", 80], ["awork", 80], ["rwork", 80], ["workpct", 64]],
  Variance: [N, ["start", 92], ["finish", 92], ["b0start", 92], ["b0finish", 92], ["vstart", 74], ["vfinish", 74]],
  "Earned Value": [N, ["bcws", 100], ["bcwp", 100], ["acwp", 100], ["sv", 90], ["cv", 90], ["spi", 54], ["cpi", 54], ["bac", 100], ["eac", 100], ["vac", 90]],
  Summary: [N, ["dur", 74], ["start", 92], ["finish", 92], ["pct", 64], ["cost", 100], ["work", 80]],
  Baseline: [N, ["b0dur", 74], ["b0start", 92], ["b0finish", 92], ["b0work", 80], ["b0cost", 100]],
  "Activities": [["code", 80], N, ["acttype", 110], ["dur", 70], ["rem", 70], ["start", 92], ["finish", 92], ["tf", 66], ["pct", 60], ["res", 120]],
};
export const tables = (): Record<string, Cols> => ({ ...TABLES, ...(out()?.project.ui?.tables ?? {}) });

// ---------- the view's query ----------
export const ALL: Filter = { name: "All Tasks", rules: [], summaries: true, highlight: false };
export const [table, setTable] = createSignal("Entry");
/** All columns of the table; a negative width marks a hidden column. `cols()` is what the grid shows. */
export const [allCols, setCols] = createSignal<Cols>(TABLES.Entry);
export const cols = (): Cols => allCols().filter(c => c[1] > 0);
export const [filter, setFilter] = createSignal<Filter>(ALL);
export const [group, setGroup] = createSignal<string[]>([]);
export const [sort, setSort] = createSignal<[string, boolean][]>([]);
export const [collapsed, setCollapsed] = createSignal<number[]>([]);
export const [level, setLevel] = createSignal(0);
export const [projSum, setProjSum] = createSignal(false);
export const [hideSum, setHideSum] = createSignal(false);
export const [baseline, setBaseline] = createSignal(0);
/** A master's combined view: its subplans shown under its tasks, and which are expanded (files). */
export const [showSubs, setShowSubs] = createSignal(true);
export const [openSubs, setOpenSubs] = createSignal<string[]>([]);
export const toggleSub = (file: string) => setOpenSubs(o => (o.includes(file) ? o.filter(f => f !== file) : [...o, file]));
export const [grid, setGrid] = createSignal<Grid>({ cols: [], rows: [], shown: 0, total: 0 });

export const query = (): Query => ({
  cols: cols().map(c => c[0]), filter: filter(), group: group(), sort: sort(), flat: false, collapsed: collapsed(), level: level(),
  hide_summaries: hideSum(), project_summary: projSum(), baseline: baseline(),
});

/** Switches table; the plan's own columns (saved with the file) back the Entry table. */
export const useTable = (name: string) => batch(() => { setTable(name); setCols(name === "Entry" && out()?.project.columns.length ? out()!.project.columns : tables()[name] ?? TABLES.Entry); });

let pending = 0;
export const refresh = async () => {
  if (!out()) return;
  const n = ++pending;
  try { const g = await invoke<Grid>("query", { q: query(), subs: { show: showSubs(), open: openSubs() } }); if (n === pending) setGrid(g); } catch (e) { toast(String(e)); }
};
createRoot(() => {
  const colKeys = createMemo(() => cols().map(c => c[0]).join("|"));
  createEffect(on([tick, colKeys, filter, group, sort, collapsed, level, projSum, hideSum, baseline, showSubs, openSubs], () => { refresh(); }));
});

/** Saves the Entry table's columns into the plan; other tables go to plan settings. */
export const saveCols = (shown: Cols) => {
  // Hidden columns keep their places; the shown ones fill the other slots in their new order.
  const rest = [...shown], c: Cols = [];
  for (const x of allCols()) c.push(x[1] > 0 ? rest.shift()! : x);
  c.push(...rest);
  saveAll(c.filter(Boolean));
};
export const saveAll = (c: Cols) => {
  setCols(c);
  if (table() === "Entry") act({ Columns: { cols: c } });
  else act({ Ui: { key: "tables", value: { ...(out()?.project.ui?.tables ?? {}), [table()]: c } } });
};

// ---------- selection ----------
export const [sel, setSel] = createSignal<number[]>([]);
export const [cur, setCur] = createSignal<{ row: number; col: number }>({ row: 0, col: 0 });
export const selRows = () => { const s = new Set(sel()); return grid().rows.filter(r => s.has(r.uid) && r.uid > 0); };
export const curRow = () => grid().rows[cur().row];
export const curUid = () => curRow()?.uid ?? 0;

// ---------- layout ----------
export const [details, setDetails] = createSignal(true);
export const [detailsTab, setDetailsTab] = createSignal("general");
export const [timeline, setTimeline] = createSignal(true);
export const [zoom, setZoom] = createSignal(24);
/** Project Setup is a page (sidebar › Setup). */
export const setupOpen = () => view() === "setup";
export const setSetupOpen = (v: boolean) => setView(v ? "setup" : ((opts().startView || "gantt") as View));
export const [gridW, setGridW] = createSignal(620);

// ---------- editing ----------
export const act = async (op: object, quiet = false): Promise<boolean> => {
  try { setOut(await invoke<Out>("apply", { op })); return true; }
  catch (e) { if (!quiet) toast(String(e)); return false; }
};
export const set = (uid: number, field: string, value: string) => act({ Set: { uid, field, value } });
export const call = async <T = any>(cmd: string, args?: object): Promise<T | undefined> => {
  try { return await invoke<T>(cmd, args); } catch (e) { toast(String(e)); return undefined; }
};
export const loadOut = async (cmd: string, args?: object) => { const o = await call<Out | null>(cmd, args); if (o) { setOut(o); setBackstage(null); useTable(table()); setView((opts().startView || "gantt") as View); if (cmd === "open") import("./plans").then(m => m.afterOpen()); } return o; };

export const hideCol = (key: string) => saveAll(allCols().map(c => (c[0] === key && c[1] > 0 ? [c[0], -c[1]] : c)));
export const unhideCol = (key: string) => saveAll(allCols().map(c => (c[0] === key ? [c[0], Math.abs(c[1]) || 90] : c)));
export const hiddenCols = () => allCols().filter(c => c[1] <= 0).map(c => c[0]);
/** Removes a column from the table; `field` also deletes a custom field's definition and values. */
export const deleteCol = (key: string, field = false) => {
  const c = allCols().filter(x => x[0] !== key);
  if (!field || !key.startsWith("c:")) return saveAll(c);
  setCols(c);
  const k = key.slice(2), ops: unknown[] = [{ DeleteField: { key: k } }];
  ops.push(table() === "Entry" ? { Columns: { cols: c } } : { Ui: { key: "tables", value: { ...(out()?.project.ui?.tables ?? {}), [table()]: c } } });
  return act({ Batch: ops });
};

// ---------- messages, menus, dialogs ----------
export const [msg, setMsg] = createSignal<{ text: string; ok: boolean } | null>(null);
let timer = 0;
export const toast = (text: string, ok = false) => { setMsg({ text, ok }); clearTimeout(timer); timer = window.setTimeout(() => setMsg(null), ok ? 3000 : 7000); };
export type Item = { label: string; run?: () => void; keys?: string; off?: boolean; on?: boolean; sub?: Item[]; icon?: string } | "-";
export const [menu, setMenu] = createSignal<{ x: number; y: number; items: Item[] } | null>(null);
export const openMenu = (e: MouseEvent | { x: number; y: number }, items: Item[]) => {
  if ("preventDefault" in e) e.preventDefault();
  const r = (e as MouseEvent).currentTarget instanceof HTMLElement && (e as MouseEvent).type === "click" ? ((e as MouseEvent).currentTarget as HTMLElement).getBoundingClientRect() : null;
  setMenu({ x: r ? r.left : (e as MouseEvent).clientX ?? (e as any).x, y: r ? r.bottom + 2 : (e as MouseEvent).clientY ?? (e as any).y, items });
};
export const [dialogs, setDialogs] = createSignal<(() => JSX.Element)[]>([]);
export const openDialog = (d: () => JSX.Element) => setDialogs(v => [...v, d]);
export const closeDialog = () => setDialogs(v => v.slice(0, -1));
