// The task sheet, built like a spreadsheet: one scroller with a sticky header (so columns always line
// up), frozen ID and name columns, range selection, fill handle, clipboard ranges, a formula bar,
// AutoFilter buttons and inline header rename.
import { For, Show, createSignal, createMemo, createEffect, onMount, onCleanup, untrack, on, JSX } from "solid-js";
import { Col, Row } from "./api";
import { toggleSub, grid, cols, setCols, saveCols, hideCol, unhideCol, hiddenCols, sel, setSel, cur, setCur, act, openMenu, Item, fields, P, setSort, sort, setGroup, refresh, table, filter, setFilter, ALL, toast, call } from "./store";
import { C } from "./cmds";
import * as D from "./dialogs";
import * as F from "./fmt";
import { parseTsv, transpose, series, clip } from "./cells";
import { Range, norm, createSel, aggregate } from "./selection";
import { Icon, DateBtn } from "./ui";

export const RH = 20;
const IDW = 44;
export const [scrollTop, setScrollTop] = createSignal(0);
export type { Range } from "./selection";
export const [ranges, setRanges] = createSignal<Range[]>([]);
/** Sum, average and count of the selected numbers, for the status bar. */
export const [agg, setAgg] = createSignal("");

const CHOICES: Record<string, string[]> = {
  ctype: ["As Soon As Possible", "As Late As Possible", "Start No Earlier Than", "Start No Later Than", "Finish No Earlier Than", "Finish No Later Than", "Must Start On", "Must Finish On"],
  mode: ["Auto Scheduled", "Manually Scheduled"],
  type: ["Fixed Duration", "Fixed Units", "Fixed Work"],
  acttype: ["Task Dependent", "Resource Dependent", "Level of Effort", "Start Milestone", "Finish Milestone", "WBS Summary"],
  pcttype: ["Duration", "Units", "Physical"],
  accrual: ["Start", "Prorated", "End"],
  active: ["Yes", "No"], effort: ["Yes", "No"], timeline: ["Yes", "No"],
};
const choices = (k: string): string[] | undefined => {
  if (CHOICES[k]) return CHOICES[k];
  if (k === "cal") return ["", ...P().calendars.map(c => c.name)];
  if (k.startsWith("code:")) return ["", ...(P().codes.find(c => c.name === k.slice(5))?.values.map(v => v.value) ?? [])];
  const def = k.startsWith("c:") && P().fields.find(f => f.key === k.slice(2));
  if (def && def.lookup.length && def.restrict) return ["", ...def.lookup.map(l => l.value)];
  if (k.startsWith("c:flag")) return ["Yes", "No"];
  return undefined;
};
const right = (c: Col) => ["Num", "Int", "Cost", "Pct", "Work", "Dur"].includes(c.kind);
export const formulaOf = (key: string) => key.startsWith("c:") ? P().fields.find(f => f.key === key.slice(2))?.formula ?? "" : "";

/** Field picker menu (Insert Column, Group by, Sort by). */
/** A column's display name: the custom alias, a renamed heading, or the field name. */
export const fieldName = (key: string) => P().fields.find(f => `c:${f.key}` === key)?.alias || P().titles?.[key] || fields().task.find(f => f.key === key)?.name || key;
export const fieldMenu = (run: (key: string) => void): Item[] => {
  const groups = new Map<string, Item[]>();
  const names = new Map(P().fields.map(f => [`c:${f.key}`, f.alias]));
  for (const f of fields().task) {
    const g = groups.get(f.group) ?? [];
    const alias = names.get(f.key) || P().titles?.[f.key];
    g.push({ label: alias ? `${alias} (${f.name})` : f.name, run: () => run(f.key) });
    groups.set(f.group, g);
  }
  const items: Item[] = [{ label: "Search all fields…", icon: "search", run: () => D.insertColumn(run) }, { label: "New Formula Column…", icon: "fx", run: () => D.formulaColumn() }, "-", ...[...groups].map(([label, sub]) => ({ label, sub }))];
  if (P().codes.length) items.push({ label: "Activity codes", sub: P().codes.map(c => ({ label: c.name, run: () => run(`code:${c.name}`) })) });
  return items;
};

