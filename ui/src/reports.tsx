// Dashboards and reports in this order: KPIs → charts → tables → notes. Period reports compare
// this week or month with the previous one; S-curves for the project, phases, cost and hours.
import { CustomDashboard, DashTabs, dashK, dashboards } from "./dashboards";
import { For, Show, createSignal, createResource, createMemo, createEffect, on, JSX } from "solid-js";
import { createStore } from "solid-js/store";
import { Curve, Grid, Rule, fmtDay, money, dateDay, iso, SCurve, PeriodReport, PeriodNote, PRow, Usage, EarnedSchedule, fmtAt } from "./api";
import { call, out, P, tick, toast, act, openMenu } from "./store";
import { C, selectUid } from "./cmds";
import { Seg } from "./ui";
import { ChartBox, COL, line, bar } from "./charts";
import { Help } from "./help";

export const q = (cols: string[], rules: Rule[] = [], extra: object = {}) => ({
  cols, filter: { name: "", rules, summaries: false, highlight: false }, group: [], sort: [], flat: false, collapsed: [], level: 0, hide_summaries: false, project_summary: false, baseline: 0, ...extra,
});
export const r = (field: string, test: string, value = "", value2 = "", or = false): Rule => ({ field, test, value, value2, or });
export const useRows = (key: () => any, query: () => object) => createResource(() => [tick(), JSON.stringify(key())], () => call<Grid>("query", { q: query() }));
export const cum = (v: number[]) => { let s = 0; return v.map(x => (s += x)); };
export const pct = (v: number) => `${v.toFixed(1)}%`;
/** The bucket (first days in `b`) that holds `day`, for the status marker; null before the first. */
export const bucketAt = (b: number[], day: number) => { const k = b.findIndex(x => x > day); return k < 0 ? b.length - 1 : k > 0 ? k - 1 : null; };
export const label = (d: number, unit: string) => (unit === "month" ? fmtDay(d, "month") : fmtDay(d));
const WEIGHTS: [string, string][] = [["duration", "Duration"], ["cost", "Cost"], ["work", "Hours"], ["physical", "Physical %"]];

/** Report page header with logo and company, as printed. */
export const Brand = (p: { title: string; sub?: string }) => (
  <div class="brand">
    <Show when={P().logo}><img src={P().logo} alt="" /></Show>
    <div><h1>{p.title}</h1><div class="muted">{[P().name, P().company, P().client].filter(Boolean).join(" · ")}{p.sub ? ` · ${p.sub}` : ""}</div></div>
    <span class="grow" /><div class="muted r">Status date {out()!.summary.status || `not set (progress at ${out()!.summary.measured_at})`}<br />Printed {fmtDay(dateDay(new Date()), "long")}</div>
  </div>
);
export const Kpi = (p: { v: JSX.Element; l: string; d?: number; unit?: string; bad?: boolean; good?: boolean }) => (
  <div class="kpi" classList={{ bad: p.bad, good: p.good }}>
    <div class="v">{p.v}</div><div class="l">{p.l}</div>
    <Show when={p.d !== undefined && !isNaN(p.d!)}><div class="d" classList={{ up: p.d! > 0.005, down: p.d! < -0.005 }}>{p.d! > 0 ? "▲" : p.d! < 0 ? "▼" : "■"} {Math.abs(p.d!).toFixed(1)}{p.unit ?? ""} vs last period</div></Show>
  </div>
);
export const Table = (p: { g?: Grid; empty: string }) => (
  <Show when={p.g?.rows.length} fallback={<p class="empty">{p.empty}</p>}>
    <table class="tbl"><thead><tr><For each={p.g!.cols}>{c => <th classList={{ r: ["Cost", "Num", "Pct", "Work", "Dur"].includes(c.kind) }}>{c.name}</th>}</For></tr></thead>
      <tbody><For each={p.g!.rows.slice(0, 300)}>{row => <tr classList={{ sumr: row.kind !== "task", link: row.uid > 0 }} onClick={() => row.uid > 0 && selectUid(row.uid)}><For each={row.cells}>{(c, k) => <td classList={{ r: ["Cost", "Num", "Pct", "Work", "Dur"].includes(p.g!.cols[k()].kind), late: row.late && k() === 0 }}>{c}</td>}</For></tr>}</For></tbody></table>
  </Show>
);
export const Rows = (p: { rows: PRow[]; empty: string; note?: string }) => (
  <Show when={p.rows.length} fallback={<p class="empty">{p.empty}</p>}>
    <table class="tbl"><thead><tr><th>Activity ID</th><th>Task</th><th>Start</th><th>Finish</th><th class="r">Complete</th><Show when={p.note}><th>{p.note}</th></Show></tr></thead>
      <tbody><For each={p.rows.slice(0, 300)}>{x => <tr><td>{x.code}</td><td>{x.name}</td><td>{x.start}</td><td>{x.finish}</td><td class="r">{x.pct}%</td><Show when={p.note}><td>{x.note}</td></Show></tr>}</For></tbody></table>
  </Show>
);
/** Earned schedule: when the plan expected the progress made so far, and the finish that implies. */
export const EsKpis = () => {
  const [es] = createResource(() => tick(), () => call<EarnedSchedule | null>("earned_schedule"));
  const d1 = (v: number) => (Math.round(v * 10) / 10).toLocaleString();
  return (
    <Show when={es()}>{e => <>
      <Kpi v={e().spi_t.toFixed(2)} l="SPI(t): schedule performance in time" bad={e().spi_t < 0.95} good={e().spi_t >= 1} />
      <Kpi v={`${e().sv_t > 0 ? "+" : ""}${d1(e().sv_t)} d`} l={`Earned schedule ${d1(e().es)} of ${d1(e().at)} days`} bad={e().sv_t < -0.5} good={e().sv_t >= 0} />
      <Kpi v={fmtAt(e().forecast_finish, true)} l={`Finish at this pace (plan ${fmtAt(e().planned_finish, true)})`} bad={e().forecast_finish > e().planned_finish} />
    </>}</Show>
  );
};

