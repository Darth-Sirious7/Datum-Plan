// Modules (Project › Modules) and the Contract Dates & Payments view: contract dates against
// their forecast with delay damages, payment milestones and the cash-in curve. The engine works
// every figure out (contract.rs); this page shows them and edits the contract.
import { For, Show, createResource, createSignal } from "solid-js";
import { Contract, ContractDate, ContractOut, Payment, money } from "./api";
import { act, call, grid, out, P, sel, tick, setView, openDialog, toast } from "./store";
import { Page } from "./views";
import { Kpi } from "./reports";
import { ChartBox, COL, line } from "./charts";
import { Dialog } from "./ui";
import { showTab } from "./plans";

/** The modules a plan can take on (the engine's `contract::MODULES`). */
export const MODULES: [string, string, string][] = [
  ["contract", "Contract Dates & Payments", "Contract dates against their forecast, delay damages at risk, payment milestones and the cash-in curve."],
];
export const hasModule = (k: string) => (out()?.project.modules ?? []).includes(k);

/** Project › Modules: tick the modules this plan uses. Switching one off can keep or delete its data. */
export const modules = () => openDialog(() => {
  const [on, setOn] = createSignal<string[]>([...(P().modules ?? [])]), [drop, setDrop] = createSignal(false);
  const off = () => (P().modules ?? []).filter(k => !on().includes(k));
  const hasData = () => { const c = P().contract; return !!c && !!(c.dates.length || c.payments.length || c.value); };
  const ok = async () => {
    if (drop() && off().includes("contract") && hasData()) await act({ Contract: { c: {} } });
    const fresh = on().includes("contract") && !(P().modules ?? []).includes("contract");
    await act({ Modules: { on: on() } });
    if (fresh) setView("contract");
  };
  return (
    <Dialog title="Modules" help="modules" okText="Apply" ok={ok}>
      <p>Modules add registers to this plan. A plan shows only the ones ticked here; the rest of the app stays as it is.</p>
      <For each={MODULES}>{([k, l, d]) => <label class="chk modrow"><input type="checkbox" checked={on().includes(k)} onChange={() => setOn(on().includes(k) ? on().filter(x => x !== k) : [...on(), k])} /><span><b>{l}</b><small class="muted">{d}</small></span></label>}</For>
      <Show when={off().includes("contract") && hasData()}>
        <div class="modoff"><b>Contract Dates & Payments is being switched off.</b>
          <label class="chk"><input type="radio" checked={!drop()} onChange={() => setDrop(false)} /> Keep its dates and payments in the plan, hidden, for later</label>
          <label class="chk"><input type="radio" checked={drop()} onChange={() => setDrop(true)} /> Delete its dates and payments</label></div>
      </Show>
      <p class="muted">More modules come with later releases: a document register and a procurement register.</p>
    </Dialog>
  );
});

const blankDate = (id: number): ContractDate => ({ id, name: "Contract date", task: 0, date: null, value: 0, ld_rate: 0, ld_cap: 0, grace: 0, note: "" });
const blankPay = (id: number): Payment => ({ id, name: "Payment", pct: 0, amount: 0, task: 0, terms: 30, invoiced: null, paid: null });
const STATUS: Record<string, string> = { Met: "good", "Met late": "warn", "On track": "good", "At risk": "warn", Late: "bad", Overdue: "bad", Paid: "good", Invoiced: "warn", Due: "warn" };

