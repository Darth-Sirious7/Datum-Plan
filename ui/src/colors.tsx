// Colour choosers as in a spreadsheet: a palette that drops down under its button (theme colours
// with lighter and darker shades, standard colours, recent ones) and More Colors…, a small dialog
// with a colour field, a hue bar and hex/RGB boxes. Never the system colour box.
import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import { openDialog, closeDialog } from "./store";
import { Dialog } from "./ui";

const BASE = ["#FFFFFF", "#000000", "#E8E8E3", "#0C1A2B", "#0B5CAD", "#E67E22", "#7F8C8D", "#F1C40F", "#3B82F6", "#1E8449"];
export const STANDARD = ["#C00000", "#FF0000", "#FFC000", "#FFFF00", "#92D050", "#00B050", "#00B0F0", "#0070C0", "#002060", "#7030A0"];
const hex2 = (n: number) => Math.round(Math.max(0, Math.min(255, n))).toString(16).padStart(2, "0").toUpperCase();
const rgb = (h: string) => { const v = parseInt(h.slice(1), 16); return [v >> 16 & 255, v >> 8 & 255, v & 255]; };
const mix = (h: string, to: number, t: number) => "#" + rgb(h).map(c => hex2(c + (to - c) * t)).join("");
/** Theme colours in rows: the colour, then 80/60/40 % lighter, then 25/50 % darker (white and black shade the other way). */
const THEME = [0, 1, 2, 3, 4, 5].map(row => BASE.map((c, i) => {
  if (row === 0) return c;
  if (i === 0) return mix(c, 0, [0, .05, .15, .25, .35, .5][row]);
  if (i === 1) return mix(c, 255, [0, .5, .35, .25, .15, .05][row]);
  return row <= 3 ? mix(c, 255, [0, .8, .6, .4][row]) : mix(c, 0, row === 4 ? .25 : .5);
}));
const [recent, setRecent] = createSignal<string[]>([]);
const remember = (c: string) => setRecent([c, ...recent().filter(x => x !== c)].slice(0, 10));

/** Opens the palette under the clicked button; `set` gets "#RRGGBB", or "" for Automatic / No Fill. */
export const colorPalette = (e: MouseEvent, current: string | undefined, set: (c: string) => void, none = "No Fill") => {
  const el0 = e.currentTarget as HTMLElement | null, b = el0?.getBoundingClientRect ? el0.getBoundingClientRect() : { left: e.clientX, top: e.clientY, bottom: e.clientY };
  openDialog(() => {
    let el!: HTMLDivElement;
    const pick = (c: string) => { closeDialog(); if (c) remember(c); set(c); };
    const key = (k: KeyboardEvent) => { if (k.key === "Escape") { k.stopPropagation(); closeDialog(); } };
    onMount(() => { document.addEventListener("keydown", key, true); const r = el.getBoundingClientRect(); if (r.right > innerWidth - 4) el.style.left = `${innerWidth - r.width - 4}px`; if (r.bottom > innerHeight - 4) el.style.top = `${Math.max(4, b.top - r.height - 2)}px`; });
    onCleanup(() => document.removeEventListener("keydown", key, true));
    const Sw = (p: { c: string }) => <button class="csw" classList={{ on: (current ?? "").toUpperCase() === p.c.toUpperCase() }} style={{ background: p.c }} title={p.c} onClick={() => pick(p.c)} />;
    return (
      <div class="cpov" onMouseDown={ev => ev.target === ev.currentTarget && closeDialog()}>
        <div class="cpop" ref={el} style={{ left: `${b.left}px`, top: `${b.bottom + 2}px` }}>
          <button class="cnone" onClick={() => pick("")}><i />{none}</button>
          <small>Theme Colors</small>
          <div class="cgrid first"><For each={THEME[0]}>{c => <Sw c={c} />}</For></div>
          <div class="cgrid shades"><For each={THEME.slice(1).flat()}>{c => <Sw c={c} />}</For></div>
          <small>Standard Colors</small>
          <div class="cgrid"><For each={STANDARD}>{c => <Sw c={c} />}</For></div>
          <Show when={recent().length}><small>Recent Colors</small><div class="cgrid"><For each={recent()}>{c => <Sw c={c} />}</For></div></Show>
          <button class="cmore" onClick={() => { closeDialog(); moreColors(current || "#0B5CAD", c => { remember(c); set(c); }); }}><svg><use href="#i-palette" /></svg>More Colors…</button>
        </div>
      </div>
    );
  });
};

