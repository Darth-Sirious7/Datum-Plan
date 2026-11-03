// Conditional formatting for sheets: rules over ranges (the first rule that applies to a cell gives its
// look), data bars and colour scales; worked out here from the values the engine shows, so the screen,
// printing and the workbook export agree. The rules are kept with the sheet (SheetData.cond).
import { For, Show, createSignal } from "solid-js";
import { CondRule, SheetData } from "./api";
import { openDialog, Item } from "./store";
import { Dialog, Row } from "./ui";
import * as F from "./fmt";

type Cell = [string, string, string, boolean, number | null];
export type Look = { st?: string; bar?: [number, string]; bg?: string };
const A1 = (s: string): [number, number] | null => { const m = /^([A-Z]+)(\d+)$/.exec(s.trim()); if (!m) return null; let c = 0; for (const ch of m[1]) c = c * 26 + ch.charCodeAt(0) - 64; return [+m[2] - 1, c - 1]; };
const name = (c: number) => { let s = ""; c += 1; while (c > 0) { const m = (c - 1) % 26; s = String.fromCharCode(65 + m) + s; c = Math.floor((c - 1) / 26); } return s; };
export const bounds = (range: string) => { const [a, b = a] = range.split(":"), p = A1(a), q = A1(b); return p && q ? { r0: Math.min(p[0], q[0]), c0: Math.min(p[1], q[1]), r1: Math.max(p[0], q[0]), c1: Math.max(p[1], q[1]) } : null; };
const hex = (h: string) => [1, 3, 5].map(i => parseInt(h.slice(i, i + 2), 16) || 0);
const mix = (a: string, b: string, t: number) => "#" + hex(a).map((x, i) => Math.round(x + (hex(b)[i] - x) * t).toString(16).padStart(2, "0")).join("");

/** Dates a rule can look for (days since 1970, local, as the cells hold them). Weeks start on Sunday, as in a workbook. */
export const DATE_KINDS: [string, string][] = [["past", "Before today (overdue)"], ["today", "Today"], ["yesterday", "Yesterday"], ["tomorrow", "Tomorrow"], ["last7", "In the last 7 days"], ["next7", "In the next 7 days"],
  ["lastweek", "Last week"], ["thisweek", "This week"], ["nextweek", "Next week"], ["lastmonth", "Last month"], ["thismonth", "This month"], ["nextmonth", "Next month"], ["future", "After today"]];
