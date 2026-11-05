// Several plans at once: a tab per open plan (above the ribbon), links from a task to tasks in
// other plans, and the Linked Plans panel that shows them and brings their dates up to date.
import { For, Show, createMemo, createResource, createSignal } from "solid-js";
import { Out, Row as GRow, SubPlan, TabInfo, fmtAt } from "./api";
import { act, call, out, P, setOut, openMenu, openDialog, closeDialog, toast, useTable, table, tick, refresh, grid, setView, view, toggleSub, setShowSubs } from "./store";
import { RH } from "./grid";
import { selectUid } from "./cmds";
import { Dialog, Row } from "./ui";
import { C } from "./cmds";

type ExtLink = { file: string; uid: number; kind: string; lag: number; plan: string; code: string; name: string; start: number; finish: number };
type LinkGo = { dir: "in" | "out"; label: string; file: string; uid: number; moved: boolean };
type Target = { tab: number; name: string; path: string | null; tasks: [number, string, string, number, boolean, number, number][] };
type LinkState = { uid: number; task: string; k: number; link: ExtLink; now_start: number | null; now_finish: number | null; state: string };

/** Shows another tab's plan (the view stays as it is). */
export const showTab = async (k: number) => { const o = await call<Out>("switch_tab", { k }); if (o) { setOut(o); useTable(table()); } };
/** Closes a tab, offering to save it first when it has unsaved changes. */
export const closeTab = async (k: number) => {
  const t = out()!.tabs[k];
  const close = async () => { const o = await call<Out>("close_tab", { k }); if (o) { setOut(o); useTable(table()); } };
  if (!t.dirty) return close();
  await showTab(k);
  openDialog(() => (
    <Dialog title="Save changes?" okText="Save" ok={async () => { const o = await call<Out | null>("save", { asNew: false }); if (o) { setOut(o); await close(); } return !!o; }}
      extra={<button class="btn" onClick={() => { closeDialog(); close(); }}>Don't save</button>}>
      <p>Do you want to save the changes to “{t.name}” before closing it?</p>
    </Dialog>
  ));
};
/** File › Close All: every plan, offering to save the ones with unsaved changes first. */
export const closeAll = () => {
  const close = async () => { const o = await call<Out>("close_all"); if (o) { setOut(o); useTable(table()); } };
  const dirty = (out()?.tabs ?? []).filter(t => t.dirty).map(t => t.name);
  if (!dirty.length) return close();
  openDialog(() => (
    <Dialog title="Save changes?" okText="Save All" ok={async () => { const o = await call<Out | null>("save_all"); if (o) await close(); return !!o; }}
      extra={<button class="btn" onClick={() => { closeDialog(); close(); }}>Don't save</button>}>
      <p>These plans have unsaved changes: {dirty.join(", ")}. Save them before closing?</p>
    </Dialog>
  ));
};
/** Shows the plan a link points at (opening it if needed) and selects the task (uid 0: none). */
export const goToLink = async (file: string, uid: number) => {
  const o = await call<Out>("goto_link", { file });
  if (!o) return;
  setOut(o); useTable(table());
  if (view() !== "gantt" && view() !== "tracking") setView("gantt");
  if (!uid) return afterOpen();
  await refresh();
  selectUid(uid);
  const k = grid().rows.findIndex(r => r.uid === uid);
  if (k >= 0) { document.dispatchEvent(new CustomEvent("grid-scroll", { detail: Math.max(0, k * RH - 60) })); document.dispatchEvent(new CustomEvent("gantt-scroll", { detail: grid().rows[k].bar.s })); }
};
/** The menu of a task's links: go to the other plan's task, edit, update. */
export const linkMenu = async (e: MouseEvent, uid: number) => {
  const pos = { x: e.clientX, y: e.clientY };
  const go = (await call<LinkGo[]>("link_go", { uid })) ?? [];
  const moved = go.some(g => g.moved);
  openMenu(pos, [
    ...go.map(g => ({ label: `${g.dir === "in" ? "Go to (waits for)" : "Go to (waits for this)"}: ${g.label}`, run: () => goToLink(g.file, g.uid) })),
    ...(go.length ? ["-" as const] : []),
    ...(moved ? [{ label: "Update Links (the other plan has moved)", run: () => updateLinks(true) }] : []),
    { label: "Link to Plan…", run: () => linkDialog(uid) }, { label: "Linked Plans…", run: linkedPlans },
  ]);
};
/** Right-click on a tab. */
const tabMenu = (e: MouseEvent, k: number) => {
  const t = out()!.tabs[k], g = groupOf(k);
  openMenu(e, [
    ...(t.role === "master" ? [{ label: "Open All Subplans", run: () => showTab(k).then(openAll) }, { label: "New Subplan…", run: () => showTab(k).then(newSubplan) }, { label: "Subplans…", run: () => showTab(k).then(subplans) }, "-" as const] : []),
    ...(t.role === "sub" && t.group < 0 && t.project ? [{ label: `Open Master (“${t.project}”)`, run: () => goToLink(`${t.project}.dplan`, 0) }, "-" as const] : []),
    { label: "Close", run: () => closeTab(k) },
    ...(g >= 0 ? [{ label: `Close Project (“${out()!.tabs[g].name}”)`, run: () => closeProject(g) }] : []),
    { label: "Close All Plans", run: closeAll },
    ...(t.moved ? ["-" as const, { label: "Update Links", run: () => showTab(k).then(() => updateLinks(true)) }] : []),
  ]);
};
/** The master tab of the project a tab belongs to (-1: none open). */
const groupOf = (k: number) => { const t = out()!.tabs[k]; return t.role === "master" ? k : t.group; };
/** Closes a master and its open subplans, offering to save the ones with unsaved changes. */
export const closeProject = (g: number) => {
  const ks = out()!.tabs.map((t, k) => (k === g || t.group === g ? k : -1)).filter(k => k >= 0);
  const close = async () => { const o = await call<Out>("close_tabs", { ks }); if (o) { setOut(o); useTable(table()); } };
  const dirty = ks.filter(k => out()!.tabs[k].dirty);
  if (!dirty.length) return close();
  const names = dirty.map(k => out()!.tabs[k].name);
  openDialog(() => (
    <Dialog title="Save changes?" okText="Save" ok={async () => {
      // Saving does not move tabs, so `ks` still names the project's tabs.
      for (const k of dirty) { await showTab(k); const o = await call<Out | null>("save", { asNew: false }); if (!o) return false; setOut(o); }
      await close();
      return true;
    }} extra={<button class="btn" onClick={() => { closeDialog(); close(); }}>Don't save</button>}>
      <p>These plans of the project have unsaved changes: {names.join(", ")}. Save them before closing?</p>
    </Dialog>
  ));
};

// ---------- a project: the master plan and its subplans ----------

type SubRow = { file: string; name: string; level: number; found: boolean; open: boolean; tab: number; start: number; finish: number; pct: number; tasks: number };
/** Opens the shown master's subplans that are not open yet. */
export const openAll = async () => {
  const r = await call<[Out, string[]]>("open_subplans");
  if (!r) return;
  setOut(r[0]); useTable(table());
  if (r[1].length) toast(`Not found next to the master: ${r[1].join(", ")}. Subplans… shows where they were.`);
};
/** After opening a plan: a master offers to open its subplans too. */
export const afterOpen = () => {
  const subs = P().subplans ?? [], open = new Set((out()?.tabs ?? []).map(t => (t.path ?? "").split(/[\\/]/).pop()!.toLowerCase()));
  const shut = subs.filter(s => !open.has(s.file.split(/[\\/]/).pop()!.toLowerCase()));
  if (!shut.length) return;
  openDialog(() => (
    <Dialog title="Open the subplans too?" okText="Open All" cancel="Not now" ok={() => { openAll(); }}>
      <p>“{P().name}” is the master of {subs.length} {subs.length === 1 ? "subplan" : "subplans"}: {subs.map(s => s.name).join(", ")}. Open {shut.length === subs.length ? "them" : `the ${shut.length} not open yet`} in tabs beside it?</p>
    </Dialog>
  ));
};
/** New Subplan: a name and its place in the hierarchy; saved next to the master and shown. */
export const newSubplan = () => {
  if (!out()?.path) return toast("Save the master plan first, so its subplans can be saved next to it.");
  if (P().master) return toast(`“${P().name}” is a subplan. Add subplans from its master (Subplans…), where the whole hierarchy is kept.`);
  openDialog(() => {
    const list = P().subplans ?? [];
    const [name, setName] = createSignal(""), [under, setUnder] = createSignal(-1);
    const ok = async () => {
      const i = under(), lv = (k: number) => list[k]?.level ?? 0;
      let at = list.length;
      if (i >= 0) { at = i + 1; while (at < list.length && lv(at) > lv(i)) at++; }
      const master = P().name, o = await call<Out>("new_subplan", { name: name() || "Subplan", level: i >= 0 ? lv(i) + 1 : 0, at });
      if (o) { setOut(o); useTable(table()); toast(`“${name() || "Subplan"}” was saved next to “${master}” and is open in its tab. Link its tasks to the master with Project › Link to Plan.`, true); }
      return !!o;
    };
    return (
      <Dialog title="New Subplan" okText="Create" help="subplans" ok={ok}>
        <p>A new plan of this project, saved in the master's folder with the same calendars. Its place sets the hierarchy; dates come from the links you make.</p>
        <div class="form">
          <Row label="Name"><input value={name()} placeholder="Engineering" onInput={e => setName(e.currentTarget.value)} ref={el => setTimeout(() => el.focus())} /></Row>
          <Row label="Place"><select onChange={e => setUnder(+e.currentTarget.value)}><option value={-1}>Directly under “{P().name}”</option>
            <For each={list}>{(s, k) => <option value={k()}>{"\u00a0\u00a0".repeat((s.level ?? 0) + 1)}Under “{s.name}”</option>}</For></select></Row>
        </div>
      </Dialog>
    );
  });
};
/** Project › Subplans: the plans that make up this project, their order and hierarchy. */
export const subplans = () => openDialog(() => {
  const [rows, { refetch }] = createResource(() => tick(), () => call<SubRow[]>("subplan_rows"));
  const [pick, setPick] = createSignal(0);
  const list = () => P().subplans ?? [];
  const set = async (l: SubPlan[], k: number) => { if (await act({ Subplans: { list: l } })) { setPick(k); refetch(); } };
  const move = (d: number) => { const l = [...list()], k = pick(), j = k + d; if (j < 0 || j >= l.length) return; [l[k], l[j]] = [l[j], l[k]]; set(l, j); };
  const indent = (d: number) => { const l = list().map(s => ({ ...s })), k = pick(); if (!l[k]) return; l[k].level = Math.max(0, (l[k].level ?? 0) + d); set(l, k); };
  const remove = async (file: string) => { const o = await call<Out>("remove_subplan", { file }); if (o) { setOut(o); refetch(); } };
  const add = async () => { const o = await call<Out | null>("add_subplan"); if (o) { setOut(o); useTable(table()); refetch(); } };
  const master = () => P().master;
  return (
    <Dialog title="Subplans" wide help="subplans" extra={<Show when={!master()}><button class="btn" onClick={() => { closeDialog(); newSubplan(); }}>New Subplan…</button><button class="btn" onClick={add}>Add Existing…</button><button class="btn pri" disabled={!list().length} onClick={() => { closeDialog(); openAll(); }}>Open All</button></Show>}>
      <Show when={!master()} fallback={<>
        <p>“{P().name}” is a subplan of <b>{master()!.replace(/\.dplan$/i, "")}</b>. The master keeps the project's list of plans, their order and hierarchy.</p>
        <p><button class="btn pri" onClick={() => { closeDialog(); goToLink(master()!, 0); }}>Open the master</button></p></>}>
        <p class="muted">“{P().name}” is the master: these plans make up its project. The order and indent set the hierarchy and how the tabs are grouped; dates come from the links between the plans. Subplans are kept next to the master.</p>
        <table class="tbl subs"><thead><tr><th>Plan</th><th>Start</th><th>Finish</th><th>% Complete</th><th>Tasks</th><th>State</th><th /></tr></thead><tbody>
          <For each={rows() ?? []} fallback={<tr><td colspan="7" class="muted">No subplans yet. New Subplan makes one next to this plan; Add Existing takes a saved plan.</td></tr>}>{(r, k) => (
            <tr classList={{ on: pick() === k() }} onClick={() => setPick(k())}>
              <td style={{ "padding-left": `${10 + r.level * 18}px` }}>{r.level ? "└ " : ""}<b>{r.name}</b></td>
              <td>{r.start ? fmtAt(r.start) : "—"}</td><td>{r.finish ? fmtAt(r.finish - 1) : "—"}</td><td>{r.found ? `${Math.round(r.pct)}%` : "—"}</td><td>{r.found ? r.tasks : "—"}</td>
              <td><span class="pill" classList={{ good: r.open, bad: !r.found }}>{r.open ? "Open" : r.found ? "Not open" : "File not found"}</span></td>
              <td class="acts"><button class="btn sm" disabled={!r.found} onClick={() => { closeDialog(); r.open ? showTab(r.tab) : goToLink(r.file, 0); }}>{r.open ? "Show" : "Open"}</button>
                <button class="x" title="Take it out of this project (the file is kept)" onClick={e => { e.stopPropagation(); remove(r.file); }}>×</button></td>
            </tr>)}</For>
        </tbody></table>
        <Show when={list().length > 1}><div class="subbar"><span class="muted">Selected:</span>
          <button class="btn sm" title="Move up" onClick={() => move(-1)}>↑</button><button class="btn sm" title="Move down" onClick={() => move(1)}>↓</button>
          <button class="btn sm" title="Outdent: one level up the hierarchy" onClick={() => indent(-1)}>⇤ Outdent</button><button class="btn sm" title="Indent: under the plan above" onClick={() => indent(1)}>Indent ⇥</button></div></Show>
      </Show>
    </Dialog>
  );
});
/** Right-click on a subplan's row in the master's combined view. */
export const subRowMenu = (e: MouseEvent, r: GRow) => openMenu({ x: e.clientX, y: e.clientY }, [
  { label: r.suid ? `Go to this task in its plan` : `Open “${r.name}” in its tab`, run: () => goToLink(r.sub!, r.suid ?? 0) },
  ...(r.kind === "subplan" ? [{ label: r.open ? "Collapse" : "Expand (read-only)", run: () => toggleSub(r.sub!) }] : []),
  "-", { label: "Subplans…", run: subplans }, { label: "Hide Subplans in This View", run: () => setShowSubs(false) },
]);
/** A project's colour: the same for its master and subplans. */
const GC = ["#2F6FCF", "#1E7D46", "#B45309", "#6D3FC8", "#0E7490", "#C0392B"];
const gcol = (t: TabInfo) => (t.role ? GC[[...t.project].reduce((h, c) => (h * 31 + c.charCodeAt(0)) >>> 0, 7) % GC.length] : "");
const tabTitle = (t: TabInfo) => `${t.role === "master" ? `Master plan of the project “${t.name}”\n` : t.role === "sub" ? `Subplan of “${t.project}”${t.group < 0 ? " (the master is not open)" : ""}\n` : ""}${t.path ?? "Not saved yet"}`;
/** Ctrl+Tab / Ctrl+Shift+Tab. */
export const nextTab = (d: number) => { const n = out()?.tabs.length ?? 1; if (n > 1) showTab((out()!.tab + d + n) % n); };
const plusMenu = (e: MouseEvent) => openMenu(e, [
  { label: "Blank plan", run: () => import("./store").then(s => s.loadOut("new_plan", { template: null })) },
  { label: "New plan from a template…", run: () => import("./store").then(s => s.setBackstage("new")) },
  { label: "Open…", run: C.open },
  "-",
  ...(out()?.path && out()!.tabs[out()!.tab].role !== "sub" ? [{ label: `New Subplan of “${P().name}”…`, run: newSubplan }] : []),
  "-",
  { label: "Example: a main plan with three linked subplans", run: example },
  { label: "Tutorial: a guided example of every view", run: () => import("./tour").then(m => m.tutorial()) },
]);
export const example = async () => {
  const o = await call<Out | null>("linked_example");
  if (o) { setOut(o); useTable(table()); import("./store").then(s => s.setBackstage(null)); toast("Four linked plans were saved in a new folder and opened in tabs: the main schedule, engineering, procurement and construction.", true); }
};

/** The tab strip: one tab per open plan, with unsaved changes marked; + adds one. A project's plans
 *  sit together in one framed group, labelled with the project: the master first, its subplans after. */
const PlanTab = (p: { t: TabInfo; k: number }) => (
  <div class="plantab" classList={{ on: p.k === out()!.tab, proj: !!p.t.role, master: p.t.role === "master", sub: p.t.role === "sub" }} style={p.t.role === "sub" && p.t.level ? { "padding-left": `${12 + p.t.level * 8}px` } : undefined} title={tabTitle(p.t)} onClick={() => p.k !== out()!.tab && showTab(p.k)} onAuxClick={e => { if (e.button === 1) closeTab(p.k); }} onContextMenu={e => { e.preventDefault(); tabMenu(e, p.k); }}>
    <Show when={p.t.role === "master"}><i class="pm">◆</i><b class="pbadge">Master</b></Show><Show when={p.t.role === "sub"}><i class="ps">{p.t.level ? "↳".repeat(p.t.level) : "▸"}</i><b class="pbadge sub">Sub</b></Show>
    <span class="nm">{p.t.name || "Untitled"}</span><Show when={p.t.dirty}><i class="dot" title="Unsaved changes">•</i></Show>
    <Show when={p.t.moved}><i class="mv" title={`${p.t.moved} ${p.t.moved === 1 ? "task waits" : "tasks wait"} for another plan that has moved. Project › Update Links brings the dates in.`}>⟳ {p.t.moved}</i></Show>
    <button class="x" title="Close (Ctrl+W)" onClick={e => { e.stopPropagation(); closeTab(p.k); }}>×</button>
  </div>
);
/** Runs of tabs of one project (same project name and colour); plans on their own stand alone. */
const tabGroups = () => {
  const g: { key: string; items: number[] }[] = [];
  (out()?.tabs ?? []).forEach((t, k) => { const key = t.role ? gcol(t) + "\u0001" + t.project : "", last = g[g.length - 1]; if (key && last?.key === key) last.items.push(k); else g.push({ key, items: [k] }); });
  return g;
};
export const PlanTabs = () => (
  <div class="plantabs">
    <For each={tabGroups()}>{grp => {
      const first = () => out()!.tabs[grp.items[0]];
      return <Show when={grp.key} fallback={<PlanTab t={first()} k={grp.items[0]} />}>
        <div class="pgroup" style={{ "--g": gcol(first()) }} title={`Project “${first().project}”: ${grp.items.length} open plan${grp.items.length > 1 ? "s" : ""}`}>
          <span class="pglabel">Project · {first().project}{first().role === "sub" && first().group < 0 ? " (master not open)" : ""}</span>
          <div class="pgtabs"><For each={grp.items}>{k => <PlanTab t={out()!.tabs[k]} k={k} />}</For></div>
        </div>
      </Show>;
    }}</For>
    <button class="padd" title="Open another plan in a new tab: a blank plan, a template, a saved plan or an example" onClick={plusMenu}>＋</button>
    <Show when={(() => { const t = out()?.tabs[out()!.tab]; return t?.role === "sub" && t.group < 0 && t.project ? t : null; })()}>{t => (
      <button class="pmaster" title="This plan is a subplan; its master keeps the project's list of plans" onClick={() => goToLink(`${t().project}.dplan`, 0)}>Part of “{t().project}” · Open master</button>)}</Show>
  </div>
);

const KINDS = ["FS", "SS", "FF", "SF"];
const lagText = (m: number) => (m ? `${m > 0 ? "+" : ""}${Math.round((m / P().day_min) * 10) / 10}d` : "");
const linkText = (x: ExtLink) => `${x.plan} › ${x.code ? `${x.code} ` : ""}${x.name}`;

/** Link the task `uid` to tasks in other open plans: pick a plan, a task, the type and the lag. */
export const linkDialog = (uid: number) => openDialog(() => {
  const [open] = createResource(() => call<Target[]>("link_targets"));
  const [files, setFiles] = createSignal<Target[]>([]);
  const targets = () => [...(open() ?? []), ...files()];
  const browse = async () => { const t = await call<Target | null>("link_target_file"); if (t) { t.tab = -1 - files().length; setFiles([...files(), t]); setTab(t.tab); setPick(0); } };
  const [info] = createResource(() => [tick(), uid], () => call<{ task: { name: string; ext?: ExtLink[] } }>("task_info", { uid }));
  const [links, setLinks] = createSignal<ExtLink[] | null>(null);
  const cur = () => links() ?? info()?.task.ext ?? [];
  const [tab, setTab] = createSignal(-1), [pick, setPick] = createSignal(0), [kind, setKind] = createSignal("FS"), [lag, setLag] = createSignal("0"), [q, setQ] = createSignal("");
  const target = () => targets().find(t => t.tab === tab()) ?? targets()[0];
  const rows = createMemo(() => (target()?.tasks ?? []).filter(t => !q() || `${t[1]} ${t[2]}`.toLowerCase().includes(q().toLowerCase())));
  const add = () => {
    const t = target(), x = t?.tasks.find(r => r[0] === pick());
    if (!t || !x) return toast("Choose a task in the other plan.");
    if (!t.path) return toast(`Save “${t.name}” first, so the link knows its file.`);
    const lagMin = Math.round((parseFloat(lag()) || 0) * P().day_min);
    setLinks([...cur().filter(l => !(l.file === t.path && l.uid === x[0])), { file: t.path, uid: x[0], kind: kind(), lag: lagMin, plan: t.name, code: x[1], name: x[2], start: x[5], finish: x[6] }]);
  };
  return (
    <Dialog title="Link to Plan" wide help="linkedplans" okText="OK" ok={() => (links() ? act({ ExtSet: { uid, links: links() } }) : undefined)}>
      <p>Predecessors of <b>{info()?.task.name}</b> in other plans. The task waits for them like for a normal link; <b>Update Links</b> keeps their dates current.</p>
      <table class="tbl"><thead><tr><th>Plan › task</th><th>Type</th><th>Last known finish</th><th /></tr></thead><tbody>
        <For each={cur()} fallback={<tr><td colspan="4" class="muted">No links to other plans yet.</td></tr>}>{(x, k) => <tr><td>{linkText(x)}</td><td>{x.kind}{lagText(x.lag)}</td><td>{x.finish ? fmtAt(x.finish - 1) : "—"}</td>
          <td><button class="x" title="Remove this link" onClick={() => setLinks(cur().filter((_, j) => j !== k()))}>×</button></td></tr>}</For>
      </tbody></table>
      <Show when={targets().length} fallback={<p class="muted">Open the other plan in a tab (File › Open, or ＋ beside the tabs), or <a class="link" onClick={browse}>browse for its file</a>.</p>}>
        <h4 class="sub">Add a link</h4>
        <div class="form">
          <Row label="Plan"><span class="linklag"><select style={{ flex: "1" }} onChange={e => { setTab(+e.currentTarget.value); setPick(0); }}><For each={targets()}>{t => <option value={t.tab} selected={t.tab === target()?.tab}>{t.name}{t.path ? "" : " (not saved yet)"}{t.tab < 0 ? " (file, not open)" : ""}</option>}</For></select>
            <button class="btn sm" title="Link to a saved plan that is not open" onClick={browse}>Browse…</button></span></Row>
          <Row label="Find"><input value={q()} placeholder="Part of the task's name or ID" onInput={e => setQ(e.currentTarget.value)} /></Row>
        </div>
        <div class="linkpick"><For each={rows()}>{t => <button classList={{ on: t[0] === pick(), sum: t[4] }} style={{ "padding-left": `${8 + t[3] * 14}px` }} onClick={() => setPick(t[0])} onDblClick={add}>
          <span class="mono">{t[1]}</span><span>{t[2]}</span><span class="muted">{fmtAt(t[6] - 1)}</span></button>}</For></div>
        <div class="form">
          <Row label="Type and lag"><span class="linklag"><select onChange={e => setKind(e.currentTarget.value)}><For each={KINDS}>{k => <option selected={k === kind()}>{k}</option>}</For></select>
            <input type="number" step="0.5" style={{ width: "90px" }} value={lag()} onInput={e => setLag(e.currentTarget.value)} /><span class="muted">days (negative for a lead)</span>
            <button class="btn sm pri" onClick={add}>Add Link</button></span></Row>
        </div>
      </Show>
    </Dialog>
  );
});

/** Brings links up to date (this plan, or every open plan) and says what changed. */
export const updateLinks = async (all: boolean) => {
  const r = await call<[Out, string]>("update_links", { all });
  if (r) { setOut(r[0]); toast(r[1], true); }
  return r;
};
/** Link to Plan for the selected task. */
export const linkSelected = () => { const u = C.ids()[0]; if (u) linkDialog(u); else toast("Select a task first, then link it to a task in another plan."); };

/** The Linked Plans panel: every link of this plan, whether the other plan has moved, and Update Links. */
export const linkedPlans = () => openDialog(() => {
  const [states, { refetch }] = createResource(() => tick(), () => call<LinkState[]>("link_states"));
  const [users] = createResource(() => tick(), () => call<{ uid: number; task: string; tab: number; by: string }[]>("link_users"));
  const update = (all: boolean) => updateLinks(all).then(() => refetch());
  const label = (s: string) => (s === "ok" ? "Up to date" : s === "changed" ? "Moved – update" : `Not found: ${s}`);
  return (
    <Dialog title="Linked Plans" wide help="linkedplans" extra={<><button class="btn" onClick={() => update(false)}>Update This Plan</button><button class="btn pri" onClick={() => update(true)}>Update All Open Plans</button></>}>
      <h4 class="sub"><span class="xin">⇠</span> This plan waits for</h4>
      <p class="muted">Tasks of “{P().name}” that wait for tasks in other plans. When another plan moves, its link shows <b>Moved</b> until you update. Plans that are not open are read from their files.</p>
      <table class="tbl linkstates"><thead><tr><th>Task here</th><th>Waits for</th><th>Type</th><th>Last known finish</th><th>Finish now</th><th>State</th><th /></tr></thead><tbody>
        <For each={states() ?? []} fallback={<tr><td colspan="7" class="muted">No links to other plans. Select a task and use Project › Link to Plan.</td></tr>}>{s => <tr classList={{ warn: s.state !== "ok" }}>
          <td>{s.task}</td><td>{linkText(s.link)}</td><td>{s.link.kind}{lagText(s.link.lag)}</td><td>{s.link.finish ? fmtAt(s.link.finish - 1) : "—"}</td>
          <td>{s.now_finish ? fmtAt(s.now_finish - 1) : "—"}</td><td><span class="pill" classList={{ good: s.state === "ok", bad: s.state !== "ok" }}>{label(s.state)}</span></td>
          <td><button class="btn sm" onClick={() => { closeDialog(); linkDialog(s.uid); }}>Edit…</button></td></tr>}</For>
      </tbody></table>
      <h4 class="sub"><span class="xout">⇢</span> Other open plans wait for</h4>
      <table class="tbl linkstates"><thead><tr><th>Task here</th><th>Waited for by</th><th /></tr></thead><tbody>
        <For each={users() ?? []} fallback={<tr><td colspan="3" class="muted">No other open plan waits for this plan's tasks.</td></tr>}>{u => <tr>
          <td>{u.task}</td><td>{u.by}</td><td><button class="btn sm" onClick={() => { closeDialog(); showTab(u.tab); }}>Show plan</button></td></tr>}</For>
      </tbody></table>
    </Dialog>
  );
});