export const Tools = (p: { title: string; help?: string; children?: JSX.Element }) => (
  <div class="ptools noprint">{p.help && <Help k={p.help} />}{p.children}<span class="grow" /><button class="btn" onClick={() => C.print()}>Print / PDF</button><button class="btn" onClick={() => saveHtml(p.title)}>Save as HTML</button></div>
);

/** S-curve chart from engine data: planned, actual and forecast cumulative %. */
export const SCurveChart = (p: { c: SCurve; unit: string; title?: string; size?: "tall" | "short" | "fill"; money?: boolean; bare?: boolean }) => {
  const k = createMemo(() => p.c.days.findIndex(d => d >= p.c.status_day));
  const conv = (v: (number | null)[]) => (p.money ? v.map(x => (x === null ? null : (x / 100) * p.c.total)) : v);
  return <ChartBox title={p.title ?? `S-curve: ${p.c.name}`} type="line" size={p.size} days={p.c.days} labels={p.c.days.map(d => label(d, p.unit))} fmt={p.money ? "money" : "pct"} max={p.money ? undefined : 100}
    y={p.money ? `Cumulative (${P().currency.code})` : "Cumulative progress"} marker={k() >= 0 ? k() : null} bare={p.bare}
    datasets={[line("Planned (baseline)", conv(p.c.planned), COL.grey, [6, 4]), line("Actual", conv(p.c.actual), COL.blue, undefined, true), line("Forecast", conv(p.c.forecast.map((v, i) => (i >= k() && k() >= 0 ? v : null))), COL.red, [2, 3])]} />;
};
/** Progress per period (not cumulative): planned vs actual bars. */
export const PeriodBars = (p: { c: SCurve; unit: string; last?: number; size?: "fill"; bare?: boolean }) => {
  const n = () => p.c.days.length;
  const from = () => Math.max(1, n() - (p.last ?? 24));
  const delta = (v: (number | null)[]) => v.slice(from()).map((x, i) => (x === null || v[from() + i - 1] === null ? null : x - (v[from() + i - 1] as number)));
  return <ChartBox title={`Progress per ${p.unit}`} type="bar" fmt="pct" y="Progress in period" size={p.size} bare={p.bare} days={p.c.days.slice(from())} labels={p.c.days.slice(from()).map(d => label(d, p.unit))}
    datasets={[bar("Planned", delta(p.c.planned), COL.grey), bar("Actual", delta(p.c.actual), COL.blue)]} />;
};

// ---------- dashboard ----------
/** The Dashboard page: the built-in project dashboard, or one of the plan's own (tabs on top). */
export const Dashboard = () => (
  <section class="page report">
    <Tools title="Dashboard" help="dashboard"><Show when={!(dashK() >= 0 && dashboards()[dashK()])}><button class="btn" title="Choose the cards on this dashboard" onClick={e => cardsMenu()?.(e)}>Cards ▾</button></Show></Tools>
    <DashTabs />
    <Show when={dashK() >= 0 && dashboards()[dashK()]} fallback={<ProjectDashboard />}><CustomDashboard k={dashK()} /></Show>
  </section>
);
const [cardsMenu, setCardsMenu] = createSignal<((e: MouseEvent) => void) | null>(null);
/** The project dashboard. A card shows when the plan has data for it (no costs: no cost cards), unless the
 *  user chose otherwise in Cards ▾ (kept with the plan); a card chosen without data stays blank, also in print. */
