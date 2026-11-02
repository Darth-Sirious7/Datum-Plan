// File › Print: print settings on the left, a live page preview on the right. Pages are laid out
// here (table + vector Gantt per page, headers, footers, logos, legend, page numbers) and printed
// with the WebView's print dialog, where "Save as PDF" makes a PDF.
import { For, Show, Switch, Match, JSX, createMemo, createSignal, createResource, onCleanup, lazy } from "solid-js";
import { Portal } from "solid-js/web";
import { createStore } from "solid-js/store";
import { Row, Net, Usage, SheetOut, Grid, fmtDay, iso, dateDay, dayDate, invoke } from "./api";
import { grid, out, opts, P, act, cols, zoom, call, view, toast, query, tick } from "./store";
import { NetDraw, netSize, netBox, setNetBox, netSums, netWbs, loadNet, NET_COLS, RES_COLS, fmtB } from "./views";
import { sheetK } from "./sheets";
import { picData } from "./pictures";
import { gstyle, floorU, nextU, label, off, Unit } from "./gantt";
import * as F from "./fmt";
import { Pick, Check, Seg } from "./ui";

/** Paper sizes in millimetres (portrait). */
export const PAPERS: [string, number, number][] = [["A4", 210, 297], ["A3", 297, 420], ["A2", 420, 594], ["A1", 594, 841], ["Letter", 215.9, 279.4], ["Legal", 215.9, 355.6], ["Tabloid", 279.4, 431.8]];
const MM = 96 / 25.4, SCALE_H = 34, IDW = 30;
type Slots = [string, string, string];
/** A picture in the header: the company logo ("@company"), the client logo ("@client") or a picture, at a height in px. */
type Pic = { v: string; h: number };
export type PageSetup = {
  paper: string; landscape: boolean; margins: [number, number, number, number]; scale: number; fit: number;
  range: "all" | "dates"; from: string; to: string; pages: string; colKeys: string[] | null; repeat: boolean; unit: string;
  font: number; rowH: number; fitCols: boolean; wrap: boolean; notes: boolean;
  header: Slots; footer: Slots; legend: "every" | "last" | "none"; legendText: string; clientLogo: string; grid: boolean; idCol: boolean;
  /** Header & footer: "simple" (pictures, title, footer choices) or "codes" (the three boxes with &[codes]). */
  hf: "simple" | "codes"; pics: [Pic[], Pic[], Pic[]]; titleOn: boolean; title: string; titleSize: number; subtitle: string;
  fPrinted: boolean; fTime: boolean; fPage: boolean; fStatus: boolean; fCompany: boolean; fRev: string; fText: string;
  /** A master's subplans: as on screen, all collapsed, all expanded, or left out. */
  subs: "shown" | "collapsed" | "expanded" | "none";
};
export const CODES: [string, string][] = [["&[Page]", "Page number"], ["&[Pages]", "Total pages"], ["&[Date]", "Today's date"], ["&[Project]", "Project name"], ["&[Company]", "Company"], ["&[Client]", "Client"],
  ["&[Manager]", "Manager"], ["&[Status]", "Status date"], ["&[Start]", "Project start"], ["&[Finish]", "Project finish"], ["&[Logo]", "Company logo"], ["&[Client Logo]", "Client logo"]];
const defaults = (): PageSetup => ({
  paper: opts().paper || "A4", landscape: opts().landscape !== false, margins: [12, 10, 12, 10], scale: 100, fit: 1, range: "all", from: "", to: "", pages: "", colKeys: null, repeat: true, unit: "auto", font: 10, rowH: 18, fitCols: false, wrap: false, notes: false,
  header: OLD_HEAD, footer: OLD_FOOT, legend: "every", legendText: "&[Project]\nStart &[Start] · Finish &[Finish]\nStatus date: &[Status]", clientLogo: "", grid: true, idCol: true,
  hf: "simple", pics: [[{ v: "@company", h: 40 }], [], []], titleOn: true, title: "", titleSize: 16, subtitle: "", fPrinted: true, fTime: false, fPage: true, fStatus: false, fCompany: false, fRev: "", fText: "", subs: "shown",
});
const OLD_HEAD: Slots = ["&[Logo]", "&[Project]", "&[Client Logo]"], OLD_FOOT: Slots = ["Printed &[Date]", "Page &[Page] of &[Pages]", "&[Company]"];
const [ps, setPs] = createStore<PageSetup>(defaults());
let loadedFor = "";
/** The plan's saved page setup (project.ui.page) over the defaults. */
const load = () => {
  const key = `${out()?.path}|${out()?.rev}`; if (key === loadedFor) return; loadedFor = key;
  const { cols: _old, ...saved } = (P().ui?.page ?? {}) as Partial<PageSetup> & { cols?: number };
  // Page setups saved before 0.5.10: own header or footer text keeps the code boxes; the client logo moves to the right picture.
  if (saved.header && !saved.hf) {
    const own = JSON.stringify(saved.header) !== JSON.stringify(OLD_HEAD) || JSON.stringify(saved.footer) !== JSON.stringify(OLD_FOOT);
    Object.assign(saved, { hf: own ? "codes" : "simple", pics: [[{ v: "@company", h: 40 }], [], saved.clientLogo ? [{ v: "@client", h: 40 }] : []] });
  }
  // 0.5.10 kept one picture per place, all one height.
  const old = saved as { imgs?: Slots; imgH?: number };
  if (old.imgs && !saved.pics) saved.pics = old.imgs.map(v => (v ? [{ v, h: old.imgH ?? 40 }] : [])) as PageSetup["pics"];
  if (saved.title === "&[Project]") saved.title = "";
  if (saved.legendText === "&[Project]\nStatus date: &[Status]") delete saved.legendText;
  setPs({ ...defaults(), ...saved });
};
const save = async () => { if (JSON.stringify(P().ui?.page ?? {}) !== JSON.stringify(ps)) await act({ Ui: { key: "page", value: JSON.parse(JSON.stringify(ps)) } }); };

const codes = (t: string, page: number, pages: number) => {
  const p = P(), s = out()!.summary;
  return t.replace(/&\[Page\]/g, String(page)).replace(/&\[Pages\]/g, String(pages)).replace(/&\[Date\]/g, fmtDay(dateDay(new Date()), "long")).replace(/&\[Project\]/g, p.name)
    .replace(/&\[Company\]/g, p.company).replace(/&\[Client\]/g, p.client).replace(/&\[Manager\]/g, p.manager).replace(/&\[Status\]/g, s.status || "not set").replace(/&\[Start\]/g, s.start).replace(/&\[Finish\]/g, s.finish);
};
/** Header/footer text with codes; logo codes become pictures. */
const Slot = (p: { t: string; page: number; pages: number; h: number }) => (
  <For each={p.t.split(/(&\[(?:Client )?Logo\])/)}>{part => part === "&[Logo]" ? (P().logo && <img src={P().logo} style={{ "max-height": `${p.h}px`, "max-width": "45%" }} />)
    : part === "&[Client Logo]" ? (ps.clientLogo && <img src={ps.clientLogo} style={{ "max-height": `${p.h}px`, "max-width": "45%" }} />)
    : <span style={{ "white-space": "pre-line" }}>{codes(part, p.page, p.pages)}</span>}</For>
);

