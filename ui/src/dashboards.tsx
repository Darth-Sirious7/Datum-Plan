// Custom dashboards: boards of tiles the user lays out, several per plan, saved with the plan
// (ui.dashboards). Tiles read the plan live: numbers and rings from spreadsheet formulas (=SPI,
// =COUNTIF(Tasks[Late],TRUE)), the plan's charts and lists, charts and tables from sheet ranges or
// formulas, and country flags. The engine works out every formula in one call (dash_eval).
import { For, Show, Switch, Match, createMemo, createResource, createSignal, JSX } from "solid-js";
import { askText, askYes } from "./ui";
import { act, call, out, P, tick, openMenu, openDialog, Item } from "./store";
import { Curve, PeriodReport, SCurve, Usage } from "./api";
import { pictures, picData, addPicture } from "./pictures";
import { Dialog, Row } from "./ui";
import { ChartBox, COL, bar, line } from "./charts";
import { Brand, PeriodBars, Rows, SCurveChart, Table, cum, label, q, r, useRows } from "./reports";
import { CountryList, Flag, countryCode, countryName } from "./flags";
import { ERRORS } from "./formulas";

export type Kind = "number" | "ring" | "chart" | "table" | "countries" | "text" | "image";
export type Tile = {
  id: string; kind: Kind; title: string; w: number; h: number;
  /** number / ring: the value; countries: the list of countries. */
  formula?: string; fmt?: string;
  /** A second line: text, or a formula when it starts with =. */
  sub?: string;
  /** number: green when the rule holds, red when not. */
  good?: [string, number];
  /** chart / table: a plan chart or list, or a sheet range or formula. */
  plan?: string; src?: string; chart?: string;
  text?: string;
  /** image: a picture of the plan's library, and whether its title shows. */
  pic?: number;
};
export type Dash = { name: string; tiles: Tile[] };
type Shown = [string, number | null, string];

export const dashboards = (): Dash[] => (P().ui?.dashboards ?? []) as Dash[];
/** The dashboard shown: -1 is the built-in project dashboard. */
export const [dashK, setDashK] = createSignal(-1);
const save = (list: Dash[]) => act({ Ui: { key: "dashboards", value: list } });
const newId = () => Math.random().toString(36).slice(2, 9);
const T = (kind: Kind, title: string, w: number, h: number, extra: Partial<Tile> = {}): Tile => ({ id: newId(), kind, title, w, h, ...extra });

const KINDS: [Kind, string, string][] = [
  ["number", "Number", "One figure from a formula: =SPI, =ProjectFinish, =COUNTIF(Tasks[Late],TRUE)"],
  ["ring", "Progress ring", "A percentage (0–100) as a ring: =PercentComplete"],
  ["chart", "Chart", "A plan chart, or a chart of a sheet range"],
  ["table", "Table", "A plan list (milestones, late tasks…), or a sheet range or formula"],
  ["countries", "Countries", "Flags with how many vendors or people come from each country"],
  ["text", "Text", "A heading or a note"],
  ["image", "Picture", "A logo or picture from the plan's library (Project › Pictures)"],
];
const PLAN_CHARTS: [string, string][] = [["scurve", "S-curve"], ["periodbars", "Progress per period"], ["phases", "Progress by phase"], ["costev", "Cost and earned value"], ["manpower", "Manpower histogram"]];
const PLAN_LISTS: [string, string][] = [["milestones", "Milestones"], ["late", "Late tasks"], ["critical", "Critical tasks"], ["upcoming", "Starting next period"], ["completed", "Completed this period"]];
const CHART_TYPES: [string, string][] = [["bar", "Columns"], ["stacked", "Stacked columns"], ["hbar", "Bars"], ["line", "Line"], ["area", "Area"], ["doughnut", "Doughnut"], ["pie", "Pie"]];
const FMTS: [string, string][] = [["", "Automatic"], ["#,##0", "Whole number"], ["#,##0.00", "Number 1,234.50"], ["0.00", "Index 0.95"], ['0"%"', "Percent of 100"], ["0%", "Percent of 1"], ["money", "Currency"], ["date", "Date"]];
const SIZES: [number, string][] = [[3, "Quarter"], [4, "Third"], [6, "Half"], [8, "Two thirds"], [12, "Full width"]];