const ProjectDashboard = () => {
  const s = () => out()!.summary;
  const unit = () => (s().f - s().s > 240 ? "month" : "week");
  const [sc] = createResource(() => [tick(), P().periods.weight], () => call<SCurve>("scurve", { unit: unit(), how: P().periods.weight, scope: null }));
  const [rep] = createResource(() => tick(), () => call<PeriodReport>("period_report", { kind: unit(), end: null }));
  const [curve] = createResource(() => tick(), () => call<Curve>("curve", { unit: unit() }));
  const [use] = createResource(() => tick(), () => call<Usage>("usage", { by: "resource", what: "work", unit: unit() }));
  const [late] = useRows(() => 2, () => q(["code", "name", "finish", "pct", "res"], [r("late", "equals", "Yes"), r("summary", "equals", "No")]));
  const [ms] = useRows(() => 3, () => q(["code", "name", "finish", "b0finish", "vfinish", "pct"], [r("milestone", "equals", "Yes")]));
  const some = (v?: number[]) => !!v?.some(x => x);
  const cost = () => !!curve() && (some(curve()!.planned) || some(curve()!.actual) || some(curve()!.forecast));
  const CARDS: [string, string, () => boolean][] = [
    ["actual", "Actual progress", () => true], ["planned", "Planned progress", () => !!rep()], ["variance", "Variance", () => !!rep()],
    ["finish", "Forecast finish", () => true], ["spi", "SPI", () => s().spi !== null], ["es", "Earned schedule", () => s().spi !== null],
    ["cpi", "CPI", () => s().cpi !== null], ["cost", "Forecast cost", cost], ["late", "Late tasks", () => true], ["done", "Tasks complete", () => true], ["critical", "Critical tasks", () => true],
    ["scurve", "Project S-curve", () => true], ["period", "Progress per period", () => true], ["phase", "Progress by phase", () => !!rep()?.wbs.length],
    ["evm", "Cost and earned value", cost], ["manpower", "Manpower histogram", () => !!use()?.rows.some(x => some(x.vals))],
    ["milestones", "Milestones", () => s().milestones > 0], ["latelist", "Late tasks list", () => true]];
  const pick = () => (P().ui?.dashcards ?? {}) as Record<string, boolean>;
  const on = (k: string) => pick()[k] ?? CARDS.find(c => c[0] === k)![2]();
  setCardsMenu(() => (e: MouseEvent) => openMenu(e, [...CARDS.map(([k, l]) => ({ label: l, on: on(k), run: () => act({ Ui: { key: "dashcards", value: { ...pick(), [k]: !on(k) } } }) })), "-",
    { label: "Show the cards that have data", run: () => act({ Ui: { key: "dashcards", value: {} } }) }]));
  const scurve = () => on("scurve") && sc();
  return (
    <>
      <Brand title="Project Dashboard" />
      <Show when={s().error}><div class="panel late">{s().error}</div></Show>
      <div class="kpis">
        <Show when={on("actual")}><Kpi v={pct(rep()?.actual ?? s().pct)} l={s().measured_at ? `Actual progress (no status date: at ${s().measured_at})` : "Actual progress"} d={rep() ? rep()!.actual - rep()!.actual_prev : undefined} unit="%" /></Show>
        <Show when={on("planned")}><Kpi v={pct(rep()?.planned ?? 0)} l="Planned progress" d={rep() ? rep()!.planned - rep()!.planned_prev : undefined} unit="%" /></Show>
        <Show when={on("variance")}><Kpi v={rep() ? `${(rep()!.actual - rep()!.planned).toFixed(1)}%` : ""} l="Variance (actual − planned)" bad={!!rep() && rep()!.actual < rep()!.planned - 0.5} good={!!rep() && rep()!.actual >= rep()!.planned} /></Show>
        <Show when={on("finish")}><Kpi v={s().finish} l={`Forecast finish${s().var_finish ? ` (${s().var_finish} vs baseline)` : ""}`} /></Show>
        <Show when={on("spi")}><Kpi v={s().spi?.toFixed(2) ?? ""} l="SPI" bad={s().spi !== null && s().spi! < 0.95} good={s().spi !== null && s().spi! >= 1} /></Show>
        <Show when={on("es")}><EsKpis /></Show>
        <Show when={on("cpi")}><Kpi v={s().cpi?.toFixed(2) ?? ""} l="CPI" bad={s().cpi !== null && s().cpi! < 0.95} good={s().cpi !== null && s().cpi! >= 1} /></Show>
        <Show when={on("cost")}><Kpi v={cost() ? s().cost : ""} l="Forecast cost" /></Show>
        <Show when={on("late")}><Kpi v={s().late} l="Late tasks" bad={s().late > 0} /></Show>
        <Show when={on("done")}><Kpi v={`${s().done} / ${s().tasks}`} l="Tasks complete" /></Show>
        <Show when={on("critical")}><Kpi v={s().critical} l="Critical tasks" /></Show>
      </div>
      <Show when={scurve()}>{c => <div class="charts wide"><SCurveChart c={c()} unit={unit()} title="Project S-curve" size="tall" /></div>}</Show>
      <div class="charts flow">
        <Show when={on("period") && sc()}>{c => <PeriodBars c={c()} unit={unit()} />}</Show>
        <Show when={on("phase") && rep()}>{x => <ChartBox title="Progress by phase" type="bar" horizontal fmt="pct" max={100} y="Complete" labels={x().wbs.map(w => w.name)}
          datasets={[bar("Planned", x().wbs.map(w => w.planned), COL.grey), bar("Actual", x().wbs.map(w => w.actual), COL.blue)]} />}</Show>
        <Show when={on("evm") && curve()}>{c => <ChartBox title="Cost and earned value" type="line" fmt="money" y={`Cumulative (${P().currency.code})`} days={c().buckets} labels={c().buckets.map(d => label(d, unit()))}
          marker={bucketAt(c().buckets, c().status_day)}
          datasets={[line("Planned value", cum(c().planned), COL.grey, [6, 4]), line("Earned value", cum(c().earned), COL.green), line("Actual cost", cum(c().actual), COL.red), line("Forecast", cum(c().forecast), COL.blue, [2, 3])]} />}</Show>
        <Show when={on("manpower") && use()}>{u => <ChartBox title="Manpower histogram" type="bar" fmt="hours" y="Hours" stacked days={u().buckets} labels={u().buckets.map(d => label(d, unit()))}
          datasets={u().rows.filter(x => x.of === null).slice(0, 8).map((x, i) => bar(x.name, x.vals, Object.values(COL)[i % 8], "h"))} />}</Show>
      </div>
      <div class="cols">
        <Show when={on("milestones")}><div class="panel"><h2>Milestones</h2><Table g={ms()} empty="No milestones." /></div></Show>
        <Show when={on("latelist")}><div class="panel"><h2>Late tasks</h2><Table g={late()} empty="Nothing is late." /></div></Show>
      </div>
    </>
  );
};