export const ContractView = () => {
  const [data] = createResource(() => [tick(), out()?.tab] as const, () => call<{ tab: number; name: string; on: boolean; out: ContractOut }[]>("contract"));
  const k = (): Contract => ({ number: "", value: 0, warn_days: 14, dates: [], payments: [], ...(P().contract ?? {}) });
  const save = (c: Partial<Contract>) => act({ Contract: { c: { ...k(), ...c } } });
  const putDate = (m: ContractDate) => save({ dates: k().dates.map(x => (x.id === m.id ? m : x)) });
  const putPay = (m: Payment) => save({ payments: k().payments.map(x => (x.id === m.id ? m : x)) });
  const next = (l: { id: number }[]) => Math.max(0, ...l.map(x => x.id)) + 1;
  const picked = () => sel().find(u => u > 0) ?? 0;
  const me = () => data()?.find(d => d.tab === out()?.tab);
  const o = () => me()?.out;
  const cur = () => P().currency;
  const m$ = (v: number) => money(v, { ...cur(), decimals: 0 });
  const tasks = () => grid().rows.filter(r => r.uid > 0);
  const TaskPick = (p: { uid: number; name: string; set: (uid: number) => void }) => (
    <span class="tpick"><select value={String(p.uid)} onChange={e => p.set(+e.currentTarget.value)}>
      <option value="0">— choose a task —</option>
      <Show when={p.uid && !tasks().some(t => t.uid === p.uid)}><option value={String(p.uid)}>{p.name || `Task ${p.uid}`}</option></Show>
      <For each={tasks()}>{t => <option value={String(t.uid)}>{t.id} · {t.name}</option>}</For>
    </select><button class="x" title="Use the task selected in the task sheet" disabled={!picked()} onClick={() => p.set(picked())}>⇦</button></span>
  );
  const dt = (v: string | null) => (v ? new Date(v + "T00:00").toLocaleDateString("en-GB", { day: "2-digit", month: "short", year: "2-digit" }) : "—");
  const days = (v: number | null) => (v === null ? "—" : v > 0 ? `${v} d late` : v < 0 ? `${-v} d early` : "on the day");
  const Spark = (p: { t: [string, string][]; date: string | null }) => {
    const pts = () => (p.date ? p.t.map(([, f]) => (Date.parse(f) - Date.parse(p.date!)) / 864e5) : []);
    const W = 90, H = 22, max = () => Math.max(5, ...pts().map(Math.abs));
    return <Show when={pts().length > 1} fallback={<span class="muted">—</span>}><svg width={W} height={H} class="spark"><line x1="0" x2={W} y1={H / 2} y2={H / 2} stroke="#C9CFD8" stroke-dasharray="2 2" />
      <polyline fill="none" stroke={pts()[pts().length - 1] > 0 ? "#C73B3B" : "#2E9D57"} stroke-width="1.5" points={pts().map((v, i) => `${(i / (pts().length - 1)) * (W - 4) + 2},${H / 2 - (v / max()) * (H / 2 - 2)}`).join(" ")} />
      <title>{p.t.map(([d, f]) => `${dt(d)}: forecast ${dt(f)}`).join("\n")}</title></svg></Show>;
  };
  return (
    <Page title="Contract Dates & Payments" help="contract" tools={<>
      <label class="muted">Contract no. <input style={{ width: "130px" }} value={k().number} onChange={e => save({ number: e.currentTarget.value })} /></label>
      <label class="muted">Value ({cur().symbol || cur().code}) <input type="number" min="0" style={{ width: "120px" }} value={k().value || ""} onChange={e => save({ value: +e.currentTarget.value || 0 })} /></label>
      <label class="muted" title="A contract date with less float than this is At risk">At risk under <input type="number" min="0" style={{ width: "52px" }} value={k().warn_days} onChange={e => save({ warn_days: +e.currentTarget.value || 0 })} /> days</label>
    </>}>
      <Show when={o()}>{s => <>
        <div class="kpis">
          <Kpi v={m$(s().value)} l={k().number ? `Contract ${k().number}` : "Contract value"} />
          <Kpi v={`${s().late} late · ${s().at_risk} at risk`} l={`of ${s().milestones.length} contract dates`} bad={s().late > 0} good={!s().late && !s().at_risk && s().milestones.length > 0} />
          <Kpi v={m$(s().ld)} l={`delay damages at today's forecast (cap ${m$(s().ld_max)})`} bad={s().ld > 0} />
          <Kpi v={m$(s().received)} l={`received of ${m$(s().payments.reduce((a, p) => a + p.amount, 0))}`} />
          <Kpi v={`${s().pay_pct.toFixed(1)} %`} l="of the value in payment milestones" bad={s().payments.length > 0 && Math.abs(s().pay_pct - 100) > 0.05} />
        </div>
        <div class="panel ctr">
          <div class="phead"><h2>Contract dates</h2><span class="grow" /><button class="btn sm pri" onClick={() => save({ dates: [...k().dates, { ...blankDate(next(k().dates)), task: picked() }] })}>+ Contract date</button></div>
          <table class="tbl"><thead><tr><th>Contract date</th><th>Met by task</th><th>Contract</th><th>Forecast</th><th>Baseline</th><th>Against contract</th><th>Status</th><th title="Forecast at each period close">Trend</th>
            <th title="Delay damages in % of the value per week or part week late">Damages %/wk</th><th title="Days late before damages start">Grace (d)</th><th title="Most the damages can reach, in % of the value">Cap %</th><th title="The value the damages are a share of (the unit's price); empty = the contract value">Of value</th><th class="r">At risk</th><th /></tr></thead>
            <tbody><For each={k().dates}>{m => { const r = () => s().milestones.find(x => x.id === m.id); return (
              <tr>
                <td><input value={m.name} onChange={e => putDate({ ...m, name: e.currentTarget.value })} /></td>
                <td><TaskPick uid={m.task} name={r()?.task_name ?? ""} set={task => putDate({ ...m, task })} /></td>
                <td><input type="date" value={m.date ?? ""} onChange={e => putDate({ ...m, date: e.currentTarget.value || null })} /></td>
                <td>{dt(r()?.forecast ?? null)}</td><td class="muted">{dt(r()?.baseline ?? null)}</td>
                <td classList={{ bad: (r()?.var ?? 0) > 0 }}>{days(r()?.var ?? null)}</td>
                <td><span class={`st ${STATUS[r()?.status ?? ""] ?? ""}`}>{r()?.status}</span></td>
                <td><Spark t={r()?.trend ?? []} date={m.date} /></td>
                <td><input type="number" min="0" step="0.1" style={{ width: "62px" }} value={m.ld_rate || ""} onChange={e => putDate({ ...m, ld_rate: +e.currentTarget.value || 0 })} /></td>
                <td><input type="number" min="0" style={{ width: "52px" }} value={m.grace || ""} onChange={e => putDate({ ...m, grace: +e.currentTarget.value || 0 })} /></td>
                <td><input type="number" min="0" step="0.5" style={{ width: "56px" }} value={m.ld_cap || ""} onChange={e => putDate({ ...m, ld_cap: +e.currentTarget.value || 0 })} /></td>
                <td><input type="number" min="0" style={{ width: "100px" }} placeholder="contract" value={m.value || ""} onChange={e => putDate({ ...m, value: +e.currentTarget.value || 0 })} /></td>
                <td class="r" classList={{ bad: (r()?.ld ?? 0) > 0 }}>{m$(r()?.ld ?? 0)}</td>
                <td><button class="x" title="Remove" onClick={() => save({ dates: k().dates.filter(x => x.id !== m.id) })}>×</button></td>
              </tr>); }}</For></tbody></table>
          <Show when={!k().dates.length}><p class="empty">No contract dates yet. Select the task that meets one (a delivery milestone, say) and click <b>+ Contract date</b>, then type the date the contract fixes.</p></Show>
        </div>
        <div class="panel ctr">
          <div class="phead"><h2>Payment milestones</h2><span class="grow" /><button class="btn sm pri" onClick={() => save({ payments: [...k().payments, { ...blankPay(next(k().payments)), task: picked() }] })}>+ Payment</button></div>
          <table class="tbl"><thead><tr><th>Payment</th><th>When this task finishes</th><th title="Share of the contract value">%</th><th title="An amount instead of a share">Amount</th><th title="Days from the event to payment">Terms (d)</th><th>Planned</th><th>Forecast due</th><th>Invoiced</th><th>Paid</th><th>Status</th><th class="r">Value</th><th /></tr></thead>
            <tbody><For each={k().payments}>{m => { const r = () => s().payments.find(x => x.id === m.id); return (
              <tr>
                <td><input value={m.name} onChange={e => putPay({ ...m, name: e.currentTarget.value })} /></td>
                <td><TaskPick uid={m.task} name={r()?.task_name ?? ""} set={task => putPay({ ...m, task })} /></td>
                <td><input type="number" min="0" step="0.5" style={{ width: "60px" }} value={m.pct || ""} onChange={e => putPay({ ...m, pct: +e.currentTarget.value || 0 })} /></td>
                <td><input type="number" min="0" style={{ width: "100px" }} value={m.amount || ""} onChange={e => putPay({ ...m, amount: +e.currentTarget.value || 0 })} /></td>
                <td><input type="number" min="0" style={{ width: "52px" }} value={m.terms} onChange={e => putPay({ ...m, terms: +e.currentTarget.value || 0 })} /></td>
                <td class="muted">{dt(r()?.planned ?? null)}</td><td>{dt(r()?.due ?? null)}</td>
                <td><input type="date" value={m.invoiced ?? ""} onChange={e => putPay({ ...m, invoiced: e.currentTarget.value || null })} /></td>
                <td><input type="date" value={m.paid ?? ""} onChange={e => putPay({ ...m, paid: e.currentTarget.value || null })} /></td>
                <td><span class={`st ${STATUS[r()?.status ?? ""] ?? ""}`}>{r()?.status}</span></td>
                <td class="r">{m$(r()?.amount ?? 0)}</td>
                <td><button class="x" title="Remove" onClick={() => save({ payments: k().payments.filter(x => x.id !== m.id) })}>×</button></td>
              </tr>); }}</For></tbody></table>
          <Show when={!k().payments.length}><p class="empty">No payment milestones yet. Each is a share of the contract value (or an amount) due some days after a task finishes: an advance on the purchase order, a share on drawings approved, on FAT, on delivery.</p></Show>
        </div>
        <Show when={s().cash.length > 1}>
          <ChartBox title="Cash in" type="line" fmt="money" labels={s().cash.map(c => dt(c[0]))} days={s().cash.map(c => Math.round(Date.parse(c[0]) / 864e5))}
            datasets={[line("Planned", s().cash.map(c => c[1]), COL.grey, [5, 4]), line("Forecast", s().cash.map(c => c[2]), COL.blue), line("Received", s().cash.map(c => c[3]), COL.green)]} />
        </Show>
      </>}</Show>
      <Show when={(data()?.length ?? 0) > 1}>
        <div class="panel ctr"><h2>The project's plans</h2>
          <table class="tbl"><thead><tr><th>Plan</th><th>Contract dates</th><th>Late</th><th>At risk</th><th class="r">Damages at risk</th><th class="r">Payments</th><th class="r">Received</th><th /></tr></thead>
            <tbody><For each={data()}>{d => <tr>
              <td><b>{d.name}</b>{d.tab === out()?.tab ? " (this plan)" : ""}</td>
              <td>{d.on ? d.out.milestones.length : <span class="muted">module off</span>}</td><td classList={{ bad: d.out.late > 0 }}>{d.out.late}</td><td>{d.out.at_risk}</td>
              <td class="r">{m$(d.out.ld)}</td><td class="r">{m$(d.out.payments.reduce((a, p) => a + p.amount, 0))}</td><td class="r">{m$(d.out.received)}</td>
              <td>{d.tab !== out()?.tab && <button class="btn sm" onClick={() => showTab(d.tab).then(() => setView("contract"))}>Open</button>}</td>
            </tr>}</For>
            <tr class="tot"><td><b>Total</b></td><td>{data()!.reduce((a, d) => a + d.out.milestones.length, 0)}</td><td>{data()!.reduce((a, d) => a + d.out.late, 0)}</td><td>{data()!.reduce((a, d) => a + d.out.at_risk, 0)}</td>
              <td class="r">{m$(data()!.reduce((a, d) => a + d.out.ld, 0))}</td><td class="r">{m$(data()!.reduce((a, d) => a + d.out.payments.reduce((x, p) => x + p.amount, 0), 0))}</td><td class="r">{m$(data()!.reduce((a, d) => a + d.out.received, 0))}</td><td /></tr></tbody></table>
          <p class="muted">The master and its open subplans. Open a subplan to edit its contract dates.</p>
        </div>
      </Show>
    </Page>
  );
};

/** Opens the view, or offers to switch the module on. */
export const openContract = () => (hasModule("contract") ? setView("contract") : (toast("Contract Dates & Payments is a module: tick it in Project › Modules."), modules()));