/** Ready-made boards to start from. */
const TEMPLATES: Dash[] = [
  { name: "Client dashboard", tiles: [
    T("number", "Actual progress", 3, 1, { formula: "=PercentComplete", sub: '="Planned "&TEXT(PlannedPercent,"0")&"%"' }),
    T("number", "Forecast finish", 3, 1, { formula: "=ProjectFinish", sub: '=IF(BaselineFinish="","No baseline yet","Baseline "&TEXT(BaselineFinish,"dd-mmm-yy"))' }),
    T("number", "SPI", 3, 1, { formula: "=SPI", fmt: "0.00", good: [">=", 1], sub: "Schedule performance index" }),
    T("number", "Late tasks", 3, 1, { formula: "=COUNTIF(Tasks[Late],TRUE)", good: ["<=", 0], sub: '="of "&TaskCount&" tasks"' }),
    T("chart", "Progress S-curve", 8, 3, { plan: "scurve" }),
    T("ring", "Complete", 4, 3, { formula: "=PercentComplete", sub: '="Planned "&TEXT(PlannedPercent,"0")&"%"' }),
    T("table", "Milestones", 6, 3, { plan: "milestones" }),
    T("table", "Late tasks", 6, 3, { plan: "late" }),
  ] },
  { name: "Weekly progress", tiles: [
    T("number", "Actual", 3, 1, { formula: "=PercentComplete" }),
    T("number", "Planned", 3, 1, { formula: "=PlannedPercent" }),
    T("number", "Variance", 3, 1, { formula: "=PercentComplete-PlannedPercent", fmt: '0.0"%"', good: [">=", 0] }),
    T("number", "Tasks done", 3, 1, { formula: "=TasksDone", sub: '="of "&TaskCount' }),
    T("chart", "S-curve", 6, 3, { plan: "scurve" }),
    T("chart", "Progress per period", 6, 3, { plan: "periodbars" }),
    T("chart", "Progress by phase", 6, 3, { plan: "phases" }),
    T("table", "Starting next period", 6, 3, { plan: "upcoming" }),
  ] },
  { name: "Vendors and countries", tiles: [
    T("number", "Vendors and crews", 3, 1, { formula: "=ROWS(Resources)" }),
    T("number", "Countries", 3, 1, { formula: '=IFERROR(ROWS(UNIQUE(FILTER(Resources[Country],Resources[Country]<>""))),0)' }),
    T("number", "Forecast cost", 3, 1, { formula: "=TotalCost", fmt: "money" }),
    T("number", "Critical tasks", 3, 1, { formula: "=CriticalCount" }),
    T("countries", "Where they come from", 6, 3, { formula: "=Resources[Country]" }),
    T("table", "Vendors", 6, 3, { src: '=IFNA(VSTACK({"Name","Group","Country"},FILTER(CHOOSECOLS(Resources,1,3,9),Resources[Country]<>"","None yet")),"")' }),
    T("chart", "Manpower", 12, 3, { plan: "manpower" }),
  ] },
];

/** The tabs over the Dashboard page: the project dashboard, the plan's own, and New. */
export const DashTabs = () => {
  const make = (d: Dash) => save([...dashboards(), JSON.parse(JSON.stringify({ ...d, tiles: d.tiles.map(t => ({ ...t, id: newId() })) }))]).then(() => setDashK(dashboards().length - 1));
  const newMenu = (e: MouseEvent) => openMenu(e, [{ label: "Blank dashboard", run: () => make({ name: `Dashboard ${dashboards().length + 1}`, tiles: [] }) }, "-",
    ...TEMPLATES.map(t => ({ label: `From template: ${t.name}`, run: () => make(t) } as Item))]);
  return (
    <div class="dashtabs noprint">
      <button classList={{ on: dashK() < 0 || !dashboards()[dashK()] }} onClick={() => setDashK(-1)}>Project</button>
      <For each={dashboards()}>{(d, i) => <button classList={{ on: dashK() === i() }} onClick={() => setDashK(i())}>{d.name}</button>}</For>
      <button class="add" title="A dashboard of your own: numbers, charts, tables and flags laid out as you like" onClick={newMenu}>+ New dashboard</button>
    </div>
  );
};