// ---------- S-curves ----------
// The choices on these pages live outside them: the printed copy is another instance and must show the same.
const [unitPick, setUnit] = createSignal<string | null>(null), [howPick, setHow] = createSignal<string | null>(null);
export const SCurves = () => {
  const s = () => out()!.summary;
  const unit = () => unitPick() ?? (s().f - s().s > 240 ? "month" : "week"), how = () => howPick() ?? (P().periods.weight || "duration");
  const [phases] = useRows(() => "top", () => q(["name"], [r("level", "equals", "1"), r("summary", "equals", "Yes")]));
  const [main] = createResource(() => [tick(), unit(), how()], () => call<SCurve>("scurve", { unit: unit(), how: how(), scope: null }));
  const [each] = createResource(() => [tick(), unit(), how(), phases()?.rows.length], async () => Promise.all((phases()?.rows ?? []).slice(0, 12).map(x => call<SCurve>("scurve", { unit: unit(), how: how(), scope: x.uid }))));
  const [costc] = createResource(() => [tick(), unit()], () => call<SCurve>("scurve", { unit: unit(), how: "cost", scope: null }));
  const [hours] = createResource(() => [tick(), unit()], () => call<SCurve>("scurve", { unit: unit(), how: "work", scope: null }));
  return (
    <section class="page report">
      <Tools title="S-Curves" help="scurves"><Seg options={[["week", "Weekly"], ["month", "Monthly"]]} value={unit()} set={setUnit} /><Seg options={WEIGHTS} value={how()} set={setHow} /></Tools>
      <Brand title="S-Curves" sub={`Weighted by ${WEIGHTS.find(w => w[0] === how())?.[1].toLowerCase()}`} />
      <div class="kpis"><EsKpis /></div>
      <Show when={P().snapshots.length}><p class="muted">Actual progress follows the {P().snapshots.length} stored period{P().snapshots.length === 1 ? "" : "s"} (Close Period), then runs to today's status.</p></Show>
      <div class="charts">
        <Show when={main()}>{c => <SCurveChart c={c()} unit={unit()} title="Project progress" size="tall" />}</Show>
        <Show when={main()}>{c => <PeriodBars c={c()} unit={unit()} />}</Show>
        <Show when={costc()?.weight === "cost" && costc()}>{c => <SCurveChart c={c()} unit={unit()} title="Cost S-curve" money />}</Show>
        <Show when={hours()?.weight === "work" && hours()}>{c => <ChartBox title="Hours S-curve" type="line" fmt="hours" y="Cumulative hours" days={c().days} labels={c().days.map(d => label(d, unit()))}
          datasets={[line("Planned", c().planned.map(v => (v / 100) * c().total), COL.grey, [6, 4]), line("Actual", c().actual.map(v => (v === null ? null : (v / 100) * c().total)), COL.blue, undefined, true), line("Forecast", c().forecast.map(v => (v / 100) * c().total), COL.red, [2, 3])]} />}</Show>
        <For each={each() ?? []}>{c => <Show when={c}><SCurveChart c={c!} unit={unit()} size="short" /></Show>}</For>
      </div>
      <Show when={main()}>{c => (
        <div class="panel"><h2>Data</h2><div class="wlwrap"><table class="tbl"><thead><tr><th>Period end</th><th class="r">Planned</th><th class="r">Actual</th><th class="r">Forecast</th><th class="r">Variance</th></tr></thead>
          <tbody><For each={c().days.slice(1)}>{(d, i) => { const k = i() + 1, a = c().actual[k]; return <tr><td>{fmtDay(d, "long")}</td><td class="r">{pct(c().planned[k])}</td><td class="r">{a === null ? "" : pct(a)}</td><td class="r">{pct(c().forecast[k])}</td><td class="r" classList={{ late: a !== null && a < c().planned[k] - 0.05 }}>{a === null ? "" : pct(a - c().planned[k])}</td></tr>; }}</For></tbody></table></div></div>
      )}</Show>
    </section>
  );
};