/** A header or footer picture: the company logo (File › Options), the client logo, or one chosen here. */
const pic = (v: string) => (v === "@company" ? out()?.project.logo : v === "@client" ? ps.clientLogo : v.startsWith("@pic:") ? (out()?.project.pictures ?? []).find(x => x.id === +v.slice(5))?.data : v) || "";
const now = () => { const d = new Date(); return `${fmtDay(dateDay(d), "long")}${ps.fTime ? `, ${d.toTimeString().slice(0, 5)}` : ""}`; };
const footParts = (page: number, pages: number, flow = false): Slots => [
  [ps.fPrinted ? `Printed ${now()}` : "", ps.fRev.trim() ? `Revision ${ps.fRev.trim()}` : ""].filter(Boolean).join("  ·  "),
  ps.fPage && !flow ? `Page ${page} of ${pages}` : "",
  [ps.fStatus ? `Status date: ${out()!.summary.status || "not set"}` : "", ps.fCompany ? P().company : "", ps.fText.trim() ? codes(ps.fText.trim(), page, pages) : ""].filter(Boolean).join("  ·  "),
];
/** The title at the top: the plan's name unless another is typed. */
const titleText = (n: number, pages: number) => (ps.titleOn ? codes(ps.title.trim() || "&[Project]", n, pages) : "");
/** Height of the header and footer bands (CSS px), 0 when empty. */
const shown = (i: number) => ps.pics[i].filter(x => pic(x.v));
const tallest = (i: number) => Math.max(0, ...shown(i).map(x => x.h));
const titleH = () => (ps.titleOn ? Math.round(ps.titleSize * 1.3) : 0) + (ps.subtitle.trim() ? 16 : 0);
const headH = () => {
  if (!out()) return 0;
  if (ps.hf === "codes") return ps.header.some(x => x.trim()) ? 52 : 0;
  const centre = (tallest(1) ? tallest(1) + 4 : 0) + titleH(), h = Math.max(tallest(0), tallest(2), centre);
  return h ? h + 14 : 0;
};
const footH = () => !out() ? 0 : ps.hf === "codes" ? (ps.footer.some(x => x.trim()) ? 30 : 0) : (footParts(1, 1).some(Boolean) ? 28 : 0);
/** The pictures of one place (left, centre, right), side by side, each at its own height. */
const Pics = (p: { i: number }) => <For each={shown(p.i)}>{x => <img src={pic(x.v)} style={{ height: `${x.h}px` }} />}</For>;
/** The header band of a page. */
const Head = (p: { n: number; pages: number; h?: number }) => (
  <Show when={ps.hf === "simple"} fallback={<div class="phead" style={p.h ? { height: `${p.h - 8}px` } : undefined}><For each={[0, 1, 2]}>{i => <div class={`ps${i}`}><Slot t={ps.header[i]} page={p.n} pages={p.pages} h={(p.h ?? 52) - 12} /></div>}</For></div>}>
    <div class="phead simple" style={p.h ? { height: `${p.h - 8}px` } : undefined}>
      <div class="ps0"><Pics i={0} /></div>
      <div class="ps1"><Show when={shown(1).length}><div class="ppics"><Pics i={1} /></div></Show>
        <Show when={ps.titleOn || ps.subtitle.trim()}><div class="ptitles"><Show when={ps.titleOn}><div class="ptitle" style={{ "font-size": `${ps.titleSize}px` }}>{titleText(p.n, p.pages)}</div></Show><Show when={ps.subtitle.trim()}><div class="psub">{codes(ps.subtitle, p.n, p.pages)}</div></Show></div></Show></div>
      <div class="ps2"><Pics i={2} /></div>
    </div>
  </Show>
);
/** The footer band of a page (in reports the page numbers come from the printer). */
const Foot = (p: { n: number; pages: number; h?: number; flow?: boolean }) => {
  const noPage = (t: string) => (p.flow ? t.replace(/[^&]*&\[Pages?\][^&]*/g, "").trim() : t);
  return (
    <div class="pfoot" style={p.h ? { height: `${p.h - 6}px` } : undefined}>
      <Show when={ps.hf === "simple"} fallback={<For each={[0, 1, 2]}>{i => <div class={`ps${i}`}><Slot t={noPage(ps.footer[i])} page={p.n} pages={p.pages} h={(p.h ?? 30) - 8} /></div>}</For>}>
        <For each={footParts(p.n, p.pages, p.flow)}>{(t, i) => <div class={`ps${i()}`}><span>{t}</span></div>}</For>
      </Show>
    </div>
  );
};
/** Reports print their page numbers at the bottom when the footer asks for them. */
const flowPageNo = () => (ps.hf === "simple" ? ps.fPage : ps.footer.some(t => /&\[Pages?\]/.test(t)));

type Page = { rows: [number, number]; x: [number, number]; n: number; notes?: [string, string, string][] };
/** The table and the chart form one long strip; pages are windows on it (across, then
 *  down). Columns are never cut at a page edge; the ID and Task Name can repeat on every page. */
/** A master printed with its subplans collapsed, expanded or left out (otherwise as on screen). */
const subMode = () => (out()?.project.subplans?.length && ps.subs !== "shown" ? ps.subs : null);
const [subGrid] = createResource(() => { const m = subMode(); return m ? [m, tick(), JSON.stringify(query())] as const : null; },
  ([m]) => call<Grid>("query", { q: query(), subs: { show: m !== "none", open: m === "expanded" ? (P().subplans ?? []).map(x => x.file) : [] } }));
const printGrid = () => (subMode() ? subGrid() ?? grid() : grid());
const layout = createMemo(() => {
  const s = out()?.summary, g = printGrid();
  if (!s) return null;
  const [pw, ph] = (() => { const p = PAPERS.find(x => x[0] === ps.paper) ?? PAPERS[0]; return ps.landscape ? [p[2], p[1]] : [p[1], p[2]]; })();
  const W0 = (pw - ps.margins[1] - ps.margins[3]) * MM, H0 = (ph - ps.margins[0] - ps.margins[2]) * MM;
  let k = ps.fit > 0 ? 1 : ps.scale / 100, W = W0 / k, H = H0 / k;
  const hH = headH(), fH = footH(), legH = ps.legend === "none" ? 0 : 58;
  const byKey = new Map(g.cols.map((c, j) => [c.key, { c, j }])), idw = ps.idCol ? IDW : 0;
  const keys = ps.colKeys ?? g.cols.map(c => c.key).filter(x => x !== "mode");
  let colsP = keys.flatMap(key => { const x = byKey.get(key); return x ? [{ ...x, w: cols().find(c => c[0] === key)?.[1] ?? 90 }] : []; });
  let fs = ps.font;
  const tw0 = idw + colsP.reduce((a, c) => a + c.w, 0);
  if (ps.fitCols && tw0 > W * 0.55) { const f = (W * 0.55 - idw) / (tw0 - idw); colsP = colsP.map(c => ({ ...c, w: c.w * f })); fs = Math.max(6.5, fs * Math.max(f, 0.7)); }
  const lefts: number[] = []; let TW = idw; for (const c of colsP) { lefts.push(TW); TW += c.w; }
  // Fit to N pages wide: shrink the whole page when the table leaves too little room for the chart.
  // (the chart keeps at least 35 % of the new page width, so the pages come out exactly `fit` wide)
  if (ps.fit > 0 && TW > (ps.fit - 0.35) * W) { W = TW / (ps.fit - 0.35); k = W0 / W; H = H0 / k; }
  const nameAt = colsP.findIndex(c => c.c.key === "name"), RW = ps.repeat && nameAt >= 0 && nameAt < 3 ? lefts[nameAt] + colsP[nameAt].w : 0;
  const d0 = ps.range === "dates" && ps.from ? dateDay(new Date(ps.from)) : Math.floor(s.s) - 3;
  const d1 = Math.max(d0 + 1, ps.range === "dates" && ps.to ? dateDay(new Date(ps.to)) + 1 : Math.ceil(s.f) + 7);
  const CW = ps.fit > 0 ? Math.max(W * 0.3, ps.fit * W - (ps.fit - 1) * RW - TW - 0.5) : (d1 - d0) * zoom(), pd = CW / (d1 - d0), total = TW + CW;
  const across: [number, number][] = [];
  for (let x = 0; x < total - 1;) {
    let e = Math.min(total, x + (x ? W - RW : W));
    if (e < TW) { const edge = Math.max(...[...lefts, TW].filter(v => v > x + 20 && v <= e)); if (isFinite(edge)) e = edge; }
    across.push([x, e]); x = e;
  }
  const RH = ps.rowH, ni = nameAt >= 0 ? colsP[nameAt] : null;
  const rowsH = g.rows.map(r => (ps.wrap && ni && r.name.length * fs * 0.55 + r.lvl * 10 > ni.w - 8 ? Math.round(RH * 1.75) : RH));
  const room = H - hH - fH - (ps.legend === "every" ? legH : 0) - SCALE_H - 6, pages: Page[] = [];
  const fill = (r: number, max: number) => { let e = r, h = 0; while (e < g.rows.length && (h + rowsH[e] <= max || e === r)) h += rowsH[e++]; return Math.max(e, r + 1); }, chunks: [number, number][] = [];
  for (let r = 0; r < Math.max(1, g.rows.length);) { const e = fill(r, room); chunks.push([r, e]); r = e; }
  // A legend on the last page only: that page keeps room for it (a full last page cut off the legend and footer).
  const [r1, e1] = chunks[chunks.length - 1];
  if (ps.legend === "last" && rowsH.slice(r1, e1).reduce((a, b) => a + b, 0) > room - legH) { const m = fill(r1, room - legH); if (m < e1) chunks.splice(-1, 1, [r1, m], [m, e1]); }
  for (const [r, e] of chunks) for (const x of across) pages.push({ rows: [r, e], x, n: pages.length + 1 });
  return { pw, ph, k, W, H, headH: hH, footH: fH, legH, colsP, lefts, TW, RW, CW, pd, d0, d1, pages, rows: g.rows, rowsH, fs, RH, idw };
});
/** Task notes for the last pages, when "Print notes" is on. */
const [notes, setNotes] = createSignal<[string, string, string][]>([]);
const loadNotes = async () => { if (!ps.notes) return setNotes([]); const g = await call<{ rows: Row[] }>("query", { q: { ...query(), cols: ["code", "name", "notes"] } }); setNotes((g?.rows ?? []).filter(r => r.uid > 0 && r.cells[2]?.trim()).map(r => [r.cells[0], r.cells[1], r.cells[2]])); };
const allPages = createMemo(() => {
  const L = layout(); if (!L) return [];
  const out: Page[] = [...L.pages], per = Math.max(1, Math.floor((L.H - L.headH - L.footH - 40) / 16)); let cur: [string, string, string][] = [], lines = 0;
  for (const n of notes()) { const l = 1 + Math.ceil(n[2].length / Math.max(40, L.W / 6.2)) + n[2].split("\n").length - 1; if (lines + l > per && cur.length) { out.push({ rows: [0, 0], x: [0, 0], n: out.length + 1, notes: cur }); cur = []; lines = 0; } cur.push(n); lines += l; }
  if (cur.length) out.push({ rows: [0, 0], x: [0, 0], n: out.length + 1, notes: cur });
  return out;
});
/** Pages chosen in "Pages" (e.g. 1-3,5), or all. */
const pick = <T extends { n: number }>(all: T[]) => {
  const want = ps.pages.trim(); if (!want) return all;
  const ok = new Set<number>(); want.split(",").forEach(x => { const [a, b] = x.split("-").map(v => +v.trim()); for (let n = a; n <= (b || a); n++) ok.add(n); });
  return all.filter(p => ok.has(p.n));
};
const chosen = () => pick(allPages());
const unit = (pd: number): [Unit, Unit] => { const u = ps.unit; if (u === "day") return ["week", "day"]; if (u === "week") return ["month", "week"]; if (u === "month") return ["quarter", "month"]; if (u === "quarter") return ["year", "quarter"]; return pd >= 16 ? ["week", "day"] : pd >= 4 ? ["month", "week"] : pd >= 1.2 ? ["quarter", "month"] : ["year", "quarter"]; };
const RIGHT = ["Num", "Int", "Dur", "Cost", "Pct", "Work"];

