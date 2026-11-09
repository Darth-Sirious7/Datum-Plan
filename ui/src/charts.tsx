// Charts in a clean, modern style: smooth lines with a soft fill, rounded bars, light gridlines,
// round legend markers and a dark tooltip. Every chart resizes with its box (and for printing),
// says so when it has nothing to show, and can be saved as a PNG.
import { onMount, onCleanup, createEffect, on, JSX, Show } from "solid-js";
import { Chart, registerables, type ChartDataset, type Plugin, type ScriptableContext } from "chart.js";
import { call, toast, themeTick, P } from "./store";
import { money } from "./api";

Chart.register(...registerables);

/** The chart colours: clear and easy to tell apart. With Options › Colour-blind safe colours they
 * become the Okabe-Ito set, told apart with every common kind of colour-blindness. */
const STD = { blue: "#3B82F6", orange: "#F59E0B", green: "#10B981", red: "#EF4444", purple: "#8B5CF6", sky: "#38BDF8", yellow: "#EAB308", grey: "#A3ABB8", black: "#1F2937" };
const SAFE = { blue: "#0072B2", orange: "#E69F00", green: "#009E73", red: "#D55E00", purple: "#CC79A7", sky: "#56B4E9", yellow: "#C9B200", grey: "#A3ABB8", black: "#1F2937" };
export const COL = { ...STD };
/** Switches the chart and status colours (the page's `data-cb`) to the colour-blind safe set or back. */
export const setColourSafe = (on: boolean) => { Object.assign(COL, on ? SAFE : STD); document.documentElement.dataset.cb = on ? "1" : "0"; };

const cssv = (n: string) => getComputedStyle(document.documentElement).getPropertyValue(n).trim();

/** A marker line with a small label (the status date), and a soft line under the pointer. */
const lines = (marker: () => number | null | undefined, label: string): Plugin => ({
  id: "lines",
  afterDatasetsDraw(ch) {
    const { ctx, chartArea: a, scales } = ch;
    const x = scales.x;
    if (!x) return;
    const m = marker();
    ctx.save();
    if (m !== null && m !== undefined && m >= 0) {
      const px = x.getPixelForValue(m);
      ctx.strokeStyle = COL.green; ctx.setLineDash([4, 4]); ctx.lineWidth = 1.5;
      ctx.beginPath(); ctx.moveTo(px, a.top + 18); ctx.lineTo(px, a.bottom); ctx.stroke();
      ctx.setLineDash([]);
      ctx.font = "600 10px 'IBM Plex Sans', sans-serif";
      const w = ctx.measureText(label).width + 12;
      ctx.fillStyle = COL.green; ctx.beginPath(); ctx.roundRect(px - w / 2, a.top, w, 17, 8.5); ctx.fill();
      ctx.fillStyle = "#fff"; ctx.textAlign = "center"; ctx.textBaseline = "middle"; ctx.fillText(label, px, a.top + 9);
    }
    const act = ch.tooltip?.getActiveElements?.() ?? [];
    if (act.length && !(ch.options as { indexAxis?: string }).indexAxis?.startsWith("y")) {
      const px = act[0].element.x;
      ctx.strokeStyle = cssv("--line3") || "#ccc"; ctx.lineWidth = 1;
      ctx.beginPath(); ctx.moveTo(px, a.top); ctx.lineTo(px, a.bottom); ctx.stroke();
    }
    ctx.restore();
  },
});
/** The last value of each line, at its end, in the line's colour (as in an engineering report). */
const ends = (f: () => Fmt | undefined): Plugin => ({
  id: "ends",
  afterDatasetsDraw(ch) {
    const { ctx, chartArea: a } = ch;
    ctx.save(); ctx.font = "600 10.5px 'IBM Plex Sans', sans-serif"; ctx.textBaseline = "middle";
    const used: number[] = [];
    ch.data.datasets.forEach((d, k) => {
      if ((d as { type?: string }).type !== "line" || !ch.isDatasetVisible(k)) return;
      const pts = ch.getDatasetMeta(k).data, vals = d.data as (number | null)[];
      let i = vals.length - 1; while (i >= 0 && (vals[i] === null || vals[i] === undefined)) i--;
      if (i < 0 || !pts[i]) return;
      const text = fmtv(vals[i] as number, f()), w = ctx.measureText(text).width;
      let y = pts[i].y;
      while (used.some(u => Math.abs(u - y) < 13)) y -= 13;
      used.push(y);
      const x = Math.min(pts[i].x + 6, a.right + 44 - w);
      ctx.fillStyle = cssv("--bg2") || "#fff"; ctx.fillRect(x - 2, y - 7, w + 4, 14);
      ctx.fillStyle = String(d.borderColor ?? "#333"); ctx.fillText(text, x, y);
    });
    ctx.restore();
  },
});
/** White (or dark) background so saved pictures are not transparent. */
const paper: Plugin = { id: "paper", beforeDraw(ch) { const c = ch.ctx; c.save(); c.globalCompositeOperation = "destination-over"; c.fillStyle = cssv("--bg2") || "#fff"; c.fillRect(0, 0, ch.width, ch.height); c.restore(); } };
/** Fills fade from the line colour to nothing at the bottom of the plot. */
const fade = (color: string) => (ctx: ScriptableContext<"line">) => {
  const { chart } = ctx, a = chart.chartArea;
  if (!a) return color + "30";
  const g = chart.ctx.createLinearGradient(0, a.top, 0, a.bottom);
  g.addColorStop(0, color + "55"); g.addColorStop(1, color + "04");
  return g;
};
// Charts follow their box when the page is laid out for printing, so nothing is cut off.
// They draw at twice the screen's sharpness (three times for printing), so lines and text stay crisp in a PDF.
const live = new Set<Chart>();
const sharp = (k: number) => live.forEach(c => { c.options.devicePixelRatio = Math.max(k, window.devicePixelRatio || 1); c.resize(); });
addEventListener("beforeprint", () => sharp(3)); addEventListener("afterprint", () => sharp(2));