// ---------- period progress report (weekly / monthly) ----------
const shrink = (file: File): Promise<string> => new Promise(ok => {
  const img = new Image();
  img.onload = () => { const k = Math.min(1, 1400 / Math.max(img.width, img.height)); const cv = document.createElement("canvas"); cv.width = img.width * k; cv.height = img.height * k; cv.getContext("2d")!.drawImage(img, 0, 0, cv.width, cv.height); ok(cv.toDataURL("image/jpeg", 0.82)); URL.revokeObjectURL(img.src); };
  img.src = URL.createObjectURL(file);
});
const [kind, setKind] = createSignal("week"), [end, setEnd] = createSignal<number | null>(null);
export const Progress = () => {
  const [ends] = createResource(() => [tick(), kind()], () => call<number[]>("periods", { kind: kind() }));
  const [rep] = createResource(() => [tick(), kind(), end()], () => call<PeriodReport>("period_report", { kind: kind(), end: end() }));
  const [sc] = createResource(() => [tick(), kind(), P().periods.weight], () => call<SCurve>("scurve", { unit: kind(), how: P().periods.weight, scope: null }));
  const blank = (): PeriodNote => ({ kind: kind(), end: rep() ? iso(rep()!.to) : null, summary: "", progress: "", issues: "", actions: "", look_ahead: "", photos: [] });
  const note = createMemo(() => (rep() ? P().notes.find(n => n.kind === kind() && n.end === iso(rep()!.to)) : undefined));
  const [draft, setDraft] = createStore<PeriodNote>(blank());
  const reload = () => setDraft(JSON.parse(JSON.stringify(note() ?? blank())));
  createEffect(on(() => [rep(), note()], reload));
  const saveNote = () => act({ Note: { note: { ...JSON.parse(JSON.stringify(draft)), kind: kind(), end: rep() ? iso(rep()!.to) : null } } }).then(ok => ok && toast("Notes saved with the plan.", true));
  const addPhotos = async (files: FileList | null) => {
    if (!files) return;
    const list = Array.from(files), datas = await Promise.all(list.map(shrink));
    setDraft("photos", ph => [...ph, ...datas.map((d, i) => ({ data: d, caption: list[i].name.replace(/\.[^.]+$/, "") }))]);
  };
  const name = () => (kind() === "week" ? "Weekly" : "Monthly");
  const sec = (k: keyof PeriodNote, l: string, ph: string) => <div class="notesec"><label>{l}</label><textarea class="noprint" value={draft[k] as string} placeholder={ph} onInput={e => setDraft(k as "summary", e.currentTarget.value)} /><div class="notes-print printonly">{draft[k] as string}</div></div>;
  return (
    <section class="page report">
      <Tools title={`${name()} progress report`} help="period">
        <Seg options={[["week", "Weekly"], ["month", "Monthly"]]} value={kind()} set={k => { setKind(k); setEnd(null); }} />
        <select onChange={e => setEnd(e.currentTarget.value ? +e.currentTarget.value : null)}>
          <option value="" selected={end() === null}>Current period</option><For each={(ends() ?? []).slice().reverse()}>{d => <option value={d} selected={d === end()}>Period ending {fmtDay(d, "long")}</option>}</For>
        </select>
        <button class="btn" onClick={() => import("./dialogs").then(d => d.periodSettings())}>Period settings</button>
      </Tools>
      <Show when={rep()}>{x => <>
        <Brand title={`${name()} Progress Report`} sub={`${fmtDay(x().from, "long")} – ${fmtDay(x().to, "long")}`} />
        <div class="kpis">
          <Kpi v={pct(x().actual)} l="Actual progress (cumulative)" d={x().actual - x().actual_prev} unit="%" />
          <Kpi v={pct(x().planned)} l="Planned progress (cumulative)" d={x().planned - x().planned_prev} unit="%" />
          <Kpi v={pct(x().actual - x().actual_prev)} l={`Progress this ${kind()}`} good={x().actual - x().actual_prev >= x().planned - x().planned_prev} bad={x().actual - x().actual_prev < x().planned - x().planned_prev - 0.5} />
          <Kpi v={pct(x().planned - x().planned_prev)} l={`Planned this ${kind()}`} />
          <Kpi v={pct(x().actual - x().planned)} l="Variance (cumulative)" bad={x().actual < x().planned - 0.5} good={x().actual >= x().planned} />
          <Kpi v={x().finish} l={`Forecast finish${x().finish_var ? ` (${x().finish_var})` : ""}`} />
          <Kpi v={x().spi?.toFixed(2) ?? "—"} l="SPI" bad={(x().spi ?? 1) < 0.95} /><Kpi v={x().cpi?.toFixed(2) ?? "—"} l="CPI" bad={(x().cpi ?? 1) < 0.95} />
          <Kpi v={x().completed.length} l={`Finished this ${kind()}`} /><Kpi v={x().late} l="Late tasks" bad={x().late > 0} />
        </div>
        <div class="charts">
          <Show when={sc()}>{c => <SCurveChart c={c()} unit={kind()} title="Cumulative progress" size="tall" />}</Show>
          <Show when={sc()}>{c => <PeriodBars c={c()} unit={kind()} last={16} />}</Show>
          <ChartBox title={`Progress by phase: last ${kind()} vs this ${kind()}`} type="bar" horizontal fmt="pct" max={100} labels={x().wbs.map(w => w.name)}
            datasets={[bar("Actual (previous)", x().wbs.map(w => w.actual_prev), COL.sky), bar("Actual (this period)", x().wbs.map(w => w.actual), COL.blue), bar("Planned", x().wbs.map(w => w.planned), COL.grey)]} />
          <ChartBox title="Planned hours per period" type="bar" fmt="hours" days={x().hours.slice(-16).map(h => h[0])} labels={x().hours.slice(-16).map(h => label(h[0], kind()))} marker={x().hours.slice(-16).findIndex(h => h[0] >= x().to)}
            datasets={[bar("Hours", x().hours.slice(-16).map(h => h[1]), COL.orange)]} />
        </div>
        <div class="panel"><h2>Progress by phase</h2>
          <table class="tbl"><thead><tr><th>Phase</th><th class="r">Planned prev.</th><th class="r">Planned now</th><th class="r">Actual prev.</th><th class="r">Actual now</th><th class="r">This period</th><th class="r">Variance</th></tr></thead>
            <tbody><For each={x().wbs}>{w => <tr><td>{w.name}</td><td class="r">{pct(w.planned_prev)}</td><td class="r">{pct(w.planned)}</td><td class="r">{pct(w.actual_prev)}</td><td class="r">{pct(w.actual)}</td>
              <td class="r"><span class="delta" classList={{ up: w.actual - w.actual_prev >= w.planned - w.planned_prev, down: w.actual - w.actual_prev < w.planned - w.planned_prev - 0.05 }}>{pct(w.actual - w.actual_prev)}</span></td>
              <td class="r" classList={{ late: w.actual < w.planned - 0.05 }}>{pct(w.actual - w.planned)}</td></tr>}</For></tbody></table></div>
        <div class="cols">
          <div class="panel"><h2>Completed this {kind()}</h2><Rows rows={x().completed} empty="Nothing finished in this period." /></div>
          <div class="panel"><h2>Started this {kind()}</h2><Rows rows={x().started} empty="Nothing started in this period." /></div>
          <div class="panel"><h2>In progress</h2><Rows rows={x().in_progress} empty="No other tasks in progress." /></div>
          <div class="panel"><h2>Behind baseline</h2><Rows rows={x().slipped} note="Baseline" empty="Nothing is behind its baseline." /></div>
          <div class="panel"><h2>Look-ahead: next {kind()}</h2><Rows rows={x().upcoming} empty="Nothing starts in the next period." /></div>
          <div class="panel"><h2>Milestones</h2><Rows rows={x().milestones} note="Variance" empty="No milestones." /></div>
        </div>
        <div class="panel"><h2>Notes</h2>
          {sec("summary", "Executive summary", "The headline of this period: overall status, key achievements, main concerns.")}
          {sec("progress", "Progress narrative", "What was done this period by area or phase.")}
          {sec("issues", "Issues and risks", "Delays, constraints, risks and their impact.")}
          {sec("actions", "Actions and decisions", "Who does what by when.")}
          {sec("look_ahead", "Look-ahead", "Plan for the next period.")}
          <div class="notesec"><label>Photos</label>
            <div class="photos"><For each={draft.photos}>{(ph, i) => <figure><img src={ph.data} alt={ph.caption} /><figcaption><input value={ph.caption} onInput={e => setDraft("photos", i(), "caption", e.currentTarget.value)} /><button class="x noprint" onClick={() => setDraft("photos", v => v.filter((_, j) => j !== i()))}>×</button></figcaption></figure>}</For></div>
            <label class="btn noprint" style={{ width: "fit-content" }}>Add photos<input type="file" accept="image/*" multiple hidden onChange={e => addPhotos(e.currentTarget.files)} /></label>
          </div>
          <div class="ptools noprint"><button class="btn pri" onClick={saveNote}>Save notes</button><button class="btn" onClick={reload}>Discard changes</button><span class="muted">Notes and photos are saved inside the plan, one set per period.</span></div>
        </div>
      </>}</Show>
    </section>
  );
};