const dateHit = (day: number, kind: string) => {
  const now = new Date(), t = Math.floor((now.getTime() - now.getTimezoneOffset() * 60000) / 86400000), dow = new Date(t * 86400000).getUTCDay(), sun = t - dow;
  const d = new Date(day * 86400000), ym = d.getUTCFullYear() * 12 + d.getUTCMonth(), tm = now.getFullYear() * 12 + now.getMonth();
  switch (kind) {
    case "past": return day < t; case "future": return day > t; case "today": return day === t; case "yesterday": return day === t - 1; case "tomorrow": return day === t + 1;
    case "last7": return day <= t && day > t - 7; case "next7": return day >= t && day < t + 7;
    case "thisweek": return day >= sun && day < sun + 7; case "lastweek": return day >= sun - 7 && day < sun; case "nextweek": return day >= sun + 7 && day < sun + 14;
    case "thismonth": return ym === tm; case "lastmonth": return ym === tm - 1; case "nextmonth": return ym === tm + 1;
  }
  return false;
};
/** What every rule does to the cells it covers: a1 → look. */
export const condLooks = (sh: SheetData | undefined, cells: Map<string, Cell>): Map<string, Look> => {
  const out = new Map<string, Look>();
  for (const rule of sh?.cond ?? []) {
    const b = bounds(rule.range); if (!b) continue;
    const ids: string[] = [];
    for (let r = b.r0; r <= Math.min(b.r1, b.r0 + 5000); r++) for (let c = b.c0; c <= Math.min(b.c1, b.c0 + 200); c++) ids.push(`${name(c)}${r + 1}`);
    const num = (id: string) => cells.get(id)?.[4] ?? null, txt = (id: string) => cells.get(id)?.[2] ?? "";
    const nums = ids.map(num).filter((x): x is number => x !== null);
    let lo = Infinity, hi = -Infinity; for (const x of nums) { if (x < lo) lo = x; if (x > hi) hi = x; }
    const avg = nums.reduce((a, x) => a + x, 0) / Math.max(1, nums.length);
    const sorted = [...nums].sort((x, y) => y - x), n = Math.max(1, Math.round(+rule.a || 10));
    const counts = new Map<string, number>(); ids.forEach(id => { const t = txt(id).toLowerCase(); if (t) counts.set(t, (counts.get(t) ?? 0) + 1); });
    const a = rule.a.trim(), bb = rule.b.trim(), an = a === "" ? NaN : +a, bn = bb === "" ? NaN : +bb;
    const test = (id: string): boolean => {
      const x = num(id), t = txt(id), tl = t.toLowerCase(), al = a.toLowerCase();
      const cmp = (f: (p: number, q: number) => boolean) => (x !== null && !isNaN(an) ? f(x, an) : false);
      switch (rule.op) {
        case "gt": return cmp((p, q) => p > q); case "ge": return cmp((p, q) => p >= q);
        case "lt": return cmp((p, q) => p < q); case "le": return cmp((p, q) => p <= q);
        case "eq": return x !== null && !isNaN(an) ? x === an : !!t && tl === al;
        case "ne": return x !== null && !isNaN(an) ? x !== an : tl !== al;
        case "between": case "notbetween": { const inside = x !== null && !isNaN(an) && !isNaN(bn) && x >= Math.min(an, bn) && x <= Math.max(an, bn); return rule.op === "between" ? inside : x !== null && !inside; }
        case "contains": return !!al && tl.includes(al); case "notcontains": return !!t && !tl.includes(al);
        case "begins": return !!al && tl.startsWith(al); case "ends": return !!al && tl.endsWith(al);
        case "blank": return !t; case "notblank": return !!t;
        case "dup": return !!t && (counts.get(tl) ?? 0) > 1; case "unique": return !!t && counts.get(tl) === 1;
        case "top": return x !== null && x >= (sorted[Math.min(n, sorted.length) - 1] ?? Infinity);
        case "bottom": return x !== null && x <= (sorted[Math.max(0, sorted.length - n)] ?? -Infinity);
        case "above": return x !== null && x > avg; case "below": return x !== null && x < avg;
        case "date": return x !== null && dateHit(Math.floor(x) - 25569, a);
      }
      return false;
    };
    for (const id of ids) {
      const look = out.get(id) ?? {};
      if (rule.op === "bar") { const x = num(id); if (x !== null && !look.bar) { const base = Math.min(0, lo); look.bar = [hi > base ? Math.max(0, Math.min(1, (x - base) / (hi - base))) : 1, rule.style || "#2F6FDE"]; } }
      else if (rule.op === "scale") { const x = num(id); if (x !== null && !look.bg) { const t = hi > lo ? (x - lo) / (hi - lo) : 0.5; look.bg = rule.style ? (t < 0.5 ? mix(rule.a, rule.style, t * 2) : mix(rule.style, rule.b, t * 2 - 1)) : mix(rule.a, rule.b, t); } }
      else if (!look.st && test(id)) look.st = rule.style;
      if (look.st || look.bar || look.bg) out.set(id, look);
    }
  }
  return out;
};
/** A look as CSS over the cell's own style. */
export const lookCss = (l: Look | undefined) => {
  if (!l) return {};
  const o = { ...(F.css(l.st) as Record<string, string>) };
  if (l.bg) o.background = l.bg;
  if (l.bar) o.background = `linear-gradient(90deg, ${l.bar[1]}66 ${Math.round(l.bar[0] * 100)}%, transparent ${Math.round(l.bar[0] * 100)}%)`;
  return o;
};

/** Looks a rule can give, as in a spreadsheet's quick choices. */
export const LOOKS: [string, string][] = [["Light red fill with dark red text", "c:#9C0006;f:#FFC7CE"], ["Yellow fill with dark yellow text", "c:#9C5700;f:#FFEB9C"], ["Green fill with dark green text", "c:#006100;f:#C6EFCE"],
  ["Light red fill", "f:#FFC7CE"], ["Red text", "c:#C00000"], ["Bold", "b"], ["Blue fill with white text", "b;c:#FFFFFF;f:#0B5CAD"], ["Grey text", "c:#8A939E"], ["Strikethrough", "s;c:#8A939E"]];