/** The strip between x = a and b, drawn at dx on the page (table cells and chart, each clipped). */
const Strip = (p: { a: number; b: number; dx: number; rows: Row[]; ys: number[]; hs: number[]; bodyH: number }) => {
  const L = layout()!, st = gstyle(), w = p.b - p.a, fs = L.fs;
  const C = (d: number) => (d - L.d0) * L.pd;
  const visCols = () => L.colsP.map((c, i) => ({ ...c, x: L.lefts[i] })).filter(c => c.x < p.b && c.x + c.w > p.a);
  const t0 = L.d0 + Math.max(0, p.a - L.TW) / L.pd, t1 = L.d0 + Math.max(0, p.b - L.TW) / L.pd;
  const Bar = (q: { r: Row; y: number; h: number }) => {
    const B = q.r.bar, y = q.y + (q.h - 18) / 2, crit = q.r.crit, x1 = C(B.s), x2 = C(B.f);
    if (B.f < t0 - 2 || B.s > t1 + 2) return null;
    if (q.r.ms) return <g opacity={q.r.sub ? 0.6 : 1}><path d={`M${x1} ${y + 3}l6 6l-6 6l-6-6z`} fill={crit ? st.crit : st.sum} /><text x={x1 + 9} y={y + 13} font-size="9" fill="#333">{fmtDay(B.s)}</text></g>;
    if (q.r.kind === "subplan") return <g fill="#6D3FC8"><rect x={x1} y={y + 5} width={Math.max(1, x2 - x1)} height={4} /><path d={`M${x1} ${y + 5}h5l-5 8z`} /><path d={`M${x2} ${y + 5}h-5l5 8z`} /></g>;
    if (q.r.kind === "summary" || q.r.kind === "project") return <g fill={st.sum} opacity={q.r.sub ? 0.6 : 1}><rect x={x1} y={y + 5} width={Math.max(1, x2 - x1)} height={4} /><path d={`M${x1} ${y + 5}h5l-5 8z`} /><path d={`M${x2} ${y + 5}h-5l5 8z`} /></g>;
    const done = C(B.s + (B.f - B.s) * B.pct / 100);
    return <g opacity={q.r.sub ? 0.6 : 1}>{B.bs !== null && B.bs !== undefined && B.bf !== null && B.bf !== undefined && <rect x={C(B.bs)} y={y + 15} width={Math.max(1.5, C(B.bf) - C(B.bs))} height={3} fill="#AEB7C4" />}
      {B.dl !== null && B.dl !== undefined && <g fill="#2E9D57"><path d={`M${C(B.dl) - 4} ${y}h8l-4 6z`} /><rect x={C(B.dl) - 0.5} y={y + 4} width={1} height={12} /></g>}
      <rect x={x1} y={y + 4} width={Math.max(1.5, x2 - x1)} height={10} fill={crit ? st.cfill : st.fill} stroke={crit ? st.crit : st.edge} stroke-width=".8" />
      {B.pct > 0 && <rect x={x1} y={y + 7} width={Math.max(0, done - x1)} height={4} fill={crit ? st.crit : st.edge} />}
      {q.r.res && <text x={x2 + 4} y={y + 13} font-size="9" fill="#333">{q.r.res}</text>}</g>;
  };
  return (
    <svg x={p.dx} y={0} width={w} height={p.bodyH} viewBox={`${p.a} 0 ${w} ${p.bodyH}`} overflow="hidden">
      <Show when={p.a < L.TW}>
        <rect x={0} y={0} width={L.TW} height={SCALE_H} fill="#EAF0F8" />
        {L.idw > 0 && <><text x={IDW / 2} y={SCALE_H - 10} font-size={String(fs - 0.5)} font-weight="600" text-anchor="middle" fill="#0C1A2B">ID</text><For each={p.rows}>{(r, i) => <text x={IDW / 2} y={p.ys[i()] + p.hs[i()] / 2 + 3.5} font-size={String(fs - 0.5)} text-anchor="middle" fill="#55606E">{r.uid > 0 ? r.id : ""}</text>}</For></>}
        <For each={visCols()}>{c => (
          <svg x={c.x} y={0} width={c.w} height={p.bodyH} overflow="hidden">
            <text x={4} y={SCALE_H - 10} font-size={String(fs - 0.5)} font-weight="600" fill="#0C1A2B">{c.c.name}</text>
            <For each={p.rows}>{(r, i) => {
              const rs = r.style ?? {}, f = F.parse([rs["*"], rs[c.c.key]].filter(Boolean).join(";")), ind = c.c.key === "name" ? r.lvl * 10 : 0, t = r.cells[c.j] ?? "", z = f.z ? f.z * 0.85 : fs;
              const al = f.a || (RIGHT.includes(c.c.kind) ? "r" : ""), y = p.ys[i()], h = p.hs[i()], two = h > L.RH && c.c.key === "name";
              const lines = two ? (() => { const n = Math.max(8, Math.floor((c.w - 8 - ind) / (z * 0.55))), cut = t.lastIndexOf(" ", n); const k = cut > 4 ? cut : n; return [t.slice(0, k), t.slice(k).trim()]; })() : [t];
              return <g>{f.f && <rect x={0} y={y} width={c.w} height={h} fill={f.f} />}
                <text x={al === "r" ? c.w - 4 : al === "c" ? c.w / 2 : 4 + ind} y={y + (two ? 0 : h / 2 + 3.5)} font-size={String(z)} text-anchor={al === "r" ? "end" : al === "c" ? "middle" : "start"} fill={f.c || (r.kind === "subplan" ? "#4C2A99" : r.off || r.sub ? "#7A8594" : "#1B2733")}
                  style={{ ...(f.n ? { "font-family": `'${f.n}', sans-serif` } : {}), "font-weight": f.b || r.kind !== "task" ? 600 : 400, "font-style": f.i || (r.sub && r.kind !== "subplan") ? "italic" : "normal", "text-decoration": [f.u && "underline", f.s && "line-through"].filter(Boolean).join(" ") || "none" }}>
                  {two ? <For each={lines}>{(l, n) => <tspan x={4 + ind} dy={n() ? z + 2 : z + 3}>{l}</tspan>}</For> : t}</text></g>;
            }}</For>
          </svg>
        )}</For>
        <For each={[...L.lefts, L.TW].filter(x => x >= p.a && x <= p.b)}>{x => <line x1={x} x2={x} y1={0} y2={p.bodyH} stroke={x === L.TW ? "#8D99A8" : "#D5DAE1"} stroke-width={x === L.TW ? ".8" : ".6"} />}</For>
      </Show>
      <Show when={p.b > L.TW}>
        <svg x={L.TW} y={0} width={L.CW} height={p.bodyH} overflow="hidden">
          <rect x={0} y={0} width={L.CW} height={SCALE_H} fill="#F3F5F8" />
          <Show when={L.pd >= 5}><For each={Array.from({ length: Math.ceil(t1 - t0) + 2 }, (_, i) => Math.floor(t0) + i).filter(d => off(d))}>{d => <rect x={C(d)} y={SCALE_H} width={L.pd} height={p.bodyH - SCALE_H} fill="#F2F4F6" />}</For></Show>
          <For each={unit(L.pd)}>{(u, ti) => { const ticks: number[] = []; for (let d = floorU(t0, u); d < t1; d = nextU(d, u)) ticks.push(d); return <For each={ticks}>{d => { const a = C(d), b = C(nextU(d, u)), y = ti() * 17, t = label(d, u, ti() === 0, L.pd); return <g><line x1={a} x2={a} y1={y} y2={y + 17} stroke="#C9CFD8" stroke-width=".6" />{b - a > t.length * 5.6 && <text x={u === "day" ? (a + b) / 2 : Math.max(a, C(t0)) + 3} y={y + 12} font-size="9.5" fill="#33404F" text-anchor={u === "day" ? "middle" : "start"}>{t}</text>}</g>; }}</For>; }}</For>
          <line x1={0} x2={L.CW} y1={17} y2={17} stroke="#C9CFD8" stroke-width=".6" />
          {/* links between tasks on this page (predecessor end → successor start), as on screen */}
          <For each={p.rows}>{(r, i) => <For each={r.bar.links}>{([pu, k]) => {
            const j = p.rows.findIndex(x => x.uid === pu); if (j < 0) return null;
            const pr = p.rows[j], ends = (x: Row) => (x.ms ? [C(x.bar.s) - 6, C(x.bar.s) + 6] : [C(x.bar.s), C(x.bar.f)]), [p1, p2] = ends(pr), [s1, s2] = ends(r);
            const fx = k[0] === "F" ? p2 : p1, tx = k[1] === "S" ? s1 : s2, out_ = k[0] === "F" ? 1 : -1, inn = k[1] === "S" ? 1 : -1, ax = fx + out_ * 5;
            const y1 = p.ys[j] + p.hs[j] / 2, y2 = p.ys[i()] + p.hs[i()] / 2, ym = y2 + (y2 > y1 ? -p.hs[i()] / 2 : p.hs[i()] / 2), bx = tx - inn * 5;
            const pts = (inn === 1 ? tx >= ax : tx <= ax) ? `M${fx} ${y1}H${ax}V${y2}H${tx}` : `M${fx} ${y1}H${ax}V${ym}H${bx}V${y2}H${tx}`;
            const col = r.crit && pr.crit ? "#C73B3B" : "#7A8BA6";
            return <g><path d={pts} fill="none" stroke={col} stroke-width=".8" /><path d={`M${tx} ${y2}l${-inn * 4} -2.6v5.2z`} fill={col} /></g>;
          }}</For>}</For>
          <For each={p.rows}>{(r, i) => <Bar r={r} y={p.ys[i()]} h={p.hs[i()]} />}</For>
          {out()!.summary.status_day && <line x1={C(out()!.summary.status_day!)} x2={C(out()!.summary.status_day!)} y1={SCALE_H} y2={p.bodyH} stroke="#2E9D57" stroke-dasharray="4 3" stroke-width=".8" />}
        </svg>
      </Show>
      {ps.grid && <For each={p.ys}>{(y, i) => <line x1={p.a} x2={p.b} y1={y + p.hs[i()]} y2={y + p.hs[i()]} stroke="#E6E9EE" stroke-width=".5" />}</For>}
      <line x1={p.a} x2={p.b} y1={SCALE_H} y2={SCALE_H} stroke="#8D99A8" stroke-width=".8" />
    </svg>
  );
};

