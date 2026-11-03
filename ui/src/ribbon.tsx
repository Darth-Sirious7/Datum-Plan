// Title bar, quick access toolbar, Tell Me and the ribbon.
import { linkSelected, linkedPlans, updateLinks, subplans } from "./plans";
import { For, Show, JSX, createSignal, createMemo, createEffect, onMount, onCleanup } from "solid-js";
import { setOut, call, out, tab, setTab, setBackstage, view, openMenu, Item, toast, fields, filter, setFilter, ALL, setGroup, group, setSort, sort, useTable, tables, table, setProjSum, projSum, showSubs, setShowSubs, setHideSum, hideSum, baseline, setBaseline, details, timeline, P, act, openDialog, closeDialog, setLevel, setZoom, saveCols, cols, theme, setTheme } from "./store";
import { C, ALL_CMDS } from "./cmds";
import * as D from "./dialogs";
import * as P2 from "./progress";
import { Icon } from "./ui";
import { STYLES, gstyle } from "./gantt";
import { showCrit, setShowCrit, showBase, setShowBase, showSlack, setShowSlack, showLate, setShowLate, showLabels, setShowLabels } from "./gantt";
import { fieldMenu } from "./grid";
import * as F from "./fmt";
import { Help, helpCard, HELP } from "./help";
import { sheetApi, CELL_STYLES, STYLE_SECTIONS } from "./sheets";
import { sheetMode, setSheetMode, setSheetK } from "./sheetlayout";
import type { Out } from "./api";
import { PAPERS, sheetPage } from "./print";
import type { SheetPage } from "./api";
import { askText } from "./ui";
import { colorPalette } from "./colors";

const GH = "https://github.com/Edgar-Mendonca/Datum-Plan";
export const FONTS = ["Aptos", "Calibri", "Arial", "Segoe UI", "Tahoma", "Verdana", "Times New Roman", "Cambria", "Georgia", "Garamond", "Consolas", "IBM Plex Sans"];
/** Spreadsheet-style Font and Alignment controls; they act on the selected cells of the task sheet or a sheet tab. */
const SIZES = [8, 9, 10, 11, 12, 13, 14, 16, 18, 20, 24, 28, 32, 36, 48];
const Font = (p0: { noAlign?: boolean }) => {
  const s = () => F.current(), [last, setLast] = createSignal<[string, string]>(["#C0392B", "#FFF2CC"]);
  const T = (p: { k: "b" | "i" | "u" | "s"; l: string; t: string; st: JSX.CSSProperties }) => <button class="rb fb" classList={{ on: !!s()[p.k] }} title={p.t} onClick={() => F.toggle(p.k)}><span style={p.st}>{p.l}</span></button>;
  const A = (p: { a: string; i: string; t: string }) => <button class="rb fb" classList={{ on: (s().a || "") === p.a }} title={p.t} onClick={() => F.setProp("a", p.a || undefined)}><Icon n={p.i} /></button>;
  return <>
    <G l="Font" h="font"><div class="fontg">
      <div><select class="ffam" title="Font" value={s().n || ""} onChange={e => F.setProp("n", e.currentTarget.value || undefined)}><option value="">Default font</option><For each={FONTS}>{f => <option value={f} style={{ "font-family": `'${f}'` }}>{f}</option>}</For></select>
        <select class="fsize" title="Font size" value={String(s().z || 12)} onChange={e => F.setProp("z", +e.currentTarget.value === 12 ? undefined : +e.currentTarget.value)}><For each={SIZES}>{z => <option value={z}>{z}</option>}</For></select>
        <button class="rb fb" title="Increase font size" onClick={() => F.setProp("z", SIZES.find(z => z > (s().z || 12)) ?? 48)}><span>A<sup>+</sup></span></button><button class="rb fb" title="Decrease font size" onClick={() => F.setProp("z", [...SIZES].reverse().find(z => z < (s().z || 12)) ?? 8)}><span>A<sup>−</sup></span></button></div>
      <div><T k="b" l="B" t="Bold (Ctrl+B)" st={{ "font-weight": 700 }} /><T k="i" l="I" t="Italic (Ctrl+I)" st={{ "font-style": "italic" }} /><T k="u" l="U" t="Underline (Ctrl+U)" st={{ "text-decoration": "underline" }} /><T k="s" l="S" t="Strikethrough" st={{ "text-decoration": "line-through" }} />
        <span class="fcol"><button class="rb fb" title="Font color" onClick={() => F.setProp("c", last()[0])}><span class="ca">A<i style={{ background: last()[0] }} /></span></button><button class="rb sarr" title="Font Color" onClick={e => colorPalette(e, F.current().c, v => { F.setProp("c", v || undefined); if (v) setLast([v, last()[1]]); }, "Automatic")}>▾</button></span>
        <span class="fcol"><button class="rb fb" title="Fill color" onClick={() => F.setProp("f", last()[1])}><span class="ca"><Icon n="bucket" /><i style={{ background: last()[1] }} /></span></button><button class="rb sarr" title="Fill Color" onClick={e => colorPalette(e, F.current().f, v => { F.setProp("f", v || undefined); if (v) setLast([last()[0], v]); }, "No Fill")}>▾</button></span></div>
    </div></G>
    <Show when={!p0.noAlign}><G l="Alignment" h="alignment"><div class="fontg"><div><A a="" i="al" t="Align left" /><A a="c" i="ac" t="Center" /><A a="r" i="ar" t="Align right" /></div><div><button class="rb fb" title="Clear Formatting" onClick={F.clearFmt}><Icon n="clear" /></button></div></div></G></Show>
  </>;
};

const B = (p: { i: string; l: string; run?: (e: MouseEvent) => void; big?: boolean; on?: boolean; off?: boolean; dd?: (e: MouseEvent) => void; title?: string }) => p.big && p.run && p.dd ? (
  // Split button: the picture runs the command, the label opens the menu.
  <div class="rbw big splitbig" classList={{ on: p.on }}>
    <button class="rb top" title={p.title ?? p.l} onClick={e => p.run!(e)}><Icon n={p.i} /></button>
    <button class="rb bot" title={`${p.l} options`} onClick={e => p.dd!(e)}><span>{p.l}</span><b class="car">▾</b></button>
  </div>
) : (
  <div class="rbw" classList={{ big: p.big, sp: !p.big && !!p.run && !!p.dd }}>
    <button class="rb" classList={{ big: p.big, on: p.on }} disabled={p.off} title={p.title ?? p.l} onClick={e => (p.run ?? p.dd)?.(e)}><Icon n={p.i} />{p.big ? <span>{p.l}{p.dd && !p.run ? " ▾" : ""}</span> : <span>{p.l}</span>}{!p.big && p.dd && !p.run && <b class="car">▾</b>}</button>
    {p.run && p.dd && <button class="rb sarr" title={`${p.l} options`} onClick={e => p.dd!(e)}>▾</button>}
  </div>
);
const G = (p: { l: string; h?: string; children: JSX.Element }) => <div class="grp"><div class="items">{p.children}</div><div class="cap">{p.l}{p.h && <Help k={p.h} />}</div></div>;
const S = (p: { children: JSX.Element }) => <div class="stack">{p.children}</div>;

