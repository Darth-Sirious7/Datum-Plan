// Sheet layout shared by the Sheets view and printing: row heights (a row grows to fit a bigger font or
// wrapped text unless its height was set by hand) and the box a picture placed in a cell fits into.
import { createSignal } from "solid-js";
import { SheetData, SheetImage } from "./api";
import * as F from "./fmt";

export const ROW_H = 22, COL_W = 96;
const parseA1 = (s: string): [number, number] | null => { const m = /^([A-Z]+)(\d+)$/.exec(s); if (!m) return null; let c = 0; for (const ch of m[1]) c = c * 26 + ch.charCodeAt(0) - 64; return [+m[2] - 1, c - 1]; };
/** Lines a text needs in a column at a font size (an estimate from average character width). */
const lines = (text: string, z: number, w: number) => text.split("\n").reduce((n, part) => n + Math.max(1, Math.ceil((part.length * z * 0.55) / Math.max(20, w - 10))), 0);

/** Rows that need more than the standard height for their fonts or wrapped text: row → height (px). */
export const autoHeights = (sh: SheetData | undefined, shown: (a1: string) => string): Map<number, number> => {
  const out = new Map<number, number>();
  if (!sh) return out;
  for (const [a1, st] of Object.entries(sh.style ?? {})) {
    const p = F.parse(st), rc = parseA1(a1);
    if (!rc || (!p.w && !(p.z && p.z > 13))) continue;
    if (sh.heights?.[rc[0]] !== undefined || (sh.merges ?? []).some(m => m.startsWith(`${a1}:`))) continue;
    const z = p.z || 12, t = shown(a1);
    const need = Math.ceil((p.w && t ? lines(t, z, sh.widths?.[rc[1]] ?? COL_W) : 1) * z * 1.3 + 6);
    if (need > ROW_H && need > (out.get(rc[0]) ?? 0)) out.set(rc[0], Math.min(600, need));
  }
  return out;
};
let cache: { sh?: SheetData; map?: Map<number, number> } = {};
/** The height of row r as printed (cells: the shown values, for wrapped text). */
export const sheetRowH = (sh: SheetData, r: number, cells: Map<string, [string, string, string, ...unknown[]]>) => {
  if (cache.sh !== sh) cache = { sh, map: autoHeights(sh, a => cells.get(a)?.[2] ?? "") };
  return sh.heights?.[r] ?? cache.map!.get(r) ?? ROW_H;
};
/** A picture placed in its cell: the size that fits inside the cell (or the merged range it starts), keeping its shape. */
export const fitBox = (sh: SheetData, r: number, c: number, im: SheetImage, rowH: (r: number) => number = x => sh.heights?.[x] ?? ROW_H): [number, number] => {
  const m = (sh.merges ?? []).map(x => x.split(":")).find(([a]) => a === im.cell);
  const end = m?.[1] ? parseA1(m[1]) : null, [r1, c1] = end ?? [r, c];
  let bw = -6, bh = -6;
  for (let j = c; j <= c1; j++) bw += sh.widths?.[j] ?? COL_W;
  for (let i = r; i <= r1; i++) bh += rowH(i);
  const k = Math.min(bw / Math.max(1, im.w), bh / Math.max(1, im.h));
  return [Math.max(4, im.w * k), Math.max(4, im.h * k)];
};

/** The sheet tab shown (also what File › Print prints). */
export const [sheetK, setSheetK] = createSignal(0);
/** How the sheet is shown: as cells (normal), with its page breaks, or as the pages that will print. */
export const [sheetMode, setSheetMode] = createSignal<"normal" | "breaks" | "layout">("normal");
