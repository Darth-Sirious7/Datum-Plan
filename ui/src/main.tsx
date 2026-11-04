// Datum Plan window: layout, sidebar, status bar, keyboard shortcuts, startup and recovery.
import { render } from "solid-js/web";
import { For, Show, Switch, Match, createSignal, onMount } from "solid-js";
import "@fontsource/ibm-plex-sans/latin-400.css";
import "@fontsource/ibm-plex-sans/latin-500.css";
import "@fontsource/ibm-plex-sans/latin-600.css";
import "@fontsource/ibm-plex-sans/latin-700.css";
import "@fontsource/ibm-plex-mono/latin-400.css";
import "@fontsource/ibm-plex-mono/latin-500.css";
import "@fontsource/ibm-plex-mono/latin-600.css";
import "./styles.css";
import { invoke, listen, Out, FieldList, Options } from "./api";
import { out, setOut, view, backstage, setBackstage, setFields, setOpts, opts, details, timeline, setSetupOpen, gridW, setGridW, dialogs, call, grid, filter, setFilter, ALL, zoom, setZoom, useTable, table, openDialog, closeDialog, P, View, toast, sel, act, loadOut, setThemeRaw, applyTheme } from "./store";
import { C } from "./cmds";
import { sheetZoom, setSheetZoom } from "./sheetlayout";
import { TitleBar, Ribbon, palette } from "./ribbon";
import { Grid, FormulaBar } from "./grid";
import { Gantt, Timeline } from "./gantt";
import { Details } from "./details";
import { Network, CalendarView, UsageView, ResourceSheet, ResourceGraph, TeamPlanner, Risks, Checks, Portfolio } from "./views";
import { Dashboard, Reports, SCurves, Progress } from "./reports";
import { Sheets } from "./sheets";
import { UpdateProgress, Relationships, ReportBuilder } from "./tools";
import { Backstage, update, checkUpdate } from "./backstage";
import { Setup } from "./setup";
import { Menu, Toast, Icon, Dialog } from "./ui";
import * as D from "./dialogs";
import { ScheduleRisk } from "./risk";
import { Lookahead, Manpower } from "./progress";
import { TourCard } from "./tour";
import { PlanTabs } from "./plans";

const SIDE: ([View, string, string] | "-")[] = [
  ["setup", "setup", "Setup"], "-", ["gantt", "gantt", "Gantt"], ["tracking", "base", "Tracking"], ["network", "net", "Network"], ["calendar", "cal", "Calendar"], ["taskusage", "table", "Task Usage"], "-",
  ["ressheet", "res", "Resources"], ["resusage", "load", "Res. Usage"], ["manpower", "team", "Manpower"], ["team", "team", "Team"], "-",
  ["sheets", "table", "Sheets"], "-",
  ["update", "update", "Update"], ["lookahead", "cal", "Look-ahead"], ["dashboard", "dash", "Dashboard"], ["progress", "track", "Progress"], ["scurve", "ev", "S-Curves"], ["reports", "report", "Reports"], ["builder", "builder", "Builder"], ["checks", "check", "Checks"], ["risks", "risk", "Risks"], ["simrisk", "ev", "Sched. Risk"], ["portfolio", "folder", "Portfolio"],
];

const GanttView = () => {
  const [detH, setDetH] = createSignal(240);
  const drag = (e: MouseEvent, set: (v: number) => void, v0: number, axis: "x" | "y", sign = 1) => {
    e.preventDefault();
    const a0 = axis === "x" ? e.clientX : e.clientY;
    const mv = (m: MouseEvent) => set(Math.max(120, v0 + sign * ((axis === "x" ? m.clientX : m.clientY) - a0)));
    const up = () => { removeEventListener("mousemove", mv); removeEventListener("mouseup", up); };
    addEventListener("mousemove", mv); addEventListener("mouseup", up);
  };
  return (
    <div class="ganttview">
      <Show when={view() === "tracking"}>
        <div class="trkbar"><b>Tracking Gantt</b><span>Grey bars: the baseline (the plan you saved). Coloured bars: today's dates, solid where done. Labels: % complete.</span>
          <Show when={!grid().rows.some(r => r.bar.bs !== null)}><span class="warn">No baseline yet, so there is nothing to compare with.</span><button class="btn sm pri" onClick={C.baseline}>Set Baseline</button></Show>
          <span class="grow" /><button class="btn sm" onClick={() => D.glossary()}>What do these mean?</button></div>
      </Show>
      <Show when={timeline()}><Timeline /></Show>
      <FormulaBar />
      <div class="gmain">
        <Grid width={gridW()} />
        <div class="split" onMouseDown={e => drag(e, setGridW, gridW(), "x")} onDblClick={() => setGridW(gridW() > 300 ? 120 : 620)} />
        <Gantt />
      </div>
      <Show when={details()}><div class="dsplit" onMouseDown={e => drag(e, setDetH, detH(), "y", -1)} /><div style={{ height: `${detH()}px`, display: "flex", "flex-direction": "column" }}><Details /></div></Show>
    </div>
  );
};

