// The Gantt chart and the timeline strip: canvas drawing, timescale, zoom, and dragging bars
// to move, resize, change progress or link.
import { createSignal, createEffect, createMemo, onMount, onCleanup, Show } from "solid-js";
import { Row, fmtDay, iso, dateDay } from "./api";
import { grid, out, sel, setSel, cur, setCur, zoom, setZoom, set, act, openMenu, view, timeline, tick, baseline, call, query, themeTick } from "./store";
import { RH } from "./grid";
import { C } from "./cmds";

export const [showCrit, setShowCrit] = createSignal(true);
export const [showBase, setShowBase] = createSignal(false);
export const [showSlack, setShowSlack] = createSignal(false);
export const [showLate, setShowLate] = createSignal(false);
export const [showLabels, setShowLabels] = createSignal(true);
/** The dates in view in the Gantt chart: the timeline draws them as its zoom box. */
export const [inView, setInView] = createSignal<[number, number] | null>(null);

export type Unit = "day" | "week" | "month" | "quarter" | "year";
const MON = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
export const ymd = (d: number) => { const x = new Date(Date.UTC(1970, 0, 1) + Math.floor(d) * 864e5); return [x.getUTCFullYear(), x.getUTCMonth(), x.getUTCDate()]; };
const dow = (d: number) => (((Math.floor(d) + 3) % 7) + 7) % 7; // 0 = Monday
export const floorU = (d: number, u: Unit) => {
  const [y, m] = ymd(d);
  return u === "day" ? Math.floor(d) : u === "week" ? Math.floor(d) - dow(d) : u === "month" ? Date.UTC(y, m, 1) / 864e5 : u === "quarter" ? Date.UTC(y, m - (m % 3), 1) / 864e5 : Date.UTC(y, 0, 1) / 864e5;
};
export const nextU = (d: number, u: Unit) => {
  const [y, m] = ymd(d);
  return u === "day" ? d + 1 : u === "week" ? d + 7 : u === "month" ? Date.UTC(y, m + 1, 1) / 864e5 : u === "quarter" ? Date.UTC(y, m + 3, 1) / 864e5 : Date.UTC(y + 1, 0, 1) / 864e5;
};
export const label = (d: number, u: Unit, top: boolean, pd: number) => {
  const [y, m, day] = ymd(d);
  if (u === "day") return pd >= 26 ? `${"MTWTFSS"[dow(d)]} ${day}` : "MTWTFSS"[dow(d)];
  if (u === "week") return top ? `${day} ${MON[m]} '${String(y).slice(2)}` : `${day} ${MON[m]}`;
  if (u === "month") return top ? `${MON[m]} ${y}` : MON[m];
  if (u === "quarter") return `Q${m / 3 + 1}${top ? " " + y : ""}`;
  return String(y);
};
const today = () => dateDay(new Date());
const fit = (cv: HTMLCanvasElement, w: number, h: number) => {
  const r = devicePixelRatio || 1;
  if (cv.width !== Math.round(w * r) || cv.height !== Math.round(h * r)) { cv.width = Math.round(w * r); cv.height = Math.round(h * r); cv.style.width = `${w}px`; cv.style.height = `${h}px`; }
  const ctx = cv.getContext("2d")!;
  ctx.setTransform(r, 0, 0, r, 0, 0);
  ctx.clearRect(0, 0, w, h);
  return ctx;
};
/** Gantt Chart Style gallery: task fill and edge, critical fill and edge, summary/milestone. */
export const STYLES: { name: string; fill: string; edge: string; cfill: string; crit: string; sum: string }[] = [
  { name: "Datum blue", fill: "#A9CAF6", edge: "#2F6FCF", cfill: "#F2B0B0", crit: "#C73B3B", sum: "#0C1A2B" },
  { name: "Ocean", fill: "#9FD3E8", edge: "#0072B2", cfill: "#F5C2A0", crit: "#D55E00", sum: "#123A54" },
  { name: "Forest", fill: "#B5DFC6", edge: "#1F7A4D", cfill: "#F2B0B0", crit: "#C0392B", sum: "#123B27" },
  { name: "Amber", fill: "#F9DDA0", edge: "#B7791F", cfill: "#F2B0B0", crit: "#C0392B", sum: "#4A3410" },
  { name: "Violet", fill: "#D7C6F0", edge: "#6C4AB6", cfill: "#F2B0B0", crit: "#C0392B", sum: "#2E1F55" },
  { name: "Graphite", fill: "#D3D7DB", edge: "#5B6878", cfill: "#F3B9B4", crit: "#C0392B", sum: "#1F2933" },
  { name: "Teal", fill: "#A6E3DC", edge: "#0E8C7F", cfill: "#F7C1C8", crit: "#C2185B", sum: "#0B3B36" },
  { name: "Solid blue", fill: "#2F6FCF", edge: "#1A4E9C", cfill: "#D64545", crit: "#9B2C2C", sum: "#0C1A2B" },
  { name: "Solid green", fill: "#2E9D57", edge: "#1E6B3C", cfill: "#D64545", crit: "#9B2C2C", sum: "#123B27" },
  { name: "Monochrome", fill: "#FFFFFF", edge: "#1A1A1A", cfill: "#E0E0E0", crit: "#1A1A1A", sum: "#1A1A1A" },
];
export const gstyle = () => ({ ...STYLES[0], grid: false, ...(out()?.project.ui?.gantt ?? {}) });