/** The paper, header, footer and legend around a page's content (CSS px at 96 dpi). */
const Frame = (p: { pw: number; ph: number; k: number; W: number; H: number; headH: number; footH: number; legH: number; n: number; pages: number; legend?: JSX.Element | false; children: JSX.Element }) => (
  <div class="ppage" style={{ width: `${p.pw * MM}px`, height: `${p.ph * MM}px` }}>
    <div class="pin" style={{ left: `${ps.margins[3] * MM}px`, top: `${ps.margins[0] * MM}px`, width: `${p.W}px`, height: `${p.H}px`, transform: `scale(${p.k})` }}>
      <Show when={p.headH}><Head n={p.n} pages={p.pages} h={p.headH} /></Show>
      {p.children}
      <Show when={p.legend}><div class="plegend" style={{ height: `${p.legH - 8}px` }}><div class="plt"><Slot t={ps.legendText} page={p.n} pages={p.pages} h={40} /></div>{p.legend}</div></Show>
      <Show when={p.footH}><Foot n={p.n} pages={p.pages} h={p.footH} /></Show>
    </div>
  </div>
);
const legendOn = (n: number, pages: number) => ps.legend === "every" || (ps.legend === "last" && n === pages);
/** The legend: every kind of mark that appears on the printed pages, drawn as it is drawn there. */
const GanttLegend = () => {
  const st = gstyle(), rows = layout()?.rows ?? [], task = (r: Row) => r.kind === "task" && !r.ms;
  type It = [string, JSX.Element];
  const items: It[] = [
    ["Task", <rect x="0" y="2" width="26" height="9" fill={st.fill} stroke={st.edge} stroke-width=".8" />],
    ...(rows.some(r => task(r) && r.crit) ? [["Critical task", <rect x="0" y="2" width="26" height="9" fill={st.cfill} stroke={st.crit} stroke-width=".8" />] as It] : []),
    ...(rows.some(r => task(r) && r.bar.pct > 0) ? [["Progress (% complete)", <g><rect x="0" y="2" width="26" height="9" fill={st.fill} stroke={st.edge} stroke-width=".8" /><rect x="0" y="4.5" width="15" height="4" fill={st.edge} /></g>] as It] : []),
    ...(rows.some(r => r.ms && !r.crit) ? [["Milestone", <path d="M13 0l6 6.5l-6 6.5l-6-6.5z" fill={st.sum} />] as It] : []),
    ...(rows.some(r => r.ms && r.crit) ? [["Critical milestone", <path d="M13 0l6 6.5l-6 6.5l-6-6.5z" fill={st.crit} />] as It] : []),
    ...(rows.some(r => r.kind === "summary" || r.kind === "project") ? [["Summary", <g fill={st.sum}><rect x="0" y="2" width="26" height="4" /><path d="M0 2h5l-5 8z" /><path d="M26 2h-5l5 8z" /></g>] as It] : []),
    ...(rows.some(r => r.kind === "subplan") ? [["Subplan", <g fill="#6D3FC8"><rect x="0" y="2" width="26" height="4" /><path d="M0 2h5l-5 8z" /><path d="M26 2h-5l5 8z" /></g>] as It] : []),
    ...(rows.some(r => r.bar.bs !== null && r.bar.bs !== undefined) ? [["Baseline", <rect x="0" y="5" width="26" height="4" fill="#AEB7C4" />] as It] : []),
    ...(rows.some(r => r.bar.dl !== null && r.bar.dl !== undefined) ? [["Deadline", <g fill="#2E9D57"><path d="M9 0h8l-4 6z" /><rect x="12.5" y="4" width="1" height="9" /></g>] as It] : []),
    ...(rows.some(r => r.bar.links?.length) ? [["Link (task to task)", <g stroke="#7A8BA6" fill="#7A8BA6" stroke-width=".8"><path d="M0 4h14v6h8" fill="none" /><path d="M26 10l-4.5-3v6z" stroke="none" /></g>] as It] : []),
    ...(out()?.summary.status_day ? [["Status date", <line x1="13" x2="13" y1="0" y2="13" stroke="#2E9D57" stroke-dasharray="3 2" />] as It] : []),
  ];
  const per = Math.ceil(items.length / 2), W = 132;
  return (
    <svg width={per * W} height="44" viewBox={`0 0 ${per * W} 44`} font-size="9.5" fill="#1B2733" font-family="'IBM Plex Sans', 'Segoe UI', sans-serif">
      <For each={items}>{([t, mark], k) => <g transform={`translate(${Math.floor(k() / 2) * W + 4},${(k() % 2) * 20 + 5})`}>{mark}<text x="32" y="10">{t}</text></g>}</For>
    </svg>);
};

/** One Gantt page. */
const Sheet = (p: { pg: Page; pages: number }) => {
  const L = layout()!;
  const rows = () => L.rows.slice(p.pg.rows[0], p.pg.rows[1]), hs = () => L.rowsH.slice(p.pg.rows[0], p.pg.rows[1]);
  const ys = () => { let y = SCALE_H; return hs().map(h => { const v = y; y += h; return v; }); };
  const bodyH = () => SCALE_H + hs().reduce((a, b) => a + b, 0) + 1;
  const rep = () => p.pg.x[0] > 0 ? L.RW : 0, w = () => rep() + p.pg.x[1] - p.pg.x[0];
  return (
    <Frame {...L} n={p.pg.n} pages={p.pages} legend={!p.pg.notes && legendOn(p.pg.n, L.pages.length) && <GanttLegend />}>
      <Show when={!p.pg.notes} fallback={<div class="pnotes"><h3>Notes</h3><For each={p.pg.notes}>{n => <div><b>{n[0]} {n[1]}</b><p>{n[2]}</p></div>}</For></div>}>
        <svg width={w()} height={bodyH()} class="psvg" text-rendering="geometricPrecision" font-family="'IBM Plex Sans', 'Segoe UI', sans-serif">
          {rep() > 0 && <Strip a={0} b={L.RW} dx={0} rows={rows()} ys={ys()} hs={hs()} bodyH={bodyH()} />}
          <Strip a={p.pg.x[0]} b={p.pg.x[1]} dx={rep()} rows={rows()} ys={ys()} hs={hs()} bodyH={bodyH()} />
          <rect x={0} y={0} width={w()} height={bodyH()} fill="none" stroke="#8D99A8" stroke-width=".8" />
        </svg>
      </Show>
    </Frame>
  );
};