const views: [string, string, Parameters<typeof C.view>[0]][] = [
  ["Gantt Chart", "gantt", "gantt"], ["Tracking Gantt", "base", "tracking"], ["Network Diagram", "net", "network"], ["Calendar", "cal", "calendar"], ["Task Usage", "table", "taskusage"],
  ["Timeline", "tl", "timeline"], ["Resource Sheet", "res", "ressheet"], ["Resource Usage", "load", "resusage"], ["Resource Graph", "load", "resgraph"], ["Team Planner", "team", "team"],
];
const viewMenu = (): Item[] => views.map(([l, , v]) => ({ label: l, run: () => C.view(v), on: view() === v }));
const filterMenu = (): Item[] => [
  ...fields().filters.map(f => ({ label: f.name, run: () => setFilter(f.rules.length ? f : ALL), on: filter().name === f.name })),
  ...((P().ui?.filters ?? []) as any[]).map(f => ({ label: `${f.name} (saved)`, run: () => setFilter(f), on: filter().name === f.name })),
  "-", { label: "Clear Filter", run: () => setFilter(ALL) }, { label: "New Filter…", run: () => D.filterEditor() },
  { label: "Display Related Summary Tasks", run: () => setFilter({ ...filter(), summaries: !filter().summaries }), on: filter().summaries },
  { label: "Highlight Instead of Filter", run: () => setFilter({ ...filter(), highlight: !filter().highlight }), on: filter().highlight },
];
const GROUPS: [string, string[]][] = [["Critical", ["critical"]], ["Duration", ["dur"]], ["Milestones", ["milestone"]], ["Priority", ["priority"]], ["Resource", ["res"]], ["Status", ["status"]], ["Constraint Type", ["ctype"]], ["Activity Type", ["acttype"]], ["Responsible", ["responsible"]]];
const groupMenu = (): Item[] => [
  { label: "No Group", run: () => setGroup([]), on: !group().length }, ...GROUPS.map(([l, g]) => ({ label: l, run: () => setGroup(g), on: group().join() === g.join() })),
  ...P().codes.map(c => ({ label: c.name, run: () => setGroup([`code:${c.name}`]) })), "-", { label: "Group by Field", sub: fieldMenu(k => setGroup([k])) }, { label: "New Group By…", run: () => D.groupEditor() },
];
const sortMenu = (): Item[] => [
  ...[["ID", "id"], ["Start Date", "start"], ["Finish Date", "finish"], ["Priority", "priority"], ["Cost", "cost"], ["Total Float", "tf"], ["Activity ID", "code"]].map(([l, k]) => ({ label: `by ${l}`, run: () => setSort(k === "id" ? [] : [[k, k === "cost" || k === "priority"]]), on: (sort()[0]?.[0] ?? "id") === k })),
  "-", { label: "Sort by…", run: () => D.sortEditor() },
];
const tableMenu = (): Item[] => [...Object.keys(tables()).map(t => ({ label: t, run: () => useTable(t), on: table() === t })), "-", { label: "More Tables…", run: () => D.tablesDialog() }];
const outlineMenu = (): Item[] => [{ label: "Show Subtasks", run: C.expandAll }, { label: "Hide Subtasks", run: C.collapseAll }, "-", ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map(n => ({ label: `Level ${n}`, run: () => C.outline(n) })), { label: "All Subtasks", run: () => setLevel(0) }];
const zoomMenu = (): Item[] => [
  ...([["Days", 40], ["Weeks", 14], ["Months", 4], ["Quarters", 1.6], ["Years", 0.5]] as [string, number][]).map(([l, z]) => ({ label: l, run: () => setZoom(z) })),
  "-", { label: "Zoom In", run: C.zoomIn }, { label: "Zoom Out", run: C.zoomOut }, { label: "Entire Project", run: C.fit },
];
const exportMenu = (): Item[] => [["Excel workbook (.xlsx)", "xlsx"], ["Project XML (.xml)", "xml"], ["XER (.xer)", "xer"], ["PMXML (.xml)", "pmxml"], ["MPX (.mpx)", "mpx"], ["CSV (.csv)", "csv"], ["Web page (.html)", "html"], ["iCalendar (.ics)", "ics"], ["JSON (.json)", "json"]].map(([l, k]) => ({ label: l, run: () => C.exportAs(k) }));
const baselineMenu = (): Item[] => Array.from({ length: 11 }, (_, k) => ({ label: k ? `Baseline ${k}` : "Baseline", run: () => { setBaseline(k); setShowBase(true); }, on: baseline() === k && showBase() }));

/** Tell Me / command search (Ctrl+K, Alt+Q). */
export const palette = () => openDialog(() => {
  const [q, setQ] = createSignal(""), [k, setK] = createSignal(0);
  const list = createMemo(() => { const s = q().toLowerCase(); return ALL_CMDS.filter(c => c[0].toLowerCase().includes(s)).slice(0, 14); });
  const go = (i: number) => { const c = list()[i]; if (c) { closeDialog(); setTimeout(() => c[1]()); } };
  let input!: HTMLInputElement;
  onMount(() => input.focus());
  return (
    <div class="ov high" onMouseDown={e => e.target === e.currentTarget && closeDialog()}>
      <div class="pal">
        <input ref={input} placeholder="Tell me what you want to do" value={q()} onInput={e => { setQ(e.currentTarget.value); setK(0); }}
          onKeyDown={e => { if (e.key === "ArrowDown") { e.preventDefault(); setK(Math.min(list().length - 1, k() + 1)); } else if (e.key === "ArrowUp") { e.preventDefault(); setK(Math.max(0, k() - 1)); } else if (e.key === "Enter") { e.preventDefault(); go(k()); } else if (e.key === "Escape") closeDialog(); }} />
        <div class="pl"><For each={list()}>{(c, i) => <button classList={{ on: i() === k() }} onMouseEnter={() => setK(i())} onClick={() => go(i())}><span>{c[0]}</span>{c[2] && <kbd>{c[2]}</kbd>}</button>}</For></div>
      </div>
    </div>
  );
});