/** Canvas colours from the theme and the chosen Gantt Chart Style. */
let K: Record<string, string> = {};
/** "Engineering › A1180 Final documentation issued (FS+5d)" → "Engineering › A1180 Final documentat…". */
const short = (t: string) => { const s = t.replace(/ \([A-Z]{2}[^)]*\)$/, ""); return s.length > 42 ? `${s.slice(0, 41)}…` : s; };
const pal = () => {
  const v = (n: string) => getComputedStyle(document.documentElement).getPropertyValue(n).trim();
  const dark = document.documentElement.dataset.theme === "dark";
  K = { ink: dark ? "#9CC2F7" : "#0C1A2B", line: v("--line"), text: v("--text2"), bg: v("--bg2"), panel: v("--panel"), off: dark ? "#141E2A" : "#F2F4F2", pale: v("--pale"), muted: v("--muted"),
    base: dark ? "#5B6878" : "#AEB7C4", link: dark ? "#6B7F99" : "#7A8BA6", fill: dark ? "#24476E" : "#A9CAF6", blue: v("--blue"), cfill: dark ? "#5A2522" : "#F2B0B0", crit: v("--crit"), ext: dark ? "#B79CF5" : "#6D3FC8", extOut: dark ? "#6FCF97" : "#1E7D46", moved: dark ? "#F5B451" : "#B45309" };
  const g = gstyle();
  if (!dark || g.name !== STYLES[0].name) Object.assign(K, { fill: g.fill, blue: g.edge, cfill: g.cfill, crit: g.crit, ...(dark ? {} : { ink: g.sum }) });
  K.grid = g.grid ? (dark ? "#1E2A38" : "#E8EBEF") : "";
};
export const off = (d: number) => { const s = out()?.summary; if (!s) return dow(d) >= 5; return !s.week[dow(d)] || s.off_days.includes(Math.floor(d)); };

/** The Gantt's canvases while it is shown (for Copy Picture). */
let shown: { scale: HTMLCanvasElement; bars: HTMLCanvasElement } | null = null;

/** The task sheet and the Gantt chart as shown (the rows and dates in view), drawn on one canvas. */
export const ganttPicture = (): HTMLCanvasElement | null => {
  const gs = document.querySelector<HTMLElement>(".gscroll"), head = document.querySelector<HTMLElement>(".ghead");
  if (!shown || !gs || !head) return null;
  const gr = gs.getBoundingClientRect(), sc = shown.scale.getBoundingClientRect(), br = shown.bars.getBoundingClientRect();
  const hh = Math.max(head.getBoundingClientRect().height, sc.height), gw = Math.round(gr.width), W = gw + Math.round(br.width), H = Math.round(hh + br.height), k = 2;
  const cv = document.createElement("canvas");
  cv.width = W * k; cv.height = H * k;
  const ctx = cv.getContext("2d")!;
  ctx.scale(k, k);
  ctx.fillStyle = "#fff"; ctx.fillRect(0, 0, W, H);
  ctx.fillStyle = "#EAF0F8"; ctx.fillRect(0, 0, gw, hh);
  ctx.textBaseline = "middle";
  const text = (t: string, x: number, y: number, w: number, h: number, font: string, color: string, right = false) => {
    ctx.save(); ctx.beginPath(); ctx.rect(x, y, w, h); ctx.clip();
    ctx.font = font; ctx.fillStyle = color; ctx.textAlign = right ? "right" : "left";
    ctx.fillText(t, right ? x + w - 4 : x + 4, y + h / 2); ctx.restore();
  };
  for (const c of head.querySelectorAll<HTMLElement>(".hc:not(.add)")) {
    const r = c.getBoundingClientRect();
    if (r.right > gr.left && r.left < gr.right) text((c.textContent ?? "").replace(/[▾▴⏷]/g, "").trim(), r.left - gr.left, 0, r.width, hh, "600 11px 'IBM Plex Sans', 'Segoe UI', sans-serif", "#0C1A2B");
  }
  for (const row of document.querySelectorAll<HTMLElement>(".gbody .row:not(.blank)")) {
    const rr = row.getBoundingClientRect(), y = rr.top - br.top + hh;
    if (rr.bottom <= br.top || rr.top >= br.bottom) continue;
    const bold = row.classList.contains("sum"), dim = row.classList.contains("off");
    for (const c of row.children as HTMLCollectionOf<HTMLElement>) {
      const r = c.getBoundingClientRect();
      if (r.right <= gr.left || r.left >= gr.right) continue;
      const pad = parseFloat(getComputedStyle(c).paddingLeft) || 0, ind = c.querySelector<HTMLElement>("span")?.offsetWidth ?? 0;
      text((c.textContent ?? "").trim(), r.left - gr.left + (c.classList.contains("nm") ? ind + Math.max(0, pad - 4) : 0), y, r.width - (c.classList.contains("nm") ? ind : 0), rr.height, `${bold ? 600 : 400} 12px 'IBM Plex Sans', 'Segoe UI', sans-serif`, dim ? "#7A8594" : "#1B2733", c.classList.contains("r"));
    }
    ctx.fillStyle = "#E6E9EE"; ctx.fillRect(0, y + rr.height - 1, gw, 1);
  }
  ctx.drawImage(shown.scale, gw, 0, br.width, Math.min(hh, sc.height));
  ctx.drawImage(shown.bars, gw, hh, br.width, br.height);
  ctx.fillStyle = "#8D99A8"; ctx.fillRect(gw, 0, 1, H); ctx.fillRect(0, hh - 1, W, 1);
  return cv;
};