/** More Colors…: a colour field (saturation × brightness), a hue bar, and hex and RGB boxes. */
export const moreColors = (start: string, set: (c: string) => void) => openDialog(() => {
  const toHsv = (h: string) => { const [r, g, b] = rgb(/^#[0-9a-f]{6}$/i.test(h) ? h : "#0B5CAD").map(x => x / 255), mx = Math.max(r, g, b), mn = Math.min(r, g, b), d = mx - mn;
    const hue = d === 0 ? 0 : mx === r ? ((g - b) / d + 6) % 6 : mx === g ? (b - r) / d + 2 : (r - g) / d + 4; return [hue * 60, mx ? d / mx : 0, mx]; };
  const fromHsv = (h: number, s: number, v: number) => { const f = (n: number) => { const k = (n + h / 60) % 6; return v - v * s * Math.max(0, Math.min(k, 4 - k, 1)); }; return "#" + [f(5), f(3), f(1)].map(x => hex2(x * 255)).join(""); };
  const [hsv, setHsv] = createSignal(toHsv(start));
  const col = () => fromHsv(...(hsv() as [number, number, number]));
  const setHex = (h: string) => { if (/^#?[0-9a-f]{6}$/i.test(h.trim())) setHsv(toHsv(h.trim().startsWith("#") ? h.trim() : `#${h.trim()}`)); };
  const drag = (e: MouseEvent, f: (x: number, y: number) => void) => { const el = e.currentTarget as HTMLElement, r = el.getBoundingClientRect(); const mv = (m: MouseEvent) => f(Math.max(0, Math.min(1, (m.clientX - r.left) / r.width)), Math.max(0, Math.min(1, (m.clientY - r.top) / r.height))); mv(e); const up = () => { removeEventListener("mousemove", mv); removeEventListener("mouseup", up); }; addEventListener("mousemove", mv); addEventListener("mouseup", up); };
  return (
    <Dialog title="Colors" okText="OK" ok={() => set(col())}>
      <div class="mcol">
        <div class="sv" style={{ background: `linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, ${fromHsv(hsv()[0], 1, 1)})` }} onMouseDown={e => drag(e, (x, y) => setHsv([hsv()[0], x, 1 - y]))}>
          <i style={{ left: `${hsv()[1] * 100}%`, top: `${(1 - hsv()[2]) * 100}%` }} /></div>
        <div class="hue" onMouseDown={e => drag(e, (_x, y) => setHsv([Math.min(359.9, y * 360), hsv()[1], hsv()[2]]))}><i style={{ top: `${hsv()[0] / 3.6}%` }} /></div>
        <div class="mside">
          <div class="cmp"><span style={{ background: col() }} /><span style={{ background: start }} /></div><small class="muted">New · Current</small>
          <label>Hex <input value={col()} onChange={e => setHex(e.currentTarget.value)} /></label>
          <For each={["Red", "Green", "Blue"]}>{(l, i) => <label>{l} <input type="number" min="0" max="255" value={rgb(col())[i()]} onChange={e => { const v = rgb(col()); v[i()] = Math.max(0, Math.min(255, +e.currentTarget.value || 0)); setHsv(toHsv("#" + v.map(hex2).join(""))); }} /></label>}</For>
        </div>
      </div>
      <div class="cgrid wide"><For each={[...STANDARD, ...BASE]}>{c => <button class="csw" style={{ background: c }} title={c} onClick={() => setHsv(toHsv(c))} />}</For></div>
    </Dialog>
  );
});