// ---------- reports gallery ----------
const REPORTS: [string, string, string][] = [
  ["dashboard", "Dashboard", "KPIs with change vs last period, S-curve, phases, cost, manpower."],
  ["progress", "Weekly / Monthly Progress", "This period vs last: progress, variance, completed, started, look-ahead, notes and photos."],
  ["scurve", "S-Curves", "Progress, cost and hours S-curves for the project and each phase."],
  ["ev", "Earned Value", "Planned, earned and actual cost over time, SPI, CPI, EAC and the EV table."],
  ["cost", "Cost Overview", "Cash flow per period, cumulative cost and cost by phase."],
  ["work", "Work Overview", "Hours per resource and overallocation."],
  ["critical", "Critical Tasks", "The tasks that drive the finish date."],
  ["late", "Late & Slipping Tasks", "Late tasks and tasks behind their baseline."],
  ["milestones", "Milestone Report", "Every milestone with baseline and variance."],
  ["trend", "Trend (Snapshots)", "Finish date, complete and cost across stored snapshots."],
];
const [pick, setPick] = createSignal<string | null>(null);
export const Reports = () => {
  return (
    <Show when={pick()} fallback={
      <section class="page"><div class="ptools"><h1>Reports <Help k="reports" /></h1></div>
        <div class="tiles"><For each={REPORTS}>{([k, n, d]) => <button class="tile" onClick={() => (["dashboard", "progress", "scurve"].includes(k) ? C.view(k as "dashboard") : setPick(k))}><div class="pic"><svg class="ic"><use href="#i-report" /></svg></div><b>{n}</b><span>{d}</span></button>}</For></div>
      </section>}>
      {k => <Report kind={k()} back={() => setPick(null)} />}
    </Show>
  );
};

