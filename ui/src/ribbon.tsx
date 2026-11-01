// Title bar, quick access toolbar, Tell Me and the ribbon.
import { linkSelected, linkedPlans, updateLinks, subplans } from "./plans";
import { For, Show, JSX, createSignal, createMemo, onMount } from "solid-js";
import { call, out, tab, setTab, setBackstage, view, openMenu, Item, toast, fields, filter, setFilter, ALL, setGroup, group, setSort, sort, useTable, tables, table, setProjSum, projSum, showSubs, setShowSubs, setHideSum, hideSum, baseline, setBaseline, details, timeline, P, act, openDialog, closeDialog, setLevel, setZoom, saveCols, cols, theme, setTheme } from "./store";
import { C, ALL_CMDS } from "./cmds";
import * as D from "./dialogs";
import * as P2 from "./progress";
import { Icon } from "./ui";
import { STYLES, gstyle } from "./gantt";
import { showCrit, setShowCrit, showBase, setShowBase, showSlack, setShowSlack, showLate, setShowLate, showLabels, setShowLabels } from "./gantt";
import { fieldMenu } from "./grid";
import * as F from "./fmt";
import { Help, helpCard, HELP } from "./help";

const GH = "https://github.com/Edgar-Mendonca/Datum-Plan";
export const FONTS = ["Aptos", "Calibri", "Arial", "Segoe UI", "Tahoma", "Verdana", "Times New Roman", "Cambria", "Georgia", "Garamond", "Consolas", "IBM Plex Sans"];
const COLORS: [string, string][] = [["Red", "#C0392B"], ["Orange", "#E67E22"], ["Amber", "#B7950B"], ["Green", "#1E8449"], ["Blue", "#0B5CAD"], ["Dark Blue", "#0C1A2B"], ["Purple", "#7D3C98"], ["Grey", "#7F8C8D"]];
const FILLS: [string, string][] = [["Yellow", "#FFF2CC"], ["Green", "#E2F0D9"], ["Blue", "#DDEBF7"], ["Red", "#FADBD8"], ["Orange", "#FCE4D6"], ["Purple", "#E8DAEF"], ["Grey", "#EDEDED"]];
const pickColor = (k: "c" | "f") => { const i = document.createElement("input"); i.type = "color"; i.value = F.current()[k] || (k === "c" ? "#000000" : "#ffffff"); i.onchange = () => F.setProp(k, i.value); i.click(); };
const colorMenu = (k: "c" | "f"): Item[] => [{ label: k === "c" ? "Automatic" : "No Fill", run: () => F.setProp(k, undefined) }, ...(k === "c" ? COLORS : FILLS).map(([l, v]) => ({ label: l, run: () => F.setProp(k, v), on: F.current()[k] === v })), "-", { label: "More Colors…", run: () => pickColor(k) }];
/** Excel-style Font and Alignment controls; they act on the selected cells of the task sheet or a sheet tab. */
const Font = () => {
  const s = () => F.current(), [last, setLast] = createSignal<[string, string]>(["#C0392B", "#FFF2CC"]);
  const T = (p: { k: "b" | "i" | "u" | "s"; l: string; t: string; st: JSX.CSSProperties }) => <button class="rb fb" classList={{ on: !!s()[p.k] }} title={p.t} onClick={() => F.toggle(p.k)}><span style={p.st}>{p.l}</span></button>;
  const A = (p: { a: string; i: string; t: string }) => <button class="rb fb" classList={{ on: (s().a || "") === p.a }} title={p.t} onClick={() => F.setProp("a", p.a || undefined)}><Icon n={p.i} /></button>;
  return <>
    <G l="Font" h="font"><div class="fontg">
      <div><select class="ffam" title="Font" value={s().n || ""} onChange={e => F.setProp("n", e.currentTarget.value || undefined)}><option value="">Default font</option><For each={FONTS}>{f => <option value={f} style={{ "font-family": `'${f}'` }}>{f}</option>}</For></select>
        <select class="fsize" title="Font size" value={String(s().z || 12)} onChange={e => F.setProp("z", +e.currentTarget.value === 12 ? undefined : +e.currentTarget.value)}><For each={[8, 9, 10, 11, 12, 13, 14]}>{z => <option value={z}>{z}</option>}</For></select>
        <button class="rb fb" title="Increase font size" onClick={() => F.setProp("z", Math.min(14, (s().z || 12) + 1))}><span>A<sup>+</sup></span></button><button class="rb fb" title="Decrease font size" onClick={() => F.setProp("z", Math.max(8, (s().z || 12) - 1))}><span>A<sup>−</sup></span></button></div>
      <div><T k="b" l="B" t="Bold (Ctrl+B)" st={{ "font-weight": 700 }} /><T k="i" l="I" t="Italic (Ctrl+I)" st={{ "font-style": "italic" }} /><T k="u" l="U" t="Underline (Ctrl+U)" st={{ "text-decoration": "underline" }} /><T k="s" l="S" t="Strikethrough" st={{ "text-decoration": "line-through" }} />
        <span class="fcol"><button class="rb fb" title="Font color" onClick={() => F.setProp("c", last()[0])}><span class="ca">A<i style={{ background: last()[0] }} /></span></button><button class="rb split" onClick={e => openMenu(e, colorMenu("c").map(x => x === "-" || !x.run ? x : { ...x, run: () => { x.run!(); const v = F.current().c; if (v) setLast([v, last()[1]]); } }))}>▾</button></span>
        <span class="fcol"><button class="rb fb" title="Fill color" onClick={() => F.setProp("f", last()[1])}><span class="ca"><Icon n="bucket" /><i style={{ background: last()[1] }} /></span></button><button class="rb split" onClick={e => openMenu(e, colorMenu("f").map(x => x === "-" || !x.run ? x : { ...x, run: () => { x.run!(); const v = F.current().f; if (v) setLast([last()[0], v]); } }))}>▾</button></span></div>
    </div></G>
    <G l="Alignment" h="alignment"><div class="fontg"><div><A a="" i="al" t="Align left" /><A a="c" i="ac" t="Center" /><A a="r" i="ar" t="Align right" /></div><div><button class="rb fb" title="Clear Formatting" onClick={F.clearFmt}><Icon n="clear" /></button></div></div></G>
  </>;
};