export type Fmt = "pct" | "money" | "hours" | "num";
export const fmtv = (v: number, f: Fmt = "num") => f === "pct" ? `${v.toFixed(1)}%` : f === "money" ? money(v, { ...P().currency, decimals: 0 }) : f === "hours" ? `${Math.round(v).toLocaleString()} h` : (Math.round(v * 100) / 100).toLocaleString();

export interface ChartProps {
  title: string;
  type: "line" | "bar" | "doughnut" | "pie";
  labels: string[];
  datasets: ChartDataset<"line" | "bar", (number | null)[]>[];
  y?: string;
  x?: string;
  fmt?: Fmt;
  max?: number;
  stacked?: boolean;
  horizontal?: boolean;
  marker?: number | null;
  markerLabel?: string;
  /** "fill": as tall as its box (dashboard tiles). */
  size?: "tall" | "short" | "fill";
  /** No title bar (a dashboard tile has its own). */
  bare?: boolean;
  /** The day (since 1970) of each label, for a time axis in Datum Plot. */
  days?: number[];
  extra?: JSX.Element;
}

export const ChartBox = (p: ChartProps) => {
  let cv!: HTMLCanvasElement;
  let ch: Chart | undefined;
  /** Nothing to draw: no periods, or every value empty or zero (no costs, no hours yet). */
  const empty = () => !p.labels.length || p.datasets.every(d => d.data.every(v => !v));
  const build = (first = false) => {
    if (ch) { live.delete(ch); ch.destroy(); }
    const ink = cssv("--text"), muted = cssv("--muted"), grid = cssv("--line2");
    const font = { family: "'IBM Plex Sans', 'Segoe UI', sans-serif", size: 11 };
    const axis = (title?: string, isY = false) => ({
      title: { display: !!title, text: title, color: muted, font: { ...font, weight: 500 as const }, padding: { bottom: 6 } },
      border: { display: !isY, color: cssv("--line3") || "#9AA3AF", width: 1 },
      grid: { display: isY, color: grid, drawTicks: false, lineWidth: 1, borderDash: [3, 3] },
      ticks: { color: muted, font, padding: 8, maxRotation: 0, autoSkipPadding: 18, ...(isY ? { maxTicksLimit: 6, callback: (v: string | number) => fmtv(+v, p.fmt) } : {}) },
      stacked: p.stacked,
    });
    const vAxis = { ...axis(p.y, true), beginAtZero: true, ...(p.max !== undefined ? { max: p.max } : {}) };
    const hAxis = axis(p.x, false);
    // A horizontal bar chart names every bar (task names), so none may be skipped.
    if (p.horizontal) hAxis.ticks = { ...hAxis.ticks, autoSkip: false, color: ink } as typeof hAxis.ticks;
    const many = p.labels.length > 40;
    const round = p.type === "doughnut" || p.type === "pie";
    if (round) {
      // One colour per slice, a thin gap between slices, the legend beside the ring.
      const pal = Object.values(COL).filter(c => c !== COL.black);
      const bg = cssv("--bg2") || "#fff";
      ch = new Chart(cv, {
        type: p.type,
        data: { labels: p.labels, datasets: p.datasets.map(d => ({ ...d, backgroundColor: p.labels.map((_, i) => pal[i % pal.length]), borderColor: bg, borderWidth: 2, hoverOffset: 6 })) as never },
        options: {
          responsive: true, maintainAspectRatio: false, devicePixelRatio: Math.max(2, window.devicePixelRatio || 1), animation: first ? { duration: 600, easing: "easeOutQuart" } : false, cutout: p.type === "doughnut" ? "62%" : 0,
          layout: { padding: 6 },
          plugins: {
            legend: { position: "right", labels: { color: ink, font, usePointStyle: true, pointStyle: "circle", boxWidth: 7, boxHeight: 7, padding: 10 } },
            tooltip: { backgroundColor: "rgba(17, 24, 39, 0.92)", cornerRadius: 8, padding: 10, usePointStyle: true, titleFont: { ...font, weight: 600 as const }, bodyFont: font,
              callbacks: { label: (c: { label: string; parsed: unknown }) => ` ${c.label}: ${fmtv(c.parsed as number, p.fmt)}` } },
          },
        } as never,
        plugins: [paper],
      });
      live.add(ch);
      return;
    }
    ch = new Chart(cv, {
      type: p.type,
      data: { labels: p.labels, datasets: p.datasets as ChartDataset<"line" | "bar", (number | null)[]>[] },
      options: {
        responsive: true, maintainAspectRatio: false, animation: first ? { duration: 600, easing: "easeOutQuart" } : false,
        indexAxis: p.horizontal ? "y" : "x",
        interaction: { mode: "index", intersect: false },
        layout: { padding: { top: 2, right: p.type === "line" || p.datasets.some(d => (d as { type?: string }).type === "line") ? 48 : 8, left: 2 } },
        devicePixelRatio: Math.max(2, window.devicePixelRatio || 1),
        elements: {
          line: { borderWidth: 2.5, tension: 0.4, cubicInterpolationMode: "monotone", borderCapStyle: "round" },
          point: { radius: 0, hoverRadius: many ? 3 : 5, hitRadius: 10, hoverBorderWidth: 2, hoverBackgroundColor: "#fff" },
          bar: { borderWidth: 0, borderRadius: p.stacked ? 3 : 6, borderSkipped: p.stacked ? "start" : false },
        },
        datasets: { bar: { categoryPercentage: 0.72, barPercentage: 0.88, maxBarThickness: 28 } },
        scales: (p.horizontal ? { x: vAxis, y: hAxis } : { x: hAxis, y: vAxis }) as never,
        plugins: {
          legend: { position: "top", align: "end", labels: { color: ink, font, usePointStyle: true, pointStyle: "circle", boxWidth: 7, boxHeight: 7, padding: 14 } },
          tooltip: {
            backgroundColor: "rgba(17, 24, 39, 0.92)", titleColor: "#fff", bodyColor: "#E5E7EB", borderWidth: 0, cornerRadius: 8, padding: 10, boxPadding: 4, caretSize: 6,
            usePointStyle: true, titleFont: { ...font, weight: 600 as const }, bodyFont: font,
            callbacks: { label: c => ` ${c.dataset.label}: ${c.parsed[p.horizontal ? "x" : "y"] === null ? "—" : fmtv(c.parsed[p.horizontal ? "x" : "y"] as number, p.fmt)}` },
          },
        },
      },
      plugins: [paper, lines(() => p.marker, `${p.markerLabel ?? "Status"}${p.marker !== null && p.marker !== undefined && p.labels[p.marker] ? ` · ${p.labels[p.marker]}` : ""}`), ends(() => p.fmt)],
    });
    ch.options.color = muted;
    live.add(ch);
  };
  // The box can change width without the window changing (sidebar, panes): follow it.
  const ro = new ResizeObserver(() => ch?.resize());
  onMount(() => { build(true); ro.observe(cv.parentElement!); });
  onCleanup(() => { ro.disconnect(); if (ch) { live.delete(ch); ch.destroy(); } });
  createEffect(on(() => [p.labels, p.datasets, themeTick(), p.marker], () => build(), { defer: true }));
  const save = async () => {
    if (!ch) return;
    const path = await call<string | null>("save_file", { name: `${p.title}.png`, data: ch.toBase64Image("image/png", 1) });
    if (path) toast(`Saved ${path}`, true);
  };
  /** A one-way copy of the values in Datum Plot: changes there never come back. */
  const toPlot = async () => {
    const units = p.fmt === "pct" ? "%" : p.fmt === "money" ? P().currency.code : p.fmt === "hours" ? "h" : "";
    const series = p.datasets.map(d => ({ name: String(d.label ?? ""), values: (d.data as (number | null)[]).map(v => (v === null || v === undefined ? null : +v)), color: String(typeof d.borderColor === "string" && d.borderColor !== cssv("--bg2") ? d.borderColor : typeof d.backgroundColor === "string" ? d.backgroundColor : ""), line: (d as { type?: string }).type === "line" || p.type === "line" }));
    const msg = await call<string>("open_in_plot", { chart: { title: p.title, y: p.y ?? "", units, labels: p.labels, days: p.days ?? [], series } });
    if (msg) toast(msg, true);
  };
  return (
    <div class="chartbox" data-marker={p.marker ?? ""} classList={{ tall: p.size === "tall", short: p.size === "short", grow: p.size === "fill", bare: p.bare }}>
      <Show when={!p.bare}><header><h3>{p.title}</h3>{p.extra}<button onClick={() => ch && import("./picture").then(m => m.copyCanvas(ch!.canvas, p.title))} title="Copy the chart as a picture, to paste into a document, slides or a workbook">Copy</button><button onClick={save} title="Save as PNG">PNG</button><button onClick={toPlot} title="Open a copy of these values in Datum Plot, to style the graph there. The plan does not change.">Datum Plot</button></header></Show>
      <div class="cv" classList={{ blank: empty() }}><canvas ref={cv} /></div>
      <Show when={empty()}><p class="nodata">{p.fmt === "money" ? "No costs in this plan yet." : p.fmt === "hours" ? "No hours assigned yet." : "No data yet."}</p></Show>
    </div>
  );
};

/** Line dataset helpers: `fill` fades the area under the line. */
export const line = (label: string, data: (number | null)[], color: string, dash?: number[], fill = false): ChartDataset<"line", (number | null)[]> =>
  ({ type: "line", label, data, borderColor: color, backgroundColor: fill ? fade(color) : color, borderDash: dash, borderWidth: dash ? 2 : 2.5, fill: fill ? "origin" : false, spanGaps: false, pointBackgroundColor: color, pointBorderColor: color });
export const bar = (label: string, data: (number | null)[], color: string, stack?: string): ChartDataset<"bar", (number | null)[]> =>
  ({ type: "bar", label, data, backgroundColor: color, hoverBackgroundColor: color + "CC", stack });