const Report = (p: { kind: string; back: () => void }) => {
  const s = () => out()!.summary;
  const name = () => REPORTS.find(x => x[0] === p.kind)![1];
  const unit = () => (s().f - s().s > 240 ? "month" : "week");
  const [curve] = createResource(() => [tick(), p.kind], () => call<Curve>("curve", { unit: unit() }));
  const cols: Record<string, string[]> = {
    critical: ["code", "name", "dur", "start", "finish", "tf", "res"], late: ["code", "name", "finish", "b0finish", "vfinish", "pct", "res"],
    milestones: ["code", "name", "finish", "b0finish", "vfinish", "pct"],
    ev: ["name", "bcws", "bcwp", "acwp", "sv", "cv", "spi", "cpi", "bac", "eac", "vac"], cost: ["name", "cost", "b0cost", "vcost", "acost", "rcost"],
    work: ["name", "work", "b0work", "awork", "rwork", "workpct"],
  };
  const rules = (): Rule[] => ({
    critical: [r("critical", "equals", "Yes"), r("summary", "equals", "No")], late: [r("late", "equals", "Yes"), r("vfinish", "is greater than", "0", "", true)],
    milestones: [r("milestone", "equals", "Yes")], ev: [r("level", "is less than or equal to", "2")], cost: [r("level", "is less than or equal to", "2")], work: [r("level", "is less than or equal to", "2")],
  } as Record<string, Rule[]>)[p.kind] ?? [];
  const [g] = useRows(() => p.kind, () => q(cols[p.kind] ?? ["name"], rules()));
  const [res] = createResource(() => [tick(), p.kind], () => (p.kind === "work" ? call<[number, boolean, string[]][]>("resources", { cols: ["name", "work", "cost"] }) : undefined));
  return (
    <section class="page report">
      <Tools title={name()}><button class="btn" onClick={p.back}>‹ All reports</button></Tools>
      <Brand title={name()} />
      <div class="kpis">
        <Kpi v={`${Math.round(s().pct)}%`} l="Complete" /><Kpi v={s().finish} l="Finish" /><Kpi v={s().var_finish || "—"} l="Finish variance" />
        <Kpi v={s().cost} l="Cost" /><Kpi v={s().spi?.toFixed(2) ?? "—"} l="SPI" bad={(s().spi ?? 1) < 0.95} /><Kpi v={s().cpi?.toFixed(2) ?? "—"} l="CPI" bad={(s().cpi ?? 1) < 0.95} /><Kpi v={s().late} l="Late tasks" bad={s().late > 0} />
      </div>
      <Show when={(p.kind === "ev" || p.kind === "cost") && curve()}>{cv => {
        const c = cv();
        const k = bucketAt(c.buckets, c.status_day);
        return <div class="charts">
          <ChartBox title={p.kind === "ev" ? "Earned value over time" : "Cumulative cost"} type="line" fmt="money" size="tall" days={c.buckets} labels={c.buckets.map(d => label(d, unit()))} marker={k}
            datasets={p.kind === "ev" ? [line("Planned value (PV)", cum(c.planned), COL.grey, [6, 4]), line("Earned value (EV)", cum(c.earned), COL.green), line("Actual cost (AC)", cum(c.actual), COL.red)]
              : [line("Baseline", cum(c.planned), COL.grey, [6, 4]), line("Forecast", cum(c.forecast), COL.blue, undefined, true)]} />
          <ChartBox title="Cash flow per period" type="bar" fmt="money" days={c.buckets} labels={c.buckets.map(d => label(d, unit()))} marker={k}
            datasets={[bar("Baseline", c.planned, COL.grey), bar("Forecast", c.forecast, COL.blue), bar("Actual", c.actual, COL.red)]} />
        </div>;
      }}</Show>
      <Show when={p.kind === "work" && res()}>{x => <ChartBox title="Hours per resource" type="bar" horizontal fmt="hours" labels={x().map(v => v[2][0])} datasets={[bar("Hours", x().map(v => parseFloat(v[2][1].replace(/,/g, "")) || 0), COL.orange)]} />}</Show>
      <Show when={p.kind === "trend"}>
        <Show when={P().snapshots.length} fallback={<div class="panel"><p class="empty">No snapshots yet. Use Project › Store Snapshot each reporting period.</p></div>}>
          <div class="charts">
            <ChartBox title="Complete at each snapshot" type="line" fmt="pct" max={100} labels={P().snapshots.map(x => x.date ?? "")} datasets={[line("% complete", P().snapshots.map(x => x.pct), COL.blue)]} />
            <ChartBox title="Forecast finish at each snapshot (days from first)" type="line" labels={P().snapshots.map(x => x.date ?? "")} datasets={[line("Finish slip (days)", P().snapshots.map(x => (x.finish - P().snapshots[0].finish) / 1440), COL.red)]} />
          </div>
          <div class="panel"><table class="tbl"><thead><tr><th>Date</th><th>Label</th><th>Finish</th><th class="r">Complete</th><th class="r">Cost</th><th>Comment</th></tr></thead>
            <tbody><For each={P().snapshots}>{x => <tr><td>{x.date}</td><td>{x.label}</td><td>{fmtDay(x.finish / 1440 - 0.01, "long")}</td><td class="r">{Math.round(x.pct)}%</td><td class="r">{money(x.cost, P().currency)}</td><td>{x.comment}</td></tr>}</For></tbody></table></div>
        </Show>
      </Show>
      <Show when={cols[p.kind]}><div class="panel"><h2>{name()}</h2><Table g={g()} empty="No tasks match." /></div></Show>
    </section>
  );
};