/** What a tile asks the engine to work out: [formula, format] pairs. */
const asks = (t: Tile): [string, string][] => {
  const v: [string, string][] = [];
  if ((t.kind === "number" || t.kind === "ring") && t.formula) v.push([t.formula, t.fmt ?? ""]);
  if ((t.kind === "number" || t.kind === "ring") && t.sub?.startsWith("=")) v.push([t.sub, ""]);
  if ((t.kind === "chart" || t.kind === "table") && !t.plan && t.src) v.push([t.src, ""]);
  if (t.kind === "countries") v.push([t.formula || "=Resources[Country]", ""]);
  return v;
};
const key = (f: [string, string]) => `${f[1]}\u0001${f[0]}`;

export const CustomDashboard = (p: { k: number }) => {
  const d = () => dashboards()[p.k];
  const [edit, setEdit] = createSignal(false);
  const s = () => out()!.summary;
  const unit = () => (s().f - s().s > 240 ? "month" : "week");
  const items = createMemo(() => { const m = new Map<string, [string, string]>(); for (const t of d()?.tiles ?? []) for (const a of asks(t)) m.set(key(a), a); return [...m.values()]; });
  const [vals] = createResource(() => [tick(), JSON.stringify(items())], async () => {
    const r = items().length ? await call<Shown[][][]>("dash_eval", { items: items() }) : [];
    return new Map(items().map((a, i) => [key(a), r?.[i] ?? []]));
  });
  const val = (f: string | undefined, fmt = "") => (f ? vals()?.get(key([f, fmt])) : undefined);
  const needs = (...plans: string[]) => d()?.tiles.some(t => t.plan && plans.includes(t.plan));
  const [sc] = createResource(() => needs("scurve", "periodbars") && [tick(), unit()], () => call<SCurve>("scurve", { unit: unit(), how: P().periods.weight, scope: null }));
  const [rep] = createResource(() => needs("phases", "upcoming", "completed") && [tick(), unit()], () => call<PeriodReport>("period_report", { kind: unit(), end: null }));
  const [curve] = createResource(() => needs("costev") && [tick(), unit()], () => call<Curve>("curve", { unit: unit() }));
  const [use] = createResource(() => needs("manpower") && [tick(), unit()], () => call<Usage>("usage", { by: "resource", what: "work", unit: unit() }));
  const put = (tiles: Tile[]) => save(dashboards().map((x, i) => (i === p.k ? { ...x, tiles } : x)));
  const setTile = (t: Tile) => put(d().tiles.map(x => (x.id === t.id ? t : x)));
  const move = (id: string, before: string) => { const v = [...d().tiles], a = v.findIndex(x => x.id === id), [m] = v.splice(a, 1); const b = v.findIndex(x => x.id === before); v.splice(b < 0 ? v.length : b, 0, m); put(v); };
  const sizeMenu = (e: MouseEvent, t: Tile) => openMenu(e, [
    { label: "Width", sub: SIZES.map(([w, l]) => ({ label: l, on: t.w === w, run: () => setTile({ ...t, w }) })) },
    { label: "Height", sub: [1, 2, 3, 4].map(h => ({ label: ["One row", "Two rows", "Three rows", "Four rows"][h - 1], on: t.h === h, run: () => setTile({ ...t, h }) })) },
  ]);
  const boardMenu = (e: MouseEvent) => openMenu(e, [
    { label: "Rename…", run: async () => { const n = await askText("Rename Dashboard", "Name", d().name); if (n?.trim()) save(dashboards().map((x, i) => (i === p.k ? { ...x, name: n.trim() } : x))); } },
    { label: "Duplicate", run: () => save([...dashboards(), JSON.parse(JSON.stringify({ ...d(), name: `${d().name} (copy)` }))]).then(() => setDashK(dashboards().length - 1)) },
    { label: "Delete…", run: async () => { if (await askYes("Delete Dashboard", `Delete the dashboard ${d().name}?`, "Delete")) save(dashboards().filter((_, i) => i !== p.k)).then(() => setDashK(-1)); } },
  ]);
  let dragged = "";
  return (
    <Show when={d()}>
      <div class="dashbar noprint">
        <button class="btn sm" classList={{ pri: edit() }} onClick={() => setEdit(!edit())}>{edit() ? "Done" : "Edit layout"}</button>
        <Show when={edit()}><button class="btn sm" onClick={() => editTile(null, t => put([...d().tiles, t]))}>+ Add tile</button></Show>
        <button class="btn sm" onClick={boardMenu}>Dashboard ▾</button>
        <Show when={edit()}><span class="muted">Drag tiles to reorder them; use the buttons on a tile to change it, size it or remove it.</span></Show>
      </div>
      <Brand title={d().name} />
      <CountryList />
      <Show when={d().tiles.length} fallback={<div class="panel"><p class="empty">This dashboard is empty. Add numbers, charts, tables and flags with <b>+ Add tile</b>.</p><button class="btn pri" style={{ width: "fit-content" }} onClick={() => { setEdit(true); editTile(null, t => put([...d().tiles, t])); }}>+ Add tile</button></div>}>
        <div class="dboard" classList={{ arranging: edit() }}>
          <For each={d().tiles}>{t => (
            <div class={`dtile dw${t.w} dh${t.h} k-${t.kind}`} draggable={edit()} onDragStart={() => { dragged = t.id; }} onDragOver={e => { if (edit()) e.preventDefault(); }} onDrop={e => { e.preventDefault(); if (dragged && dragged !== t.id) move(dragged, t.id); dragged = ""; }}>
              <Show when={edit()}><div class="dtools">
                <button title="Change" onClick={() => editTile(t, setTile)}>✎</button><button title="Size" onClick={e => sizeMenu(e, t)}>⤢</button>
                <button title="Remove" onClick={() => put(d().tiles.filter(x => x.id !== t.id))}>×</button></div></Show>
              <TileView t={t} val={val} unit={unit()} sc={sc()} rep={rep()} curve={curve()} use={use()} />
            </div>
          )}</For>
        </div>
      </Show>
    </Show>
  );
};

