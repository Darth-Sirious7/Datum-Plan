// Views other than the Gantt chart.
import { CountryList, Flag, countryCode, countryName } from "./flags";
import { For, Index, Show, createSignal, createResource, createMemo, createEffect, JSX } from "solid-js";
import { Net, Row, Usage, Check, Resource, fmtDay, dayDate, dateDay, money } from "./api";
import { call, grid, out, P, query, tick, sel, setSel, act, setFilter, setView, openMenu, toast, setBackstage, loadOut } from "./store";
import { C, selectUid } from "./cmds";
import { Seg, Icon, Check as Chk } from "./ui";
import { Help } from "./help";

export const Page = (p: { title: string; help?: string; tools?: JSX.Element; children: JSX.Element }) => (
  <section class="page"><div class="ptools"><h1>{p.title}{p.help && <Help k={p.help} />}</h1><span class="grow" />{p.tools}</div>{p.children}</section>
);

// ---------- Network Diagram (one box per task) ----------
/** Box sizes for the three box layouts. */
export const NET_BOX: Record<string, { W: number; H: number }> = { compact: { W: 160, H: 50 }, standard: { W: 214, H: 72 }, detailed: { W: 230, H: 104 } };
const NET_COLS = ["code", "name", "dur", "start", "finish", "pct", "tf", "res"];
export const [netBox, setNetBox] = createSignal<"compact" | "standard" | "detailed">("standard");
/** Show summary tasks as boxes, and lay the network out in WBS bands (one per top-level phase). */
export const [netSums, setNetSums] = createSignal(false), [netWbs, setNetWbs] = createSignal(false);
export const loadNet = () => call<Net>("network", { q: query(), summaries: netSums() || netWbs(), wbs: netWbs() });
/** One network box: shape by type, outline by criticality, cross or slash for progress, amber flag when late. */
export const NetBox = (p: { v: (k: string) => string; r: Row; W: number; H: number; mode: string; sel?: boolean; dim?: boolean }) => {
  const done = p.r.bar.pct >= 100, prog = p.r.bar.pct > 0 && !done, sum = p.r.kind !== "task", W = p.W, H = p.H;
  const name = p.r.name.length > (W / 6.4 | 0) ? p.r.name.slice(0, (W / 6.4 | 0) - 1) + "…" : p.r.name;
  const lines: [string, string][][] = p.mode === "compact" ? [[["ID", String(p.r.id)], ["Dur", p.v("dur")]]]
    : [[["Start", p.v("start")], ["ID", String(p.r.id)]], [["Finish", p.v("finish")], ["Dur", p.v("dur")]], ...(p.mode === "detailed" ? [[["Float", p.v("tf")], ["Done", p.v("pct")]] as [string, string][], [["Res", p.v("res")], ["", ""]] as [string, string][]] : [])];
  return (
    <g class="nbox" classList={{ crit: p.r.crit, ms: p.r.ms, sum, sel: p.sel, dim: p.dim }}>
      {p.r.ms ? <path class="shape" d={`M12 0H${W - 12}L${W} ${H / 2}L${W - 12} ${H}H12L0 ${H / 2}z`} />
        : sum ? <path class="shape" d={`M8 0H${W}L${W - 8} ${H}H0z`} /> : <rect class="shape" width={W} height={H} rx="6" />}
      {!p.r.ms && !sum && <rect class="accent" x="0" y="0" width="4" height={H} rx="2" />}
      <Show when={done || prog}><path d={done ? `M4 4L${W - 4} ${H - 4}M${W - 4} 4L4 ${H - 4}` : `M4 ${H - 4}L${W - 4} 4`} class="x" /></Show>
      {p.r.late && <path d={`M${W - 14} 0H${W}V14z`} fill="#F2B233"><title>Late: finishes after its baseline or deadline</title></path>}
      <text x="12" y="16" class="t">{p.v("code") ? `${p.v("code")} ` : ""}{name}<title>{p.r.name}</title></text>
      <line x1="0" x2={W} y1="23" y2="23" />
      <For each={lines}>{(l, i) => <><text x="12" y={39 + i() * 16}>{l[0][0] && `${l[0][0]}: `}{l[0][1]}</text><text x={W * 0.6} y={39 + i() * 16}>{l[1][0] && `${l[1][0]}: `}{l[1][1]}</text></>}</For>
    </g>
  );
};
/** The legend: what the box shapes, outlines and link labels mean. */
export const NetLegend = () => (
  <div class="netleg">
    <b>Legend</b>
    <svg width="560" height="44" class="net">
      <g transform="translate(0,4)"><g class="nbox crit"><rect class="shape" width="34" height="18" rx="3" /></g><text x="40" y="13">Critical</text></g>
      <g transform="translate(96,4)"><g class="nbox"><rect class="shape" width="34" height="18" rx="3" /></g><text x="40" y="13">Not critical</text></g>
      <g transform="translate(200,4)"><g class="nbox"><path class="shape" d="M6 0H28L34 9L28 18H6L0 9z" /></g><text x="40" y="13">Milestone</text></g>
      <g transform="translate(296,4)"><g class="nbox"><rect class="shape" width="34" height="18" rx="3" /><path class="x" d="M3 3L31 15M31 3L3 15" /></g><text x="40" y="13">Completed</text></g>
      <g transform="translate(400,4)"><g class="nbox"><rect class="shape" width="34" height="18" rx="3" /><path class="x" d="M3 15L31 3" /></g><text x="40" y="13">In progress</text></g>
      <g transform="translate(0,28)"><path d="M0 6H30" stroke="#D64545" stroke-width="2.6" /><text x="40" y="10">Driving link (sets the date)</text></g>
      <g transform="translate(200,28)"><path d="M0 6H30" stroke="#6E7F99" /><text x="40" y="10">Link</text></g>
      <g transform="translate(296,28)"><path d="M0 0H14V14z" fill="#F2B233" /><text x="22" y="10">Late</text></g>
    </svg>
    <span class="muted">Labels on links: <b>FS</b> Finish-to-Start (the usual, not labelled) · <b>SS</b> Start-to-Start · <b>FF</b> Finish-to-Finish · <b>SF</b> Start-to-Finish · <b>+2d</b> lag, <b>−2d</b> lead. <button class="help" title="Glossary" onClick={() => import("./dialogs").then(d => d.glossary())}>?</button></span>
  </div>
);