// ---------- Other views: what they print and how ----------
/** gantt: table + chart strip · net: diagram tiles · cal: a month per page · table: rows down, columns across · flow: the page as on screen. */
export type PKind = "gantt" | "net" | "cal" | "table" | "flow";
export const pkind = (): PKind => { const v = view(); return ["gantt", "tracking", "timeline"].includes(v) ? "gantt" : v === "network" ? "net" : v === "calendar" ? "cal" : ["ressheet", "taskusage", "resusage", "team", "sheets"].includes(v) ? "table" : "flow"; };
const VIEW_NAMES: Record<string, string> = { network: "Network Diagram", calendar: "Calendar", ressheet: "Resource Sheet", taskusage: "Task Usage", resusage: "Resource Usage", team: "Team Planner", sheets: "Sheet", simrisk: "Schedule Risk", lookahead: "Look-ahead", manpower: "Manpower", dashboard: "Dashboard", scurve: "S-Curves", progress: "Progress Report" };
/** Paper and margins for the views other than the Gantt chart. */
const geom = createMemo(() => {
  const p = PAPERS.find(x => x[0] === ps.paper) ?? PAPERS[0], [pw, ph] = ps.landscape ? [p[2], p[1]] : [p[1], p[2]];
  const k = ps.scale / 100, W = (pw - ps.margins[1] - ps.margins[3]) * MM / k, H = (ph - ps.margins[0] - ps.margins[2]) * MM / k;
  return { pw, ph, k, W, H, headH: headH(), footH: footH(), legH: ps.legend === "none" ? 0 : 58 };
});
type PT = { head: string[]; w: number[]; right: boolean[]; imgs?: { src: string; r: number; c: number; w: number; h: number }[]; rows: { c: string[]; b?: boolean; ind?: number; st?: (string | undefined)[] }[] };
type VPage = { n: number; rows?: [number, number]; cols?: number[]; win?: [number, number, number, number]; month?: number };
const unitOf = () => (ps.unit === "auto" ? "week" : ps.unit);
const [vdata] = createResource(() => (pkind() === "gantt" || pkind() === "flow" ? null : [view(), tick(), unitOf(), sheetK(), netBox(), netSums(), netWbs()] as const), async ([v]) => {
  if (v === "network") { const [n, g] = await Promise.all([loadNet(), call<{ rows: Row[] }>("query", { q: { ...query(), cols: NET_COLS } })]); return { net: n, rows: new Map((g?.rows ?? []).map(r => [r.uid, r])) }; }
  if (v === "calendar") return {};
  const t: PT = { head: [], w: [], right: [], rows: [] };
  if (v === "ressheet") {
    const rs = await call<[number, boolean, string[]][]>("resources", { cols: RES_COLS.map(c => c[0]) });
    Object.assign(t, { head: RES_COLS.map(c => c[1]), w: RES_COLS.map(c => c[2]), right: RES_COLS.map(c => ["max", "rate", "ovt", "per_use", "work", "cost"].includes(c[0])), rows: (rs ?? []).map(r => ({ c: r[2] })) });
  } else if (v === "taskusage" || v === "resusage") {
    const u = await call<Usage>("usage", { by: v === "taskusage" ? "task" : "resource", what: "work", unit: unitOf() });
    const f = (x: number) => (x === 0 ? "" : `${Math.round(x * 10) / 10}h`);
    Object.assign(t, { head: [v === "taskusage" ? "Task / assignment" : "Resource / assignment", "Total", ...(u?.buckets ?? []).map(b => fmtB(b, unitOf()))], w: [230, 64, ...(u?.buckets ?? []).map(() => 58)], right: [false, true, ...(u?.buckets ?? []).map(() => true)],
      rows: (u?.rows ?? []).map(r => ({ c: [r.name, f(r.total), ...r.vals.map(f)], b: r.sum || r.of === null, ind: r.lvl })) });
  } else if (v === "team") {
    Object.assign(t, { head: ["Resource / task", "Start", "Finish", "Duration", "% Complete"], w: [260, 90, 90, 80, 80], right: [false, false, false, true, true], rows: [] });
    const tasks = grid().rows.filter(r => r.kind === "task" && !r.off);
    for (const res of P().resources.filter(r => r.kind === "Work")) {
      t.rows.push({ c: [res.name, "", "", "", ""], b: true });
      tasks.filter(r => r.res.split(/,\s*/).some(x => x.replace(/\[.*\]/, "").trim() === res.name)).forEach(r => t.rows.push({ c: [r.name, fmtDay(r.bar.s, "long"), fmtDay(r.bar.f % 1 ? r.bar.f : r.bar.f - 1, "long"), `${Math.round((r.bar.f - r.bar.s) * 10) / 10}d`, `${r.bar.pct}%`], ind: 1 }));
    }
    const un = tasks.filter(r => !r.res); if (un.length) { t.rows.push({ c: ["Unassigned tasks", "", "", "", ""], b: true }); un.forEach(r => t.rows.push({ c: [r.name, fmtDay(r.bar.s, "long"), fmtDay(r.bar.f, "long"), "", `${r.bar.pct}%`], ind: 1 })); }
  } else if (v === "sheets" && P().sheets.length) {
    const k = Math.min(sheetK(), P().sheets.length - 1), sv = await call<SheetOut>("sheet_view", { k }), st = P().sheets[k]?.style ?? {};
    const cells = new Map((sv?.cells ?? []).map(c => [c[0], c])), nr = Math.max(1, sv?.rows ?? 1), nc = Math.max(1, sv?.cols ?? 1);
    const col = (c: number) => { let s = ""; c += 1; while (c > 0) { const m = (c - 1) % 26; s = String.fromCharCode(65 + m) + s; c = Math.floor((c - 1) / 26); } return s; };
    Object.assign(t, { head: ["", ...Array.from({ length: nc }, (_, c) => col(c))], w: [34, ...Array.from({ length: nc }, (_, c) => P().sheets[k].widths?.[c] ?? 96)], right: [true, ...Array.from({ length: nc }, () => false)],
      rows: Array.from({ length: nr }, (_, r) => ({ c: [String(r + 1), ...Array.from({ length: nc }, (_, c) => cells.get(col(c) + (r + 1))?.[2] ?? "")], st: [undefined, ...Array.from({ length: nc }, (_, c) => { const a1 = col(c) + (r + 1); return [cells.get(a1)?.[3] ? "a:r" : "", st[a1] ?? ""].filter(Boolean).join(";"); })] })) });
    t.imgs = (P().sheets[k].images ?? []).flatMap(im => { const m = /^([A-Z]+)(\d+)$/.exec(im.cell), src = picData(im.pic); if (!m || !src) return []; const c = [...m[1]].reduce((a, ch) => a * 26 + ch.charCodeAt(0) - 64, 0), sc = ps.rowH / 22; return [{ src, r: +m[2] - 1, c, w: im.w * sc, h: im.h * sc }]; });
  }
  return { table: t };
});
/** Splits the view into pages. */
const vpages = createMemo((): VPage[] => {
  const G = geom(), d = vdata(), K = pkind(), out_: VPage[] = [];
  if (K === "cal") {
    const s = out()?.summary; if (!s) return [];
    const a = ps.range === "dates" && ps.from ? new Date(ps.from) : dayDate(s.s), b = ps.range === "dates" && ps.to ? new Date(ps.to) : dayDate(s.f);
    for (let m = new Date(a.getFullYear(), a.getMonth(), 1); m <= b; m = new Date(m.getFullYear(), m.getMonth() + 1, 1)) out_.push({ n: out_.length + 1, month: m.getTime() });
    return out_;
  }
  if (!d) return [];
  const room = G.H - G.headH - G.footH - (ps.legend === "every" && K === "net" ? G.legH : 0) - 6;
  if (K === "net" && "net" in d && d.net) {
    const [cw, ch] = netSize(d.net, netBox()), sc = ps.fit > 0 ? Math.max(0.5, Math.min(1.2, (ps.fit * G.W) / cw)) : 1, pw = G.W / sc, ph = room / sc;
    for (let y = 0; y < ch; y += ph) for (let x = 0; x < cw; x += pw) out_.push({ n: out_.length + 1, win: [x, y, pw, ph] });
    return out_;
  }
  if (K === "table" && "table" in d && d.table) {
    const t = d.table, RH = ps.rowH, per = Math.max(1, Math.floor((room - RH) / RH)), key = t.w[0] ?? 0;
    const across: number[][] = []; let cur: number[] = [], w = key;
    for (let c = 1; c < t.w.length; c++) { if (w + t.w[c] > G.W && cur.length) { across.push(cur); cur = []; w = key; } cur.push(c); w += t.w[c]; }
    across.push(cur);
    for (let r = 0; r < Math.max(1, t.rows.length); r += per) for (const cols of across) out_.push({ n: out_.length + 1, rows: [r, Math.min(t.rows.length, r + per)], cols: [0, ...cols] });
  }
  return out_;
});
/** One page of a view other than the Gantt chart. */
const ViewPage = (p: { pg: VPage; pages: number }) => {
  const G = geom(), d = vdata();
  const room = () => G.H - G.headH - G.footH - (legendOn(p.pg.n, p.pages) && pkind() === "net" ? G.legH : 0) - 6;
  return (
    <Frame {...G} n={p.pg.n} pages={p.pages} legend={pkind() === "net" && legendOn(p.pg.n, p.pages) && <NetLegendSvg />}>
      <Switch>
        <Match when={p.pg.win && d && "net" in d && d.net}>
          <svg class="net psvg" width={G.W} height={room()} viewBox={p.pg.win!.join(" ")} preserveAspectRatio="xMinYMin meet" font-family="'IBM Plex Sans', 'Segoe UI', sans-serif">
            <NetDraw n={(d as { net: Net }).net} rows={(d as { rows: Map<number, Row> }).rows} mode={netBox()} />
          </svg>
        </Match>
        <Match when={p.pg.month !== undefined}><MonthPage month={p.pg.month!} W={G.W} H={room()} /></Match>
        <Match when={p.pg.rows && d && "table" in d && d.table}>{(() => { const t = (d as { table: PT }).table; return (<div class="ptwrap">
          <table class="ptable" style={{ "font-size": `${ps.font}px` }}>
            <colgroup><For each={p.pg.cols}>{c => <col style={{ width: `${t.w[c]}px` }} />}</For></colgroup>
            <thead><tr style={{ height: `${ps.rowH}px` }}><For each={p.pg.cols}>{c => <th classList={{ r: t.right[c] }}>{t.head[c]}</th>}</For></tr></thead>
            <tbody><For each={t.rows.slice(p.pg.rows![0], p.pg.rows![1])}>{r => <tr style={{ height: `${ps.rowH}px` }} classList={{ b: r.b }}><For each={p.pg.cols}>{c => <td classList={{ r: t.right[c] }} style={{ ...(c === 0 && r.ind ? { "padding-left": `${4 + r.ind * 12}px` } : {}), ...F.css(r.st?.[c]) }}>{r.c[c]}</td>}</For></tr>}</For></tbody>
          </table>
          <For each={(d as { table: PT }).table.imgs?.filter(im => im.r >= p.pg.rows![0] && im.r < p.pg.rows![1] && p.pg.cols!.includes(im.c))}>{im => {
            const x = p.pg.cols!.slice(0, p.pg.cols!.indexOf(im.c)).reduce((a, c) => a + t.w[c], 0);
            return <img class="psimg" src={im.src} alt="" style={{ left: `${x}px`, top: `${(im.r - p.pg.rows![0] + 1) * ps.rowH}px`, width: `${im.w}px`, height: `${im.h}px` }} />; }}</For></div>); })()}</Match>
      </Switch>
    </Frame>
  );
};
const NetLegendSvg = () => (
  <svg width="460" height="44" class="net" font-size="9.5" font-family="'IBM Plex Sans', 'Segoe UI', sans-serif">
    <g transform="translate(0,4)"><g class="nbox crit"><rect class="shape" width="30" height="16" rx="3" /></g><text x="36" y="12">Critical</text></g>
    <g transform="translate(90,4)"><g class="nbox"><rect class="shape" width="30" height="16" rx="3" /></g><text x="36" y="12">Not critical</text></g>
    <g transform="translate(190,4)"><g class="nbox"><path class="shape" d="M5 0H25L30 8L25 16H5L0 8z" /></g><text x="36" y="12">Milestone</text></g>
    <g transform="translate(280,4)"><g class="nbox"><rect class="shape" width="30" height="16" rx="3" /><path class="x" d="M3 3L27 13M27 3L3 13" /></g><text x="36" y="12">Completed</text></g>
    <g transform="translate(370,4)"><g class="nbox"><rect class="shape" width="30" height="16" rx="3" /><path class="x" d="M3 13L27 3" /></g><text x="36" y="12">In progress</text></g>
    <g transform="translate(0,28)"><path d="M0 6H30" stroke="#D64545" stroke-width="2.4" /><text x="36" y="10">Driving link</text></g>
    <text x="120" y="38">SS Start-to-Start · FF Finish-to-Finish · SF Start-to-Finish · +2d lag</text>
  </svg>
);
/** A calendar month: weeks as rows, tasks as bars across the days they run. */
const MonthPage = (p: { month: number; W: number; H: number }) => {
  const m = new Date(p.month), first = new Date(m); first.setDate(1 - ((first.getDay() + 6) % 7));
  const weeks = Array.from({ length: 6 }, (_, w) => dateDay(new Date(first.getFullYear(), first.getMonth(), first.getDate() + w * 7))).filter((d, w) => w < 4 || dayDate(d).getMonth() === m.getMonth());
  const cw = p.W / 7, wh = (p.H - 44) / weeks.length, tasks = grid().rows.filter(r => r.uid > 0 && r.kind === "task" && !r.off);
  return (
    <svg width={p.W} height={p.H} class="psvg pcal" font-family="'IBM Plex Sans', 'Segoe UI', sans-serif">
      <text x={0} y={18} font-size="16" font-weight="600" fill="#0C1A2B">{m.toLocaleString("en-GB", { month: "long", year: "numeric" })}</text>
      <For each={["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"]}>{(d, i) => <text x={i() * cw + 4} y={38} font-size="10" fill="#55606E">{d}</text>}</For>
      <For each={weeks}>{(w0, wi) => {
        const y = 44 + wi() * wh, lanes: number[] = [];
        const bars = tasks.filter(r => r.bar.s < w0 + 7 && (r.bar.f > w0 || r.bar.s >= w0)).sort((a, b) => a.bar.s - b.bar.s).map(r => {
          const a = Math.max(0, Math.floor(r.bar.s) - w0), b = Math.min(7, Math.max(a + 1, Math.ceil(r.bar.f) - w0)); let l = lanes.findIndex(e => e <= a); if (l < 0) l = lanes.length; lanes[l] = b; return { r, a, b, l };
        });
        const max = Math.max(1, Math.floor((wh - 18) / 14)), more = (d: number) => bars.filter(x => x.l >= max && x.a <= d && x.b > d).length;
        return <g>
          <For each={[0, 1, 2, 3, 4, 5, 6]}>{d => { const day = dayDate(w0 + d); return <g><rect x={d * cw} y={y} width={cw} height={wh} fill={off(w0 + d) ? "#F4F6F8" : "#fff"} stroke="#C9CFD8" stroke-width=".6" />
            <text x={d * cw + 4} y={y + 12} font-size="10" fill={day.getMonth() === m.getMonth() ? "#1B2733" : "#A5ADB8"}>{day.getDate()}</text>{more(d) > 0 && <text x={d * cw + cw - 4} y={y + 12} font-size="9" text-anchor="end" fill="#0B5CAD">+{more(d)} more</text>}</g>; }}</For>
          <For each={bars.filter(x => x.l < max)}>{x => <g><rect x={x.a * cw + 2} y={y + 16 + x.l * 14} width={(x.b - x.a) * cw - 4} height={12} rx="2" fill={x.r.ms ? "#0C1A2B" : x.r.crit ? "#F2B0B0" : "#A9CAF6"} stroke={x.r.crit ? "#C73B3B" : "#2F6FCF"} stroke-width=".6" />
            <svg x={x.a * cw + 4} y={y + 16 + x.l * 14} width={(x.b - x.a) * cw - 8} height={12}><text x={0} y={9.5} font-size="9" fill={x.r.ms ? "#fff" : "#1B2733"}>{x.r.name}</text></svg></g>}</For>
        </g>;
      }}</For>
    </svg>
  );
};