const OPS: [string, string, number][] = [["gt", "Greater than", 1], ["ge", "Greater than or equal to", 1], ["lt", "Less than", 1], ["le", "Less than or equal to", 1], ["eq", "Equal to", 1], ["ne", "Not equal to", 1],
  ["between", "Between", 2], ["notbetween", "Not between", 2], ["contains", "Text that contains", 1], ["notcontains", "Text that does not contain", 1], ["begins", "Text that begins with", 1], ["ends", "Text that ends with", 1],
  ["date", "A date occurring", 1], ["blank", "Blank cells", 0], ["notblank", "Cells that are not blank", 0], ["dup", "Duplicate values", 0], ["unique", "Unique values", 0], ["top", "Top items", 1], ["bottom", "Bottom items", 1], ["above", "Above average", 0], ["below", "Below average", 0]];
export const describe = (r: CondRule) => r.op === "bar" ? "Data bars" : r.op === "scale" ? "Colour scale" : r.op === "date" ? `Date: ${DATE_KINDS.find(d => d[0] === r.a)?.[1] ?? r.a}` : `${OPS.find(o => o[0] === r.op)?.[1] ?? r.op}${(OPS.find(o => o[0] === r.op)?.[2] ?? 0) > 0 ? ` ${r.a}` : ""}${(OPS.find(o => o[0] === r.op)?.[2] ?? 0) > 1 ? ` and ${r.b}` : ""}`;

/** The rule dialog: when, which value(s), which look, over which cells. */
export const ruleDialog = (start: CondRule, save: (r: CondRule) => void) => openDialog(() => {
  const [r, setR] = createSignal<CondRule>({ ...start, style: start.style || LOOKS[0][1] });
  const k = () => OPS.find(o => o[0] === r().op)?.[2] ?? 0;
  return <Dialog title="Conditional Formatting Rule" okText="OK" help="sheetcond" ok={() => { if (!bounds(r().range)) return false; save(r()); }}>
    <div class="form">
      <Row label="Cells"><input value={r().range} onInput={e => setR({ ...r(), range: e.currentTarget.value.toUpperCase().replace(/\$/g, "") })} /></Row>
      <Row label="Format cells that are"><select onChange={e => setR({ ...r(), op: e.currentTarget.value })}><For each={OPS}>{o => <option value={o[0]} selected={o[0] === r().op}>{o[1]}</option>}</For></select></Row>
      <Show when={r().op === "date"}><Row label="When"><select onChange={e => setR({ ...r(), a: e.currentTarget.value })}><For each={DATE_KINDS}>{([v, l]) => <option value={v} selected={v === r().a}>{l}</option>}</For></select></Row></Show>
      <Show when={k() > 0 && r().op !== "date"}><Row label={r().op === "top" || r().op === "bottom" ? "How many" : "Value"}><input value={r().a} onInput={e => setR({ ...r(), a: e.currentTarget.value })} /></Row></Show>
      <Show when={k() > 1}><Row label="and"><input value={r().b} onInput={e => setR({ ...r(), b: e.currentTarget.value })} /></Row></Show>
      <Row label="With"><select onChange={e => setR({ ...r(), style: e.currentTarget.value })}><For each={LOOKS}>{([l, st]) => <option value={st} selected={st === r().style}>{l}</option>}</For></select>
        <span class="cfprev" style={F.css(r().style)}>AaBbCc 123</span></Row>
    </div>
  </Dialog>;
});

