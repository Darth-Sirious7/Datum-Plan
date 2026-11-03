// Screenshots for the guide (docs/guide/img): the built window (ui/dist) with a plan in progress
// (cargo run -p datum-core --example dump guide > tests_ui/guide_fixture.json) behind the mocked
// bridge, with numbered markers drawn on the page. Run: npm run build:ui && node tests_ui/guide_shots.cjs
const { chromium } = require("playwright");
const fs = require("fs"), http = require("http"), path = require("path");
const root = path.join(__dirname, "../ui/dist"), out = path.join(__dirname, "../docs/guide/img");
const fix = JSON.parse(fs.readFileSync(path.join(__dirname, "guide_fixture.json")));
const types = { ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".woff2": "font/woff2", ".woff": "font/woff", ".html": "text/html" };
const srv = http.createServer((q, r) => {
  const f = path.join(root, q.url === "/" ? "index.html" : decodeURIComponent(q.url.split("?")[0]));
  fs.readFile(f, (e, b) => { if (e) { r.writeHead(404); return r.end(); } r.writeHead(200, { "content-type": types[path.extname(f)] ?? "application/octet-stream" }); r.end(b); });
});

(async () => {
  await new Promise(ok => srv.listen(0, ok));
  const b = await chromium.launch(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {});
  const page = async fx => { const p = await b.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1.5 });
  await p.addInitScript(f => {
    const opts = { dateFmt: "%d-%b-%y", autosave: 0, beta: false, currency: { code: "USD", symbol: "$", decimals: 2, after: false }, company: "Northwind Engineering", logo: "", workspace: "", setupPanel: false, theme: "light" };
    const m = { ...f, options: opts, startup_file: null, recovery: null, check_update: null, templates: [], recent: [], version: "0.5.3", checks: [], sheet_view: f.sheet_view ?? null, task_info: null };
    window.__TAURI__ = { core: { invoke: async (c, a) => {
      if (c === "query" && JSON.stringify(a).includes('"timeline"')) return m["query:timeline"];
      if (c === "query" && JSON.stringify(a).includes('"astart"')) return m["query:tracking"];
      if (a && a.by && `${c}:${a.by}` in m) return m[`${c}:${a.by}`];
      if (c === "dash_eval") return a.items.map(i => (f.dash_map ?? {})[i[1] + "\u0001" + i[0]] ?? []);
      if (c in m) return m[c];
      if (c === "apply") return f.state;
      return null;
    } }, event: { listen: async () => () => {} } };
  }, fx);
  await p.goto(`http://localhost:${srv.address().port}/`);
  await p.waitForTimeout(800);
  await p.click(".bs aside .back");
  await p.waitForTimeout(600);
  return p; };
  let p = await page(fix);

  const wait = ms => p.waitForTimeout(ms);
  const side = async t => { await p.click(`.side button[title="${t}"]`); await wait(700); };
  const tab = async t => { await p.click(`.tabs > button:text-is('${t}')`); await wait(250); };
  /** Numbered markers: [selector or {x,y,w,h}, number, optional nth match]. */
  const mark = marks => p.evaluate(ms => {
    document.querySelectorAll(".gmark").forEach(e => e.remove());
    for (const [s, n, k, pos] of ms) {
      // "grp:Insert" is the ribbon group with that caption; "text:Tell me" the smallest element with that text.
      const find = q => q.startsWith("grp:") ? [...document.querySelectorAll(".ribbon .grp")].find(g => (g.querySelector(".cap")?.textContent ?? "").replace("?", "").trim().toLowerCase() === q.slice(4).toLowerCase())
        : q.startsWith("text:") ? [...document.querySelectorAll("body *")].filter(e => e.textContent.includes(q.slice(5)) && e.getBoundingClientRect().width > 0).sort((a, b) => a.textContent.length - b.textContent.length)[0]
        : document.querySelectorAll(q)[k ?? 0];
      const el = typeof s === "string" ? find(s) : null;
      const r = el ? el.getBoundingClientRect() : s;
      if (!r || !r.width) continue;
      const box = document.createElement("div");
      box.className = "gmark";
      Object.assign(box.style, { position: "fixed", left: `${r.x - 3}px`, top: `${r.y - 3}px`, width: `${r.width + 6}px`, height: `${r.height + 6}px`, border: "3px solid #E4572E", borderRadius: "6px", zIndex: 99999, pointerEvents: "none", boxShadow: "0 0 0 2px rgba(255,255,255,.85)" });
      const dot = document.createElement("div");
      dot.textContent = n;
      // Outside the top-left corner, or beside the right edge where that corner is off the page.
      const where = pos ?? (r.x < 14 || r.y < 14 ? "right" : "corner");
      const at = { corner: ["-14px", "-14px"], right: [`${r.width + 10}px`, `${Math.min(r.height / 2 - 10, 40)}px`], in: ["6px", "6px"], below: ["0px", `${r.height + 10}px`] }[where];
      Object.assign(dot.style, { position: "absolute", left: at[0], top: at[1], width: "26px", height: "26px", borderRadius: "13px", background: "#E4572E", color: "#fff", font: "700 14px/26px 'IBM Plex Sans', sans-serif", textAlign: "center", boxShadow: "0 1px 3px rgba(0,0,0,.35)" });
      box.append(dot);
      document.body.append(box);
    }
  }, marks);
  /** `clip`: only part of the window (x, y, width, height), or a selector to frame with a margin. */
  const shot = async (name, marks = [], clip) => {
    await mark(marks); await wait(150);
    if (typeof clip === "string") clip = await p.evaluate(s => { const r = document.querySelector(s).getBoundingClientRect(); return { x: Math.max(0, r.x - 24), y: Math.max(0, r.y - 24), width: r.width + 48, height: r.height + 48 }; }, clip);
    await p.screenshot({ path: path.join(out, `${name}.png`), clip });
    await mark([]); console.log("shot", name);
  };
  const TOP = { x: 0, y: 0, width: 1440, height: 250 };
  const box = sel => p.evaluate(s => { const r = document.querySelector(s)?.getBoundingClientRect(); return r && { x: r.x, y: r.y, width: r.width, height: r.height }; }, sel);

  // 1. The window.
  const tabsBox = await p.evaluate(() => { const t = [...document.querySelectorAll(".tabs > button")]; const a = t[0].getBoundingClientRect(), z = t[t.length - 1].getBoundingClientRect(); return { x: a.x + 2, y: a.y + 2, width: z.right - a.x - 2, height: a.height - 4 }; });
  await shot("window", [[tabsBox, 1, 0, "right"], [".ribbon", 2], [".side", 3, 0, "in"], [".tlwrap", 4], [".grid", 5], [".chart", 6], [".details", 7], ["text:Tell me what you want", 8]]);
  // 2. The task sheet: duration and predecessors, and the Schedule group.
  const durHead = await p.$$eval(".ghead .hc", hs => { const h = hs.find(x => x.textContent.trim().startsWith("Durati")); const r = h.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: 400 }; });
  const preHead = await p.$$eval(".ghead .hc", hs => { const h = hs.find(x => x.textContent.trim().startsWith("Pre")); const r = h.getBoundingClientRect(); return { x: r.x, y: r.y, width: Math.min(r.width, 60), height: 400 }; });
  await shot("sheet", [[".grid .gbody .row", 1, 9], [durHead, 2], [preHead, 3], ["grp:Insert", 4]]);
  // 3. Schedule tab: links, outline, levelling, paths.
  await tab("Schedule");
  await shot("schedule-tab", [["grp:Links", 1, 0, "below"], ["grp:Outline", 2], ["grp:Leveling", 3], ["grp:Paths", 4]], TOP);
  // 4. Progress tab.
  await tab("Progress");
  await shot("progress-tab", [["grp:Update", 1], ["grp:Period", 2], ["grp:Baselines", 3], ["grp:Progress Reports", 4], ["grp:Measure", 5]], TOP);
  await tab("Task");
  // 5. Views.
  await side("Tracking"); await shot("tracking", [[".chart", 1]]);
  await side("Network"); await shot("network");
  await side("Update"); await shot("update");
  await side("Resources"); await shot("resources", [["text:Group", 1], ["text:Std. Rate", 2]], { x: 66, y: 160, width: 1374, height: 320 });
  // Sheets: a weekly report that reads the plan, then a formula being typed.
  await side("Sheets");
  const cells = (r0, c0, r1, c1) => p.evaluate(([r0, c0, r1, c1]) => { const rows = document.querySelectorAll(".sheetgrid .gbody .row"), a = rows[r0].children[c0 + 1].getBoundingClientRect(), z = rows[r1].children[c1 + 1].getBoundingClientRect(); return { x: a.x, y: a.y, width: z.right - a.x, height: z.bottom - a.y }; }, [r0, c0, r1, c1]);
  const toolsBox = await p.evaluate(() => { const bs = [...document.querySelectorAll(".sheettools .btn")]; const a = bs[0].getBoundingClientRect(), z = bs[bs.length - 1].getBoundingClientRect(); return { x: a.x, y: a.y, width: z.right - a.x, height: a.height }; });
  await shot("sheets", [[toolsBox, 1], [await cells(2, 0, 6, 1), 2, 0, "right"], [await cells(8, 0, 12, 2), 3, 0, "right"], [await cells(3, 5, 5, 5), 4]], { x: 66, y: 160, width: 1374, height: 470 });
  const g8 = await cells(8, 4, 8, 4); await p.mouse.click(g8.x + 20, g8.y + 10); await p.keyboard.type("=COUNTIF(Tasks[La"); await wait(300);
  await shot("sheet-formula", [["input.edit", 1], [".fhint", 2, 0, "right"]], { x: 66, y: 160, width: 1374, height: 470 });
  await p.keyboard.press("Escape"); await p.keyboard.press("Escape"); await wait(200);
  await side("Dashboard"); await shot("dashboard", [[".kpis", 1]]);
  // Own dashboards: the Client dashboard (tabs, tiles, a rule-coloured number, flags), then the Add tile dialog.
  await p.click(".dashtabs button:text-is('Client dashboard')"); await wait(1500);
  await shot("dashboards", [[".dashtabs", 1, 0, "in"], [".dashbar .btn", 2, 0], [".dtile.k-number", 3, 2], [".dtile.k-ring", 4]]);
  await p.click(".dashbar button:has-text('Edit layout')"); await wait(200);
  await p.click(".dashbar button:has-text('+ Add tile')"); await wait(300);
  await shot("dashboard-tile", [[".dlg select", 1, 0], [".dlg input.mono", 2, 0], [".dlg select", 3, 2]], ".dlg");
  await p.keyboard.press("Escape"); await p.click(".dashbar button:has-text('Done')"); await p.click(".dashtabs button:text-is('Project')"); await wait(300);
  await side("S-Curves"); await shot("scurve", [[".page .kpis", 1]]);
  await side("Progress"); await shot("progress-report");
  await side("Sched. Risk");
  await p.click(".page .ptools .btn.pri"); await wait(900);
  await shot("risk", [[".page .kpis", 1], [".page .riskcharts .panel", 2, 0], [".page .riskcharts .panel", 3, 1]]);
  await side("Look-ahead");
  await shot("lookahead", [[".tbl.look", 1], [".page .panel", 2, 1]], { x: 66, y: 160, width: 1374, height: 560 });
  await side("Manpower"); await shot("manpower", [[".page .ptools select", 1]]);
  await side("Risks"); await shot("risks", [[".page thead th:nth-child(8)", 1]], { x: 66, y: 160, width: 1374, height: 380 });
  await side("Gantt");
  // 6. Help tab and Report tab.
  await tab("Help"); await shot("help-tab", [["grp:Learn", 1]], TOP);
  await p.click(".ribbon button:has-text('Guide')"); await wait(400);
  await shot("guide-dialog", [[".guidecards", 1]], ".dlg");
  await p.keyboard.press("Escape"); await wait(200);
  await tab("Report"); await shot("report-tab", [["grp:View Reports", 1], ["grp:Health", 2], ["grp:Export", 3]], TOP);
  await tab("Task");
  // 7. Print.
  await p.click(".tabs .file"); await wait(200);
  await p.click(".bs aside button:has-text('Print')"); await wait(1200);
  await shot("print", [[".pbtns", 1], [".pset", 2, 0, "in"], [".pwrap", 3]]);
  // 8. The tutorial example with its note (dump tutorial > tests_ui/tutorial_fixture.json).
  const tut = path.join(__dirname, "tutorial_fixture.json");
  if (fs.existsSync(tut)) {
    p = await page(JSON.parse(fs.readFileSync(tut)));
    await side("Gantt"); await shot("tutorial", [[".tour", 1]]);
  }
  await b.close();
  srv.close();
})();
