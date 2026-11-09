// The Procurement Register module: sub-orders from enquiry to delivery, the vendor's promise
// against the date the plan needs each one, an expediting log, weighted progress and S-curves.
// The engine works everything out (procure.rs); linking orders to the Document Register is the
// user's choice (Settings).
import { For, Show, createMemo, createResource, createSignal } from "solid-js";
import { PItem, ProcOut, ProcRegister } from "./api";
import { act, call, grid, out, P, tick, setView, openDialog, toast } from "./store";
import { Page } from "./views";
import { Kpi } from "./reports";
import { ChartBox, COL, line } from "./charts";
import { Dialog, Seg } from "./ui";
import { showTab } from "./plans";
import { asDate, reg as docReg } from "./docreg";

const MS = ["RFQ issued", "PO placed", "Manufacturing start", "Inspection release (IRN)", "Delivered"];
const PRESETS: [string, string, number[]][] = [["balanced", "Balanced: most credit for manufacturing", [5, 15, 60, 10, 10]], ["order", "More credit at the order", [5, 25, 50, 10, 10]], ["delivery", "More credit at delivery", [5, 10, 50, 15, 20]]];
const DEF: ProcRegister = { preset: "balanced", weights: [5, 15, 60, 10, 10], weighting: "value", irn_days: 14, warn_days: 14, link_docs: false, items: [] };
export const preg = (): ProcRegister => ({ ...DEF, ...(P().procurement ?? {}) });
const save = (r: Partial<ProcRegister>) => act({ Procurement: { r: { ...preg(), ...r } } });
const blank = (id: number): PItem => ({ id, tag: "", name: "New order", vendor: "", origin: "", po_no: "", value: 0, manual_w: 1, long_lead: false, task: 0, need: 0, required: null, planned: [null, null, null, null, null], actual: [null, null, null, null, null], mfg_pct: 0, promised: null, log: [], docs: [], remarks: "" });
const next = (l: { id: number }[]) => Math.max(0, ...l.map(x => x.id)) + 1;
const dt = (v: string | null | undefined) => (v ? new Date(v + "T00:00").toLocaleDateString("en-GB", { day: "2-digit", month: "short", year: "2-digit" }) : "");
const today = () => new Date().toISOString().slice(0, 10);
const docsOn = () => (P().modules ?? []).includes("documents");
const ST: Record<string, string> = { Delivered: "good", "Inspection released": "good", "In manufacture": "", "PO placed": "", "RFQ issued": "", "Not started": "" };

// ---------- Settings ----------
export const procSettings = () => openDialog(() => {
  const [r, setR] = createSignal<ProcRegister>(JSON.parse(JSON.stringify(preg())));
  const put = (x: Partial<ProcRegister>) => setR({ ...r(), ...x });
  const w = () => PRESETS.find(p => p[0] === r().preset)?.[2] ?? r().weights;
  const total = () => w().reduce((a, b) => a + b, 0);
  return (
    <Dialog title="Procurement Register Settings" wide help="procurement" okText="Save" ok={() => { if (Math.abs(total() - 100) > 0.01) { toast("The milestone weights must add up to 100 %."); return false; } save(r()); }}>
      <div class="dset">
        <div>
          <h3>Milestone weights (rules of credit)</h3>
          <For each={PRESETS}>{p => <label class="chk"><input type="radio" checked={r().preset === p[0]} onChange={() => put({ preset: p[0] })} /> {p[1]}</label>}</For>
          <label class="chk"><input type="radio" checked={!PRESETS.some(p => p[0] === r().preset)} onChange={() => put({ preset: "own", weights: [...w()] })} /> My own weights</label>
          <table class="tbl"><thead><tr><th>Milestone</th><th class="r">%</th></tr></thead><tbody>
            <For each={MS}>{(m, i) => <tr><td>{m === "Manufacturing start" ? "Manufacturing (pro rata to its %)" : m}</td><td class="r"><input type="number" min="0" max="100" style={{ width: "60px" }} disabled={PRESETS.some(p => p[0] === r().preset)} value={w()[i()]} onChange={e => put({ weights: w().map((v, j) => (j === i() ? +e.currentTarget.value || 0 : v)) })} /></td></tr>}</For>
            <tr class="tot"><td><b>Total</b></td><td class="r" classList={{ bad: Math.abs(total() - 100) > 0.01 }}><b>{total()}</b></td></tr></tbody></table>
        </div>
        <div>
          <h3>Orders weigh against each other by</h3>
          <For each={[["value", "Their PO value (equal when no values are typed)"], ["equal", "Equally"], ["manual", "A weight typed for each"]]}>{([k, l]) => <label class="chk"><input type="radio" checked={r().weighting === k} onChange={() => put({ weighting: k })} /> {l}</label>}</For>
          <h3>Dates</h3>
          <div class="prow"><label>Inspection release (days before delivery, when not typed)<input type="number" min="0" value={r().irn_days} onChange={e => put({ irn_days: +e.currentTarget.value || 0 })} /></label>
            <label>Amber when the float is under (days)<input type="number" min="0" value={r().warn_days} onChange={e => put({ warn_days: +e.currentTarget.value || 0 })} /></label></div>
          <h3>Vendor documents</h3>
          <label class="chk"><input type="checkbox" checked={r().link_docs} onChange={e => put({ link_docs: e.currentTarget.checked })} /> Link orders to the Document Register</label>
          <p class="muted">{r().link_docs ? (docsOn() ? "Each order can list the vendor documents it waits for: the register shows how many are accepted and warns when drawings hold up manufacturing." : "Switch on the Document Register in Project › Modules for the links to count.") : "Off: orders stand on their own, without vendor documents."}</p>
        </div>
      </div>
    </Dialog>
  );
});

