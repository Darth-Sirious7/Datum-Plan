// Update Progress (status a period), the Relationship Diagram, and the Report Builder.
import { For, Show, createSignal, createResource, JSX } from "solid-js";
import { Grid, TaskInfo, SCurve, PeriodReport, Curve, Usage, fmtDay, iso, dateDay } from "./api";
import { act, call, curUid, out, P, tick, toast, fields, openMenu, setView, Item } from "./store";
import { C, selectUid } from "./cmds";
import { Seg, Icon } from "./ui";
import * as D from "./dialogs";
import { Brand, Kpi, Table, Rows, Tools, SCurveChart, PeriodBars, q, r, useRows, cum, pct, label } from "./reports";
import { ChartBox, COL, line, bar } from "./charts";
import { Help } from "./help";

// ---------- Update Progress ----------
const UPD_COLS = ["code", "name", "b0start", "b0finish", "start", "finish", "astart", "afinish", "pct", "plannedpct", "pctvar", "rem", "notes"];
const EDIT = new Set(["astart", "afinish", "pct", "rem", "notes"]);
export const UpdateProgress = () => {
  const s = () => out()!.summary;
  const [kind, setKind] = createSignal("week");
  const [all, setAll] = createSignal(false);
  const status = () => P().status ?? iso(dateDay(new Date()));
  const next = () => iso(dateDay(new Date(status() + "T00:00")) + (kind() === "week" ? 7 : 31));
  const [g] = useRows(() => [kind(), all(), status()], () => q(UPD_COLS, all() ? [r("summary", "equals", "No")] : [
    r("summary", "equals", "No"), r("start", "is less than or equal to", next()), r("pct", "is less than", "100"),
    r("afinish", "is within", iso(dateDay(new Date(status() + "T00:00")) - (kind() === "week" ? 7 : 31)), status(), true),
  ], { flat: true }));
  const set = (uid: number, key: string, v: string) => act({ Cells: { cells: [[uid, key, v]] } });
  const asPlanned = (rows: Grid["rows"]) => act({ Cells: { cells: rows.filter(x => x.uid > 0).map(x => [x.uid, "pct", String(Math.round(parseFloat(x.cells[9]) || 0))] as [number, string, string]) } });
  const close = async () => {
    if (await act({ Snapshot: { label: `${kind() === "week" ? "Week" : "Month"} ending ${status()}`, comment: "" } })) { toast("Period closed: a snapshot of this status is stored for trend reports.", true); setView("progress"); }
  };
  return (
    <section class="page report">
      <div class="ptools noprint">
        <h1>Update Progress <Help k="updprogress" /></h1><span class="grow" />
        <label class="muted">Status date <input type="date" value={P().status ?? ""} onChange={e => act({ ProjectSet: { field: "status", value: e.currentTarget.value || null } })} /></label>
        <Seg options={[["week", "Weekly"], ["month", "Monthly"]]} value={kind()} set={setKind} />
        <label class="chk"><input type="checkbox" checked={all()} onChange={e => setAll(e.currentTarget.checked)} />All tasks</label>
      </div>
      <div class="kpis">
        <Kpi v={s().status || "not set"} l="Status date" /><Kpi v={`${Math.round(s().pct)}%`} l="Complete" /><Kpi v={g()?.rows.length ?? 0} l="Tasks to update" />
        <Kpi v={g()?.rows.filter(x => parseFloat(x.cells[10]) < 0).length ?? 0} l="Behind plan" bad={(g()?.rows.filter(x => parseFloat(x.cells[10]) < 0).length ?? 0) > 0} />
      </div>
      <div class="panel">
        <div class="ptools"><b>Tasks due by {fmtDay(dateDay(new Date(next() + "T00:00")), "long")}</b><span class="grow" />
          <button class="btn sm" onClick={() => g() && asPlanned(g()!.rows)}>Set all to planned %</button>
          <button class="btn sm" onClick={() => C.update()}>Update Project…</button>
          <button class="btn sm pri" onClick={close}>Close period</button></div>
        <Show when={g()} fallback={<p class="empty">Loading…</p>}>{x => <div class="wlwrap"><table class="tbl updt">
          <thead><tr><For each={x().cols}>{c => <th>{c.name}</th>}</For></tr></thead>
          <tbody><For each={x().rows}>{row => <tr classList={{ behind: parseFloat(row.cells[10]) < 0 }}>
            <For each={x().cols}>{(c, k) => <td classList={{ r: ["Pct", "Dur"].includes(c.kind) }}>{EDIT.has(c.key)
              ? <input type={c.key === "astart" || c.key === "afinish" ? "text" : "text"} value={row.cells[k()]} placeholder={c.key.startsWith("a") ? "dd-mmm-yy" : ""} size={c.key === "notes" ? 24 : 9} onChange={e => set(row.uid, c.key, e.currentTarget.value)} />
              : c.key === "name" ? <a class="link" onClick={() => selectUid(row.uid)}>{row.cells[k()]}</a> : row.cells[k()]}</td>}</For>
          </tr>}</For></tbody></table>
          <Show when={!x().rows.length}><p class="empty">Nothing to update for this period. Tick "All tasks" to see everything.</p></Show></div>}</Show>
        <p class="muted">Type actual start and finish dates, % complete or remaining duration; the plan reschedules as you go. Baseline % Complete is where the baseline says each task should be at the status date. Close the period to store a snapshot for trend reports, then write the notes in the Progress report.</p>
      </div>
    </section>
  );
};

