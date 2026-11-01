// Spreadsheet tabs inside the plan (dashboards, quantities, estimates, vendor lists). They work like
// Excel: type = and click cells to build a formula, functions are suggested as you type, the fill
// handle and paste carry formulas along, and the plan is read live by name: Tasks[Finish],
// Resources[Country], ProjectFinish. The GridCraft engine (in crates/core) calculates everything.
import { For, Show, batch, createEffect, createMemo, createResource, createSignal, onCleanup, onMount } from "solid-js";
import { SheetOut } from "./api";
import { act, call, grid, P, tick, openMenu, toast, Item } from "./store";
import * as F from "./fmt";
import { parseTsv, toTsv, transpose, series, clip } from "./cells";
import { createSel, aggregate } from "./selection";
import { Catalog, ERRORS, Suggestion, a1, blankStrings, callAt, canPoint, colName, cycleAnchor, refs, sheetRef, sigArgs, suggest } from "./formulas";
import { CountryList, FlagText } from "./flags";
import { Help } from "./help";

const RH = 22, CW = 96, HW = 46;
const COLORS = ["#2F6FDE", "#D64545", "#7A4FD8", "#1F9D55", "#C77700", "#C2185B"];
const parseA1 = (s: string): [number, number] => { const m = s.match(/^([A-Z]+)(\d+)$/)!; let c = 0; for (const ch of m[1]) c = c * 26 + ch.charCodeAt(0) - 64; return [+m[2] - 1, c - 1]; };
/** Excel closes the brackets left open when Enter is pressed. */
const closeParens = (t: string) => { const b = blankStrings(t); return t + ")".repeat(Math.max(0, (b.match(/\(/g)?.length ?? 0) - (b.match(/\)/g)?.length ?? 0))); };
/** Number formats offered on the toolbar and the cell menu: label and format (Excel code or our short name). */
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
    if (r * RH < box.scrollTop) box.scrollTop = r * RH; else if ((r + 2) * RH > box.scrollTop + box.clientHeight - 24) box.scrollTop = (r + 2) * RH - box.clientHeight + 24;
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
  /** What a paste puts at the cursor: copies made in Sheets bring their formulas (moved by the distance, as in Excel); anything else pastes as text. */
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
   *  their series, formulas move their references as in Excel; Ctrl copies. */
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

  // ---------- Excel commands ----------
  /** AutoSum (Alt+=): one cell gets =SUM of the numbers above it (or to its left); a range gets totals under each column. */
  const autoSum = (fn: string) => {
    const g = S.last(), num = (r: number, c: number) => !!cells().get(a1(r, c))?.[3];
    if (g.r0 === g.r1 && g.c0 === g.c1) {
      const [r, c] = [g.r0, g.c0];
      let t = r - 1, l = c - 1;
      while (t >= 0 && num(t, c)) t--;
      while (l >= 0 && num(r, l)) l--;
      const ref = t < r - 1 ? `${a1(t + 1, c)}:${a1(r - 1, c)}` : l < c - 1 ? `${a1(r, l + 1)}:${a1(r, c - 1)}` : "";
      // As in Excel the proposed range stays "live": clicking or dragging other cells replaces it.
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
    const y = vy < fr * RH ? vy : vy + box.scrollTop, x = fc && vx < lefts()[fc] ? vx : vx + box.scrollLeft;
    return [Math.floor(y / RH), lefts().findIndex((l, j) => x >= l && x < l + width(j))];
  };
  const down = (e: MouseEvent) => {
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
    if (c < 0) S.rows(r, r, e.shiftKey, e.ctrlKey); else go(r, c, e.shiftKey, e.ctrlKey);
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
  const addSheet = () => { let n = P().sheets.length + 1; while (P().sheets.some(s => s.name.toLowerCase() === `sheet${n}`)) n++; act({ Sheet: { k: null, sheet: { name: `Sheet${n}`, cells: {}, fmt: {}, widths: {} } } }).then(() => setK(P().sheets.length - 1)); };
  const tabMenu = (e: MouseEvent, i: number) => openMenu(e, [
    { label: "Rename…", run: () => { const n = prompt("Sheet name", P().sheets[i].name); if (n?.trim()) act({ Sheet: { k: i, sheet: { ...P().sheets[i], name: n.trim() } } }); } },
    { label: "Delete Sheet", run: () => { if (confirm(`Delete ${P().sheets[i].name}? Formulas on other sheets that read it will show #REF!.`)) act({ Sheet: { k: i, sheet: null } }).then(() => setK(0)); } },
  ]);
  const vis = createMemo(() => { const f = Math.max(0, Math.floor(top() / RH) - 2); return Array.from({ length: Math.ceil(h() / RH) + 6 }, (_, j) => f + j).filter(r => r < nrows()); });
  const curCell = () => cells().get(a1(...cur()));
  /** Coloured boxes round the references of the formula being typed (those on the tab shown). */
  const boxes = createMemo(() => {
    if (edit() === null || !edit()!.startsWith("=") || !home()) return [];
    const here = sheet()?.name.toLowerCase();
    return refs(edit()!).map((r, i) => ({ ...r, color: COLORS[i % COLORS.length] })).filter(r => (r.sheet ? r.sheet.toLowerCase() === here : home()!.k === k()) && r.c1 < ncols());
  });
  const rowView = (r: number) => (
    <div class="row" style={{ top: `${r * RH}px`, height: `${RH}px` }}>
      <div class="c idc" style={{ width: `${HW}px`, position: "sticky", left: "0", "z-index": 2, "text-align": "center" }}>{r + 1}</div>
      <For each={Array.from({ length: ncols() }, (_, c) => c)}>{c => {
        const x = () => cells().get(a1(r, c));
        const err = () => ERRORS[x()?.[2] ?? ""];
        return <div class="c" classList={{ r: !!x()?.[3], in: inR(r, c), at: cur()[0] === r && cur()[1] === c, err: !!err(), fz: c < frozen()[1], spill: x()?.[1] === "" }} title={err() ?? undefined}
          style={{ width: `${width(c)}px`, "line-height": `${RH - 1}px`, ...(c < frozen()[1] ? { position: "sticky", left: `${lefts()[c]}px`, "z-index": 1 } : {}), ...F.css(sheet()?.style?.[a1(r, c)]) }}>
          <Show when={fmtOf(r, c) === "flag" && x()?.[2]} fallback={x()?.[2] ?? ""}><FlagText country={x()![2]} /></Show></div>;
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
        <span class="sep" />
        <button class="btn sm" title="Number format" onClick={e => openMenu(e, fmtItems())}>{FORMATS.find(f => f[1] === fmtOf(...cur()))?.[0].split(" (")[0] ?? (fmtOf(...cur()) || "General")} ▾</button>
        <button class="btn sm" title="More decimals" onClick={() => moreDecimals(1)}>.0+</button>
        <button class="btn sm" title="Fewer decimals" onClick={() => moreDecimals(-1)}>.0−</button>
        <span class="sep" />
        <button class="btn sm" title="Insert rows or columns (Ctrl+Shift++)" onClick={e => openMenu(e, [{ label: "Insert Rows Above", run: () => insertCells(false, false) }, { label: "Insert Columns Left", run: () => insertCells(false, true) }])}>Insert ▾</button>
        <button class="btn sm" title="Delete rows or columns (Ctrl+−)" onClick={e => openMenu(e, [{ label: "Delete Rows", run: () => insertCells(true, false) }, { label: "Delete Columns", run: () => insertCells(true, true) }])}>Delete ▾</button>
        <button class="btn sm" title="Keep the rows above and the columns left of the selected cell in view" onClick={() => freeze(frozen()[0] || frozen()[1] ? [0, 0] : [cur()[0], cur()[1]])}>{frozen()[0] || frozen()[1] ? "Unfreeze Panes" : "Freeze Panes"}</button>
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
          <div class="gsheet" style={{ width: `${lefts()[ncols() - 1] + width(ncols() - 1) + 40}px`, height: `${(nrows() + 1) * RH}px` }}>
            <div class="ghead" style={{ height: `${RH}px` }}>
              <div class="hc idc" style={{ width: `${HW}px`, position: "sticky", left: "0", "z-index": 2 }} />
              <For each={Array.from({ length: ncols() }, (_, c) => c)}>{c => <div class="hc colh" onMouseDown={e => colDown(e, c)} classList={{ on: S.ranges().some(g => c >= Math.min(g.c0, g.c1) && c <= Math.max(g.c0, g.c1)) }} style={{ width: `${width(c)}px`, "justify-content": "center", "align-items": "center", position: "relative" }}>{colName(c)}<i class="rsz" title="Drag to resize; double-click to fit" onMouseDown={e => resize(e, c)} onDblClick={e => { e.stopPropagation(); autofit(c); }} /></div>}</For>
            </div>
            <Show when={frozen()[0] > 0}><div class="sfrozen" style={{ top: `${RH}px`, height: `${frozen()[0] * RH}px`, "margin-bottom": `${-frozen()[0] * RH}px` }}><For each={Array.from({ length: frozen()[0] }, (_, r) => r)}>{r => rowView(r)}</For></div></Show>
            <div class="gbody">
              <For each={vis().filter(r => r >= frozen()[0])}>{r => rowView(r)}</For>
              <For each={boxes()}>{b => <div class="refbox" style={{ left: `${lefts()[b.c0]}px`, top: `${b.r0 * RH}px`, width: `${lefts()[b.c1] + width(b.c1) - lefts()[b.c0]}px`, height: `${(Math.min(b.r1, nrows() - 1) - b.r0 + 1) * RH}px`, "border-color": b.color, background: `${b.color}14` }} />}</For>
              <Show when={fillTo()}>{t => { const [a, b] = cur(); return <div class="fillprev" style={{ left: `${lefts()[Math.min(b, range()[1])]}px`, top: `${Math.min(a, range()[0]) * RH}px`, width: `${lefts()[t()[1]] + width(t()[1]) - lefts()[Math.min(b, range()[1])]}px`, height: `${(t()[0] - Math.min(a, range()[0]) + 1) * RH}px` }} />; }}</Show>
              <Show when={edit() === null}><i class="fill" title="Drag to fill (series; formulas follow); hold Ctrl to copy" style={{ left: `${lefts()[range()[3]] + width(range()[3]) - 4}px`, top: `${(range()[2] + 1) * RH - 4}px` }} onMouseDown={fillDrag} /></Show>
              <Show when={edit() !== null && home()?.k === k()}>
                <input class="edit" list={fmtOf(home()!.r, home()!.c) === "flag" ? "countries" : undefined} style={{ left: `${lefts()[home()!.c]}px`, top: `${home()!.r * RH}px`, width: `${Math.max(width(home()!.c), 180)}px`, height: `${RH}px` }} value={edit()!}
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
        <For each={P().sheets}>{(s, i) => <button classList={{ on: i() === k() }} onMouseDown={e => { if (pointing()) e.preventDefault(); }} onClick={() => setK(i())} onContextMenu={e => { e.preventDefault(); tabMenu(e, i()); }} onDblClick={() => tabMenu(new MouseEvent("click", { clientX: 200, clientY: innerHeight - 80 }), i())}>{s.name}</button>}</For>
        <button onClick={addSheet} title="New sheet">＋</button>
      </div>
    </section>
  );
};
export { parseA1, toast };
