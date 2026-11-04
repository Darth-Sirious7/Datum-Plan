// Spreadsheet tabs inside the plan (dashboards, quantities, estimates, vendor lists). They work like
// a spreadsheet: type = and click cells to build a formula, functions are suggested as you type, the fill
// handle and paste carry formulas along, and the plan is read live by name: Tasks[Finish],
// Resources[Country], ProjectFinish. The GridCraft engine (in crates/core) calculates everything.
import { JSX, For, Show, batch, createEffect, createMemo, createResource, createSignal, onCleanup, onMount } from "solid-js";
import { SheetOut, SheetData, SheetPage } from "./api";
import { act, call, grid, P, tick, openMenu, toast, Item } from "./store";
import * as F from "./fmt";
import { parseTsv, toTsv, transpose, series, clip } from "./cells";
import { createSel, aggregate } from "./selection";
import { Catalog, ERRORS, Suggestion, a1, blankStrings, callAt, canPoint, colName, cycleAnchor, refs, sheetRef, sigArgs, suggest } from "./formulas";
import { CountryList, FlagText } from "./flags";
import { Help } from "./help";
import { SheetImage } from "./api";
import { pictureMenu, picData } from "./pictures";
import { autoHeights, fitBox, sheetK, setSheetK, sheetMode, setSheetMode } from "./sheetlayout";
import { sheetPages, SheetPagesView, pageRoom } from "./print";
import { Icon } from "./ui";
import { askText, askYes, Dialog, Row } from "./ui";
import { condLooks, lookCss, condMenu, bounds as cfBounds } from "./condfmt";
import type { CondRule } from "./api";
import { colorPalette } from "./colors";
import { openDialog } from "./store";

const RH = 22, CW = 96, HW = 46;
const COLORS = ["#2F6FDE", "#D64545", "#7A4FD8", "#1F9D55", "#C77700", "#C2185B"];
const parseA1 = (s: string): [number, number] => { const m = s.match(/^([A-Z]+)(\d+)$/)!; let c = 0; for (const ch of m[1]) c = c * 26 + ch.charCodeAt(0) - 64; return [+m[2] - 1, c - 1]; };
/** Brackets left open are closed when Enter is pressed. */
const closeParens = (t: string) => { const b = blankStrings(t); return t + ")".repeat(Math.max(0, (b.match(/\(/g)?.length ?? 0) - (b.match(/\)/g)?.length ?? 0))); };
/** Number formats offered on the toolbar and the cell menu: label and format (spreadsheet format code or our short name). */
export const FORMATS: [string, string][] = [["General", ""], ["Number (1,234.50)", "#,##0.00"], ["Whole number (1,235)", "#,##0"], ["Percent of 1 (0.28 → 28%)", "0%"], ["Percent of 100 (28 → 28%)", '0"%"'],
  ["Currency", "money"], ["Date", "date"], ["Long date (30 October 2026)", "d mmmm yyyy"], ["Month (Oct-26)", "mmm-yy"], ["Text", "@"], ["Country with flag", "flag"]];
/** One more or one fewer decimal in a format code. */
const decimals = (code: string, d: number) => {
  // Currency becomes its own code ("$"#,##0.00) so its decimals can change too.
  if (code === "money") { const c = P().currency; code = `"${c.symbol}"#,##0${c.decimals ? "." + "0".repeat(c.decimals) : ""}`; }
  if (["date", "flag", "@"].includes(code)) return code;
  if (code === "pct") code = "0.0%";
  return (code || "0").split(";").map(sec => sec.replace(/0(\.(0+))?(?=[^0.]*$)/, (_m, _x, zs?: string) => { const n = Math.max(0, (zs?.length ?? 0) + d); return n ? `0.${"0".repeat(n)}` : "0"; })).join(";");
};
/** The last copy inside Sheets: typed inputs (formulas move when pasted elsewhere) and where they came from. */
const sclip = { merges: [] as { r0: number; c0: number; r1: number; c1: number }[], text: "", inputs: [] as string[][], styles: [] as string[][], fmts: [] as string[][], k: 0, r0: 0, c0: 0, cut: false };

/** Cell styles (palette after GridCraft's built-in styles). */
export const CELL_STYLES: [string, string][] = [
  ["Normal", ""], ["Good", "c:#006100;f:#C6EFCE"], ["Bad", "c:#9C0006;f:#FFC7CE"], ["Neutral", "c:#9C5700;f:#FFEB9C"],
  ["Calculation", "b;c:#FA7D00;f:#F2F2F2;bt:thin #7F7F7F;bb:thin #7F7F7F;bl:thin #7F7F7F;br:thin #7F7F7F"], ["Check Cell", "b;c:#FFFFFF;f:#A5A5A5;bt:dbl #3F3F3F;bb:dbl #3F3F3F;bl:dbl #3F3F3F;br:dbl #3F3F3F"],
  ["Input", "c:#3F3F76;f:#FFCC99;bt:thin #7F7F7F;bb:thin #7F7F7F;bl:thin #7F7F7F;br:thin #7F7F7F"], ["Output", "b;c:#3F3F3F;f:#F2F2F2;bt:thin #3F3F3F;bb:thin #3F3F3F;bl:thin #3F3F3F;br:thin #3F3F3F"],
  ["Note", "f:#FFFFCC;bt:thin #B2B2B2;bb:thin #B2B2B2;bl:thin #B2B2B2;br:thin #B2B2B2"], ["Warning Text", "c:#FF0000"], ["Explanatory Text", "i;c:#7F7F7F"], ["Linked Cell", "c:#FA7D00;bb:dbl #FF8001"],
  ["Title", "b;c:#0C1A2B;z:24"], ["Heading 1", "b;c:#0C1A2B;z:20;bb:thick #0B5CAD"], ["Heading 2", "b;c:#0C1A2B;z:17;bb:thick #9DC3E6"], ["Heading 3", "b;c:#0C1A2B;z:14;bb:med #BDD7EE"], ["Heading 4", "b;c:#0C1A2B"],
  ["Total", "b;bt:thin #0B5CAD;bb:dbl #0B5CAD"], ["Accent", "b;c:#FFFFFF;f:#0B5CAD"], ["Accent, light", "c:#0C1A2B;f:#DDEBF7"],
];
/** The cell style gallery: sections of styles. Accents use the family colours, at 20, 40 and 60 % strength and full. */
const ACCENTS: [string, string][] = [["Blue", "#0B5CAD"], ["Orange", "#E67E22"], ["Green", "#1E8449"], ["Purple", "#7D3C98"], ["Teal", "#138D90"], ["Red", "#C0392B"]];
const tint = (h: string, t: number) => "#" + [1, 3, 5].map(i => Math.round(parseInt(h.slice(i, i + 2), 16) + (255 - parseInt(h.slice(i, i + 2), 16)) * (1 - t)).toString(16).padStart(2, "0")).join("").toUpperCase();
export const STYLE_SECTIONS: [string, [string, string][]][] = [
  ["Good, bad and neutral", CELL_STYLES.filter(x => ["Normal", "Good", "Bad", "Neutral"].includes(x[0]))],
  ["Data and checks", CELL_STYLES.filter(x => ["Calculation", "Check Cell", "Input", "Output", "Note", "Warning Text", "Explanatory Text", "Linked Cell"].includes(x[0]))],
  ["Titles and headings", CELL_STYLES.filter(x => ["Title", "Heading 1", "Heading 2", "Heading 3", "Heading 4", "Total"].includes(x[0]))],
  ...[0.2, 0.4, 0.6, 1].map(t => [t < 1 ? `Accents, ${t * 100} %` : "Accents", ACCENTS.map(([n, c]) => [t < 1 ? `${n} ${t * 100} %` : n, t < 1 ? `c:#0C1A2B;f:${tint(c, t)}` : `b;c:#FFFFFF;f:${c}`] as [string, string])] as [string, [string, string][]]),
];
/** Standard tab and line colours. */
export const STD_COLORS = ["#C00000", "#FF0000", "#FFC000", "#FFFF00", "#92D050", "#00B050", "#00B0F0", "#0070C0", "#002060", "#7030A0", "#000000", "#7F7F7F"];
/** What the Sheet Tools ribbon tab can do on the sheet shown (set while Sheets is open). */
export type SheetApi = Record<string, (...a: any[]) => unknown> & { frozen: () => boolean; pen: () => [string, string]; page: () => SheetPage; setPage: (x: Partial<SheetPage>) => Promise<unknown> };
export const [sheetApi, setSheetApi] = createSignal<SheetApi | null>(null);