// ---------- Relationship Diagram ----------
export const Relationships = () => {
  const [info] = createResource(() => [curUid(), tick()] as const, ([u]) => (u > 0 ? call<TaskInfo>("task_info", { uid: u }) : undefined));
  const W = 230, H = 62, GAP = 14;
  const box = (x: number, y: number, title: string, sub: string, main: boolean, uid: number, crit = false): JSX.Element => (
    <g transform={`translate(${x},${y})`} class="rbox" classList={{ main, crit }} onClick={() => uid && selectUid(uid)}>
      <rect width={W} height={H} /><text x="10" y="22" class="t">{title.length > 30 ? title.slice(0, 29) + "…" : title}</text><text x="10" y="44">{sub}</text>
    </g>
  );
  return (
    <section class="page">
      <div class="ptools"><h1>Relationship Diagram</h1><span class="muted">The selected task with its predecessors (left) and successors (right). Click a box to follow the chain. Driving links are bold.</span></div>
      <Show when={info()} fallback={<p class="empty">Select a task in the Gantt chart first.</p>}>{i => {
        const n = Math.max(i().preds.length, i().succs.length, 1), h = n * (H + GAP) + 40, mid = h / 2 - H / 2;
        const ys = (k: number, m: number) => 20 + (h - 40 - m * (H + GAP) + GAP) / 2 + k * (H + GAP);
        return <div class="netwrap"><svg width={3 * W + 260} height={h} class="rel">
          <defs><marker id="ra" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0 0L8 4L0 8z" fill="var(--muted)" /></marker></defs>
          <For each={i().preds}>{(x, k) => <><path d={`M${20 + W} ${ys(k(), i().preds.length) + H / 2}C${W + 90} ${ys(k(), i().preds.length) + H / 2} ${W + 60} ${mid + H / 2} ${W + 130} ${mid + H / 2}`} class="rl" classList={{ drv: x.driving }} marker-end="url(#ra)" />
            <text x={W + 40} y={ys(k(), i().preds.length) + H / 2 - 6} class="rlab">{x.kind}{x.lag && x.lag !== "0d" ? ` ${x.lag}` : ""}</text>
            {box(20, ys(k(), i().preds.length), `${x.code || x.id} ${x.name}`, `${x.kind}${x.lag && x.lag !== "0 days" ? " " + x.lag : ""} · float ${x.tf}`, false, x.uid)}</>}</For>
          {box(W + 130, mid, `${i().task.code} ${i().task.name}`, i().fields.filter(f => ["start", "finish", "tf"].includes(f[0])).map(f => f[3]).join(" → ").replace(/ → ([^→]*)$/, " · float $1"), true, 0)}
          <For each={i().succs}>{(x, k) => <><path d={`M${2 * W + 130} ${mid + H / 2}C${2 * W + 190} ${mid + H / 2} ${2 * W + 170} ${ys(k(), i().succs.length) + H / 2} ${2 * W + 240} ${ys(k(), i().succs.length) + H / 2}`} class="rl" classList={{ drv: x.driving }} marker-end="url(#ra)" />
            <text x={2 * W + 150} y={ys(k(), i().succs.length) + H / 2 - 6} class="rlab">{x.kind}{x.lag && x.lag !== "0d" ? ` ${x.lag}` : ""}</text>
            {box(2 * W + 240, ys(k(), i().succs.length), `${x.code || x.id} ${x.name}`, `${x.kind}${x.lag && x.lag !== "0 days" ? " " + x.lag : ""} · float ${x.tf}`, false, x.uid)}</>}</For>
        </svg></div>;
      }}</Show>
    </section>
  );
};

