// The details pane under the Gantt chart: details tabs for the selected task.
import { For, Show, createResource, JSX, Switch, Match } from "solid-js";
import { TaskInfo, Rel } from "./api";
import { call, curUid, tick, detailsTab, setDetailsTab, set, act, P, out, fields } from "./store";
import { selectUid } from "./cmds";
import { Tabs, DateBtn } from "./ui";
import { Help } from "./help";

export const TABS: [string, string][] = [
  ["general", "General"], ["status", "Status"], ["res", "Resources"], ["preds", "Predecessors"], ["succs", "Successors"], ["codes", "Codes"],
  ["steps", "Steps"], ["notebook", "Notebook"], ["expenses", "Expenses"], ["docs", "WPs & Docs"], ["udf", "Custom Fields"], ["risks", "Risks"], ["notes", "Notes"],
];

export const useInfo = (uid: () => number) => createResource(() => [uid(), tick()] as const, ([u]) => (u > 0 ? call<TaskInfo>("task_info", { uid: u }) : undefined));

/** One editable value: label + input bound to a task field. */
export const F = (p: { info: TaskInfo; k: string; label?: string; opts?: string[]; wide?: boolean }) => {
  const f = () => p.info.fields.find(x => x[0] === p.k);
  const text = () => f()?.[3] ?? "";
  return <>
    <label>{p.label ?? f()?.[1] ?? p.k}</label>
    {p.opts ? <select value={text()} onChange={e => set(p.info.task.uid, p.k, e.currentTarget.value)}><For each={p.opts}>{o => <option>{o}</option>}</For></select>
      : ["Date", "Fin"].includes(fields().task.find(x => x.key === p.k)?.kind ?? "")
        ? <span class="dfield"><input value={text()} onChange={e => set(p.info.task.uid, p.k, e.currentTarget.value)} /><DateBtn value={text()} set={v => set(p.info.task.uid, p.k, v)} /></span>
        : <input value={text()} class={p.wide ? "wide" : ""} onChange={e => set(p.info.task.uid, p.k, e.currentTarget.value)} />}
  </>;
};
export const RO = (p: { info: TaskInfo; k: string; label?: string }) => {
  const f = () => p.info.fields.find(x => x[0] === p.k);
  return <><label>{p.label ?? f()?.[1]}</label><span class="ro">{f()?.[3] || "—"}</span></>;
};

export const LINKS: [string, string][] = [["FS", "Finish-to-Start"], ["SS", "Start-to-Start"], ["FF", "Finish-to-Finish"], ["SF", "Start-to-Finish"]];
const Rels = (p: { info: TaskInfo; list: Rel[]; preds: boolean }) => {
  const uid = p.info.task.uid;
  const put = (other: number, kind: string | null, lag: string) => act({ LinkSet: p.preds ? { pred: other, succ: uid, kind, lag } : { pred: uid, succ: other, kind, lag } });
  let add!: HTMLInputElement;
  return (
    <table class="tbl">
      <thead><tr><th>ID</th><th>Activity ID</th><th>Name</th><th>Type <button class="help" title="What FS, SS, FF and SF mean" onClick={() => import("./dialogs").then(d => d.glossary())}>?</button></th><th title="Wait (+2d) or overlap (−2d, −20%) after the link">Lag</th><th>Driving</th><th>Total float</th><th /></tr></thead>
      <tbody>
        <For each={p.list}>{r => (
          <tr>
            <td class="link" onClick={() => selectUid(r.uid)}>{r.id}</td><td>{r.code}</td><td>{r.name}</td>
            <td><select value={r.kind} onChange={e => put(r.uid, e.currentTarget.value, r.lag)}><For each={LINKS}>{([k, l]) => <option value={k} title={l}>{k} · {l}</option>}</For></select></td>
            <td><input size="6" value={r.lag} onChange={e => put(r.uid, r.kind, e.currentTarget.value)} /></td>
            <td>{r.driving ? "Yes" : ""}</td><td>{r.tf}</td>
            <td><button class="x" title="Remove link" onClick={() => put(r.uid, null, "")}>×</button></td>
          </tr>
        )}</For>
        <tr class="add"><td colspan="8">Add {p.preds ? "predecessor" : "successor"} by ID: <input size="6" ref={add} onChange={async e => {
          const id = parseInt(e.currentTarget.value);
          const all = await call<{ rows: { uid: number; id: number }[] }>("query", { q: { cols: [], filter: { name: "", rules: [], summaries: true, highlight: false }, group: [], sort: [], flat: true, collapsed: [], level: 0, hide_summaries: false, project_summary: false, baseline: 0 } });
          const t = all?.rows.find(r => r.id === id);
          if (t) put(t.uid, "FS", ""); add.value = "";
        }} /></td></tr>
      </tbody>
    </table>
  );
};