const Status = () => {
  const s = () => out()?.summary;
  return (
    <footer class="status">
      <span classList={{ late: !!s()?.error }}>{s()?.error ?? "Ready"}</span>
      <button class="mode" onClick={() => out() && act({ Project: { name: P().name, start: P().start, status: P().status ?? "", calendar: P().calendar, day_hours: P().day_min / 60, manual_default: !P().manual_default } })}>
        New Tasks: {out()?.project.manual_default ? "Manually Scheduled" : "Auto Scheduled"}</button>
      <Show when={filter().rules.length}><button class="mode" onClick={() => setFilter(ALL)}>Filter applied: {filter().name || "custom"} ✕</button></Show>
      <span class="muted">{grid().shown} of {grid().total} tasks{sel().length > 1 ? ` · ${sel().length} selected` : ""}</span>
      <span class="muted">{s() ? `${s()!.start} – ${s()!.finish} · ${s()!.dur} · ${Math.round(s()!.pct)}% · ${s()!.cost}` : ""}</span>
      <span class="grow" />
      <span class="vsc">
        <button title="Gantt Chart" onClick={() => C.view("gantt")}><Icon n="gantt" /></button><button title="Task Usage" onClick={() => C.view("taskusage")}><Icon n="table" /></button>
        <button title="Team Planner" onClick={() => C.view("team")}><Icon n="team" /></button><button title="Resource Sheet" onClick={() => C.view("ressheet")}><Icon n="res" /></button><button title="Reports" onClick={() => C.view("reports")}><Icon n="report" /></button>
      </span>
      <button onClick={C.zoomOut} title="Zoom Out">−</button><Show when={view() === "sheets"} fallback={<input class="zoom" type="range" min="0" max="100" value={Math.round(Math.log(zoom() / 0.3) / Math.log(400) * 100)} onInput={e => setZoom(0.3 * Math.pow(400, +e.currentTarget.value / 100))} />}>
        <input class="zoom szoom" type="range" min="10" max="400" step="5" title="Zoom the sheet (Ctrl+wheel)" value={Math.round(sheetZoom() * 100)} onInput={e => setSheetZoom(+e.currentTarget.value / 100)} /></Show><button onClick={C.zoomIn} title="Zoom In">+</button><Show when={view() === "sheets"}><button class="zpct" title="Back to 100 %" onClick={() => setSheetZoom(1)}>{Math.round(sheetZoom() * 100)}%</button></Show>
    </footer>
  );
};