const B = (p: { i: string; l: string; run?: (e: MouseEvent) => void; big?: boolean; on?: boolean; off?: boolean; dd?: (e: MouseEvent) => void; title?: string }) => p.big && p.run && p.dd ? (
  // MS Project split button: the picture runs the command, the label opens the menu.
  <div class="rbw big splitbig" classList={{ on: p.on }}>
    <button class="rb top" title={p.title ?? p.l} onClick={e => p.run!(e)}><Icon n={p.i} /></button>
    <button class="rb bot" title={`${p.l} options`} onClick={e => p.dd!(e)}><span>{p.l}</span><b class="car">▾</b></button>
  </div>
) : (
  <div class="rbw" classList={{ big: p.big }}>
    <button class="rb" classList={{ big: p.big, on: p.on }} disabled={p.off} title={p.title ?? p.l} onClick={e => (p.run ?? p.dd)?.(e)}><Icon n={p.i} />{p.big ? <span>{p.l}{p.dd && !p.run ? " ▾" : ""}</span> : <span>{p.l}</span>}{!p.big && p.dd && !p.run && <b class="car">▾</b>}</button>
    {p.run && p.dd && <button class="rb split" onClick={e => p.dd!(e)}>▾</button>}
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
const exportMenu = (): Item[] => [["Excel workbook (.xlsx)", "xlsx"], ["MS Project XML (.xml)", "xml"], ["Primavera P6 XER (.xer)", "xer"], ["Primavera P6 XML (.pmxml)", "pmxml"], ["MPX (.mpx)", "mpx"], ["CSV (.csv)", "csv"], ["Web page (.html)", "html"], ["iCalendar (.ics)", "ics"], ["JSON (.json)", "json"]].map(([l, k]) => ({ label: l, run: () => C.exportAs(k) }));
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

export const TitleBar = (p: { update: string | null }) => (
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
      <For each={[["task", "Task"], ["schedule", "Schedule"], ["progress", "Progress"], ["resource", "Resource"], ["report", "Report"], ["project", "Project"], ["view", "View"], ["format", "Format"], ["help", "Help"]]}>{([k, l]) =>
        k === "format" ? <div class="ctxgrp"><small>Gantt Chart Tools</small><button classList={{ on: tab() === k }} onClick={() => setTab(k)}>{l}</button></div>
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
      <G l="Properties" h="project"><B big i="proj" l="Project Information" run={() => D.projectInfo()} /><S><B i="fields" l="Custom Fields" run={() => D.customFields()} /><B i="group" l="Activity Codes" run={() => D.activityCodes()} /><B i="calendar2" l="Change Working Time" run={() => D.calendars()} /></S>
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