/** Manage Rules: the sheet's rules in order (the first that applies wins); edit, move, delete. */
export const manageDialog = (list: () => CondRule[], set: (l: CondRule[]) => void, fresh: () => CondRule) => openDialog(() => {
  const move = (i: number, d: number) => { const l = [...list()], [x] = l.splice(i, 1); l.splice(Math.max(0, Math.min(l.length, i + d)), 0, x); set(l); };
  return <Dialog title="Conditional Formatting Rules" wide help="sheetcond">
    <p class="muted">Rules apply in this order; for each cell the first rule that matches gives its look. Data bars and colour scales show together with it.</p>
    <table class="tbl cftable"><thead><tr><th>Cells</th><th>Rule</th><th>Look</th><th /></tr></thead>
      <tbody><For each={list()} fallback={<tr><td colSpan={4} class="muted">No rules on this sheet yet.</td></tr>}>{(r, i) => <tr>
        <td>{r.range}</td><td>{describe(r)}</td>
        <td><span class="cfprev" style={r.op === "bar" ? { background: `linear-gradient(90deg, ${r.style}66 60%, transparent 60%)` } : r.op === "scale" ? { background: `linear-gradient(90deg, ${r.a}, ${r.style || r.a}, ${r.b})` } : F.css(r.style)}>AaBbCc 123</span></td>
        <td class="cfbtns"><button class="btn sm" title="Up" disabled={i() === 0} onClick={() => move(i(), -1)}>↑</button><button class="btn sm" title="Down" disabled={i() === list().length - 1} onClick={() => move(i(), 1)}>↓</button>
          <Show when={r.op !== "bar" && r.op !== "scale"}><button class="btn sm" onClick={() => ruleDialog(r, x => set(list().map((y, j) => (j === i() ? x : y))))}>Edit…</button></Show>
          <button class="btn sm" onClick={() => set(list().filter((_, j) => j !== i()))}>Delete</button></td></tr>}</For></tbody></table>
    <button class="btn sm" onClick={() => ruleDialog(fresh(), x => set([...list(), x]))}>+ New Rule…</button>
  </Dialog>;
});

/** The Conditional menu: quick rules on the selection, data bars, colour scales, and the rules list. */
export const condMenu = (sel: string, list: () => CondRule[], set: (l: CondRule[]) => void, clearSel: () => void): Item[] => {
  const add = (r: Partial<CondRule>) => set([...list(), { range: sel, op: "gt", a: "", b: "", style: LOOKS[0][1], ...r }]);
  const ask = (op: string, a = "") => () => ruleDialog({ range: sel, op, a, b: "", style: LOOKS[0][1] }, x => set([...list(), x]));
  const BARS: [string, string][] = [["Blue", "#2F6FDE"], ["Green", "#1E8449"], ["Red", "#C0392B"], ["Orange", "#E67E22"], ["Purple", "#7D3C98"], ["Teal", "#138D90"]];
  const SCALES: [string, string, string, string][] = [["Green – yellow – red", "#63BE7B", "#FFEB84", "#F8696B"], ["Red – yellow – green", "#F8696B", "#FFEB84", "#63BE7B"], ["White – green", "#FFFFFF", "", "#63BE7B"], ["White – red", "#FFFFFF", "", "#F8696B"], ["Blue – white – red", "#5A8AC6", "#FFFFFF", "#F8696B"], ["White – blue", "#FFFFFF", "", "#5A8AC6"]];
  return [
    { label: "Highlight Cells", icon: "cfhi", sub: [{ label: "Greater Than…", run: ask("gt") }, { label: "Less Than…", run: ask("lt") }, { label: "Between…", run: ask("between") }, { label: "Equal To…", run: ask("eq") },
      { label: "Text That Contains…", run: ask("contains") }, { label: "A Date Occurring…", run: ask("date", "past") }, { label: "Blank Cells", run: () => add({ op: "blank", style: LOOKS[1][1] }) }, { label: "Duplicate Values", run: () => add({ op: "dup" }) }] },
    { label: "Top and Bottom", icon: "cftop", sub: [{ label: "Top 10 Items…", run: ask("top", "10") }, { label: "Bottom 10 Items…", run: ask("bottom", "10") }, { label: "Above Average", run: () => add({ op: "above", style: LOOKS[2][1] }) }, { label: "Below Average", run: () => add({ op: "below" }) }] },
    { label: "Data Bars", icon: "cfbar", sub: BARS.map(([l, c]) => ({ label: l, swatch: c, run: () => add({ op: "bar", style: c }) })) },
    { label: "Colour Scales", icon: "cfscale", sub: SCALES.map(([l, a, m, b]) => ({ label: l, swatch: m || b, run: () => add({ op: "scale", a, b, style: m }) })) },
    "-",
    { label: "New Rule…", run: ask("gt") },
    { label: "Clear Rules from Selected Cells", run: clearSel },
    { label: "Clear Rules from Entire Sheet", off: !list().length, run: () => set([]) },
    { label: "Manage Rules…", run: () => manageDialog(list, set, () => ({ range: sel, op: "gt", a: "", b: "", style: LOOKS[0][1] })) },
  ];
};