/** Reports and other pages print as they look, with the header and footer on every page. */
const FLOWS: Record<string, () => Promise<{ default: () => JSX.Element }>> = {
  dashboard: () => import("./reports").then(m => ({ default: m.Dashboard })), progress: () => import("./reports").then(m => ({ default: m.Progress })), scurve: () => import("./reports").then(m => ({ default: m.SCurves })),
  reports: () => import("./reports").then(m => ({ default: m.Reports })), builder: () => import("./tools").then(m => ({ default: m.ReportBuilder })), update: () => import("./tools").then(m => ({ default: m.UpdateProgress })),
  relations: () => import("./tools").then(m => ({ default: m.Relationships })), checks: () => import("./views").then(m => ({ default: m.Checks })), risks: () => import("./views").then(m => ({ default: m.Risks })),
  portfolio: () => import("./views").then(m => ({ default: m.Portfolio })), resgraph: () => import("./views").then(m => ({ default: m.ResourceGraph })), setup: () => import("./setup").then(m => ({ default: m.Setup })),
  simrisk: () => import("./risk").then(m => ({ default: m.ScheduleRisk })), lookahead: () => import("./progress").then(m => ({ default: m.Lookahead })), manpower: () => import("./progress").then(m => ({ default: m.Manpower })),
};
const Flow = () => { const C2 = lazy(FLOWS[view()] ?? FLOWS.dashboard); return <C2 />; };
const FlowDoc = (p: { preview?: boolean }) => {
  const G = geom();
  return (
    <table class="pflow" style={{ width: `${G.W}px` }}>
      <thead><tr><td><Show when={G.headH}><Head n={1} pages={1} /></Show></td></tr></thead>
      <tbody><tr><td class="pflowbody"><Flow /></td></tr></tbody>
      <tfoot><tr><td><Show when={G.footH}><Foot n={1} pages={1} flow /></Show>{p.preview && flowPageNo() && <small class="muted">Page numbers are added when printed.</small>}</td></tr></tfoot>
    </table>
  );
};

/** Every view can be printed; the Gantt chart, network, calendar and tables get exact pages. */
export const printable = () => true;
const pagesNow = () => (pkind() === "gantt" ? allPages().length : vpages().length);
const [printing, setPrinting] = createSignal(false);
let endJob: (() => void) | null = null;
/** Lays the pages out for the printer and keeps the page setup (paper, orientation, margins) in
 *  place until the job has finished: the print window reads it after `window.print()` returns. */
const startJob = async () => {
  endJob?.();
  // The page setup is kept with the plan first: its answer would otherwise rebuild the pages while they print.
  await save();
  const G = pkind() === "gantt" ? layout() : geom(); if (!G) return null;
  setPrinting(true);
  const css = document.createElement("style"), flow = pkind() === "flow", m = ps.margins;
  css.textContent = flow ? `@page { size: ${G.pw}mm ${G.ph}mm; margin: ${m[0]}mm ${m[1]}mm ${m[2]}mm ${m[3]}mm;${flowPageNo() ? ` @bottom-center { content: "Page " counter(page) " of " counter(pages); font: 9pt 'IBM Plex Sans', sans-serif; color: #55606E; }` : ""} }` : `@page { size: ${G.pw}mm ${G.ph}mm; margin: 0 }`;
  document.head.append(css);
  document.documentElement.classList.add("printing");
  const end = () => { document.documentElement.classList.remove("printing"); css.remove(); setPrinting(false); window.removeEventListener("afterprint", end); if (endJob === end) endJob = null; };
  endJob = end;
  return { G, flow, end };
};
/** Waits until the pages can print: reports drawn, fonts loaded, pictures decoded, then laid out and painted (two frames). */
const settle = async (flow: boolean) => {
  await new Promise(r => setTimeout(r, flow ? 900 : 100));
  await document.fonts?.ready;
  await Promise.all([...document.querySelectorAll<HTMLImageElement>("#printroot img")].map(i => i.decode().catch(() => {})));
  await Promise.race([new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))), new Promise(r => setTimeout(r, 300))]);
};
/** Print…: the print window, for a printer (or its own Save as PDF). */
export const printNow = async () => {
  const job = await startJob(); if (!job) return;
  await settle(job.flow);
  window.addEventListener("afterprint", job.end);
  window.print();
};
/** Save as PDF: the pages written straight to a file at the paper size, no browser header or footer
 *  (Windows); where that is not available, the print window opens instead. */