// ---------- An order ----------
const editItem = (it: PItem) => openDialog(() => {
  const [x, setX] = createSignal<PItem>(JSON.parse(JSON.stringify(it)));
  const put = (v: Partial<PItem>) => setX({ ...x(), ...v });
  const tasks = () => grid().rows.filter(t => t.uid > 0);
  const [dq, setDq] = createSignal("");
  const T = (p: { l: string; k: "tag" | "name" | "vendor" | "origin" | "po_no" | "remarks" }) => <label>{p.l}<input value={x()[p.k]} onChange={e => put({ [p.k]: e.currentTarget.value } as Partial<PItem>)} /></label>;
  const TaskSel = (p: { l: string; v: number; set: (v: number) => void }) => <label>{p.l}<select value={String(p.v)} onChange={e => p.set(+e.currentTarget.value)}><option value="0">—</option><For each={tasks()}>{t => <option value={String(t.uid)}>{t.id} · {t.name}</option>}</For></select></label>;
  const setDate = (k: "planned" | "actual", i: number, v: string) => put({ [k]: x()[k].map((d, j) => (j === i ? v || null : d)) } as Partial<PItem>);
  return (
    <Dialog title={`Order ${x().tag || x().name}`} wide help="procurement" okText="Save" ok={() => save({ items: preg().items.map(y => (y.id === it.id ? x() : y)) })}>
      <div class="dform">
        <T l="Item no." k="tag" /><T l="Order / equipment" k="name" /><T l="Sub-vendor" k="vendor" /><T l="Country of origin" k="origin" /><T l="PO number" k="po_no" />
        <label>PO value<input type="number" min="0" value={x().value || ""} onChange={e => put({ value: +e.currentTarget.value || 0 })} /></label>
        <Show when={preg().weighting === "manual"}><label>Weight<input type="number" min="0" value={x().manual_w} onChange={e => put({ manual_w: +e.currentTarget.value || 0 })} /></label></Show>
        <TaskSel l="Task for its manufacture & delivery" v={x().task} set={task => put({ task })} />
        <TaskSel l="Task that needs it" v={x().need} set={need => put({ need })} />
        <label>…or required on<input type="date" value={x().required ?? ""} onChange={e => put({ required: e.currentTarget.value || null })} /></label>
        <label>Manufacturing %<input type="number" min="0" max="100" value={x().mfg_pct} onChange={e => put({ mfg_pct: +e.currentTarget.value || 0 })} /></label>
        <label>Vendor's promised delivery<input type="date" value={x().promised ?? ""} onChange={e => put({ promised: e.currentTarget.value || null })} /></label>
        <T l="Remarks" k="remarks" />
      </div>
      <label class="chk" style={{ "margin-top": "8px" }}><input type="checkbox" checked={x().long_lead} onChange={e => put({ long_lead: e.currentTarget.checked })} /> Long-lead item</label>
      <table class="tbl" style={{ "margin-top": "8px" }}><thead><tr><th>Milestone</th><th>Planned</th><th>Actual</th></tr></thead>
        <tbody><For each={MS}>{(m, i) => <tr><td>{m}</td><td><input type="date" value={x().planned[i()] ?? ""} onChange={e => setDate("planned", i(), e.currentTarget.value)} /></td><td><input type="date" value={x().actual[i()] ?? ""} onChange={e => setDate("actual", i(), e.currentTarget.value)} /></td></tr>}</For></tbody></table>
      <p class="muted">Empty planned dates come from the task for its manufacture and delivery (start and finish); the inspection release from the delivery less the days in Settings.</p>
      <Show when={preg().link_docs}>
        <h3>Vendor documents it waits for</h3>
        <Show when={docsOn() && docReg().docs.length} fallback={<p class="muted">The Document Register is off or empty in this plan.</p>}>
          <input placeholder="Find a document…" value={dq()} onInput={e => setDq(e.currentTarget.value)} style={{ width: "240px" }} />
          <div class="pdocs"><For each={docReg().docs.filter(d => !dq() || `${d.number} ${d.title} ${d.vendor}`.toLowerCase().includes(dq().toLowerCase()))}>{d =>
            <label class="chk"><input type="checkbox" checked={x().docs.includes(d.id)} onChange={e => put({ docs: e.currentTarget.checked ? [...x().docs, d.id] : x().docs.filter(y => y !== d.id) })} /> <span class="mono">{d.number}</span> {d.title}</label>}</For></div>
        </Show>
      </Show>
    </Dialog>
  );
});

