// Progress history views: Manpower (people planned against work done, by trade), the Look-ahead
// with its constraint log, and the Rules of Credit dialog. The engine works out every value
// (crates/core/src/progress.rs); these pages only show them.
import { For, Index, Show, createSignal, createResource, createMemo, createEffect } from "solid-js";
import { Manpower as MP, Lookahead as LA, LogItem, CreditRule, fmtDay } from "./api";
import { call, P, act, tick, selRows, curRow, openDialog, toast, setView, grid } from "./store";
import { selectUid } from "./cmds";
import { Page } from "./views";
import { ChartBox, bar, line, COL } from "./charts";
import { label } from "./reports";
import { Seg, Dialog } from "./ui";

// ---------- Manpower ----------
// The choices live outside the pages so that their printed copies show the same.
const [unit, setUnit] = createSignal("week"), [trade, setTrade] = createSignal("All trades");
export const Manpower = () => {
  const [m] = createResource(() => [tick(), unit()], () => call<MP>("manpower", { unit: unit() }));
  const t = () => m()?.trades.find(x => x.name === trade()) ?? m()?.trades[0];
  const r1 = (v: number | null) => (v === null ? null : Math.round(v * 10) / 10);
  return (
    <Page title="Manpower" help="manpower" tools={<>
      <select onChange={e => setTrade(e.currentTarget.value)}><For each={m()?.trades ?? []}>{x => <option selected={x.name === trade()}>{x.name}</option>}</For></select>
      <Seg options={[["week", "Weeks"], ["month", "Months"]]} value={unit()} set={setUnit} />
    </>}>
      <Show when={t() && (m()?.trades.length ?? 0) > 1} fallback={<p class="empty">Assign work resources to tasks to see the people each trade needs. A resource's Group is its trade.</p>}>
        <div class="panel">
          <ChartBox title={`${t()!.name}: people per ${unit()}`} type="bar" fmt="num" y="People" size="tall" days={m()!.buckets} labels={m()!.buckets.map(b => label(b, unit()))}
            marker={m()!.buckets.findIndex(b => b > m()!.status_day) - 1} markerLabel="Status"
            datasets={[bar("Planned", t()!.planned.map(r1), COL.grey), bar("Done (earned from progress)", t()!.actual.map(r1), COL.blue), { ...line("Forecast", t()!.forecast.map(r1), COL.red, [5, 4]), tension: 0, cubicInterpolationMode: "default" as const }]} />
          <p class="muted">Planned spreads each task's hours over its baseline dates (current dates without a baseline). Done is the work earned from progress (% complete × planned hours) in each period, as stored at each period close; it is not hours booked. People = hours ÷ one person's working hours in the period.</p>
        </div>
      </Show>
    </Page>
  );
};

// ---------- Look-ahead and constraint log ----------
const KINDS = ["Material", "Drawing or information", "Permit", "Access", "Labour", "Equipment", "Previous work", "Other"];

const [weeks, setWeeks] = createSignal("3");
export const Lookahead = () => {
  const [pick, setPick] = createSignal(0);
  const [la] = createResource(() => [tick(), weeks()], () => call<LA>("lookahead", { weeks: +weeks() }));
  const name = (uid: number) => grid().rows.find(r => r.uid === uid)?.name ?? la()?.rows.find(r => r.uid === uid)?.name ?? `Task ${uid}`;
  const days = () => (la() ? la()!.to - la()!.from : 21);
  const X = (t: number) => Math.max(0, Math.min(1, (t / 1440 - la()!.from) / days())) * 360;
  const put = (x: LogItem) => act({ Log: { id: x.id, item: x } });
  const add = () => {
    const uid = pick() || la()?.rows[0]?.uid;
    if (!uid) { toast("No tasks in the look-ahead to add a constraint to."); return; }
    act({ Log: { id: 0, item: { id: 0, uid, kind: "Material", text: "", owner: "", need: null, status: "Open" } } });
  };
  const shown = createMemo(() => { const ids = new Set((la()?.rows ?? []).map(r => r.uid)); return [...P().log].sort((a, b) => +ids.has(b.uid) - +ids.has(a.uid) || a.id - b.id); });
  return (
    <Page title="Look-ahead" help="lookahead" tools={<>
      <span class="muted">From the status date</span><Seg options={[["2", "2 weeks"], ["3", "3 weeks"], ["4", "4 weeks"], ["6", "6 weeks"]]} value={weeks()} set={setWeeks} />
    </>}>
      <Show when={la()}>{l => <>
        <div class="panel">
          <h2>Work from {fmtDay(l().from, "long")} to {fmtDay(l().to - 1, "long")}</h2>
          <Show when={l().rows.length} fallback={<p class="empty">No unfinished work in these weeks.</p>}>
            <table class="tbl look"><thead><tr><th>ID</th><th>Task</th><th>Start</th><th>Finish</th><th class="r">%</th><th>Resources</th><th>Ready?</th><th class="lbar"><svg width="360" height="14"><Index each={Array.from({ length: Math.ceil(days() / 7) })}>{(_, k) => <><line x1={k * 7 / days() * 360} x2={k * 7 / days() * 360} y1="0" y2="14" stroke="#C9D3E0" /><text x={k * 7 / days() * 360 + 3} y="11" class="ax">{fmtDay(l().from + k * 7)}</text></>}</Index></svg></th></tr></thead>
              <tbody><For each={l().rows}>{r => (
                <tr class="click" classList={{ on: pick() === r.uid }} onClick={() => setPick(r.uid)} onDblClick={() => { setView("gantt"); selectUid(r.uid); }}>
                  <td>{r.code}</td><td>{r.name}</td><td>{fmtDay(Math.floor(r.es / 1440))}</td><td>{fmtDay(Math.floor((r.ef - 1) / 1440))}</td><td class="r">{r.pct}%</td><td class="muted">{r.res}</td>
                  <td><span class="badge" classList={{ ok: !r.open, warn: r.open > 0 && !r.overdue, bad: r.overdue > 0 }}>{!r.open ? "Ready" : r.overdue ? `${r.overdue} overdue` : `${r.open} open`}</span></td>
                  <td class="lbar"><svg width="360" height="14"><rect x={X(r.es)} y="3" width={Math.max(3, X(r.ef) - X(r.es))} height="8" rx="2" fill={r.crit ? "#D64545" : "#6E9FE6"} /></svg></td>
                </tr>
              )}</For></tbody></table>
          </Show>
        </div>
        <div class="panel">
          <h2>Constraint log</h2>
          <p class="muted">What each task needs before it can start: materials, drawings, permits, access, people. A task is ready when all its constraints are cleared; overdue means the need-by date has passed.</p>
          <table class="tbl"><thead><tr><th>#</th><th>Task</th><th>Type</th><th>What is needed</th><th>Owner</th><th>Need by</th><th>Status</th><th /></tr></thead>
            <tbody><For each={shown()}>{x => (
              <tr>
                <td>C{x.id}</td>
                <td><select onChange={e => put({ ...x, uid: +e.currentTarget.value })}><Show when={!l().rows.some(r => r.uid === x.uid)}><option value={x.uid} selected>{name(x.uid)}</option></Show><For each={l().rows}>{r => <option value={r.uid} selected={r.uid === x.uid}>{r.name}</option>}</For></select></td>
                <td><select onChange={e => put({ ...x, kind: e.currentTarget.value })}><For each={KINDS}>{k => <option selected={k === x.kind}>{k}</option>}</For></select></td>
                <td><input value={x.text} placeholder="e.g. cable drums on site" onChange={e => put({ ...x, text: e.currentTarget.value })} /></td>
                <td><input value={x.owner} onChange={e => put({ ...x, owner: e.currentTarget.value })} /></td>
                <td><input type="date" value={x.need ?? ""} onChange={e => put({ ...x, need: e.currentTarget.value || null })} /></td>
                <td><select onChange={e => put({ ...x, status: e.currentTarget.value })}><option selected={x.status !== "Cleared"}>Open</option><option selected={x.status === "Cleared"}>Cleared</option></select></td>
                <td><button class="x" title="Delete" onClick={() => act({ Log: { id: x.id, item: null } })}>×</button></td>
              </tr>
            )}</For></tbody></table>
          <p><button class="btn pri" onClick={add}>+ Add constraint{pick() ? ` to ${name(pick())}` : ""}</button></p>
        </div>
      </>}</Show>
    </Page>
  );
};

// ---------- Rules of Credit ----------
export const rulesOfCredit = async () => {
  const lib = (await call<CreditRule[]>("credit_library")) ?? [];
  const uids = () => { const s = selRows().map(r => r.uid); return s.length ? s : curRow()?.uid > 0 ? [curRow()!.uid] : []; };
  openDialog(() => {
    const all = () => [...P().credit.map(r => ({ ...r, own: true })), ...lib.filter(r => !P().credit.some(x => x.name === r.name)).map(r => ({ ...r, own: false }))];
    const [cur, setCur] = createSignal(all()[0]?.name ?? "");
    const rule = () => all().find(r => r.name === cur());
    const [draft, setDraft] = createSignal<CreditRule | null>(null);
    createEffect(() => { const r = rule(); setDraft(r ? { name: r.name, steps: r.steps.map(s => [...s] as [string, number]) } : null); });
    const total = () => (draft()?.steps ?? []).reduce((a, s) => a + (+s[1] || 0), 0);
    const save = () => { const d = draft(); if (d) act({ Credit: { old: rule()?.own ? rule()!.name : null, rule: d } }).then(ok => ok && (setCur(d.name), toast("Rule saved in this plan.", true))); };
    const step = (k: number, i: 0 | 1, v: string) => setDraft({ ...draft()!, steps: draft()!.steps.map((s, j) => (j === k ? (i === 0 ? [v, s[1]] : [s[0], +v || 0]) : s)) as [string, number][] });
    return (
      <Dialog title="Rules of Credit" wide help="credit" okText={`Apply to ${uids().length} selected task${uids().length === 1 ? "" : "s"}`} ok={async () => {
        if (!uids().length) { toast("Select the tasks first."); return false; }
        const ok = await act({ ApplyCredit: { uids: uids(), rule: cur() } });
        if (ok) toast(`"${cur()}" applied: open a task's Steps tab to tick steps off.`, true);
        return ok;
      }}>
        <div class="credit">
          <div class="list"><For each={all()}>{r => <button classList={{ on: cur() === r.name }} onClick={() => setCur(r.name)}>{r.name}<span class="muted">{r.own ? "this plan" : "built in"}</span></button>}</For>
            <button class="btn" onClick={() => { setDraft({ name: "New rule", steps: [["First step", 50], ["Second step", 50]] }); setCur(""); }}>+ New rule</button></div>
          <Show when={draft()}>{d => <div>
            <label>Name <input value={d().name} onChange={e => setDraft({ ...d(), name: e.currentTarget.value })} /></label>
            <table class="tbl"><thead><tr><th>Step</th><th class="r">Weight %</th><th /></tr></thead>
              <tbody><Index each={d().steps}>{(s, k) => <tr><td><input value={s()[0]} onChange={e => step(k, 0, e.currentTarget.value)} /></td><td class="r"><input class="num" type="number" min="0" value={s()[1]} onChange={e => step(k, 1, e.currentTarget.value)} /></td>
                <td><button class="x" onClick={() => setDraft({ ...d(), steps: d().steps.filter((_, j) => j !== k) })}>×</button></td></tr>}</Index></tbody></table>
            <p><button class="btn" onClick={() => setDraft({ ...d(), steps: [...d().steps, ["New step", 0]] })}>+ Step</button> <span class="muted" classList={{ late: Math.abs(total() - 100) > 0.01 }}>Total {total()}%</span></p>
            <p><button class="btn" onClick={save}>Save in this plan</button><Show when={rule()?.own}> <button class="btn" onClick={() => act({ Credit: { old: rule()!.name, rule: null } })}>Delete</button></Show></p>
          </div>}</Show>
        </div>
        <p class="muted">A rule of credit splits a kind of work into steps, each worth a share of the progress. Applying it gives the selected tasks those steps and measures them by physical % complete; tick steps off in the task's Steps tab. Installed quantities (the Quantity and Installed Quantity columns) measure progress the same way.</p>
      </Dialog>
    );
  });
};
