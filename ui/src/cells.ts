// Shared by the task sheet and the Sheets tabs: tab-separated clipboard text, spreadsheet-style series
// for the fill handle, and the last copy (values and formats) for Paste Special.
import { dateDay, dayDate, iso } from "./api";

export const parseTsv = (t: string): string[][] => {
  const out: string[][] = [[]];
  let cell = "", q = false;
  for (let i = 0; i < t.length; i++) {
    const ch = t[i];
    if (q) { if (ch === '"' && t[i + 1] === '"') { cell += '"'; i++; } else if (ch === '"') q = false; else cell += ch; }
    else if (ch === '"' && cell === "") q = true;
    else if (ch === "\t") { out[out.length - 1].push(cell); cell = ""; }
    else if (ch === "\n" || ch === "\r") { if (ch === "\r" && t[i + 1] === "\n") i++; out[out.length - 1].push(cell); cell = ""; out.push([]); }
    else cell += ch;
  }
  out[out.length - 1].push(cell);
  while (out.length > 1 && out[out.length - 1].every(x => !x)) out.pop();
  return out;
};
export const toTsv = (m: string[][]) => m.map(r => r.map(v => (/[\t\n"]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v)).join("\t")).join("\n");
export const transpose = (m: string[][]) => Array.from({ length: Math.max(...m.map(r => r.length)) }, (_, j) => m.map(r => r[j] ?? ""));

/** The last copy from either grid: values and cell styles (for Paste Special › Formats). */
export const clip = { text: "", styles: [] as string[][] };

const NUM = /^(-?[\d,]*\.?\d+)(\s*[^\d\s]*)$/, TAIL = /^(.*?)(\d+)$/;
const DATE = /^\d{4}-\d{1,2}-\d{1,2}$|^\d{1,2}[/.-]\d{1,2}[/.-]\d{2,4}$|^\d{1,2}[ -][A-Za-z]{3,9}[ -]\d{2,4}$|^[A-Za-z]{3,9} \d{1,2},? \d{4}$/;
/** Whether 03/04/2026 is 3 April (the plan's date format puts the day first); set by the store. */
export const order = { dayFirst: true };
const DMY = /^(\d{1,2})([/.-])(\d{1,2})\2(\d{2,4})$/, DMON = /^(\d{1,2})([ -])([A-Za-z]{3,9})\2(\d{2,4})$/, MON = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
export const asDate = (s: string) => {
  s = s.trim(); if (!DATE.test(s)) return null;
  const m = DMY.exec(s);
  if (m) { const [mo, da] = order.dayFirst ? [+m[3], +m[1]] : [+m[1], +m[3]], y = m[4].length === 2 ? 2000 + +m[4] : +m[4], d = new Date(y, mo - 1, da); return d.getMonth() === mo - 1 && d.getDate() === da ? dateDay(d) : null; }
  const d = new Date(/^\d{4}-/.test(s) ? s.replace(/-/g, "/") : s.replace(/-(\d\d)$/, " 20$1").replace(/-/g, " ")); return isNaN(+d) ? null : dateDay(d);
};
/** Day `d` written the way `s` was (03/04/2026, 3-Apr-26, 2026-04-03). */
const like = (s: string, d: number) => {
  const x = dayDate(d), [Y, M, D] = [x.getFullYear(), x.getMonth() + 1, x.getDate()], p = (n: number, w: number) => String(n).padStart(w, "0"), m = DMY.exec(s.trim()), n = DMON.exec(s.trim());
  const yy = (t: string) => (t.length === 2 ? p(Y % 100, 2) : String(Y));
  if (m) return order.dayFirst ? `${p(D, m[1].length)}${m[2]}${p(M, m[3].length)}${m[2]}${yy(m[4])}` : `${p(M, m[1].length)}${m[2]}${p(D, m[3].length)}${m[2]}${yy(m[4])}`;
  if (n) return `${p(D, n[1].length)}${n[2]}${MON[M - 1]}${n[2]}${yy(n[4])}`;
  return iso(d);
};
/**
 * The k-th value (k = 1, 2, …) after the source cells `src`, as a spreadsheet fill handle does:
 * numbers continue their step (a single number counts up by 1), "Phase 1" → "Phase 2",
 * dates continue their step (a single date is copied); anything else repeats. `copy` (Ctrl) repeats.
 */
export const series = (src: string[], k: number, copy = false): string => {
  const rep = src[(k - 1) % src.length] ?? "";
  if (copy || !src.length || src.some(s => !s.trim())) return rep;
  const nums = src.map(s => s.trim().match(NUM));
  if (nums.every(Boolean)) {
    const v = nums.map(m => +m![1].replace(/,/g, "")), unit = nums[0]![2], step = v.length > 1 ? (v[v.length - 1] - v[0]) / (v.length - 1) : 1;
    const x = v[v.length - 1] + step * k, dec = Math.max(...nums.map(m => (m![1].split(".")[1] ?? "").length));
    return `${x.toFixed(dec)}${unit}`;
  }
  const dates = src.map(asDate);
  if (dates.every(d => d !== null)) {
    if (dates.length < 2) return rep;
    const step = (dates[dates.length - 1]! - dates[0]!) / (dates.length - 1);
    return like(src[src.length - 1], dates[dates.length - 1]! + step * k);
  }
  const tails = src.map(s => s.match(TAIL));
  if (tails.every(t => t && t[1] === tails[0]![1])) {
    const v = tails.map(t => +t![2]), step = v.length > 1 ? (v[v.length - 1] - v[0]) / (v.length - 1) : 1;
    return `${tails[0]![1]}${v[v.length - 1] + step * k}`;
  }
  return rep;
};
