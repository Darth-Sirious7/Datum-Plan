// Commands shared by the ribbon, menus, keys and Tell Me.
import { table, useTable, act, call, cur, curRow, grid, loadOut, out, refresh, selRows, setBackstage, setCollapsed, setCur, setLevel, setOut, setSel, setView, setZoom, toast, zoom, view, View, setDetails, details, setTimeline, timeline, setupOpen, setSetupOpen, openDialog, collapsed } from "./store";
import { Out } from "./api";
import { zoomSheet } from "./sheetlayout";
import * as D from "./dialogs";

const uids = () => { const s = selRows().map(r => r.uid); return s.length ? s : curRow()?.uid > 0 ? [curRow()!.uid] : []; };
const leafIdx = () => grid().rows[cur().row]?.id ?? (grid().rows.filter(r => r.uid > 0).length + 1);
/** Row position (0-based, in plan order) where a new task goes: above the cursor row. */
const insertAt = () => { const r = curRow(); return r && r.uid > 0 ? r.id - 1 : grid().total; };

export const selectUid = (uid: number) => {
  const k = grid().rows.findIndex(r => r.uid === uid);
  if (k >= 0) { setCur({ row: k, col: cur().col }); setSel([uid]); }
};

export const C = {
  newTask: async (ms = false) => { const at = insertAt(); if (await act({ Insert: { at, name: ms ? "New milestone" : "New task", ms } })) { await refresh(); const u = grid().rows.find(r => r.id === at + 1); if (u) selectUid(u.uid); } },
  summary: async () => {
    const u = uids();
    if (!u.length) return C.newTask();
    const first = grid().rows.find(r => r.uid === u[0])!;
    // One step, so one Undo takes it all back.
    await act({ Batch: [{ Insert: { at: first.id - 1, name: "New summary task", ms: false } }, { Indent: { uids: u } }] });
  },
  del: () => { const u = uids(); if (u.length) act({ Delete: { uids: u } }); },
  indent: () => act({ Indent: { uids: uids() } }),
  outdent: () => act({ Outdent: { uids: uids() } }),
  link: () => { const u = uids(); if (u.length > 1) act({ Link: { uids: u } }); else toast("Select two or more tasks to link."); },
  unlink: () => act({ Unlink: { uids: uids() } }),
  move: (up: boolean) => act({ Move: { uids: uids(), up } }),
  onTrack: () => act({ MarkOnTrack: { uids: uids() } }),
  // As cells: a summary in the selection is skipped (its progress comes from its tasks), not an error that stops the rest.
  pct: (v: number) => act({ Cells: { cells: uids().map(uid => [uid, "pct", String(v)] as [number, string, string]) } }),
  mode: (manual: boolean) => act({ Cells: { cells: uids().map(uid => [uid, "mode", manual ? "manual" : "auto"] as [number, string, string]) } }),
  inactive: () => { const r = selRows(); const on = r.some(x => x.off); act({ Batch: uids().map(uid => ({ Set: { uid, field: "active", value: on ? "yes" : "no" } })) }); },
  timeline: () => act({ Batch: uids().map(uid => ({ Set: { uid, field: "timeline", value: "yes" } })) }),
  split: () => { const r = curRow(); if (r?.kind === "task") D.splitTask(r.uid); },
  copy: async () => { const n = await call<number>("copy", { uids: uids() }); if (n) toast(`${n} task${n > 1 ? "s" : ""} copied.`, true); },
  cut: async () => { await C.copy(); C.del(); },
  paste: async () => setOut(await call<Out>("paste", { at: insertAt() })),
  undo: async () => setOut(await call<Out>("history", { redo: false })),
  redo: async () => setOut(await call<Out>("history", { redo: true })),
  save: async (asNew = false) => { const o = await call<Out | null>("save", { asNew }); if (o) { setOut(o); toast("Saved.", true); } },
  open: () => loadOut("open", { path: null }),
  importFile: () => D.importFile(),
  info: () => { const r = curRow(); if (r?.uid) D.taskInfo(r.uid); else D.projectInfo(); },
  notes: () => { const r = curRow(); if (r?.uid) D.taskInfo(r.uid, "notes"); },
  recurring: () => D.recurring(insertAt()),
  globalChange: () => D.globalChange(uids()),
  scrollTo: () => { const r = curRow(); if (r) document.dispatchEvent(new CustomEvent("gantt-scroll", { detail: r.bar.s })); },
  view: (v: View) => {
    setBackstage(null);
    // The Tracking Gantt shows the Tracking table; leaving it brings the Entry table back.
    if (v === "tracking" && table() !== "Tracking") useTable("Tracking");
    else if (v !== "tracking" && table() === "Tracking" && ["gantt", "timeline"].includes(v)) useTable("Entry");
    setView(v);
  },
  zoomIn: () => (view() === "sheets" ? zoomSheet(1) : setZoom(Math.min(400, zoom() * 1.5))),
  zoomOut: () => (view() === "sheets" ? zoomSheet(-1) : setZoom(Math.max(0.6, zoom() / 1.5))),
  fit: () => document.dispatchEvent(new CustomEvent("gantt-fit")),
  outline: (n: number) => { setCollapsed([]); setLevel(n); },
  expandAll: () => { setLevel(0); setCollapsed([]); },
  collapseAll: () => setCollapsed(grid().rows.filter(r => r.kids && r.uid > 0).map(r => r.uid)),
  toggle: (uid: number) => setCollapsed(collapsed().includes(uid) ? collapsed().filter(x => x !== uid) : [...collapsed(), uid]),
  details: () => setDetails(!details()),
  timelineBar: () => setTimeline(!timeline()),
  setup: () => setSetupOpen(!setupOpen()),
  level: (all = true) => D.levelPreview(all ? [] : uids()),
  clearLevel: () => act({ Level: { uids: [], clear: true } }),
  baseline: () => D.setBaseline(uids()),
  update: () => D.updateProject(),
  print: () => setBackstage("print"),
  pageSetup: () => setBackstage("print"),
  exportAs: (kind: string) => D.exportAs(kind),
  file: (page: string) => setBackstage(page),
  options: () => setBackstage("options"),
  ids: () => uids(),
  leafIdx,
};