// ---------- Report Builder ----------
type Section = { kind: string; title: string; text?: string; cols?: string[]; filter?: string; unit?: string };
type Report = { name: string; sections: Section[] };
const KINDS: [string, string][] = [
  ["kpis", "Key figures"], ["scurve", "S-curve"], ["periodbars", "Progress per period"], ["phases", "Progress by phase"], ["costev", "Cost and earned value"],
  ["manpower", "Manpower histogram"], ["table", "Task table"], ["completed", "Completed this period"], ["lookahead", "Look-ahead"], ["milestones", "Milestones"],
  ["behind", "Behind baseline"], ["notes", "Period notes"], ["text", "Text"], ["break", "Page break"],
];
const sec = (kind: string, title?: string, extra: Partial<Section> = {}): Section => ({ kind, title: title ?? KINDS.find(k => k[0] === kind)![1], ...extra });
export const TEMPLATES: Report[] = [
  { name: "Weekly management report", sections: [sec("kpis"), sec("scurve"), sec("periodbars"), sec("phases"), sec("completed"), sec("lookahead", "Next week"), sec("notes")] },
  { name: "Monthly client report", sections: [sec("text", "Summary", { text: "Overall status, key achievements and concerns this month." }), sec("kpis"), sec("scurve", "Progress S-curve", { unit: "month" }), sec("phases"), sec("milestones"), sec("costev"), sec("notes")] },
  { name: "Schedule health", sections: [sec("kpis"), sec("behind"), sec("table", "Critical tasks", { cols: ["code", "name", "start", "finish", "tf"], filter: "Critical" }), sec("table", "Negative float", { cols: ["code", "name", "finish", "tf"], filter: "Negative Float" })] },
  { name: "Cost report", sections: [sec("kpis"), sec("costev"), sec("table", "Cost by task", { cols: ["name", "cost", "b0cost", "vcost", "acost", "rcost"] }), sec("manpower")] },
];
const SectionView = (p: { s: Section; unit: string; rep?: PeriodReport; sc?: SCurve; curve?: Curve; use?: Usage }): JSX.Element => {
  const s = () => out()!.summary;
  const flt = () => fields().filters.find(f => f.name === p.s.filter)?.rules ?? [];
  const [g] = useRows(() => [p.s.cols, p.s.filter], () => q(p.s.cols?.length ? p.s.cols : ["code", "name", "start", "finish", "pct"], flt()));
  const note = () => p.rep ? P().notes.find(n => n.kind === p.unit && n.end === iso(p.rep!.to)) : undefined;
  switch (p.s.kind) {
    case "kpis": return <Show when={p.rep}>{x => <div class="kpis">
      <Kpi v={pct(x().actual)} l="Actual progress" d={x().actual - x().actual_prev} unit="%" /><Kpi v={pct(x().planned)} l="Planned progress" d={x().planned - x().planned_prev} unit="%" />
      <Kpi v={pct(x().actual - x().planned)} l="Variance" bad={x().actual < x().planned - 0.5} /><Kpi v={x().finish} l="Forecast finish" /><Kpi v={s().spi?.toFixed(2) ?? "—"} l="SPI" /><Kpi v={s().cpi?.toFixed(2) ?? "—"} l="CPI" /><Kpi v={s().cost} l="Cost" /><Kpi v={x().late} l="Late tasks" bad={x().late > 0} />
    </div>}</Show>;
    case "scurve": return <Show when={p.sc}>{c => <SCurveChart c={c()} unit={p.unit} title={p.s.title} />}</Show>;
    case "periodbars": return <Show when={p.sc}>{c => <PeriodBars c={c()} unit={p.unit} />}</Show>;
    case "phases": return <Show when={p.rep}>{x => <ChartBox title={p.s.title} type="bar" horizontal fmt="pct" max={100} labels={x().wbs.map(w => w.name)} datasets={[bar("Previous", x().wbs.map(w => w.actual_prev), COL.sky), bar("Actual", x().wbs.map(w => w.actual), COL.blue), bar("Planned", x().wbs.map(w => w.planned), COL.grey)]} />}</Show>;
    case "costev": return <Show when={p.curve}>{c => <ChartBox title={p.s.title} type="line" fmt="money" days={c().buckets} labels={c().buckets.map(d => label(d, p.unit))} marker={c().buckets.findIndex(d => d >= (s().status_day ?? 0))}
      datasets={[line("Planned value", cum(c().planned), COL.grey, [6, 4]), line("Earned value", cum(c().earned), COL.green), line("Actual cost", cum(c().actual), COL.red), line("Forecast", cum(c().forecast), COL.blue, [2, 3])]} />}</Show>;
    case "manpower": return <Show when={p.use}>{u => <ChartBox title={p.s.title} type="bar" fmt="hours" stacked days={u().buckets} labels={u().buckets.map(d => label(d, p.unit))} datasets={u().rows.filter(x => x.of === null).slice(0, 8).map((x, i) => bar(x.name, x.vals, Object.values(COL)[i % 8], "h"))} />}</Show>;
    case "table": return <div class="panel"><h2>{p.s.title}</h2><Table g={g()} empty="No tasks match." /></div>;
    case "completed": return <div class="panel"><h2>{p.s.title}</h2><Rows rows={p.rep?.completed ?? []} empty="Nothing finished in this period." /></div>;
    case "lookahead": return <div class="panel"><h2>{p.s.title}</h2><Rows rows={p.rep?.upcoming ?? []} empty="Nothing starts in the next period." /></div>;
    case "milestones": return <div class="panel"><h2>{p.s.title}</h2><Rows rows={p.rep?.milestones ?? []} note="Variance" empty="No milestones." /></div>;
    case "behind": return <div class="panel"><h2>{p.s.title}</h2><Rows rows={p.rep?.slipped ?? []} note="Baseline" empty="Nothing is behind its baseline." /></div>;
    case "notes": return <div class="panel"><h2>{p.s.title}</h2><Show when={note()} fallback={<p class="empty">No notes for this period yet. Write them in the Progress report.</p>}>{n => <>
      <For each={[["Summary", n().summary], ["Progress", n().progress], ["Issues and risks", n().issues], ["Actions", n().actions], ["Look-ahead", n().look_ahead]].filter(x => x[1])}>{([t, v]) => <div class="notesec"><label>{t}</label><div class="notes-print" style={{ display: "block" }}>{v}</div></div>}</For>
      <div class="photos"><For each={n().photos}>{ph => <figure><img src={ph.data} alt={ph.caption} /><figcaption>{ph.caption}</figcaption></figure>}</For></div></>}</Show></div>;
    case "text": return <div class="panel"><h2>{p.s.title}</h2><div class="notes-print" style={{ display: "block" }}>{p.s.text}</div></div>;
    case "break": return <div class="pagebreak"><span class="noprint muted">— page break —</span></div>;
  }
  return null;
};
export const ReportBuilder = () => {
  const reports = () => (P().ui?.reports ?? []) as Report[];
  const [k, setK] = createSignal(0);
  const [unit, setUnit] = createSignal("week");
  const cur = () => reports()[k()];
  const save = (list: Report[]) => act({ Ui: { key: "reports", value: list } });
  const put = (rep: Report) => save(reports().map((x, i) => (i === k() ? rep : x)));
  const editSec = (i: number, s: Section) => put({ ...cur(), sections: cur().sections.map((x, j) => (j === i ? s : x)) });
  const move = (i: number, d: number) => { const v = [...cur().sections]; const [m] = v.splice(i, 1); v.splice(Math.max(0, Math.min(v.length, i + d)), 0, m); put({ ...cur(), sections: v }); };
  const [rep] = createResource(() => [tick(), unit()], () => call<PeriodReport>("period_report", { kind: unit(), end: null }));
  const [sc] = createResource(() => [tick(), unit()], () => call<SCurve>("scurve", { unit: unit(), how: P().periods.weight, scope: null }));
  const [curve] = createResource(() => [tick(), unit()], () => call<Curve>("curve", { unit: unit() }));
  const [use] = createResource(() => [tick(), unit()], () => call<Usage>("usage", { by: "resource", what: "work", unit: unit() }));
  const addMenu = (e: MouseEvent) => openMenu(e, KINDS.map(([kind, l]) => ({ label: l, run: () => put({ ...cur(), sections: [...cur().sections, sec(kind)] }) } as Item)));
  const colMenu = (e: MouseEvent, i: number, s: Section) => openMenu(e, fields().task.filter(f => !f.key.startsWith("c:") || P().fields.some(d => `c:${d.key}` === f.key)).slice(0, 200).map(f => ({ label: f.name, on: (s.cols ?? []).includes(f.key), run: () => editSec(i, { ...s, cols: (s.cols ?? []).includes(f.key) ? s.cols!.filter(x => x !== f.key) : [...(s.cols ?? ["name"]), f.key] }) })));
  return (
    <section class="page report">
      <div class="ptools noprint">
        <select value={k()} onChange={e => setK(+e.currentTarget.value)}><For each={reports()}>{(x, i) => <option value={i()}>{x.name}</option>}</For></select>
        <button class="btn sm" onClick={e => openMenu(e, [...TEMPLATES.map(t => ({ label: `From template: ${t.name}`, icon: "builder", run: () => save([...reports(), JSON.parse(JSON.stringify(t))]).then(() => setK(reports().length - 1)) } as Item)), "-", { label: "Blank report", run: () => save([...reports(), { name: "New report", sections: [sec("kpis")] }]).then(() => setK(reports().length - 1)) }])}>+ New report</button>
        <Show when={cur()}><button class="btn sm" onClick={() => { const n = prompt("Report name", cur().name); if (n) put({ ...cur(), name: n }); }}>Rename</button>
          <button class="btn sm" onClick={() => save([...reports(), JSON.parse(JSON.stringify({ ...cur(), name: `${cur().name} (copy)` }))])}>Duplicate</button>
          <button class="btn sm" onClick={() => confirm(`Delete ${cur().name}?`) && save(reports().filter((_, i) => i !== k())).then(() => setK(0))}>Delete</button>
          <button class="btn sm pri" onClick={addMenu}>+ Add section</button></Show>
        <Seg options={[["week", "Weekly"], ["month", "Monthly"]]} value={unit()} set={setUnit} />
        <span class="grow" /><button class="btn" onClick={() => C.print()}>Print / PDF</button><button class="btn" onClick={() => import("./reports").then(m => m.saveHtml(cur()?.name ?? "Report"))}>Save as HTML</button>
      </div>
      <Show when={cur()} fallback={<div class="panel"><p class="empty">Build your own report from sections: key figures, charts, tables with the columns and filter you choose, period notes and text. Start from a template:</p>
        <div class="tiles"><For each={TEMPLATES}>{t => <button class="tile" onClick={() => save([...reports(), JSON.parse(JSON.stringify(t))]).then(() => setK(reports().length - 1))}><div class="pic"><svg class="ic"><use href="#i-builder" /></svg></div><b>{t.name}</b><span>{t.sections.map(x => x.title).join(" · ")}</span></button>}</For></div></div>}>
        <Brand title={cur().name} sub={rep() ? `${fmtDay(rep()!.from, "long")} – ${fmtDay(rep()!.to, "long")}` : undefined} />
        <For each={cur().sections}>{(s, i) => (
          <div class="rsec">
            <div class="rsec-bar noprint">
              <input value={s.title} onChange={e => editSec(i(), { ...s, title: e.currentTarget.value })} />
              <span class="muted">{KINDS.find(x => x[0] === s.kind)?.[1]}</span><span class="grow" />
              <Show when={s.kind === "table"}><button class="btn sm" onClick={e => colMenu(e, i(), s)}>Columns ▾</button>
                <select value={s.filter ?? ""} onChange={e => editSec(i(), { ...s, filter: e.currentTarget.value })}><option value="">All tasks</option><For each={fields().filters.slice(1)}>{f => <option>{f.name}</option>}</For></select></Show>
              <button class="x" title="Move up" onClick={() => move(i(), -1)}>▲</button><button class="x" title="Move down" onClick={() => move(i(), 1)}>▼</button>
              <button class="x" title="Remove" onClick={() => put({ ...cur(), sections: cur().sections.filter((_, j) => j !== i()) })}>×</button>
            </div>
            <Show when={s.kind === "text"}><textarea class="noprint rtext" value={s.text ?? ""} placeholder="Type the text for this section" onChange={e => editSec(i(), { ...s, text: e.currentTarget.value })} /></Show>
            <SectionView s={s} unit={s.unit ?? unit()} rep={rep()} sc={sc()} curve={curve()} use={use()} />
          </div>
        )}</For>
      </Show>
    </section>
  );
};
export { D, Icon, Tools };
