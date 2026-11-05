// Spreadsheet dialogs for the Sheets: Format Cells (number, alignment, font, border, fill), Insert and
// Delete cells, notes, the AutoFilter's column menu, Custom Sort, data validation, links, defined names
// and Go To. They only collect choices; sheets.tsx turns them into edits (ops), so Undo works as for any edit.
import { For, Show, createMemo, createSignal } from "solid-js";
import { P, openDialog, closeDialog } from "./store";
import { Dialog, Row, Tabs } from "./ui";
import * as F from "./fmt";
import { colorPalette, STANDARD } from "./colors";
import type { ColFilter, DefName, Validation } from "./api";

type B4 = { r0: number; c0: number; r1: number; c1: number };
/** One cell's style with a border kind applied, as a spreadsheet draws it on a selection (inner lines once). */
export const edges = (kind: string, [kd, col]: [string, string], b: B4, r: number, c: number, st: F.St): F.St => {
  const o = { ...st }, top = r === b.r0, bot = r === b.r1, lft = c === b.c0, rgt = c === b.c1, ln = (k = kd) => `${k} ${col}`;
  const set = (k: F.Side, on: boolean, v = ln()) => { if (on) o[k] = v; };
  switch (kind) {
    case "none": F.SIDES.forEach(k => delete o[k]); break;
    case "all": set("bb", true); set("br", true); set("bt", top); set("bl", lft); break;
    case "inside": set("bb", !bot); set("br", !rgt); break;
    case "outside": set("bt", top); set("bb", bot); set("bl", lft); set("br", rgt); break;
    case "thickbox": set("bt", top, ln("thick")); set("bb", bot, ln("thick")); set("bl", lft, ln("thick")); set("br", rgt, ln("thick")); break;
    case "top": set("bt", top); break; case "bottom": set("bb", bot); break; case "left": set("bl", lft); break; case "right": set("br", rgt); break;
    case "topbottom": set("bt", top); set("bb", bot); break;
    case "dblbottom": set("bb", bot, ln("dbl")); break; case "thickbottom": set("bb", bot, ln("thick")); break;
    case "topthickbottom": set("bt", top); set("bb", bot, ln("thick")); break;
  }
  return o;
};

