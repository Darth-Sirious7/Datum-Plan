// Spreadsheet tabs inside the plan (dashboards, quantities, estimates, vendor lists). They work like
// a spreadsheet: type = and click cells to build a formula, functions are suggested as you type, the fill
// handle and paste carry formulas along, and the plan is read live by name: Tasks[Finish],
// Resources[Country], ProjectFinish. The GridCraft engine (in crates/core) calculates everything.
import { JSX, For, Show, batch, createEffect, createMemo, createResource, createSignal, onCleanup, onMount } from "solid-js";
import { SheetOut, SheetData } from "./api";
import { act, call, grid, P, tick, openMenu, toast, Item } from "./store";
import * as F from "./fmt";
import { parseTsv, toTsv, transpose, series, clip } from "./cells";
import { createSel, aggregate } from "./selection";
import { Catalog, ERRORS, Suggestion, a1, blankStrings, callAt, canPoint, colName, cycleAnchor, refs, sheetRef, sigArgs, suggest } from "./formulas";
import { CountryList, FlagText } from "./flags";
import { Help } from "./help";
import { SheetImage } from "./api";
import { pictureMenu, picData } from "./pictures";
import { askText, askYes, Dialog } from "./ui";
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
  if (["money", "date", "flag", "@"].includes(code)) return code;
  if (code === "pct") code = "0.0%";
  return (code || "0").split(";").map(sec => sec.replace(/0(\.(0+))?(?=[^0.]*$)/, (_m, _x, zs?: string) => { const n = Math.max(0, (zs?.length ?? 0) + d); return n ? `0.${"0".repeat(n)}` : "0"; })).join(";");
};
/** The last copy inside Sheets: typed inputs (formulas move when pasted elsewhere) and where they came from. */
const sclip = { text: "", inputs: [] as string[][], r0: 0, c0: 0, cut: false };

/** Cell styles (palette after GridCraft's built-in styles). */
export const CELL_STYLES: [string, string][] = [
  ["Normal", ""], ["Good", "c:#006100;f:#C6EFCE"], ["Bad", "c:#9C0006;f:#FFC7CE"], ["Neutral", "c:#9C5700;f:#FFEB9C"],
  ["Calculation", "b;c:#FA7D00;f:#F2F2F2;bt:thin #7F7F7F;bb:thin #7F7F7F;bl:thin #7F7F7F;br:thin #7F7F7F"], ["Check Cell", "b;c:#FFFFFF;f:#A5A5A5;bt:dbl #3F3F3F;bb:dbl #3F3F3F;bl:dbl #3F3F3F;br:dbl #3F3F3F"],
  ["Input", "c:#3F3F76;f:#FFCC99;bt:thin #7F7F7F;bb:thin #7F7F7F;bl:thin #7F7F7F;br:thin #7F7F7F"], ["Output", "b;c:#3F3F3F;f:#F2F2F2;bt:thin #3F3F3F;bb:thin #3F3F3F;bl:thin #3F3F3F;br:thin #3F3F3F"],
  ["Note", "f:#FFFFCC;bt:thin #B2B2B2;bb:thin #B2B2B2;bl:thin #B2B2B2;br:thin #B2B2B2"], ["Warning Text", "c:#FF0000"], ["Explanatory Text", "i;c:#7F7F7F"], ["Linked Cell", "c:#FA7D00;bb:dbl #FF8001"],
  ["Title", "b;c:#0C1A2B;z:24"], ["Heading 1", "b;c:#0C1A2B;z:20;bb:thick #0B5CAD"], ["Heading 2", "b;c:#0C1A2B;z:17;bb:thick #9DC3E6"], ["Heading 3", "b;c:#0C1A2B;z:14;bb:med #BDD7EE"], ["Heading 4", "b;c:#0C1A2B"],
  ["Total", "b;bt:thin #0B5CAD;bb:dbl #0B5CAD"], ["Accent", "b;c:#FFFFFF;f:#0B5CAD"], ["Accent, light", "c:#0C1A2B;f:#DDEBF7"],
];
/** Standard tab and line colours. */
export const STD_COLORS = ["#C00000", "#FF0000", "#FFC000", "#FFFF00", "#92D050", "#00B050", "#00B0F0", "#0070C0", "#002060", "#7030A0", "#000000", "#7F7F7F"];
/** What the Sheet Tools ribbon tab can do on the sheet shown (set while Sheets is open). */
export type SheetApi = Record<string, (...a: any[]) => unknown> & { frozen: () => boolean; pen: () => [string, string] };
export const [sheetApi, setSheetApi] = createSignal<SheetApi | null>(null);

