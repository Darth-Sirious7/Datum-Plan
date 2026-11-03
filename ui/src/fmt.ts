// Text formatting (Font and Alignment groups) for the task sheet and the Sheets tabs.
// A style is a short string such as "b;i;c:#C0392B;f:#FFF2CC;z:12;a:c"; the engine stores it as is.
import { createSignal, JSX } from "solid-js";
import { toast } from "./store";

export type St = { b?: boolean; i?: boolean; u?: boolean; s?: boolean; w?: boolean; c?: string; f?: string; z?: number; a?: string; n?: string; v?: string; x?: number; bt?: string; bb?: string; bl?: string; br?: string };
/** Border sides; a side is "kind #colour", kind thin, med, thick, dbl, dash or dot. */
export const SIDES = ["bt", "bb", "bl", "br"] as const;
export type Side = typeof SIDES[number];
export const parse = (x = ""): St => {
  const s: St = {};
  for (const t of x.split(";")) {
    if (!t) continue;
    const i = t.indexOf(":"), k = i < 0 ? t : t.slice(0, i), v = i < 0 ? "" : t.slice(i + 1);
    if (i < 0 && "biusw".includes(k) && k.length === 1) (s as any)[k] = true;
    else if (k === "z" || k === "x") s[k] = +v;
    else if (["c", "f", "a", "n", "v", ...SIDES].includes(k)) (s as any)[k] = v;
  }
  return s;
};
export const ser = (s: St) => [s.b && "b", s.i && "i", s.u && "u", s.s && "s", s.w && "w", s.c && `c:${s.c}`, s.f && `f:${s.f}`, s.z && `z:${s.z}`, s.a && `a:${s.a}`, s.n && `n:${s.n}`, s.v && `v:${s.v}`, s.x && `x:${s.x}`,
  ...SIDES.map(k => s[k] && `${k}:${s[k]}`)].filter(Boolean).join(";");
const LINE: Record<string, string> = { thin: "1px solid", med: "2px solid", thick: "3px solid", dbl: "3px double", dash: "1px dashed", dot: "1px dotted" };
/** A border side as CSS ("thin #000" → "1px solid #000"). */
export const line = (v?: string) => { if (!v) return undefined; const [k, c] = v.split(" "); return `${LINE[k] ?? LINE.thin} ${c || "#000"}`; };
/** CSS for a style; `fill: false` leaves the background out (used where selection colours must win). */
export const css = (x?: string): JSX.CSSProperties => {
  if (!x) return {};
  const s = parse(x), o: JSX.CSSProperties = {};
  if (s.b) o["font-weight"] = "700";
  if (s.i) o["font-style"] = "italic";
  if (s.u || s.s) o["text-decoration"] = [s.u && "underline", s.s && "line-through"].filter(Boolean).join(" ");
  if (s.c) o.color = s.c;
  if (s.f) o.background = s.f;
  if (s.z) o["font-size"] = `${s.z}px`;
  if (s.n) o["font-family"] = `'${s.n}', sans-serif`;
  if (s.a) { o["text-align"] = s.a === "c" ? "center" : s.a === "r" ? "right" : "left"; o["justify-content"] = s.a === "c" ? "center" : s.a === "r" ? "flex-end" : "flex-start"; }
  if (s.x) o["padding-left"] = `${5 + s.x * 12}px`;
  if (s.bt) o["border-top"] = line(s.bt);
  if (s.bb) o["border-bottom"] = line(s.bb);
  if (s.bl) o["border-left"] = line(s.bl);
  if (s.br) o["border-right"] = line(s.br);
  return o;
};

/** The view with the focus says how to read and write the styles of its selected cells: [id, style]. */
/** `paste` (Paste Special) is called with what to paste and whether to transpose. */
export type Target = { get: () => [string, string][]; set: (list: [string, string][]) => Promise<unknown>; paste?: (what: "all" | "values" | "formats", transpose: boolean) => void };
export const [target, setTarget] = createSignal<Target | null>(null);
export const current = (): St => parse(target()?.get()[0]?.[1]);
const change = (f: (s: St) => St) => {
  const t = target(), list = t?.get() ?? [];
  if (!t || !list.length) return toast("Select the cells to format first.");
  const out = list.map(([id, v]) => [id, ser(f(parse(v)))] as [string, string]).filter(([, v], k) => v !== list[k][1]);
  if (out.length) t.set(out);
};
export const toggle = (k: "b" | "i" | "u" | "s") => { const on = !(target()?.get() ?? []).every(([, v]) => parse(v)[k]); change(s => ({ ...s, [k]: on })); };
export const setProp = <K extends "c" | "f" | "z" | "a" | "n" | "v" | "x" | "w">(k: K, v: St[K]) => change(s => ({ ...s, [k]: v }));
/** Changes each selected cell's style with `f`, given the cell's id. */
export const changeEach = (f: (s: St, id: string) => St) => { const t = target(), list = t?.get() ?? []; if (!t || !list.length) return toast("Select the cells to format first."); const out = list.map(([id, v]) => [id, ser(f(parse(v), id))] as [string, string]).filter(([, v], k) => v !== list[k][1]); if (out.length) t.set(out); };
export const clearFmt = () => change(() => ({}));

// Format Painter: copy the active cell's style, then the next selection gets it.
export const [painter, setPainter] = createSignal<string | null>(null);
export const paint = () => { const s = target()?.get()[0]?.[1] ?? ""; setPainter(s); toast("Format Painter: select the cells to format."); };
/** Called by the grids when a mouse selection ends. */
export const painted = () => { const s = painter(); if (s === null) return; setPainter(null); change(() => parse(s)); };