/** What Format Cells returns: the style keys changed (with their values), border kinds to draw with a pen, and the number format (null: unchanged). */
export type FmtChoice = { st: F.St; touched: (keyof F.St)[]; borders: string[]; pen: [string, string]; fmt: string | null };
const CATS = ["General", "Number", "Currency", "Percentage", "Date", "Time", "Text", "Country with flag", "Custom"];
const DATES: [string, string][] = [["date", "As the plan shows dates"], ["dd/mm/yyyy", "30/10/2026"], ["d mmmm yyyy", "30 October 2026"], ["dd-mmm-yy", "30-Oct-26"], ["mmm-yy", "Oct-26"], ["yyyy-mm-dd", "2026-10-30"], ["dddd, d mmmm yyyy", "Friday, 30 October 2026"]];
const TIMES: [string, string][] = [["h:mm", "13:30"], ["h:mm:ss", "13:30:55"], ["h:mm AM/PM", "1:30 PM"], ["dd/mm/yyyy h:mm", "30/10/2026 13:30"]];
/** The category of a format code, and its decimals. */
const catOf = (f: string): [string, number] => {
  const d = (f.match(/\.(0+)/)?.[1].length) ?? 0;
  if (!f) return ["General", 2];
  if (f === "money" || /^"[^"]*"#/.test(f)) return ["Currency", f === "money" ? P().currency.decimals : d];
  if (f === "@") return ["Text", 0]; if (f === "flag") return ["Country with flag", 0];
  if (f.includes("%")) return ["Percentage", d];
  if (/h:mm/.test(f)) return ["Time", 0];
  if (f === "date" || /[dy]/.test(f) || /^m+[-/ ]/.test(f)) return ["Date", 0];
  if (/^#?,?#*0(\.0+)?$|^#,##0(\.0+)?$/.test(f)) return ["Number", d];
  return ["Custom", 0];
};
const LINES: [string, string][] = [["thin", "Thin"], ["med", "Medium"], ["thick", "Thick"], ["dbl", "Double"], ["dash", "Dashed"], ["dot", "Dotted"]];
const FILLS = ["#FFFFFF", "#F2F2F2", "#D9D9D9", "#DDEBF7", "#BDD7EE", "#E2EFDA", "#C6EFCE", "#FFF2CC", "#FFEB9C", "#FCE4D6", "#FFC7CE", "#E4DFEC", ...STANDARD];
const FONTS = ["Aptos", "Calibri", "Arial", "Segoe UI", "Tahoma", "Verdana", "Times New Roman", "Cambria", "Georgia", "Garamond", "Consolas", "IBM Plex Sans"];

/** Format Cells (Ctrl+1): every look of the selected cells in one place, with the ribbon's style strings. */
export const formatCells = (start: { st: F.St; fmt: string; tab?: string }, ok: (r: FmtChoice) => void) => openDialog(() => {
  const [tab, setTab] = createSignal(start.tab ?? "num");
  const [st, setSt] = createSignal<F.St>({ ...start.st }), touched = new Set<keyof F.St>();
  const put = <K extends keyof F.St>(k: K, v: F.St[K]) => { touched.add(k); setSt({ ...st(), [k]: v }); };
  const [c0, d0] = catOf(start.fmt);
  const [cat, setCat] = createSignal(c0), [dec, setDec] = createSignal(d0), [sep, setSep] = createSignal(!start.fmt || start.fmt.includes(","));
  const [of100, setOf100] = createSignal(start.fmt.includes('"%"')), [pick, setPick] = createSignal(start.fmt), [custom, setCustom] = createSignal(start.fmt);
  const [borders, setBorders] = createSignal<string[]>([]), [pen, setPen] = createSignal<[string, string]>(["thin", "#000000"]);
  const zeros = () => (dec() > 0 ? "." + "0".repeat(dec()) : "");
  const code = createMemo(() => {
    switch (cat()) {
      case "General": return ""; case "Text": return "@"; case "Country with flag": return "flag"; case "Custom": return custom().trim();
      case "Number": return `${sep() ? "#,##0" : "0"}${zeros()}`;
      case "Currency": return dec() === P().currency.decimals ? "money" : `"${P().currency.symbol}"#,##0${zeros()}`;
      case "Percentage": return of100() ? `0${zeros()}"%"` : `0${zeros()}%`;
      case "Date": return DATES.some(d => d[0] === pick()) ? pick() : "date";
      case "Time": return TIMES.some(d => d[0] === pick()) ? pick() : "h:mm";
    }
    return "";
  });
  const sides = createMemo(() => { let o: F.St = { ...start.st }; for (const k of borders()) o = edges(k, pen(), { r0: 0, c0: 0, r1: 0, c1: 0 }, 0, 0, o); return o; });
  const BB = (p: { k: string; l: string }) => <button class="btn sm" onClick={() => setBorders(p.k === "none" ? ["none"] : [...borders(), p.k])}>{p.l}</button>;
  return <Dialog title="Format Cells" okText="OK" help="formatcells" ok={() => ok({ st: st(), touched: [...touched], borders: borders(), pen: pen(), fmt: code() === start.fmt ? null : code() })}>
    <Tabs tabs={[["num", "Number"], ["align", "Alignment"], ["font", "Font"], ["border", "Border"], ["fill", "Fill"]]} on={tab()} set={setTab} />
    <div class="fcbody">
      <Show when={tab() === "num"}><div class="fcnum">
        <select size="9" class="fccat" onChange={e => { setCat(e.currentTarget.value); setPick(""); }}><For each={CATS}>{c => <option selected={c === cat()}>{c}</option>}</For></select>
        <div class="form">
          <Show when={["Number", "Currency", "Percentage"].includes(cat())}><Row label="Decimal places"><input type="number" min="0" max="10" value={dec()} onInput={e => setDec(Math.max(0, Math.min(10, +e.currentTarget.value || 0)))} /></Row></Show>
          <Show when={cat() === "Number"}><Row label=""><label class="chk"><input type="checkbox" checked={sep()} onChange={e => setSep(e.currentTarget.checked)} />Use 1,000 separator</label></Row></Show>
          <Show when={cat() === "Percentage"}><Row label="The values are"><select onChange={e => setOf100(e.currentTarget.value === "100")}><option value="1" selected={!of100()}>Parts of 1 (0.28 shows 28%)</option><option value="100" selected={of100()}>Out of 100 (28 shows 28%)</option></select></Row></Show>
          <Show when={cat() === "Date" || cat() === "Time"}><Row label="Type"><select size="7" onChange={e => setPick(e.currentTarget.value)}><For each={cat() === "Date" ? DATES : TIMES}>{([v, l]) => <option value={v} selected={v === code()}>{l}</option>}</For></select></Row></Show>
          <Show when={cat() === "Custom"}><Row label="Format code"><input value={custom()} placeholder="#,##0.0;[Red]-#,##0.0" onInput={e => setCustom(e.currentTarget.value)} /></Row></Show>
          <Row label="Code"><code class="fccode">{code() || "General"}</code></Row>
        </div>
      </div><p class="muted">The format changes how the value shows, never the value itself.</p></Show>
      <Show when={tab() === "align"}><div class="form">
        <Row label="Horizontal"><select onChange={e => put("a", e.currentTarget.value || undefined)}><For each={[["", "General"], ["l", "Left"], ["c", "Center"], ["r", "Right"]]}>{([v, l]) => <option value={v} selected={(st().a ?? "") === v}>{l}</option>}</For></select></Row>
        <Row label="Vertical"><select onChange={e => put("v", e.currentTarget.value || undefined)}><For each={[["t", "Top"], ["", "Middle"], ["b", "Bottom"]]}>{([v, l]) => <option value={v} selected={(st().v ?? "") === v}>{l}</option>}</For></select></Row>
        <Row label="Indent"><input type="number" min="0" max="15" value={st().x ?? 0} onInput={e => put("x", Math.max(0, Math.min(15, +e.currentTarget.value || 0)) || undefined)} /></Row>
        <Row label="Text control"><label class="chk"><input type="checkbox" checked={!!st().w} onChange={e => put("w", e.currentTarget.checked || undefined)} />Wrap text</label></Row>
      </div></Show>
      <Show when={tab() === "font"}><div class="form">
        <Row label="Font"><select onChange={e => put("n", e.currentTarget.value || undefined)}><option value="">Default font</option><For each={FONTS}>{f => <option selected={st().n === f} style={{ "font-family": `'${f}'` }}>{f}</option>}</For></select></Row>
        <Row label="Size"><input type="number" min="6" max="96" value={st().z ?? 12} onInput={e => put("z", +e.currentTarget.value === 12 ? undefined : Math.max(6, Math.min(96, +e.currentTarget.value || 12)))} /></Row>
        <Row label="Style"><span><For each={[["b", "Bold"], ["i", "Italic"], ["u", "Underline"], ["s", "Strikethrough"]] as ["b" | "i" | "u" | "s", string][]}>{([k, l]) => <label class="chk"><input type="checkbox" checked={!!st()[k]} onChange={e => put(k, e.currentTarget.checked || undefined)} />{l}</label>}</For></span></Row>
        <Row label="Color"><button class="btn sm" onClick={e => colorPalette(e, st().c, v => put("c", v || undefined), "Automatic")}><i class="fcsw" style={{ background: st().c || "#000" }} /> {st().c || "Automatic"}</button></Row>
      </div></Show>
      <Show when={tab() === "border"}><div class="form">
        <Row label="Presets"><span class="row2"><BB k="none" l="None" /><BB k="outside" l="Outline" /><BB k="inside" l="Inside" /><BB k="all" l="All" /></span></Row>
        <Row label="Sides"><span class="row2"><BB k="top" l="Top" /><BB k="bottom" l="Bottom" /><BB k="left" l="Left" /><BB k="right" l="Right" /></span></Row>
        <Row label="Line"><select onChange={e => setPen([e.currentTarget.value, pen()[1]])}><For each={LINES}>{([v, l]) => <option value={v} selected={pen()[0] === v}>{l}</option>}</For></select></Row>
        <Row label="Color"><button class="btn sm" onClick={e => colorPalette(e, pen()[1], v => setPen([pen()[0], v || "#000000"]), "Automatic")}><i class="fcsw" style={{ background: pen()[1] }} /> {pen()[1]}</button></Row>
        <Row label="Preview"><div class="fcprev" style={F.css(F.ser(sides()))}>Text</div></Row>
      </div><p class="muted">Presets act on the whole selection: Outline draws round it, Inside between its cells.</p></Show>
      <Show when={tab() === "fill"}><div class="form">
        <Row label="Background"><div class="fcfills"><button class="btn sm" onClick={() => put("f", undefined)}>No Color</button><For each={FILLS}>{c => <button class="fcsw big" classList={{ on: st().f === c }} title={c} style={{ background: c }} onClick={() => put("f", c)} />}</For>
          <button class="btn sm" onClick={e => colorPalette(e, st().f, v => put("f", v || undefined))}>More Colors…</button></div></Row>
        <Row label="Sample"><div class="fcprev" style={F.css(F.ser(st()))}>AaBbCc 123</div></Row>
      </div></Show>
    </div>
  </Dialog>;
});

/** Insert… or Delete…: move the other cells, or whole rows or columns. */
export const shiftDialog = (del: boolean, run: (how: "down" | "right" | "rows" | "cols") => void) => openDialog(() => {
  type How = "down" | "right" | "rows" | "cols";
  const [h, setH] = createSignal<How>("down");
  const opts: [How, string][] = [["right", del ? "Shift cells left" : "Shift cells right"], ["down", del ? "Shift cells up" : "Shift cells down"], ["rows", "Entire row"], ["cols", "Entire column"]];
  return <Dialog title={del ? "Delete" : "Insert"} okText="OK" help="sheetcells" ok={() => run(h())}>
    <div class="radios"><For each={opts}>{([v, l]) => <label class="chk"><input type="radio" name="shift" checked={h() === v} onChange={() => setH(v)} />{l}</label>}</For></div>
  </Dialog>;
});

/** A note on a cell: plain text, shown when the pointer is over the cell. */
export const noteDialog = (at: string, text: string, save: (t: string) => void) => openDialog(() => {
  const [t, setT] = createSignal(text);
  return <Dialog title={`${text ? "Edit" : "New"} Note: ${at}`} okText="OK" help="sheetdata" ok={() => save(t())} extra={text ? <button class="btn" onClick={() => { closeDialog(); save(""); }}>Delete Note</button> : undefined}>
    <textarea class="notebox" value={t()} placeholder="A note for whoever reads this cell" onInput={e => setT(e.currentTarget.value)} onKeyDown={e => { if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); closeDialog(); save(t()); } }} />
  </Dialog>;
});

