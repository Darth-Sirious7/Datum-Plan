// File (backstage): home with templates and recent plans, export, print, options, updates.
import { For, Show, createSignal, createResource, onMount, JSX } from "solid-js";
import { PAPERS, PrintPage } from "./print";
import { RoadmapPage } from "./roadmap";
import { Currency, Options, Out } from "./api";
import { backstage, setBackstage, call, loadOut, out, setOut, opts, setOpts, toast, setSetupOpen, act, setThemeRaw, applyTheme } from "./store";
import { C } from "./cmds";
import * as D from "./dialogs";
import { Icon, Text, Pick, Check } from "./ui";
import { listen, invoke } from "./api";

type Tpl = { id: string; name: string; desc: string; color: string; tasks: number; days: number; bars: [number, number, number][] };
type Info = { current: string; latest: string | null; beta: boolean; available: boolean; notes: string; published: string; url: string | null; sums: string | null; size: number; page: string };
export const [update, setUpdate] = createSignal<Info | null>(null);
export const [checking, setChecking] = createSignal(false), [checkedAt, setCheckedAt] = createSignal("");
export const checkUpdate = async (quiet: boolean) => {
  setChecking(true);
  try { setUpdate(await invoke<Info>("check_update")); setCheckedAt(new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })); if (!quiet) toast(update()?.available ? `Version ${update()!.latest} is available.` : "You have the latest version.", true); }
  catch (e) { if (!quiet) toast(`Could not check for updates: ${e}`); }
  finally { setChecking(false); }
};
const GH = "https://github.com/Edgar-Mendonca/Datum-Plan";
/** Links to the project on GitHub: code, releases (change log) and issues (report a problem). */
export const GitHubLinks = () => (
  <div class="ghlinks">
    <For each={[["", "GitHub", "The project and its code"], ["/releases", "Releases", "Every version and its change log"], ["/issues/new", "Report an issue", "Tell us about a bug or an idea"]]}>{([path, l, t]) =>
      <button class="btn" title={t} onClick={() => call("open_url", { url: GH + path })}><svg class="ic"><use href={path === "/issues/new" ? "#i-bug" : path ? "#i-tag" : "#i-github"} /></svg>{l}</button>}</For>
  </div>
);

const CURRENCIES: [string, string, number][] = [
  ["USD", "$", 2], ["EUR", "€", 2], ["GBP", "£", 2], ["INR", "₹", 2], ["AED", "AED", 2], ["SAR", "SAR", 2], ["QAR", "QAR", 2], ["KWD", "KWD", 3], ["OMR", "OMR", 3], ["BHD", "BHD", 3],
  ["JPY", "¥", 0], ["CNY", "¥", 2], ["AUD", "A$", 2], ["CAD", "C$", 2], ["CHF", "CHF", 2], ["SGD", "S$", 2], ["HKD", "HK$", 2], ["NZD", "NZ$", 2], ["ZAR", "R", 2], ["BRL", "R$", 2],
  ["MXN", "MX$", 2], ["SEK", "kr", 2], ["NOK", "kr", 2], ["DKK", "kr", 2], ["PLN", "zł", 2], ["CZK", "Kč", 2], ["HUF", "Ft", 0], ["TRY", "₺", 2], ["RUB", "₽", 2], ["KRW", "₩", 0],
  ["IDR", "Rp", 0], ["MYR", "RM", 2], ["THB", "฿", 2], ["PHP", "₱", 2], ["VND", "₫", 0], ["PKR", "Rs", 2], ["BDT", "৳", 2], ["LKR", "Rs", 2], ["NGN", "₦", 2], ["KES", "KSh", 2],
  ["EGP", "E£", 2], ["MAD", "MAD", 2], ["ILS", "₪", 2], ["CLP", "CLP$", 0], ["COP", "COL$", 0], ["PEN", "S/", 2], ["ARS", "AR$", 2], ["UAH", "₴", 2], ["RON", "lei", 2], ["ISK", "kr", 0],
];
export const currencyFor = (code: string): Currency => { const c = CURRENCIES.find(x => x[0] === code.toUpperCase()); return { code: code.toUpperCase(), symbol: c?.[1] ?? code.toUpperCase(), decimals: c?.[2] ?? 2, after: !!c && c[1].length > 1 && /^[A-Z]{3}$/.test(c[1]) }; };
export const CurrencyPick = (p: { value: Currency; set: (c: Currency) => void }) => <>
  <input list="iso4217" value={p.value.code} onChange={e => p.set(currencyFor(e.currentTarget.value))} style={{ width: "90px" }} />
  <datalist id="iso4217"><For each={CURRENCIES}>{c => <option value={c[0]}>{c[1]}</option>}</For></datalist>
  <span class="muted"> symbol </span><input value={p.value.symbol} style={{ width: "60px" }} onChange={e => p.set({ ...p.value, symbol: e.currentTarget.value })} />
  <span class="muted"> decimals </span><input type="number" min="0" max="4" value={p.value.decimals} style={{ width: "50px" }} onChange={e => p.set({ ...p.value, decimals: +e.currentTarget.value })} />
  <label class="chk"><input type="checkbox" checked={p.value.after} onChange={e => p.set({ ...p.value, after: e.currentTarget.checked })} />after amount</label>