type TileProps = { t: Tile; val: (f: string | undefined, fmt?: string) => Shown[][] | undefined; unit: string; sc?: SCurve; rep?: PeriodReport; curve?: Curve; use?: Usage };
const one = (v?: Shown[][]) => v?.[0]?.[0];
const subText = (p: TileProps) => (p.t.sub?.startsWith("=") ? one(p.val(p.t.sub))?.[0] : p.t.sub) ?? "";

const TileView = (p: TileProps): JSX.Element => (
  <Switch>
    <Match when={p.t.kind === "number"}><NumberTile {...p} /></Match>
    <Match when={p.t.kind === "ring"}><RingTile {...p} /></Match>
    <Match when={p.t.kind === "chart"}><h3>{p.t.title}</h3><div class="tbody"><ChartTile {...p} /></div></Match>
    <Match when={p.t.kind === "table"}><h3>{p.t.title}</h3><div class="tbody scroll"><TableTile {...p} /></div></Match>
    <Match when={p.t.kind === "countries"}><h3>{p.t.title}</h3><div class="tbody scroll"><CountriesTile {...p} /></div></Match>
    <Match when={p.t.kind === "text"}><h3>{p.t.title}</h3><div class="tbody dtext">{p.t.text}</div></Match>
    <Match when={p.t.kind === "image"}><Show when={p.t.title}><h3>{p.t.title}</h3></Show><div class="tbody dimg">{picData(p.t.pic ?? 0) ? <img src={picData(p.t.pic ?? 0)} alt={p.t.title} /> : <span class="muted">Choose a picture (✎).</span>}</div></Match>
  </Switch>
);