export const FILTER_OPS: [string, string, number][] = [["", "(none)", 0], ["eq", "Equals", 1], ["ne", "Does not equal", 1], ["gt", "Greater than", 1], ["ge", "Greater than or equal to", 1], ["lt", "Less than", 1], ["le", "Less than or equal to", 1], ["between", "Between", 2],
  ["contains", "Contains", 1], ["notcontains", "Does not contain", 1], ["begins", "Begins with", 1], ["ends", "Ends with", 1], ["blank", "Is blank", 0], ["notblank", "Is not blank", 0], ["top", "Top items", 1], ["bottom", "Bottom items", 1], ["above", "Above average", 0], ["below", "Below average", 0]];
/** The AutoFilter menu of one column: sort, a condition, and the values to show. */
export const filterDialog = (p: { title: string; values: string[]; cur?: ColFilter; col: number; sort: (asc: boolean) => void; save: (f: ColFilter | null) => void }) => openDialog(() => {
  const [q, setQ] = createSignal(""), [on, setOn] = createSignal(new Set(p.cur?.values ?? p.values));
  const [op, setOp] = createSignal(p.cur?.op ?? ""), [a, setA] = createSignal(p.cur?.a ?? ""), [b, setB] = createSignal(p.cur?.b ?? "");
  const shown = () => p.values.filter(v => !q() || (v || "(Blanks)").toLowerCase().includes(q().toLowerCase())).slice(0, 2000);
  const n = () => FILTER_OPS.find(o => o[0] === op())?.[2] ?? 0;
  const toggle = (v: string, x: boolean) => { const s = new Set(on()); if (x) s.add(v); else s.delete(v); setOn(s); };
  const all = () => shown().every(v => on().has(v));
  return <Dialog title={`Filter: ${p.title}`} okText="OK" help="sheetdata" ok={() => {
    const values = p.values.every(v => on().has(v)) ? null : p.values.filter(v => on().has(v));
    p.save(values || op() ? { col: p.col, values, op: op(), a: a(), b: b() } : null);
  }} extra={<button class="btn" disabled={!p.cur} onClick={() => { closeDialog(); p.save(null); }}>Clear Filter</button>}>
    <div class="row2"><button class="btn sm" onClick={() => { closeDialog(); p.sort(true); }}>Sort A to Z</button><button class="btn sm" onClick={() => { closeDialog(); p.sort(false); }}>Sort Z to A</button></div>
    <div class="form fltform">
      <Row label="Show rows where"><select onChange={e => setOp(e.currentTarget.value)}><For each={FILTER_OPS}>{o => <option value={o[0]} selected={o[0] === op()}>{o[1]}</option>}</For></select></Row>
      <Show when={n() > 0}><Row label={op() === "top" || op() === "bottom" ? "How many" : "Value"}><input value={a()} onInput={e => setA(e.currentTarget.value)} /></Row></Show>
      <Show when={n() > 1}><Row label="and"><input value={b()} onInput={e => setB(e.currentTarget.value)} /></Row></Show>
      <Row label="Values"><input type="search" placeholder="Search" value={q()} onInput={e => setQ(e.currentTarget.value)} /></Row>
    </div>
    <div class="fltvals">
      <label class="chk"><input type="checkbox" checked={all()} onChange={e => { const s = new Set(on()); shown().forEach(v => (e.currentTarget.checked ? s.add(v) : s.delete(v))); setOn(s); }} /><b>(Select All)</b></label>
      <For each={shown()}>{v => <label class="chk"><input type="checkbox" checked={on().has(v)} onChange={e => toggle(v, e.currentTarget.checked)} />{v || "(Blanks)"}</label>}</For>
    </div>
  </Dialog>;
});