export const Gantt = () => {
  let body!: HTMLDivElement, scaleCv!: HTMLCanvasElement, barsCv!: HTMLCanvasElement;
  const [size, setSize] = createSignal({ w: 800, h: 600 });
  const [tip, setTip] = createSignal<{ x: number; y: number; html: string } | null>(null);
  const d0 = () => Math.floor((out()?.summary.s ?? today()) - 30);
  const d1 = () => Math.ceil((out()?.summary.f ?? today()) + 120);
  const X = (d: number) => (d - d0()) * zoom() - body.scrollLeft;
  type Drag = { r: Row; i: number; mode: "move" | "size" | "link" | "pct"; x: number; y: number; dd: number; mx: number; my: number; pct: number };
  let drag: Drag | null = null;
  let pending = false;
  const draw = () => { if (pending) return; pending = true; requestAnimationFrame(() => { pending = false; if (body) { pal(); drawScale(); drawBars(); const a = d0() + body.scrollLeft / zoom(); setInView([a, a + body.clientWidth / zoom()]); } }); };

  const drawScale = () => {
    const w = size().w, x0 = body.scrollLeft, pd = zoom();
    const ctx = fit(scaleCv, w, 40);
    ctx.fillStyle = K.panel; ctx.fillRect(0, 0, w, 40);
    const [top, bot]: [Unit, Unit] = pd >= 16 ? ["week", "day"] : pd >= 4 ? ["month", "week"] : pd >= 1.2 ? ["quarter", "month"] : ["year", "quarter"];
    const first = d0() + Math.floor(x0 / pd) - 1, last = d0() + Math.ceil((x0 + w) / pd) + 1;
    ctx.font = "11px 'Segoe UI', sans-serif"; ctx.textBaseline = "middle";
    for (const [u, y, isTop] of [[top, 0, true], [bot, 20, false]] as [Unit, number, boolean][]) {
      for (let d = floorU(first, u); d < last; d = nextU(d, u)) {
        const x = (d - d0()) * pd - x0, x2 = (nextU(d, u) - d0()) * pd - x0;
        ctx.fillStyle = K.line; ctx.fillRect(Math.round(x), y + 2, 1, 18);
        ctx.fillStyle = u === "day" && off(d) ? K.muted : K.text;
        let t = label(d, u, isTop, pd), tw = ctx.measureText(t).width;
        if (tw >= x2 - x - 4 && u === "week") { t = String(ymd(d)[2]); tw = ctx.measureText(t).width; }
        if (tw < x2 - x - 4) ctx.fillText(t, u === "day" ? x + (x2 - x - tw) / 2 : Math.max(x + 4, Math.min(4, x2 - tw - 4)), y + 11);
      }
    }
    ctx.fillStyle = K.line; ctx.fillRect(0, 20, w, 1); ctx.fillRect(0, 39, w, 1);
  };

  // Row of each task, worked out once per change of the rows (not on every scroll frame).
  const rowAt = createMemo(() => new Map(grid().rows.map((r, i) => [r.uid, i])));
  const drawBars = () => {
    const { w, h } = size(), top = body.scrollTop, pd = zoom(), rows = grid().rows, trk = view() === "tracking", crit = showCrit() || trk, base = showBase() || trk;
    const ctx = fit(barsCv, w, h);
    if (pd >= 6) { ctx.fillStyle = K.off; for (let d = d0() + Math.floor(body.scrollLeft / pd); X(d) < w; d++) if (off(d)) ctx.fillRect(X(d), 0, pd, h); }
    const a = Math.floor(top / RH), b = Math.min(rows.length, a + Math.ceil(h / RH) + 1);
    if (K.grid) { ctx.fillStyle = K.grid; for (let i = a; i <= b; i++) ctx.fillRect(0, (i + 1) * RH - top - 1, w, 1); }
    const ss = new Set(sel());
    for (let i = a; i < b; i++) {
      const r = rows[i];
      if (ss.has(r.uid)) { ctx.fillStyle = "rgba(47,111,207,.09)"; ctx.fillRect(0, i * RH - top, w, RH); }
      else if (r.hl) { ctx.fillStyle = "rgba(242,178,51,.14)"; ctx.fillRect(0, i * RH - top, w, RH); }
    }
    const line = (d: number, color: string, dash: number[]) => { ctx.strokeStyle = color; ctx.setLineDash(dash); ctx.beginPath(); ctx.moveTo(Math.round(X(d)) + 0.5, 0); ctx.lineTo(Math.round(X(d)) + 0.5, h); ctx.stroke(); ctx.setLineDash([]); };
    line(today(), "#F2B233", [4, 3]);
    const sd = out()?.summary.status_day;
    if (sd) line(sd, "#2E9D57", [6, 3]);
    // Links
    const at = rowAt();
    const ends = (r: Row) => (r.ms ? [X(r.bar.s) - 6, X(r.bar.s) + 6] : [X(r.bar.s), X(r.bar.f)]);
    ctx.lineWidth = 1;
    rows.forEach((r, i) => r.bar.links.forEach(([pu, k]) => {
      const pi = at.get(pu);
      if (pi === undefined || Math.max(i, pi) < a || Math.min(i, pi) > b) return;
      const p = rows[pi], [p1, p2] = ends(p), [s1, s2] = ends(r);
      const fx = k[0] === "F" ? p2 : p1, tx = k[1] === "S" ? s1 : s2, outD = k[0] === "F" ? 1 : -1, inn = k[1] === "S" ? 1 : -1;
      const y1 = pi * RH - top + RH / 2, y2 = i * RH - top + RH / 2, ax = fx + outD * 6;
      const pts: [number, number][] = (inn === 1 ? tx >= ax : tx <= ax) ? [[fx, y1], [ax, y1], [ax, y2], [tx, y2]]
        : (() => { const ym = y2 + (y2 > y1 ? -RH / 2 : RH / 2), bx = tx - inn * 6; return [[fx, y1], [ax, y1], [ax, ym], [bx, ym], [bx, y2], [tx, y2]] as [number, number][]; })();
      const col = crit && r.crit && p.crit ? K.crit : K.link;
      ctx.strokeStyle = col; ctx.beginPath();
      pts.forEach(([x, y], n) => (n ? ctx.lineTo(Math.round(x) + 0.5, Math.round(y) + 0.5) : ctx.moveTo(Math.round(x) + 0.5, Math.round(y) + 0.5)));
      ctx.stroke();
      ctx.fillStyle = col; ctx.beginPath(); ctx.moveTo(tx, y2); ctx.lineTo(tx - inn * 5, y2 - 3.5); ctx.lineTo(tx - inn * 5, y2 + 3.5); ctx.fill();
    }));
    // Bars
    ctx.font = "11px 'Segoe UI', sans-serif"; ctx.textBaseline = "middle";
    for (let i = a; i < b; i++) {
      const r = rows[i], B = r.bar, y = i * RH - top, x1 = X(B.s), x2 = X(B.f);
      const isCrit = crit && r.crit, hasBase = base && B.bs !== null, by = hasBase ? y + 3 : y + 4, bh = hasBase ? 9 : 11;
      if (hasBase && B.bf !== null) {
        ctx.fillStyle = K.base;
        if (B.bf === B.bs) { const bx = X(B.bs!); ctx.beginPath(); ctx.moveTo(bx, y + 12); ctx.lineTo(bx + 4, y + 16); ctx.lineTo(bx, y + 20); ctx.lineTo(bx - 4, y + 16); ctx.fill(); }
        else ctx.fillRect(X(B.bs!), y + 14, Math.max(X(B.bf) - X(B.bs!), 2), 4);
      }
      if (showSlack() && r.kind === "task" && B.lf > B.f) { ctx.fillStyle = "#7FA6DA"; ctx.fillRect(x2, y + 15, X(B.lf) - x2, 2); }
      if (showLate() && r.kind === "task" && B.ls > B.s) { ctx.strokeStyle = K.muted; ctx.setLineDash([2, 2]); ctx.strokeRect(X(B.ls) + 0.5, by + 0.5, X(B.lf) - X(B.ls), bh); ctx.setLineDash([]); }
      let lx = x2 + 6;
      ctx.globalAlpha = r.off ? 0.35 : r.sub && r.kind !== "subplan" ? 0.6 : 1;
      if (r.kind === "group") { ctx.fillStyle = K.muted; ctx.fillRect(x1, y + 7, x2 - x1, 5); }
      else if (r.ms) {
        ctx.fillStyle = isCrit ? K.crit : K.ink;
        ctx.beginPath(); ctx.moveTo(x1, by - 1); ctx.lineTo(x1 + 6, by + 5); ctx.lineTo(x1, by + 11); ctx.lineTo(x1 - 6, by + 5); ctx.fill();
        lx = x1 + 10;
      } else if (r.kind === "summary" || r.kind === "project" || r.kind === "subplan") {
        ctx.fillStyle = r.kind === "project" ? K.muted : r.kind === "subplan" ? K.ext : K.ink;
        ctx.fillRect(x1, y + 6, x2 - x1, 4);
        ctx.beginPath(); ctx.moveTo(x1, y + 6); ctx.lineTo(x1 + 5, y + 6); ctx.lineTo(x1, y + 14); ctx.fill();
        ctx.beginPath(); ctx.moveTo(x2, y + 6); ctx.lineTo(x2 - 5, y + 6); ctx.lineTo(x2, y + 14); ctx.fill();
        if (B.pct > 0) { ctx.fillStyle = "#2E9D57"; ctx.fillRect(x1, y + 10, (x2 - x1) * B.pct / 100, 2); }
      } else {
        const segs = B.segs.length ? B.segs : [[B.s, B.f] as [number, number]];
        const fill = r.manual ? "#BFE6D3" : isCrit ? K.cfill : K.fill, edge = r.manual ? "#2E9D6B" : isCrit ? K.crit : K.blue;
        const done = B.s + (B.f - B.s) * B.pct / 100;
        segs.forEach(([s, f], n) => {
          const a1 = X(s), wd = Math.max(X(f) - a1, 2);
          if (n) { ctx.strokeStyle = edge; ctx.setLineDash([2, 2]); ctx.beginPath(); ctx.moveTo(X(segs[n - 1][1]), by + bh / 2); ctx.lineTo(a1, by + bh / 2); ctx.stroke(); ctx.setLineDash([]); }
          ctx.fillStyle = fill; ctx.fillRect(a1, by, wd, bh);
          // Tracking Gantt: the done part solid, the rest light; Gantt: a progress line through the bar.
          if (trk) { ctx.fillStyle = edge; ctx.fillRect(a1, by, Math.max(0, Math.min(X(f), X(done)) - a1), bh); }
          else { ctx.fillStyle = edge; ctx.fillRect(a1, by + bh / 2 - 2, Math.max(0, Math.min(X(f), X(done)) - a1), 4); }
          ctx.strokeStyle = edge;
          if (r.manual) { ctx.fillRect(a1, by, 3, bh); ctx.fillRect(a1 + wd - 3, by, 3, bh); } else ctx.strokeRect(a1 + 0.5, by + 0.5, wd - 1, bh - 1);
        });
      }
      if (B.dl !== null) {
        const dx = X(B.dl);
        ctx.fillStyle = B.dl < B.f ? K.crit : "#2E9D57";
        ctx.beginPath(); ctx.moveTo(dx - 4, y + 1); ctx.lineTo(dx + 4, y + 1); ctx.lineTo(dx, y + 7); ctx.fill(); ctx.fillRect(dx - 0.5, y + 5, 1, 13);
      }
      let lt = "";
      if (trk && r.kind !== "group") { lt = r.ms ? `${fmtDay(B.s)}` : `${B.pct}%`; lx = r.ms ? lx : Math.max(lx, X(B.bf ?? B.f) + 6); ctx.fillStyle = K.text; ctx.fillText(lt, lx, y + RH / 2); }
      else if (showLabels() && r.kind !== "group") { lt = r.ms ? `${fmtDay(B.s)}` : r.res; ctx.fillStyle = K.text; ctx.fillText(lt, lx, y + RH / 2); }
      // Links between plans: what this task waits for, left of the bar (blue, dashed arrow in);
      // which other plans wait for it, after its label (green arrow out).
      if (r.ext?.length) {
        const t = `${short(r.ext[0])}${r.ext.length > 1 ? ` +${r.ext.length - 1}` : ""}${r.ext_moved ? " – moved, update links" : ""}`, ex = (r.ms ? x1 - 7 : x1) - 2, tw = ctx.measureText(t).width;
        ctx.fillStyle = ctx.strokeStyle = r.ext_moved ? K.moved : K.ext; ctx.setLineDash([3, 2]); ctx.beginPath(); ctx.moveTo(ex - 16, y + RH / 2); ctx.lineTo(ex - 3, y + RH / 2); ctx.stroke(); ctx.setLineDash([]);
        ctx.beginPath(); ctx.moveTo(ex, y + RH / 2); ctx.lineTo(ex - 5, y + RH / 2 - 3); ctx.lineTo(ex - 5, y + RH / 2 + 3); ctx.fill();
        // Too close to the left edge: say it after the bar instead.
        if (ex - 20 - tw > 4) ctx.fillText(t, ex - 20 - tw, y + RH / 2);
        else { lt += lt ? "   " : ""; ctx.fillText(`⇠ ${t}`, lx + ctx.measureText(lt).width, y + RH / 2); lt += `⇠ ${t}`; }
      }
      if (r.ext_out?.length) {
        const t = `→ ${short(r.ext_out[0])}${r.ext_out.length > 1 ? ` +${r.ext_out.length - 1}` : ""}`;
        ctx.fillStyle = K.extOut; ctx.fillText(t, lx + (lt ? ctx.measureText(lt).width + 10 : 0), y + RH / 2);
      }
      ctx.globalAlpha = 1;
    }
    if (drag) {
      const r = drag.r, y = drag.i * RH - top;
      ctx.setLineDash([4, 3]); ctx.strokeStyle = K.blue; ctx.lineWidth = 1.5;
      if (drag.mode === "link") { ctx.beginPath(); ctx.moveTo(X(r.bar.f), y + RH / 2); ctx.lineTo(drag.mx - body.scrollLeft, drag.my - top); ctx.stroke(); }
      else if (drag.mode === "pct") { ctx.fillStyle = "rgba(46,157,87,.55)"; ctx.fillRect(X(r.bar.s), y + 8, (X(r.bar.f) - X(r.bar.s)) * drag.pct / 100, 4); }
      else { const g1 = X(r.bar.s + (drag.mode === "move" ? drag.dd : 0)), g2 = X(r.bar.f + drag.dd); ctx.strokeRect(g1 + 0.5, y + 2.5, Math.max(g2 - g1, 6), RH - 5); }
      ctx.setLineDash([]); ctx.lineWidth = 1;
    }
  };

  const pos = (e: MouseEvent) => { const b = body.getBoundingClientRect(); return [e.clientX - b.left + body.scrollLeft, e.clientY - b.top + body.scrollTop]; };
  const hit = (e: MouseEvent) => {
    const [x, y] = pos(e), i = Math.floor(y / RH), r = grid().rows[i], pd = zoom();
    let part = "";
    if (r && r.kind === "task" && r.uid > 0) {
      const x1 = (r.bar.s - d0()) * pd, x2 = (r.bar.f - d0()) * pd, xp = x1 + (x2 - x1) * r.bar.pct / 100, low = y - i * RH > 10;
      part = r.ms ? (Math.abs(x - x1) <= 8 ? "move" : "") : x >= x2 - 5 && x <= x2 + 3 ? "size" : low && x >= x1 - 2 && Math.abs(x - xp) <= 4 ? "pct" : x >= x1 - 2 && x < x2 - 5 ? "move" : "";
    }
    return { i, x, y, r, part };
  };
  const endDay = (r: Row) => (r.bar.f % 1 === 0 && !r.ms ? r.bar.f - 1 : Math.floor(r.bar.f));
  const tipAt = (e: MouseEvent, html: string) => setTip({ x: Math.min(e.clientX + 14, innerWidth - 300), y: e.clientY + 18, html });
  const esc = (s: string) => s.replace(/[&<>]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c]!);

  const down = (e: MouseEvent) => {
    if (e.button !== 0) return;
    const h = hit(e);
    if (h.r) { setSel(e.ctrlKey ? [...sel(), h.r.uid] : [h.r.uid]); setCur({ row: h.i, col: cur().col }); }
    if (!h.r || !h.part || e.shiftKey || e.ctrlKey) return;
    drag = { r: h.r, i: h.i, mode: h.part as Drag["mode"], x: h.x, y: h.y, dd: 0, mx: h.x, my: h.y, pct: h.r.bar.pct };
    e.preventDefault();
  };
  const move = (e: MouseEvent) => {
    if (!drag) {
      if (!body.contains(e.target as Node)) return setTip(null);
      const h = hit(e), r = h.r;
      body.style.cursor = h.part === "size" ? "ew-resize" : h.part === "pct" ? "col-resize" : h.part ? "grab" : "default";
      if (r && (h.part || r.kind === "summary")) tipAt(e, `<b>${esc(r.name)}</b><br>${fmtDay(r.bar.s, "long")} → ${fmtDay(endDay(r), "long")}<br>${r.bar.pct}% complete${r.res ? "<br>" + esc(r.res) : ""}${r.bar.bs !== null ? `<br>Baseline ${fmtDay(r.bar.bs)} → ${fmtDay(r.bar.bf! - 0.01)}` : ""}`
        + (r.kind === "task" ? '<br><span class="muted">Drag to move · drag the end to resize · drag the progress · drag down to link</span>' : ""));
      else setTip(null);
      return;
    }
    const [x, y] = pos(e), d = drag, pd = zoom();
    d.mx = x; d.my = y; d.dd = Math.round((x - d.x) / pd);
    if (d.mode === "move" && Math.abs(y - d.y) > RH) d.mode = "link";
    if (d.mode === "pct") { const x1 = (d.r.bar.s - d0()) * pd, x2 = (d.r.bar.f - d0()) * pd; d.pct = Math.max(0, Math.min(100, Math.round((x - x1) / Math.max(1, x2 - x1) * 20) * 5)); }
    const t = grid().rows[Math.floor(y / RH)];
    tipAt(e, d.mode === "link" ? `Link <b>${d.r.id}</b> → <b>${t && t !== d.r && t.uid > 0 ? t.id + " " + esc(t.name) : "…"}</b>` : d.mode === "pct" ? `Progress <b>${d.pct}%</b>`
      : d.mode === "move" ? `Start <b>${fmtDay(d.r.bar.s + d.dd, "long")}</b>` : `Finish <b>${fmtDay(Math.max(Math.floor(d.r.bar.s), endDay(d.r) + d.dd), "long")}</b>`);
    draw();
  };
  const up = async () => {
    const d = drag;
    if (!d) return;
    drag = null; setTip(null); draw();
    if (d.mode === "link") { const t = grid().rows[Math.floor(d.my / RH)]; if (t && t !== d.r && t.uid > 0) await act({ LinkSet: { pred: d.r.uid, succ: t.uid, kind: "FS", lag: "" } }); }
    else if (d.mode === "pct") { if (d.pct !== d.r.bar.pct) await set(d.r.uid, "pct", String(d.pct)); }
    else if (d.dd && d.mode === "move") await set(d.r.uid, "start", iso(d.r.bar.s + d.dd));
    else if (d.dd) await set(d.r.uid, "finish", iso(Math.max(Math.floor(d.r.bar.s), endDay(d.r) + d.dd)));
  };
  const setZ = (v: number, keep?: number) => {
    const day = keep ?? d0() + (body.scrollLeft + body.clientWidth / 2) / zoom();
    setZoom(Math.max(0.3, Math.min(120, v)));
    body.scrollLeft = (day - d0()) * zoom() - body.clientWidth / 2;
  };
  const fitAll = () => { const s = out()?.summary; if (!s) return; setZoom(Math.max(0.3, (body.clientWidth - 160) / Math.max(7, s.f - s.s + 4))); body.scrollLeft = (s.s - d0() - 2) * zoom(); };
  /** Shows the dates a..b across the chart (the timeline's zoom box). */
  const range = (e: Event) => { const [a, b] = (e as CustomEvent<[number, number]>).detail; setZoom(Math.max(0.3, Math.min(120, body.clientWidth / Math.max(1, b - a)))); body.scrollLeft = (a - d0()) * zoom(); draw(); };
  const scrollDay = (e: Event) => { body.scrollLeft = Math.max(0, ((e as CustomEvent).detail - d0()) * zoom() - 60); };
  // Follows the sheet; a scroll caused by the sheet is not sent back to it (see grid.tsx).
  let echo = false;
  const syncY = (e: Event) => { const v = (e as CustomEvent).detail, was = body.scrollTop; if (Math.abs(was - v) > 1) { body.scrollTop = v; echo = body.scrollTop !== was; } };

  const ro = new ResizeObserver(() => setSize({ w: body.clientWidth, h: body.clientHeight }));
  onMount(() => {
    shown = { scale: scaleCv, bars: barsCv };
    ro.observe(body);
    addEventListener("mousemove", move); addEventListener("mouseup", up);
    document.addEventListener("gantt-fit", fitAll); document.addEventListener("gantt-range", range); document.addEventListener("gantt-scroll", scrollDay); document.addEventListener("chart-scroll", syncY);
    const s = out()?.summary;
    if (s) body.scrollLeft = Math.max(0, (s.s - d0() - 2) * zoom());
  });
  onCleanup(() => {
    setInView(null);
    shown = null;
    ro.disconnect(); removeEventListener("mousemove", move); removeEventListener("mouseup", up);
    document.removeEventListener("gantt-fit", fitAll); document.removeEventListener("gantt-range", range); document.removeEventListener("gantt-scroll", scrollDay); document.removeEventListener("chart-scroll", syncY);
  });
  createEffect(() => { themeTick(); out(); grid(); sel(); zoom(); size(); showCrit(); showBase(); showSlack(); showLate(); showLabels(); view(); baseline(); draw(); });

  return (
    <div class="chart">
      <canvas class="scale" ref={scaleCv} onDblClick={() => fitAll()} />
      <div class="cwrap">
        <div class="cbody" ref={body} onMouseDown={down} onScroll={() => { draw(); if (!echo) document.dispatchEvent(new CustomEvent("grid-scroll", { detail: body.scrollTop })); echo = false; document.dispatchEvent(new CustomEvent("tl-redraw")); }}
          onWheel={e => { if (e.ctrlKey) { e.preventDefault(); setZ(zoom() * (e.deltaY < 0 ? 1.2 : 1 / 1.2), d0() + (body.scrollLeft + e.offsetX) / zoom()); } }}
          onDblClick={e => { const h = hit(e); if (h.r?.sub) import("./plans").then(m => m.goToLink(h.r!.sub!, h.r!.suid ?? 0)); else C.info(); }} onContextMenu={e => { const h = hit(e); if (h.r?.sub) { e.preventDefault(); const x = h.r; return void import("./plans").then(m => m.subRowMenu(e, x)); } if (h.r && !sel().includes(h.r.uid)) { setSel([h.r.uid]); setCur({ row: h.i, col: cur().col }); } openMenu(e, [
            { label: "Task Information…", icon: "info", run: C.info }, { label: "Scroll to Task", icon: "scroll", run: C.scrollTo }, { label: "Split Task…", icon: "split", run: C.split }, { label: "Mark on Track", icon: "track", run: C.onTrack }, "-",
            { label: "Show Critical Path", run: () => setShowCrit(!showCrit()), on: showCrit() }, { label: "Show Baseline", run: () => setShowBase(!showBase()), on: showBase() },
            { label: "Show Slack", run: () => setShowSlack(!showSlack()), on: showSlack() }, { label: "Show Late Bars", run: () => setShowLate(!showLate()), on: showLate() },
            { label: "Show Bar Labels", run: () => setShowLabels(!showLabels()), on: showLabels() }, "-", { label: "Zoom In", run: C.zoomIn }, { label: "Zoom Out", run: C.zoomOut }, { label: "Entire Project", run: fitAll },
          ]); }}>
          <div style={{ width: `${(d1() - d0()) * zoom()}px`, height: `${(grid().rows.length + 3) * RH}px` }} />
        </div>
        <canvas class="bars" ref={barsCv} />
      </div>
      <Show when={tip()}>{t => <div id="tip" style={{ left: `${t().x}px`, top: `${t().y}px` }} innerHTML={t().html} />}</Show>
    </div>
  );
};