// Sheets opens its own Home tab; leaving Sheets goes back to the Task tab.
let lastView = "";
const followView = () => { const v = view(); if (v === lastView) return; const was = lastView; lastView = v; if (v === "sheets") setTab("sheet"); else if (was === "sheets" && (tab() === "sheet" || tab() === "sheetpage")) setTab("task"); };
export const TitleBar = (p: { update: string | null }) => (void createEffect(followView),
  <header class="top">
    <div class="bar">
      <div class="qat">
        <img src="logo.svg" alt="" class="logo" />
        <button title="Save (Ctrl+S)" onClick={() => C.save()}><Icon n="save" /></button>
        <button title="Undo (Ctrl+Z)" disabled={!out()?.undo} onClick={C.undo}><Icon n="undo" /></button>
        <button title="Redo (Ctrl+Y)" disabled={!out()?.redo} onClick={C.redo}><Icon n="redo" /></button>
        <button title="Print (Ctrl+P)" onClick={C.print}><Icon n="print" /></button>
      </div>
      <div class="title">{out() ? `${out()!.project.name}${out()!.dirty ? " •" : ""} — Datum Plan` : "Datum Plan"}</div>
      <button class="search" onClick={palette} title="Tell me what you want to do (Alt+Q)"><Icon n="search" /><span>Tell me what you want to do</span><kbd>Ctrl+K</kbd></button>
      <Show when={p.update}><button class="upd" onClick={() => setBackstage("about")}>Update {p.update} available</button></Show>
      <button class="theme" title="Theme: Auto follows Windows" onClick={() => setTheme(theme() === "auto" ? "light" : theme() === "light" ? "dark" : "auto")}>Theme · {theme()}</button>
    </div>
    <nav class="tabs">
      <button class="file" onClick={() => setBackstage("home")}>File</button>
      <For each={[["task", "Task"], ["schedule", "Schedule"], ["progress", "Progress"], ["resource", "Resource"], ["report", "Report"], ["project", "Project"], ["view", "View"], ["format", "Format"], ["sheet", "Home"], ["help", "Help"]]}>{([k, l]) =>
        k === "sheet" ? <Show when={view() === "sheets"}><div class="ctxgrp sheetctx"><small>Sheet Tools</small><div class="ctxtabs"><button classList={{ on: tab() === "sheet" }} onClick={() => setTab("sheet")}>Cells</button><button classList={{ on: tab() === "sheetpage" }} onClick={() => setTab("sheetpage")}>Page</button></div></div></Show>
          : k === "format" ? <div class="ctxgrp"><small>Gantt Chart Tools</small><button classList={{ on: tab() === k }} onClick={() => setTab(k)}>{l}</button></div>
          : <button classList={{ on: tab() === k }} onClick={() => setTab(k)}>{l}</button>}</For>
    </nav>
  </header>
);