/** Saves the visible report as a standalone HTML file; charts become pictures. */
export const saveHtml = async (title: string) => {
  const el = document.querySelector(".report") as HTMLElement | null;
  if (!el) return;
  const copy = el.cloneNode(true) as HTMLElement;
  const live = el.querySelectorAll("canvas"), dead = copy.querySelectorAll("canvas");
  live.forEach((cv, i) => { const img = document.createElement("img"); img.src = cv.toDataURL("image/png"); img.style.width = "100%"; dead[i].replaceWith(img); });
  copy.querySelectorAll("textarea").forEach(t => t.remove());
  const css = [...document.styleSheets].flatMap(s => { try { return [...s.cssRules].map(r => r.cssText).filter(t => !t.startsWith("@font-face")); } catch { return []; } }).join("\n");
  const html = `<!doctype html><html data-theme="light"><head><meta charset="utf-8"><title>${title.replace(/[&<]/g, c => (c === "&" ? "&amp;" : "&lt;"))}</title><style>${css}\n.noprint{display:none!important}.printonly{display:block!important}html,body{overflow:auto;height:auto}</style></head><body>${copy.outerHTML}</body></html>`;
  const path = await call<string | null>("save_file", { name: `${P().name} - ${title}.html`, data: html });
  if (path) toast(`Saved ${path}`, true);
};