export const savePdf = async () => {
  const job = await startJob(); if (!job) return;
  await settle(job.flow);
  const margins = job.flow ? [...ps.margins] : [0, 0, 0, 0];
  try {
    const path = await invoke<string | null>("save_pdf", { name: P().name || "Plan", wMm: job.G.pw, hMm: job.G.ph, margins });
    job.end();
    if (path) toast(`Saved ${path}`, true);
  } catch (e) {
    if (String(e).includes("native-pdf-unavailable")) { window.addEventListener("afterprint", job.end); window.print(); return; }
    job.end(); toast(`The PDF could not be saved: ${e}`);
  }
};

const Slots = (p: { k: "header" | "footer"; label: string }) => (
  <div class="pslots"><b>{p.label}</b>
    <For each={["Left", "Center", "Right"]}>{(l, i) => <label><span>{l}</span><textarea rows="2" value={ps[p.k][i()]} onInput={e => setPs(p.k, i(), e.currentTarget.value)} /></label>}</For>
  </div>
);
const Codes = () => <p class="muted pcodes">Codes: <For each={CODES}>{([c, l]) => <code title={l} onClick={() => navigator.clipboard?.writeText(c).then(() => toast(`${c} copied: paste it into a box.`, true))}>{c}</code>}</For></p>;

/** Header & footer: pictures, a title and the footer, simply; the code boxes for exact control. */
const HeaderFooter = (p: { pickClient: () => void }) => {
  const add = async (i: number) => { const l = await call<string | null>("pick_logo"); if (l) setPs("pics", i, list => [...list, { v: l, h: 40 }]); };
  const move = (i: number, k: number, d: number) => {
    const list = [...ps.pics[i]], j = k + d;
    if (j >= 0 && j < list.length) { [list[k], list[j]] = [list[j], list[k]]; setPs("pics", i, list); return; }
    // Past the end of a place: on to the next place (left ↔ centre ↔ right).
    const to = i + d; if (to < 0 || to > 2) return;
    const [x] = list.splice(k, 1); setPs("pics", i, list); setPs("pics", to, l => (d > 0 ? [x, ...l] : [...l, x]));
  };
  return (
    <Show when={ps.hf === "simple"} fallback={<>
      <Slots k="header" label="Header" /><Slots k="footer" label="Footer" /><Codes />
      <div class="prow"><button class="btn sm" onClick={p.pickClient}>Client logo…</button>{ps.clientLogo && <><img src={ps.clientLogo} class="logoprev" /><button class="btn sm" onClick={() => setPs("clientLogo", "")}>Remove</button></>}</div>
      <p><button class="btn sm" onClick={() => setPs("hf", "simple")}>Back to the simple header & footer</button></p></>}>
      <label class="pl">Pictures at the top</label>
      <p class="muted">As many as you like in each place, side by side; they repeat on every page.</p>
      <For each={["Left", "Centre", "Right"]}>{(l, i) => (
        <div class="pplace"><div class="pph"><b>{l}</b>
          <button class="btn sm" title="A picture from the plan's library (client, vendor and sub-vendor logos)" onClick={e => import("./pictures").then(m => m.pictureMenu(e, id => setPs("pics", i(), list => [...list, { v: `@pic:${id}`, h: 40 }])))}>+ Logo ▾</button>
          <button class="btn sm" onClick={() => add(i())}>+ Picture…</button>
          <Show when={P().logo && !ps.pics[i()].some(x => x.v === "@company")}><button class="btn sm" title="The company logo from File › Options" onClick={() => setPs("pics", i(), list => [...list, { v: "@company", h: 40 }])}>+ Company logo</button></Show></div>
          <For each={ps.pics[i()]} fallback={<span class="muted">No picture</span>}>{(x, k) => (
            <div class="pimg2">
              <div class="thumb">{pic(x.v) ? <img src={pic(x.v)} /> : <span class="muted" title="Add the company logo in File › Options">Company logo (not set)</span>}</div>
              <label title="Height of this picture">Size<input type="range" min="14" max="110" value={x.h} onInput={e => setPs("pics", i(), k(), "h", +e.currentTarget.value)} /></label>
              <span class="acts"><button class="x" title="Move left" onClick={() => move(i(), k(), -1)}>←</button><button class="x" title="Move right" onClick={() => move(i(), k(), 1)}>→</button>
                <button class="x" title="Remove this picture" onClick={() => setPs("pics", i(), list => list.filter((_, j) => j !== k()))}>×</button></span>
            </div>)}</For>
        </div>)}</For>
      <Check label="Title" value={ps.titleOn} set={v => setPs("titleOn", v)} />
      <Show when={ps.titleOn}><input class="wide" value={ps.title} placeholder={`${P().name} (the plan's name)`} onInput={e => setPs("title", e.currentTarget.value)} />
        <div class="prow"><label>Title size<Seg value={String(ps.titleSize)} set={v => setPs("titleSize", +v)} options={[["13", "Small"], ["16", "Medium"], ["22", "Large"], ["30", "Very large"]]} /></label></div></Show>
      <label class="pl">Subtitle</label><input class="wide" value={ps.subtitle} placeholder="e.g. Revision B · Issued for review" onInput={e => setPs("subtitle", e.currentTarget.value)} />
      <label class="pl">At the bottom of each page</label>
      <Check label="Printed date" value={ps.fPrinted} set={v => setPs("fPrinted", v)} />
      <Show when={ps.fPrinted}><div class="pind"><Check label="With the time" value={ps.fTime} set={v => setPs("fTime", v)} /></div></Show>
      <Check label="Page number (Page 1 of 3)" value={ps.fPage} set={v => setPs("fPage", v)} />
      <Check label="Status date" value={ps.fStatus} set={v => setPs("fStatus", v)} />
      <Check label={`Company name${P().company ? ` (${P().company})` : ""}`} value={ps.fCompany} set={v => setPs("fCompany", v)} />
      <div class="prow"><label>Revision<input value={ps.fRev} placeholder="e.g. B" onInput={e => setPs("fRev", e.currentTarget.value)} /></label><label>Your text<input value={ps.fText} placeholder="e.g. Confidential" onInput={e => setPs("fText", e.currentTarget.value)} /></label></div>
      <p class="muted">Nothing else is printed: no program name and no web address.</p>
      <p><button class="btn sm" onClick={() => setPs("hf", "codes")}>Advanced: write them with codes…</button></p>
    </Show>
  );
};