/** Shrinks the ribbon in steps (labels of small buttons, then of big ones) instead of scrolling sideways. */
const fitRibbon = (el: HTMLElement) => {
  // Tighten everything first, then fold groups from the right: small-button labels, then big-button labels.
  const fit = () => {
    const gs = [...el.querySelectorAll<HTMLElement>(":scope > .grp")];
    el.dataset.fit = "0"; gs.forEach(g => delete g.dataset.fit);
    const over = () => el.scrollWidth > el.clientWidth + 1;
    if (!over()) return;
    el.dataset.fit = "1";
    for (const lvl of ["2", "3"]) for (let i = gs.length - 1; i >= 0 && over(); i--) gs[i].dataset.fit = lvl;
  };
  new ResizeObserver(fit).observe(el);
  new MutationObserver(() => requestAnimationFrame(fit)).observe(el, { childList: true, subtree: false });
};
/** The cell style gallery: every style drawn as it looks, in sections, under the button. */
const styleGallery = (e: MouseEvent, apply: (st: string) => void) => {
  const b = (e.currentTarget as HTMLElement).getBoundingClientRect();
  openDialog(() => {
    let el!: HTMLDivElement;
    const key = (k: KeyboardEvent) => { if (k.key === "Escape") { k.stopPropagation(); closeDialog(); } };
    onMount(() => { document.addEventListener("keydown", key, true); const r = el.getBoundingClientRect(); if (r.right > innerWidth - 4) el.style.left = `${Math.max(4, innerWidth - r.width - 4)}px`; });
    onCleanup(() => document.removeEventListener("keydown", key, true));
    return (
      <div class="cpov" onMouseDown={ev => ev.target === ev.currentTarget && closeDialog()}>
        <div class="sgal" ref={el} style={{ left: `${b.left}px`, top: `${b.bottom + 2}px` }}>
          <For each={STYLE_SECTIONS}>{([title, list]) => <section><small>{title}</small><div>
            <For each={list}>{([l, st]) => <button class="cst" style={F.css(st)} title={l} onClick={() => { closeDialog(); apply(st); }}>{l}</button>}</For></div></section>}</For>
          <section><small>Numbers</small><div>
            <For each={[["Comma", "#,##0.00"], ["Whole", "#,##0"], ["Currency", "money"], ["Percent", '0"%"'], ["Date", "date"]]}>{([l, f]) => <button class="cst num" onClick={() => { closeDialog(); sheetApi()?.setFmt(f); }}>{l}</button>}</For></div></section>
        </div>
      </div>
    );
  });
};
/** Brings a workbook's sheets in as tabs (one step to undo). */
export const importWorkbook = async () => {
  const r = await call<[Out, string[]]>("import_workbook");
  if (r) { setOut(r[0]); const sh = P().sheets; setSheetK(Math.max(0, sh.length - r[1].length)); toast(`Brought in ${r[1].length} sheet${r[1].length > 1 ? "s" : ""} from the workbook: ${r[1].join(", ")}.`, true); }
};
/** Saves the plan's sheets as a workbook to share. */
export const exportSheets = async () => { const p = await call<string | null>("export_sheets"); if (p) toast(`Saved ${p}`, true); };
/** Sheet Tools › Cells: everything for the cells of a sheet. It opens by itself in Sheets. */
const SheetCells = () => {
  const A = () => sheetApi(), s = () => F.current(), run = (n: string) => (e?: MouseEvent) => A()?.[n]?.(e);
  const AL = (p: { k: "a" | "v"; v: string; i: string; t: string }) => <button class="rb fb" classList={{ on: (s()[p.k] || "") === p.v }} title={p.t} onClick={() => F.setProp(p.k, p.v || undefined)}><Icon n={p.i} /></button>;
  const QUICK = ["Good", "Bad", "Neutral", "Accent"];
  return <>
    <G l="Clipboard" h="clipboard"><B big i="paste" l="Paste" run={run("paste")} dd={e => openMenu(e, [{ label: "Paste", icon: "paste", run: run("paste"), keys: "Ctrl+V" }, { label: "Paste Special…", icon: "paste", run: () => D.pasteSpecial(), keys: "Ctrl+Alt+V" }, "-", { label: "Paste Values", run: () => F.target()?.paste?.("values", false) }, { label: "Paste Formatting", icon: "paint", run: () => F.target()?.paste?.("formats", false) }, { label: "Transpose", run: () => F.target()?.paste?.("all", true) }])} />
      <S><B i="cut" l="Cut" run={run("cut")} /><B i="copy" l="Copy" run={run("copy")} /><B i="paint" l="Format Painter" run={F.paint} /></S></G>
    <Font noAlign />
    <G l="Align" h="sheetalign"><div class="fontg">
      <div><AL k="v" v="t" i="vt" t="Top Align" /><AL k="v" v="" i="vm" t="Middle Align" /><AL k="v" v="b" i="vb" t="Bottom Align" /><span class="vsep" />
        <button class="rb fb" title="Decrease Indent" onClick={() => F.setProp("x", Math.max(0, (s().x || 0) - 1) || undefined)}><Icon n="outdent" /></button><button class="rb fb" title="Increase Indent" onClick={() => F.setProp("x", Math.min(15, (s().x || 0) + 1))}><Icon n="indent" /></button></div>
      <div><AL k="a" v="" i="al" t="Align Left" /><AL k="a" v="c" i="ac" t="Center" /><AL k="a" v="r" i="ar" t="Align Right" /><span class="vsep" />
        <button class="rb fb" title="Clear Formatting" onClick={F.clearFmt}><Icon n="clear" /></button></div></div>
      <S><B i="wrap" l="Wrap Text" on={!!s().w} run={() => F.setProp("w", !s().w || undefined)} />
        <B i={A()?.merged() ? "unmerge" : "merge"} l={A()?.merged() ? "Unmerge Cells" : "Merge & Center"} run={() => A()?.merge(A()?.merged() ? "unmerge" : "center")} dd={e => openMenu(e, [{ label: "Merge & Center", icon: "merge", run: () => A()?.merge("center") }, { label: "Merge Across", icon: "mergeacross", run: () => A()?.merge("across") }, { label: "Merge Cells", icon: "merge", run: () => A()?.merge("cells") }, { label: "Unmerge Cells", icon: "unmerge", run: () => A()?.merge("unmerge") }])} /></S></G>
    <G l="Lines" h="sheetalign"><B big i="bd-bottom" l="Borders" run={() => A()?.border("bottom")} dd={e => openMenu(e, (A()?.borderItems() ?? []) as Item[])} title="Bottom border (the arrow: every border, line colour and style)" />
      <S><B i="bd-all" l="All Borders" run={() => A()?.border("all")} /><B i="bd-outside" l="Outside" run={() => A()?.border("outside")} /><B i="bd-none" l="No Border" run={() => A()?.border("none")} /></S></G>
    <G l="Number" h="sheetnumber"><div class="fontg">
      <div><button class="rb numfmt" title="Number format" onClick={e => openMenu(e, (A()?.fmtItems() ?? []) as Item[])}>{(A()?.fmtLabel() as string) ?? "General"} ▾</button></div>
      <div><button class="rb fb" title="Currency" onClick={() => A()?.setFmt("money")}>$</button><button class="rb fb" title="Percent Style (28 → 28%)" onClick={() => A()?.setFmt('0"%"')}>%</button><button class="rb fb" title="Comma Style (1,234.50)" onClick={() => A()?.setFmt("#,##0.00")}>,</button>
        <button class="rb fb" title="Increase Decimal" onClick={() => A()?.moreDecimals(1)}>.0+</button><button class="rb fb" title="Decrease Decimal" onClick={() => A()?.moreDecimals(-1)}>.0−</button></div></div></G>
    <G l="Look" h="sheetstyles"><B big i="cond" l="Conditional" title="Conditional formatting: colour cells by their values, data bars, colour scales" dd={e => A()?.condMenu(e)} /><div class="cstyles"><For each={CELL_STYLES.filter(x => QUICK.includes(x[0]))}>{([l, st]) => <button class="cst" style={F.css(st)} title={`Cell style: ${l}`} onClick={() => A()?.cellStyle(st)}>{l}</button>}</For>
      <button class="rb sarr" title="Cell Styles: the whole gallery" onClick={e => styleGallery(e, st => A()?.cellStyle(st))}>▾</button></div></G>
    <G l="Rows & Columns" h="sheetcells"><B big i="task" l="Insert" dd={e => openMenu(e, [{ label: "Insert Rows Above", run: () => A()?.insertCells(false, false) }, { label: "Insert Columns Left", run: () => A()?.insertCells(false, true) }, "-", { label: "Insert Sheet", run: run("addSheet") }])} />
      <B big i="del" l="Delete" dd={e => openMenu(e, [{ label: "Delete Rows", run: () => A()?.insertCells(true, false) }, { label: "Delete Columns", run: () => A()?.insertCells(true, true) }, "-", { label: "Delete Sheet", run: run("del") }])} />
      <B big i="rowh" l="Size" dd={e => openMenu(e, [{ label: "Row Height…", run: run("rowHeight") }, { label: "AutoFit Row Height", run: run("autofitRows") }, { label: "Column Width…", run: run("colWidth") }, { label: "AutoFit Column Width", run: run("autofitCols") }, "-", { label: "Rename Sheet", run: run("rename") }, { label: "Move or Copy Sheet…", run: run("moveCopy") }, { label: "Tab Color, Hide and Unhide", run: run("tabMenu") }])} /></G>
    <G l="Insert" h="sheets"><B big i="image" l="Picture" dd={run("picture")} title="Place a logo or picture from the plan's library on the selected cell" /><S><B i="fx" l="Function" run={run("insertFn")} /><B i="table" l="Plan Data" run={run("planData")} /><B i="import" l="Excel Workbook…" title="Bring the sheets of a workbook (.xlsx, .xls, .ods) in as sheet tabs: values, formulas, formats, styles, merged cells, sizes, pictures" run={importWorkbook} /></S></G>
    <G l="Data" h="sheetediting"><S><B i="fx" l="AutoSum" run={() => A()?.autoSum("SUM")} dd={e => openMenu(e, (A()?.sumItems() ?? []) as Item[])} /><B i="filldown" l="Fill" dd={e => openMenu(e, [{ label: "Down", keys: "Ctrl+D", run: run("fillDown") }, { label: "Right", keys: "Ctrl+R", run: run("fillRight") }])} /><B i="clear" l="Clear" dd={e => openMenu(e, [{ label: "Clear All", run: () => A()?.clearKind("all") }, { label: "Clear Formats", run: () => A()?.clearKind("formats") }, { label: "Clear Contents", keys: "Delete", run: () => A()?.clearKind("contents") }])} /></S>
      <S><B i="az" l="Sort A to Z" run={() => A()?.sortRows(true)} /><B i="za" l="Sort Z to A" run={() => A()?.sortRows(false)} /><B i="find" l="Find" dd={e => openMenu(e, [{ label: "Find…", keys: "Ctrl+F", run: () => A()?.find() }, { label: "Replace…", keys: "Ctrl+H", run: () => A()?.replace() }])} /></S></G>
  </>;
};
/** Sheet Tools › Page: how the sheet prints, and the page views. Each sheet keeps its own page setup. */
const SheetPageTab = () => {
  const A = () => sheetApi(), pg = () => A()?.page() ?? {}, set = (x: Partial<SheetPage>) => A()?.setPage(x);
  const eff = () => sheetPage();
  const Ck = (p: { k: "no_hf" | "no_grid" | "headings" | "center"; l: string; inv?: boolean; t: string }) => <label class="chk" title={p.t}><input type="checkbox" checked={p.inv ? !pg()[p.k] : !!pg()[p.k]} onChange={e => set({ [p.k]: p.inv ? !e.currentTarget.checked : e.currentTarget.checked })} />{p.l}</label>;
  const MARGINS: [string, number[]][] = [["Normal (19 mm, 18 mm sides)", [19, 18, 19, 18]], ["Narrow (13 mm)", [13, 6, 13, 6]], ["Wide (25 mm)", [25, 25, 25, 25]], ["None (5 mm)", [5, 5, 5, 5]]];
  return <>
    <G l="Views" h="sheetviews"><B big i="vnormal" l="Normal" on={sheetMode() === "normal"} run={() => setSheetMode("normal")} /><B big i="vlayout" l="Page Layout" on={sheetMode() === "layout"} run={() => setSheetMode("layout")} /><B big i="vbreaks" l="Page Breaks" on={sheetMode() === "breaks"} run={() => setSheetMode("breaks")} />
      <S><B i="fill" l={A()?.frozen() ? "Unfreeze Panes" : "Freeze Panes"} run={() => A()?.freeze()} /></S></G>
    <G l="Paper" h="sheetviews"><B big i="pagesetup" l={`Size: ${eff()?.paper || P().ui?.page?.paper || "A4"}`} dd={e => openMenu(e, [{ label: "As the plan", on: !pg().paper, run: () => set({ paper: "" }) }, ...PAPERS.map(([n, w, h]) => ({ label: `${n} (${w} × ${h} mm)`, on: pg().paper === n, run: () => set({ paper: n }) }))])} />
      <B big i="vlayout" l={pg().orient === "landscape" ? "Landscape" : pg().orient === "portrait" ? "Portrait" : "Orientation"} dd={e => openMenu(e, [{ label: "As the plan", on: !pg().orient, run: () => set({ orient: "" }) }, { label: "Portrait", on: pg().orient === "portrait", run: () => set({ orient: "portrait" }) }, { label: "Landscape", on: pg().orient === "landscape", run: () => set({ orient: "landscape" }) }])} />
      <B big i="cols" l="Margins" dd={e => openMenu(e, [{ label: "As the plan", on: !pg().margins, run: () => set({ margins: null }) }, ...MARGINS.map(([l, m]) => ({ label: l, on: JSON.stringify(pg().margins) === JSON.stringify(m), run: () => set({ margins: m }) })), "-", { label: "Custom Margins…", run: async () => { const v = await askText("Margins", "Top, right, bottom, left (mm)", (pg().margins ?? [15, 12, 15, 12]).join(", ")); const m = v?.split(/[ ,;]+/).map(Number); if (m?.length === 4 && m.every(x => x >= 0 && x <= 100)) set({ margins: m }); } }])} /></G>
    <G l="What Prints" h="sheetviews"><S><B i="table" l="Print Area" dd={e => openMenu(e, [{ label: "Set to the Selection", run: () => A()?.areaFromSel() }, { label: "Clear Print Area", off: !pg().area, run: () => set({ area: "" }) }, ...(pg().area ? ["-" as const, { label: `Now: ${pg().area}`, off: true }] : [])])} />
      <B i="repeat" l="Titles" title="Rows printed at the top of every page" dd={e => openMenu(e, [{ label: "Repeat the Selected Rows on Every Page", run: () => A()?.titlesFromSel() }, { label: "No Title Rows", off: !pg().titles, run: () => set({ titles: "" }) }, ...(pg().titles ? ["-" as const, { label: `Now: rows ${pg().titles}`, off: true }] : [])])} />
      <B i="vbreaks" l="Breaks" dd={e => openMenu(e, [{ label: "New Page at This Row", run: () => A()?.breakHere() }, { label: "Remove Break at This Row", run: () => A()?.unbreakHere() }, { label: "Remove All Breaks", off: !pg().breaks?.length, run: () => set({ breaks: [] }) }])} /></S></G>
    <G l="Fit" h="sheetviews"><S>
      <B i="fit" l={`Width: ${pg().fit_w ? `${pg().fit_w} page${pg().fit_w! > 1 ? "s" : ""}` : "Automatic"}`} dd={e => openMenu(e, [{ label: "Automatic", on: !pg().fit_w, run: () => set({ fit_w: 0 }) }, ...[1, 2, 3, 4].map(n => ({ label: `${n} page${n > 1 ? "s" : ""}`, on: pg().fit_w === n, run: () => set({ fit_w: n }) }))])} />
      <B i="fit" l={`Height: ${pg().fit_h ? `${pg().fit_h} page${pg().fit_h! > 1 ? "s" : ""}` : "Automatic"}`} dd={e => openMenu(e, [{ label: "Automatic", on: !pg().fit_h, run: () => set({ fit_h: 0 }) }, ...[1, 2, 3, 4].map(n => ({ label: `${n} page${n > 1 ? "s" : ""}`, on: pg().fit_h === n, run: () => set({ fit_h: n }) }))])} />
      <B i="zin" l={`Scale: ${pg().scale || 100} %`} off={!!(pg().fit_w || pg().fit_h)} dd={e => openMenu(e, [50, 70, 80, 90, 100, 110, 125, 150].map(n => ({ label: `${n} %`, on: (pg().scale || 100) === n, run: () => set({ scale: n === 100 ? 0 : n }) })))} /></S></G>
    <G l="On the Paper" h="sheetviews"><div class="chkcol"><Ck k="no_hf" inv l="Header & footer" t="The plan's header (logos, title) and footer on each page; untick to print the cells only" /><Ck k="no_grid" inv l="Gridlines" t="Light lines round every cell" /></div>
      <div class="chkcol"><Ck k="headings" l="Row & column labels" t="Row numbers and column letters" /><Ck k="center" l="Centre on page" t="Centre the cells across the paper" /></div></G>
    <G l="Print & Share" h="report"><B big i="print" l="Print Preview" run={C.print} /><B big i="export" l="Save as Excel" title="Save the sheets as an Excel workbook to share; formulas, formats, styles, merged cells, pictures and page setup go with them" run={exportSheets} /></G>
  </>;
};
export const Ribbon = () => (
  <section class="ribbon" ref={fitRibbon}>
    <Show when={tab() === "task"}>
      <G l="View" h="view"><B big i="gantt" l="Gantt Chart" run={() => C.view("gantt")} dd={e => openMenu(e, viewMenu())} /></G>
      <G l="Clipboard" h="clipboard"><B big i="paste" l="Paste" run={C.paste} dd={e => openMenu(e, [{ label: "Paste", icon: "paste", run: C.paste, keys: "Ctrl+V" }, { label: "Paste Special…", icon: "paste", run: () => D.pasteSpecial(), keys: "Ctrl+Alt+V" }, "-", { label: "Paste Values", run: () => F.target()?.paste?.("values", false) }, { label: "Paste Formatting", icon: "paint", run: () => F.target()?.paste?.("formats", false) }, { label: "Transpose", run: () => F.target()?.paste?.("all", true) }])} /><S><B i="cut" l="Cut" run={C.cut} /><B i="copy" l="Copy" run={C.copy} /><B i="paint" l="Format Painter" run={F.paint} on={F.painter() !== null} /></S></G>
      <Font />
      <G l="Insert" h="insert"><B big i="task" l="Task" run={() => C.newTask()} dd={e => openMenu(e, [{ label: "Task", run: () => C.newTask() }, { label: "Blank Row", run: () => C.newTask() }, { label: "Summary Task", run: C.summary }, { label: "Milestone", run: () => C.newTask(true) }, { label: "Recurring Task…", icon: "repeat", run: C.recurring }])} />
        <S><B i="proj" l="Summary" run={C.summary} /><B i="ms" l="Milestone" run={() => C.newTask(true)} /><B i="repeat" l="Recurring" run={C.recurring} /></S><S><B i="del" l="Delete" run={C.del} /></S></G>
      <G l="Schedule" h="sched"><div class="pcts"><For each={[0, 25, 50, 75, 100]}>{v => <button class="rb" title={`Mark the selected tasks ${v}% complete`} onClick={() => C.pct(v)}><Icon n="pct" /><span>{v}%</span></button>}</For></div>
        <S><B i="track" l="Mark on Track" run={C.onTrack} /><B i="link" l="Link" title="Link the selected tasks Finish-to-Start (Ctrl+F2)" run={C.link} /><B i="unlink" l="Unlink" run={C.unlink} /></S><S><B i="outdent" l="Outdent" run={C.outdent} /><B i="indent" l="Indent" run={C.indent} /></S></G>
      <G l="Properties" h="properties"><B big i="info" l="Information" run={C.info} /><S><B i="note" l="Notes" run={C.notes} /><B i="detail" l="Details" run={C.details} on={details()} /><B i="tl" l="Add to Timeline" run={C.timeline} /></S></G>
      <G l="Editing" h="editing"><S><B i="find" l="Find" run={palette} /><B i="scroll" l="Scroll to Task" run={C.scrollTo} /><B i="filldown" l="Fill Down" title="Copy the top cell of the selection down (Ctrl+D)" run={() => document.querySelector(".gscroll")?.dispatchEvent(new KeyboardEvent("keydown", { key: "d", ctrlKey: true, bubbles: true }))} /></S></G>
    </Show>
    <Show when={tab() === "schedule"}>
      <G l="Links" h="links"><B big i="link" l="Link Tasks" run={C.link} /><S><B i="unlink" l="Unlink" run={C.unlink} /><B i="link" l="Relationships" run={() => C.view("relations")} /></S></G>
      <G l="Outline" h="outline"><S><B i="outdent" l="Outdent" run={C.outdent} /><B i="indent" l="Indent" run={C.indent} /><B i="list" l="Outline Level" dd={e => openMenu(e, outlineMenu())} /></S><S><B i="up" l="Move Up" run={() => C.move(true)} /><B i="down" l="Move Down" run={() => C.move(false)} /></S></G>
      <G l="Tasks" h="tasks"><B big i="manual" l="Manually Schedule" run={() => C.mode(true)} /><B big i="auto" l="Auto Schedule" run={() => C.mode(false)} /><S><B i="split" l="Split" run={C.split} /><B i="inact" l="Inactivate" run={C.inactive} /></S></G>
      <G l="Leveling" h="leveling"><B big i="level" l="Level All" run={() => C.level()} /><S><B i="level" l="Level Selection" run={() => C.level(false)} /><B i="del" l="Clear Leveling" run={C.clearLevel} /></S></G>
      <G l="Paths" h="paths"><S><B i="crit" l="Critical Path" run={() => { setShowCrit(true); setFilter(fields().filters.find(f => f.name === "Critical") ?? ALL); }} /><B i="crit" l="Longest Path" run={() => setFilter(fields().filters.find(f => f.name === "Longest Path") ?? ALL)} /><B i="check" l="Check Schedule" run={() => C.view("checks")} /></S></G>
      <G l="Editing" h="change"><S><B i="find" l="Find" run={palette} /><B i="scroll" l="Scroll to Task" run={C.scrollTo} /><B i="search" l="Inspect" run={() => D.inspect(C.ids()[0])} /></S><S><B i="change" l="Global Change" run={C.globalChange} /><B i="opts" l="Schedule Options" run={() => D.scheduleOptions()} /></S></G>
    </Show>
    <Show when={tab() === "progress"}>
      <G l="Update" h="update"><div class="pcts"><For each={[0, 25, 50, 75, 100]}>{v => <button class="rb" title={`${v}% complete`} onClick={() => C.pct(v)}><Icon n="pct" /><span>{v}%</span></button>}</For></div>
        <S><B i="track" l="Mark on Track" run={C.onTrack} /><B i="info" l="Update Tasks" run={C.info} /><B i="track" l="Update Project" run={C.update} /></S></G>
      <G l="Period" h="period"><B big i="update" l="Update Progress" run={() => C.view("update")} /><B big i="cal" l="Look-ahead" run={() => C.view("lookahead")} /><S><B i="calendar2" l="Status Date" run={() => D.projectInfo()} /><B i="calendar2" l="Reporting Periods" run={() => D.periodSettings()} /></S></G>
      <G l="Baselines" h="baselines"><B big i="base" l="Set Baseline" run={C.baseline} /><S><B i="report" l="Store Snapshot" run={() => D.snapshot()} /><B i="base" l="Show Baseline" dd={e => openMenu(e, baselineMenu())} on={showBase()} /><B i="base" l="Tracking Gantt" run={() => C.view("tracking")} /></S></G>
      <G l="Progress Reports" h="progressReports"><B big i="track" l="Progress Report" run={() => C.view("progress")} /><B big i="ev" l="S-Curves" run={() => C.view("scurve")} /><B big i="team" l="Manpower" run={() => C.view("manpower")} /><B big i="dash" l="Dashboard" run={() => C.view("dashboard")} /></G>
      <G l="Measure" h="credit"><B big i="list" l="Rules of Credit" run={() => P2.rulesOfCredit()} /></G>
    </Show>
    <Show when={tab() === "resource"}>
      <G l="View" h="resviews"><B big i="res" l="Resource Sheet" run={() => C.view("ressheet")} dd={e => openMenu(e, viewMenu())} /><B big i="team" l="Team Planner" run={() => C.view("team")} /></G>
      <G l="Assignments" h="assignments"><B big i="res" l="Assign Resources" run={() => C.info()} /><S><B i="res" l="Resource Pool" run={() => C.view("ressheet")} /><B i="load" l="Resource Usage" run={() => C.view("resusage")} /><B i="load" l="Resource Graph" run={() => C.view("resgraph")} /></S></G>
      <G l="Insert" h="assignments"><B big i="task" l="Add Resource" run={() => act({ ResInsert: { name: `Resource ${P().resources.length + 1}` } }).then(() => C.view("ressheet"))} /></G>
      <G l="Level" h="leveling"><B big i="level" l="Level All" run={() => C.level()} /><S><B i="level" l="Level Selection" run={() => C.level(false)} /><B i="del" l="Clear Leveling" run={C.clearLevel} /><B i="opts" l="Leveling Options" run={() => D.scheduleOptions()} /></S></G>
    </Show>
    <Show when={tab() === "report"}>
      <G l="View Reports" h="reports"><B big i="dash" l="Dashboard" run={() => C.view("dashboard")} /><B big i="track" l="Progress Report" run={() => C.view("progress")} /><B big i="ev" l="S-Curves" run={() => C.view("scurve")} /><B big i="builder" l="Report Builder" run={() => C.view("builder")} /><B big i="report" l="Reports" run={() => C.view("reports")} /></G>
      <G l="Health" h="health"><B big i="check" l="Check Schedule" run={() => C.view("checks")} /><B big i="risk" l="Risks" run={() => C.view("risks")} /><B big i="ev" l="Schedule Risk" run={() => C.view("simrisk")} /></G>
      <G l="Portfolio" h="portfolio"><B big i="folder" l="Portfolio" run={() => C.view("portfolio")} /><B big i="copy" l="Compare Projects" run={() => D.compare()} /></G>
      <G l="Export" h="export"><B big i="print" l="Print / PDF" run={C.print} /><B big i="pagesetup" l="Page Setup" run={C.pageSetup} /><B big i="export" l="Export" dd={e => openMenu(e, exportMenu())} /></G>
    </Show>
    <Show when={tab() === "project"}>
      <G l="Properties" h="project"><B big i="proj" l="Project Information" run={() => D.projectInfo()} /><S><B i="fields" l="Custom Fields" run={() => D.customFields()} /><B i="group" l="Activity Codes" run={() => D.activityCodes()} /><B i="calendar2" l="Change Working Time" run={() => D.calendars()} /></S><S><B i="image" l="Pictures" title="The plan's pictures: logos of the client, your company, vendors and sub-vendors" run={() => import("./pictures").then(m => m.pictureLibrary())} /></S>
        <S><B i="setup" l="Project Setup" run={C.setup} on={view() === "setup"} /><B i="opts" l="Schedule Options" run={() => D.scheduleOptions()} /></S></G>
      <G l="Schedule" h="baselines"><B big i="base" l="Set Baseline" run={C.baseline} /><S><B i="track" l="Update Project" run={C.update} /><B i="calendar2" l="Reporting Periods" run={() => D.periodSettings()} /><B i="check" l="Check Schedule" run={() => C.view("checks")} /></S></G>
      <G l="Linked Plans" h="linkedplans"><B big i="link" l="Link to Plan" title="Link the selected task to a task in another plan" run={() => linkSelected()} /><S><B i="list" l="Linked Plans" run={linkedPlans} /><B i="update" l="Update Links" run={() => updateLinks(true)} /><B i="proj" l="Subplans" title="The plans that make up this project: the master and its subplans" run={subplans} /></S></G>
      <G l="Proofing" h="portfolio"><B big i="copy" l="Compare Projects" run={() => D.compare()} /></G>
    </Show>
    <Show when={tab() === "view"}>
      <G l="Task Views" h="taskviews"><B big i="gantt" l="Gantt Chart" run={() => C.view("gantt")} dd={e => openMenu(e, viewMenu())} /><S><B i="base" l="Tracking Gantt" run={() => C.view("tracking")} /><B i="net" l="Network Diagram" run={() => C.view("network")} /><B i="cal" l="Calendar" run={() => C.view("calendar")} /></S><S><B i="link" l="Relationship Diagram" run={() => C.view("relations")} /><B i="table" l="Sheets" run={() => C.view("sheets")} /></S><S><B i="table" l="Task Usage" run={() => C.view("taskusage")} /><B i="tl" l="Timeline" run={C.timelineBar} on={timeline()} /></S></G>
      <G l="Resource Views" h="resviews"><S><B i="res" l="Resource Sheet" run={() => C.view("ressheet")} /><B i="load" l="Resource Usage" run={() => C.view("resusage")} /><B i="team" l="Team Planner" run={() => C.view("team")} /></S></G>
      <G l="Data" h="data">
        <S><B i="sort" l={`Sort${sort().length ? ": on" : ""}`} dd={e => openMenu(e, sortMenu())} /><B i="list" l="Outline" dd={e => openMenu(e, outlineMenu())} /><B i="table" l={`Tables: ${table()}`} dd={e => openMenu(e, tableMenu())} /></S>
        <S><B i="cols" l="Columns…" run={() => D.columnsDialog()} /></S>
        <S><B i="filter" l={`Filter: ${filter().name || "Custom"}`} dd={e => openMenu(e, filterMenu())} on={filter().rules.length > 0} /><B i="filter" l="Highlight" dd={e => openMenu(e, filterMenu().map(x => x === "-" ? x : { ...x, run: () => { x.run?.(); setFilter({ ...filter(), highlight: true }); } }))} /><B i="group" l={`Group by: ${group().length ? group().join(", ") : "None"}`} dd={e => openMenu(e, groupMenu())} on={group().length > 0} /></S>
      </G>
      <G l="Zoom" h="zoom"><B big i="zin" l="Timescale" dd={e => openMenu(e, zoomMenu())} /><S><B i="zin" l="Zoom In" run={C.zoomIn} /><B i="zout" l="Zoom Out" run={C.zoomOut} /><B i="fit" l="Entire Project" run={C.fit} /></S></G>
      <G l="Split View" h="split"><S><B i="detail" l="Details" run={C.details} on={details()} /><B i="tl" l="Timeline" run={C.timelineBar} on={timeline()} /></S></G>
    </Show>
    <Show when={tab() === "sheet"}><SheetCells /></Show>
    <Show when={tab() === "sheetpage"}><SheetPageTab /></Show>
    <Show when={tab() === "format"}>
      <G l="Format" h="format"><B big i="style" l="Bar Styles" run={() => D.barStyles()} /><B big i="grid" l="Conditional Formatting" run={() => D.condFormat()} /><S><label class="chk"><input type="checkbox" checked={gstyle().grid} onChange={e => act({ Ui: { key: "gantt", value: { ...gstyle(), grid: e.currentTarget.checked } } })} />Gridlines</label><B i="detail" l="Details Pane" run={C.details} on={details()} /><B i="tl" l="Timeline" run={C.timelineBar} on={timeline()} /></S></G>
      <G l="Columns" h="columns"><B big i="fx" l="Formula Column" run={() => D.formulaColumn()} /><B big i="cols" l="Insert Column" dd={e => openMenu(e, fieldMenu(k => saveCols([...cols(), [k, 90]])))} /><S><B i="table" l="Columns…" run={() => D.columnsDialog()} /><B i="fields" l="Custom Fields" run={() => D.customFields()} /></S></G>
      <G l="Bar Styles" h="barstyles"><S><label class="chk"><input type="checkbox" checked={showCrit()} onChange={e => setShowCrit(e.currentTarget.checked)} />Critical Tasks</label><label class="chk"><input type="checkbox" checked={showSlack()} onChange={e => setShowSlack(e.currentTarget.checked)} />Slack</label><label class="chk"><input type="checkbox" checked={showLate()} onChange={e => setShowLate(e.currentTarget.checked)} />Late Tasks</label></S>
        <B big i="base" l="Baseline" dd={e => openMenu(e, baselineMenu())} on={showBase()} /></G>
      <G l="Gantt Chart Style" h="ganttstyle"><div class="swatches"><For each={STYLES}>{st => <button class="swatch" classList={{ on: gstyle().name === st.name }} title={st.name} onClick={() => act({ Ui: { key: "gantt", value: { ...st, grid: gstyle().grid } } })}><i style={{ background: st.sum }} /><i style={{ background: st.fill, border: `1px solid ${st.edge}` }} /><i style={{ background: st.cfill, border: `1px solid ${st.crit}` }} /></button>}</For></div></G>
      <G l="Show/Hide" h="showhide"><S><label class="chk"><input type="checkbox" checked={projSum()} onChange={e => setProjSum(e.currentTarget.checked)} />Project Summary Task</label><label class="chk"><input type="checkbox" checked={!hideSum()} onChange={e => setHideSum(!e.currentTarget.checked)} />Summary Tasks</label><label class="chk"><input type="checkbox" checked={showLabels()} onChange={e => setShowLabels(e.currentTarget.checked)} />Bar Labels</label><Show when={P().subplans?.length}><label class="chk" title="A master plan: show its subplans under its tasks (read-only; double-click one to open it)"><input type="checkbox" checked={showSubs()} onChange={e => setShowSubs(e.currentTarget.checked)} />Subplans</label></Show></S></G>
    </Show>
    <Show when={tab() === "help"}>
      <G l="Learn"><B big i="pdf" l="Guide" title="The Datum Plan Guide (PDF), in English or 中文" run={() => D.guide()} /><B big i="report" l="Getting Started" run={() => D.gettingStarted()} /><B big i="help" l="Tutorial" title="Open the guided example: a plan that uses every view, with a note on each" run={() => import("./tour").then(m => m.tutorial())} /><B big i="info" l="Tutorial Notes" title="Show or hide the tutorial notes of a plan that has them" run={() => import("./tour").then(m => { m.setTourOff(!m.tourOff()); if (!m.hasTour()) toast("This plan has no tutorial notes. Help › Tutorial opens the guided example.", false); })} /><B big i="info" l="Glossary" title="What FS, SS, float, baseline and other terms mean" run={() => D.glossary()} /><B big i="help" l="What Is This Tool?" title="Every tool explained: click a ? beside a tool, or pick one here" dd={e => openMenu(e, Object.entries(HELP).map(([k, h]) => ({ label: h.t, run: () => helpCard(k) })))} /><B big i="keys" l="Shortcuts" run={() => D.shortcuts()} /><B big i="search" l="Tell Me" run={palette} /></G>
      <G l="Datum Plan on GitHub"><B big i="github" l="GitHub" title="The project and its code" run={() => call("open_url", { url: GH })} /><B big i="tag" l="Releases" title="Every version and its change log" run={() => call("open_url", { url: `${GH}/releases` })} /><B big i="bug" l="Report an Issue" run={() => call("open_url", { url: `${GH}/issues/new` })} /></G>
      <G l="Updates"><B big i="upd" l="Check for Updates" run={() => setBackstage("about")} /></G>
    </Show>
  </section>
);