/** Custom Sort: sort the selected rows by several columns, each up or down. */
export const sortDialog = (cols: [number, string][], headers: boolean, run: (levels: [number, boolean][], headers: boolean) => void) => openDialog(() => {
  const [lv, setLv] = createSignal<[number, boolean][]>([[cols[0]?.[0] ?? 0, true]]), [hd, setHd] = createSignal(headers);
  const set = (i: number, x: [number, boolean]) => setLv(lv().map((y, j) => (j === i ? x : y)));
  return <Dialog title="Sort" okText="OK" help="editing" ok={() => run(lv(), hd())}>
    <div class="sortlv"><For each={lv()}>{(l, i) => <div class="row2"><span class="muted">{i() ? "Then by" : "Sort by"}</span>
      <select onChange={e => set(i(), [+e.currentTarget.value, l[1]])}><For each={cols}>{([c, n]) => <option value={c} selected={c === l[0]}>{n}</option>}</For></select>
      <select onChange={e => set(i(), [l[0], e.currentTarget.value === "a"])}><option value="a" selected={l[1]}>A to Z, smallest first</option><option value="z" selected={!l[1]}>Z to A, largest first</option></select>
      <Show when={i() > 0}><button class="btn sm" onClick={() => setLv(lv().filter((_, j) => j !== i()))}>×</button></Show></div>}</For></div>
    <div class="row2"><button class="btn sm" disabled={lv().length >= 6} onClick={() => setLv([...lv(), [cols[Math.min(lv().length, cols.length - 1)]?.[0] ?? 0, true]])}>Add Level</button>
      <label class="chk"><input type="checkbox" checked={hd()} onChange={e => setHd(e.currentTarget.checked)} />My data has headers</label></div>
  </Dialog>;
});