/** Links and boxes of a network, shared by the view and the printout. */
export const NetDraw = (p: { n: Net; rows: Map<number, Row>; mode: string; chain?: Set<number> | null; sel?: number[]; click?: (uid: number) => void }) => {
  const box = () => NET_BOX[p.mode], GX = () => box().W + 54, GY = () => box().H + 28;
  const pos = (n: { col: number; row: number }) => [20 + n.col * GX(), 20 + n.row * GY()];
  const at = createMemo(() => new Map(p.n.nodes.map(n => [n.uid, pos(n)])));
  const v = (r: Row) => (k: string) => r.cells[NET_COLS.indexOf(k)] ?? "";
  return <>
    <For each={p.n.bands ?? []}>{(b, i) => <g class="nband"><rect x="6" y={20 + b.row * GY() - 12} width={netSize(p.n, p.mode)[0] - 12} height={b.rows * GY() - 6} rx="8" classList={{ alt: i() % 2 === 1 }} /><text x="16" y={20 + b.row * GY() - 1}>{b.name}</text></g>}</For>
    <defs><For each={[["ar", "#6E7F99"], ["arc", "#D64545"]]}>{([id, c]) => <marker id={id} viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0 0L8 4L0 8z" fill={c} /></marker>}</For></defs>
    <For each={p.n.links}>{l => {
      const a = at().get(l.from), b = at().get(l.to);
      if (!a || !b) return null;
      const { W, H } = box(), fromS = l.kind[0] === "S", toF = l.kind[1] === "F";
      const x1 = fromS ? a[0] : a[0] + W, y1 = a[1] + H / 2, x2 = toF ? b[0] + W : b[0], y2 = b[1] + H / 2, mx = toF ? Math.max(x1, x2) + 16 : x2 - 18;
      const lab = `${l.kind === "FS" ? "" : l.kind}${l.lag ? (l.lag.startsWith("-") ? l.lag.replace("-", "−") : `+${l.lag}`) : ""}`;
      const dim = () => !!p.chain && !(p.chain.has(l.from) && p.chain.has(l.to));
      return <g class="nlink" classList={{ dim: dim() }}>
        <path d={`M${x1} ${y1}H${mx}V${y2}H${x2 + (toF ? 1 : -1)}`} fill="none" stroke={l.crit ? "#D64545" : "#6E7F99"} stroke-width={l.driving ? 2.4 : 1} marker-end={`url(#${l.crit ? "arc" : "ar"})`}>
          <title>{`${p.rows.get(l.from)?.name ?? ""} → ${p.rows.get(l.to)?.name ?? ""}: ${{ FS: "Finish-to-Start", SS: "Start-to-Start", FF: "Finish-to-Finish", SF: "Start-to-Finish" }[l.kind]}${l.lag ? `, lag ${l.lag}` : ""}${l.driving ? " (driving)" : ""}`}</title></path>
        {lab && <text x={mx + 3} y={(y1 + y2) / 2 + (y1 === y2 ? -4 : 0)} class="nlab">{lab}</text>}
      </g>;
    }}</For>
    <For each={p.n.nodes}>{nd => <Show when={p.rows.get(nd.uid)}>{r => { const [x, y] = pos(nd); return (
      <g transform={`translate(${x},${y})`} onClick={() => p.click?.(nd.uid)} onDblClick={() => p.click && C.info()}>
        <NetBox v={v(r())} r={r()} W={box().W} H={box().H} mode={p.mode} sel={p.sel?.includes(nd.uid)} dim={!!p.chain && !p.chain.has(nd.uid)} />
      </g>); }}</Show>}</For>
  </>;
};
export const netSize = (n: Net, mode: string) => [n.cols * (NET_BOX[mode].W + 54) + 40, n.rows * (NET_BOX[mode].H + 28) + 40];
export { NET_COLS };

export const Network = () => {
  const [scale, setScale] = createSignal(1), [legend, setLegend] = createSignal(true), [path, setPath] = createSignal(true);
  const [net] = createResource(() => [tick(), JSON.stringify(query()), netSums(), netWbs()], loadNet);
  const [data] = createResource(() => [tick(), JSON.stringify(query())], () => call<{ rows: Row[] }>("query", { q: { ...query(), cols: NET_COLS } }));
  const rows = createMemo(() => new Map((data()?.rows ?? []).map(r => [r.uid, r])));
  const box = () => NET_BOX[netBox()], GX = () => box().W + 54, GY = () => box().H + 28;
  const pos = (n: { col: number; row: number }) => [20 + n.col * GX(), 20 + n.row * GY()];
  const size = () => (net() ? netSize(net()!, netBox()) : [100, 100]);
  // The selected task's whole chain (all predecessors and successors), for "Highlight path".
  const chain = createMemo(() => {
    const s = sel(); if (!path() || s.length !== 1 || !net()) return null;
    const out = new Set([s[0]]), links = net()!.links;
    for (const dir of [0, 1]) { const q = [s[0]]; while (q.length) { const u = q.pop()!; for (const l of links) { const [a, b] = dir ? [l.to, l.from] : [l.from, l.to]; if (a === u && !out.has(b)) { out.add(b); q.push(b); } } } }
    return out;
  });
  let wrap!: HTMLDivElement;
  const [vp, setVp] = createSignal([0, 0, 1, 1]);
  const track = () => wrap && setVp([wrap.scrollLeft / scale(), wrap.scrollTop / scale(), wrap.clientWidth / scale(), wrap.clientHeight / scale()]);
  const fit = () => { if (!wrap) return; const [w, h] = size(); setScale(Math.max(0.2, Math.min(1.5, Math.min(wrap.clientWidth / w, wrap.clientHeight / h)))); requestAnimationFrame(track); };
  const pan = (e: MouseEvent) => {
    if (e.button !== 0 || (e.target as Element).closest(".nbox")) return;
    const x0 = e.clientX, y0 = e.clientY, l0 = wrap.scrollLeft, t0 = wrap.scrollTop; wrap.style.cursor = "grabbing";
    const mv = (m: MouseEvent) => { wrap.scrollLeft = l0 - (m.clientX - x0); wrap.scrollTop = t0 - (m.clientY - y0); };
    const up = () => { wrap.style.cursor = ""; removeEventListener("mousemove", mv); removeEventListener("mouseup", up); };
    addEventListener("mousemove", mv); addEventListener("mouseup", up);
  };
  createEffect(() => { scale(); net(); requestAnimationFrame(track); });
  return (
    <Page title="Network Diagram" help="network" tools={<>
      <span class="muted">{net()?.nodes.length ?? 0} tasks</span>
      <Seg value={netBox()} set={x => setNetBox(x as "standard")} options={[["compact", "Compact"], ["standard", "Standard"], ["detailed", "Detailed"]]} />
      <Chk label="Summary tasks" value={netSums()} set={setNetSums} /><Chk label="By WBS" value={netWbs()} set={setNetWbs} />
      <Chk label="Highlight path" value={path()} set={setPath} /><Chk label="Legend" value={legend()} set={setLegend} />
      <button class="btn" title="Zoom out" onClick={() => setScale(Math.max(0.2, scale() / 1.25))}>−</button><button class="btn" title="Zoom in" onClick={() => setScale(Math.min(2, scale() * 1.25))}>+</button>
      <button class="btn" onClick={fit}>Fit</button><button class="btn" onClick={() => setScale(1)}>100%</button>
      <button class="btn" onClick={() => C.print()}>Print</button>
    </>}>
      <Show when={legend()}><NetLegend /></Show>
      <div class="netouter"><div class="netwrap" ref={wrap} onMouseDown={pan} onScroll={track} onWheel={e => { if (e.ctrlKey) { e.preventDefault(); setScale(Math.max(0.2, Math.min(2, scale() * (e.deltaY < 0 ? 1.1 : 1 / 1.1)))); } }}>
        <Show when={net()} fallback={<p class="empty">Loading…</p>}>{n => (
          <svg class="net" width={size()[0] * scale()} height={size()[1] * scale()} viewBox={`0 0 ${size()[0]} ${size()[1]}`}>
            <NetDraw n={n()} rows={rows()} mode={netBox()} chain={chain()} sel={sel()} click={uid => selectUid(uid)} />
          </svg>
        )}</Show>
      </div>
        <Show when={net() && (size()[0] * scale() > 900 || size()[1] * scale() > 500)}>
          <svg class="minimap" width="180" height={180 * size()[1] / size()[0]} viewBox={`0 0 ${size()[0]} ${size()[1]}`} onMouseDown={e => { e.stopPropagation(); const r = (e.currentTarget as SVGElement).getBoundingClientRect(), k = size()[0] / r.width; wrap.scrollLeft = ((e.clientX - r.left) * k - vp()[2] / 2) * scale(); wrap.scrollTop = ((e.clientY - r.top) * k - vp()[3] / 2) * scale(); }}>
            <rect width={size()[0]} height={size()[1]} fill="var(--panel)" />
            <For each={net()!.nodes}>{nd => { const [x, y] = pos(nd); return <rect x={x} y={y} width={box().W} height={box().H} fill={rows().get(nd.uid)?.crit ? "#E8A0A0" : "#A9CAF6"} />; }}</For>
            <rect x={vp()[0]} y={vp()[1]} width={vp()[2]} height={vp()[3]} fill="rgba(11,92,173,.12)" stroke="#0B5CAD" stroke-width={size()[0] / 180 * 1.5} />
          </svg>
        </Show>
      </div>
    </Page>
  );
};

/** Every task the filter keeps, also those under collapsed summaries (the outline is the Gantt's, not these views'). */
const allTasks = () => {
  const [d] = createResource(() => [tick(), JSON.stringify(query())], () => call<{ rows: Row[] }>("query", { q: { ...query(), flat: true, collapsed: [], level: 0, group: [] } }));
  return () => (d()?.rows ?? grid().rows).filter(r => r.kind === "task" && !r.off);
};

// ---------- Calendar ----------
export const CalendarView = () => {
  const [month, setMonth] = createSignal<Date>((() => { const s = out()?.summary.s; const d = s ? dayDate(s) : new Date(); return new Date(d.getFullYear(), d.getMonth(), 1); })());
  const tasks = allTasks();
  const days = createMemo(() => {
    const m = month(), first = new Date(m); first.setDate(1 - ((first.getDay() + 6) % 7));
    return Array.from({ length: 42 }, (_, k) => { const d = new Date(first); d.setDate(first.getDate() + k); return d; });
  });
  const on = (d: Date) => { const n = dateDay(d); return tasks().filter(r => Math.floor(r.bar.s) <= n && n < Math.max(Math.ceil(r.bar.f), Math.floor(r.bar.s) + 1)); };
  const shift = (k: number) => setMonth(new Date(month().getFullYear(), month().getMonth() + k, 1));
  const td = dateDay(new Date());
  return (
    <Page title={month().toLocaleString("en-GB", { month: "long", year: "numeric" })} tools={<><button class="btn" onClick={() => shift(-1)}>‹</button><button class="btn" onClick={() => setMonth(new Date(new Date().getFullYear(), new Date().getMonth(), 1))}>Today</button><button class="btn" onClick={() => shift(1)}>›</button></>}>
      <div class="calgrid">
        <For each={["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]}>{d => <div class="dh">{d}</div>}</For>
        <For each={days()}>{d => {
          const list = on(d);
          return (
            <div class="day" classList={{ out: d.getMonth() !== month().getMonth(), nw: d.getDay() % 6 === 0, today: dateDay(d) === td }}>
              <span class="dn">{d.getDate()}</span>
              <For each={list.slice(0, 4)}>{r => <div class="chip" classList={{ ms: r.ms, crit: r.crit, sel: sel().includes(r.uid) }} title={r.name} onClick={() => selectUid(r.uid)} onDblClick={() => C.info()}>{r.name}</div>}</For>
              <Show when={list.length > 4}><div class="chip more">+{list.length - 4} more</div></Show>
            </div>
          );
        }}</For>
      </div>
    </Page>
  );
};

// ---------- Task Usage, Resource Usage ----------
const fmtB = (b: number, unit: string) => unit === "month" ? fmtDay(b, "month") : unit === "quarter" ? `Q${Math.floor(dayDate(b).getMonth() / 3) + 1} ${dayDate(b).getFullYear()}` : fmtDay(b);
export { RES_COLS, fmtB };
export const UsageView = (p: { by: "task" | "resource" }) => {
  const [unit, setUnit] = createSignal("week");
  const [what, setWhat] = createSignal("work");
  const [u] = createResource(() => [tick(), unit(), what()], () => call<Usage>("usage", { by: p.by, what: what(), unit: unit() }));
  const fmt = (v: number) => (v === 0 ? "" : what() === "cost" ? money(v, P().currency) : `${Math.round(v * 10) / 10}h`);
  return (
    <Page title={p.by === "task" ? "Task Usage" : "Resource Usage"} help={p.by === "task" ? "taskviews" : "resviews"} tools={<>
      <Seg options={[["work", "Work"], ["cost", "Cost"]]} value={what()} set={setWhat} />
      <Seg options={[["day", "Days"], ["week", "Weeks"], ["month", "Months"], ["quarter", "Quarters"]]} value={unit()} set={setUnit} />
    </>}>
      <div class="wlwrap">
        <Show when={u()}>{x => (
          <table class="tbl wltbl">
            <thead><tr><th class="nm">{p.by === "task" ? "Task / assignment" : "Resource / assignment"}</th><th class="r">Total</th><For each={x().buckets}>{b => <th class="r">{fmtB(b, unit())}</th>}</For></tr></thead>
            <tbody><For each={x().rows}>{r => (
              <tr classList={{ sub: r.of !== null, sumr: r.sum }}>
                <td class="nm" style={{ "padding-left": `${6 + r.lvl * 14}px` }}>{r.name}</td><td class="r">{fmt(r.total)}</td>
                <For each={r.vals}>{(v, k) => <td class="r wl" classList={{ over: r.over[k()] }}>{fmt(v)}</td>}</For>
              </tr>
            )}</For></tbody>
          </table>
        )}</Show>
      </div>
    </Page>
  );
};

// ---------- Resource Sheet ----------
const RES_COLS: [string, string, number][] = [["name", "Resource Name", 180], ["kind", "Type", 90], ["label", "Material", 70], ["initials", "Initials", 60], ["group", "Group", 100], ["max", "Max", 60],
  ["rate", "Std. Rate", 90], ["ovt", "Ovt. Rate", 90], ["per_use", "Cost/Use", 80], ["cal", "Base Calendar", 110], ["code", "Code", 70], ["country", "Country", 150], ["email", "Email", 150], ["work", "Work", 80], ["cost", "Cost", 100]];
export const ResourceSheet = () => {
  const [rows] = createResource(() => tick(), () => call<[number, boolean, string[]][]>("resources", { cols: RES_COLS.map(c => c[0]) }));
  const raw = (r: Resource, k: string): string => k === "max" ? `${Math.round(r.max * 100)}%` : k === "kind" ? r.kind : k === "cal" ? r.cal ?? "" : String((r as any)[k] ?? "");
  const [pick, setPick] = createSignal<number[]>([]);
  return (
    <Page title="Resource Sheet" help="resviews" tools={<>
      <button class="btn" onClick={() => act({ ResInsert: { name: `Resource ${P().resources.length + 1}` } })}>+ Add resource</button>
      <button class="btn" disabled={!pick().length} onClick={() => act({ ResDelete: { uids: pick() } }).then(() => setPick([]))}>Delete</button>
      <button class="btn" onClick={() => C.level()}>Level all</button>
    </>}>
      <div class="wlwrap">
        <CountryList />
        <table class="tbl sheet">
          <thead><tr><th /><th>ID</th><For each={RES_COLS}>{c => <th style={{ "min-width": `${c[2]}px` }}>{c[1]}</th>}</For><th /></tr></thead>
          <tbody><For each={P().resources}>{(r, k) => {
            const calc = () => rows()?.find(x => x[0] === r.uid);
            return (
              <tr classList={{ over: calc()?.[1] }}>
                <td><input type="checkbox" checked={pick().includes(r.uid)} onChange={e => setPick(e.currentTarget.checked ? [...pick(), r.uid] : pick().filter(x => x !== r.uid))} /></td>
                <td class="muted">{k() + 1}{calc()?.[1] && <span class="late" title="Overallocated"> ◆</span>}</td>
                <For each={RES_COLS}>{([key], j) => <td>{
                  key === "work" || key === "cost" ? <span class="ro">{calc()?.[2][j()]}</span>
                    : key === "kind" ? <select value={r.kind} onChange={e => act({ ResSet: { uid: r.uid, field: "kind", value: e.currentTarget.value } })}><option>Work</option><option>Material</option><option>Cost</option></select>
                    : key === "country" ? <span class="country"><Flag country={r.country ?? ""} /><input list="countries" value={countryCode(r.country ?? "") ? countryName(r.country ?? "") : r.country ?? ""} placeholder="Code or name" onChange={e => act({ ResSet: { uid: r.uid, field: key, value: e.currentTarget.value } })} /></span>
                    : key === "cal" ? <select value={r.cal ?? ""} onChange={e => act({ ResJson: { uid: r.uid, field: "cal", value: e.currentTarget.value || null } })}><option value="">(project)</option><For each={P().calendars}>{c => <option>{c.name}</option>}</For></select>
                    : <input value={raw(r, key)} onChange={e => act({ ResSet: { uid: r.uid, field: key, value: e.currentTarget.value } })} />
                }</td>}</For>
                <td><button class="btn sm" onClick={() => import("./dialogs").then(d => d.resourceInfo(r.uid))}>Info…</button></td>
              </tr>
            );
          }}</For></tbody>
        </table>
        <Show when={!P().resources.length}><p class="empty">No resources yet. Add people, equipment, materials or cost items, or type names in a task's Resource Names column.</p></Show>
      </div>
    </Page>
  );
};

// ---------- Resource Graph ----------
export const ResourceGraph = () => {
  const [unit, setUnit] = createSignal("week");
  const [by, setBy] = createSignal("resource");
  const [show, setShow] = createSignal("hours");
  const [pick, setPick] = createSignal("");
  const [u] = createResource(() => [tick(), unit(), by()], () => call<Usage>("usage", { by: by(), what: "work", unit: unit() }));
  const heads = () => (u()?.rows ?? []).filter(r => r.of === null && r.cap?.length);
  createEffect(() => { if (!heads().some(r => `${r.uid}` === pick()) && heads()[0]) setPick(`${heads()[0].uid}`); });
  const row = () => heads().find(r => `${r.uid}` === pick());
  const per = (v: number, k: number) => (show() === "people" ? v / Math.max(1, u()!.person[k] ?? 1) : v);
  const unitL = () => (show() === "people" ? " people" : "h");
  return (
    <Page title="Resource Graph" help="resviews" tools={<>
      <Seg options={[["resource", "People"], ["group", "Trades"]]} value={by()} set={setBy} />
      <select onChange={e => setPick(e.currentTarget.value)}><For each={heads()}>{x => <option value={x.uid} selected={`${x.uid}` === pick()}>{x.name}</option>}</For></select>
      <Seg options={[["hours", "Hours"], ["people", "People"]]} value={show()} set={setShow} />
      <Seg options={[["day", "Days"], ["week", "Weeks"], ["month", "Months"]]} value={unit()} set={setUnit} />
    </>}>
      <Show when={row() && u()} fallback={<p class="empty">{by() === "group" ? "Give work resources a Group (their trade) in the Resource Sheet and assign them to see each trade's load." : "Add work resources and assign them to see their load."}</p>}>{_ => {
        const vals = () => row()!.vals.map(per), caps = () => row()!.cap!.map(per), b = () => u()!.buckets;
        const W = () => Math.max(600, vals().length * 34), H = 300, max = () => Math.max(1, ...vals(), ...caps());
        return (
          <div class="wlwrap panel"><svg width={W() + 60} height={H + 50} class="rgraph">
            <For each={[0, 0.25, 0.5, 0.75, 1]}>{f => <><line x1="50" x2={W() + 50} y1={H - f * H + 10} y2={H - f * H + 10} stroke="#E6EBF2" /><text x="44" y={H - f * H + 14} text-anchor="end" class="ax">{Math.round(max() * f * 10) / 10}{show() === "people" ? "" : "h"}</text></>}</For>
            <Index each={vals()}>{(v, k) => {
              // Index (not For): bars are plain numbers, so each slot keeps its position when the data changes.
              const x = () => 54 + k * (W() / vals().length), w = () => W() / vals().length - 6, c = () => caps()[k] ?? 0, h = () => v() / max() * H;
              return <><rect x={x()} y={H - h() + 10} width={w()} height={h()} fill={v() > c() + 0.01 ? "#D64545" : "#6E9FE6"} rx="2"><title>{`${fmtB(b()[k], unit())}: ${Math.round(v() * 10) / 10}${unitL()} of ${Math.round(c() * 10) / 10}${unitL()}`}</title></rect>
                <line x1={x() - 2} x2={x() + w() + 2} y1={H - c() / max() * H + 10} y2={H - c() / max() * H + 10} stroke="#14325E" stroke-dasharray="4 3" />
                <Show when={vals().length < 40 || k % Math.ceil(vals().length / 30) === 0}><text x={x() + w() / 2} y={H + 28} text-anchor="middle" class="ax">{fmtB(b()[k], unit())}</text></Show></>;
            }}</Index>
          </svg><p class="muted">Bars show the {show() === "people" ? "people needed (hours ÷ one person's working hours)" : "assigned hours"}{by() === "group" ? " for everyone in the trade" : ""}; the dashed line is capacity (max units × working hours). Red is over capacity.</p></div>
        );
      }}</Show>
    </Page>
  );
};

// ---------- Team Planner ----------
export const TeamPlanner = () => {
  const tasks = allTasks();
  const s = () => out()!.summary, span = () => Math.max(7, s().f - s().s + 2);
  const lanes = createMemo(() => {
    const people = P().resources.filter(r => r.kind === "Work");
    const m = new Map<string, Row[]>(people.map(r => [r.name, []]));
    const none: Row[] = [];
    for (const t of tasks()) { const names = t.res.split(",").map(x => x.trim().replace(/\[.*\]/, "")).filter(Boolean); let any = false; for (const n of names) if (m.has(n)) { m.get(n)!.push(t); any = true; } if (!any) none.push(t); }
    return [...[...m].map(([k, v]) => [k, v] as [string, Row[]]), ["Unassigned tasks", none] as [string, Row[]]];
  });
  const W = 1100, px = (d: number) => ((d - s().s) / span()) * W;
  return (
    <Page title="Team Planner" help="resviews" tools={<span class="muted">Each lane shows a person's tasks. Overlaps mean overallocation.</span>}>
      <div class="wlwrap panel"><svg width={W + 190} height={lanes().length * 44 + 30}>
        <For each={lanes()}>{([name, list], k) => <g transform={`translate(0,${k() * 44 + 20})`}>
          <text x="6" y="22" class="lane">{name}</text><line x1="0" x2={W + 190} y1="40" y2="40" stroke="#E6EBF2" />
          <For each={list}>{(t, j) => {
            const overlap = list.slice(0, j()).some(o => o.bar.s < t.bar.f && t.bar.s < o.bar.f);
            return <rect x={180 + px(t.bar.s)} y={overlap ? 22 : 6} width={Math.max(4, px(t.bar.f) - px(t.bar.s))} height="14" rx="3" fill={t.crit ? "#F2B0B0" : "#A9CAF6"} stroke={overlap && name !== "Unassigned tasks" ? "#D64545" : "#2F6FCF"} onClick={() => selectUid(t.uid)} onDblClick={() => C.info()}><title>{`${t.name}\n${fmtDay(t.bar.s)} → ${fmtDay(t.bar.f)}`}</title></rect>;
          }}</For>
        </g>}</For>
      </svg></div>
    </Page>
  );
};

// ---------- Risks ----------
export const Risks = () => {
  const blank = () => ({ id: Math.max(0, ...P().risks.map(r => r.id)) + 1, name: "New risk", cause: "", prob: 3, impact: 3, owner: "", response: "", status: "Open", tasks: [], kind: "Threat", days: 0 });
  const put = (r: any) => act({ Risk: { id: r.id, risk: r } });
  const color = (s: number) => s >= 15 ? "#F8D4D4" : s >= 8 ? "#FDF0D2" : "#DDF2E5";
  return (
    <Page title="Risk Register" help="risks" tools={<><button class="btn" onClick={() => setView("simrisk")}>Schedule Risk…</button><button class="btn pri" onClick={() => put(blank())}>+ Add risk</button></>}>
      <div class="cols">
        <div class="panel" style={{ "grid-column": "span 2" }}>
          <table class="tbl"><thead><tr><th>ID</th><th>Risk</th><th>Type</th><th>Cause</th><th>Prob. (1–5)</th><th>Impact (1–5)</th><th>Score</th><th title="Working days added to each linked task when the risk happens (taken off for an opportunity). Used by Schedule Risk.">Delay (days)</th><th>Owner</th><th>Response</th><th>Status</th><th>Tasks</th><th /></tr></thead>
            <tbody><For each={P().risks}>{r => (
              <tr>
                <td>R{r.id}</td><td><input value={r.name} onChange={e => put({ ...r, name: e.currentTarget.value })} /></td>
                <td><select value={r.kind} onChange={e => put({ ...r, kind: e.currentTarget.value })}><option>Threat</option><option>Opportunity</option></select></td>
                <td><input value={r.cause} onChange={e => put({ ...r, cause: e.currentTarget.value })} /></td>
                <td><input type="number" min="1" max="5" value={r.prob} onChange={e => put({ ...r, prob: +e.currentTarget.value })} /></td>
                <td><input type="number" min="1" max="5" value={r.impact} onChange={e => put({ ...r, impact: +e.currentTarget.value })} /></td>
                <td style={{ background: color(r.prob * r.impact) }}><b>{r.prob * r.impact}</b></td>
                <td><input type="number" min="0" step="0.5" value={r.days ?? 0} onChange={e => put({ ...r, days: +e.currentTarget.value || 0 })} /></td>
                <td><input value={r.owner} onChange={e => put({ ...r, owner: e.currentTarget.value })} /></td>
                <td><input value={r.response} onChange={e => put({ ...r, response: e.currentTarget.value })} /></td>
                <td><select value={r.status} onChange={e => put({ ...r, status: e.currentTarget.value })}><option>Open</option><option>Mitigating</option><option>Closed</option></select></td>
                <td class="muted">{r.tasks.length}</td><td><button class="x" onClick={() => act({ Risk: { id: r.id, risk: null } })}>×</button></td>
              </tr>
            )}</For></tbody></table>
          <Show when={!P().risks.length}><p class="empty">No risks recorded. Add threats and opportunities, score them, and link them to tasks in the details pane.</p></Show>
        </div>
        <div class="panel"><h2>Probability × impact</h2>
          <div class="matrix"><For each={[5, 4, 3, 2, 1]}>{p => <For each={[1, 2, 3, 4, 5]}>{i => {
            const n = P().risks.filter(r => r.prob === p && r.impact === i && r.status !== "Closed").length;
            return <div style={{ background: color(p * i) }}>{n || ""}</div>;
          }}</For>}</For></div><p class="muted">Rows: probability (5 at the top). Columns: impact.</p>
        </div>
      </div>
    </Page>
  );
};

// ---------- Check Schedule (DCMA 14 and further checks) ----------
export const Checks = () => {
  const [lim, setLim] = createSignal({ pct: 5, high_float: 44, high_dur: 44, long_lag: 20, index: 0.95 });
  const [list] = createResource(() => [tick(), lim()], () => call<Check[]>("checks", { limits: lim() }));
  const show = (c: Check) => {
    if (!c.uids.length) return toast("No tasks to show for this check.", true);
    setFilter({ name: c.name, rules: c.uids.map(u => ({ field: "uid", test: "equals", value: String(u), value2: "", or: true })), summaries: true, highlight: false });
    setView("gantt");
  };
  const fmt = (c: Check) => (c.key === "cpli" || c.key === "bei" ? c.value.toFixed(2) : c.key === "cpt" ? (c.pass ? "Pass" : "Fail") : `${c.value.toFixed(1)}%`);
  return (
    <Page title="Check Schedule" help="checks" tools={<>
      <label class="muted">Limit % <input type="number" style={{ width: "60px" }} value={lim().pct} onChange={e => setLim({ ...lim(), pct: +e.currentTarget.value })} /></label>
      <label class="muted">High float (d) <input type="number" style={{ width: "60px" }} value={lim().high_float} onChange={e => setLim({ ...lim(), high_float: +e.currentTarget.value })} /></label>
      <label class="muted">High duration (d) <input type="number" style={{ width: "60px" }} value={lim().high_dur} onChange={e => setLim({ ...lim(), high_dur: +e.currentTarget.value })} /></label>
      <button class="btn" onClick={() => C.print()}>Print</button>
    </>}>
      <Show when={list()}>{l => <>
        <div class="kpis"><div class="kpi" classList={{ good: l().every(c => c.pass) }}><div class="v">{l().filter(c => c.pass).length} / {l().length}</div><div class="l">checks passed</div></div></div>
        <div class="panel" style={{ "margin-top": "14px" }}><table class="tbl">
          <thead><tr><th>Check</th><th>What it looks for</th><th class="r">Tasks</th><th class="r">Of</th><th class="r">Result</th><th>Limit</th><th /></tr></thead>
          <tbody><For each={l()}>{c => <tr class="link" onClick={() => show(c)}>
            <td><b>{c.name}</b></td><td class="muted">{c.about}</td><td class="r">{c.count}</td><td class="r">{c.total}</td><td class="r">{fmt(c)}</td><td>{c.limit}</td>
            <td><span class="pill" classList={{ ok: c.pass, err: !c.pass }}>{c.pass ? "Pass" : "Fail"}</span></td>
          </tr>}</For></tbody>
        </table><p class="muted">Click a check to filter the Gantt chart to its tasks.</p></div>
      </>}</Show>
    </Page>
  );
};

// ---------- Portfolio (workspace folder) ----------
/** Plans grouped by folder (the EPS), with a rollup row per folder. */
const eps = (cards: any[]) => {
  const out: any[] = [];
  const folders = [...new Set(cards.map(c => c.folder))].sort();
  const all = new Set<string>();
  folders.forEach(f => { const parts = f ? f.split(" › ") : []; parts.forEach((_: string, k: number) => all.add(parts.slice(0, k + 1).join(" › "))); });
  const heads = [...all].sort();
  const under = (h: string) => cards.filter(c => c.folder === h || c.folder.startsWith(h + " › "));
  for (const c of cards.filter(c => !c.folder)) out.push({ ...c, depth: 0 });
  for (const h of heads) {
    const u = under(h);
    const depth = h.split(" › ").length - 1;
    out.push({ head: h, depth, n: u.length, pct: u.reduce((a, c) => a + c.pct, 0) / Math.max(1, u.length), late: u.reduce((a, c) => a + c.late, 0), cost: "" });
    for (const c of cards.filter(c => c.folder === h)) out.push({ ...c, depth: depth + 1 });
  }
  return out;
};
export const Portfolio = () => {
  const [shut, setShut] = createSignal<string[]>([]);
  const [again, setAgain] = createSignal(0);
  const [data] = createResource(() => again(), k => call<[string, any[]]>("workspace", { choose: k < 0 }));
  return (
    <Page title="Portfolio" help="portfolio" tools={<><span class="muted">{data()?.[0] || "No workspace folder"}</span><button class="btn" onClick={() => setAgain(-Math.abs(again()) - 1)}><Icon n="folder" /> Choose folder…</button><button class="btn" onClick={() => setAgain(Math.abs(again()) + 1)}>Refresh</button></>}>
      <div class="panel"><table class="tbl">
        <thead><tr><th>Plan</th><th>Manager</th><th>Start</th><th>Finish</th><th class="r">Tasks</th><th>Complete</th><th class="r">Cost</th><th class="r">SPI</th><th class="r">CPI</th><th class="r">Late</th></tr></thead>
        <tbody><For each={eps(data()?.[1] ?? [])}>{c => c.head !== undefined ? (
          <tr class="eps" onClick={() => setShut(shut().includes(c.head) ? shut().filter(x => x !== c.head) : [...shut(), c.head])}>
            <td colspan="5" style={{ "padding-left": `${6 + c.depth * 16}px` }}><b>{shut().includes(c.head) ? "▷" : "◢"} {c.head.split(" › ").pop() || "Workspace"}</b> <span class="muted">{c.n} plans</span></td>
            <td><div class="meter"><i style={{ width: `${c.pct}%` }} /></div></td><td class="r">{c.cost}</td><td /><td /><td class="r" classList={{ late: c.late > 0 }}>{c.late}</td></tr>
        ) : <Show when={!shut().some(h => c.folder === h || c.folder.startsWith(h + " › "))}><tr class="link" onClick={() => loadOut("open", { path: c.path })}>
          <td style={{ "padding-left": `${22 + c.depth * 16}px` }}><b>{c.name}</b>{c.error && <div class="late">{c.error}</div>}</td><td>{c.manager}</td><td>{c.start}</td><td>{c.finish}</td><td class="r">{c.tasks}</td>
          <td><div class="meter"><i style={{ width: `${c.pct}%` }} /></div></td><td class="r">{c.cost}</td>
          <td class="r" classList={{ late: c.spi !== null && c.spi < 0.95 }}>{c.spi?.toFixed(2) ?? "—"}</td><td class="r" classList={{ late: c.cpi !== null && c.cpi < 0.95 }}>{c.cpi?.toFixed(2) ?? "—"}</td>
          <td class="r" classList={{ late: c.late > 0 }}>{c.late}</td>
        </tr></Show>}</For></tbody>
      </table><Show when={!data()?.[1].length}><p class="empty">Choose a folder that holds your .dplan files to see every plan side by side. Subfolders become the EPS (enterprise project structure) levels.</p></Show></div>
    </Page>
  );
};
export { Chk, openMenu, setSel, setBackstage };