/** Every command with a name, for Tell Me and the shortcut list. */
export const ALL_CMDS: [string, () => void, string?][] = [
  ["New task", () => C.newTask(), "Insert"], ["New milestone", () => C.newTask(true)], ["New summary task", C.summary], ["Delete tasks", C.del, "Delete"],
  ["Indent", C.indent, "Alt+Shift+→"], ["Outdent", C.outdent, "Alt+Shift+←"], ["Link tasks", C.link, "Ctrl+F2"], ["Unlink tasks", C.unlink],
  ["Move up", () => C.move(true)], ["Move down", () => C.move(false)], ["Mark on track", C.onTrack], ["Split task", C.split], ["Inactivate task", C.inactive],
  ["Link to another plan", () => import("./plans").then(m => m.linkSelected())], ["Linked plans", () => import("./plans").then(m => m.linkedPlans())], ["Update links between plans", () => import("./plans").then(m => m.updateLinks(true))],
  ["Next plan tab", () => import("./plans").then(m => m.nextTab(1)), "Ctrl+Tab"], ["Close plan", () => import("./plans").then(m => m.closeTab(out()!.tab)), "Ctrl+W"], ["Close all plans", () => import("./plans").then(m => m.closeAll())], ["Subplans of this project", () => import("./plans").then(m => m.subplans())], ["New subplan", () => import("./plans").then(m => m.newSubplan())], ["Open all subplans", () => import("./plans").then(m => m.openAll())], ["Go to linked task", () => { const r = curRow(); if (r) import("./plans").then(m => m.linkMenu(new MouseEvent("click", { clientX: 400, clientY: 300 }), r.uid)); }], ["Linked plans example", () => import("./plans").then(m => m.example())],
  ["Manually schedule", () => C.mode(true)], ["Auto schedule", () => C.mode(false)], ["Add to timeline", C.timeline], ["Task information", C.info, "Shift+F2"],
  ["Notes", C.notes], ["Copy", C.copy, "Ctrl+C"], ["Cut", C.cut, "Ctrl+X"], ["Paste", C.paste, "Ctrl+V"], ["Undo", C.undo, "Ctrl+Z"], ["Redo", C.redo, "Ctrl+Y"],
  ["Save", () => C.save(), "Ctrl+S"], ["Save as", () => C.save(true)], ["Open", C.open, "Ctrl+O"], ["Import", C.importFile], ["Print", C.print, "Ctrl+P"], ["Page Setup", C.pageSetup], ["Getting started", () => D.gettingStarted()], ["Glossary", () => D.glossary()], ["Paste Special", () => D.pasteSpecial(), "Ctrl+Alt+V"], ["GitHub page", () => call("open_url", { url: "https://github.com/Edgar-Mendonca/Datum-Plan" })], ["Report an issue", () => call("open_url", { url: "https://github.com/Edgar-Mendonca/Datum-Plan/issues/new" })], ["Bold", () => import("./fmt").then(f => f.toggle("b")), "Ctrl+B"], ["Italic", () => import("./fmt").then(f => f.toggle("i")), "Ctrl+I"], ["Underline", () => import("./fmt").then(f => f.toggle("u")), "Ctrl+U"], ["Clear Formatting", () => import("./fmt").then(f => f.clearFmt())], ["Format Painter", () => import("./fmt").then(f => f.paint())],
  ["Gantt Chart", () => C.view("gantt")], ["Tracking Gantt", () => C.view("tracking")], ["Network Diagram", () => C.view("network")], ["Calendar view", () => C.view("calendar")],
  ["Task Usage", () => C.view("taskusage")], ["Resource Sheet", () => C.view("ressheet")], ["Resource Usage", () => C.view("resusage")], ["Resource Graph", () => C.view("resgraph")],
  ["Team Planner", () => C.view("team")], ["Dashboard", () => C.view("dashboard")], ["Reports", () => C.view("reports")], ["Portfolio", () => C.view("portfolio")],
  ["Risks", () => C.view("risks")], ["Check schedule", () => C.view("checks")], ["Zoom in", C.zoomIn], ["Zoom out", C.zoomOut], ["Entire project", C.fit],
  ["Expand all", C.expandAll], ["Collapse all", C.collapseAll], ["Show details pane", C.details], ["Show timeline", C.timelineBar], ["Project setup", C.setup],
  ["Open the guide (PDF, English or 中文)", () => D.guide()], ["Level resources", () => C.level()], ["Clear leveling", C.clearLevel], ["Set baseline", C.baseline], ["Update project", C.update],
  ["Project information", () => D.projectInfo()], ["Change working time", () => D.calendars()], ["Schedule options", () => D.scheduleOptions()],
  ["Custom fields", () => D.customFields()], ["Activity codes", () => D.activityCodes()], ["Filter", () => D.filterEditor()], ["Group by", () => D.groupEditor()],
  ["Sort", () => D.sortEditor()], ["Insert column", () => D.insertColumn()], ["Tables", () => D.tablesDialog()], ["Compare projects", () => D.compare()],
  ["Store period performance (snapshot)", () => D.snapshot()], ["Options", C.options], ["Check for updates", () => C.file("about")], ["Keyboard shortcuts", () => D.shortcuts()],
  ["Export to Excel", () => C.exportAs("xlsx")], ["Export to Project XML", () => C.exportAs("xml")], ["Export to XER", () => C.exportAs("xer")],
  ["Export to PMXML", () => C.exportAs("pmxml")], ["Export to CSV", () => C.exportAs("csv")], ["Export to HTML", () => C.exportAs("html")], ["Export to iCalendar", () => C.exportAs("ics")],
  ["Export to MPX", () => C.exportAs("mpx")], ["Recurring task", () => C.recurring()], ["Global change", () => C.globalChange()],
  ["Update progress", () => C.view("update")], ["Report builder", () => C.view("builder")], ["Relationship diagram", () => C.view("relations")],
  ["Conditional formatting", () => D.condFormat()], ["Bar styles", () => D.barStyles()], ["Reporting periods", () => D.periodSettings()], ["Formula column", () => D.formulaColumn()], ["Export to JSON", () => C.exportAs("json")],
];
export const ready = () => !!out();
export const openDlg = openDialog;