const holds = (n: number, [op, v]: [string, number]) => (op === ">=" ? n >= v : op === "<=" ? n <= v : op === ">" ? n > v : op === "<" ? n < v : n === v);
const NumberTile = (p: TileProps) => {
  const v = () => one(p.val(p.t.formula, p.t.fmt ?? ""));
  const state = () => { const n = v()?.[1]; return p.t.good && n !== null && n !== undefined ? (holds(n, p.t.good) ? "good" : "bad") : ""; };
  return (
    <div class={`dnum ${state()}`}>
      <div class="v" title={ERRORS[v()?.[0] ?? ""]}>{v()?.[0] || "—"}</div>
      <div class="l">{p.t.title}</div>
      <Show when={subText(p)}><div class="sub">{subText(p)}</div></Show>
    </div>
  );
};

/** A percentage (0–100) as a ring with the figure in the middle. */
const RingTile = (p: TileProps) => {
  const n = () => Math.max(0, Math.min(100, one(p.val(p.t.formula, p.t.fmt ?? ""))?.[1] ?? 0));
  const R = 52, L = 2 * Math.PI * R;
  return (
    <div class="dring">
      <h3>{p.t.title}</h3>
      <svg viewBox="0 0 140 140"><circle cx="70" cy="70" r={R} class="track" /><circle cx="70" cy="70" r={R} class="val" opacity={n() > 0 ? 1 : 0} stroke-dasharray={`${(n() / 100) * L} ${L}`} transform="rotate(-90 70 70)" />
        <text x="70" y="76" text-anchor="middle">{`${Math.round(n())}%`}</text></svg>
      <Show when={subText(p)}><div class="sub">{subText(p)}</div></Show>
    </div>
  );
};

/** A table of shown values as series: first column = labels; a first row with text = series names. */
const series = (m: Shown[][]) => {
  const head = m.length > 1 && m[0].slice(1).some(c => c[1] === null && c[0] !== "");
  const rows = (head ? m.slice(1) : m).filter(r => r.some(c => c[0] !== ""));
  const n = Math.max(1, ...m.map(r => r.length)) - 1;
  const names = Array.from({ length: Math.max(1, n) }, (_, j) => (head ? m[0][j + 1]?.[0] : "") || `Series ${j + 1}`);
  return { labels: rows.map(r => r[0]?.[0] ?? ""), names, data: names.map((_, j) => rows.map(r => r[j + 1]?.[1] ?? null)) };
};
const ChartTile = (p: TileProps) => {
  const pal = Object.values(COL);
  return (
    <Switch fallback={<RangeChart {...p} pal={pal} />}>
      <Match when={p.t.plan === "scurve"}><Show when={p.sc}>{c => <SCurveChart c={c()} unit={p.unit} title={p.t.title} size="fill" bare />}</Show></Match>
      <Match when={p.t.plan === "periodbars"}><Show when={p.sc}>{c => <PeriodBars c={c()} unit={p.unit} size="fill" bare />}</Show></Match>
      <Match when={p.t.plan === "phases"}><Show when={p.rep}>{x => <ChartBox title={p.t.title} type="bar" horizontal fmt="pct" max={100} size="fill" bare labels={x().wbs.map(w => w.name)}
        datasets={[bar("Planned", x().wbs.map(w => w.planned), COL.grey), bar("Actual", x().wbs.map(w => w.actual), COL.blue)]} />}</Show></Match>
      <Match when={p.t.plan === "costev"}><Show when={p.curve}>{c => <ChartBox title={p.t.title} type="line" fmt="money" size="fill" bare days={c().buckets} labels={c().buckets.map(d => label(d, p.unit))} marker={c().buckets.findIndex(d => d >= (out()!.summary.status_day ?? 0))}
        datasets={[line("Planned value", cum(c().planned), COL.grey, [6, 4]), line("Earned value", cum(c().earned), COL.green), line("Actual cost", cum(c().actual), COL.red), line("Forecast", cum(c().forecast), COL.blue, [2, 3])]} />}</Show></Match>
      <Match when={p.t.plan === "manpower"}><Show when={p.use}>{u => <ChartBox title={p.t.title} type="bar" fmt="hours" stacked size="fill" bare days={u().buckets} labels={u().buckets.map(d => label(d, p.unit))}
        datasets={u().rows.filter(x => x.of === null).slice(0, 8).map((x, i) => bar(x.name, x.vals, pal[i % 8], "h"))} />}</Show></Match>
    </Switch>
  );
};
const RangeChart = (p: TileProps & { pal: string[] }) => {
  const s = createMemo(() => series(p.val(p.t.src) ?? []));
  const kind = () => p.t.chart ?? "bar";
  return (
    <Show when={p.t.src} fallback={<p class="empty">Choose what to chart: ✎ in Edit layout.</p>}>
      <ChartBox title={p.t.title} size="fill" bare labels={s().labels} horizontal={kind() === "hbar"} stacked={kind() === "stacked"}
        type={kind() === "doughnut" || kind() === "pie" ? kind() as "pie" : kind() === "line" || kind() === "area" ? "line" : "bar"}
        datasets={kind() === "doughnut" || kind() === "pie" ? [{ label: s().names[0], data: s().data[0] ?? [] } as never]
          : s().names.map((n, j) => (kind() === "line" || kind() === "area" ? line(n, s().data[j], p.pal[j % 8], undefined, kind() === "area") : bar(n, s().data[j], p.pal[j % 8], kind() === "stacked" ? "s" : undefined)))} />
    </Show>
  );
};