</>;
/** Release notes: headings, bullets and bold, enough for the changelog. */
const md = (s: string) => s.split("\n").map(l => {
  const e = l.replace(/[&<>]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c]!).replace(/\*\*(.+?)\*\*/g, "<b>$1</b>").replace(/`(.+?)`/g, "<code>$1</code>");
  return l.startsWith("## ") ? `<h3>${e.slice(3)}</h3>` : l.startsWith("- ") ? `<li>${e.slice(2)}</li>` : e.trim() ? `<p>${e}</p>` : "";
}).join("");

const Mini = (p: { t: Tpl }) => (
  <svg viewBox="0 0 100 60" preserveAspectRatio="none">
    <For each={p.t.bars.slice(0, 14)}>{([k, a, b], i) => k === 2 ? <path d={`M${a * 92 + 4} ${i() * 4 + 3}l2 2l-2 2l-2 -2z`} fill="#F2B233" /> : <rect x={a * 92 + 4} y={i() * 4 + 2} width={Math.max(1.5, (b - a) * 92)} height={k ? 1.6 : 2.6} fill={k ? "#14325E" : p.t.color} />}</For>
  </svg>
);

const Home = () => {
  const [tpls] = createResource(() => call<Tpl[]>("templates"));
  const [recent] = createResource(() => call<{ path: string; name: string; modified: string }[]>("recent"));
  const neu = (template: string | null) => loadOut("new_plan", { template }).then(o => { if (o && !o.project.setup_done) setSetupOpen(true); });
  return (
    <div class="home">
      <div>
        <h1>Good {new Date().getHours() < 12 ? "morning" : new Date().getHours() < 18 ? "afternoon" : "evening"}</h1>
        <div class="tiles">
          <button class="tile" onClick={() => neu(null)}><div class="pic"><svg class="ic"><use href="#i-new" /></svg></div><b>Blank Project</b><span>Start with the Project Setup panel.</span></button>
          <For each={tpls() ?? []}>{t => <button class="tile" onClick={() => neu(t.id)}><div class="pic"><Mini t={t} /><small>{t.tasks} tasks · {t.days} days</small></div><b>{t.name}</b><span>{t.desc}</span></button>}</For>
          <button class="tile" onClick={() => import("./tour").then(m => m.tutorial())}><div class="pic"><svg class="ic"><use href="#i-help" /></svg><small>Every view · notes on each</small></div><b>Tutorial (guided example)</b><span>An office fit-out in progress that uses every part of Datum Plan, with a note on each view saying how it was filled in.</span></button>
          <button class="tile" onClick={() => import("./plans").then(m => m.example())}><div class="pic"><svg class="ic"><use href="#i-link" /></svg><small>4 plans · 13 links</small></div><b>Linked plans (example)</b><span>A compressor station: a main schedule and engineering, procurement and construction plans linked to each other, opened in tabs.</span></button>
          <button class="tile" onClick={() => C.importFile()}><div class="pic"><svg class="ic"><use href="#i-import" /></svg></div><b>Import</b><span>XML and XER project files, Excel, CSV or JSON.</span></button>
          <button class="tile" onClick={() => D.guide()}><div class="pic"><svg class="ic"><use href="#i-pdf" /></svg></div><b>Guide</b><span>The step-by-step guide (PDF), in English or 中文.</span></button>
          <button class="tile" onClick={() => setBackstage("roadmap")}><div class="pic"><svg class="ic"><use href="#i-list" /></svg></div><b>Roadmap</b><span>Everything Datum Plan will do, and when.</span></button>
        </div>
        <h2>Recent</h2>
        <table class="recent"><tbody>
          <For each={recent() ?? []} fallback={<tr><td class="muted">Plans you open or save appear here.</td></tr>}>{r => <tr class="f" onClick={() => loadOut("open", { path: r.path })}><td><b>{r.name}</b><div class="p">{r.path}</div></td><td class="p">{r.modified}</td></tr>}</For>
        </tbody></table>
      </div>
      <UpdateCard />
    </div>
  );
};

