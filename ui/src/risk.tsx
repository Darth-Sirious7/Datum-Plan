// Schedule Risk: a Monte Carlo analysis of the plan. The engine (crates/core/src/risk.rs) runs the
// plan many times with every unfinished task's duration drawn from its range and each open risk
// happening with its probability; this page shows when the project is likely to finish and why.
import { For, Show, createSignal } from "solid-js";
import { SimResult, fmtAt, money } from "./api";
import { call, P, setView, act } from "./store";
import { selectUid } from "./cmds";
import { Page } from "./views";
import { Kpi } from "./reports";
import { ChartBox, bar, line, COL } from "./charts";
import { fmtDay } from "./api";

const [res, setRes] = createSignal<SimResult | null>(null);
/** The last run is kept with the plan (project.ui.risk), so it is still there after reopening and prints. */
const saved = () => (P().ui?.risk?.result ? (P().ui.risk as { result: SimResult; at: string; opts: { iterations: number; low: number; high: number } }) : null);
const shown = () => res() ?? saved()?.result ?? null;
const [busy, setBusy] = createSignal(false);
const [opts, setOpts] = createSignal({ iterations: 1000, low: 10, high: 25, seed: 1 });

export const runRisk = async () => {
  setBusy(true);
  try {
    const r = await call<SimResult>("risk_run", { opts: opts() });
    if (r) { setRes(r); act({ Ui: { key: "risk", value: { result: r, at: new Date().toISOString().slice(0, 16).replace("T", " "), opts: opts() } } }); }
  } finally { setBusy(false); }
};

const pct = (v: number) => `${Math.round(v * 100)}%`;
const cur = (v: number) => money(v, { ...P().currency, decimals: 0 });

export const ScheduleRisk = () => {
  const num = (k: "iterations" | "low" | "high", v: string) => setOpts({ ...opts(), [k]: Math.max(0, +v || 0) });
  return (
    <Page title="Schedule Risk" help="simrisk" tools={<>
      <label class="muted">Runs <select value={opts().iterations} onChange={e => num("iterations", e.currentTarget.value)}><For each={[500, 1000, 2000, 5000]}>{n => <option value={n}>{n.toLocaleString()}</option>}</For></select></label>
      <label class="muted" title="Tasks without their own optimistic and pessimistic durations vary between these limits.">Default range −<input class="num" type="number" min="0" max="100" value={opts().low} onChange={e => num("low", e.currentTarget.value)} />% to +<input class="num" type="number" min="0" value={opts().high} onChange={e => num("high", e.currentTarget.value)} />%</label>
      <button class="btn pri" disabled={busy()} onClick={runRisk}>{busy() ? "Running…" : res() ? "Run again" : "Run analysis"}</button>
    </>}>
      <Show when={!res() && saved()}><p class="muted">Results of the run on {saved()!.at} ({saved()!.opts.iterations.toLocaleString()} runs, −{saved()!.opts.low}% to +{saved()!.opts.high}%). Run again after changing the plan.</p></Show>
      <Show when={shown()} fallback={
        <div class="panel"><p>Schedule Risk runs the plan {opts().iterations.toLocaleString()} times. In each run every unfinished task takes a duration between its <b>optimistic</b> and <b>pessimistic</b> duration (most often its planned one), and each open risk in the <a class="link" onClick={() => setView("risks")}>Risk Register</a> with a <b>Delay (days)</b> happens with its probability (1 = 10 %, 2 = 30 %, 3 = 50 %, 4 = 70 %, 5 = 90 %).</p>
          <p class="muted">Add the Optimistic Duration and Pessimistic Duration columns to the task sheet (right-click a column heading › Insert Column) to give tasks their own range. Tasks without one use the default range above.</p>
          <p><button class="btn pri" disabled={busy()} onClick={runRisk}>{busy() ? "Running…" : "Run analysis"}</button></p></div>
      }>{r => {
        const total = () => r().runs;
        let acc = 0;
        const cum = () => { acc = 0; return r().days.map(([, n]) => (acc += n) / total() * 100); };
        const mark = () => r().days.findIndex(([d]) => d >= Math.floor((r().planned - 1) / 1440));
        return <>
          <div class="kpis">
            <Kpi v={fmtAt(r().planned, true)} l={`Planned finish: ${pct(r().planned_chance)} chance`} bad={r().planned_chance < 0.5} good={r().planned_chance >= 0.8} />
            <Kpi v={fmtAt(r().p50, true)} l="P50: as likely early as late" />
            <Kpi v={fmtAt(r().p80, true)} l="P80: 8 in 10 runs finish by then" />
            <Kpi v={fmtAt(r().p90, true)} l="P90" />
            <Kpi v={cur(r().cost_p50)} l={`Cost P50 (plan ${cur(r().cost)})`} />
            <Kpi v={cur(r().cost_p80)} l="Cost P80" />
          </div>
          <div class="cols riskcharts">
            <div class="panel" style={{ "grid-column": "span 2" }}>
              <ChartBox title="Finish date: share of runs per day and cumulative" type="bar" fmt="pct" y="Runs" days={r().days.map(([d]) => d)} labels={r().days.map(([d]) => fmtDay(d))}
                datasets={[bar("Finished that day", r().days.map(([, n]) => n / total() * 100), COL.sky), line("Finished by then (cumulative)", cum(), COL.blue)]}
                marker={mark()} markerLabel="Planned" />
            </div>
            <div class="panel">
              <Show when={r().drivers.length} fallback={<p class="empty">No task's duration moves the finish much.</p>}>
                <ChartBox title="What drives the finish" type="bar" horizontal fmt="num" labels={r().drivers.map(d => d.name)} datasets={[bar("Correlation with the finish", r().drivers.map(d => Math.round(d.value * 100) / 100), COL.orange)]} />
              </Show>
              <p class="muted">The longer these tasks take, the later the project finishes (1 = always together).</p>
            </div>
          </div>
          <div class="cols">
            <div class="panel"><h2>Criticality: how often each task was critical</h2>
              <table class="tbl"><tbody><For each={r().criticality}>{t => <tr class="click" onClick={() => { setView("gantt"); selectUid(t.uid); }}><td>{t.name}</td><td class="r"><span class="meter"><i style={{ width: pct(t.value) }} /></span> {pct(t.value)}</td></tr>}</For></tbody></table>
            </div>
            <Show when={r().risks.length}><div class="panel"><h2>Risks in the analysis</h2>
              <table class="tbl"><tbody><For each={r().risks}>{t => <tr><td>R{t.uid} {t.name}</td><td class="r">happened in {pct(t.value)} of runs</td></tr>}</For></tbody></table>
            </div></Show>
            <div class="panel"><h2>About this run</h2><ul class="muted"><li>{r().runs.toLocaleString()} runs; the same settings give the same answer.</li><For each={r().notes}>{n => <li>{n}</li>}</For></ul></div>
          </div>
        </>;
      }}</Show>
    </Page>
  );
};