export const Grid = (p: { width?: number }) => {
  let box!: HTMLDivElement;
  const [edit, setEdit] = createSignal<{ row: number; col: number; text: string; all?: boolean } | null>(null);
  const [rename, setRename] = createSignal<number | null>(null);
  const [viewH, setViewH] = createSignal(600);
  const [fillTo, setFillTo] = createSignal<number | null>(null);
  const widths = createMemo(() => cols().map(c => c[1]));
  const lefts = createMemo(() => { let x = IDW; return widths().map(w => { const l = x; x += w; return l; }); });
  const total = createMemo(() => IDW + widths().reduce((a, b) => a + b, 0));
  // Columns up to and including the task name stay in view when scrolling sideways.
  const frozen = createMemo(() => { const k = cols().findIndex(c => c[0] === "name"); return k >= 0 && k < 3 ? k : -1; });
  const rows = () => grid().rows;
  const ncols = () => grid().cols.length;
  const first = () => Math.max(0, Math.floor(scrollTop() / RH) - 4);
  const visible = createMemo(() => rows().slice(first(), first() + Math.ceil(viewH() / RH) + 10).map((r, j) => [r, first() + j] as [Row, number]));
  const selSet = createMemo(() => new Set(sel()));
  // The shared grid engine's selection (also used by the Sheets tabs), kept in the store's signals.
  const S = createSel(() => [rows().length, ncols()], (r, c) => reveal(r, c), { ranges, setRanges, cur, setCur });
  const inRange = S.inRange, wholeRows = S.wholeRows;

  // Keep the task selection (used by commands and the Gantt) in step with the ranges.
  createEffect(() => {
    const u = new Set<number>();
    ranges().forEach(g => { const n = norm(g); for (let r = n.r0; r <= n.r1; r++) { const x = rows()[r]; if (x && x.uid > 0) u.add(x.uid); } });
    setSel([...u]);
    setAgg(aggregate(S, (r, c) => rows()[r]?.cells[c] ?? ""));
  });
  // Selection from elsewhere (Gantt, network, find) moves the active cell.
  // Runs only when the selection changes (not on a cell click, which would race the sync above and jump back).
  createEffect(on(sel, s => untrack(() => {
    if (s.length === 1 && rows()[cur().row]?.uid !== s[0]) {
      const k = rows().findIndex(r => r.uid === s[0]);
      if (k >= 0 && !ranges().some(g => k >= Math.min(g.r0, g.r1) && k <= Math.max(g.r0, g.r1))) { setCur({ row: k, col: cur().col }); setRanges([{ r0: k, c0: cur().col, r1: k, c1: cur().col }]); }
    }
  }), { defer: true }));

  const ro = new ResizeObserver(() => setViewH(box.clientHeight));
  // The chart scrolls with the sheet. A scroll the chart caused is not sent back to it: an echo
  // arrives late during smooth scrolling and pulled the sheet back (it felt stuck).
  let echo = false;
  const sync = (e: Event) => { const v = (e as CustomEvent<number>).detail, was = box.scrollTop; if (Math.abs(was - v) > 1) { box.scrollTop = v; echo = box.scrollTop !== was; } };
  // Font and Alignment commands act on the selected cells (whole rows style the row).
  const fmtTarget: F.Target = {
    get: () => {
      const out: [string, string][] = [], whole = wholeRows();
      for (const g of ranges()) { const n = norm(g); for (let r = n.r0; r <= n.r1 && out.length < 20000; r++) { const x = rows()[r]; if (!x || x.uid <= 0) continue; for (let c = whole ? -1 : n.c0; c <= (whole ? -1 : n.c1); c++) { const key = c < 0 ? "*" : grid().cols[c]?.key; if (key) out.push([`${x.uid}|${key}`, x.style?.[key] ?? ""]); } } }
      return out;
    },
    set: list => act({ Style: { cells: list.map(([id, v]) => { const [u, key] = id.split("|"); return [+u, key, v]; }) } }),
    paste: (what, tr) => pasteSpecial(what, tr),
  };
  onMount(() => { ro.observe(box); document.addEventListener("grid-scroll", sync); F.setTarget(fmtTarget); });
  onCleanup(() => { ro.disconnect(); document.removeEventListener("grid-scroll", sync); if (F.target() === fmtTarget) F.setTarget(null); });

  const reveal = (row: number, col: number) => {
    const top = row * RH, h = box.clientHeight - 40 - 16;
    if (top < box.scrollTop) box.scrollTop = top;
    else if (top + RH > box.scrollTop + h) box.scrollTop = top + RH - h;
    if (col >= 0 && col > frozen()) {
      const l = lefts()[col], w = widths()[col], fz = frozen() >= 0 ? lefts()[frozen()] + widths()[frozen()] : IDW;
      if (l - fz < box.scrollLeft) box.scrollLeft = l - fz;
      else if (l + w > box.scrollLeft + box.clientWidth - 16) box.scrollLeft = l + w - box.clientWidth + 16;
    }
  };
  const pick = S.pick;
  const cellAt = (e: MouseEvent): [number, number] => {
    const b = box.getBoundingClientRect();
    const y = e.clientY - b.top + box.scrollTop - 40, xAbs = e.clientX - b.left;
    const fzEnd = frozen() >= 0 ? lefts()[frozen()] + widths()[frozen()] : IDW;
    const x = xAbs < fzEnd ? xAbs : xAbs + box.scrollLeft;
    let col = lefts().findIndex((l, k) => x >= l && x < l + widths()[k]);
    if (col < 0) col = x < IDW ? -1 : ncols() - 1;
    return [Math.floor(y / RH), col];
  };
  // Drag across cells to select a range; drag the fill handle to fill down.
  const down = (e: MouseEvent) => {
    if (e.button !== 0 || edit()) return;
    const [r, c] = cellAt(e);
    if (c < 0) { // the ID column selects whole rows
      if (e.shiftKey && ranges().length) setRanges([...ranges().slice(0, -1), { ...ranges()[ranges().length - 1], r1: r, c0: 0, c1: ncols() - 1 }]);
      else { setRanges([...(e.ctrlKey ? ranges() : []), { r0: r, c0: 0, r1: r, c1: ncols() - 1 }]); setCur({ row: r, col: cur().col }); }
    } else pick(r, c, e.shiftKey, e.ctrlKey);
    box.focus();
    const mv = (m: MouseEvent) => { const [r2, c2] = cellAt(m); const last = ranges()[ranges().length - 1]; if (last) setRanges([...ranges().slice(0, -1), { ...last, r1: Math.max(0, r2), c1: c < 0 ? last.c1 : Math.max(0, c2) }]); };
    const up = () => { removeEventListener("mousemove", mv); removeEventListener("mouseup", up); F.painted(); };
    addEventListener("mousemove", mv); addEventListener("mouseup", up);
  };
  const fillDrag = (e: MouseEvent) => {
    e.preventDefault(); e.stopPropagation();
    let ctrl = e.ctrlKey;
    addEventListener("keydown", k => { if (k.key === "Control") ctrl = true; }, { once: true });
    const g = norm(ranges()[ranges().length - 1]);
    const mv = (m: MouseEvent) => setFillTo(Math.max(g.r1, cellAt(m)[0]));
    const up = async () => {
      removeEventListener("mousemove", mv); removeEventListener("mouseup", up);
      const to = fillTo(); setFillTo(null);
      if (to === null || to <= g.r1) return;
      await fill(g, to, ctrl);
    };
    addEventListener("mousemove", mv); addEventListener("mouseup", up);
  };
  /** Fills the rows below the range down to row `to`: series (1, 2, 3; Phase 1, 2; dates), Ctrl copies. */
  const fill = async (g: Range, to: number, copy = false) => {
    const out: [number, string, string][] = [];
    for (let c = g.c0; c <= g.c1; c++) {
      const col = grid().cols[c];
      if (!col?.edit) continue;
      const src = Array.from({ length: g.r1 - g.r0 + 1 }, (_, i) => rows()[g.r0 + i]?.cells[c] ?? "");
      // Durations, links and resources copy; numbers, text, dates and IDs continue a series.
      const ser = ["Num", "Int", "Cost", "Pct", "Text", "Date", "Fin"].includes(col.kind) && !["preds", "succs", "res"].includes(col.key);
      for (let r = g.r1 + 1; r <= to; r++) { const target = rows()[r]; if (target && target.uid > 0) out.push([target.uid, col.key, series(src, r - g.r1, copy || !ser)]); }
    }
    if (out.length) await act({ Cells: { cells: out } });
    setRanges([{ ...g, r1: to }]);
  };

  const startEdit = (row: number, col: number, text?: string) => {
    const r = rows()[row], c = grid().cols[col];
    if (!c) return;
    if (row >= rows().length) { setEdit({ row, col, text: text ?? "" }); return; }
    if (!r || r.uid <= 0) return;
    if (!c.edit) return toast(formulaOf(c.key) ? `${c.name} is calculated by its column formula: ${formulaOf(c.key)}` : `${c.name} is calculated by Datum Plan.`);
    setEdit({ row, col, text: text ?? r.cells[col] });
  };
  /** Saves the edit; Ctrl+Enter puts the value into every selected cell. */
  const commit = async (move: [number, number] | null, all = false) => {
    const e = edit();
    if (!e) return;
    setEdit(null);
    const r = rows()[e.row], c = grid().cols[e.col];
    if (!r && e.text.trim()) {
      if (await act({ InsertRows: { at: grid().total, rows: [[[c.key === "name" ? "name" : "name", c.key === "name" ? e.text : "New task"], ...(c.key !== "name" && c.edit ? [[c.key, e.text]] : [])]] } })) await refresh();
    } else if (all) {
      const out: [number, string, string][] = [];
      ranges().forEach(g => { const n = norm(g); for (let k = n.r0; k <= n.r1; k++) for (let j = n.c0; j <= n.c1; j++) { const x = rows()[k], col = grid().cols[j]; if (x?.uid > 0 && col.edit) out.push([x.uid, col.key, e.text]); } });
      await act({ Cells: { cells: out } });
    } else if (r && c && e.text !== r.cells[e.col]) await act({ Cells: { cells: [[r.uid, c.key, e.text]] } });
    if (move) pick(e.row + move[0], e.col + move[1]);
    box.focus();
  };

  // ---------- clipboard: ranges as tab-separated text (works with spreadsheet programs) ----------
  let lastCopy = { text: "", rows: false };
  const rangeText = () => {
    const g = norm(ranges()[ranges().length - 1] ?? { r0: cur().row, c0: cur().col, r1: cur().row, c1: cur().col });
    const lines: string[] = [];
    for (let r = g.r0; r <= g.r1; r++) { const x = rows()[r]; if (x) lines.push(x.cells.slice(g.c0, g.c1 + 1).map(v => /[\t\n"]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v).join("\t")); }
    return lines.join("\n");
  };
  const onCopy = (e: ClipboardEvent, cut = false) => {
    if (edit()) return;
    e.preventDefault();
    const text = rangeText();
    e.clipboardData?.setData("text/plain", text);
    lastCopy = { text, rows: wholeRows() };
    const g = norm(ranges()[ranges().length - 1] ?? { r0: cur().row, c0: cur().col, r1: cur().row, c1: cur().col });
    clip.text = text; clip.styles = Array.from({ length: g.r1 - g.r0 + 1 }, (_, i) => Array.from({ length: g.c1 - g.c0 + 1 }, (_, j) => { const x = rows()[g.r0 + i], k = grid().cols[g.c0 + j]?.key ?? ""; return [x?.style?.["*"], x?.style?.[k]].filter(Boolean).join(";"); }));
    if (wholeRows()) { if (cut) C.cut(); else C.copy(); }
    else if (cut) clear();
  };
  const onPaste = async (e: ClipboardEvent) => {
    if (edit()) return;
    e.preventDefault();
    const text = e.clipboardData?.getData("text/plain") ?? "";
    if (lastCopy.rows && text === lastCopy.text) return C.paste();
    await pasteMatrix(parseTsv(text));
  };
  /** Paste Special: values, formats or both, optionally transposed (rows become columns). */
  const pasteSpecial = async (what: "all" | "values" | "formats", tr: boolean) => {
    const text = clip.text || (await navigator.clipboard?.readText().catch(() => "")) || "";
    if (!text) return toast("Copy some cells first.");
    let m = parseTsv(text), st = clip.text === text ? clip.styles : [];
    if (tr) { m = transpose(m); st = st.length ? transpose(st) : st; }
    if (what !== "formats") await pasteMatrix(m);
    if (what !== "values" && st.length) {
      const g = norm(ranges()[ranges().length - 1] ?? { r0: cur().row, c0: cur().col, r1: cur().row, c1: cur().col }), out: [number, string, string][] = [];
      st.forEach((line, i) => line.forEach((v, j) => { const x = rows()[g.r0 + i], k = grid().cols[g.c0 + j]?.key; if (x && x.uid > 0 && k) out.push([x.uid, k, v]); }));
      if (out.length) await act({ Style: { cells: out } });
    }
  };
  const pasteMatrix = async (m: string[][]) => {
    const g = norm(ranges()[ranges().length - 1] ?? { r0: cur().row, c0: cur().col, r1: cur().row, c1: cur().col });
    const out: [number, string, string][] = [];
    const extra: [string, string][][] = [];
    // One value into a larger selection fills it; otherwise the block lands at the active cell.
    const h = m.length === 1 && m[0].length === 1 ? g.r1 - g.r0 + 1 : m.length, w = m.length === 1 && m[0].length === 1 ? g.c1 - g.c0 + 1 : Math.max(...m.map(x => x.length));
    for (let i = 0; i < h; i++) {
      const r = rows()[g.r0 + i];
      const line: [string, string][] = [];
      for (let j = 0; j < w; j++) {
        const col = grid().cols[g.c0 + j];
        const v = m.length === 1 && m[0].length === 1 ? m[0][0] : m[i]?.[j] ?? "";
        if (!col || !col.edit) continue;
        if (r && r.uid > 0) out.push([r.uid, col.key, v]); else if (!r) line.push([col.key, v]);
      }
      if (!r && line.length) extra.push(line.some(x => x[0] === "name") ? line : [["name", "New task"], ...line]);
    }
    // One step for the whole paste, so one Undo takes it back.
    const ops: object[] = [...(out.length ? [{ Cells: { cells: out } }] : []), ...(extra.length ? [{ InsertRows: { at: grid().total, rows: extra } }] : [])];
    if (ops.length) await act(ops.length > 1 ? { Batch: ops } : ops[0]);
    setRanges([{ r0: g.r0, c0: g.c0, r1: g.r0 + h - 1, c1: g.c0 + w - 1 }]);
  };
  const clear = () => {
    const out: [number, string, string][] = [];
    ranges().forEach(g => { const n = norm(g); for (let k = n.r0; k <= n.r1; k++) for (let j = n.c0; j <= n.c1; j++) { const x = rows()[k], col = grid().cols[j]; if (x?.uid > 0 && col.edit && col.key !== "name") out.push([x.uid, col.key, ""]); } });
    if (out.length) act({ Cells: { cells: out } });
  };

  const key = (e: KeyboardEvent) => {
    if (edit() || rename() !== null) return;
    const { row, col } = cur();
    const ctrl = e.ctrlKey || e.metaKey, last = ranges()[ranges().length - 1];
    const at = e.shiftKey && last ? { row: last.r1, col: last.c1 } : { row, col };
    const go = (r: number, c: number) => { e.preventDefault(); pick(r, c, e.shiftKey); };
    const jump = (d: number) => (ctrl ? (d > 0 ? rows().length - 1 : 0) : at.row + d);
    switch (e.key) {
      case "ArrowDown": return go(jump(1), at.col);
      case "ArrowUp": return go(jump(-1), at.col);
      case "ArrowLeft": if (e.altKey && e.shiftKey) { e.preventDefault(); return C.outdent(); } return go(at.row, ctrl ? 0 : at.col - 1);
      case "ArrowRight": if (e.altKey && e.shiftKey) { e.preventDefault(); return C.indent(); } return go(at.row, ctrl ? ncols() - 1 : at.col + 1);
      case "PageDown": return go(at.row + Math.floor(box.clientHeight / RH), at.col);
      case "PageUp": return go(at.row - Math.floor(box.clientHeight / RH), at.col);
      case "Home": return go(ctrl ? 0 : at.row, 0);
      case "End": return go(ctrl ? rows().length - 1 : at.row, ncols() - 1);
      case "Tab": e.preventDefault(); return pick(row, col + (e.shiftKey ? -1 : 1));
      case "Enter": e.preventDefault(); return e.shiftKey ? pick(row - 1, col) : startEdit(row, col);
      case "F2": e.preventDefault(); return e.shiftKey ? C.info() : ctrl ? C.link() : startEdit(row, col);
      case "Insert": e.preventDefault(); return C.newTask();
      case "Delete": e.preventDefault(); return wholeRows() ? C.del() : clear();
      case "Escape": return setRanges([{ r0: row, c0: col, r1: row, c1: col }]);
    }
    const k = e.key.toLowerCase();
    if (ctrl && e.altKey && k === "v") { e.preventDefault(); return D.pasteSpecial(); }
    if (ctrl && !e.shiftKey && (k === "b" || k === "i" || k === "u")) { e.preventDefault(); return F.toggle(k); }
    if (ctrl && k === "a") { e.preventDefault(); setRanges([{ r0: 0, c0: 0, r1: rows().length - 1, c1: ncols() - 1 }]); return; }
    if (ctrl && k === "d" && last) { e.preventDefault(); const n = norm(last); return n.r1 > n.r0 ? fill({ ...n, r1: n.r0 }, n.r1, true) : undefined; }
    if (ctrl && (k === "-" || k === "delete")) { e.preventDefault(); return C.del(); }
    if (ctrl && (k === "+" || k === "=")) { e.preventDefault(); return C.newTask(); }
    if (!ctrl && !e.altKey && e.key.length === 1) { e.preventDefault(); startEdit(row, col, e.key); }
  };

  const rowMenu = (e: MouseEvent) => {
    const [r, c] = cellAt(e);
    if (rows()[r]?.sub) { const x = rows()[r]; return void import("./plans").then(m => m.subRowMenu(e, x)); }
    if (!inRange(r, Math.max(0, c))) pick(r, Math.max(0, c));
    const k = cur().row;
    openMenu(e, [
      { label: "Cut", icon: "cut", run: () => document.execCommand("cut"), keys: "Ctrl+X" }, { label: "Copy", icon: "copy", run: () => document.execCommand("copy"), keys: "Ctrl+C" }, { label: "Paste", icon: "paste", run: () => toast("Press Ctrl+V to paste."), keys: "Ctrl+V" }, { label: "Paste Special…", icon: "paste", run: () => D.pasteSpecial(), keys: "Ctrl+Alt+V" },
      { label: "Clear Contents", icon: "clear", run: clear, keys: "Delete" }, "-",
      { label: "Rename Task", icon: "rename", run: () => { const nc = grid().cols.findIndex(x => x.key === "name"); if (nc >= 0) startEdit(k, nc); }, keys: "F2" },
      { label: "Insert Task", icon: "task", run: () => C.newTask(), keys: "Insert" }, { label: "Insert Milestone", icon: "ms", run: () => C.newTask(true) }, { label: "Delete Task", icon: "del", run: C.del, keys: "Ctrl+−" }, "-",
      { label: "Indent", icon: "indent", run: C.indent, keys: "Alt+Shift+→" }, { label: "Outdent", icon: "outdent", run: C.outdent, keys: "Alt+Shift+←" }, { label: "Link Tasks", icon: "link", run: C.link, keys: "Ctrl+F2" }, { label: "Unlink Tasks", icon: "unlink", run: C.unlink }, "-",
      { label: "Task Information…", icon: "info", run: C.info, keys: "Shift+F2" }, { label: "Notes…", icon: "detail", run: C.notes }, { label: "Link to Plan…", icon: "link", run: () => import("./plans").then(m => m.linkSelected()) }, ...(rows()[k]?.ext?.length || rows()[k]?.ext_out?.length ? [{ label: "Linked Tasks…", icon: "link", run: () => import("./plans").then(m => m.linkMenu(e, rows()[k].uid)) }] : []), { label: "Inspect…", icon: "find", run: () => D.inspect(rows()[k]?.uid) }, "-",
      { label: "Mark on Track", icon: "track", run: C.onTrack }, { label: "Progress", icon: "pct", sub: [0, 25, 50, 75, 100].map(v => ({ label: `${v}% Complete`, run: () => C.pct(v) })) },
      { label: "Split Task…", icon: "split", run: C.split }, { label: "Inactivate / Activate", icon: "inact", run: C.inactive }, { label: "Manually Schedule", icon: "manual", run: () => C.mode(true) }, { label: "Auto Schedule", icon: "auto", run: () => C.mode(false) },
      { label: "Global Change…", icon: "change", run: () => D.globalChange(sel()) }, { label: "Add to Timeline", icon: "tl", run: C.timeline }, { label: "Scroll to Task", icon: "scroll", run: C.scrollTo },
    ]);
  };

  const headMenu = (e: MouseEvent, k: number) => {
    const c = grid().cols[k];
    const ins = (key: string) => { const v = [...cols()]; v.splice(k, 0, [key, 90]); saveCols(v); };
    const f = formulaOf(c.key);
    openMenu(e, [
      { label: "Rename Column", icon: "rename", run: () => setRename(k) }, { label: "Insert Column", icon: "cols", sub: fieldMenu(ins) }, { label: "New Formula Column…", icon: "fx", run: () => D.formulaColumn(undefined, k + 1) },
      ...(c.key.startsWith("c:") ? [{ label: f ? "Edit Formula…" : "Add Formula…", icon: "fx", run: () => D.formulaColumn(c.key.slice(2), k) }] : []),
      { label: "Hide Column", icon: "hide", run: () => hideCol(c.key), off: cols().length < 2 },
      { label: "Unhide Columns", icon: "cols", off: !hiddenCols().length, sub: [...hiddenCols().map(h => ({ label: fieldName(h), run: () => unhideCol(h) })), "-", { label: "Unhide All", run: () => hiddenCols().forEach(unhideCol) }] },
      { label: "Delete Column", icon: "del", run: () => D.deleteColumn(c.key), off: cols().length < 2 },
      { label: "Columns…", icon: "table", run: () => D.columnsDialog() },
      { label: "Field Settings…", icon: "table", run: () => D.fieldSettings(k) }, { label: "Custom Fields…", icon: "fields", run: () => D.customFields(c?.key.startsWith("c:") ? c.key.slice(2) : undefined) }, "-",
      { label: "Sort A to Z", icon: "az", run: () => setSort([[c.key, false]]), on: sort()[0]?.[0] === c.key && !sort()[0][1] }, { label: "Sort Z to A", icon: "za", run: () => setSort([[c.key, true]]), on: sort()[0]?.[0] === c.key && sort()[0][1] },
      { label: "No Sort", icon: "sort", run: () => setSort([]) }, { label: "Group by This Field", icon: "group", run: () => setGroup([c.key]) }, { label: "Filter…", icon: "filter", run: () => autoFilter(e, k) }, "-",
      { label: "Conditional Formatting…", icon: "style", run: () => D.condFormat(c.key) }, { label: "Best Fit", icon: "fit", run: () => bestFit(k) }, { label: `Table: ${table()}`, icon: "table", sub: [{ label: "Tables…", run: () => D.tablesDialog() }] },
    ]);
  };
  const bestFit = (k: number) => { const c = grid().cols[k], v = [...cols()]; v[k] = [v[k][0], Math.min(420, Math.max(48, Math.max(c.name.length, ...rows().slice(0, 800).map(r => (r.cells[k]?.length ?? 0) + (k === frozen() ? r.lvl * 2.3 + 2 : 0))) * 6.6 + 22))]; saveCols(v); };
  /** AutoFilter: sort, and tick the values to show. */
  const autoFilter = (e: MouseEvent, k: number) => {
    const c = grid().cols[k];
    const vals = [...new Set(rows().filter(r => r.kind === "task" || r.kind === "summary").map(r => r.cells[k]))].sort().slice(0, 300);
    const on = filter().rules.filter(r => r.field === c.key && r.test === "equals").map(r => r.value);
    const apply = (keep: string[]) => setFilter(keep.length === vals.length || !keep.length ? ALL : { name: `${c.name} filter`, rules: keep.map(v => ({ field: c.key, test: "equals", value: v, value2: "", or: true })), summaries: true, highlight: false });
    openMenu(e, [
      { label: "Sort A to Z", icon: "az", run: () => setSort([[c.key, false]]) }, { label: "Sort Z to A", icon: "za", run: () => setSort([[c.key, true]]) }, "-",
      { label: "(Select All)", run: () => apply([]), on: !on.length },
      ...vals.map(v => ({ label: v || "(Blanks)", on: on.includes(v), run: () => apply(on.includes(v) ? on.filter(x => x !== v) : [...on, v]) })),
      "-", { label: "Custom Filter…", icon: "filter", run: () => D.filterEditor(c.key) }, { label: "Clear Filter", icon: "clear", run: () => setFilter(ALL), off: !filter().rules.length },
    ]);
  };
  const resize = (e: MouseEvent, k: number) => {
    e.preventDefault(); e.stopPropagation();
    const x0 = e.clientX, w0 = widths()[k], v = [...cols()];
    const mv = (m: MouseEvent) => { v[k] = [v[k][0], Math.max(24, w0 + m.clientX - x0)]; setCols([...v]); };
    const up = () => { removeEventListener("mousemove", mv); removeEventListener("mouseup", up); saveCols(v); };
    addEventListener("mousemove", mv); addEventListener("mouseup", up);
  };
  const [drag, setDrag] = createSignal<number | null>(null);
  const colSelect = (e: MouseEvent, k: number) => {
    if ((e.target as HTMLElement).closest(".dd,.rz,input")) return;
    const last = ranges()[ranges().length - 1];
    if (e.shiftKey && last) setRanges([...ranges().slice(0, -1), { r0: 0, c0: last.c0, r1: rows().length - 1, c1: k }]);
    else setRanges([...(e.ctrlKey ? ranges() : []), { r0: 0, c0: k, r1: rows().length - 1, c1: k }]);
    setCur({ row: cur().row, col: k });
  };
  const sticky = (k: number): JSX.CSSProperties => (k <= frozen() ? { position: "sticky", left: `${lefts()[k]}px`, "z-index": 1 } : {});

  const cell = (r: Row, c: Col, k: number): JSX.Element => {
    const t = r.cells[k] ?? "";
    if (c.key === "mode") return <span class="mode" title={t}>{r.kind === "group" || r.kind === "project" ? "" : t.startsWith("Manual") ? <Icon n="manual" /> : <Icon n="auto" />}</span>;
    const icon = r.icons?.find(x => x[0] === k)?.[1];
    if (c.key === "name") return <>
      <span style={{ width: `${r.lvl * 16}px`, display: "inline-block" }} />
      {r.kids ? <b class="tg" onMouseDown={e => { e.stopPropagation(); if (r.uid > 0) C.toggle(r.uid); else if (r.kind === "subplan") toggleSub(r.sub!); }}>{r.open ? "◢" : "▷"}</b> : <span class="tg" />}
      {r.kind === "subplan" && <i class="subico" title="A subplan of this project (read-only here; double-click to open it)">◇</i>}{t}
    </>;
    if (icon) return <span class="ico" title={t}>{icon}</span>;
    if (c.key === "pct" && r.kind !== "group") return <span class="pctc"><i style={{ width: `${parseFloat(t) || 0}%` }} />{t}</span>;
    if (t.startsWith("#")) return <span class="err" title="Formula error">{t}</span>;
    return t;
  };
  const fillHandle = createMemo(() => {
    const g = ranges()[ranges().length - 1];
    if (!g || edit()) return null;
    const n = norm(g);
    return { left: lefts()[n.c1] + widths()[n.c1] - 4, top: (fillTo() ?? n.r1) * RH + RH - 4 + 40, r0: n.r0, c0: n.c0, c1: n.c1 };
  });

  return (
    <div class="grid" style={{ width: p.width ? `${p.width}px` : undefined }}>
      <div class="gscroll" ref={box} tabindex="0" onFocus={() => F.setTarget(fmtTarget)} onKeyDown={key} onMouseDown={down} onContextMenu={rowMenu}
        onCopy={e => onCopy(e)} onCut={e => onCopy(e, true)} onPaste={onPaste}
        onDblClick={e => { const [r, c] = cellAt(e), x = rows()[r]; if (x?.sub) return void import("./plans").then(m => m.goToLink(x.sub!, x.suid ?? 0)); if (c < 0) C.info(); else startEdit(r, c); }}
        onScroll={() => { setScrollTop(box.scrollTop); if (echo) { echo = false; return; } document.dispatchEvent(new CustomEvent("chart-scroll", { detail: box.scrollTop })); }}>
        <div class="gsheet" style={{ width: `${total() + 160}px`, height: `${(rows().length + 3) * RH + 40}px` }}>
          <div class="ghead" onMouseDown={e => e.stopPropagation()} onContextMenu={e => e.stopPropagation()} onDblClick={e => e.stopPropagation()}>
            <div class="hc idc" style={{ width: `${IDW}px`, position: "sticky", left: "0", "z-index": 2 }} title="Select all" onClick={() => setRanges([{ r0: 0, c0: 0, r1: rows().length - 1, c1: ncols() - 1 }])}>◢</div>
            <For each={grid().cols}>{(c, k) => (
              <div class="hc" classList={{ r: right(c), drop: drag() !== null && drag() !== k(), fx: !!formulaOf(c.key), on: ranges().some(g => k() >= Math.min(g.c0, g.c1) && k() <= Math.max(g.c0, g.c1)), filt: filter().rules.some(r => r.field === c.key) }}
                style={{ width: `${widths()[k()]}px`, ...sticky(k()), ...(k() <= frozen() ? { "z-index": 2 } : {}) }} title={formulaOf(c.key) ? `${c.name} = ${formulaOf(c.key).replace(/^=/, "")}` : c.name}
                draggable={rename() !== k()} onDragStart={() => setDrag(k())} onDragEnd={() => setDrag(null)} onDragOver={e => e.preventDefault()}
                onDrop={() => { const f = drag(); if (f === null || f === k()) return; const v = [...cols()]; const [m] = v.splice(f, 1); v.splice(k(), 0, m); saveCols(v); setDrag(null); }}
                onClick={e => colSelect(e, k())} onContextMenu={e => { e.preventDefault(); headMenu(e, k()); }} onDblClick={() => setRename(k())}>
                <Show when={rename() === k()} fallback={<span>{c.key === "mode" && widths()[k()] < 50 ? "" : c.name}</span>}>
                  <input class="hrename" value={c.name} ref={el => setTimeout(() => { el.focus(); el.select(); })}
                    onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") e.currentTarget.blur(); if (e.key === "Escape") setRename(null); }}
                    onBlur={e => { const v = e.currentTarget.value.trim(); setRename(null); if (v !== c.name) act({ Title: { key: c.key, title: v } }); }} />
                </Show>
                <button class="dd" title="Filter and sort" onClick={e => { e.stopPropagation(); autoFilter(e, k()); }}><Icon n={filter().rules.some(r => r.field === c.key) ? "filter" : "chev"} /></button>
                <i class="rz" onMouseDown={e => resize(e, k())} onDblClick={e => { e.stopPropagation(); bestFit(k()); }} />
              </div>
            )}</For>
            <div class="hc add" onClick={e => openMenu(e, fieldMenu(key => saveCols([...cols(), [key, 90]])))}>Add New Column ▾</div>
          </div>
          <div class="gbody">
            <For each={visible()}>{([r, k]) => (
              <div class="row" style={{ top: `${k * RH}px`, width: `${total()}px` }}
                classList={{ sum: r.kind === "summary" || r.kind === "project" || r.kind === "subplan", subp: r.kind === "subplan", subc: !!r.sub && r.kind !== "subplan", grp: r.kind === "group", sel: selSet().has(r.uid), off: r.off, late: r.late, hl: r.hl, proj: r.kind === "project" }}>
                <div class="c idc" style={{ width: `${IDW}px`, position: "sticky", left: "0", "z-index": 1 }} title={r.late ? "Late" : ""}>{r.uid > 0 ? r.id : ""}{r.late && <i class="ind late">!</i>}{r.manual && <i class="ind">✎</i>}{r.uid > 0 && r.ext?.length ? <i class="ind xin" classList={{ moved: r.ext_moved }} title={`Waits for a task in another plan:\n${r.ext.join("\n")}${r.ext_moved ? "\n\nThat plan has moved since: Project › Update Links." : ""}\n(click: go to it, edit or update)`} onClick={e => { e.stopPropagation(); import("./plans").then(m => m.linkMenu(e, r.uid)); }}>{r.ext_moved ? "⇠!" : "⇠"}</i> : null}{r.uid > 0 && r.ext_out?.length ? <i class="ind xout" title={`Another plan waits for this task:\n${r.ext_out.join("\n")}\n(click: go to it)`} onClick={e => { e.stopPropagation(); import("./plans").then(m => m.linkMenu(e, r.uid)); }}>⇢</i> : null}</div>
                <For each={grid().cols}>{(c, j) => (
                  <div class="c" classList={{ r: right(c), nm: c.key === "name", ro: !c.edit, in: inRange(k, j()), at: cur().row === k && cur().col === j(), fz: j() === frozen(), fx: !!formulaOf(c.key) }}
                    style={{ width: `${widths()[j()]}px`, ...sticky(j()), ...(r.style ? { ...F.css(r.style["*"]), ...F.css(r.style[c.key]) } : {}), ...(D.cfRules().length ? D.cfStyle(c.key, r.cells[j()] ?? "") : {}) }}>
                    {cell(r, c, j())}
                  </div>
                )}</For>
              </div>
            )}</For>
            <div class="row blank" style={{ top: `${rows().length * RH}px`, width: `${total()}px` }}>
              <div class="c idc" style={{ width: `${IDW}px`, position: "sticky", left: "0", "z-index": 1 }}>＋</div>
              <For each={grid().cols}>{(c, j) => <div class="c" classList={{ at: cur().row === rows().length && cur().col === j() }} style={{ width: `${widths()[j()]}px`, ...sticky(j()) }}>{c.key === "name" ? <span class="ph">Type a new task…</span> : ""}</div>}</For>
            </div>
            <Show when={fillHandle()}>{h => <>
              <Show when={fillTo() !== null}><div class="fillzone" style={{ left: `${lefts()[h().c0]}px`, top: `${h().r0 * RH}px`, width: `${lefts()[h().c1] + widths()[h().c1] - lefts()[h().c0]}px`, height: `${(fillTo()! - h().r0 + 1) * RH}px` }} /></Show>
              <i class="fill" title="Drag to fill down" style={{ left: `${h().left}px`, top: `${h().top - 40}px` }} onMouseDown={fillDrag} />
            </>}</Show>
            <Show when={!edit() && (() => { const c = grid().cols[cur().col], r = rows()[cur().row]; return c && r && r.uid > 0 && c.edit && ["Date", "Fin"].includes(c.kind) ? { c, r } : null; })()}>{x => (
              <div class="gdate" style={{ left: `${(cur().col <= frozen() ? lefts()[cur().col] + box.scrollLeft : lefts()[cur().col]) + widths()[cur().col] - 20}px`, top: `${cur().row * RH + 1}px` }}>
                <DateBtn value={x().r.cells[cur().col] ?? ""} title="Pick a date (or type it)" set={v => act({ Cells: { cells: [[x().r.uid, x().c.key, v]] } })} />
              </div>)}</Show>
            <Show when={edit()}>{e => {
              const c = grid().cols[e().col];
              const opts = c && choices(c.key);
              const style = { left: `${e().col <= frozen() ? lefts()[e().col] + box.scrollLeft : lefts()[e().col]}px`, top: `${e().row * RH}px`, width: `${Math.max(widths()[e().col], c?.key === "name" ? 220 : 96)}px` };
              const keys = (x: KeyboardEvent) => {
                x.stopPropagation();
                if (x.key === "Enter") { x.preventDefault(); commit(x.shiftKey ? [-1, 0] : [1, 0], x.ctrlKey); }
                else if (x.key === "Tab") { x.preventDefault(); commit([0, x.shiftKey ? -1 : 1]); }
                else if (x.key === "Escape") { setEdit(null); box.focus(); }
              };
              return opts && !e().text.match(/^.$/)
                ? <select class="edit" style={style} value={e().text} ref={el => setTimeout(() => el.focus())} onMouseDown={x => x.stopPropagation()} onChange={x => { setEdit({ ...e(), text: x.currentTarget.value }); commit(null); }} onKeyDown={keys} onBlur={() => setEdit(null)}>
                    <For each={opts}>{o => <option value={o}>{o}</option>}</For>
                  </select>
                : <input class="edit" style={style} value={e().text} list={c?.key.startsWith("c:") && P().fields.find(f => f.key === c.key.slice(2))?.lookup.length ? "lookup" : c?.key === "res" ? "resnames" : undefined}
                    ref={el => setTimeout(() => { el.focus(); if (e().text.length > 1) el.select(); else el.setSelectionRange(1, 1); })} onMouseDown={x => x.stopPropagation()}
                    onInput={x => setEdit({ ...e(), text: x.currentTarget.value })} onKeyDown={keys} onBlur={() => commit(null)} />;
            }}</Show>
          </div>
        </div>
      </div>
      <datalist id="resnames"><For each={P().resources}>{r => <option value={r.name} />}</For></datalist>
      <datalist id="lookup"><For each={(() => { const c = grid().cols[cur().col]; return c?.key.startsWith("c:") ? P().fields.find(f => f.key === c.key.slice(2))?.lookup ?? [] : []; })()}>{l => <option value={l.value}>{l.desc}</option>}</For></datalist>
    </div>
  );
};

/** The formula bar: the active cell's column, its value, and the column formula if it has one. */
export const FormulaBar = () => {
  const c = () => grid().cols[cur().col];
  const r = () => grid().rows[cur().row];
  const f = () => (c() ? formulaOf(c()!.key) : "");
  const [text, setText] = createSignal("");
  createEffect(() => setText(f() || (r()?.cells[cur().col] ?? "")));
  const save = () => {
    const col = c(), row = r();
    if (!col || !row || row.uid <= 0) return;
    if (f()) return D.formulaColumn(col.key.slice(2));
    if (col.edit && text() !== row.cells[cur().col]) act({ Cells: { cells: [[row.uid, col.key, text()]] } });
  };
  return (
    <div class="fbar">
      <span class="fname">{c() ? `${r()?.uid ? r()!.id : ""} · ${c()!.name}` : ""}</span>
      <button class="fx" title={f() ? "Edit the column formula" : "New formula column"} onClick={() => (f() ? D.formulaColumn(c()!.key.slice(2)) : D.formulaColumn(undefined, cur().col + 1))}>fx</button>
      <input value={text()} readOnly={!!f() || !c()?.edit} onInput={e => setText(e.currentTarget.value)} placeholder={c()?.edit ? "" : "Calculated"}
        onKeyDown={e => { if (e.key === "Enter") { save(); (document.querySelector(".gscroll") as HTMLElement | null)?.focus(); } if (e.key === "Escape") setText(r()?.cells[cur().col] ?? ""); }} onClick={() => f() && D.formulaColumn(c()!.key.slice(2))} />
      <Show when={agg()}><span class="agg">{agg()}</span></Show>
    </div>
  );
};
export { call };