const UpdateCard = () => {
  const [pct, setPct] = createSignal<number | null>(null);
  const [ver] = createResource(() => call<string>("version"));
  onMount(() => { listen("update-progress", (p: number) => setPct(p)); if (!update()) checkUpdate(true); });
  const install = async () => { const u = update(); if (!u?.url) return; setPct(0); await call("install_update", { url: u.url, sums: u.sums }); setPct(null); };
  return (
    <div class="card">
      <div class="brand"><img src="logo.svg" alt="" /><div><b>Datum Plan</b><span class="muted">Version {ver()}{opts().beta ? " · beta channel" : ""}</span></div></div>
      <Show when={update()} fallback={<><span class="pill">{checking() ? "Checking for updates…" : "Not checked yet"}</span><button class="btn wide" disabled={checking()} onClick={() => checkUpdate(false)}>{checking() ? <><i class="spin" /> Checking…</> : "Check for updates"}</button><GitHubLinks /></>}>{u => <>
        <Show when={u().available} fallback={<span class="pill ok">You have the latest version</span>}><span class="pill warn">Version {u().latest}{u().beta ? " (beta)" : ""} is ready</span></Show>
        <Show when={u().available && u().url}><button class="btn pri wide" disabled={pct() !== null} onClick={install}>{pct() === null ? `Update now (${Math.round(u().size / 1048576)} MB)` : `Downloading… ${pct()}%`}</button></Show>
        <Show when={pct() !== null}><div class="prog"><i style={{ width: `${pct()}%` }} /></div></Show>
        <button class="btn wide" disabled={checking()} onClick={() => checkUpdate(false)}>{checking() ? <><i class="spin" /> Checking…</> : "Check again"}</button>
        <Show when={checkedAt() && !checking()}><span class="muted small">Checked at {checkedAt()}</span></Show>
        <GitHubLinks />
        <div class="notes"><h4>What's new{u().latest ? ` in ${u().latest}` : ""}</h4><div innerHTML={md(u().notes || "Release notes will appear here.")} /></div>
      </>}</Show>
    </div>
  );
};

const Export = () => (
  <div><h1>Export</h1>
    <div class="tiles">
      <For each={[["xlsx", "Excel workbook", "The current table's columns, with outline indent."], ["xml", "Project XML", "For scheduling programs that read project XML files."], ["xer", "XER", "For scheduling programs that read XER files."],
        ["pmxml", "PMXML", "XML with WBS, activities, relationships and resources."], ["mpx", "MPX", "The classic exchange format."], ["csv", "CSV", "Plain text, the current table's columns."],
        ["html", "Web page", "A branded report of the current table."], ["ics", "iCalendar", "Tasks and milestones as calendar events."], ["json", "JSON", "For scripts and other tools."]]}>{([k, n, d]) =>
        <button class="tile" onClick={() => C.exportAs(k)}><div class="pic"><svg class="ic"><use href="#i-export" /></svg><small>.{k}</small></div><b>{n}</b><span>{d}</span></button>}</For>
    </div>
    <h2>Pictures and PDF</h2>
    <div class="tiles"><button class="tile" onClick={() => setBackstage("print")}><div class="pic"><svg class="ic"><use href="#i-print" /></svg></div><b>Print or save as PDF</b><span>Choose “Save as PDF” as the printer to make a PDF of the current view.</span></button>
      <button class="tile" onClick={() => { setBackstage(null); copyPicture(); }}><div class="pic"><svg class="ic"><use href="#i-copy" /></svg></div><b>Save picture (PNG)</b><span>A picture of the Gantt chart as shown.</span></button></div>
  </div>
);
const copyPicture = async () => {
  const cv = document.querySelector(".chart canvas.bars") as HTMLCanvasElement | null, sc = document.querySelector(".chart canvas.scale") as HTMLCanvasElement | null;
  if (!cv || !sc) return toast("Open the Gantt chart first.");
  const out = document.createElement("canvas"); out.width = cv.width; out.height = cv.height + sc.height;
  const ctx = out.getContext("2d")!; ctx.fillStyle = "#fff"; ctx.fillRect(0, 0, out.width, out.height); ctx.drawImage(sc, 0, 0); ctx.drawImage(cv, 0, sc.height);
  const path = await call<string | null>("save_file", { name: "Gantt chart.png", data: out.toDataURL("image/png") });
  if (path) toast(`Saved ${path}`, true);
};

const DAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"].map((d, k) => [String(k), d] as [string, string]);
const VIEWS: [string, string][] = [["gantt", "Gantt Chart"], ["tracking", "Tracking Gantt"], ["dashboard", "Dashboard"], ["sheets", "Sheets"], ["network", "Network Diagram"], ["calendar", "Calendar"]];
/** One option: name and a one-line explanation on the left, the control on the right. */
const Opt = (p: { l: string; hint?: string; children: JSX.Element }) => <div class="opt"><div><b>{p.l}</b>{p.hint && <small>{p.hint}</small>}</div><div class="ctl">{p.children}</div></div>;
const Sec = (p: { t: string; about: string; children: JSX.Element }) => <section class="osec"><h3>{p.t}</h3><p class="muted">{p.about}</p>{p.children}</section>;

const OptionsPage = () => {
  const [o, setO] = createSignal<Options>({ ...opts() });
  const save = async () => { const r = await call<Out>("set_options", { opts: o() }); if (r) { setOpts(o()); setOut(r); toast("Options saved.", true); } };
  const put = <K extends keyof Options>(k: K, v: Options[K]) => setO({ ...o(), [k]: v });
  const folder = async () => { const r = await call<[string, unknown[]]>("workspace", { choose: true }); if (r?.[0]) put("workspace", r[0]); };
  return (
    <div class="options"><h1>Options</h1>
      <p class="muted">Settings for Datum Plan on this computer. Defaults for new plans apply when a plan is created; change an open plan in Project › Project Setup.</p>
      <Sec t="General" about="How dates and the window look.">
        <Opt l="Date format" hint="Used in the task sheet, reports and printouts."><Pick value={o().dateFmt} set={v => put("dateFmt", v)} options={[["%d-%b-%y", "24-Oct-26"], ["%d/%m/%Y", "24/10/2026"], ["%m/%d/%Y", "10/24/2026"], ["%Y-%m-%d", "2026-10-24"], ["%d %b %Y", "24 Oct 2026"], ["%a %d-%b-%y", "Sat 24-Oct-26"]]} /></Opt>
        <Opt l="Theme" hint="Auto follows the Windows light or dark setting."><Pick value={o().theme || "auto"} set={v => { put("theme", v); setThemeRaw(v); applyTheme(); }} options={[["auto", "Auto (follow Windows)"], ["light", "Light"], ["dark", "Dark"]]} /></Opt>
      </Sec>
      <Sec t="Start-up and saving" about="What happens when Datum Plan starts, and how your work is protected.">
        <Opt l="When Datum Plan starts" hint="Open the File page, or reopen the plan you used last."><Pick value={o().openLast ? "last" : "home"} set={v => put("openLast", v === "last")} options={[["home", "Show the File page"], ["last", "Reopen the last plan"]]} /></Opt>
        <Opt l="Open plans in" hint="The view shown when a plan is opened or created."><Pick value={o().startView || "gantt"} set={v => put("startView", v)} options={VIEWS} /></Opt>
        <Opt l="Recovery copy every" hint="A copy is kept while you work, so a crash or power cut loses little. 0 turns it off."><span class="inline"><input type="number" min="0" style={{ width: "70px" }} value={o().autosave} onChange={e => put("autosave", +e.currentTarget.value)} /> minutes</span></Opt>
      </Sec>
      <Sec t="New plans" about="Defaults given to every new plan. Each plan keeps its own settings afterwards.">
        <Opt l="Project Setup panel" hint="Ask for the project name, client, dates and calendar when a plan is created."><Check label="Show for new plans" value={o().setupPanel} set={v => put("setupPanel", v)} /></Opt>
        <Opt l="New tasks are" hint="Auto scheduled tasks move with their links; manual ones stay where you put them."><Pick value={o().manualDefault ? "1" : "0"} set={v => put("manualDefault", v === "1")} options={[["0", "Auto Scheduled"], ["1", "Manually Scheduled"]]} /></Opt>
        <Opt l="Reporting week ends on" hint="Cut-off day for weekly progress reports and S-curves."><Pick value={String(o().weekEnd ?? 4)} set={v => put("weekEnd", +v)} options={DAYS} /></Opt>
        <Opt l="Reporting month closes on" hint="Cut-off day for monthly reports."><Pick value={String(o().monthEnd ?? 0)} set={v => put("monthEnd", +v)} options={[["0", "Last day of the month"], ...Array.from({ length: 28 }, (_, k) => [String(k + 1), `Day ${k + 1}`] as [string, string])]} /></Opt>
        <Opt l="Currency" hint="Symbol and decimals for costs in new plans."><span class="inline"><CurrencyPick value={o().currency} set={c => put("currency", c)} /></span></Opt>
        <Show when={out()}><Opt l="Currency of this plan" hint="Changes the open plan right away."><span class="inline"><CurrencyPick value={out()!.project.currency} set={c => act({ ProjectSet: { field: "currency", value: c } })} /></span></Opt></Show>
      </Sec>
      <Sec t="Printing and branding" about="Your company name and logo go in report headers and printed page headers (File › Print › Page Setup).">
        <Opt l="Paper" hint="Default paper for new printouts."><span class="inline"><Pick value={o().paper || "A4"} set={v => put("paper", v)} options={PAPERS.map(p => p[0])} /><Pick value={o().landscape === false ? "0" : "1"} set={v => put("landscape", v === "1")} options={[["1", "Landscape"], ["0", "Portrait"]]} /></span></Opt>
        <Opt l="Company" hint="Shown on reports and printouts."><Text value={o().company} set={v => put("company", v)} /></Opt>
        <Opt l="Company logo" hint="PNG or JPG. Clients' logos are set per plan in Page Setup."><span class="inline">{o().logo && <img src={o().logo} class="logoprev" />}<button class="btn sm" onClick={async () => { const l = await call<string | null>("pick_logo"); if (l) put("logo", l); }}>Choose picture…</button>{o().logo && <button class="btn sm" onClick={() => put("logo", "")}>Remove</button>}
          <Show when={out()}><button class="btn sm" onClick={() => act({ Batch: [{ ProjectSet: { field: "logo", value: o().logo } }, { ProjectSet: { field: "company", value: o().company } }] })}>Use for this plan</button></Show></span></Opt>
      </Sec>
      <Sec t="Portfolio" about="The folder (with subfolders) whose plans appear in Report › Portfolio.">
        <Opt l="Workspace folder" hint={o().workspace || "Not chosen yet."}><button class="btn sm" onClick={folder}><Icon n="folder" /> Choose folder…</button></Opt>
      </Sec>
      <Sec t="Updates" about="Datum Plan checks GitHub for new versions when it starts. Updates install from File › Updates.">
        <Opt l="Beta versions" hint="Get new features early. They may have bugs."><Check label="Get beta versions" value={o().beta} set={v => put("beta", v)} /></Opt>
      </Sec>
      <p class="osave"><button class="btn pri" onClick={save}>Save options</button><span class="muted">Datum Plan keeps everything on this computer. Its only connection is the update check to GitHub.</span></p>
    </div>
  );
};