// ---------- Import ----------
const FIELDS: [string, string, RegExp][] = [["tag", "Item no.", /^item$|item no|^no\.?$|s\.?no/i], ["name", "Order / equipment", /order|equipment|description|item name/i], ["vendor", "Sub-vendor", /vendor|supplier/i], ["origin", "Country of origin", /origin|country/i],
  ["po_no", "PO number", /po no|order no/i], ["value", "PO value", /value|price|amount/i],
  ...MS.map((m, i) => [`p${i}`, `Planned: ${m}`, [/plan.*rfq|^rfq issue$/i, /plan.*po\b|po placement/i, /plan.*(mfg|manuf)/i, /plan.*(irn|ready)|ready to ship/i, /plan.*deliver|^delivery to works$/i][i]] as [string, string, RegExp]),
  ...MS.map((m, i) => [`a${i}`, `Actual: ${m}`, new RegExp(`(actual.*)?^(${["rfq issued", "po placed", "mfg start|manufactur.*start", "irn date|irn", "delivered"][i]})`, "i")] as [string, string, RegExp]),
  ["mfg_pct", "Manufacturing %", /mfg %|manufactur.*%/i], ["promised", "Promised / forecast delivery", /forecast deliv|promised/i], ["remarks", "Remarks / expediting notes", /remark|note/i]];