/** An editable list of records (steps, expenses, notebook, documents). */
const ListEditor = <T extends Record<string, any>>(p: { uid: number; field: string; items: T[]; cols: [keyof T & string, string, "text" | "number" | "check" | "select", string[]?][]; blank: T }) => {
  const save = (v: T[]) => act({ TaskList: { uid: p.uid, field: p.field, value: v } });
  const put = (k: number, key: string, val: any) => save(p.items.map((x, j) => (j === k ? { ...x, [key]: val } : x)));
  return (
    <table class="tbl">
      <thead><tr><For each={p.cols}>{c => <th>{c[1]}</th>}</For><th /></tr></thead>
      <tbody>
        <For each={p.items}>{(it, k) => (
          <tr>
            <For each={p.cols}>{([key, , kind, opts]) => <td>{
              kind === "check" ? <input type="checkbox" checked={!!it[key]} onChange={e => put(k(), key, e.currentTarget.checked)} />
                : kind === "select" ? <select value={it[key]} onChange={e => put(k(), key, e.currentTarget.value)}><For each={opts}>{o => <option>{o}</option>}</For></select>
                : <input value={it[key] ?? ""} type={kind} onChange={e => put(k(), key, kind === "number" ? parseFloat(e.currentTarget.value) || 0 : e.currentTarget.value)} />
            }</td>}</For>
            <td><button class="x" onClick={() => save(p.items.filter((_, j) => j !== k()))}>×</button></td>
          </tr>
        )}</For>
        <tr class="add"><td colspan={p.cols.length + 1}><button class="btn sm" onClick={() => save([...p.items, p.blank])}>+ Add</button></td></tr>
      </tbody>
    </table>
  );
};