export const Backstage = () => (
  <div class="bs">
    <aside>
      <button class="back" onClick={() => out() && setBackstage(null)} title="Back">←</button>
      <For each={[["home", "home", "Home"], ["new", "new", "New"], ["open", "open", "Open"], ["import", "import", "Import"], ["save", "save", "Save"], ["saveas", "save", "Save As"], ["export", "export", "Export"], ["print", "print", "Print"], ["close", "clear", "Close"], ["closeall", "clear", "Close All"], ["roadmap", "list", "Roadmap"]]}>{([k, i, l]) =>
        <button classList={{ on: backstage() === k }} onClick={() => {
          if (k === "open") C.open(); else if (k === "import") C.importFile(); else if (k === "save") C.save(); else if (k === "saveas") C.save(true);
          else if (k === "close") import("./plans").then(m => { setBackstage(null); m.closeTab(out()!.tab); });
          else if (k === "closeall") import("./plans").then(m => { setBackstage(null); m.closeAll(); });
          else setBackstage(k === "new" ? "home" : k);
        }}><Icon n={i} size={22} />{l}</button>}</For>
      <span class="grow" />
      <button classList={{ on: backstage() === "options" }} onClick={() => setBackstage("options")}><Icon n="opts" size={22} />Options</button>
      <button classList={{ on: backstage() === "about" }} onClick={() => setBackstage("about")}><Icon n="upd" size={22} />Updates</button>
    </aside>
    <section class="bsmain">
      <Show when={backstage() === "home"}><Home /></Show>
      <Show when={backstage() === "export"}><Export /></Show>
      <Show when={backstage() === "options"}><OptionsPage /></Show>
      <Show when={backstage() === "print"}><PrintPage /></Show>
      <Show when={backstage() === "roadmap"}><RoadmapPage /></Show>
      <Show when={backstage() === "about"}><div class="about"><h1>Updates</h1><UpdateCard /><p><button class="btn" onClick={() => D.shortcuts()}>Keyboard shortcuts</button></p></div></Show>
    </section>
  </div>
);