/** Remove Duplicates: the columns that make two rows the same. */
export const dedupeDialog = (cols: [number, string][], headers: boolean, run: (cols: number[], headers: boolean) => void) => openDialog(() => {
  const [on, setOn] = createSignal(new Set(cols.map(c => c[0]))), [hd, setHd] = createSignal(headers);
  return <Dialog title="Remove Duplicates" okText="OK" help="sheetdata" ok={() => { if (!on().size) return false; run(cols.map(c => c[0]).filter(c => on().has(c)), hd()); }}>
    <p class="muted">A row whose values in the ticked columns match a row above it is removed; the rows below move up.</p>
    <label class="chk"><input type="checkbox" checked={hd()} onChange={e => setHd(e.currentTarget.checked)} />My data has headers</label>
    <div class="fltvals"><For each={cols}>{([c, n]) => <label class="chk"><input type="checkbox" checked={on().has(c)} onChange={e => { const s = new Set(on()); if (e.currentTarget.checked) s.add(c); else s.delete(c); setOn(s); }} />{n}</label>}</For></div>
  </Dialog>;
});

const KINDS: [string, string][] = [["", "List"], ["whole", "Whole number"], ["decimal", "Decimal"], ["date", "Date"], ["time", "Time"], ["len", "Text length"], ["any", "Any value"]];
const VOPS: [string, string][] = [["between", "between"], ["notbetween", "not between"], ["eq", "equal to"], ["ne", "not equal to"], ["gt", "greater than"], ["lt", "less than"], ["ge", "greater than or equal to"], ["le", "less than or equal to"]];
/** Data Validation on the selected cells: a drop-down list (typed values or cells to read them from), or
 *  whole numbers, decimals, dates, times or a text length between bounds (values or cells), with the
 *  message shown while a cell is selected and the one shown when a value is refused (stop or warn). */