export const TaskTab = (p: { info: TaskInfo; tab: string }): JSX.Element => {
  const t = () => p.info.task;
  const uid = () => t().uid;
  return (
    <Switch>
      <Match when={p.tab === "general"}>
        <div class="dgrid">
          <F info={p.info} k="code" /><F info={p.info} k="name" wide />
          <F info={p.info} k="dur" /><F info={p.info} k="pct" />
          <F info={p.info} k="start" /><F info={p.info} k="finish" />
          <F info={p.info} k="mode" opts={["Auto Scheduled", "Manually Scheduled"]} /><F info={p.info} k="type" opts={["Fixed Duration", "Fixed Units", "Fixed Work"]} />
          <F info={p.info} k="acttype" opts={["Task Dependent", "Resource Dependent", "Level of Effort", "Start Milestone", "Finish Milestone", "WBS Summary"]} /><F info={p.info} k="pcttype" opts={["Duration", "Units", "Physical"]} />
          <F info={p.info} k="ctype" opts={["As Soon As Possible", "As Late As Possible", "Start No Earlier Than", "Start No Later Than", "Finish No Earlier Than", "Finish No Later Than", "Must Start On", "Must Finish On"]} /><F info={p.info} k="cdate" />
          <F info={p.info} k="deadline" /><F info={p.info} k="cal" opts={["", ...P().calendars.map(c => c.name)]} />
          <Show when={P().opts.res_cals}><F info={p.info} k="ignore_rc" opts={["No", "Yes"]} /><span /><span /></Show>
          <F info={p.info} k="priority" /><F info={p.info} k="responsible" />
          <RO info={p.info} k="wbs" /><RO info={p.info} k="tf" />
        </div>
      </Match>
      <Match when={p.tab === "status"}>
        <div class="dgrid">
          <F info={p.info} k="astart" /><F info={p.info} k="afinish" />
          <F info={p.info} k="pct" /><F info={p.info} k="phys" />
          <F info={p.info} k="rem" /><RO info={p.info} k="adur" />
          <RO info={p.info} k="b0start" /><RO info={p.info} k="b0finish" />
          <RO info={p.info} k="vstart" /><RO info={p.info} k="vfinish" />
          <RO info={p.info} k="es" /><RO info={p.info} k="ef" />
          <RO info={p.info} k="ls" /><RO info={p.info} k="lf" />
          <RO info={p.info} k="tf" /><RO info={p.info} k="ff" />
          <RO info={p.info} k="status" /><F info={p.info} k="leveldelay" />
        </div>
      </Match>
      <Match when={p.tab === "res"}>
        <table class="tbl">
          <thead><tr><th>Resource</th><th>Type</th><th>Units</th><th>Work</th><th>Cost</th><th /></tr></thead>
          <tbody>
            <For each={p.info.assigns}>{a => <tr><td>{a.name}</td><td>{a.kind}</td><td>{a.units}</td><td>{a.work}</td><td>{a.cost}</td>
              <td><button class="x" onClick={() => set(uid(), "res", p.info.assigns.filter(x => x.res !== a.res).map(x => `${x.name}[${x.units}]`).join(", "))}>×</button></td></tr>}</For>
            <tr class="add"><td colspan="6">Assign: <input list="resnames" placeholder="Resource[units]" onChange={e => { const v = e.currentTarget.value.trim(); if (v) set(uid(), "res", [...p.info.assigns.map(x => `${x.name}[${x.units}]`), v].join(", ")); e.currentTarget.value = ""; }} />
              <datalist id="resnames"><For each={P().resources}>{r => <option value={r.name} />}</For></datalist>
              <span class="muted"> Work · fixed cost </span><F info={p.info} k="fixedcost" /></td></tr>
          </tbody>
        </table>
      </Match>
      <Match when={p.tab === "preds"}><Rels info={p.info} list={p.info.preds} preds /></Match>
      <Match when={p.tab === "succs"}><Rels info={p.info} list={p.info.succs} preds={false} /></Match>
      <Match when={p.tab === "codes"}>
        <Show when={P().codes.length} fallback={<p class="empty">No activity codes yet. Add them in Project › Activity Codes.</p>}>
          <div class="dgrid"><For each={P().codes}>{c => <F info={p.info} k={`code:${c.name}`} label={c.name} opts={["", ...c.values.map(v => v.value)]} />}</For></div>
        </Show>
      </Match>
      <Match when={p.tab === "steps"}>
        <ListEditor uid={uid()} field="steps" items={t().steps ?? []} blank={{ name: "New step", weight: 1, done: false }} cols={[["name", "Step", "text"], ["weight", "Weight", "number"], ["done", "Done", "check"]]} />
        <p class="muted">With steps, physical % complete follows the weighted share of steps done.</p>
      </Match>
      <Match when={p.tab === "notebook"}>
        <ListEditor uid={uid()} field="notebook" items={t().notebook ?? []} blank={{ topic: "Topic", text: "" }} cols={[["topic", "Topic", "text"], ["text", "Text", "text"]]} />
      </Match>
      <Match when={p.tab === "expenses"}>
        <ListEditor uid={uid()} field="expenses" items={t().expenses ?? []} blank={{ name: "Expense", category: "", vendor: "", planned: 0, actual: 0, accrual: "Prorated" }}
          cols={[["name", "Expense", "text"], ["category", "Category", "text"], ["vendor", "Vendor", "text"], ["planned", `Planned (${P().currency.code})`, "number"], ["actual", "Actual", "number"], ["accrual", "Accrual", "select", ["Start", "Prorated", "End"]]]} />
      </Match>
      <Match when={p.tab === "docs"}>
        <ListEditor uid={uid()} field="docs" items={t().docs ?? []} blank={{ title: "Document", path: "", status: "", deliverable: false }} cols={[["title", "Title", "text"], ["path", "Location", "text"], ["status", "Status", "text"], ["deliverable", "Deliverable", "check"]]} />
        <div class="dgrid" style={{ "margin-top": "8px" }}><F info={p.info} k="hyperlink" wide /><F info={p.info} k="costaccount" /></div>
      </Match>
      <Match when={p.tab === "udf"}>
        <Show when={P().fields.length} fallback={<p class="empty">No custom fields defined. Right-click a column heading › Custom Fields.</p>}>
          <div class="dgrid"><For each={P().fields.filter(f => !f.key.startsWith("r"))}>{f => f.formula ? <RO info={p.info} k={`c:${f.key}`} label={f.alias || f.key} /> : <F info={p.info} k={`c:${f.key}`} label={f.alias || f.key} opts={f.lookup.length ? ["", ...f.lookup.map(l => l.value)] : undefined} />}</For></div>
        </Show>
      </Match>
      <Match when={p.tab === "risks"}>
        <table class="tbl"><thead><tr><th>Risk</th><th>Probability</th><th>Impact</th><th>Score</th><th>Status</th><th>Linked</th></tr></thead>
          <tbody><For each={P().risks}>{r => <tr><td>{r.name}</td><td>{r.prob}</td><td>{r.impact}</td><td>{r.prob * r.impact}</td><td>{r.status}</td>
            <td><input type="checkbox" checked={r.tasks.includes(uid())} onChange={e => act({ Risk: { id: r.id, risk: { ...r, tasks: e.currentTarget.checked ? [...r.tasks, uid()] : r.tasks.filter(x => x !== uid()) } } })} /></td></tr>}</For></tbody></table>
        <Show when={!P().risks.length}><p class="empty">No risks yet. Open the Risks view to add them.</p></Show>
      </Match>
      <Match when={p.tab === "notes"}>
        <textarea class="dnotes" value={t().notes ?? ""} onChange={e => set(uid(), "notes", e.currentTarget.value)} placeholder="Notes for this task" />
      </Match>
    </Switch>
  );
};

export const Details = () => {
  const [info] = useInfo(curUid);
  return (
    <section class="details">
      <div class="dhead"><Tabs tabs={TABS} on={detailsTab()} set={setDetailsTab} /><Help k="details" /><span class="grow" /><Show when={info()}>{i => <span class="muted">{i().task.code} {i().task.name}</span>}</Show></div>
      <div class="dbody">
        <Show when={info()} fallback={<p class="empty">{out()?.summary.tasks ? "Select a task to see its details." : "Add a task to get started."}</p>}>{i => <TaskTab info={i()} tab={detailsTab()} />}</Show>
      </div>
    </section>
  );
};