/** A column shows flags when its cells have the flag format, or are all countries. */
const flagCol = (m: Shown[][], j: number, head: boolean) => {
  const cells = (head ? m.slice(1) : m).map(r => r[j]).filter(c => c && c[0]);
  return cells.length > 0 && (cells.every(c => c[2] === "flag") || cells.every(c => countryCode(c[0]) !== ""));
};
const TableTile = (p: TileProps) => (
  <Switch fallback={<RangeTable m={p.val(p.t.src) ?? []} />}>
    <Match when={p.t.plan === "milestones"}><PlanList cols={["name", "finish", "b0finish", "pct"]} rules={[r("milestone", "equals", "Yes")]} empty="No milestones." /></Match>
    <Match when={p.t.plan === "late"}><PlanList cols={["name", "finish", "pct", "res"]} rules={[r("late", "equals", "Yes"), r("summary", "equals", "No")]} empty="Nothing is late." /></Match>
    <Match when={p.t.plan === "critical"}><PlanList cols={["name", "start", "finish", "tf"]} rules={[r("critical", "equals", "Yes"), r("summary", "equals", "No")]} empty="No critical tasks." /></Match>
    <Match when={p.t.plan === "upcoming"}><Rows rows={p.rep?.upcoming ?? []} empty="Nothing starts in the next period." /></Match>
    <Match when={p.t.plan === "completed"}><Rows rows={p.rep?.completed ?? []} empty="Nothing finished in this period." /></Match>
  </Switch>
);
const PlanList = (p: { cols: string[]; rules: ReturnType<typeof r>[]; empty: string }) => {
  const [g] = useRows(() => [p.cols, p.rules], () => q(p.cols, p.rules));
  return <Table g={g()} empty={p.empty} />;
};
const RangeTable = (p: { m: Shown[][] }) => {
  const head = () => p.m.length > 1 && p.m[0].every(c => c[1] === null);
  const flags = createMemo(() => (p.m[0] ?? []).map((_, j) => flagCol(p.m, j, head())));
  return (
    <Show when={p.m.length && p.m.some(r => r.some(c => c[0]))} fallback={<p class="empty">Nothing to show yet.</p>}>
      <table class="tbl">
        <Show when={head()}><thead><tr><For each={p.m[0]}>{c => <th>{c[0]}</th>}</For></tr></thead></Show>
        <tbody><For each={(head() ? p.m.slice(1) : p.m).slice(0, 200)}>{row => <tr><For each={row}>{(c, j) => <td classList={{ r: c[1] !== null && c[2] !== "flag" }} title={ERRORS[c[0]]}>
          <Show when={flags()[j()] && c[0]} fallback={c[0]}><span class="flagtext"><Flag country={c[0]} />{countryName(c[0])}</span></Show></td>}</For></tr>}</For></tbody>
      </table>
    </Show>
  );
};