/** The timeline bar: the whole project with the tasks marked Display on Timeline. */
export const Timeline = () => {
  let cv!: HTMLCanvasElement;
  const [w, setW] = createSignal(800);
  const [items, setItems] = createSignal<Row[]>([]);
  createEffect(() => { tick(); grid(); (async () => { const g = await call<{ rows: Row[] }>("query", { q: { ...query(), cols: ["name"], filter: { name: "", rules: [{ field: "timeline", test: "equals", value: "Yes", value2: "", or: false }], summaries: false, highlight: false }, group: [], collapsed: [], level: 0, project_summary: false } }); setItems(g?.rows ?? []); })(); });
  const draw = () => {
    if (!cv || !timeline()) return;
    pal();
    const h = 64, ctx = fit(cv, w(), h), s = out()?.summary;
    ctx.fillStyle = K.bg; ctx.fillRect(0, 0, w(), h);
    ctx.font = "11px 'Segoe UI', sans-serif"; ctx.textBaseline = "middle";
    if (!s) return;
    const L = 76, R = w() - 76, a = s.s, b = Math.max(s.f, a + 1), px = (d: number) => L + (d - a) / (b - a) * (R - L);
    ctx.fillStyle = K.muted; ctx.fillText("Start", 10, 20); ctx.fillText(s.start, 10, 36); ctx.fillText("Finish", R + 10, 20); ctx.fillText(s.finish, R + 10, 36);
    ctx.fillStyle = K.pale; ctx.fillRect(L, 22, R - L, 20);
    const step: Unit = (b - a) / (R - L) > 3 ? "quarter" : "month";
    ctx.fillStyle = K.muted;
    for (let d = nextU(floorU(a, step), step); d < b; d = nextU(d, step)) { ctx.fillRect(px(d), 22, 1, 20); if (px(nextU(d, step)) - px(d) > 40) ctx.fillText(label(d, step, true, 1), px(d) + 3, 53); }
    const lanes: number[] = [];
    for (const r of items()) {
      const x1 = px(r.bar.s), x2 = Math.max(px(r.bar.f), x1 + 2);
      if (r.ms) { ctx.fillStyle = "#F2B233"; ctx.beginPath(); ctx.moveTo(x1, 24); ctx.lineTo(x1 + 7, 32); ctx.lineTo(x1, 40); ctx.lineTo(x1 - 7, 32); ctx.fill(); ctx.fillStyle = K.ink; ctx.fillText(r.name, x1 + 8, 12); continue; }
      let lane = lanes.findIndex(e => e <= x1); if (lane < 0) lane = lanes.length; lanes[lane] = x2 + 4;
      const y = 23 + (lane % 2) * 9;
      ctx.fillStyle = r.kind === "summary" ? K.blue : "#6E9FE6"; ctx.fillRect(x1, y, x2 - x1, 9);
      ctx.fillStyle = K.ink; if (ctx.measureText(r.name).width < x2 - x1 + 40) ctx.fillText(r.name, x1, lane % 2 ? 58 : 12);
    }
    if (!items().length) { ctx.fillStyle = K.muted; ctx.fillText("Right-click a task and choose Add to Timeline", L + 8, 32); }
    const t = today(); ctx.strokeStyle = "#F2B233"; ctx.beginPath(); ctx.moveTo(px(t), 18); ctx.lineTo(px(t), 46); ctx.stroke();
    if (s.status_day) { ctx.strokeStyle = "#2E9D57"; ctx.setLineDash([3, 2]); ctx.beginPath(); ctx.moveTo(px(s.status_day), 18); ctx.lineTo(px(s.status_day), 46); ctx.stroke(); ctx.setLineDash([]); }
    // Zoom box: the part of the project shown in the Gantt chart.
    const v = inView();
    if (v) {
      const x1 = Math.max(L - 4, px(v[0])), x2 = Math.min(R + 4, px(v[1]));
      ctx.fillStyle = "rgba(11,92,173,.08)"; ctx.fillRect(x1, 18, x2 - x1, 28);
      ctx.strokeStyle = K.blue; ctx.lineWidth = 1.5; ctx.strokeRect(x1 + 0.5, 18.5, x2 - x1 - 1, 27); ctx.lineWidth = 1;
      ctx.fillStyle = K.blue; ctx.fillRect(x1 - 2, 26, 4, 12); ctx.fillRect(x2 - 2, 26, 4, 12);
    }
  };
  const ro = new ResizeObserver(() => setW(cv.parentElement!.clientWidth));
  onMount(() => { ro.observe(cv.parentElement!); document.addEventListener("tl-redraw", draw); });
  onCleanup(() => { ro.disconnect(); document.removeEventListener("tl-redraw", draw); });
  createEffect(() => { themeTick(); items(); w(); timeline(); out(); inView(); draw(); });
  // Drag the box to scroll, drag an edge to zoom, click outside it to jump there, double-click for the whole project.
  const geo = () => { const s = out()!.summary, L = 76, R = w() - 76, a = s.s, b = Math.max(s.f, a + 1); return { L, R, day: (x: number) => a + (x - L) / (R - L) * (b - a), px: (d: number) => L + (d - a) / (b - a) * (R - L) }; };
  const part = (x: number) => { const v = inView(); if (!v || !out()) return ""; const g = geo(), x1 = g.px(v[0]), x2 = g.px(v[1]); return Math.abs(x - x1) < 6 ? "l" : Math.abs(x - x2) < 6 ? "r" : x > x1 && x < x2 ? "m" : ""; };
  const send = (a: number, b: number) => document.dispatchEvent(new CustomEvent("gantt-range", { detail: [a, b] }));
  const down = (e: MouseEvent) => {
    const v = inView(); if (!out() || !v || e.button !== 0) return;
    const g = geo(), k = part(e.offsetX), x0 = e.clientX, [a0, b0] = v, per = (g.day(1) - g.day(0));
    if (!k) { const d = g.day(e.offsetX), half = (b0 - a0) / 2; return send(d - half, d + half); }
    const mv = (m: MouseEvent) => { const dd = (m.clientX - x0) * per; if (k === "m") send(a0 + dd, b0 + dd); else if (k === "l") send(Math.min(a0 + dd, b0 - 2), b0); else send(a0, Math.max(b0 + dd, a0 + 2)); };
    const up = () => { removeEventListener("mousemove", mv); removeEventListener("mouseup", up); };
    addEventListener("mousemove", mv); addEventListener("mouseup", up);
  };
  return <div class="tlwrap" title="Drag the box to scroll the Gantt chart, drag its edges to zoom, double-click for the whole project">
    <canvas ref={cv} onMouseDown={down} onMouseMove={e => { cv.style.cursor = { l: "ew-resize", r: "ew-resize", m: "grab", "": "pointer" }[part(e.offsetX)]!; }} onDblClick={() => document.dispatchEvent(new CustomEvent("gantt-fit"))}
      onContextMenu={e => openMenu(e, [{ label: "Show Entire Project", run: () => document.dispatchEvent(new CustomEvent("gantt-fit")) }, { label: "Hide Timeline", run: C.timelineBar }])} /></div>;
};