const App = () => {
  onMount(async () => {
    try {
      setFields(await invoke<FieldList>("field_list"));
      const o = await invoke<Options>("options"); setOpts(o); setThemeRaw(o.theme || "auto"); applyTheme();
      setOut(await invoke<Out>("state"));
      useTable("Entry");
      const start = await invoke<string | null>("startup_file");
      if (start) await loadOut("open", { path: start });
      const rec = await invoke<{ name: string; saved: string } | null>("recovery");
      if (rec) openDialog(() => (
        <Dialog title="Recover unsaved work?" okText="Recover" cancel="Discard" ok={async () => { const r = await call<Out>("recover", { keep: true }); if (r) { setOut(r); setBackstage(null); useTable("Entry"); } }}>
          <p>Datum Plan closed before “{rec.name}” was saved. A copy from {rec.saved} was kept.</p>
        </Dialog>
      ));
      checkUpdate(true);
      // Once, on the first start: point new users to the guide.
      try { if (!localStorage.getItem("guide-hint")) { localStorage.setItem("guide-hint", "1"); setTimeout(() => toast("New to Datum Plan? Help › Guide has a step-by-step guide (PDF) in English and 中文."), 1500); } } catch { /* no storage: no hint */ }
      setInterval(() => { if (opts().autosave > 0) invoke("autosave").catch(() => {}); }, Math.max(1, opts().autosave) * 60000);
      listen("close-requested", () => D.confirmClose(() => invoke("quit")));
      // No browser menu (Back, Reload…) outside text fields: our own menus open where they apply.
      document.addEventListener("contextmenu", e => { if (!(e.target as HTMLElement).closest?.("input, textarea, [contenteditable]")) e.preventDefault(); });
    } catch (e) { toast(String(e)); }
  });
  const keys = (e: KeyboardEvent) => {
    const ctrl = e.ctrlKey || e.metaKey, k = e.key.toLowerCase();
    if (dialogs().length) return;
    if ((ctrl && k === "k") || (e.altKey && k === "q")) { e.preventDefault(); palette(); }
    else if (ctrl && k === "s") { e.preventDefault(); C.save(e.shiftKey); }
    else if (ctrl && e.key === "Tab") { e.preventDefault(); import("./plans").then(m => m.nextTab(e.shiftKey ? -1 : 1)); }
    else if (ctrl && k === "w") { e.preventDefault(); import("./plans").then(m => m.closeTab(out()!.tab)); }
    else if (ctrl && k === "o") { e.preventDefault(); C.open(); }
    else if (ctrl && k === "n") { e.preventDefault(); setBackstage("home"); }
    else if (ctrl && k === "p") { e.preventDefault(); C.print(); }
    else if (ctrl && k === "z" && !(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)) { e.preventDefault(); C.undo(); }
    else if (ctrl && k === "y" && !(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)) { e.preventDefault(); C.redo(); }
    else if (e.key === "F1") { e.preventDefault(); D.shortcuts(); }
    else if (e.key === "Escape" && backstage() && out()) setBackstage(null);
  };
  return (
    <div class="app" ref={() => document.addEventListener("keydown", keys)}>
      <TitleBar update={update()?.available ? update()!.latest : null} />
      <Ribbon />
      <Show when={out()}><PlanTabs /></Show>
      <main class="work">
        <nav class="side"><For each={SIDE}>{x => x === "-" ? <hr /> : <button classList={{ on: view() === x[0] }} onClick={() => C.view(x[0])} title={x[2]}><Icon n={x[1]} /><span>{x[2]}</span></button>}</For></nav>
        <div class="content">
          <Show when={out()}>
            <Switch fallback={<GanttView />}>
              <Match when={view() === "setup"}><div class="setuppage"><Setup /></div></Match>
              <Match when={view() === "network"}><Network /></Match>
              <Match when={view() === "calendar"}><CalendarView /></Match>
              <Match when={view() === "taskusage"}><UsageView by="task" /></Match>
              <Match when={view() === "resusage"}><UsageView by="resource" /></Match>
              <Match when={view() === "ressheet"}><ResourceSheet /></Match>
              <Match when={view() === "resgraph"}><ResourceGraph /></Match>
              <Match when={view() === "team"}><TeamPlanner /></Match>
              <Match when={view() === "dashboard"}><Dashboard /></Match>
              <Match when={view() === "progress"}><Progress /></Match>
              <Match when={view() === "scurve"}><SCurves /></Match>
              <Match when={view() === "sheets"}><Sheets /></Match>
              <Match when={view() === "update"}><UpdateProgress /></Match>
              <Match when={view() === "relations"}><Relationships /></Match>
              <Match when={view() === "builder"}><ReportBuilder /></Match>
              <Match when={view() === "reports"}><Reports /></Match>
              <Match when={view() === "risks"}><Risks /></Match>
              <Match when={view() === "simrisk"}><ScheduleRisk /></Match>
              <Match when={view() === "lookahead"}><Lookahead /></Match>
              <Match when={view() === "manpower"}><Manpower /></Match>
              <Match when={view() === "checks"}><Checks /></Match>
              <Match when={view() === "portfolio"}><Portfolio /></Match>
            </Switch>
          </Show>
          <TourCard views={SIDE.filter(x => x !== "-").map(x => [(x as [View, string, string])[0], (x as [View, string, string])[2]] as [View, string])} />
        </div>
      </main>
      <Status />
      <Show when={backstage()}><Backstage /></Show>
      <For each={dialogs()}>{d => d()}</For>
      <Menu /><Toast />
    </div>
  );
};

render(() => <App />, document.getElementById("app")!);
export { table, closeDialog, setSetupOpen };