/** The sheet tab shown (also what File › Print prints). */
export { sheetK, setSheetK };
export const Sheets = () => {
  const k = sheetK, setK = setSheetK;
  const [data] = createResource(() => [tick(), k(), P().sheets.length], () => (P().sheets.length ? call<SheetOut>("sheet_view", { k: Math.min(k(), P().sheets.length - 1) }) : undefined));
  const [cat] = createResource(() => P().fields.length, () => call<Catalog>("sheet_catalog"));
  const cells = createMemo(() => new Map((data()?.cells ?? []).map(c => [c[0], c])));
  const sheet = () => P().sheets[k()];
  const looks = createMemo(() => condLooks(sheet(), cells()));
  const input = (kk: number, r: number, c: number) => P().sheets[kk]?.cells[a1(r, c)] ?? "";
  const fmtOf = (r: number, c: number) => sheet()?.fmt?.[a1(r, c)] ?? "";
  // Editing: the text, the cell it goes to (which may be on another tab while pointing), the caret,
  // the reference last added by clicking (a drag replaces it), and the chosen suggestion.
  const [edit, setEdit] = createSignal<string | null>(null);
  const [home, setHome] = createSignal<{ k: number; r: number; c: number } | null>(null);
  const [caret, setCaret] = createSignal(0);
  const [point, setPoint] = createSignal<{ start: number; end: number } | null>(null);
  const [sug, setSug] = createSignal(0), [hideSug, setHideSug] = createSignal(false);
  const [quick, setQuick] = createSignal(false);
  const [top, setTop] = createSignal(0);
  const [h, setH] = createSignal(600);
  const [tw, setTw] = createSignal<[number, number] | null>(null);
  // The grid reaches past the used cells, and grows as you go further (by keys or by scrolling).
  const [reach, setReach] = createSignal<[number, number]>([0, 0]);
  const nrows = () => Math.max(200, (data()?.rows ?? 0) + 30, reach()[0] + 40), ncols = () => Math.max(26, (data()?.cols ?? 0) + 6, reach()[1] + 8);
  const hidR = createMemo(() => new Set(sheet()?.hide_rows ?? [])), hidC = createMemo(() => new Set(sheet()?.hide_cols ?? []));
  const width = (c: number) => (hidC().has(c) ? 0 : tw()?.[0] === c ? tw()![1] : sheet()?.widths?.[c] ?? CW);
  const lefts = createMemo(() => { let x = HW; return Array.from({ length: ncols() }, (_, c) => { const l = x; x += width(c); return l; }); });
  // Row heights: tops[r] is where row r starts (below the header); a row being dragged uses th.
  const [th, setTh] = createSignal<[number, number] | null>(null);
  // Rows grow to fit a bigger font or wrapped text, unless their height was set by hand.
  const auto = createMemo(() => autoHeights(sheet(), a => cells().get(a)?.[2] ?? ""));
  const rh = (r: number) => (hidR().has(r) ? 0 : th()?.[0] === r ? th()![1] : sheet()?.heights?.[r] ?? auto().get(r) ?? RH);
  const tops = createMemo(() => { let y = 0; const t = new Array<number>(nrows() + 1); for (let r = 0; r <= nrows(); r++) { t[r] = y; y += rh(r); } return t; });
  const top_ = (r: number) => tops()[Math.min(r, nrows())] ?? r * RH;
  /** The row at y (px below the header). */
  const rowAt = (y: number) => { const t = tops(); let lo = 0, hi = nrows() - 1; if (y < 0) return -1; while (lo < hi) { const m = (lo + hi + 1) >> 1; if (t[m] <= y) lo = m; else hi = m - 1; } return lo; };
  // Merged ranges: the top-left cell shows across the range.
  const merges = createMemo(() => (sheet()?.merges ?? []).map(m => { const [a, b = a] = m.split(":"); const [r0, c0] = parseA1(a), [r1, c1] = parseA1(b); return { r0, c0, r1, c1 }; }));
  const mergeAt = (r: number, c: number) => merges().find(m => r >= m.r0 && r <= m.r1 && c >= m.c0 && c <= m.c1);
  let box!: HTMLDivElement, cellEl: HTMLInputElement | undefined, barEl!: HTMLInputElement, lastEl: "cell" | "bar" = "cell";
  const ro = new ResizeObserver(() => setH(box.clientHeight));
  // The shared grid engine's selection (as in the task sheet): several ranges, whole rows and columns.
  const S = createSel(() => [nrows() - 1, ncols()], (r, c) => reveal(r, c));
  const cur = (): [number, number] => [S.cur().row, S.cur().col];
  const end = (): [number, number] => { const g = S.ranges()[S.ranges().length - 1]; return g ? [g.r1, g.c1] : cur(); };
  const setCur = ([r, c]: [number, number]) => { S.setCur({ row: r, col: c }); S.setRanges([{ r0: r, c0: c, r1: r, c1: c }]); };
  const setEnd = ([r, c]: [number, number]) => S.drag(r, c);
  const reveal = (r: number, c: number) => {
    if (r > reach()[0] - 10 || c > reach()[1] - 4) setReach([Math.max(reach()[0], r), Math.max(reach()[1], c)]);
    if (!box) return;
    const [fr, fc] = frozen(), y0 = top_(fr), x0 = lefts()[fc] ?? HW;
    if (r >= fr) { if (top_(r) - y0 < box.scrollTop) box.scrollTop = top_(r) - y0; else if (top_(r + 1) + RH > box.scrollTop + box.clientHeight - 24) box.scrollTop = top_(r + 1) + RH - box.clientHeight + 24; }
    if (c >= fc) { const l = lefts()[c] ?? 0, w = width(c); if (l - x0 < box.scrollLeft) box.scrollLeft = l - x0; else if (l + w > box.scrollLeft + box.clientWidth - 16) box.scrollLeft = l + w - box.clientWidth + 16; }
  };
  // The number format the Format Painter copied (with the style).
  let paintFmt: string | null = null;
  createEffect(() => { if (F.painter() !== null && F.target() === fmtTarget) paintFmt = fmtOf(...cur()); });
  const fmtTarget: F.Target = {
    get: () => { const st = sheet()?.style ?? {}, out: [string, string][] = []; S.each((r, c) => out.push([a1(r, c), st[a1(r, c)] ?? ""]), 20000); return out; },
    set: list => act({ SheetStyle: { k: k(), cells: list } }),
    paste: (what, tr) => pasteSpecial(what, tr),
  };
  onMount(() => { if (box) ro.observe(box); F.setTarget(fmtTarget); });
  onCleanup(() => { ro.disconnect(); if (F.target() === fmtTarget) F.setTarget(null); if (edit() !== null) commit(null); });
  const range = () => { const g = S.last(); return [g.r0, g.c0, g.r1, g.c1]; };
  const inR = S.inRange;
  const put = async (list: [string, string][], kk = k()) => { if (list.length) await act({ SheetCells: { k: kk, cells: list, fmt: null } }); };
  const go = (r: number, c: number, ext = false, add = false) => S.pick(r, c, ext, add);

  // ---------- editing ----------
  const el = () => (lastEl === "cell" && cellEl?.isConnected ? cellEl : barEl);
  const startEdit = (text: string, r = cur()[0], c = cur()[1], typed = false) => batch(() => { setHome({ k: k(), r, c }); setEdit(text); setCaret(text.length); setPoint(null); setSug(0); setHideSug(false); setQuick(typed); });
  const setText = (t: string, at: number) => {
    batch(() => { setEdit(t); setCaret(at); setSug(0); });
    const e = el();
    queueMicrotask(() => { if (!e) return; e.value = t; e.focus(); e.setSelectionRange(at, at); });
  };
  const cancel = () => { const hm = home(); batch(() => { setEdit(null); setHome(null); setPoint(null); }); if (hm && hm.k !== k()) setK(hm.k); box?.focus(); };
  /** Saves the typed text into the cell being edited; `move` then moves the cursor (null: stay put). */
  const commit = async (move: [number, number] | null) => {
    const v = edit(), hm = home();
    if (v === null || !hm) return;
    batch(() => { setEdit(null); setHome(null); setPoint(null); });
    if (hm.k !== k()) setK(hm.k);
    // "+A1*2" or "-SUM(B:B)" starts a formula, as in a spreadsheet; "-5" stays a number.
    const v2 = /^[+-](?=[A-Za-z$(])/.test(v) ? `=${v}` : v;
    const val = v2.startsWith("=") && v2.length > 1 ? closeParens(v2) : v2;
    if (move) { const m = mergeAt(hm.r, hm.c), [fr, fc] = m ? [move[0] > 0 ? m.r1 : m.r0, move[1] > 0 ? m.c1 : m.c0] : [hm.r, hm.c]; go(Math.max(0, fr + move[0]), Math.max(0, fc + move[1])); box?.focus(); }
    if (val !== input(hm.k, hm.r, hm.c)) await put([[a1(hm.r, hm.c), val]], hm.k);
  };
  /** Ctrl+Enter: the text typed goes into every selected cell (formulas move with each cell). */
  const fillSel = async () => {
    const v = edit(), hm = home(); if (v === null || !hm) return;
    batch(() => { setEdit(null); setHome(null); setPoint(null); });
    const val = v.startsWith("=") && v.length > 1 ? closeParens(v) : v, items: [string, number, number][] = [], at: string[] = [];
    S.each((r, c) => { items.push([val, r - hm.r, c - hm.c]); at.push(a1(r, c)); }, 20000);
    const moved = val.startsWith("=") ? await call<string[]>("formula_shift", { items }) : null;
    await put(at.map((a, i) => [a, moved?.[i] ?? val]));
    box?.focus();
  };
  const typed = (e: HTMLInputElement) => batch(() => { setEdit(e.value); setCaret(e.selectionStart ?? e.value.length); setPoint(null); setSug(0); setHideSug(false); });
  const sugs = createMemo((): Suggestion[] => (edit() === null || hideSug() ? [] : suggest(edit()!, caret(), cat())));
  const accept = (s: Suggestion) => { const t = edit()!; setText(t.slice(0, s.from) + s.insert + t.slice(caret()), s.from + s.insert.length); };
  /** The function the caret is in: its signature split into arguments, the current one marked. */
  const sig = createMemo(() => {
    if (edit() === null || !edit()!.startsWith("=")) return null;
    const c = callAt(edit()!, caret()), f = c && cat()?.functions.find(x => x[0] === c.name);
    if (!c || !f) return null;
    const args = sigArgs(f[1]), dots = args[args.length - 1] === "...";
    return { name: f[0], args, at: dots && c.arg >= args.length - 1 ? args.length - 2 : Math.min(c.arg, args.length - 1), about: f[2] };
  });
  /** Adds (or, while dragging, replaces) a reference at the caret: A1, A1:B5, Sheet2!A1, C:C. */
  const pointText = (ref: string) => {
    const t = edit()!, p = point(), s = p ? p.start : caret(), e = p ? p.end : caret();
    setPoint({ start: s, end: s + ref.length });
    setText(t.slice(0, s) + ref + t.slice(e), s + ref.length);
  };
  const pointTo = (r0: number, c0: number, r1: number, c1: number) => {
    const pre = home() && home()!.k !== k() ? `${sheetRef(sheet().name)}!` : "";
    const [a, b] = [a1(Math.min(r0, r1), Math.min(c0, c1)), a1(Math.max(r0, r1), Math.max(c0, c1))];
    pointText(pre + (a === b ? a : `${a}:${b}`));
  };
  const pointing = () => edit() !== null && (point() !== null || canPoint(edit()!, caret()));
  const edKey = (e: KeyboardEvent) => {
    e.stopPropagation();
    const list = sugs();
    if (list.length && (e.key === "ArrowDown" || e.key === "ArrowUp")) { e.preventDefault(); setSug((sug() + (e.key === "ArrowDown" ? 1 : -1) + list.length) % list.length); return; }
    if (list.length && e.key === "Tab") { e.preventDefault(); accept(list[sug()]); return; }
    if (e.key === "F4") { e.preventDefault(); const [t, c] = cycleAnchor(edit()!, caret()); setText(t, c); return; }
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); fillSel(); return; }
    if (e.key === "Enter") { e.preventDefault(); commit([e.shiftKey ? -1 : 1, 0]); }
    else if (e.key === "Tab") { e.preventDefault(); commit([0, e.shiftKey ? -1 : 1]); }
    else if (e.key === "Escape") { e.preventDefault(); if (list.length) setHideSug(true); else cancel(); }
    else if (quick() && !edit()!.startsWith("=") && e.key.startsWith("Arrow")) { e.preventDefault(); commit(({ ArrowDown: [1, 0], ArrowUp: [-1, 0], ArrowLeft: [0, -1], ArrowRight: [0, 1] } as Record<string, [number, number]>)[e.key]); }
  };
  const caretOf = (e: Event) => setCaret((e.currentTarget as HTMLInputElement).selectionStart ?? 0);
  const blur = (e: FocusEvent) => { const to = e.relatedTarget as HTMLElement | null; if (to?.closest(".sheetgrid, .fbar, .sheettabs, .dlg, .fhint")) return; if (edit() !== null) commit(null); };
  // While pointing at another tab, the formula bar holds the text.
  createEffect(() => { if (edit() !== null && home() && home()!.k !== k()) { lastEl = "bar"; queueMicrotask(() => { barEl?.focus(); barEl?.setSelectionRange(caret(), caret()); }); } });

  // ---------- keys, clipboard, fill ----------
  const clear = () => { const out: [string, string][] = []; S.each((r, c) => { if (input(k(), r, c)) out.push([a1(r, c), ""]); }); put(out); };
  const key = (e: KeyboardEvent) => {
    if (picSel() >= 0 && edit() === null && (e.key === "Delete" || e.key === "Backspace")) { e.preventDefault(); putImgs(imgs().filter((_, j) => j !== picSel())); setPicSel(-1); return; }
    // Keys typed before the cell editor has focus still go into it (fast typing loses nothing).
    if (edit() !== null) { if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) { e.preventDefault(); const t = edit()!, at = caret(); setText(t.slice(0, at) + e.key + t.slice(at), at + 1); } return; }
    const [r, c] = e.shiftKey ? end() : cur();
    // Arrows step over a merged range as over one cell, and land on its top-left cell.
    const mv = (dr: number, dc: number) => {
      e.preventDefault();
      const here = mergeAt(r, c), [fr, fc] = here ? [dr > 0 ? here.r1 : here.r0, dc > 0 ? here.c1 : here.c0] : [r, c];
      let tr = Math.max(0, fr + dr), tc = Math.max(0, fc + dc);
      // Hidden rows and columns are stepped over.
      for (let n = 0; n < 5000 && (hidR().has(tr) || hidC().has(tc)) && tr + Math.sign(dr) >= 0 && tc + Math.sign(dc) >= 0 && (dr || dc); n++) { tr += Math.sign(dr); tc += Math.sign(dc); }
      const m = mergeAt(tr, tc);
      if (m && !e.shiftKey) { go(m.r0, m.c0); setEnd([m.r1, m.c1]); } else go(tr, tc, e.shiftKey);
    };
    const ctrl = e.ctrlKey || e.metaKey;
    if (e.altKey && e.key === "=") { e.preventDefault(); return autoSum("SUM"); }
    if (ctrl && e.shiftKey && (e.key === "+" || e.key === "=")) { e.preventDefault(); return insertCells(false); }
    if (ctrl && !e.shiftKey && e.key === "-") { e.preventDefault(); return insertCells(true); }
    // Ctrl+arrow: to the edge of the data (the next filled cell after a gap, or the last of a filled run).
    const filled = (rr: number, cc: number) => !!cells().get(a1(rr, cc))?.[2];
    const jump = (dr: number, dc: number) => {
      e.preventDefault();
      let [rr, cc] = [r, c]; const lim = (x: number, y: number) => x >= 0 && y >= 0 && x < nrows() && y < ncols();
      if (lim(rr + dr, cc + dc) && filled(rr, cc) && filled(rr + dr, cc + dc)) { while (lim(rr + dr, cc + dc) && filled(rr + dr, cc + dc)) { rr += dr; cc += dc; } }
      else { rr += dr; cc += dc; while (lim(rr, cc) && !filled(rr, cc) && lim(rr + dr, cc + dc)) { rr += dr; cc += dc; } if (!lim(rr, cc)) { rr -= dr; cc -= dc; } }
      go(Math.max(0, rr), Math.max(0, cc), e.shiftKey);
    };
    if (ctrl && e.key.startsWith("Arrow")) return jump(...(({ ArrowDown: [1, 0], ArrowUp: [-1, 0], ArrowLeft: [0, -1], ArrowRight: [0, 1] } as Record<string, [number, number]>)[e.key]));
    const page = Math.max(1, Math.floor((box?.clientHeight ?? 400) / RH) - 2);
    switch (e.key) {
      case "Home": e.preventDefault(); return ctrl ? go(0, 0, e.shiftKey) : go(r, 0, e.shiftKey);
      case "End": e.preventDefault(); return ctrl ? go(Math.max(0, (data()?.rows ?? 1) - 1), Math.max(0, (data()?.cols ?? 1) - 1), e.shiftKey) : go(r, Math.max(0, (data()?.cols ?? 1) - 1), e.shiftKey);
      case "PageDown": return mv(page, 0); case "PageUp": return mv(-page, 0);
      case "ArrowDown": return mv(1, 0); case "ArrowUp": return mv(-1, 0); case "ArrowLeft": return mv(0, -1); case "ArrowRight": return mv(0, 1);
      case "Tab": e.preventDefault(); return go(cur()[0], cur()[1] + (e.shiftKey ? -1 : 1));
      case "Enter": case "F2": e.preventDefault(); return startEdit(input(k(), ...cur()));
      case "Delete": e.preventDefault(); return clear();
      // Backspace on one cell clears it and starts typing there; on a range it clears the range.
      case "Backspace": e.preventDefault(); { const g = S.last(); return g.r0 === g.r1 && g.c0 === g.c1 && S.ranges().length === 1 ? startEdit("", cur()[0], cur()[1], true) : clear(); }
    }
    if (ctrl && e.altKey && e.key.toLowerCase() === "v") { e.preventDefault(); return import("./dialogs").then(d => d.pasteSpecial()); }
    if (ctrl && e.key.toLowerCase() === "d" && range()[0] === range()[2] && range()[0] > 0) { e.preventDefault(); const [r0, c0, , c1] = range(); setCur([r0 - 1, c0]); setEnd([r0 - 1, c1]); fillRange(r0, c1, true); return; }
    if (ctrl && e.key.toLowerCase() === "d") { e.preventDefault(); const [a, , c2] = range(); if (c2 > a) { const [r0, c0, , c1] = range(); setCur([r0, c0]); setEnd([r0, c1]); fillRange(c2, c1, true); } return; }
    if (ctrl && e.key.toLowerCase() === "r") { e.preventDefault(); const [, b, , d] = range(); if (d > b) { const [r0, c0, r1] = range(); setCur([r0, c0]); setEnd([r1, c0]); fillRange(r1, d, true); } return; }
    if (ctrl && ["b", "i", "u"].includes(e.key.toLowerCase())) { e.preventDefault(); return F.toggle(e.key.toLowerCase() as "b"); }
    if (ctrl && e.key.toLowerCase() === "a") { e.preventDefault(); S.all(); return; }
    if (ctrl && (e.key.toLowerCase() === "f" || e.key.toLowerCase() === "h")) { e.preventDefault(); return findDialog(e.key.toLowerCase() === "h"); }
    if (!ctrl && !e.altKey && e.key.length === 1) { e.preventDefault(); startEdit(e.key, cur()[0], cur()[1], true); }
  };
  const copy = (cut = false) => {
    const [r0, c0, r1, c1] = range();
    const lines: string[][] = [], inputs: string[][] = [];
    for (let r = r0; r <= r1; r++) { const l = [], m = []; for (let c = c0; c <= c1; c++) { const shown = cells().get(a1(r, c))?.[2] ?? ""; l.push(shown); m.push(input(k(), r, c) || shown); } lines.push(l); inputs.push(m); }
    const st = sheet()?.style ?? {};
    clip.text = toTsv(lines); clip.styles = Array.from({ length: r1 - r0 + 1 }, (_, i) => Array.from({ length: c1 - c0 + 1 }, (_, j) => st[a1(r0 + i, c0 + j)] ?? ""));
    const fm = sheet()?.fmt ?? {};
    const mg = merges().filter(m => m.r0 >= r0 && m.r1 <= r1 && m.c0 >= c0 && m.c1 <= c1).map(m => ({ r0: m.r0 - r0, c0: m.c0 - c0, r1: m.r1 - r0, c1: m.c1 - c0 }));
    Object.assign(sclip, { merges: mg, text: clip.text, inputs, styles: clip.styles, fmts: Array.from({ length: r1 - r0 + 1 }, (_, i) => Array.from({ length: c1 - c0 + 1 }, (_, j) => fm[a1(r0 + i, c0 + j)] ?? "")), k: k(), r0, c0, cut });
    return clip.text;
  };
  const onCopy = (e: ClipboardEvent, cut = false) => { if (edit() !== null) return; e.preventDefault(); e.clipboardData?.setData("text/plain", copy(cut)); };
  /** What a paste puts at the cursor: copies made in Sheets bring their formulas (moved by the distance); anything else pastes as text. */
  const pasted = async (text: string, tr = false): Promise<[string, string][]> => {
    const [r0, c0] = cur();
    if (text && text === sclip.text) {
      let m = sclip.inputs;
      if (tr) m = transpose(m);
      const dr = r0 - sclip.r0, dc = c0 - sclip.c0, list = m.flatMap((l, i) => l.map((v, j) => [a1(r0 + i, c0 + j), v] as [string, string]));
      if (sclip.cut) { sclip.text = ""; return list; }
      const moved = await call<string[]>("formula_shift", { items: list.map(([, v]) => [v, dr, dc]) });
      return list.map(([at], i) => [at, moved?.[i] ?? list[i][1]]);
    }
    let m = parseTsv(text);
    if (tr) m = transpose(m);
    return m.flatMap((l, i) => l.map((v, j) => [a1(r0 + i, c0 + j), v] as [string, string]));
  };
  /** A paste in one undo step: a copy made in Sheets brings formulas (moved), styles and number formats; a cut
   *  clears where it came from; one copied cell pasted on a larger selection fills all of it. */
  const pasteAll = async (text: string, tr = false) => {
    const own = !!text && text === sclip.text, cutting = own && sclip.cut, src = { ...sclip }, b = bounds();
    let list = await pasted(text, tr);
    if (!list.length) return;
    const [r0, c0] = cur(), ops: unknown[] = [];
    const one = src.inputs.length === 1 && src.inputs[0].length === 1 && own && !cutting;
    if (one && (b.r1 > b.r0 || b.c1 > b.c0)) {
      const items: [string, number, number][] = [], at: string[] = [];
      for (let r = b.r0; r <= b.r1; r++) for (let c = b.c0; c <= b.c1; c++) { items.push([src.inputs[0][0], r - src.r0, c - src.c0]); at.push(a1(r, c)); }
      const moved = await call<string[]>("formula_shift", { items });
      list = at.map((a, i) => [a, moved?.[i] ?? src.inputs[0][0]]);
    }
    if (cutting) { const ids = src.inputs.flatMap((l, i) => l.map((_, j) => a1(src.r0 + i, src.c0 + j)));
      ops.push({ SheetCells: { k: src.k, cells: ids.map(a => [a, ""]), fmt: null } }, { SheetStyle: { k: src.k, cells: ids.map(a => [a, ""]) } }, { SheetCells: { k: src.k, cells: ids.map(a => [a, ""]), fmt: "" } }); }
    ops.push({ SheetCells: { k: k(), cells: list, fmt: null } });
    if (own) {
      const rep = (m: string[][]) => one ? list.map(([a]) => [a, m[0]?.[0] ?? ""] as [string, string]) : (tr ? transpose(m) : m).flatMap((l, i) => l.map((v, j) => [a1(r0 + i, c0 + j), v] as [string, string]));
      ops.push({ SheetStyle: { k: k(), cells: rep(src.styles) } });
      const by = new Map<string, [string, string][]>(); rep(src.fmts).forEach(([a, f]) => by.set(f, [...(by.get(f) ?? []), [a, ""]]));
      by.forEach((cells2, f) => ops.push({ SheetCells: { k: k(), cells: cells2, fmt: f } }));
      // Merged cells inside the copy are merged where they land (not when transposed).
      if (!tr && !one && src.merges.length) {
        const add = src.merges.map(m => ({ r0: r0 + m.r0, c0: c0 + m.c0, r1: r0 + m.r1, c1: c0 + m.c1 }));
        const inSrc = (m: { r0: number; c0: number; r1: number; c1: number }) => cutting && src.k === k() && m.r0 >= src.r0 && m.c0 >= src.c0 && m.r1 < src.r0 + src.inputs.length && m.c1 < src.c0 + (src.inputs[0]?.length ?? 0);
        const keep = merges().filter(m => !inSrc(m) && !add.some(x => m.r0 <= x.r1 && x.r0 <= m.r1 && m.c0 <= x.c1 && x.c0 <= m.c1)).map(m => `${a1(m.r0, m.c0)}:${a1(m.r1, m.c1)}`);
        ops.unshift({ Sheet: { k: k(), sheet: { ...sheet(), merges: [...keep, ...add.map(m => `${a1(m.r0, m.c0)}:${a1(m.r1, m.c1)}`)] } } });
      }
    }
    await act({ Batch: ops });
  };
  const clipText = async () => ((await navigator.clipboard?.readText().catch(() => "")) || sclip.text).replace(/\r/g, "");
  /** Paste Special: values, formats or both, optionally transposed. */
  const pasteSpecial = async (what: "all" | "values" | "formats", tr: boolean) => {
    const text = (await clipText()) || clip.text;
    if (!text) return toast("Copy some cells first.");
    let st = clip.text === text ? clip.styles : [];
    if (tr && st.length) st = transpose(st);
    const [r0, c0] = cur();
    if (what === "all") return pasteAll(text, tr);
    if (what === "values") { let m = parseTsv(text); if (tr) m = transpose(m); await put(m.flatMap((l, i) => l.map((v, j) => [a1(r0 + i, c0 + j), v] as [string, string]))); }
    if (what !== "values" && st.length) await act({ SheetStyle: { k: k(), cells: st.flatMap((l, i) => l.map((v, j) => [a1(r0 + i, c0 + j), v] as [string, string])) } });
  };
  const onPaste = async (e: ClipboardEvent) => { if (edit() !== null) return; e.preventDefault(); pasteAll((e.clipboardData?.getData("text/plain") ?? "").replace(/\r/g, "")); };
  /** Fills from the selection's first rows down (or first columns right): numbers and dates continue
   *  their series, formulas move their references; Ctrl copies. */
  const fillRange = async (r1: number, c1: number, copyOnly = false) => {
    const [a, b, c, d] = range(), out: [string, string][] = [], moves: [string, number, number][] = [], at: string[] = [], sty: [string, string][] = [], fms = new Map<string, [string, string][]>();
    const one = (src: string[], k2: number, srcAt: (i: number) => [number, number], to: [number, number]) => {
      const i = (k2 - 1) % src.length, s = src[i], [sr0, sc0] = srcAt(i), f0 = fmtOf(sr0, sc0);
      sty.push([a1(...to), sheet()?.style?.[a1(sr0, sc0)] ?? ""]); fms.set(f0, [...(fms.get(f0) ?? []), [a1(...to), ""]]);
      if (src.some(x => x.startsWith("="))) { if (s.startsWith("=")) { const [sr, sc] = srcAt(i); moves.push([s, to[0] - sr, to[1] - sc]); at.push(a1(...to)); } else out.push([a1(...to), s]); }
      else out.push([a1(...to), series(src, k2, copyOnly)]);
    };
    if (r1 > c) for (let cc = b; cc <= d; cc++) { const src = Array.from({ length: c - a + 1 }, (_, i) => input(k(), a + i, cc)); for (let r = c + 1; r <= r1; r++) one(src, r - c, i => [a + i, cc], [r, cc]); }
    else if (c1 > d) for (let r = a; r <= c; r++) { const src = Array.from({ length: d - b + 1 }, (_, i) => input(k(), r, b + i)); for (let cc = d + 1; cc <= c1; cc++) one(src, cc - d, i => [r, b + i], [r, cc]); }
    if (moves.length) (await call<string[]>("formula_shift", { items: moves }))?.forEach((f, i) => out.push([at[i], f]));
    setEnd([Math.max(c, r1), Math.max(d, c1)]);
    await act({ Batch: [{ SheetCells: { k: k(), cells: out, fmt: null } }, { SheetStyle: { k: k(), cells: sty } }, ...[...fms].map(([f, cl]) => ({ SheetCells: { k: k(), cells: cl, fmt: f } }))] });
  };
  const [fillTo, setFillTo] = createSignal<[number, number] | null>(null);
  const [back, setBack] = createSignal<[number, number] | null>(null);
  /** Fills up (or left) from the selection: the series runs backwards, formulas move, looks go along. */
  const fillBack = async (dir: "up" | "left", to: number, copyOnly: boolean) => {
    const [a, b, c, d] = range(), out: [string, string][] = [], moves: [string, number, number][] = [], at2: string[] = [], sty: [string, string][] = [], fms = new Map<string, [string, string][]>();
    const lines = dir === "up" ? Array.from({ length: d - b + 1 }, (_, j) => b + j) : Array.from({ length: c - a + 1 }, (_, i) => a + i);
    for (const ln of lines) {
      const src = dir === "up" ? Array.from({ length: c - a + 1 }, (_, i) => c - i) : Array.from({ length: d - b + 1 }, (_, j) => d - j);
      const vals = src.map(x => (dir === "up" ? input(k(), x, ln) : input(k(), ln, x)));
      const first = dir === "up" ? a : b;
      for (let pos = first - 1, n = 1; pos >= to; pos--, n++) {
        const i = (n - 1) % src.length, [sr, sc] = dir === "up" ? [src[i], ln] : [ln, src[i]], [tr, tc] = dir === "up" ? [pos, ln] : [ln, pos];
        const v = vals[i];
        if (vals.some(x => x.startsWith("="))) { if (v.startsWith("=")) { moves.push([v, tr - sr, tc - sc]); at2.push(a1(tr, tc)); } else out.push([a1(tr, tc), v]); }
        else out.push([a1(tr, tc), series(vals, n, copyOnly)]);
        sty.push([a1(tr, tc), sheet()?.style?.[a1(sr, sc)] ?? ""]); const f0 = fmtOf(sr, sc); fms.set(f0, [...(fms.get(f0) ?? []), [a1(tr, tc), ""]]);
      }
    }
    if (moves.length) (await call<string[]>("formula_shift", { items: moves }))?.forEach((f, i) => out.push([at2[i], f]));
    if (dir === "up") { setCur([to, b]); setEnd([c, d]); } else { setCur([a, to]); setEnd([c, d]); }
    await act({ Batch: [{ SheetCells: { k: k(), cells: out, fmt: null } }, { SheetStyle: { k: k(), cells: sty } }, ...[...fms].map(([f, cl]) => ({ SheetCells: { k: k(), cells: cl, fmt: f } }))] });
  };
  const fillDrag = (e: MouseEvent) => {
    e.preventDefault(); e.stopPropagation();
    const [, , r1, c1] = range();
    const [r0, c0] = range();
    // Down or right past the selection, or up or left before it (the series runs backwards).
    let back: "up" | "left" | null = null, reach = 0;
    const mv = (m: MouseEvent) => {
      const [r, c0m] = at(m.clientX, m.clientY), c = c0m < 0 ? c1 : c0m;
      const down = r - r1, right = c - c1, upw = r0 - r, left = c0 - c;
      const best = Math.max(down, right, upw, left);
      back = null;
      if (best <= 0) { setFillTo(null); return; }
      if (best === upw) { back = "up"; reach = Math.max(0, r); setFillTo([r1, c1]); setBack([reach, c0]); }
      else if (best === left) { back = "left"; reach = Math.max(0, c); setFillTo([r1, c1]); setBack([r0, reach]); }
      else { setBack(null); setFillTo(best === down ? [Math.max(r1, r), c1] : [r1, Math.max(c1, c)]); }
    };
    const up = (u: MouseEvent) => { removeEventListener("mousemove", mv); removeEventListener("mouseup", up); const t = fillTo(); setFillTo(null); setBack(null); if (back) fillBack(back, reach, u.ctrlKey); else if (t && (t[0] > r1 || t[1] > c1)) fillRange(t[0], t[1], u.ctrlKey); };
    addEventListener("mousemove", mv); addEventListener("mouseup", up);
  };

  // ---------- spreadsheet commands ----------
  /** AutoSum (Alt+=): one cell gets =SUM of the numbers above it (or to its left); a range gets totals under each column. */
  const autoSum = (fn: string) => {
    const g = S.last(), num = (r: number, c: number) => !!cells().get(a1(r, c))?.[3];
    if (g.r0 === g.r1 && g.c0 === g.c1) {
      const [r, c] = [g.r0, g.c0];
      let t = r - 1, l = c - 1;
      while (t >= 0 && num(t, c)) t--;
      while (l >= 0 && num(r, l)) l--;
      const ref = t < r - 1 ? `${a1(t + 1, c)}:${a1(r - 1, c)}` : l < c - 1 ? `${a1(r, l + 1)}:${a1(r, c - 1)}` : "";
      // The proposed range stays "live": clicking or dragging other cells replaces it.
      startEdit(`=${fn}(${ref})`);
      setCaret(fn.length + 2 + ref.length);
      setPoint(ref ? { start: fn.length + 2, end: fn.length + 2 + ref.length } : null);
      return;
    }
    const out: [string, string][] = [];
    for (let c = g.c0; c <= g.c1; c++) out.push([a1(g.r1 + 1, c), `=${fn}(${a1(g.r0, c)}:${a1(g.r1, c)})`]);
    put(out);
  };
  const insertFn = () => import("./sheetdlg").then(d => d.insertFunction(cat(), name => {
    if (edit() === null) { startEdit(`=${name}(`); return; }
    const t = edit()!, at = caret(), ins = `${name}(`;
    setText(t.slice(0, at) + ins + t.slice(at), at + ins.length);
  }));
  /** Writes a Plan Data list at the cursor: bold headings, one live formula, formats down the columns. */
  const putList = (x: { formula: string; heads: string[]; fmt: string[] }) => {
    const [r, c] = cur();
    const n = Math.max(grid().rows.length, P().resources.length) + 20;
    const cellsOut: [string, string][] = x.heads.map((hd, j) => [a1(r, c + j), hd]);
    cellsOut.push([a1(r + 1, c), x.formula]);
    const ops: unknown[] = [{ SheetCells: { k: k(), cells: cellsOut, fmt: null } }, { SheetStyle: { k: k(), cells: x.heads.map((_, j) => [a1(r, c + j), "b"]) } }];
    x.fmt.forEach((f, j) => { if (f) ops.push({ SheetCells: { k: k(), cells: Array.from({ length: n }, (_, i) => [a1(r + 1 + i, c + j), ""]), fmt: f } }); });
    // Names need room; other columns keep their width. (First: it replaces the tab as it is now.)
    const nameCol = x.heads.indexOf("Name");
    if (nameCol >= 0 && width(c + nameCol) < 220) ops.unshift({ Sheet: { k: k(), sheet: { ...sheet(), widths: { ...(sheet().widths ?? {}), [c + nameCol]: 240 } } } });
    act({ Batch: ops });
  };
  const planData = () => import("./sheetdlg").then(d => d.planDataDialog(cat(), putList));
  /** One click from the plan: a ready-made live list at the selected cell. */
  const quickList = (table: "Tasks" | "Resources", rows: string, cols: string[], sortBy = "") => import("./sheetdlg").then(d => {
    const all = (table === "Tasks" ? cat()?.tasks : cat()?.resources) ?? [];
    putList(d.planData(table, all, cols.filter(x => all.includes(x)), rows, sortBy, false, 0));
  });
  /** The project's key figures, each a live name (ProjectFinish, SPI…), with labels. */
  const projectFacts = () => {
    const [r, c] = cur();
    const facts: [string, string][] = [["Project", "=ProjectName"], ["Start", "=ProjectStart"], ["Finish", "=ProjectFinish"], ["Status date", "=StatusDate"], ["Baseline finish", "=BaselineFinish"], ["Finish variance (days)", "=FinishVariance"],
      ["% complete", "=PercentComplete"], ["Planned % complete", "=PlannedPercent"], ["SPI", "=SPI"], ["CPI", "=CPI"], ["Tasks", "=TaskCount"], ["Tasks done", "=TasksDone"], ["Late tasks", "=TasksLate"], ["Critical tasks", "=CriticalCount"], ["Milestones", "=MilestoneCount"]];
    const ops: unknown[] = [{ SheetCells: { k: k(), cells: facts.flatMap(([l, f], i) => [[a1(r + i, c), l], [a1(r + i, c + 1), f]]), fmt: null } }, { SheetStyle: { k: k(), cells: facts.map((_, i) => [a1(r + i, c), "b"]) } }];
    if (width(c) < 170) ops.unshift({ Sheet: { k: k(), sheet: { ...sheet(), widths: { ...(sheet().widths ?? {}), [c]: 180 } } } });
    act({ Batch: ops });
  };
  /** One value of one task (its finish, % complete…), live: =XLOOKUP(its Unique ID, Tasks[Unique ID], Tasks[field]). */
  const taskValue = () => openDialog(() => {
    const tasks = grid().rows.filter(x => x.uid > 0 && (x.kind === "task" || x.kind === "summary") && !x.sub), fields = () => (cat()?.tasks ?? []).filter(f => f !== "Unique ID");
    const [uid, setUid] = createSignal(tasks[0]?.uid ?? 0), [field, setField] = createSignal("Finish"), [label, setLabel] = createSignal(true), [q, setQ] = createSignal("");
    const shown = () => tasks.filter(x => !q() || x.name.toLowerCase().includes(q().toLowerCase())).slice(0, 400);
    return <Dialog title="Task Value" okText="Insert" ok={async () => {
      const t = tasks.find(x => x.uid === uid()); if (!t) return false;
      const [r, c] = cur(), d = await import("./sheetdlg"), fm = d.planData("Tasks", cat()?.tasks ?? [], [field()], "all", "", false, 0).fmt[0];
      const at = label() ? c + 1 : c, f = `=XLOOKUP(${t.uid},Tasks[Unique ID],Tasks[${field()}])`;
      const ops: unknown[] = [{ SheetCells: { k: k(), cells: [...(label() ? [[a1(r, c), `${t.name} – ${field()}`] as [string, string]] : []), [a1(r, at), f]], fmt: null } }];
      if (fm) ops.push({ SheetCells: { k: k(), cells: [[a1(r, at), ""]], fmt: fm } });
      act({ Batch: ops });
    }}>
      <p class="muted">Puts one value of a task in the selected cell. It stays live: when the plan changes, so does the cell.</p>
      <div class="form">
        <Row label="Find"><input placeholder="Type part of the task's name" value={q()} onInput={e => setQ(e.currentTarget.value)} /></Row>
        <Row label="Task"><select size="8" class="tasklist" onChange={e => setUid(+e.currentTarget.value)}><For each={shown()}>{t => <option value={t.uid} selected={t.uid === uid()}>{"\u00a0".repeat(t.lvl * 3)}{t.id}. {t.name}</option>}</For></select></Row>
        <Row label="Value"><select onChange={e => setField(e.currentTarget.value)}><For each={fields()}>{f => <option selected={f === field()}>{f}</option>}</For></select></Row>
        <Row label=""><label class="chk"><input type="checkbox" checked={label()} onChange={e => setLabel(e.currentTarget.checked)} />Put the task's name in the cell to the left</label></Row>
      </div>
    </Dialog>;
  });
  /** Inserts (or deletes) as many whole rows as the selection is tall, or columns when whole columns are selected. */
  const insertCells = (del: boolean, cols?: boolean) => {
    const g = S.last(), wholeCols = g.r0 === 0 && g.r1 >= nrows() - 1;
    const c = cols ?? wholeCols;
    const n = Math.min(1000, c ? g.c1 - g.c0 + 1 : g.r1 - g.r0 + 1);
    act({ SheetInsert: { k: k(), cols: c, at: c ? g.c0 : g.r0, count: del ? -n : n } });
  };
  const setFmt = (f: string) => { const out: [string, string][] = []; S.each((r, c) => out.push([a1(r, c), ""]), 20000); act({ SheetCells: { k: k(), cells: out, fmt: f } }); };
  const customFmt = () => { const f = prompt("Number format code, for example #,##0.0 or dd/mm/yyyy or 0.0\"%\"", fmtOf(...cur())); if (f !== null) setFmt(f.trim()); };
  const moreDecimals = (d: number) => {
    const by = new Map<string, [string, string][]>();
    S.each((r, c) => { const f = decimals(fmtOf(r, c), d); by.set(f, [...(by.get(f) ?? []), [a1(r, c), ""]]); }, 20000);
    act({ Batch: [...by].map(([f, list]) => ({ SheetCells: { k: k(), cells: list, fmt: f } })) });
  };
  // ---------- Home tab commands ----------
  const bounds = () => { const g = S.last(); return { r0: Math.min(g.r0, g.r1), c0: Math.min(g.c0, g.c1), r1: Math.max(g.r0, g.r1), c1: Math.max(g.c0, g.c1) }; };
  const [pen, setPen] = createSignal<[string, string]>(["thin", "#000000"]);
  let lastClick: MouseEvent | undefined;
  const trackClick = (e: MouseEvent) => { lastClick = e; };
  addEventListener("mousedown", trackClick, true); onCleanup(() => removeEventListener("mousedown", trackClick, true));
  /** Borders on the selection, as a spreadsheet draws them (inner lines once, not twice). */
  const border = (kind: string) => {
    const b = bounds(), [kd, col] = pen(), ln = (k = kd) => `${k} ${col}`;
    F.changeEach((st, id) => {
      const [r, c] = parseA1(id), o = { ...st }, top = r === b.r0, bot = r === b.r1, lft = c === b.c0, rgt = c === b.c1;
      const set = (k: F.Side, on: boolean, v = ln()) => { if (on) o[k] = v; };
      switch (kind) {
        case "none": F.SIDES.forEach(k => delete o[k]); break;
        case "all": set("bb", true); set("br", true); set("bt", top); set("bl", lft); break;
        case "outside": set("bt", top); set("bb", bot); set("bl", lft); set("br", rgt); break;
        case "thickbox": set("bt", top, ln("thick")); set("bb", bot, ln("thick")); set("bl", lft, ln("thick")); set("br", rgt, ln("thick")); break;
        case "top": set("bt", top); break; case "bottom": set("bb", bot); break; case "left": set("bl", lft); break; case "right": set("br", rgt); break;
        case "topbottom": set("bt", top); set("bb", bot); break;
        case "dblbottom": set("bb", bot, ln("dbl")); break; case "thickbottom": set("bb", bot, ln("thick")); break;
        case "topthickbottom": set("bt", top); set("bb", bot, ln("thick")); break;
      }
      return o;
    });
  };
  const BI = (label: string, kind: string): Item => ({ label, icon: `bd-${kind}`, run: () => border(kind) });
  const borderItems = (): Item[] => [
    BI("Bottom Border", "bottom"), BI("Top Border", "top"), BI("Left Border", "left"), BI("Right Border", "right"), "-",
    BI("No Border", "none"), BI("All Borders", "all"), BI("Outside Borders", "outside"), BI("Thick Outside Borders", "thickbox"), "-",
    BI("Bottom Double Border", "dblbottom"), BI("Thick Bottom Border", "thickbottom"), BI("Top and Bottom Border", "topbottom"), BI("Top and Thick Bottom Border", "topthickbottom"), "-",
    { label: "Line Color…", icon: "palette", swatch: pen()[1], run: () => colorPalette(lastClick ?? new MouseEvent("click", { clientX: 400, clientY: 160 }), pen()[1], c => setPen([pen()[0], c || "#000000"]), "Automatic") },
    { label: "Line Style", sub: ([["Thin", "thin"], ["Medium", "med"], ["Thick", "thick"], ["Double", "dbl"], ["Dashed", "dash"], ["Dotted", "dot"]] as [string, string][]).map(([l, k2]) => ({ label: l, icon: `ls-${k2}`, on: pen()[0] === k2, run: () => setPen([k2, pen()[1]]) })) },
  ];
  const saveMerges = (list: string[], extra: unknown[] = []) => act({ Batch: [{ Sheet: { k: k(), sheet: { ...sheet(), merges: list } } }, ...extra] });
  const overlaps = (b: { r0: number; c0: number; r1: number; c1: number }) => (m: { r0: number; c0: number; r1: number; c1: number }) => m.r0 <= b.r1 && b.r0 <= m.r1 && m.c0 <= b.c1 && b.c0 <= m.c1;
  const asA1 = (m: { r0: number; c0: number; r1: number; c1: number }) => `${a1(m.r0, m.c0)}:${a1(m.r1, m.c1)}`;
  /** Merge & Center, Merge Across, Merge Cells, Unmerge Cells. */
  const merge = (kind: "center" | "across" | "cells" | "unmerge") => {
    const b = bounds(), keep = merges().filter(m => !overlaps(b)(m)).map(asA1);
    if (kind === "unmerge") return saveMerges(keep);
    if (b.r0 === b.r1 && b.c0 === b.c1) return toast("Select two or more cells to merge.");
    const add = kind === "across" ? Array.from({ length: b.r1 - b.r0 + 1 }, (_, i) => ({ ...b, r0: b.r0 + i, r1: b.r0 + i })).filter(m => m.c1 > m.c0) : [b];
    const centre = kind === "center" ? [{ SheetStyle: { k: k(), cells: [[a1(b.r0, b.c0), F.ser({ ...F.parse(sheet()?.style?.[a1(b.r0, b.c0)]), a: "c" })]] } }] : [];
    const lost = [] as string[]; S.each((r, c) => { if (!(r === b.r0 && c === b.c0) && input(k(), r, c)) lost.push(a1(r, c)); }, 20000);
    if (lost.length && kind !== "across") toast("Merging keeps only the upper-left value; the others are hidden under it.");
    return saveMerges([...keep, ...add.map(asA1)], centre);
  };
  const merged = () => merges().some(overlaps(bounds()));
  /** A cell style sets font and fill; alignment, wrapping and (unless the style has its own) borders stay, as in a spreadsheet. */
  const cellStyle = (st: string) => { const n = F.parse(st), own = F.SIDES.some(k2 => n[k2]); F.changeEach(o => ({ a: o.a, v: o.v, w: o.w, x: o.x, ...(own ? {} : { bt: o.bt, bb: o.bb, bl: o.bl, br: o.br }), ...n })); };
  const clearKind = async (kind: "all" | "formats" | "contents") => {
    const ids: string[] = []; S.each((r, c) => ids.push(a1(r, c)), 20000);
    const ops: unknown[] = [];
    if (kind !== "formats") ops.push({ SheetCells: { k: k(), cells: ids.filter(a => sheet().cells[a]).map(a => [a, ""]), fmt: null } });
    if (kind !== "contents") { ops.push({ SheetStyle: { k: k(), cells: ids.filter(a => sheet().style?.[a]).map(a => [a, ""]) } }); ops.push({ SheetCells: { k: k(), cells: ids.filter(a => sheet().fmt?.[a]).map(a => [a, ""]), fmt: "" } }); }
    await act({ Batch: ops });
  };
  const fillDown = () => { const b = bounds(); if (b.r1 > b.r0) { setCur([b.r0, b.c0]); setEnd([b.r0, b.c1]); fillRange(b.r1, b.c1, true); } };
  const fillRight = () => { const b = bounds(); if (b.c1 > b.c0) { setCur([b.r0, b.c0]); setEnd([b.r1, b.c0]); fillRange(b.r1, b.c1, true); } };
  /** Sorts the selected rows by the selection's first column (values and formats move with their rows). */
  const sortRows = async (asc: boolean) => {
    const b = bounds(); if (b.r1 <= b.r0) return toast("Select the rows to sort (two or more).");
    const key = (r: number) => { const x = cells().get(a1(r, b.c0)); return x?.[4] !== null && x?.[4] !== undefined ? x[4] : (x?.[2] ?? "").toLowerCase(); };
    const blank = (r: number) => !(cells().get(a1(r, b.c0))?.[2] ?? "");
    const rows = Array.from({ length: b.r1 - b.r0 + 1 }, (_, i) => b.r0 + i).sort((x, y) => {
      if (blank(x) !== blank(y)) return blank(x) ? 1 : -1;
      const p = key(x), q = key(y); const d = typeof p === "number" && typeof q === "number" ? p - q : String(p).localeCompare(String(q), undefined, { numeric: true }); return asc ? d : -d; });
    const items: [string, number, number][] = [], at: string[] = [], sts: [string, string][] = [], fms = new Map<string, [string, string][]>();
    rows.forEach((src, i) => { for (let c = b.c0; c <= b.c1; c++) { const to = a1(b.r0 + i, c); items.push([input(k(), src, c), b.r0 + i - src, 0]); at.push(to); sts.push([to, sheet().style?.[a1(src, c)] ?? ""]); const f = fmtOf(src, c); fms.set(f, [...(fms.get(f) ?? []), [to, ""]]); } });
    const moved = await call<string[]>("formula_shift", { items });
    act({ Batch: [{ SheetCells: { k: k(), cells: at.map((a, i) => [a, moved?.[i] ?? items[i][0]]), fmt: null } }, { SheetStyle: { k: k(), cells: sts } }, ...[...fms].map(([f, cl]) => ({ SheetCells: { k: k(), cells: cl, fmt: f } }))] });
  };
  const rowHeight = async () => { const b = bounds(), v = await askText("Row Height", "Row height (pixels)", String(rh(b.r0))); const n = Math.round(+(v ?? "")); if (!v || !(n >= 6 && n <= 600)) return; const hs = { ...(sheet().heights ?? {}) }; for (let r = b.r0; r <= Math.min(b.r1, b.r0 + 5000); r++) { if (n === RH) delete hs[r]; else hs[r] = n; } act({ Sheet: { k: k(), sheet: { ...sheet(), heights: hs } } }); };
  const colWidth = async () => { const b = bounds(), v = await askText("Column Width", "Column width (pixels)", String(width(b.c0))); const n = Math.round(+(v ?? "")); if (!v || !(n >= 16 && n <= 1200)) return; const ws = { ...(sheet().widths ?? {}) }; for (let c = b.c0; c <= Math.min(b.c1, b.c0 + 500); c++) ws[c] = n; saveWidths(ws); };
  /** AutoFit Row Height: rows with wrapped text grow to fit it; others go back to the standard height. */
  const autofitRows = () => {
    const b = bounds(), hs = { ...(sheet().heights ?? {}) };
    for (let r = b.r0; r <= Math.min(b.r1, b.r0 + 2000); r++) {
      let need = RH;
      for (let c = 0; c < ncols(); c++) { const st = F.parse(sheet()?.style?.[a1(r, c)]), t = cells().get(a1(r, c))?.[2] ?? "", z = st.z || 12; if (!t) continue; const lines = st.w ? Math.ceil((t.length * z * 0.55) / Math.max(20, width(c) - 10)) : 1; need = Math.max(need, Math.ceil(lines * z * 1.25 + 6)); }
      if (need === RH) delete hs[r]; else hs[r] = need;
    }
    act({ Sheet: { k: k(), sheet: { ...sheet(), heights: hs } } });
  };
  const autofitCols = () => { const b = bounds(); for (let c = b.c0; c <= b.c1; c++) autofit(c); };
  const fmtItems = (): Item[] => [...FORMATS.map(([l, f]) => ({ label: l, on: fmtOf(...cur()) === f, run: () => setFmt(f) })), "-", { label: "Custom…", run: customFmt }];
  const sumItems = (): Item[] => ["SUM", "AVERAGE", "COUNT", "MAX", "MIN"].map(f => ({ label: f[0] + f.slice(1).toLowerCase(), keys: f === "SUM" ? "Alt+=" : undefined, run: () => autoSum(f) }));
  const cellMenu = (e: MouseEvent) => {
    const [r, c] = at(e.clientX, e.clientY);
    if (r >= 0 && c >= 0 && !inR(r, c)) go(r, c);
    const g = S.last(), nr = g.r1 - g.r0 + 1, nc = g.c1 - g.c0 + 1;
    const pg = page(), [cr, cc] = cur();
    openMenu(e, [
      ...(sheetMode() === "breaks" ? [
        { label: "Insert Page Break", run: () => breakHere() },
        { label: "Remove Page Break", off: !(pg.breaks ?? []).includes(cr) && !(pg.col_breaks ?? []).includes(cc), run: unbreakHere },
        { label: "Reset All Page Breaks", off: !(pg.breaks ?? []).length && !(pg.col_breaks ?? []).length, run: resetBreaks },
        { label: "Set Print Area", run: areaFromSel },
        { label: "Reset Print Area", off: !pg.area, run: () => setPage({ area: "" }) }, "-" as const] : []),
      { label: "Cut", keys: "Ctrl+X", run: () => { navigator.clipboard?.writeText(copy(true)).catch(() => {}); } },
      { label: "Copy", keys: "Ctrl+C", run: () => { navigator.clipboard?.writeText(copy()).catch(() => {}); } },
      { label: "Paste", keys: "Ctrl+V", run: async () => pasteAll(await clipText()) },
      { label: "Paste Special…", keys: "Ctrl+Alt+V", run: () => import("./dialogs").then(d => d.pasteSpecial()) },
      "-",
      { label: nr > 1 ? `Insert ${nr} Rows Above` : "Insert Row Above", run: () => insertCells(false, false) },
      { label: nc > 1 ? `Insert ${nc} Columns Left` : "Insert Column Left", run: () => insertCells(false, true) },
      { label: nr > 1 ? `Delete ${nr} Rows` : "Delete Row", run: () => insertCells(true, false) },
      { label: nc > 1 ? `Delete ${nc} Columns` : "Delete Column", run: () => insertCells(true, true) },
      { label: "Clear Contents", keys: "Delete", run: clear },
      "-",
      { label: "Hide Rows", run: () => hideLines(false, true) }, { label: "Unhide Rows", off: !(sheet()?.hide_rows ?? []).length, run: () => hideLines(false, false) },
      { label: "Hide Columns", run: () => hideLines(true, true) }, { label: "Unhide Columns", off: !(sheet()?.hide_cols ?? []).length, run: () => hideLines(true, false) },
      "-",
      { label: "Number Format", sub: fmtItems() },
      { label: "AutoSum", sub: sumItems() },
      { label: "Insert Function…", run: insertFn },
      { label: "Plan Data…", run: planData },
    ]);
  };

  // ---------- panes, mouse, columns ----------
  // Freeze panes: rows above and columns left of the cursor stay in view (saved per sheet).
  const frozen = (): [number, number] => (P().ui?.freeze?.[sheet()?.name ?? ""] as [number, number]) ?? [0, 0];
  const freeze = (v: [number, number]) => act({ Ui: { key: "freeze", value: { ...(P().ui?.freeze ?? {}), [sheet().name]: v } } });
  /** The cell under a point; frozen rows and columns do not scroll. */
  const at = (cx: number, cy: number): [number, number] => {
    const b = box.getBoundingClientRect(), [fr, fc] = frozen(), vy = cy - b.top - RH, vx = cx - b.left;
    const y = vy < top_(fr) ? vy : vy + box.scrollTop, x = fc && vx < lefts()[fc] ? vx : vx + box.scrollLeft;
    return [rowAt(y), lefts().findIndex((l, j) => x >= l && x < l + width(j))];
  };
  // Pictures on the sheet: anchored at a cell, dragged to another cell, resized from the corner.
  const [picSel, setPicSel] = createSignal(-1);
  const imgs = () => (sheet()?.images ?? []) as SheetImage[];
  /** Where a picture is drawn: its own size at its cell, or (placed in the cell) fitted and centred in the cell or merged range. */
  const picBox = (im: SheetImage): [number, number, number, number] => {
    const [r, c] = parseA1(im.cell), x = lefts()[c] ?? 0, y = top_(r);
    if (!im.fit) return [x, y, im.w, im.h];
    const [w, h] = fitBox(sheet(), r, c, im, rh), m = mergeAt(r, c), bw = m ? lefts()[m.c1] + width(m.c1) - x : width(c), bh = m ? top_(m.r1 + 1) - y : rh(r);
    return [x + (bw - w) / 2, y + (bh - h) / 2, w, h];
  };
  const putImgs = (images: SheetImage[]) => act({ SheetImages: { k: k(), images } });
  const placePic = (id: number) => {
    const im = new Image();
    im.onload = () => { const w = Math.min(240, im.naturalWidth || 160), h = Math.max(8, Math.round(w * (im.naturalHeight || 80) / (im.naturalWidth || 160))); putImgs([...imgs(), { pic: id, cell: a1(...cur()), w, h, fit: true }]).then(() => { setPicSel(imgs().length - 1); box.focus(); }); };
    im.src = picData(id);
  };
  const picDrag = (e: MouseEvent, i: number, resize: boolean) => {
    e.preventDefault(); e.stopPropagation(); setPicSel(i); box.focus();
    const o = imgs()[i], x0 = e.clientX;
    let next = o;
    const mv = (m: MouseEvent) => {
      if (resize) { const w = Math.max(16, o.w + m.clientX - x0); next = { ...o, w: Math.round(w), h: Math.max(8, Math.round(w * o.h / o.w)) }; }
      else { const [r, c] = at(m.clientX, m.clientY); if (r >= 0 && c >= 0) next = { ...o, cell: a1(r, c) }; }
      const el = document.querySelector<HTMLElement>(`.simg[data-i="${i}"]`);
      if (el) { const [r, c] = parseA1(next.cell); Object.assign(el.style, { left: `${lefts()[c]}px`, top: `${top_(r)}px`, width: `${next.w}px`, height: `${next.h}px` }); }
    };
    const up = () => { removeEventListener("mousemove", mv); removeEventListener("mouseup", up); if (next !== o) putImgs(imgs().map((x, j) => (j === i ? next : x))); };
    addEventListener("mousemove", mv); addEventListener("mouseup", up);
  };
  const down = (e: MouseEvent) => {
    setPicSel(-1);
    if (e.button !== 0) return;
    const [r, c] = at(e.clientX, e.clientY);
    if (r < 0) return;
    if (pointing() && c >= 0) {
      // Building a formula: the click (or drag) adds a reference instead of leaving the cell.
      e.preventDefault();
      pointTo(r, c, r, c);
      const mv = (m: MouseEvent) => { const [r2, c2] = at(m.clientX, m.clientY); if (r2 >= 0 && c2 >= 0) pointTo(r, c, r2, c2); };
      const up = () => { removeEventListener("mousemove", mv); removeEventListener("mouseup", up); };
      addEventListener("mousemove", mv); addEventListener("mouseup", up);
      return;
    }
    if (edit() !== null) commit(null);
    const mg = c >= 0 && !e.shiftKey ? mergeAt(r, c) : undefined;
    if (c < 0) S.rows(r, r, e.shiftKey, e.ctrlKey); else if (mg) { go(mg.r0, mg.c0, false, e.ctrlKey); setEnd([mg.r1, mg.c1]); } else go(r, c, e.shiftKey, e.ctrlKey);
    const mv = (m: MouseEvent) => { const [r2, c2] = at(m.clientX, m.clientY); setEnd([Math.max(0, r2), c < 0 ? ncols() - 1 : c2 < 0 ? end()[1] : c2]); };
    const up = () => { removeEventListener("mousemove", mv); removeEventListener("mouseup", up); if (F.painter() !== null && paintFmt !== null) { const f = paintFmt; paintFmt = null; setFmt(f); } F.painted(); };
    addEventListener("mousemove", mv); addEventListener("mouseup", up);
  };
  const colDown = (e: MouseEvent, c: number) => {
    e.stopPropagation();
    if (pointing()) { e.preventDefault(); pointText(`${colName(c)}:${colName(c)}`); return; }
    if (edit() !== null) commit(null);
    S.cols(c, c, e.shiftKey, e.ctrlKey); box.focus();
  };
  const saveWidths = (widths: Record<number, number>) => act({ Sheet: { k: k(), sheet: { ...sheet(), widths } } });
  const resize = (e: MouseEvent, c: number) => {
    e.preventDefault(); e.stopPropagation();
    const x0 = e.clientX, w0 = width(c);
    const mv = (m: MouseEvent) => setTw([c, Math.max(24, Math.round(w0 + m.clientX - x0))]);
    const up = () => { removeEventListener("mousemove", mv); removeEventListener("mouseup", up); const t = tw(); if (t) saveWidths({ ...(sheet().widths ?? {}), [c]: t[1] }).then(() => setTw(null)); };
    addEventListener("mousemove", mv); addEventListener("mouseup", up);
  };
  /** Double-click on a column edge: fit the widest text in the column. */
  const autofit = (c: number) => {
    const w = Math.max(40, ...[...cells().values()].filter(x => parseA1(x[0])[1] === c).map(x => x[2].length * 7.2 + (fmtOf(...parseA1(x[0])) === "flag" ? 26 : 12)));
    saveWidths({ ...(sheet().widths ?? {}), [c]: Math.min(600, Math.round(w)) });
  };
  const newName = (base = "Sheet") => { let n = P().sheets.length + 1; while (P().sheets.some(s2 => s2.name.toLowerCase() === `${base}${n}`.toLowerCase())) n++; return `${base}${n}`; };
  const addSheet = () => act({ Sheet: { k: null, sheet: { name: newName(), cells: {}, fmt: {}, widths: {} } } }).then(() => setK(P().sheets.length - 1));
  // Tabs as in a spreadsheet: double-click renames in place; right-click for the rest; drag to move.
  const [renaming, setRenaming] = createSignal(-1);
  const rename = (i: number, n: string) => {
    setRenaming(-1); n = n.trim();
    if (!n || n === P().sheets[i].name) return;
    if (/[:\\/?*[\]]/.test(n) || n.length > 31) return toast("A sheet name has at most 31 characters and none of : \\ / ? * [ ].");
    const old = P().sheets[i].name, fz = { ...(P().ui?.freeze ?? {}) } as Record<string, unknown>;
    const ops: unknown[] = [{ Sheet: { k: i, sheet: { ...P().sheets[i], name: n } } }];
    if (fz[old]) { fz[n] = fz[old]; delete fz[old]; ops.push({ Ui: { key: "freeze", value: fz } }); }
    act({ Batch: ops });
  };
  const putTab = (i: number, x: Partial<SheetData>) => act({ Sheet: { k: i, sheet: { ...P().sheets[i], ...x } } });
  const delSheet = async (i: number) => { if (await askYes("Delete Sheet", `Delete the sheet ${P().sheets[i].name}? Formulas on other sheets that read it will show #REF!.`, "Delete")) act({ Sheet: { k: i, sheet: null } }).then(() => setK(Math.max(0, Math.min(i, P().sheets.length - 1)))); };
  const moveCopy = (i: number) => openDialog(() => {
    const [to, setTo] = createSignal(i), [copy, setCopy] = createSignal(false);
    return <Dialog title="Move or Copy" okText="OK" ok={async () => {
      const n = P().sheets.length, dest = Math.min(to(), n);
      if (copy()) {
        // A copy's name stays within 31 characters, so it fits a workbook.
        const named = (j: number) => { const t = ` (${j})`; return P().sheets[i].name.slice(0, 31 - t.length).trimEnd() + t; };
        let j = 2, name = named(j); while (P().sheets.some(x => x.name.toLowerCase() === name.toLowerCase())) name = named(++j);
        await act({ Sheet: { k: null, sheet: { ...JSON.parse(JSON.stringify(P().sheets[i])), name } } });
        if (dest < n) await act({ SheetMove: { from: n, to: dest } });
        setK(dest);
      } else { const t = dest > i ? dest - 1 : dest; if (t !== i) await act({ SheetMove: { from: i, to: t } }); setK(t); }
    }}>
      <p>Move <b>{P().sheets[i].name}</b> before sheet:</p>
      <div class="mclist"><For each={[...P().sheets.map((x, j) => [j, x.name] as [number, string]), [P().sheets.length, "(move to end)"] as [number, string]]}>{([j, n]) => <button classList={{ on: to() === j }} onClick={() => setTo(j)}>{n}</button>}</For></div>
      <label class="chk"><input type="checkbox" checked={copy()} onChange={e => setCopy(e.currentTarget.checked)} /> Create a copy</label>
    </Dialog>;
  });
  const tabColor = (i: number, c: string) => putTab(i, { color: c });
  const tabMenu = (e: MouseEvent, i: number) => {
    const hidden = P().sheets.map((x, j) => [j, x] as const).filter(([, x]) => x.hidden);
    openMenu(e, [
      { label: "Insert", run: addSheet },
      { label: "Delete", run: () => delSheet(i) },
      { label: "Rename", run: () => setRenaming(i) },
      { label: "Move or Copy…", run: () => moveCopy(i) },
      "-",
      { label: "Tab Color…", icon: "palette", swatch: P().sheets[i].color || undefined, run: () => colorPalette(e, P().sheets[i].color, c => tabColor(i, c), "No Color") },
      "-",
      { label: "Hide", off: P().sheets.filter(x => !x.hidden).length < 2, run: () => { putTab(i, { hidden: true }).then(() => { const v = P().sheets.findIndex(x => !x.hidden); setK(Math.max(0, v)); }); } },
      { label: "Unhide", off: !hidden.length, sub: hidden.length ? hidden.map(([j, x]) => ({ label: x.name, run: () => putTab(j, { hidden: false }).then(() => setK(j)) })) : undefined },
    ]);
  };
  const [dragTab, setDragTab] = createSignal(-1);
  /** Hide or show the selected rows or columns (Unhide also shows hidden ones inside the selection). */
  const hideLines = (cols: boolean, hide: boolean) => {
    const b = bounds(), [a, z] = cols ? [b.c0, b.c1] : [b.r0, b.r1], key = cols ? "hide_cols" : "hide_rows";
    const now = new Set((cols ? sheet()?.hide_cols : sheet()?.hide_rows) ?? []);
    for (let i = a; i <= Math.min(z, a + 20000); i++) { if (hide) now.add(i); else now.delete(i); }
    // Unhide with one line selected next to hidden ones: show the hidden run beside it.
    if (!hide && a === z) { for (const d of [-1, 1]) { let i = a + d; while (now.has(i)) { now.delete(i); i += d; } } }
    act({ Sheet: { k: k(), sheet: { ...sheet(), [key]: [...now].sort((x, y) => x - y) } } });
  };
  const unhideAll = () => act({ Sheet: { k: k(), sheet: { ...sheet(), hide_rows: [], hide_cols: [] } } });
  const condList = () => sheet()?.cond ?? [];
  const setCond = (l: CondRule[]) => act({ Sheet: { k: k(), sheet: { ...sheet(), cond: l } } });
  const selA1 = () => { const b = bounds(); return b.r0 === b.r1 && b.c0 === b.c1 ? a1(b.r0, b.c0) : `${a1(b.r0, b.c0)}:${a1(b.r1, b.c1)}`; };
  const condMenuAt = (e: MouseEvent) => openMenu(e, condMenu(selA1(), condList, setCond, () => { const b = bounds(); setCond(condList().filter(r => { const x = cfBounds(r.range); return !x || x.r1 < b.r0 || x.r0 > b.r1 || x.c1 < b.c0 || x.c0 > b.c1; })); }));
  // ---------- Find and Replace (Ctrl+F, Ctrl+H) ----------
  const findDialog = (replace: boolean) => openDialog(() => {
    const [q, setQ] = createSignal(""), [w, setW] = createSignal(""), [mc, setMc] = createSignal(false), [whole, setWhole] = createSignal(false), [note, setNote] = createSignal("");
    const hit = (t: string) => { const a = mc() ? t : t.toLowerCase(), b = mc() ? q() : q().toLowerCase(); return !!b && (whole() ? a === b : a.includes(b)); };
    const order = () => [...cells().values()].map(x => [parseA1(x[0]), x] as const).sort((p2, q2) => p2[0][0] - q2[0][0] || p2[0][1] - q2[0][1]);
    const next = () => {
      const [r0, c0] = cur(), list = order().filter(([, x]) => hit(x[2]) || (x[1] && hit(x[1])));
      if (!list.length) return setNote(`Nothing found for “${q()}”.`);
      const after = list.find(([[r, c]]) => r > r0 || (r === r0 && c > c0)) ?? list[0];
      go(after[0][0], after[0][1]); setNote(`${list.length} cell${list.length > 1 ? "s" : ""} found.`);
    };
    const swap = (t: string) => { const re = new RegExp(q().replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), mc() ? "g" : "gi"); return whole() ? w() : t.replace(re, () => w()); };
    const replaceOne = () => { const t = input(k(), ...cur()); if (t && hit(t)) put([[a1(...cur()), swap(t)]]); next(); };
    const replaceAll = () => { const list: [string, string][] = Object.entries(sheet().cells).filter(([, t]) => hit(t)).map(([a, t]) => [a, swap(t)]); put(list); setNote(`Replaced in ${list.length} cell${list.length === 1 ? "" : "s"}.`); };
    return <Dialog title={replace ? "Find and Replace" : "Find"} help="sheetediting">
      <div class="form">
        <Row label="Find"><input value={q()} onInput={e => setQ(e.currentTarget.value)} onKeyDown={e => { if (e.key === "Enter") { e.preventDefault(); e.stopPropagation(); next(); } }} /></Row>
        <Show when={replace}><Row label="Replace with"><input value={w()} onInput={e => setW(e.currentTarget.value)} /></Row></Show>
        <Row label=""><label class="chk"><input type="checkbox" checked={mc()} onChange={e => setMc(e.currentTarget.checked)} />Match case</label><label class="chk"><input type="checkbox" checked={whole()} onChange={e => setWhole(e.currentTarget.checked)} />Match the whole cell</label></Row>
      </div>
      <p class="muted">{note() || "Looks in the values shown and in what was typed (formulas too)."}</p>
      <div class="row2"><button class="btn pri" onClick={next}>Find Next</button><Show when={replace}><button class="btn" onClick={replaceOne}>Replace</button><button class="btn" onClick={replaceAll}>Replace All</button></Show></div>
    </Dialog>;
  });
  // ---------- the sheet's own page setup (Sheet Tools › Page) ----------
  const page = (): SheetPage => sheet()?.page ?? {};
  const setPage = (x: Partial<SheetPage>) => act({ Sheet: { k: k(), sheet: { ...sheet(), page: { ...page(), ...x } } } });
  const areaFromSel = () => { const b = bounds(); setPage({ area: `${a1(b.r0, b.c0)}:${a1(b.r1, b.c1)}` }); toast(`Print area: ${a1(b.r0, b.c0)}:${a1(b.r1, b.c1)}. Only these cells print.`, true); };
  const titlesFromSel = () => { const b = bounds(); setPage({ titles: `${b.r0 + 1}:${b.r1 + 1}` }); toast(`Rows ${b.r0 + 1}–${b.r1 + 1} print at the top of every page.`, true); };
  const addTo = (l: number[] | undefined, x: number) => [...new Set([...(l ?? []), x])].sort((a, b) => a - b);
  /** A new page at the cursor: above its row and left of its column ("row" or "col" for only one of them). */
  const breakHere = (only?: "row" | "col") => {
    const [r, c] = cur(), row = only !== "col" && r > 0, cl = only !== "row" && c > 0;
    if (!row && !cl) return toast(only === "col" ? "Select a cell right of column A: the new page starts at its column." : "Select a cell below the first row: the new page starts at its row.");
    setPage({ ...(row ? { breaks: addTo(page().breaks, r) } : {}), ...(cl ? { col_breaks: addTo(page().col_breaks, c) } : {}) }); setSheetMode("breaks");
  };
  const unbreakHere = () => setPage({ breaks: (page().breaks ?? []).filter(b => b !== cur()[0]), col_breaks: (page().col_breaks ?? []).filter(b => b !== cur()[1]) });
  const resetBreaks = () => setPage({ breaks: [], col_breaks: [] });
  // ---------- Page Breaks view: drag the blue print-area edges and the page breaks ----------
  const colX = (c: number) => (c < ncols() ? lefts()[c] : (lefts()[ncols() - 1] ?? HW) + width(ncols() - 1) + (c - ncols()) * CW);
  const abox = createMemo(() => { const p = sheetMode() === "breaks" ? sheetPages() : []; if (!p.length) return null; return { r0: Math.min(...p.map(x => x.r0)), r1: Math.max(...p.map(x => x.r1)), c0: Math.min(...p.map(x => x.c0)), c1: Math.max(...p.map(x => x.c1)) }; });
  const rowCuts = () => { const b = abox(); return b ? [...new Set(sheetPages().map(p => p.r0))].filter(r => r > b.r0).sort((x, y) => x - y) : []; };
  const colCuts = () => { const b = abox(); return b ? [...new Set(sheetPages().map(p => p.c0))].filter(c => c > b.c0).sort((x, y) => x - y) : []; };
  type PDrag = { kind: "top" | "bottom" | "left" | "right" | "row" | "col"; from: number; to: number };
  const [pdrag, setPdrag] = createSignal<PDrag | null>(null);
  /** The row or column edge nearest the pointer. */
  const edgeAt = (e: MouseEvent, v: boolean) => {
    const b = box.getBoundingClientRect(), [r, c] = at(e.clientX, e.clientY);
    if (v) { const x = e.clientX - b.left + box.scrollLeft; return Math.max(0, c < 0 ? 0 : x - colX(c) > width(c) / 2 ? c + 1 : c); }
    const y = e.clientY - b.top - RH + box.scrollTop; return Math.max(0, r < 0 ? 0 : y - top_(r) > rh(r) / 2 ? r + 1 : r);
  };
  const pageDrag = (e: MouseEvent, kind: PDrag["kind"], from: number) => {
    e.preventDefault(); e.stopPropagation();
    const v = kind === "left" || kind === "right" || kind === "col";
    setPdrag({ kind, from, to: from });
    const mv = (m: MouseEvent) => setPdrag({ kind, from, to: edgeAt(m, v) });
    const up = () => { removeEventListener("mousemove", mv); removeEventListener("mouseup", up); const d = pdrag(); setPdrag(null); if (d && d.to !== d.from) dropPage(d); };
    addEventListener("mousemove", mv); addEventListener("mouseup", up);
  };
  /** A dragged edge: the print area's sides set the area; a break moved inside it becomes a manual one (dragged out, it goes). A page made larger than the paper shrinks the print to fit, as the page setup's scale. */
  const dropPage = (d: PDrag) => {
    const b = abox(); if (!b) return;
    const pg = page(), x: Partial<SheetPage> = {};
    if (d.kind === "row" || d.kind === "col") {
      const row = d.kind === "row", [lo, hi] = row ? [b.r0, b.r1] : [b.c0, b.c1], key = row ? "breaks" : "col_breaks";
      const list = (pg[key] ?? []).filter(z => z !== d.from);
      x[key] = d.to > lo && d.to <= hi ? addTo(list, d.to) : list;
      if (d.to > lo && d.to <= hi) {
        const cuts = (row ? rowCuts() : colCuts()).filter(z => z !== d.from), prev = Math.max(lo, ...cuts.filter(z => z < d.to)), next = Math.min(hi + 1, ...cuts.filter(z => z > d.to));
        const room = pageRoom(), need = Math.max(row ? top_(d.to) - top_(prev) : colX(d.to) - colX(prev), row ? top_(next) - top_(d.to) : colX(next) - colX(d.to));
        const have = row ? room.h : room.w;
        if (need > have) { x.scale = Math.max(10, Math.floor((room.k * 100 * have) / need * 0.97)); x.fit_w = 0; x.fit_h = 0; }
      }
    } else {
      const r = { ...b };
      if (d.kind === "top") r.r0 = d.to; if (d.kind === "bottom") r.r1 = d.to - 1; if (d.kind === "left") r.c0 = d.to; if (d.kind === "right") r.c1 = d.to - 1;
      const [r0, r1, c0, c1] = [Math.min(r.r0, r.r1), Math.max(r.r0, r.r1), Math.min(r.c0, r.c1), Math.max(r.c0, r.c1)];
      x.area = `${a1(r0, c0)}:${a1(r1, c1)}`;
    }
    setPage(x);
  };
  const api: SheetApi = {
    resetBreaks,
    frozen: () => !!(frozen()[0] || frozen()[1]), pen, merged,
    border, borderItems, merge, cellStyle, clearKind, fillDown, fillRight, sortRows, rowHeight, colWidth, autofitRows, autofitCols,
    autoSum, sumItems, insertFn, planData, fmtItems, setFmt, moreDecimals, insertCells, addSheet, rename: () => setRenaming(k()), del: () => delSheet(k()), moveCopy: () => moveCopy(k()), tabMenu: (e: MouseEvent) => tabMenu(e, k()),
    picture: (e: MouseEvent) => pictureMenu(e, placePic), freeze: () => freeze(frozen()[0] || frozen()[1] ? [0, 0] : [cur()[0], cur()[1]]),
    fmtLabel: () => FORMATS.find(f => f[1] === fmtOf(...cur()))?.[0].split(" (")[0] ?? (fmtOf(...cur()) || "General"),
    copy: () => { navigator.clipboard?.writeText(copy()).catch(() => {}); box?.focus(); }, cut: () => { navigator.clipboard?.writeText(copy(true)).catch(() => {}); box?.focus(); },
    paste: async () => { await pasteAll(await clipText()); box?.focus(); },
    focus: () => { box?.focus(); F.setTarget(fmtTarget); },
    page, setPage, areaFromSel, titlesFromSel, breakHere, unbreakHere, condMenu: condMenuAt, hideLines, unhideAll, find: () => findDialog(false), replace: () => findDialog(true),
  };
  setSheetApi(api);
  onCleanup(() => { if (sheetApi() === api) setSheetApi(null); });
  // The tab shown is never a hidden one.
  createEffect(() => { const sh = P().sheets; if (sh[k()]?.hidden) { const v = sh.findIndex(x => !x.hidden); if (v >= 0) setK(v); } });
  const vis = createMemo(() => { const f = Math.max(0, rowAt(top()) - 2), l = Math.min(nrows() - 1, rowAt(top() + h()) + 4); return Array.from({ length: Math.max(0, l - f + 1) }, (_, j) => f + j); });
  const curCell = () => cells().get(a1(...cur()));
  /** Coloured boxes round the references of the formula being typed (those on the tab shown). */
  const boxes = createMemo(() => {
    if (edit() === null || !edit()!.startsWith("=") || !home()) return [];
    const here = sheet()?.name.toLowerCase();
    return refs(edit()!).map((r, i) => ({ ...r, color: COLORS[i % COLORS.length] })).filter(r => (r.sheet ? r.sheet.toLowerCase() === here : home()!.k === k()) && r.c1 < ncols());
  });
  /** A cell's look: its style, and how its text sits when wrapped, aligned up or down, or in a tall row. */
  const cellCss = (r: number, c: number, hgt: number, st = sheet()?.style?.[a1(r, c)]): JSX.CSSProperties => {
    const p = F.parse(st), o = { ...(F.css(st) as Record<string, string>), ...lookCss(looks().get(a1(r, c))) }, flex = p.w || p.v || hgt !== RH;
    if (!flex) return { ...o, "line-height": `${hgt - 1}px` };
    return { ...o, display: "flex", "align-items": p.v === "t" ? "flex-start" : p.v === "b" ? "flex-end" : "center", "justify-content": o["justify-content"] ?? (cells().get(a1(r, c))?.[3] ? "flex-end" : "flex-start"),
      "line-height": p.w ? "1.25" : `${RH - 3}px`, "white-space": p.w ? "normal" : "nowrap", "text-align": o["text-align"] ?? "left", "padding-top": "1px", "padding-bottom": "1px" } as JSX.CSSProperties;
  };
  /** Text runs on over the empty cells to its right (unless it wraps, is a number or the next cell has something). */
  const spill = (r: number, c: number): JSX.CSSProperties => {
    const x = cells().get(a1(r, c));
    if (!x?.[2] || x[3] || mergeAt(r, c)) return {};
    const p = F.parse(sheet()?.style?.[a1(r, c)]); if (p.w || p.a === "r" || p.a === "c") return {};
    if (cells().get(a1(r, c + 1))?.[2] || mergeAt(r, c + 1)) return {};
    return { overflow: "visible", "text-overflow": "clip", "z-index": 1, position: "relative" };
  };
  /** Merged ranges drawn over the grid: the top-left cell's value and style across the range. */
  const MergeView = (m: { r0: number; c0: number; r1: number; c1: number }) => {
    const x = () => cells().get(a1(m.r0, m.c0)), st = () => sheet()?.style ?? {};
    const sides = () => { const t = F.parse(st()[a1(m.r0, m.c0)]), b = F.parse(st()[a1(m.r1, m.c0)]), rr = F.parse(st()[a1(m.r0, m.c1)]); return { "border-bottom": F.line(b.bb) ?? "1px solid var(--line2)", "border-right": F.line(rr.br) ?? "1px solid var(--line2)", ...(t.bt ? { "border-top": F.line(t.bt) } : {}), ...(t.bl ? { "border-left": F.line(t.bl) } : {}) }; };
    return <div class="c mcell" classList={{ r: !!x()?.[3], sin: inR(m.r0, m.c0), at: cur()[0] === m.r0 && cur()[1] === m.c0 }}
      style={{ left: `${lefts()[m.c0]}px`, top: `${top_(m.r0)}px`, width: `${lefts()[m.c1] + width(m.c1) - lefts()[m.c0]}px`, height: `${top_(m.r1 + 1) - top_(m.r0)}px`, ...cellCss(m.r0, m.c0, top_(m.r1 + 1) - top_(m.r0)), ...sides() }}>
      <Show when={fmtOf(m.r0, m.c0) === "flag" && x()?.[2]} fallback={<span>{x()?.[2] ?? ""}</span>}><FlagText country={x()![2]} /></Show></div>;
  };
  const saveHeights = (r: number, hgt: number | null) => { const hs = { ...(sheet().heights ?? {}) }; if (hgt === null || hgt === RH) delete hs[r]; else hs[r] = hgt; return act({ Sheet: { k: k(), sheet: { ...sheet(), heights: hs } } }); };
  const rowResize = (e: MouseEvent, r: number) => {
    e.preventDefault(); e.stopPropagation();
    const y0 = e.clientY, h0 = rh(r);
    const mv = (m: MouseEvent) => setTh([r, Math.max(8, Math.round(h0 + m.clientY - y0))]);
    const up = () => { removeEventListener("mousemove", mv); removeEventListener("mouseup", up); const t = th(); if (t) saveHeights(t[0], t[1]).then(() => setTh(null)); };
    addEventListener("mousemove", mv); addEventListener("mouseup", up);
  };
  const EditBox = () => (
                <input class="edit" list={fmtOf(home()!.r, home()!.c) === "flag" ? "countries" : undefined} style={{ left: `${lefts()[home()!.c]}px`, top: `${top_(home()!.r)}px`, width: `${Math.max(mergeAt(home()!.r, home()!.c) ? lefts()[mergeAt(home()!.r, home()!.c)!.c1] + width(mergeAt(home()!.r, home()!.c)!.c1) - lefts()[home()!.c] : width(home()!.c), 180)}px`, height: `${Math.max(RH, rh(home()!.r))}px` }} value={edit()!}
                  ref={e => { cellEl = e; setTimeout(() => { if (document.activeElement !== barEl) { lastEl = "cell"; e.focus(); e.setSelectionRange(caret(), caret()); } }); }} onMouseDown={e => e.stopPropagation()}
                  onFocus={() => { lastEl = "cell"; }} onInput={e => typed(e.currentTarget)} onKeyDown={edKey} onKeyUp={caretOf} onClick={caretOf} onBlur={blur} />);
  const rowView = (r: number) => (
    <div class="row" classList={{ rhid: rh(r) === 0, rnext: hidR().has(r - 1) }} style={{ top: `${top_(r)}px`, height: `${rh(r)}px` }}>
      <div class="c idc rowh" classList={{ on: S.ranges().some(g => r >= Math.min(g.r0, g.r1) && r <= Math.max(g.r0, g.r1)) }} style={{ width: `${HW}px`, position: "sticky", left: "0", "z-index": 5, "text-align": "center", "line-height": `${rh(r) - 1}px` }}>{r + 1}<i class="rszr" title="Drag to change the row height; double-click for the standard height" onMouseDown={e => rowResize(e, r)} onDblClick={e => { e.stopPropagation(); saveHeights(r, null); }} /></div>
      <For each={Array.from({ length: ncols() }, (_, c) => c)}>{c => {
        const x = () => cells().get(a1(r, c));
        const err = () => ERRORS[x()?.[2] ?? ""];
        return <div class="c" classList={{ chid: width(c) === 0, r: !!x()?.[3], sin: inR(r, c), at: cur()[0] === r && cur()[1] === c, err: !!err(), fz: c < frozen()[1], spill: x()?.[1] === "" }} title={err() ?? undefined}
          style={{ width: `${width(c)}px`, ...cellCss(r, c, rh(r)), ...(c < frozen()[1] ? { position: "sticky", left: `${lefts()[c]}px`, "z-index": 3 } : {}), ...spill(r, c) }}>
          <Show when={!mergeAt(r, c)}><Show when={fmtOf(r, c) === "flag" && x()?.[2]} fallback={<span>{x()?.[2] ?? ""}</span>}><FlagText country={x()![2]} /></Show></Show></div>;
      }}</For>
    </div>
  );
  const hintPos = () => { void edit(); void caret(); const b = el()?.getBoundingClientRect(); return b ? { left: `${Math.min(b.left, innerWidth - 420)}px`, top: `${b.bottom + 2}px` } : {}; };
  return (
    <section class="ganttview">
      <CountryList />
      <div class="ptools sheettools">
        <b title="Type = and click cells to build a formula · plan data by name: Tasks[Finish], Resources[Country], ProjectFinish">Sheets</b><Help k="sheets" />
        <button class="btn sm" title="AutoSum (Alt+=): add up the numbers above" onMouseDown={e => e.preventDefault()} onClick={() => autoSum("SUM")}>Σ AutoSum</button>
        <button class="btn sm caret" title="Average, Count, Max, Min" onClick={e => openMenu(e, sumItems())}>▾</button>
        <button class="btn sm" title="Insert Function: find any of the 490 functions" onMouseDown={e => e.preventDefault()} onClick={insertFn}><i class="fxi">fx</i> Function</button>
        <button class="btn sm" title="A live list of tasks or resources: milestones, late tasks, vendors…" onClick={planData}>Plan Data…</button>
        <span class="sep" /><span class="muted fromplan">From the plan:</span>
        <button class="btn sm" title="The project's key figures with labels: start, finish, status date, % complete, SPI, CPI, counts. They stay live." onClick={projectFacts}>Project facts</button>
        <button class="btn sm" title="Milestones with their finish dates and % complete, live" onClick={() => quickList("Tasks", "milestones", ["Name", "Finish", "% Complete"])}>Milestones</button>
        <button class="btn sm" title="Critical tasks with their dates and float, live" onClick={() => quickList("Tasks", "critical", ["Name", "Start", "Finish", "Total Float", "% Complete"])}>Critical</button>
        <button class="btn sm" title="Late tasks with their finish and % complete, live" onClick={() => quickList("Tasks", "late", ["Name", "Finish", "% Complete"])}>Late</button>
        <button class="btn sm" title="Tasks starting in the next four weeks, live" onClick={() => quickList("Tasks", "next", ["Name", "Start", "Finish", "Resources"], "Start")}>Next 4 weeks</button>
        <button class="btn sm" title="Tasks in progress with their % complete, live" onClick={() => quickList("Tasks", "now", ["Name", "Start", "Finish", "% Complete"])}>In progress</button>
        <button class="btn sm" title="The resources (people, vendors) with their group and country, live" onClick={() => quickList("Resources", "", ["Name", "Group", "Country", "Work"])}>Resources</button>
        <button class="btn sm" title="One value of one task (its finish, % complete…), live" onClick={taskValue}>Task value…</button>
      </div>
      <Show when={P().sheets.length} fallback={<div class="page"><p class="empty">No sheets yet. Sheets are spreadsheets inside the plan: dashboards, quantities, estimates, vendor lists. They read the plan live, so nothing is copied by hand.</p><div class="row2"><button class="btn pri" onClick={addSheet}>Add a sheet</button><button class="btn" onClick={() => import("./ribbon").then(m => m.importWorkbook())}>Bring in an Excel workbook…</button></div></div>}>
        <div class="fbar"><span class="fname">{home() ? `${home()!.k !== k() ? `${P().sheets[home()!.k]?.name}!` : ""}${a1(home()!.r, home()!.c)}` : a1(...cur())}</span><Show when={aggregate(S, (r, c) => { const x = cells().get(a1(r, c)); return x?.[3] ? x[2] : ""; })}>{t => <span class="agg">{t()}</span>}</Show>
          <button class="fx" title="Insert Function" onMouseDown={e => e.preventDefault()} onClick={insertFn}>fx</button>
          <input ref={barEl} value={edit() ?? curCell()?.[1] ?? ""} placeholder={curCell() && curCell()![1] === "" ? "(the result of a formula above or to the left)" : ""}
            onFocus={() => { lastEl = "bar"; if (edit() === null) startEdit(input(k(), ...cur())); }} onInput={e => typed(e.currentTarget)} onKeyDown={edKey} onKeyUp={caretOf} onClick={caretOf} onBlur={blur} /></div>
        <Show when={sheetMode() === "layout"}><div class="slayoutwrap" title="The pages as they will print. Double-click to go back to the cells." onDblClick={() => setSheetMode("normal")}><SheetPagesView zoom={0.85} /></div></Show>
        <div class="gscroll sheetgrid" classList={{ breaks: sheetMode() === "breaks", hidden: sheetMode() === "layout" }} ref={el => { box = el; ro.observe(el); }} tabindex="0" onFocus={() => F.setTarget(fmtTarget)} onKeyDown={key} onMouseDown={down} onCopy={e => onCopy(e)} onCut={e => onCopy(e, true)} onPaste={onPaste} onScroll={() => { setTop(box.scrollTop); if (box.scrollTop + box.clientHeight > top_(nrows()) - 300) setReach([nrows(), reach()[1]]); if (box.scrollLeft + box.clientWidth > (lefts()[ncols() - 1] ?? 0) - 200) setReach([reach()[0], ncols()]); }}
          onContextMenu={e => { e.preventDefault(); if (edit() === null) cellMenu(e); }} onDblClick={() => { if (edit() === null) startEdit(input(k(), ...cur())); }}>
          <div class="gsheet" style={{ width: `${lefts()[ncols() - 1] + width(ncols() - 1) + 40}px`, height: `${top_(nrows()) + RH}px` }}>
            <div class="ghead" style={{ height: `${RH}px` }}>
              <div class="hc idc" style={{ width: `${HW}px`, position: "sticky", left: "0", "z-index": 2 }} />
              <For each={Array.from({ length: ncols() }, (_, c) => c)}>{c => <div class="hc colh" onMouseDown={e => colDown(e, c)} classList={{ chid: width(c) === 0, cnext: hidC().has(c - 1), on: S.ranges().some(g => c >= Math.min(g.c0, g.c1) && c <= Math.max(g.c0, g.c1)) }} style={{ width: `${width(c)}px`, "justify-content": "center", "align-items": "center", position: "relative" }}>{colName(c)}<i class="rsz" title="Drag to resize; double-click to fit" onMouseDown={e => resize(e, c)} onDblClick={e => { e.stopPropagation(); autofit(c); }} /></div>}</For>
            </div>
            <Show when={frozen()[0] > 0}><div class="sfrozen" style={{ top: `${RH}px`, height: `${top_(frozen()[0])}px`, "margin-bottom": `${-top_(frozen()[0])}px` }}><For each={Array.from({ length: frozen()[0] }, (_, r) => r)}>{r => rowView(r)}</For>
              <For each={merges().filter(m => m.r0 < frozen()[0])}>{m => MergeView(m)}</For>
              <Show when={edit() !== null && home()?.k === k() && home()!.r < frozen()[0]}>{EditBox()}</Show></div></Show>
            <div class="gbody">
              <Show when={sheetMode() === "breaks"}><For each={sheetPages()}>{pg => <div class="pbg" style={{ left: `${lefts()[pg.c0]}px`, top: `${top_(pg.r0)}px`, width: `${lefts()[pg.c1] + width(pg.c1) - lefts()[pg.c0]}px`, height: `${top_(pg.r1 + 1) - top_(pg.r0)}px` }} />}</For></Show>
              <For each={vis().filter(r => r >= frozen()[0])}>{r => rowView(r)}</For>
              <For each={merges().filter(m => m.r0 >= frozen()[0] && m.r1 >= (vis()[0] ?? 0) && m.r0 <= (vis()[vis().length - 1] ?? 0))}>{m => MergeView(m)}</For>
              <Show when={abox()}>{b => <>
                <div class="parea" style={{ left: `${colX(b().c0)}px`, top: `${top_(b().r0)}px`, width: `${colX(b().c1 + 1) - colX(b().c0)}px`, height: `${top_(b().r1 + 1) - top_(b().r0)}px` }} />
                <For each={rowCuts()}>{r => <div class="pcut h" classList={{ man: (page().breaks ?? []).includes(r) }} title="Drag to move this page break; drag it out of the print area to remove it" style={{ left: `${colX(b().c0)}px`, top: `${top_(r) - 4}px`, width: `${colX(b().c1 + 1) - colX(b().c0)}px` }} onMouseDown={e => pageDrag(e, "row", r)} />}</For>
                <For each={colCuts()}>{c => <div class="pcut v" classList={{ man: (page().col_breaks ?? []).includes(c) }} title="Drag to move this page break; drag it out of the print area to remove it" style={{ left: `${colX(c) - 4}px`, top: `${top_(b().r0)}px`, height: `${top_(b().r1 + 1) - top_(b().r0)}px` }} onMouseDown={e => pageDrag(e, "col", c)} />}</For>
                <div class="pedge h" title="Drag to change the print area" style={{ left: `${colX(b().c0)}px`, top: `${top_(b().r0) - 4}px`, width: `${colX(b().c1 + 1) - colX(b().c0)}px` }} onMouseDown={e => pageDrag(e, "top", b().r0)} />
                <div class="pedge h" title="Drag to change the print area" style={{ left: `${colX(b().c0)}px`, top: `${top_(b().r1 + 1) - 4}px`, width: `${colX(b().c1 + 1) - colX(b().c0)}px` }} onMouseDown={e => pageDrag(e, "bottom", b().r1 + 1)} />
                <div class="pedge v" title="Drag to change the print area" style={{ left: `${colX(b().c0) - 4}px`, top: `${top_(b().r0)}px`, height: `${top_(b().r1 + 1) - top_(b().r0)}px` }} onMouseDown={e => pageDrag(e, "left", b().c0)} />
                <div class="pedge v" title="Drag to change the print area" style={{ left: `${colX(b().c1 + 1) - 4}px`, top: `${top_(b().r0)}px`, height: `${top_(b().r1 + 1) - top_(b().r0)}px` }} onMouseDown={e => pageDrag(e, "right", b().c1 + 1)} />
                <Show when={pdrag()}>{d => { const v = ["left", "right", "col"].includes(d().kind); return <div class={`pghost ${v ? "v" : "h"}`} style={v ? { left: `${colX(d().to) - 1}px`, top: `${top_(b().r0)}px`, height: `${top_(b().r1 + 1) - top_(b().r0)}px` } : { left: `${colX(b().c0)}px`, top: `${top_(d().to) - 1}px`, width: `${colX(b().c1 + 1) - colX(b().c0)}px` }} />; }}</Show>
              </>}</Show>
              <Show when={sheetMode() === "breaks"}><For each={sheetPages()}>{pg => <div class="pbreak" style={{ left: `${lefts()[pg.c0]}px`, top: `${top_(pg.r0)}px`, width: `${lefts()[pg.c1] + width(pg.c1) - lefts()[pg.c0]}px`, height: `${top_(pg.r1 + 1) - top_(pg.r0)}px` }}><span>Page {pg.n}</span></div>}</For></Show>
              <For each={boxes()}>{b => <div class="refbox" style={{ left: `${lefts()[b.c0]}px`, top: `${top_(b.r0)}px`, width: `${lefts()[b.c1] + width(b.c1) - lefts()[b.c0]}px`, height: `${top_(Math.min(b.r1, nrows() - 1) + 1) - top_(b.r0)}px`, "border-color": b.color, background: `${b.color}14` }} />}</For>
              <For each={imgs()}>{(im, i) => { const rc = () => parseA1(im.cell), box2 = () => picBox(im); return (
                <div class="simg" data-i={i()} classList={{ on: picSel() === i(), fit: !!im.fit }} style={{ left: `${box2()[0]}px`, top: `${box2()[1]}px`, width: `${box2()[2]}px`, height: `${box2()[3]}px` }}
                  title={im.fit ? "Placed in the cell: it follows the cell's size. Drag to move it to another cell." : "Drag to move; drag the corner to resize; Delete removes it"} onMouseDown={e => picDrag(e, i(), false)}>
                  <img src={picData(im.pic)} alt="" draggable={false} />
                  <Show when={picSel() === i()}><Show when={!im.fit}><i class="corner" onMouseDown={e => picDrag(e, i(), true)} /></Show>
                    <button class="x" title="Remove the picture from the sheet" onMouseDown={e => e.stopPropagation()} onClick={() => { putImgs(imgs().filter((_, j) => j !== i())); setPicSel(-1); }}>×</button>
                    <button class="pinc" title={im.fit ? "Place over cells (free size)" : "Place in cell: fit the picture inside its cell or merged cells"} onMouseDown={e => e.stopPropagation()}
                      onClick={() => putImgs(imgs().map((x, j) => (j === i() ? { ...x, fit: !x.fit } : x)))}><Icon n={im.fit ? "image" : "pincell"} /></button></Show>
                </div>); void rc; }}</For>
              <Show when={back()}>{t => { const [a, b, c, d] = range(); return <div class="fillprev" style={{ left: `${lefts()[Math.min(t()[1], b)]}px`, top: `${top_(Math.min(t()[0], a))}px`, width: `${lefts()[d] + width(d) - lefts()[Math.min(t()[1], b)]}px`, height: `${top_(c + 1) - top_(Math.min(t()[0], a))}px` }} />; }}</Show>
              <Show when={fillTo()}>{t => { const [a, b] = cur(); return <div class="fillprev" style={{ left: `${lefts()[Math.min(b, range()[1])]}px`, top: `${top_(Math.min(a, range()[0]))}px`, width: `${lefts()[t()[1]] + width(t()[1]) - lefts()[Math.min(b, range()[1])]}px`, height: `${top_(t()[0] + 1) - top_(Math.min(a, range()[0]))}px` }} />; }}</Show>
              <Show when={edit() === null}><i class="fill" title="Drag to fill (series; formulas follow); hold Ctrl to copy" style={{ left: `${lefts()[range()[3]] + width(range()[3]) - 4}px`, top: `${top_(range()[2] + 1) - 4}px` }} onMouseDown={fillDrag} /></Show>
              <Show when={edit() !== null && home()?.k === k() && home()!.r >= frozen()[0]}>{EditBox()}</Show>
            </div>
          </div>
        </div>
      </Show>
      <Show when={edit() !== null && (sugs().length || sig())}>
        <div class="fhint" style={hintPos()} onMouseDown={e => e.preventDefault()}>
          <Show when={sugs().length} fallback={<Show when={sig()}>{s => <div class="fsig"><code>{s().name}(<For each={s().args}>{(a, i) => <>{i() ? ", " : ""}<span classList={{ on: i() === s().at }}>{a}</span></>}</For>)</code><p>{s().about}</p></div>}</Show>}>
            <For each={sugs()}>{(s, i) => <button classList={{ on: i() === sug() }} onClick={() => accept(s)}><b>{s.label}</b><span>{s.about}</span></button>}</For>
            <p class="muted">Tab to insert · ↑↓ to choose</p>
          </Show>
        </div>
      </Show>
      <div class="sheettabs">
        <For each={P().sheets}>{(s2, i) => <Show when={!s2.hidden}>
          <Show when={renaming() === i()} fallback={
            <button classList={{ on: i() === k(), drop: dragTab() >= 0 && dragTab() !== i() }} style={s2.color ? { "box-shadow": `inset 0 -3px ${s2.color}` } : undefined} draggable={true}
              onDragStart={e => { setDragTab(i()); e.dataTransfer?.setData("text/plain", String(i())); }} onDragEnd={() => setDragTab(-1)} onDragOver={e => e.preventDefault()}
              onDrop={e => { e.preventDefault(); const f = dragTab(); setDragTab(-1); if (f >= 0 && f !== i()) act({ SheetMove: { from: f, to: i() } }).then(() => setK(i())); }}
              onMouseDown={e => { if (pointing()) e.preventDefault(); }} onClick={() => setK(i())} onContextMenu={e => { e.preventDefault(); setK(i()); tabMenu(e, i()); }} onDblClick={() => setRenaming(i())}
              title="Double-click to rename; right-click for more; drag to move">{s2.name}</button>}>
            <input class="tabname" value={s2.name} ref={el2 => setTimeout(() => { el2.focus(); el2.select(); })} onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") rename(i(), e.currentTarget.value); if (e.key === "Escape") setRenaming(-1); }} onBlur={e => rename(i(), e.currentTarget.value)} />
          </Show></Show>}</For>
        <button onClick={addSheet} title="New sheet">＋</button>
        <span class="grow" />
        <span class="smodes">
          <button classList={{ on: sheetMode() === "normal" }} title="Normal: the cells" onClick={() => setSheetMode("normal")}><Icon n="vnormal" /></button>
          <button classList={{ on: sheetMode() === "layout" }} title="Page Layout: the pages as they will print, with header and footer" onClick={() => setSheetMode("layout")}><Icon n="vlayout" /></button>
          <button classList={{ on: sheetMode() === "breaks" }} title="Page Breaks: where each printed page starts and ends" onClick={() => setSheetMode("breaks")}><Icon n="vbreaks" /></button>
        </span>
      </div>
    </section>
  );
};
export { parseA1, toast };