export const listDialog = (range: string, cur: Validation | undefined, save: (v: Validation | null) => void) => openDialog(() => {
  const [r, setR] = createSignal(cur?.range ?? range), [kind, setKind] = createSignal(cur?.kind ?? ""), [fromCells, setFromCells] = createSignal(!!cur?.src), [items, setItems] = createSignal((cur?.list ?? []).join("\n")), [src, setSrc] = createSignal(cur?.src ?? ""), [warn, setWarn] = createSignal(!!cur?.warn);
  const [op, setOp] = createSignal(cur?.op || "between"), [a, setA] = createSignal(cur?.a ?? ""), [b, setB] = createSignal(cur?.b ?? ""), [prompt, setPrompt] = createSignal(cur?.prompt ?? ""), [err, setErr] = createSignal(cur?.error ?? "");
  const two = () => op() === "between" || op() === "notbetween", hint = () => (kind() === "date" ? "31/12/2026 or =B1" : kind() === "time" ? "13:30 or =B1" : "10 or =$B$1");
  return <Dialog title="Data Validation" okText="OK" help="sheetdata" ok={() => {
    const list = items().split(/\n|,/).map(x => x.trim()).filter(Boolean), k = kind();
    if (!k && (fromCells() ? !src().trim() : !list.length)) return false;
    if (k && k !== "any" && (!a().trim() || (two() && !b().trim()))) return false;
    if (k === "any" && !prompt().trim()) return false;
    const v: Validation = { range: r().toUpperCase().replace(/\$/g, ""), list: !k && !fromCells() ? list : [], src: !k && fromCells() ? src().trim() : "", warn: warn() };
    if (k) Object.assign(v, { kind: k, ...(k !== "any" ? { op: op(), a: a().trim(), ...(two() ? { b: b().trim() } : {}) } : {}) });
    if (prompt().trim()) v.prompt = prompt().trim();
    if (err().trim()) v.error = err().trim();
    save(v);
  }} extra={cur ? <button class="btn" onClick={() => { closeDialog(); save(null); }}>Clear All</button> : undefined}>
    <div class="form">
      <Row label="Cells"><input value={r()} onInput={e => setR(e.currentTarget.value)} /></Row>
      <Row label="Allow"><select class="vkind" onChange={e => setKind(e.currentTarget.value)}><For each={KINDS}>{([v, l]) => <option value={v} selected={v === kind()}>{l}</option>}</For></select></Row>
      <Show when={!kind()}>
        <Row label="Values from"><span><label class="chk"><input type="radio" name="lsrc" checked={!fromCells()} onChange={() => setFromCells(false)} />Typed here</label><label class="chk"><input type="radio" name="lsrc" checked={fromCells()} onChange={() => setFromCells(true)} />Cells</label></span></Row>
        <Show when={!fromCells()} fallback={<Row label="Cells with the values"><input value={src()} placeholder="$A$2:$A$20 or Lists!A1:A9" onInput={e => setSrc(e.currentTarget.value)} /></Row>}>
          <Row label="Values (one a line)"><textarea class="vitems" value={items()} onInput={e => setItems(e.currentTarget.value)} /></Row></Show>
      </Show>
      <Show when={kind() && kind() !== "any"}>
        <Row label={kind() === "len" ? "Length is" : "Value is"}><select class="vop" onChange={e => setOp(e.currentTarget.value)}><For each={VOPS}>{([v, l]) => <option value={v} selected={v === op()}>{l}</option>}</For></select></Row>
        <Row label={two() ? "Minimum" : "Value"}><input class="va" value={a()} placeholder={hint()} onInput={e => setA(e.currentTarget.value)} /></Row>
        <Show when={two()}><Row label="Maximum"><input class="vb" value={b()} placeholder={hint()} onInput={e => setB(e.currentTarget.value)} /></Row></Show>
      </Show>
      <Row label="Message when selected"><input class="vprompt" value={prompt()} placeholder="Optional: what to type here" onInput={e => setPrompt(e.currentTarget.value)} /></Row>
      <Row label="Message when wrong"><input value={err()} placeholder="Optional: said when a value is refused" onInput={e => setErr(e.currentTarget.value)} /></Row>
      <Row label="A wrong value"><select class="vstyle" onChange={e => setWarn(e.currentTarget.value === "warn")}><option value="stop" selected={!warn()}>Stop: refuse it</option><option value="warn" selected={warn()}>Warn: ask, then allow it</option></select></Row>
    </div>
    <p class="muted">Bounds can be values or cells (=$B$1, =Lists!A1). Circle Invalid Data marks the cells that already break the rule.</p>
  </Dialog>;
});