export const procImport = async () => {
  const first = await call<{ path: string; names: string[]; rows: string[][] } | null>("docs_read", {});
  if (!first) return;
  openDialog(() => {
    const [data, setData] = createSignal(first);
    const guessHead = (rows: string[][]) => { const sc = rows.slice(0, 30).map(r => r.filter(c => c.length < 60 && FIELDS.some(([, , re]) => re.test(c))).length); return Math.max(0, sc.indexOf(Math.max(...sc))); };
    const [head, setHead] = createSignal(guessHead(first.rows));
    const headers = () => data().rows[head()] ?? [];
    // Planned and actual columns often share a heading (planned first): a second "Mfg Start" makes the first one planned.
    const guess = () => headers().reduce<string[]>((m, h) => {
      const f = FIELDS.find(([k, , re]) => re.test(h) && !m.includes(k));
      const twice = !f && FIELDS.find(([k, , re]) => k[0] === "a" && re.test(h) && m.includes(k) && !m.includes("p" + k.slice(1)));
      return twice ? [...m.map(x => (x === twice[0] ? "p" + x.slice(1) : x)), twice[0]] : [...m, f ? f[0] : ""];
    }, []);
    const [map, setMap] = createSignal<string[]>(guess());
    const body = () => data().rows.slice(head() + 1).filter(r => r.some(c => c.trim()));
    const sheet = async (name: string) => { const d = await call<typeof first>("docs_read", { path: data().path, sheet: name }); if (d) { setData(d); setHead(guessHead(d.rows)); setMap(guess()); } };
    const col = (r: string[], f: string) => { const j = map().indexOf(f); return j >= 0 ? (r[j] ?? "").trim() : ""; };
    const run = () => {
      if (!map().includes("name") && !map().includes("tag")) { toast("Choose the column with the order or the item number."); return false; }
      const items = [...preg().items]; let id = next(items), added = 0, changed = 0;
      for (const r of body()) {
        const tag = col(r, "tag"), name = col(r, "name"); if (!tag && !name) continue;
        const at = items.findIndex(x => (tag && x.tag === tag) || (!tag && x.name === name));
        const o = at >= 0 ? items[at] : blank(id);
        const dates = (k: "p" | "a", old: (string | null)[]) => old.map((d, i) => asDate(col(r, `${k}${i}`)) ?? d);
        const pct = col(r, "mfg_pct").replace("%", "");
        const n: PItem = { ...o, tag: tag || o.tag, name: name || o.name, vendor: col(r, "vendor") || o.vendor, origin: col(r, "origin") || o.origin, po_no: col(r, "po_no") || o.po_no, value: +col(r, "value").replace(/[^\d.]/g, "") || o.value,
          planned: dates("p", o.planned), actual: dates("a", o.actual), mfg_pct: pct ? Math.min(100, (+pct <= 1 && pct.includes(".") ? +pct * 100 : +pct) || 0) : o.mfg_pct, promised: asDate(col(r, "promised")) ?? o.promised, remarks: col(r, "remarks") || o.remarks };
        if (at >= 0) { items[at] = n; changed++; } else { items.push(n); id++; added++; }
      }
      save({ items }); toast(`${added} orders added, ${changed} updated.`, true);
    };
    return (
      <Dialog title="Import into the Procurement Register" wide help="procurement" okText="Import" ok={run}>
        <div class="prow"><Show when={data().names.length > 1}><label>Sheet<select onChange={e => sheet(e.currentTarget.value)}><For each={data().names}>{n => <option>{n}</option>}</For></select></label></Show>
          <label>Headings are in row<input type="number" min="1" value={head() + 1} onChange={e => { setHead(Math.max(0, +e.currentTarget.value - 1)); setMap(guess()); }} /></label></div>
        <p class="muted">{body().length} rows under the headings. Orders with the same item number are updated. Columns left on "—" are not read.</p>
        <div class="dmap"><table class="tbl"><thead><tr><th>Column</th><th>Holds</th><th>First rows</th></tr></thead>
          <tbody><For each={headers()}>{(h, j) => <tr><td><b>{h || `(column ${j() + 1})`}</b></td>
            <td><select value={map()[j()] ?? ""} onChange={e => setMap(headers().map((_, k) => (k === j() ? e.currentTarget.value : (map()[k] === e.currentTarget.value && e.currentTarget.value ? "" : map()[k] ?? ""))))}><option value="">—</option><For each={FIELDS}>{f => <option value={f[0]}>{f[1]}</option>}</For></select></td>
            <td class="muted">{body().slice(0, 3).map(r => r[j()] ?? "").filter(Boolean).join(" · ").slice(0, 80)}</td></tr>}</For></tbody></table></div>
      </Dialog>
    );
  });
};