/** Flags with a count per country, most first. */
const CountriesTile = (p: TileProps) => {
  const list = createMemo(() => {
    const m = new Map<string, number>();
    for (const row of p.val(p.t.formula || "=Resources[Country]") ?? []) for (const c of row) { const k = countryCode(c[0]); if (k) m.set(k, (m.get(k) ?? 0) + 1); }
    return [...m].sort((a, b) => b[1] - a[1] || countryName(a[0]).localeCompare(countryName(b[0])));
  });
  return (
    <Show when={list().length} fallback={<p class="empty">No countries yet. Give resources a Country in the Resource Sheet, or point this tile at a sheet column.</p>}>
      <div class="dflags"><For each={list()}>{([c, n]) => <div class="dflag"><Flag country={c} h={22} /><span>{countryName(c)}</span><b>{n}</b></div>}</For></div>
    </Show>
  );
};

/** Add or change a tile. */
const editTile = (t: Tile | null, done: (t: Tile) => void) => openDialog(() => {
  const [x, setX] = createSignal<Tile>(t ? { ...t } : T("number", "New number", 3, 1, { formula: "=PercentComplete" }));
  const set = (v: Partial<Tile>) => setX({ ...x(), ...v });
  const kindDefaults: Record<Kind, Partial<Tile>> = {
    number: { w: 3, h: 1, formula: "=PercentComplete", title: "Actual progress" }, ring: { w: 4, h: 3, formula: "=PercentComplete", title: "Complete" },
    chart: { w: 6, h: 3, plan: "scurve", title: "S-curve" }, table: { w: 6, h: 3, plan: "milestones", title: "Milestones" },
    countries: { w: 6, h: 3, formula: "=Resources[Country]", title: "Countries" }, text: { w: 6, h: 1, text: "", title: "Notes" }, image: { w: 3, h: 1, title: "" },
  };
  // A component (not a call in the JSX), so typing never rebuilds the field and the cursor stays in it.
  const Formula = (q: { label: string; k: "formula" | "src" | "sub"; hint: string }) => (
    <Row label={q.label}><input class="mono" value={(x()[q.k] as string) ?? ""} placeholder={q.hint} onInput={e => set({ [q.k]: e.currentTarget.value })} /></Row>
  );
  return (
    <Dialog title={t ? "Change tile" : "Add tile"} okText={t ? "OK" : "Add"} help="dashboard" ok={() => done(x())}>
      <div class="form">
        <Row label="Tile"><select onChange={e => { const k = e.currentTarget.value as Kind; set({ kind: k, plan: undefined, src: undefined, ...kindDefaults[k] }); }}>
          <For each={KINDS}>{([k, l]) => <option value={k} selected={x().kind === k}>{l}</option>}</For></select></Row>
        <Row label="Title"><input value={x().title} onInput={e => set({ title: e.currentTarget.value })} /></Row>
        <Show when={x().kind === "number" || x().kind === "ring"}>
          <Formula label={"Value"} k="formula" hint={x().kind === "ring" ? "=PercentComplete (0–100)" : "=SPI or =COUNTIF(Tasks[Late],TRUE)"} />
          <Show when={x().kind === "number"}><Row label="Format"><select onChange={e => set({ fmt: e.currentTarget.value })}><For each={FMTS}>{([f, l]) => <option value={f} selected={(x().fmt ?? "") === f}>{l}</option>}</For></select></Row>
            <Row label="Green when"><span class="stackr"><select onChange={e => set({ good: e.currentTarget.value ? [e.currentTarget.value, x().good?.[1] ?? 0] : undefined })}><option value="">(no colour)</option>
              <For each={[">=", "<=", ">", "<", "="]}>{o => <option selected={x().good?.[0] === o}>{o}</option>}</For></select>
              <Show when={x().good}><input type="number" step="any" style={{ width: "90px" }} value={x().good![1]} onInput={e => set({ good: [x().good![0], +e.currentTarget.value] })} /><span class="muted">otherwise red</span></Show></span></Row></Show>
          <Formula label={"Second line"} k="sub" hint={'Text, or a formula such as ="Baseline "&TEXT(BaselineFinish,"dd-mmm-yy")'} />
        </Show>
        <Show when={x().kind === "chart" || x().kind === "table"}>
          <Row label="Show"><select onChange={e => set(e.currentTarget.value ? { plan: e.currentTarget.value, src: undefined } : { plan: undefined, src: x().src ?? "" })}>
            <For each={x().kind === "chart" ? PLAN_CHARTS : PLAN_LISTS}>{([k, l]) => <option value={k} selected={x().plan === k}>{l}</option>}</For>
            <option value="" selected={!x().plan}>{x().kind === "chart" ? "A sheet range…" : "A sheet range or formula…"}</option></select></Row>
          <Show when={!x().plan}>
            <Formula label={x().kind === "chart" ? "Range" : "Range or formula"} k="src" hint={x().kind === "chart" ? "=Vendors!A1:C8 (labels in the first column, names in the first row)" : '=Vendors!A1:D20 or =FILTER(CHOOSECOLS(Tasks,3,8),Tasks[Late])'} />
            <Show when={x().kind === "chart"}><Row label="Chart"><select onChange={e => set({ chart: e.currentTarget.value })}><For each={CHART_TYPES}>{([k, l]) => <option value={k} selected={(x().chart ?? "bar") === k}>{l}</option>}</For></select></Row></Show>
          </Show>
        </Show>
        <Show when={x().kind === "countries"}><Formula label={"Countries from"} k="formula" hint={"=Resources[Country] or =Vendors!B2:B40"} /></Show>
        <Show when={x().kind === "image"}><Row label="Picture"><span class="stackr"><select onChange={e => set({ pic: +e.currentTarget.value || undefined })}><option value="">(choose)</option>
          <For each={pictures()}>{q => <option value={q.id} selected={x().pic === q.id}>{q.name || "Picture"}{q.role ? ` (${q.role})` : ""}</option>}</For></select>
          <button class="btn sm" onClick={() => addPicture().then(id => id && set({ pic: id }))}>Add…</button></span></Row></Show>
        <Show when={x().kind === "text"}><Row label="Text"><textarea rows={4} value={x().text ?? ""} onInput={e => set({ text: e.currentTarget.value })} /></Row></Show>
        <Row label="Size"><span class="stackr"><select onChange={e => set({ w: +e.currentTarget.value })}><For each={SIZES}>{([w, l]) => <option value={w} selected={x().w === w}>{l}</option>}</For></select>
          <select onChange={e => set({ h: +e.currentTarget.value })}><For each={[1, 2, 3, 4]}>{h => <option value={h} selected={x().h === h}>{h === 1 ? "One row" : `${h} rows`}</option>}</For></select></span></Row>
      </div>
      <p class="muted">{KINDS.find(k => k[0] === x().kind)?.[2]}. Formulas read the plan by name: Tasks[Finish], Resources[Country], ProjectFinish, PercentComplete, PlannedPercent, SPI, CPI, TotalCost, TasksLate… and your sheets (=Sheet1!B4).</p>
    </Dialog>
  );
});