/** File › Print: settings on the left, the pages on the right. */
export const PrintPage = () => {
  load(); loadNotes();
  const [tab, setTab] = createSignal("page"), [zoomP, setZoomP] = createSignal(0.55);
  const tableKeys = () => grid().cols.map(c => c.key).filter(k => k !== "mode");
  onCleanup(save);
  const pickClient = async () => { const l = await call<string | null>("pick_logo"); if (l) setPs("clientLogo", l); };
  const L = layout;
  return (
    <div class="printpage">
      <div class="pset">
        <h1>Print</h1>
        <p class="pwhat">Printing: <b>{pkind() === "gantt" ? (view() === "tracking" ? "Tracking Gantt" : "Gantt Chart") : VIEW_NAMES[view()] ?? (document.querySelector(".content h1")?.textContent || "This page")}</b><br /><span class="muted">Open another view first to print it.</span></p>
        <Show when={printable()}>
          <div class="pbtns"><button class="btn pri big" onClick={savePdf}><svg class="ic"><use href="#i-pdf" /></svg> Save as PDF</button><button class="btn big" onClick={printNow}><svg class="ic"><use href="#i-print" /></svg> Print…</button></div>
          <p class="muted">{pkind() === "flow" ? "As many pages as needed" : `${pagesNow()} ${pagesNow() === 1 ? "page" : "pages"}`} · {ps.paper} {ps.landscape ? "landscape" : "portrait"}. <b>Save as PDF</b> writes exactly these pages to a file; <b>Print…</b> opens the print window to choose a printer.</p>
          <h4>Settings</h4>
          <label class="pl">What to print</label><Pick value={ps.range} set={v => setPs("range", v as "all")} options={[["all", "Entire project"], ["dates", "Specific dates"]]} />
          <Show when={ps.range === "dates"}><div class="prow"><label>From<input type="date" value={ps.from || iso(out()!.summary.s)} onChange={e => setPs("from", e.currentTarget.value)} /></label><label>To<input type="date" value={ps.to || iso(out()!.summary.f)} onChange={e => setPs("to", e.currentTarget.value)} /></label></div></Show>
          <Show when={pkind() === "gantt" && P().subplans?.length}><label class="pl">Subplans of this master</label><Pick value={ps.subs} set={v => setPs("subs", v as "shown")} options={[["shown", "As shown on screen (expanded ones with their tasks)"], ["collapsed", "One row each"], ["expanded", "Every subplan with all its tasks"], ["none", "Leave them out"]]} /></Show>
          <label class="pl">Pages <span class="muted">(e.g. 1-3, 5; empty = all)</span></label><input value={ps.pages} placeholder={`1-${pagesNow() || 1}`} onInput={e => setPs("pages", e.currentTarget.value)} />
          <div class="prow"><label>Paper<Pick value={ps.paper} set={v => setPs("paper", v)} options={PAPERS.map(p => p[0])} /></label><label>Orientation<Pick value={ps.landscape ? "1" : "0"} set={v => setPs("landscape", v === "1")} options={[["1", "Landscape"], ["0", "Portrait"]]} /></label></div>
          <h4>Page Setup</h4>
          <Seg value={tab()} set={setTab} options={[["page", "Page"], ["margins", "Margins"], ["hf", "Header & footer"], ["legend", "Legend"], ["view", "View"]]} />
          <div class="ptab">
            <Show when={tab() === "page"}>
              <label class="pl">Scaling</label>
              <div class="prow"><label><input type="radio" checked={!ps.fit} onChange={() => setPs("fit", 0)} /> Adjust to <input type="number" min="20" max="200" style={{ width: "62px" }} value={ps.scale} onChange={e => setPs("scale", Math.min(200, Math.max(20, +e.currentTarget.value)))} /> % size</label></div>
              <div class="prow"><label><input type="radio" checked={ps.fit > 0} onChange={() => setPs("fit", 1)} /> {pkind() === "gantt" ? "Fit timescale to" : "Fit to"} <input type="number" min="1" max="20" style={{ width: "52px" }} value={ps.fit || 1} onChange={e => setPs("fit", Math.max(1, +e.currentTarget.value))} /> page(s) wide</label></div>
              <p class="muted">{pkind() === "gantt" ? "Adjust prints at that size with the Gantt chart's current zoom. Fit sizes the dates (and shrinks the page if needed) so table and chart fill that many pages wide." : pkind() === "net" ? "Fit sizes the diagram to that many pages wide; it never goes below half size so the boxes stay readable, and continues on more pages instead." : "Adjust scales the whole page."}</p>
            </Show>
            <Show when={tab() === "margins"}>
              <label class="pl">Margins (mm)</label>
              <div class="prow">{(["Top", "Right", "Bottom", "Left"] as const).map((l, i) => <label>{l}<input type="number" min="0" max="60" value={ps.margins[i]} onChange={e => setPs("margins", i, +e.currentTarget.value)} /></label>)}</div>
            </Show>
            <Show when={tab() === "hf"}><HeaderFooter pickClient={pickClient} /></Show>
            <Show when={tab() === "legend"}>
              <label class="pl">Legend on</label><Seg value={ps.legend} set={v => setPs("legend", v as "every")} options={[["every", "Every page"], ["last", "Last page"], ["none", "None"]]} />
              <label class="pl">Legend text</label><textarea rows="3" value={ps.legendText} onInput={e => setPs("legendText", e.currentTarget.value)} /><Codes />
            </Show>
            <Show when={tab() === "view" && pkind() !== "gantt"}>
              <Show when={pkind() === "net"}><label class="pl">Boxes</label><Seg value={netBox()} set={x => setNetBox(x as "standard")} options={[["compact", "Compact"], ["standard", "Standard"], ["detailed", "Detailed"]]} /></Show>
              <Show when={pkind() === "table"}><div class="prow"><label>Text size<input type="number" min="6" max="14" step="0.5" value={ps.font} onChange={e => setPs("font", Math.min(14, Math.max(6, +e.currentTarget.value)))} /></label><label>Row height<input type="number" min="12" max="40" value={ps.rowH} onChange={e => setPs("rowH", Math.min(40, Math.max(12, +e.currentTarget.value)))} /></label></div></Show>
              <Show when={["table", "cal"].includes(pkind()) && ["taskusage", "resusage", "calendar"].includes(view())}><label class="pl">Timescale</label><Pick value={ps.unit} set={v => setPs("unit", v)} options={[["auto", "Weeks"], ["day", "Days"], ["week", "Weeks"], ["month", "Months"], ["quarter", "Quarters"]]} /></Show>
              <Show when={pkind() === "flow"}><p class="muted">Reports print as shown, at the paper width, with your header and footer on every page and page numbers at the bottom.</p></Show>
            </Show>
            <Show when={tab() === "view" && pkind() === "gantt"}>
              <label class="pl">Columns to print</label>
              <div class="prow"><label><input type="radio" checked={!ps.colKeys} onChange={() => setPs("colKeys", null)} /> All columns of the table</label></div>
              <div class="prow"><label><input type="radio" checked={!!ps.colKeys} onChange={() => setPs("colKeys", tableKeys())} /> Choose columns</label></div>
              <Show when={ps.colKeys}><div class="pcols"><For each={tableKeys()}>{k => { const on = () => ps.colKeys!.includes(k), i = () => ps.colKeys!.indexOf(k);
                return <div><Check label={grid().cols.find(c => c.key === k)?.name ?? k} value={on()} set={v => setPs("colKeys", v ? tableKeys().filter(x => x === k || ps.colKeys!.includes(x)) : ps.colKeys!.filter(x => x !== k))} />
                  <Show when={on()}><button class="x" title="Move up" onClick={() => { const v = [...ps.colKeys!], j = i(); if (j > 0) { [v[j - 1], v[j]] = [v[j], v[j - 1]]; setPs("colKeys", v); } }}>↑</button></Show></div>; }}</For></div></Show>
              <Check label="Repeat ID and Task Name on every page" value={ps.repeat} set={v => setPs("repeat", v)} />
              <Check label="Shrink the columns to fit the page" value={ps.fitCols} set={v => setPs("fitCols", v)} />
              <Check label="Wrap long task names" value={ps.wrap} set={v => setPs("wrap", v)} />
              <Check label="Print task notes on the last pages" value={ps.notes} set={v => { setPs("notes", v); loadNotes(); }} />
              <div class="prow"><label>Text size<input type="number" min="6" max="14" step="0.5" value={ps.font} onChange={e => setPs("font", Math.min(14, Math.max(6, +e.currentTarget.value)))} /></label><label>Row height<input type="number" min="12" max="40" value={ps.rowH} onChange={e => setPs("rowH", Math.min(40, Math.max(12, +e.currentTarget.value)))} /></label></div>
              <Check label="Show the ID column" value={ps.idCol} set={v => setPs("idCol", v)} />
              <Check label="Gridlines between rows" value={ps.grid} set={v => setPs("grid", v)} />
              <label class="pl">Timescale</label><Pick value={ps.unit} set={v => setPs("unit", v)} options={[["auto", "Automatic"], ["day", "Days"], ["week", "Weeks"], ["month", "Months"], ["quarter", "Quarters"]]} />
            </Show>
          </div>
          <p><button class="btn sm" onClick={() => setPs(defaults())}>Reset to defaults</button></p>
        </Show>
      </div>
      <div class="pprev">
        <div class="pzoom"><span class="muted">Preview</span><input type="range" min="0.25" max="1.2" step="0.05" value={zoomP()} onInput={e => setZoomP(+e.currentTarget.value)} /><span class="muted">{Math.round(zoomP() * 100)}%</span></div>
        <Switch>
          <Match when={pkind() === "gantt" && L()}><For each={chosen()}>{pg => <div class="pwrap" style={{ width: `${L()!.pw * MM * zoomP()}px`, height: `${L()!.ph * MM * zoomP()}px` }}><div style={{ transform: `scale(${zoomP()})`, "transform-origin": "0 0" }}><Sheet pg={pg} pages={allPages().length} /></div><small>Page {pg.n} of {allPages().length}</small></div>}</For></Match>
          <Match when={pkind() === "flow"}><div class="pwrap pflowprev" style={{ width: `${geom().pw * MM * zoomP()}px` }}><div style={{ zoom: zoomP(), padding: `${ps.margins[0] * MM}px ${ps.margins[1] * MM}px ${ps.margins[2] * MM}px ${ps.margins[3] * MM}px`, width: `${geom().pw * MM}px` }}><FlowDoc preview /></div></div></Match>
          <Match when={true}><Show when={vpages().length} fallback={<p class="pempty">{vdata.loading ? "Preparing pages…" : "Nothing to print in this view."}</p>}><For each={pick(vpages())}>{pg => <div class="pwrap" style={{ width: `${geom().pw * MM * zoomP()}px`, height: `${geom().ph * MM * zoomP()}px` }}><div style={{ transform: `scale(${zoomP()})`, "transform-origin": "0 0" }}><ViewPage pg={pg} pages={vpages().length} /></div><small>Page {pg.n} of {vpages().length}</small></div>}</For></Show></Match>
        </Switch>
      </div>
      <Show when={printing()}><Portal mount={document.body}><div id="printroot" classList={{ flow: pkind() === "flow" }}>
        <Switch>
          <Match when={pkind() === "gantt"}><For each={chosen()}>{pg => <Sheet pg={pg} pages={allPages().length} />}</For></Match>
          <Match when={pkind() === "flow"}><FlowDoc /></Match>
          <Match when={true}><For each={pick(vpages())}>{pg => <ViewPage pg={pg} pages={vpages().length} />}</For></Match>
        </Switch>
      </div></Portal></Show>
    </div>
  );
};
