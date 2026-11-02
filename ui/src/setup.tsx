// Project Setup: a skippable step-by-step page (sidebar › Setup), shown first for new plans.
import { For, Show, createSignal } from "solid-js";
import { act, call, grid, P, refresh, setSetupOpen, toast, opts } from "./store";
import * as D from "./dialogs";
import { CurrencyPick } from "./backstage";
import { Help } from "./help";
import { Library } from "./pictures";

const STEPS = ["Project", "Calendar", "Currency", "WBS", "Resources", "Logos", "Done"];

export const Setup = () => {
  const [step, setStep] = createSignal(0);
  const p = () => P();
  const put = (field: string, value: any) => act({ ProjectSet: { field, value } });
  const [wbs, setWbs] = createSignal("Phase 1\n  Design\n  Approvals\nPhase 2\n  Build\n  Test\nHandover");
  const [res, setRes] = createSignal("");
  const finish = () => { put("setup_done", true); setSetupOpen(false); toast("Project set up. Add tasks in the Gantt chart.", true); };
  const addWbs = async () => {
    const lines = wbs().split("\n").filter(l => l.trim());
    if (!lines.length) return;
    const lv = lines.map(l => Math.floor((l.match(/^\s*/)![0].replace(/\t/g, "  ").length) / 2));
    const at = grid().total;
    if (!(await act({ InsertRows: { at, rows: lines.map(l => [["name", l.trim()]]) } }))) return;
    await refresh();
    const rows = grid().rows.filter(r => r.uid > 0 && r.id > at).sort((a, b) => a.id - b.id);
    for (let L = 1; L <= Math.max(...lv); L++) {
      const u = rows.filter((_, k) => lv[k] >= L).map(r => r.uid);
      if (u.length) await act({ Indent: { uids: u } }, true);
    }
    toast(`${lines.length} WBS rows added.`, true);
    setStep(step() + 1);
  };
  const addRes = async () => {
    const names = res().split(/[\n,]/).map(x => x.trim()).filter(Boolean);
    for (const n of names) await act({ ResInsert: { name: n } }, true);
    if (names.length) toast(`${names.length} resources added.`, true);
    setRes(""); setStep(step() + 1);
  };
  return (
    <aside class="setup">
      <header><b>Project Setup</b><Help k="setup" /><button class="x" title="Close (reopen it from the Setup icon in the sidebar)" onClick={() => setSetupOpen(false)}>×</button></header>
      <ol class="steps"><For each={STEPS}>{(s, k) => <li classList={{ on: k() === step(), done: k() < step() }} onClick={() => setStep(k())}>{s}</li>}</For></ol>
      <div class="sbody">
        <Show when={step() === 0}>
          <p class="muted">Tell Datum Plan about the project. Everything can be changed later in Project Information.</p>
          <label>Project name<input value={p().name} onChange={e => act({ Project: { name: e.currentTarget.value, start: p().start, status: p().status ?? "", calendar: p().calendar, day_hours: p().day_min / 60, manual_default: p().manual_default } })} /></label>
          <label>Project code<input value={p().code} onChange={e => put("code", e.currentTarget.value)} /></label>
          <label>Start date<input type="date" value={p().start} onChange={e => act({ Project: { name: p().name, start: e.currentTarget.value, status: p().status ?? "", calendar: p().calendar, day_hours: p().day_min / 60, manual_default: p().manual_default } })} /></label>
          <label>Client<input value={p().client} onChange={e => put("client", e.currentTarget.value)} /></label>
          <label>Project manager<input value={p().manager} onChange={e => put("manager", e.currentTarget.value)} /></label>
          <label>Location<input value={p().location} onChange={e => put("location", e.currentTarget.value)} /></label>
          <label>Activity IDs<span class="inline"><input value={p().id_prefix} style={{ width: "50px" }} onChange={e => put("id_prefix", e.currentTarget.value)} /> starting at <input type="number" value={p().id_start} style={{ width: "70px" }} onChange={e => put("id_start", +e.currentTarget.value)} /> step <input type="number" value={p().id_step} style={{ width: "50px" }} onChange={e => put("id_step", +e.currentTarget.value)} /></span></label>
        </Show>
        <Show when={step() === 1}>
          <p class="muted">The project calendar sets working days and hours. Add holidays and shutdowns as exceptions.</p>
          <label>Calendar<select value={p().calendar} onChange={e => act({ Project: { name: p().name, start: p().start, status: p().status ?? "", calendar: e.currentTarget.value, day_hours: p().day_min / 60, manual_default: p().manual_default } })}><For each={p().calendars}>{c => <option>{c.name}</option>}</For></select></label>
          <label>Hours per day<input type="number" value={p().day_min / 60} onChange={e => act({ Project: { name: p().name, start: p().start, status: p().status ?? "", calendar: p().calendar, day_hours: +e.currentTarget.value, manual_default: p().manual_default } })} /></label>
          <button class="btn" onClick={() => D.calendars()}>Change working time…</button>
          <p class="muted">Reporting periods: weekly and monthly cut-offs for progress reports and S-curves.</p>
          <label>Week ends on<select value={p().periods.week_end} onChange={e => put("periods", { ...p().periods, week_end: +e.currentTarget.value })}><For each={["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"]}>{(d, k) => <option value={k()}>{d}</option>}</For></select></label>
          <label>Month closes on<select value={p().periods.month_end} onChange={e => put("periods", { ...p().periods, month_end: +e.currentTarget.value })}><option value="0">Last day of the month</option><For each={Array.from({ length: 28 }, (_, k) => k + 1)}>{d => <option value={d}>Day {d}</option>}</For></select></label>
          <label>Weight progress by<select value={p().periods.weight} onChange={e => put("periods", { ...p().periods, weight: e.currentTarget.value })}><option value="duration">Duration</option><option value="cost">Cost</option><option value="work">Hours</option><option value="physical">Physical %</option></select></label>
        </Show>
        <Show when={step() === 2}>
          <p class="muted">Costs, reports and exports use this currency. Any ISO 4217 code works.</p>
          <div class="inline"><CurrencyPick value={p().currency} set={c => put("currency", c)} /></div>
        </Show>
        <Show when={step() === 3}>
          <p class="muted">Type the work breakdown, one item per line. Indent with two spaces for each level.</p>
          <textarea class="wbs" value={wbs()} onInput={e => setWbs(e.currentTarget.value)} />
          <button class="btn pri" onClick={addWbs}>Add to plan</button>
        </Show>
        <Show when={step() === 4}>
          <p class="muted">People, equipment or materials, one per line. Rates and types are set in the Resource Sheet.</p>
          <textarea class="wbs" value={res()} placeholder={"Project Manager\nSite Engineer\nCrane"} onInput={e => setRes(e.currentTarget.value)} />
          <button class="btn pri" onClick={addRes}>Add resources</button>
        </Show>
        <Show when={step() === 5}>
          <p class="muted">Your company name and logo appear on reports and printouts.</p>
          <label>Company<input value={p().company} onChange={e => put("company", e.currentTarget.value)} /></label>
          <Show when={p().logo}><img src={p().logo} class="logoprev" /></Show>
          <div class="inline"><button class="btn" onClick={async () => { const l = await call<string | null>("pick_logo"); if (l) put("logo", l); }}>Choose logo…</button>
            <Show when={!p().logo && opts().logo}><button class="btn" onClick={() => put("logo", opts().logo)}>Use my default logo</button></Show></div>
          <p class="muted">The logos of the other parties (the client above you, vendors and sub-vendors below you) and any other pictures for dashboards and sheets. They are kept in the plan file.</p>
          <Library />
        </Show>
        <Show when={step() === 6}>
          <p>All set. Next: add tasks in the Gantt chart, link them, assign resources and set a baseline.</p>
          <button class="btn pri" onClick={finish}>Go to the Gantt chart</button>
        </Show>
      </div>
      <footer><button class="btn" disabled={step() === 0} onClick={() => setStep(step() - 1)}>Back</button><span class="grow" /><button class="btn" onClick={finish}>Skip setup</button>
        <Show when={step() < 6}><button class="btn pri" onClick={() => setStep(step() + 1)}>Next</button></Show></footer>
    </aside>
  );
};