/** The sheet tab shown (also what File › Print prints). */
export const [sheetK, setSheetK] = createSignal(0);
export const Sheets = () => {
  const k = sheetK, setK = setSheetK;
  const [data] = createResource(() => [tick(), k(), P().sheets.length], () => (P().sheets.length ? call<SheetOut>("sheet_view", { k: Math.min(k(), P().sheets.length - 1) }) : undefined));
  const [cat] = createResource(() => P().fields.length, () => call<Catalog>("sheet_catalog"));
  const cells = createMemo(() => new Map((data()?.cells ?? []).map(c => [c[0], c])));
  const sheet = () => P().sheets[k()];
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
  const nrows = () => Math.max(200, (data()?.rows ?? 0) + 30), ncols = () => Math.max(26, (data()?.cols ?? 0) + 6);
  const width = (c: number) => (tw()?.[0] === c ? tw()![1] : sheet()?.widths?.[c] ?? CW);
  const lefts = createMemo(() => { let x = HW; return Array.from({ length: ncols() }, (_, c) => { const l = x; x += width(c); return l; }); });
  // Row heights: tops[r] is where row r starts (below the header); a row being dragged uses th.
  const [th, setTh] = createSignal<[number, number] | null>(null);
  const rh = (r: number) => (th()?.[0] === r ? th()![1] : sheet()?.heights?.[r] ?? RH);
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
  const S = createSel(() => [nrows(), ncols()], (r, c) => reveal(r, c));
  const cur = (): [number, number] => [S.cur().row, S.cur().col];
  const end = (): [number, number] => { const g = S.ranges()[S.ranges().length - 1]; return g ? [g.r1, g.c1] : cur(); };
  const setCur = ([r, c]: [number, number]) => { S.setCur({ row: r, col: c }); S.setRanges([{ r0: r, c0: c, r1: r, c1: c }]); };
  const setEnd = ([r, c]: [number, number]) => S.drag(r, c);
  const reveal = (r: number, c: number) => {
    if (!box) return;
    if (top_(r) < box.scrollTop) box.scrollTop = top_(r); else if (top_(r + 1) + RH > box.scrollTop + box.clientHeight - 24) box.scrollTop = top_(r + 1) + RH - box.clientHeight + 24;
    const l = lefts()[c] ?? 0, w = width(c);
    if (l - HW < box.scrollLeft) box.scrollLeft = l - HW; else if (l + w > box.scrollLeft + box.clientWidth - 16) box.scrollLeft = l + w - box.clientWidth + 16;
  };
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
    const val = v.startsWith("=") && v.length > 1 ? closeParens(v) : v;
    if (move) { go(Math.max(0, hm.r + move[0]), Math.max(0, hm.c + move[1])); box?.focus(); }
    if (val !== input(hm.k, hm.r, hm.c)) await put([[a1(hm.r, hm.c), val]], hm.k);
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
    const mv = (dr: number, dc: number) => { e.preventDefault(); go(r + dr, c + dc, e.shiftKey); };
    const ctrl = e.ctrlKey || e.metaKey;
    if (e.altKey && e.key === "=") { e.preventDefault(); return autoSum("SUM"); }
    if (ctrl && e.shiftKey && (e.key === "+" || e.key === "=")) { e.preventDefault(); return insertCells(false); }
    if (ctrl && !e.shiftKey && e.key === "-") { e.preventDefault(); return insertCells(true); }
    switch (e.key) {
      case "ArrowDown": return mv(1, 0); case "ArrowUp": return mv(-1, 0); case "ArrowLeft": return mv(0, -1); case "ArrowRight": return mv(0, 1);
      case "Tab": e.preventDefault(); return go(cur()[0], cur()[1] + (e.shiftKey ? -1 : 1));
      case "Enter": case "F2": e.preventDefault(); return startEdit(input(k(), ...cur()));
      case "Delete": case "Backspace": e.preventDefault(); return clear();
    }
    if (ctrl && e.altKey && e.key.toLowerCase() === "v") { e.preventDefault(); return import("./dialogs").then(d => d.pasteSpecial()); }
    if (ctrl && e.key.toLowerCase() === "d") { e.preventDefault(); const [a, , c2] = range(); if (c2 > a) { const [r0, c0, , c1] = range(); setCur([r0, c0]); setEnd([r0, c1]); fillRange(c2, c1, true); } return; }
    if (ctrl && e.key.toLowerCase() === "r") { e.preventDefault(); const [, b, , d] = range(); if (d > b) { const [r0, c0, r1] = range(); setCur([r0, c0]); setEnd([r1, c0]); fillRange(r1, d, true); } return; }
    if (ctrl && ["b", "i", "u"].includes(e.key.toLowerCase())) { e.preventDefault(); return F.toggle(e.key.toLowerCase() as "b"); }
    if (ctrl && e.key.toLowerCase() === "a") { e.preventDefault(); S.all(); return; }
    if (!ctrl && !e.altKey && e.key.length === 1) { e.preventDefault(); startEdit(e.key, cur()[0], cur()[1], true); }
  };
  const copy = (cut = false) => {
    const [r0, c0, r1, c1] = range();
    const lines: string[][] = [], inputs: string[][] = [];
    for (let r = r0; r <= r1; r++) { const l = [], m = []; for (let c = c0; c <= c1; c++) { const shown = cells().get(a1(r, c))?.[2] ?? ""; l.push(shown); m.push(input(k(), r, c) || shown); } lines.push(l); inputs.push(m); }
    const st = sheet()?.style ?? {};
    clip.text = toTsv(lines); clip.styles = Array.from({ length: r1 - r0 + 1 }, (_, i) => Array.from({ length: c1 - c0 + 1 }, (_, j) => st[a1(r0 + i, c0 + j)] ?? ""));
    Object.assign(sclip, { text: clip.text, inputs, r0, c0, cut });
    if (cut) clear();
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
  /** Paste Special: values, formats or both, optionally transposed. */
  const pasteSpecial = async (what: "all" | "values" | "formats", tr: boolean) => {
    const text = clip.text || (await navigator.clipboard?.readText().catch(() => "")) || "";
    if (!text) return toast("Copy some cells first.");
    let st = clip.text === text ? clip.styles : [];
    if (tr && st.length) st = transpose(st);
    const [r0, c0] = cur();
    if (what === "all") await put(await pasted(text, tr));
    if (what === "values") { let m = parseTsv(text); if (tr) m = transpose(m); await put(m.flatMap((l, i) => l.map((v, j) => [a1(r0 + i, c0 + j), v] as [string, string]))); }
    if (what !== "values" && st.length) await act({ SheetStyle: { k: k(), cells: st.flatMap((l, i) => l.map((v, j) => [a1(r0 + i, c0 + j), v] as [string, string])) } });
  };
  const onPaste = async (e: ClipboardEvent) => { if (edit() !== null) return; e.preventDefault(); put(await pasted((e.clipboardData?.getData("text/plain") ?? "").replace(/\r/g, ""))); };
  /** Fills from the selection's first rows down (or first columns right): numbers and dates continue
   *  their series, formulas move their references; Ctrl copies. */
  const fillRange = async (r1: number, c1: number, copyOnly = false) => {
    const [a, b, c, d] = range(), out: [string, string][] = [], moves: [string, number, number][] = [], at: string[] = [];
    const one = (src: string[], k2: number, srcAt: (i: number) => [number, number], to: [number, number]) => {
      const i = (k2 - 1) % src.length, s = src[i];
      if (src.some(x => x.startsWith("="))) { if (s.startsWith("=")) { const [sr, sc] = srcAt(i); moves.push([s, to[0] - sr, to[1] - sc]); at.push(a1(...to)); } else out.push([a1(...to), s]); }
      else out.push([a1(...to), series(src, k2, copyOnly)]);
    };
    if (r1 > c) for (let cc = b; cc <= d; cc++) { const src = Array.from({ length: c - a + 1 }, (_, i) => input(k(), a + i, cc)); for (let r = c + 1; r <= r1; r++) one(src, r - c, i => [a + i, cc], [r, cc]); }
    else if (c1 > d) for (let r = a; r <= c; r++) { const src = Array.from({ length: d - b + 1 }, (_, i) => input(k(), r, b + i)); for (let cc = d + 1; cc <= c1; cc++) one(src, cc - d, i => [r, b + i], [r, cc]); }
    if (moves.length) (await call<string[]>("formula_shift", { items: moves }))?.forEach((f, i) => out.push([at[i], f]));
    setEnd([Math.max(c, r1), Math.max(d, c1)]);
    await put(out);
  };
  const [fillTo, setFillTo] = createSignal<[number, number] | null>(null);
  const fillDrag = (e: MouseEvent) => {
    e.preventDefault(); e.stopPropagation();
    const [, , r1, c1] = range();
    const mv = (m: MouseEvent) => { const [r, c] = at(m.clientX, m.clientY); const dr = r - r1, dc = (c < 0 ? c1 : c) - c1; setFillTo(dr >= dc ? [Math.max(r1, r), c1] : [r1, Math.max(c1, c)]); };
    const up = (u: MouseEvent) => { removeEventListener("mousemove", mv); removeEventListener("mouseup", up); const t = fillTo(); setFillTo(null); if (t) fillRange(t[0], t[1], u.ctrlKey); };
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
  const planData = () => import("./sheetdlg").then(d => d.planDataDialog(cat(), x => {
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
  }));
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
  const borderItems = (): Item[] => [
    { label: "Bottom Border", run: () => border("bottom") }, { label: "Top Border", run: () => border("top") }, { label: "Left Border", run: () => border("left") }, { label: "Right Border", run: () => border("right") }, "-",
    { label: "No Border", run: () => border("none") }, { label: "All Borders", run: () => border("all") }, { label: "Outside Borders", run: () => border("outside") }, { label: "Thick Outside Borders", run: () => border("thickbox") }, "-",
    { label: "Bottom Double Border", run: () => border("dblbottom") }, { label: "Thick Bottom Border", run: () => border("thickbottom") }, { label: "Top and Bottom Border", run: () => border("topbottom") }, { label: "Top and Thick Bottom Border", run: () => border("topthickbottom") }, "-",
    { label: "Line Color", sub: [...STD_COLORS.map(c => ({ label: c, swatch: c, on: pen()[1] === c, run: () => setPen([pen()[0], c]) })), { label: "More Colors…", run: () => { const i = document.createElement("input"); i.type = "color"; i.value = pen()[1]; i.onchange = () => setPen([pen()[0], i.value]); i.click(); } }] },
    { label: "Line Style", sub: ([["Thin", "thin"], ["Medium", "med"], ["Thick", "thick"], ["Double", "dbl"], ["Dashed", "dash"], ["Dotted", "dot"]] as [string, string][]).map(([l, k2]) => ({ label: l, on: pen()[0] === k2, run: () => setPen([k2, pen()[1]]) })) },
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
  const sortRows = (asc: boolean) => {
    const b = bounds(); if (b.r1 <= b.r0) return toast("Select the rows to sort (two or more).");
    const key = (r: number) => { const x = cells().get(a1(r, b.c0)); return x?.[3] ? Number(x[3]) : (x?.[2] ?? "").toLowerCase(); };
    const rows = Array.from({ length: b.r1 - b.r0 + 1 }, (_, i) => b.r0 + i).sort((x, y) => { const p = key(x), q = key(y); const d = typeof p === "number" && typeof q === "number" ? p - q : String(p).localeCompare(String(q), undefined, { numeric: true }); return asc ? d : -d; });
    const vals: [string, string][] = [], sts: [string, string][] = [];
    rows.forEach((src, i) => { for (let c = b.c0; c <= b.c1; c++) { vals.push([a1(b.r0 + i, c), input(k(), src, c)]); sts.push([a1(b.r0 + i, c), sheet().style?.[a1(src, c)] ?? ""]); } });
    act({ Batch: [{ SheetCells: { k: k(), cells: vals, fmt: null } }, { SheetStyle: { k: k(), cells: sts } }] });
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
    openMenu(e, [
      { label: "Cut", keys: "Ctrl+X", run: () => { navigator.clipboard?.writeText(copy(true)).catch(() => {}); } },
      { label: "Copy", keys: "Ctrl+C", run: () => { navigator.clipboard?.writeText(copy()).catch(() => {}); } },
      { label: "Paste", keys: "Ctrl+V", run: async () => put(await pasted(sclip.text || (await navigator.clipboard?.readText().catch(() => "")) || "")) },
      { label: "Paste Special…", keys: "Ctrl+Alt+V", run: () => import("./dialogs").then(d => d.pasteSpecial()) },
      "-",
      { label: nr > 1 ? `Insert ${nr} Rows Above` : "Insert Row Above", run: () => insertCells(false, false) },
      { label: nc > 1 ? `Insert ${nc} Columns Left` : "Insert Column Left", run: () => insertCells(false, true) },
      { label: nr > 1 ? `Delete ${nr} Rows` : "Delete Row", run: () => insertCells(true, false) },
      { label: nc > 1 ? `Delete ${nc} Columns` : "Delete Column", run: () => insertCells(true, true) },
      { label: "Clear Contents", keys: "Delete", run: clear },
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
  const putImgs = (images: SheetImage[]) => act({ SheetImages: { k: k(), images } });
  const placePic = (id: number) => {
    const im = new Image();
    im.onload = () => { const w = Math.min(240, im.naturalWidth || 160), h = Math.max(8, Math.round(w * (im.naturalHeight || 80) / (im.naturalWidth || 160))); putImgs([...imgs(), { pic: id, cell: a1(...cur()), w, h }]).then(() => { setPicSel(imgs().length - 1); box.focus(); }); };
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
    const up = () => { removeEventListener("mousemove", mv); removeEventListener("mouseup", up); F.painted(); };
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
  const rename = (i: number, n: string) => { setRenaming(-1); n = n.trim(); if (n && n !== P().sheets[i].name) act({ Sheet: { k: i, sheet: { ...P().sheets[i], name: n } } }); };
  const putTab = (i: number, x: Partial<SheetData>) => act({ Sheet: { k: i, sheet: { ...P().sheets[i], ...x } } });
  const delSheet = async (i: number) => { if (await askYes("Delete Sheet", `Delete the sheet ${P().sheets[i].name}? Formulas on other sheets that read it will show #REF!.`, "Delete")) act({ Sheet: { k: i, sheet: null } }).then(() => setK(Math.max(0, Math.min(i, P().sheets.length - 1)))); };
  const moveCopy = (i: number) => openDialog(() => {
    const [to, setTo] = createSignal(i), [copy, setCopy] = createSignal(false);
    return <Dialog title="Move or Copy" okText="OK" ok={async () => {
      const n = P().sheets.length, dest = Math.min(to(), n);
      if (copy()) {
        let name = `${P().sheets[i].name} (2)`, j = 2; while (P().sheets.some(x => x.name.toLowerCase() === name.toLowerCase())) name = `${P().sheets[i].name} (${++j})`;
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
      { label: "Tab Color", sub: [{ label: "No Color", on: !P().sheets[i].color, run: () => tabColor(i, "") }, ...STD_COLORS.map(c => ({ label: c, swatch: c, on: P().sheets[i].color === c, run: () => tabColor(i, c) })),
        { label: "More Colors…", run: () => { const el2 = document.createElement("input"); el2.type = "color"; el2.value = P().sheets[i].color || "#0B5CAD"; el2.onchange = () => tabColor(i, el2.value); el2.click(); } }] },
      "-",
      { label: "Hide", off: P().sheets.filter(x => !x.hidden).length < 2, run: () => { putTab(i, { hidden: true }).then(() => { const v = P().sheets.findIndex(x => !x.hidden); setK(Math.max(0, v)); }); } },
      { label: "Unhide", off: !hidden.length, sub: hidden.length ? hidden.map(([j, x]) => ({ label: x.name, run: () => putTab(j, { hidden: false }).then(() => setK(j)) })) : undefined },
    ]);
  };
  const [dragTab, setDragTab] = createSignal(-1);
  const api: SheetApi = {
    frozen: () => !!(frozen()[0] || frozen()[1]), pen, merged,
    border, borderItems, merge, cellStyle, clearKind, fillDown, fillRight, sortRows, rowHeight, colWidth, autofitRows, autofitCols,
    autoSum, sumItems, insertFn, planData, fmtItems, setFmt, moreDecimals, insertCells, addSheet, rename: () => setRenaming(k()), del: () => delSheet(k()), moveCopy: () => moveCopy(k()), tabMenu: (e: MouseEvent) => tabMenu(e, k()),
    picture: (e: MouseEvent) => pictureMenu(e, placePic), freeze: () => freeze(frozen()[0] || frozen()[1] ? [0, 0] : [cur()[0], cur()[1]]),
    fmtLabel: () => FORMATS.find(f => f[1] === fmtOf(...cur()))?.[0].split(" (")[0] ?? (fmtOf(...cur()) || "General"),
    copy: () => { navigator.clipboard?.writeText(copy()).catch(() => {}); box?.focus(); }, cut: () => { navigator.clipboard?.writeText(copy(true)).catch(() => {}); box?.focus(); },
    paste: async () => { await put(await pasted(sclip.text || (await navigator.clipboard?.readText().catch(() => "")) || "")); box?.focus(); },
    focus: () => { box?.focus(); F.setTarget(fmtTarget); },
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
    const p = F.parse(st), o = F.css(st) as Record<string, string>, flex = p.w || p.v || hgt !== RH;
    if (!flex) return { ...o, "line-height": `${hgt - 1}px` };
    return { ...o, display: "flex", "align-items": p.v === "t" ? "flex-start" : p.v === "b" ? "flex-end" : "center", "justify-content": o["justify-content"] ?? (cells().get(a1(r, c))?.[3] ? "flex-end" : "flex-start"),
      "line-height": p.w ? "1.25" : `${RH - 3}px`, "white-space": p.w ? "normal" : "nowrap", "text-align": o["text-align"] ?? "left", "padding-top": "1px", "padding-bottom": "1px" } as JSX.CSSProperties;
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
  const rowView = (r: number) => (
    <div class="row" style={{ top: `${top_(r)}px`, height: `${rh(r)}px` }}>
      <div class="c idc rowh" classList={{ on: S.ranges().some(g => r >= Math.min(g.r0, g.r1) && r <= Math.max(g.r0, g.r1)) }} style={{ width: `${HW}px`, position: "sticky", left: "0", "z-index": 2, "text-align": "center", "line-height": `${rh(r) - 1}px` }}>{r + 1}<i class="rszr" title="Drag to change the row height; double-click for the standard height" onMouseDown={e => rowResize(e, r)} onDblClick={e => { e.stopPropagation(); saveHeights(r, null); }} /></div>
      <For each={Array.from({ length: ncols() }, (_, c) => c)}>{c => {
        const x = () => cells().get(a1(r, c));
        const err = () => ERRORS[x()?.[2] ?? ""];
        return <div class="c" classList={{ r: !!x()?.[3], sin: inR(r, c), at: cur()[0] === r && cur()[1] === c, err: !!err(), fz: c < frozen()[1], spill: x()?.[1] === "" }} title={err() ?? undefined}
          style={{ width: `${width(c)}px`, ...cellCss(r, c, rh(r)), ...(c < frozen()[1] ? { position: "sticky", left: `${lefts()[c]}px`, "z-index": 1 } : {}) }}>
          <Show when={!mergeAt(r, c)}><Show when={fmtOf(r, c) === "flag" && x()?.[2]} fallback={<span>{x()?.[2] ?? ""}</span>}><FlagText country={x()![2]} /></Show></Show></div>;
      }}</For>
    </div>
  );
  const hintPos = () => { void edit(); void caret(); const b = el()?.getBoundingClientRect(); return b ? { left: `${Math.min(b.left, innerWidth - 420)}px`, top: `${b.bottom + 2}px` } : {}; };
  return (
    <section class="ganttview">
      <CountryList />
      <div class="ptools sheettools">
        <b>Sheets</b><Help k="sheets" />
        <button class="btn sm" title="AutoSum (Alt+=): add up the numbers above" onMouseDown={e => e.preventDefault()} onClick={() => autoSum("SUM")}>Σ AutoSum</button>
        <button class="btn sm caret" title="Average, Count, Max, Min" onClick={e => openMenu(e, sumItems())}>▾</button>
        <button class="btn sm" title="Insert Function: find any of the 490 functions" onMouseDown={e => e.preventDefault()} onClick={insertFn}><i class="fxi">fx</i> Function</button>
        <button class="btn sm" title="A live list of tasks or resources: milestones, late tasks, vendors…" onClick={planData}>Plan Data…</button>
        <span class="grow" />
        <span class="muted hintline">Type = and click cells · plan data: Tasks[Finish], Resources[Country], ProjectFinish</span>
      </div>
      <Show when={P().sheets.length} fallback={<div class="page"><p class="empty">No sheets yet. Sheets are spreadsheets inside the plan: dashboards, quantities, estimates, vendor lists. They read the plan live, so nothing is copied by hand.</p><button class="btn pri" style={{ width: "fit-content" }} onClick={addSheet}>Add a sheet</button></div>}>
        <div class="fbar"><span class="fname">{home() ? `${home()!.k !== k() ? `${P().sheets[home()!.k]?.name}!` : ""}${a1(home()!.r, home()!.c)}` : a1(...cur())}</span><Show when={aggregate(S, (r, c) => cells().get(a1(r, c))?.[2] ?? "")}>{t => <span class="agg">{t()}</span>}</Show>
          <button class="fx" title="Insert Function" onMouseDown={e => e.preventDefault()} onClick={insertFn}>fx</button>
          <input ref={barEl} value={edit() ?? curCell()?.[1] ?? ""} placeholder={curCell() && curCell()![1] === "" ? "(the result of a formula above or to the left)" : ""}
            onFocus={() => { lastEl = "bar"; if (edit() === null) startEdit(input(k(), ...cur())); }} onInput={e => typed(e.currentTarget)} onKeyDown={edKey} onKeyUp={caretOf} onClick={caretOf} onBlur={blur} /></div>
        <div class="gscroll sheetgrid" ref={el => { box = el; ro.observe(el); }} tabindex="0" onFocus={() => F.setTarget(fmtTarget)} onKeyDown={key} onMouseDown={down} onCopy={e => onCopy(e)} onCut={e => onCopy(e, true)} onPaste={onPaste} onScroll={() => setTop(box.scrollTop)}
          onContextMenu={e => { e.preventDefault(); if (edit() === null) cellMenu(e); }} onDblClick={() => { if (edit() === null) startEdit(input(k(), ...cur())); }}>
          <div class="gsheet" style={{ width: `${lefts()[ncols() - 1] + width(ncols() - 1) + 40}px`, height: `${top_(nrows()) + RH}px` }}>
            <div class="ghead" style={{ height: `${RH}px` }}>
              <div class="hc idc" style={{ width: `${HW}px`, position: "sticky", left: "0", "z-index": 2 }} />
              <For each={Array.from({ length: ncols() }, (_, c) => c)}>{c => <div class="hc colh" onMouseDown={e => colDown(e, c)} classList={{ on: S.ranges().some(g => c >= Math.min(g.c0, g.c1) && c <= Math.max(g.c0, g.c1)) }} style={{ width: `${width(c)}px`, "justify-content": "center", "align-items": "center", position: "relative" }}>{colName(c)}<i class="rsz" title="Drag to resize; double-click to fit" onMouseDown={e => resize(e, c)} onDblClick={e => { e.stopPropagation(); autofit(c); }} /></div>}</For>
            </div>
            <Show when={frozen()[0] > 0}><div class="sfrozen" style={{ top: `${RH}px`, height: `${top_(frozen()[0])}px`, "margin-bottom": `${-top_(frozen()[0])}px` }}><For each={Array.from({ length: frozen()[0] }, (_, r) => r)}>{r => rowView(r)}</For></div></Show>
            <div class="gbody">
              <For each={vis().filter(r => r >= frozen()[0])}>{r => rowView(r)}</For>
              <For each={merges().filter(m => m.r1 >= (vis()[0] ?? 0) && m.r0 <= (vis()[vis().length - 1] ?? 0))}>{m => MergeView(m)}</For>
              <For each={boxes()}>{b => <div class="refbox" style={{ left: `${lefts()[b.c0]}px`, top: `${top_(b.r0)}px`, width: `${lefts()[b.c1] + width(b.c1) - lefts()[b.c0]}px`, height: `${top_(Math.min(b.r1, nrows() - 1) + 1) - top_(b.r0)}px`, "border-color": b.color, background: `${b.color}14` }} />}</For>
              <For each={imgs()}>{(im, i) => { const rc = () => parseA1(im.cell); return (
                <div class="simg" data-i={i()} classList={{ on: picSel() === i() }} style={{ left: `${lefts()[rc()[1]] ?? 0}px`, top: `${top_(rc()[0])}px`, width: `${im.w}px`, height: `${im.h}px` }}
                  title="Drag to move; drag the corner to resize; Delete removes it" onMouseDown={e => picDrag(e, i(), false)}>
                  <img src={picData(im.pic)} alt="" draggable={false} />
                  <Show when={picSel() === i()}><i class="corner" onMouseDown={e => picDrag(e, i(), true)} /><button class="x" title="Remove the picture from the sheet" onMouseDown={e => e.stopPropagation()} onClick={() => { putImgs(imgs().filter((_, j) => j !== i())); setPicSel(-1); }}>×</button></Show>
                </div>); }}</For>
              <Show when={fillTo()}>{t => { const [a, b] = cur(); return <div class="fillprev" style={{ left: `${lefts()[Math.min(b, range()[1])]}px`, top: `${top_(Math.min(a, range()[0]))}px`, width: `${lefts()[t()[1]] + width(t()[1]) - lefts()[Math.min(b, range()[1])]}px`, height: `${top_(t()[0] + 1) - top_(Math.min(a, range()[0]))}px` }} />; }}</Show>
              <Show when={edit() === null}><i class="fill" title="Drag to fill (series; formulas follow); hold Ctrl to copy" style={{ left: `${lefts()[range()[3]] + width(range()[3]) - 4}px`, top: `${top_(range()[2] + 1) - 4}px` }} onMouseDown={fillDrag} /></Show>
              <Show when={edit() !== null && home()?.k === k()}>
                <input class="edit" list={fmtOf(home()!.r, home()!.c) === "flag" ? "countries" : undefined} style={{ left: `${lefts()[home()!.c]}px`, top: `${top_(home()!.r)}px`, width: `${Math.max(mergeAt(home()!.r, home()!.c) ? lefts()[mergeAt(home()!.r, home()!.c)!.c1] + width(mergeAt(home()!.r, home()!.c)!.c1) - lefts()[home()!.c] : width(home()!.c), 180)}px`, height: `${Math.max(RH, rh(home()!.r))}px` }} value={edit()!}
                  ref={e => { cellEl = e; setTimeout(() => { if (document.activeElement !== barEl) { lastEl = "cell"; e.focus(); e.setSelectionRange(caret(), caret()); } }); }} onMouseDown={e => e.stopPropagation()}
                  onFocus={() => { lastEl = "cell"; }} onInput={e => typed(e.currentTarget)} onKeyDown={edKey} onKeyUp={caretOf} onClick={caretOf} onBlur={blur} />
              </Show>
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
      </div>
    </section>
  );
};
export { parseA1, toast };