// ---------- The view ----------
export const ProcurementView = () => {
  const [data] = createResource(() => [tick(), out()?.tab] as const, () => call<{ tab: number; name: string; on: boolean; out: ProcOut }[]>("procurement"));
  const me = () => data()?.find(d => d.tab === out()?.tab)?.out;
  const [tab, setTab] = createSignal("register"), [q, setQ] = createSignal(""), [rag, setRag] = createSignal(""), [per, setPer] = createSignal<"weekly" | "monthly">("monthly");
  const rowOf = (id: number) => me()?.items.find(x => x.id === id);
  const items = createMemo(() => preg().items.filter(it => { const r = rowOf(it.id), t = q().toLowerCase(); return (!t || `${it.tag} ${it.name} ${it.vendor} ${it.po_no}`.toLowerCase().includes(t)) && (!rag() || r?.rag === rag()); }));
  const put = (it: PItem) => save({ items: preg().items.map(x => (x.id === it.id ? it : x)) });
  const [note, setNote] = createSignal({ id: 0, date: today(), note: "", by: "" });
  const addNote = () => { const n = note(), it = preg().items.find(x => x.id === n.id); if (!it || !n.note.trim()) return toast("Choose an order and write the note."); put({ ...it, log: [...it.log, { date: n.date || null, note: n.note.trim(), by: n.by }] }); setNote({ ...n, note: "" }); };
  const notes = () => preg().items.flatMap(it => it.log.map((x, k) => ({ it, x, k }))).sort((a, b) => (b.x.date ?? "").localeCompare(a.x.date ?? ""));
  const by = (key: (it: PItem) => string) => { const m = new Map<string, { n: number; red: number; done: number; e: number; w: number }>(); for (const it of preg().items) { const r = rowOf(it.id); if (!r) continue; const k = key(it) || "(none)", g = m.get(k) ?? { n: 0, red: 0, done: 0, e: 0, w: 0 }; g.n++; if (r.rag === "red") g.red++; if (r.actual[4]) g.done++; g.e += r.weight * r.earned; g.w += r.weight; m.set(k, g); } return [...m].sort((a, b) => b[1].n - a[1].n); };
  return (
    <Page title="Procurement Register" help="procurement" tools={<>
      <button class="btn" onClick={procSettings}>Settings…</button><button class="btn" onClick={procImport}>Import…</button>
      <button class="btn" onClick={async () => { const f = await call<string | null>("proc_export"); if (f) toast(`Saved ${f}`, true); }}>Export to Workbook</button>
      <button class="btn pri" onClick={() => { const id = next(preg().items); save({ items: [...preg().items, blank(id)] }).then(() => editItem(preg().items.find(x => x.id === id) ?? blank(id))); }}>+ Order</button>
    </>}>
      <Show when={me()}>{o => <>
        <div class="kpis">
          <Kpi v={String(preg().items.length)} l={`orders · ${preg().items.filter(x => x.long_lead).length} long lead`} />
          <Kpi v={`${o().items.filter(x => x.actual[1]).length} / ${o().items.length}`} l="purchase orders placed" />
          <Kpi v={`${o().items.filter(x => x.actual[4]).length} / ${o().items.length}`} l="delivered" />
          <Kpi v={`${o().items.filter(x => x.rag === "red").length} · ${o().items.filter(x => x.rag === "amber").length}`} l="late for the plan · little float" bad={o().items.some(x => x.rag === "red")} />
          <Kpi v={`${o().earned.toFixed(1)} %`} l={`earned · planned ${o().planned.toFixed(1)} %`} bad={o().earned < o().planned - 0.05} good={o().earned >= o().planned && o().planned > 0} />
          <Show when={preg().link_docs}><Kpi v={String(o().items.filter(x => x.docs_late).length)} l="held up by vendor documents" bad={o().items.some(x => x.docs_late)} /></Show>
        </div>
        <Show when={!o().weights_ok}><p class="dwarn">The milestone weights do not add up to 100 %: Settings.</p></Show>
        <div class="dtabs"><Seg value={tab()} set={setTab} options={[["register", "Register"], ["log", "Expediting log"], ["progress", "Progress"]]} /></div>
        <Show when={tab() === "register"}>
          <div class="panel ctr">
            <div class="phead"><input placeholder="Search item, order, vendor, PO…" value={q()} onInput={e => setQ(e.currentTarget.value)} style={{ width: "240px" }} />
              <select value={rag()} onChange={e => setRag(e.currentTarget.value)}><option value="">Every order</option><option value="red">Late for the plan</option><option value="amber">Little float</option><option value="green">On time</option></select>
              <span class="muted">{items().length} shown</span></div>
            <table class="tbl dreg preg"><thead><tr><th>Item</th><th>Order / equipment</th><th>Sub-vendor</th><th>PO No.</th><th>PO planned</th><th>PO placed</th><th class="r">Mfg %</th><th>Delivery planned</th><th>Promised</th><th>Forecast</th><th class="r">Var.</th><th>Required</th><th class="r">Float</th><th>Status</th><th>Next</th><th>Done on</th><th class="r">Earned</th><Show when={preg().link_docs}><th>Docs</th></Show><th /></tr></thead>
              <tbody><For each={items()}>{it => { const r = () => rowOf(it.id); const k = () => [0, 1, 2, 3, 4].find(i => !it.actual[i]); return (
                <tr classList={{ key: it.long_lead }}>
                  <td><input style={{ width: "60px" }} value={it.tag} onChange={e => put({ ...it, tag: e.currentTarget.value })} /></td>
                  <td><input value={it.name} onChange={e => put({ ...it, name: e.currentTarget.value })} /></td>
                  <td><input style={{ width: "90px" }} value={it.vendor} onChange={e => put({ ...it, vendor: e.currentTarget.value })} /></td>
                  <td><input style={{ width: "90px" }} value={it.po_no} onChange={e => put({ ...it, po_no: e.currentTarget.value })} /></td>
                  <td class="muted">{dt(r()?.planned[1])}</td><td>{dt(r()?.actual[1])}</td>
                  <td class="r"><input type="number" min="0" max="100" style={{ width: "54px" }} value={it.mfg_pct} onChange={e => put({ ...it, mfg_pct: +e.currentTarget.value || 0 })} /></td>
                  <td class="muted">{dt(r()?.planned[4])}</td>
                  <td><input type="date" value={it.promised ?? ""} onChange={e => put({ ...it, promised: e.currentTarget.value || null })} /></td>
                  <td>{dt(r()?.forecast)}</td><td class="r" classList={{ bad: (r()?.var ?? 0) > 0 }}>{r()?.var ? (r()!.var! > 0 ? `+${r()!.var}` : r()!.var) : ""}</td>
                  <td title={r()?.need_name}>{dt(r()?.required)}</td>
                  <td class="r"><span class={`st ${r()?.rag === "red" ? "bad" : r()?.rag === "amber" ? "warn" : r()?.rag === "green" ? "good" : ""}`}>{r()?.float ?? ""}</span></td>
                  <td><span class={`st ${ST[r()?.status ?? ""] ?? ""}`}>{r()?.status}</span></td>
                  <td classList={{ bad: (r()?.days_late ?? 0) > 0 }} title={r()?.next_date ? `Planned ${dt(r()?.next_date)}` : ""}>{r()?.next}{(r()?.days_late ?? 0) > 0 ? ` (${r()!.days_late} d late)` : ""}</td>
                  <td><Show when={k() !== undefined}><input type="date" title={`Mark "${MS[k()!]}" done on this date`} value="" onChange={e => { const v = e.currentTarget.value; if (v) put({ ...it, actual: it.actual.map((d, i) => (i === k() ? v : d)), mfg_pct: k() === 3 || k() === 4 ? 100 : it.mfg_pct }); }} /></Show></td>
                  <td class="r">{(r()?.earned ?? 0).toFixed(0)} %</td>
                  <Show when={preg().link_docs}><td classList={{ bad: !!r()?.docs_late }} title={r()?.docs_late ? "Vendor documents not accepted when manufacturing should start" : ""}>{r()?.docs ? `${r()!.docs_ok}/${r()!.docs}` : "—"}</td></Show>
                  <td class="acts"><button class="x" title="Edit" onClick={() => editItem(it)}>✎</button><button class="x" title="Add an expediting note" onClick={() => { setNote({ ...note(), id: it.id }); setTab("log"); }}>✉</button><button class="x" title="Remove" onClick={() => save({ items: preg().items.filter(x => x.id !== it.id) })}>×</button></td>
                </tr>); }}</For></tbody></table>
            <Show when={!preg().items.length}><p class="empty">No orders yet. <b>Import…</b> a procurement status register from a workbook (.xlsx), or add them with <b>+ Order</b>. Link each to the task for its manufacture and delivery and to the task that needs it.</p></Show>
          </div>
        </Show>
        <Show when={tab() === "log"}>
          <div class="panel ctr">
            <div class="phead"><select value={String(note().id)} onChange={e => setNote({ ...note(), id: +e.currentTarget.value })}><option value="0">— order —</option><For each={preg().items}>{it => <option value={String(it.id)}>{it.tag} {it.name}</option>}</For></select>
              <input type="date" value={note().date} onChange={e => setNote({ ...note(), date: e.currentTarget.value })} />
              <input placeholder="What the vendor said, what was agreed…" style={{ "flex-grow": 1 }} value={note().note} onInput={e => setNote({ ...note(), note: e.currentTarget.value })} />
              <input placeholder="By" style={{ width: "100px" }} value={note().by} onInput={e => setNote({ ...note(), by: e.currentTarget.value })} /><button class="btn sm pri" onClick={addNote}>Add note</button></div>
            <table class="tbl"><thead><tr><th>Date</th><th>Order</th><th>Note</th><th>By</th><th /></tr></thead>
              <tbody><For each={notes()}>{n => <tr><td>{dt(n.x.date)}</td><td><b>{n.it.tag}</b> {n.it.name}</td><td class="wrap">{n.x.note}</td><td>{n.x.by}</td><td><button class="x" onClick={() => put({ ...n.it, log: n.it.log.filter((_, k) => k !== n.k) })}>×</button></td></tr>}</For></tbody></table>
            <Show when={!notes().length}><p class="empty">No expediting notes yet.</p></Show>
          </div>
        </Show>
        <Show when={tab() === "progress"}>
          <div class="phead"><Seg value={per()} set={v => setPer(v as "monthly")} options={[["weekly", "Weekly"], ["monthly", "Monthly"]]} /></div>
          <Show when={o()[per()].length > 1} fallback={<p class="empty">Give the orders planned dates (or link them to tasks) to see the S-curve.</p>}>
            <ChartBox title="Procurement progress (weighted)" type="line" fmt="pct" max={100} labels={o()[per()].map(p => dt(p.end))} days={o()[per()].map(p => Math.round(Date.parse(p.end) / 864e5))}
              datasets={[line("Planned", o()[per()].map(p => p.planned), COL.grey, [5, 4]), line("Actual", o()[per()].map(p => p.actual), COL.blue)]} />
          </Show>
          <div class="cols2"><For each={[["By sub-vendor", (it: PItem) => it.vendor], ["By country of origin", (it: PItem) => it.origin]] as [string, (it: PItem) => string][]}>{([t, f]) =>
            <div class="panel ctr"><h2>{t}</h2><table class="tbl"><thead><tr><th>{t.replace("By ", "")}</th><th class="r">Orders</th><th class="r">Late</th><th class="r">Delivered</th><th class="r">Earned</th></tr></thead>
              <tbody><For each={by(f)}>{([k, g]) => <tr><td>{k}</td><td class="r">{g.n}</td><td class="r" classList={{ bad: g.red > 0 }}>{g.red}</td><td class="r">{g.done}</td><td class="r">{g.w ? (g.e / g.w).toFixed(1) : "—"} %</td></tr>}</For></tbody></table></div>}</For></div>
        </Show>
      </>}</Show>
      <Show when={(data()?.length ?? 0) > 1}>
        <div class="panel ctr"><h2>The project's plans</h2>
          <table class="tbl"><thead><tr><th>Plan</th><th class="r">Orders</th><th class="r">PO placed</th><th class="r">Delivered</th><th class="r">Late</th><th class="r">Earned</th><th class="r">Planned</th><th /></tr></thead>
            <tbody><For each={data()}>{d => <tr><td><b>{d.name}</b>{d.tab === out()?.tab ? " (this plan)" : ""}{!d.on && <span class="muted"> · module off</span>}</td><td class="r">{d.out.items.length}</td><td class="r">{d.out.items.filter(x => x.actual[1]).length}</td><td class="r">{d.out.items.filter(x => x.actual[4]).length}</td>
              <td class="r">{d.out.items.filter(x => x.rag === "red").length}</td><td class="r">{d.out.earned.toFixed(1)} %</td><td class="r">{d.out.planned.toFixed(1)} %</td>
              <td>{d.tab !== out()?.tab && <button class="btn sm" onClick={() => showTab(d.tab).then(() => setView("procurement"))}>Open</button>}</td></tr>}</For></tbody></table>
        </div>
      </Show>
    </Page>
  );
};