/** Insert or edit a link: the address (web or mail) and the text the cell shows. */
export const linkDialog = (cur: { url: string; text: string }, save: (x: { url: string; text: string } | null) => void) => openDialog(() => {
  const [url, setUrl] = createSignal(cur.url), [text, setText] = createSignal(cur.text);
  const ok = () => /^(https?:\/\/|mailto:)[^\s"<>`\\^{}|]+$/i.test(url().trim());
  return <Dialog title={cur.url ? "Edit Link" : "Insert Link"} okText="OK" help="sheetdata" ok={() => { if (!ok()) return false; save({ url: url().trim(), text: text().trim() }); }} extra={cur.url ? <button class="btn" onClick={() => { closeDialog(); save(null); }}>Remove Link</button> : undefined}>
    <div class="form">
      <Row label="Address"><input class="laddr" value={url()} placeholder="https://… or mailto:name@example.com" onInput={e => setUrl(e.currentTarget.value)} /></Row>
      <Row label="Text to show"><input class="ltext" value={text()} placeholder="Optional: the address shows when empty" onInput={e => setText(e.currentTarget.value)} /></Row>
    </div>
    <p class="muted">{url().trim() && !ok() ? "A link starts with http://, https:// or mailto: and has no spaces or quotes." : "Ctrl+click the cell (or right-click › Open Link) to open it in the browser."}</p>
  </Dialog>;
});

/** Before a link opens: the full address, so a link that is not what its text says is seen. */
export const openLinkDialog = (url: string, open: () => void) => openDialog(() =>
  <Dialog title="Open Link" okText="Open" ok={open}><p>Open this address in your browser?</p><p class="linkurl"><code>{url}</code></p><p class="muted">Only open links from people you trust.</p></Dialog>);

/** Define Name: a name for a range, a value or a formula, for every sheet's formulas. */
export const nameDialog = (cur: DefName, save: (n: DefName) => void | boolean | Promise<void | boolean>) => openDialog(() => {
  const [n, setN] = createSignal(cur.name), [f, setF] = createSignal(cur.formula), [c, setC] = createSignal(cur.comment ?? "");
  return <Dialog title={cur.name ? "Edit Name" : "New Name"} okText="OK" help="sheetnames" ok={() => (n().trim() && f().trim() ? save({ name: n().trim(), formula: f().trim().startsWith("=") ? f().trim() : `=${f().trim()}`, ...(c().trim() ? { comment: c().trim() } : {}) }) : false)}>
    <div class="form">
      <Row label="Name"><input class="nname" value={n()} placeholder="Rates" onInput={e => setN(e.currentTarget.value)} /></Row>
      <Row label="Refers to"><input class="nref" value={f()} placeholder="=Prices!$B$2:$B$20 or =0.05" onInput={e => setF(e.currentTarget.value)} /></Row>
      <Row label="Comment"><input value={c()} onInput={e => setC(e.currentTarget.value)} /></Row>
    </div>
    <p class="muted">A name starts with a letter or _, has no spaces and is not a cell address (B12) or one of the plan's names (ProjectFinish, Tasks…).</p>
  </Dialog>;
});

/** Name Manager: every defined name, to add, edit, delete or go to. Changes are saved together with OK. */
export const nameManager = (names: DefName[], here: string, save: (l: DefName[]) => Promise<boolean>, go: (f: string) => void) => openDialog(() => {
  const [list, setList] = createSignal(names.map(x => ({ ...x }))), [sel, setSel] = createSignal(0);
  const edit = (i: number) => nameDialog(i >= 0 ? list()[i] : { name: "", formula: here }, n => { if (list().some((x, j) => j !== i && x.name.toLowerCase() === n.name.toLowerCase())) return false; setList(i >= 0 ? list().map((x, j) => (j === i ? n : x)) : [...list(), n]); setSel(i >= 0 ? i : list().length - 1); });
  return <Dialog title="Name Manager" okText="OK" wide help="sheetnames" ok={async () => { if (!(await save(list()))) return false; }}>
    <div class="row2"><button class="btn sm" onClick={() => edit(-1)}>New…</button><button class="btn sm" disabled={!list().length} onClick={() => edit(sel())}>Edit…</button>
      <button class="btn sm" disabled={!list().length} onClick={() => { setList(list().filter((_, j) => j !== sel())); setSel(Math.max(0, sel() - 1)); }}>Delete</button>
      <button class="btn sm" disabled={!list()[sel()]} onClick={() => { const x = list()[sel()]; if (x) { closeDialog(); go(x.formula); } }}>Go To</button></div>
    <table class="tbl names"><thead><tr><th>Name</th><th>Refers to</th><th>Comment</th></tr></thead>
      <tbody><For each={list()} fallback={<tr><td colSpan={3} class="muted">No names yet. New… names a range, a value or a formula.</td></tr>}>{(x, i) => <tr classList={{ on: sel() === i(), click: true }} onClick={() => setSel(i())} onDblClick={() => edit(i())}><td><b>{x.name}</b></td><td><code>{x.formula}</code></td><td>{x.comment}</td></tr>}</For></tbody></table>
  </Dialog>;
});

/** Go To (F5, Ctrl+G): a cell, a range or another sheet's cell. */
export const goToDialog = (go: (ref: string) => boolean) => openDialog(() => {
  const [v, setV] = createSignal("");
  return <Dialog title="Go To" okText="OK" help="sheetediting" ok={() => go(v())}>
    <div class="form"><Row label="Reference"><input value={v()} placeholder="B12, A1:D20 or Sheet2!A1" onInput={e => setV(e.currentTarget.value)} /></Row></div>
  </Dialog>;
});
