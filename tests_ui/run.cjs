// UI tests: the built window (ui/dist) in Chromium with the engine's real output for a template
// (cargo run -p datum-core --example dump > tests_ui/fixture.json) behind a mocked Tauri bridge.
// Run: npm run build:ui && node tests_ui/run.cjs
const { chromium } = require("playwright");
const fs = require("fs"), http = require("http"), path = require("path");
const root = path.join(__dirname, "../ui/dist"), fix = JSON.parse(fs.readFileSync(path.join(__dirname, "fixture.json")));
const types = { ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".woff2": "font/woff2", ".woff": "font/woff", ".html": "text/html" };
const srv = http.createServer((q, r) => {
  const f = path.join(root, q.url === "/" ? "index.html" : decodeURIComponent(q.url.split("?")[0]));
  fs.readFile(f, (e, b) => { if (e) { r.writeHead(404); return r.end(); } r.writeHead(200, { "content-type": types[path.extname(f)] ?? "application/octet-stream" }); r.end(b); });
});
let failed = 0;
const check = (name, ok, info = "") => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : "  " + info}`); if (!ok) failed++; };

(async () => {
  await new Promise(ok => srv.listen(0, ok));
  const url = `http://localhost:${srv.address().port}/`;
  const b = await chromium.launch(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {});
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  p.on("pageerror", e => { errors.push(e.message); if (process.env.DBG) console.log("PAGEERR", e.message, e.stack); });
  await p.addInitScript(f => {
    const opts = { dateFmt: "%d-%b-%y", autosave: 0, beta: false, currency: { code: "USD", symbol: "$", decimals: 2, after: false }, company: "", logo: "", workspace: "", setupPanel: false, theme: "light" };
    const m = { ...f, options: opts, startup_file: null, recovery: null, check_update: null, templates: [], recent: [], version: "test", resources: [], checks: [], sheet_view: f.sheet_view ?? null, task_info: null };
    window.__calls = [];
    window.__TAURI__ = { core: { invoke: async (c, a) => { window.__calls.push([c, a]); if (c === "dash_eval") return a.items.map(i => (f.dash_map ?? {})[i[1] + "\u0001" + i[0]] ?? [[["42", 42, ""]]]); if (c === "apply" && a.op && a.op.Ui) { f.state = { ...f.state, rev: f.state.rev + 1, project: { ...f.state.project, ui: { ...(f.state.project.ui ?? {}), [a.op.Ui.key]: a.op.Ui.value } } }; return f.state; } if (c === "tutorial_example") return { ...f.state, project: { ...f.state.project, ui: { ...(f.state.project.ui ?? {}), tour: { setup: "Setup holds the facts.", gantt: "The Gantt is where the plan is built.\n\n• Links\n• Milestones", tracking: "Tracking compares with the baseline." } } } }; if (c === "query" && window.__subsView) return f.query_subs; if (a && a.by && `${c}:${a.by}` in m) return m[`${c}:${a.by}`]; if (c in m) return m[c]; if (c === "apply") return f.state; return null; } }, event: { listen: async () => () => {} } };
  }, fix);
  await p.goto(url);
  await p.waitForTimeout(700);
  await p.click(".bs aside .back");
  await p.waitForTimeout(500);

  // 1. Headings line up with the cells under them (the 0.3.0 bug).
  const offsets = await p.evaluate(() => {
    const heads = [...document.querySelectorAll(".ghead .hc")].slice(1, -1), cells = [...document.querySelector(".gbody .row").children].slice(1);
    return heads.map((h, i) => Math.abs(h.getBoundingClientRect().left - cells[i].getBoundingClientRect().left));
  });
  check("column headings line up with cells", offsets.length > 3 && Math.max(...offsets) < 1, JSON.stringify(offsets));
  await p.evaluate(() => { document.querySelector(".gscroll").scrollLeft = 300; });
  await p.waitForTimeout(200);
  const after = await p.evaluate(() => {
    const heads = [...document.querySelectorAll(".ghead .hc")].slice(1, -1), cells = [...document.querySelector(".gbody .row").children].slice(1);
    return heads.map((h, i) => Math.abs(h.getBoundingClientRect().left - cells[i].getBoundingClientRect().left));
  });
  check("headings still line up after scrolling sideways", Math.max(...after) < 1, JSON.stringify(after));
  await p.evaluate(() => { document.querySelector(".gscroll").scrollLeft = 0; });

  // 2. Dragging selects exactly the cells dragged over.
  const cellBox = (row, col) => p.evaluate(([r, c]) => { const el = document.querySelectorAll(".gbody .row")[r].children[c + 1].getBoundingClientRect(); return { x: el.left + el.width / 2, y: el.top + el.height / 2 }; }, [row, col]);
  const a = await cellBox(2, 2), z = await cellBox(4, 4);
  await p.mouse.move(a.x, a.y); await p.mouse.down(); await p.mouse.move(z.x, z.y, { steps: 6 }); await p.mouse.up();
  const picked = await p.evaluate(() => [...document.querySelectorAll(".gbody .row")].slice(0, 6).map(r => [...r.children].slice(1).map(c => (c.classList.contains("in") ? 1 : 0)).join("")));
  check("drag selects a 3×3 block", picked[2] === picked[3] && picked[3] === picked[4] && picked[2].slice(2, 5) === "111" && !picked[1].includes("1") && !picked[5].includes("1") && (picked[2].match(/1/g) || []).length === 3, JSON.stringify(picked));
  check("status shows Sum/Count of the selection", (await p.textContent(".fbar")).includes("Count"));

  // 2b. Clicking cell after cell moves the selection each time (0.3.2 kept the first row), and double-click edits.
  const curAt = () => p.evaluate(() => { const rs = [...document.querySelectorAll(".gbody .row")]; for (let i = 0; i < rs.length; i++) { const j = [...rs[i].children].slice(1).findIndex(c => c.classList.contains("at")); if (j >= 0) return [i, j]; } return null; });
  const seq = [[1, 1], [4, 2], [6, 1], [3, 3]], got = [];
  for (const [r, c] of seq) { const q = await cellBox(r, c); await p.mouse.click(q.x, q.y); await p.waitForTimeout(80); got.push(await curAt()); }
  check("clicking cells selects each one", JSON.stringify(got) === JSON.stringify(seq), JSON.stringify(got));
  const e1 = await cellBox(2, 1); await p.mouse.dblclick(e1.x, e1.y); await p.waitForTimeout(150);
  await p.keyboard.press("Control+A"); await p.keyboard.type("Renamed task"); await p.keyboard.press("Enter"); await p.waitForTimeout(150);
  check("double-click edits a cell", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").pop())).includes("Renamed task"));

  // 3. Right-click menu commands run (they did nothing in 0.3.0) and show icons.
  const head = await p.$(".ghead .hc:nth-child(3)");
  await head.click({ button: "right" });
  check("heading menu has icons", (await p.$$(".menu button .tick svg")).length > 5);
  await p.getByText("New Formula Column…", { exact: true }).click();
  await p.waitForTimeout(300);
  check("menu command opens the Formula Column dialog", (await p.textContent(".dlg header").catch(() => "")).includes("Formula"));
  await p.keyboard.press("Escape");

  // 4. Ribbon: split buttons and the separate contextual Format tab.
  check("Gantt Chart is a split button", (await p.$$(".splitbig")).length >= 2);
  check("Format tab carries its Gantt Chart Tools label", (await p.textContent(".ctxgrp")).includes("Gantt Chart Tools"));
  await p.click(".ctxgrp button");
  check("Gantt Chart Style gallery", (await p.$$(".swatch")).length >= 8);

  // 5. The File sidebar is blue, not black.
  await p.click(".tabs .file");
  await p.waitForTimeout(200);
  const bg = await p.$eval(".bs aside", el => getComputedStyle(el).backgroundColor);
  check("File sidebar is blue", bg === "rgb(11, 92, 173)", bg);
  await p.click(".bs aside .back");

  // 5b. Options: each label sits left of its control (0.3.3 had them swapped).
  await p.click(".tabs .file"); await p.waitForTimeout(150);
  await p.click(".bs aside button:has-text('Options')"); await p.waitForTimeout(200);
  const bad = await p.$$eval(".opt", os => os.filter(o => o.children[0].getBoundingClientRect().right > o.children[1].getBoundingClientRect().left + 1).length);
  check("Options labels sit left of their controls", bad === 0 && (await p.$$(".opt")).length > 8, String(bad));
  await p.click(".bs aside .back"); await p.waitForTimeout(200);

  // 5c. Text formatting: Ctrl+B on a cell sends a Style edit; the Font group is on the Task tab.
  const fc = await cellBox(3, 1); await p.mouse.click(fc.x, fc.y); await p.keyboard.press("Control+b"); await p.waitForTimeout(150);
  check("Ctrl+B formats the selected cells", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").pop())).includes('"Style"'));
  await p.click(".tabs > button:text-is('Task')"); await p.waitForTimeout(100);
  check("Font group on the Task tab", (await p.$$(".fontg .fb")).length >= 8);

  // 5d. Print: Ctrl+P opens the print page with a preview; printing lays out the same pages with the paper size.
  await p.keyboard.press("Control+p"); await p.waitForTimeout(400);
  const nprev = (await p.$$(".pprev .ppage")).length;
  check("Print page shows a page preview", nprev >= 1 && (await p.textContent(".pprev .phead")).includes(fix.state.project.name), String(nprev));
  await p.evaluate(() => { window.print = () => { window.__printed = [document.querySelectorAll("#printroot .ppage").length, [...document.querySelectorAll("style")].map(s => s.textContent).find(t => t.includes("@page")) || ""]; }; });
  await p.click(".pbtns button:has-text('Print…')"); await p.waitForTimeout(500);
  const pr = await p.evaluate(() => window.__printed);
  check("Print renders every page with its paper size", pr && pr[0] === nprev && /size: 297mm 210mm/.test(pr[1]), JSON.stringify(pr));
  check("the page setup stays in place until the print window has finished", await p.evaluate(() => [...document.querySelectorAll("style")].some(s => s.textContent.includes("@page")) && document.documentElement.classList.contains("printing")));
  // The same engine as the Windows WebView prints the pages to a PDF: one sheet per page, landscape A4.
  const pdf = (await p.pdf({ preferCSSPageSize: true, printBackground: true })).toString("latin1");
  const sheets = (pdf.match(/\/Type\s*\/Page[^s]/g) || []).length, box = (pdf.match(/\/MediaBox\s*\[([^\]]+)\]/) || [])[1] || "";
  check("printed to PDF: one landscape A4 sheet per page, none split or blank", sheets === nprev && /84[12]\.\d+ 59[45]\.\d+/.test(box), `${sheets} sheets, box ${box}`);
  await p.evaluate(() => window.dispatchEvent(new Event("afterprint"))); await p.waitForTimeout(100);
  check("…and is removed afterwards", await p.evaluate(() => ![...document.querySelectorAll("style")].some(s => s.textContent.includes("@page")) && !document.documentElement.classList.contains("printing")));
  // Several pages (200 %): still one sheet per page.
  await p.click(".pset .seg button:has-text('Page')"); await p.click(".ptab input[type=radio]"); await p.fill(".ptab input[type=number]", "200"); await p.press(".ptab input[type=number]", "Tab"); await p.waitForTimeout(300);
  const many = (await p.$$(".pprev .ppage")).length;
  await p.click(".pbtns button:has-text('Print…')"); await p.waitForTimeout(500);
  const pdf2 = (await p.pdf({ preferCSSPageSize: true, printBackground: true })).toString("latin1");
  check("several pages print as that many sheets", many > 1 && (pdf2.match(/\/Type\s*\/Page[^s]/g) || []).length === many, `${many} pages`);
  await p.evaluate(() => window.dispatchEvent(new Event("afterprint"))); await p.waitForTimeout(100);
  await p.click(".pset button:has-text('Reset to defaults')"); await p.waitForTimeout(200);
  await p.click(".pbtns button:has-text('Save as PDF')"); await p.waitForTimeout(500);
  check("Save as PDF writes the pages at the paper size, no margins added", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "save_pdf").pop())).includes('"wMm":297,"hMm":210,"margins":[0,0,0,0]'));
  // The settings panel lays out in its width (no squashed boxes, no sideways scroll).
  await p.click(".pset .seg button:has-text('Header & footer')"); await p.waitForTimeout(150);
  check("the print settings panel never scrolls sideways", await p.evaluate(() => { const s = document.querySelector(".pset"); return s.scrollWidth <= s.clientWidth + 1 && document.querySelector(".ptab").getBoundingClientRect().width > 250; }),
    await p.evaluate(() => { const s = document.querySelector(".pset"), w = s.getBoundingClientRect().right; return `${s.scrollWidth}/${s.clientWidth} ` + [...s.querySelectorAll("*")].filter(e => e.getBoundingClientRect().right > w + 1).slice(0, 4).map(e => e.tagName + "." + e.className + ":" + Math.round(e.getBoundingClientRect().width)).join(" "); }));
  const hf = await p.textContent(".ptab");
  check("Header & footer: pictures, title and footer choices, nothing else printed", ["Pictures at the top", "Title", "Subtitle", "Printed date", "Page number", "Revision"].every(t => hf.includes(t)), hf.slice(0, 200));
  await p.fill(".ptab input[placeholder^='e.g. Revision']", "Rev B · Issued for review"); await p.waitForTimeout(150);
  await p.click(".ptab .chk:has-text('Status date') input"); await p.waitForTimeout(150);
  check("the preview shows the subtitle and the chosen footer", (await p.textContent(".pprev .phead")).includes("Rev B · Issued for review") && (await p.textContent(".pprev .pfoot")).includes("Status date") && !(await p.textContent(".pprev .ppage")).includes("Datum Plan"));
  // Several pictures in one place, side by side, each its own size; they come with every page.
  await p.evaluate(() => { const inv = window.__TAURI__.core.invoke; window.__TAURI__.core.invoke = async (c, a) => c === "pick_logo" ? "data:image/svg+xml;base64," + btoa('<svg xmlns="http://www.w3.org/2000/svg" width="80" height="30"><rect width="80" height="30" fill="#0B5CAD"/></svg>') : inv(c, a); });
  const plus = await p.$$(".pplace button:has-text('+ Picture')"); await plus[2].click(); await p.waitForTimeout(100); await plus[2].click(); await p.waitForTimeout(200);
  check("two pictures on the right, side by side in the header", (await p.$$(".pprev .ppage .phead .ps2 img")).length === 2);
  const sz = await p.$$(".pplace:nth-of-type(3) .pimg2 input[type=range]");
  await sz[0].evaluate(e => { e.value = "80"; e.dispatchEvent(new Event("input", { bubbles: true })); }); await p.waitForTimeout(200);
  check("each picture has its own size, and the header grows to fit it", await p.evaluate(() => { const im = document.querySelectorAll(".pprev .ppage .phead .ps2 img"); return im[0].getBoundingClientRect().height > im[1].getBoundingClientRect().height * 1.8; }));
  const lg = await p.textContent(".pprev .plegend");
  check("the legend explains every mark on the page", ["Task", "Critical task", "Milestone", "Summary", "Link"].every(t => lg.includes(t)) && lg.includes("Start "), lg.slice(0, 160));
  await p.click(".ptab button:has-text('Advanced')"); await p.waitForTimeout(150);
  check("the code boxes are still there for exact control, and wide enough to use", await p.evaluate(() => [...document.querySelectorAll(".pslots textarea")].every(t => t.getBoundingClientRect().width > 150)));
  await p.click(".ptab button:has-text('Back to the simple')"); await p.waitForTimeout(100);
  await p.click(".bs aside .back"); await p.waitForTimeout(200);

  // 0.3.5: nested Insert Column menu, hide/unhide/delete, every column printed, ribbon fits, Setup in the sidebar.
  await p.keyboard.press("Escape");
  const hd = await p.$(".ghead .hc:nth-child(4)"); await hd.click({ button: "right" }); await p.waitForTimeout(150);
  await p.hover(".menu button:has-text('Insert Column')"); await p.waitForTimeout(120);
  await p.hover(".menu button:has-text('Schedule')"); await p.waitForTimeout(150);
  check("Insert Column categories open their fields", (await p.$$(".menu")).length === 3 && (await p.$$eval(".menu", ms => ms[2].querySelectorAll("button").length)) > 10);
  await p.mouse.click(5, 5);
  const hd2 = await p.$(".ghead .hc:nth-child(4)"); const hname = (await hd2.textContent()).trim(); await hd2.click({ button: "right" }); await p.click(".menu button:has-text('Hide Column')"); await p.waitForTimeout(150);
  const hc = JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").pop()));
  check("Hide Column keeps the column, hidden", /"Columns".*-\d+/.test(hc), hc.slice(0, 200));
  const hd3 = await p.$(".ghead .hc:nth-child(4)"); await hd3.click({ button: "right" });
  check("Unhide Columns lists the hidden column", !(await p.$eval(".menu button:has-text('Unhide Columns')", b => b.disabled)) && (await p.$$(".menu button:has-text('Delete Column')")).length === 1, hname);
  await p.mouse.click(5, 5);
  await p.evaluate(() => document.querySelector(".gscroll").focus()); await p.keyboard.press("Control+p"); await p.waitForTimeout(400);
  const heads = await p.$$eval(".pprev svg text", t => t.map(x => x.textContent));
  check("Print includes every column of the table", ["Task Name", "Duration", "Start", "Finish"].every(h => heads.includes(h)), heads.slice(0, 12).join("|"));
  await p.click(".bs aside .back"); await p.waitForTimeout(150);
  for (const w of [1280, 1600]) { await p.setViewportSize({ width: w, height: 900 }); for (const t of ["Task", "Schedule", "Progress", "Report", "View", "Format"]) { await p.click(`.tabs button:text-is('${t}')`); await p.waitForTimeout(120); const o = await p.$eval(".ribbon", r => r.scrollWidth - r.clientWidth); if (o > 1) check(`ribbon fits at ${w}px (${t})`, false, String(o)); } }
  check("ribbon never scrolls sideways (1280/1600 px)", true);
  await p.setViewportSize({ width: 1440, height: 900 });
  await p.click('.side button[title="Setup"]'); await p.waitForTimeout(200);
  check("Setup opens as a page from the sidebar", !!(await p.$(".setuppage .setup")));
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(150);

  // 0.3.6: every view prints: exact pages for Gantt, Network, Calendar and tables; reports as a flowing document.
  const noPages = [];
  for (const v of ["Gantt", "Network", "Calendar", "Resources", "Task Usage", "Res. Usage", "Team", "Dashboard", "Progress", "S-Curves", "Builder"]) {
    if (!(await p.$(`.side button[title="${v}"]`))) continue;
    await p.click(`.side button[title="${v}"]`); await p.waitForTimeout(300);
    await p.click(".tabs .file"); await p.waitForTimeout(100); await p.click(".bs aside button:has-text('Print')"); await p.waitForTimeout(700);
    const n = await p.evaluate(() => document.querySelectorAll(".pprev .ppage, .pprev .pflow").length);
    if (!n) noPages.push(v);
    await p.click(".bs aside .back"); await p.waitForTimeout(150);
  }
  check("every view gives print pages", noPages.length === 0, noPages.join(", "));
  await p.click('.side button[title="Network"]'); await p.waitForTimeout(400);
  check("Network Diagram has a legend and link labels explained", (await p.textContent(".netleg")).includes("Start-to-Start") && (await p.$$(".net .nbox")).length > 3);
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(400);
  check("timeline shows the zoom box", await p.evaluate(() => !!document.querySelector(".tlwrap canvas")));

  // 0.3.6: Help tab links and glossary; font list.
  await p.click(".tabs > button:text-is('Help')"); await p.waitForTimeout(150);
  check("Help tab has GitHub, Releases and Report an Issue", (await p.$$eval(".ribbon .rb span", s => s.map(x => x.textContent))).filter(t => ["GitHub", "Releases", "Report an Issue", "Glossary", "Getting Started"].includes(t)).length === 5);
  await p.click(".ribbon button[title*='FS, SS']"); await p.waitForTimeout(200);
  await p.fill(".dlg .find", "start-to-start"); await p.waitForTimeout(100);
  check("Glossary explains link types", (await p.textContent(".dlg .gloss")).includes("Start-to-Start"));
  await p.keyboard.press("Escape");
  await p.click(".tabs > button:text-is('Task')"); await p.waitForTimeout(100);
  check("Font list on the Task tab", (await p.$$eval(".ffam option", o => o.map(x => x.textContent))).includes("Times New Roman"));

  // 0.3.7: Tracking Gantt is its own view (banner + Tracking table), network WBS bands.
  await p.click('.side button[title="Tracking"]'); await p.waitForTimeout(400);
  const trkCols = await p.evaluate(() => window.__calls.filter(c => c[0] === "query").map(c => c[1]?.q?.cols ?? []).filter(c => c.length > 2).pop() ?? []);
  check("Tracking Gantt explains itself and uses the Tracking table", !!(await p.$(".trkbar")) && trkCols.includes("astart") && trkCols.includes("pct"), JSON.stringify(trkCols));
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(300);
  const ganttCols = await p.evaluate(() => window.__calls.filter(c => c[0] === "query").map(c => c[1]?.q?.cols ?? []).filter(c => c.length > 2).pop() ?? []);
  check("back on the Gantt chart the Entry table returns", !ganttCols.includes("astart") && !(await p.$(".trkbar")), JSON.stringify(ganttCols));
  await p.click('.side button[title="Network"]'); await p.waitForTimeout(300);
  await p.click("label.chk:has-text('By WBS') input"); await p.waitForTimeout(300);
  check("Network by WBS asks the engine for bands", await p.evaluate(() => window.__calls.some(c => c[0] === "network" && c[1]?.wbs === true)));
  check("Network has the whiteboard canvas", (await p.$eval(".netwrap", e => getComputedStyle(e).backgroundImage)).includes("radial-gradient"));
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(200);

  // 0.3.8: printed Gantt draws the links; date fields have a date picker.
  await p.keyboard.press("Control+p"); await p.waitForTimeout(500);
  const linkPaths = await p.$$eval(".pprev svg path[fill='none']", ps => ps.filter(x => /^M[\d.]+ [\d.]+H/.test(x.getAttribute("d") ?? "")).length);
  check("printed Gantt shows the links between tasks", linkPaths > 5, String(linkPaths));
  await p.click(".bs aside .back"); await p.waitForTimeout(200);
  const dc = await cellBox(2, 3); await p.mouse.click(dc.x, dc.y); await p.waitForTimeout(150);
  check("a date cell offers a date picker", !!(await p.$(".gdate .dbtn input[type=date]")));
  // 0.3.11: picking a date closes the picker (the input loses focus) and sets the cell.
  const closed = await p.$eval(".gdate .dbtn input[type=date]", i => { i.focus(); i.value = "2026-12-01"; i.dispatchEvent(new Event("change", { bubbles: true })); return document.activeElement !== i; });
  const lastCalls = JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").slice(-1)));
  check("picking a date closes the picker and sets the cell", closed && lastCalls.includes("2026-12-01"), `${closed} ${lastCalls.slice(0, 300)}`);

  // 0.3.9: a "?" beside a tool opens its card; Help › What Is This Tool? lists every card.
  await p.click(".tabs > button:text-is('Schedule')"); await p.waitForTimeout(100);
  await p.click(".ribbon .grp .cap:text-is('Leveling') .help"); await p.waitForTimeout(150);
  check("? beside Leveling opens its card", (await p.textContent(".dlg header")).includes("Leveling") && (await p.textContent(".dlg .helpcard")).includes("How to use it"));
  await p.keyboard.press("Escape");
  check("every ribbon group on the Task tab has a ?", await (async () => { await p.click(".tabs > button:text-is('Task')"); await p.waitForTimeout(100); return p.$$eval(".ribbon > .grp", gs => gs.every(g => g.querySelector(".cap .help"))); })());
  await p.click(".tabs > button:text-is('Help')"); await p.waitForTimeout(100);
  await p.click(".ribbon button[title*='Every tool explained']"); await p.waitForTimeout(150);
  await p.getByText("Baselines", { exact: true }).click(); await p.waitForTimeout(150);
  check("What Is This Tool? opens a card from the list", (await p.textContent(".dlg .helpcard")).includes("saved copy of the plan"));
  await p.keyboard.press("Escape");
  await p.click('.side button[title="Network"]'); await p.waitForTimeout(300);
  await p.click(".page h1 .help"); await p.waitForTimeout(150);
  check("? beside a page title opens its card", (await p.textContent(".dlg header")).includes("Network Diagram"));
  await p.keyboard.press("Escape");
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(200);
  // 0.3.10: Roadmap on the File page; "fit to 1 page wide" gives exactly one page across.
  await p.click(".tabs .file"); await p.waitForTimeout(150);
  await p.click(".tile:has-text('Roadmap')"); await p.waitForTimeout(200);
  check("Roadmap page lists what comes next", (await p.$$(".rmstage")).length >= 4 && (await p.textContent(".roadmap")).includes("What-if"));
  await p.click(".bs aside button:has-text('Print')"); await p.waitForTimeout(600);
  const across = await p.$$eval(".pprev .ppage", ps => ps.length);
  check("fit to 1 page wide gives one page for this plan", across === 1, String(across));
  await p.click(".bs aside .back"); await p.waitForTimeout(150);

  // 0.4.0: Schedule Risk, the Delay column, the levelling preview, trades and the import report.
  await p.click('.side button[title="Sched. Risk"]'); await p.waitForTimeout(200);
  await p.click(".page .ptools .btn.pri"); await p.waitForTimeout(500);
  const rk = await p.textContent(".page");
  check("Schedule Risk runs and shows P50, P80 and criticality", rk.includes("P80") && rk.includes("P50") && (await p.$$(".page tr.click")).length >= 3 && JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "risk_run").pop())).includes('"iterations":1000'));
  await p.click('.side button[title="Risks"]'); await p.waitForTimeout(200);
  check("the risk register has a Delay (days) column", (await p.textContent(".page thead")).includes("Delay (days)"));
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(200);
  await p.click(".tabs > button:text-is('Schedule')"); await p.waitForTimeout(100);
  await p.click(".ribbon button[title='Level All'], .ribbon button:has-text('Level All')"); await p.waitForTimeout(250);
  check("Level All with nobody overallocated says so and changes nothing", (await p.textContent("#toast")).includes("Nobody is overallocated") && !(await p.$(".dlg")));
  await p.evaluate(() => { const inv = window.__TAURI__.core.invoke; window.__TAURI__.core.invoke = async (c, a) => c === "level_preview" ? { moved: [{ uid: 3, name: "Wire panel", start: 29990880, new_start: 29993760, finish: 29991900, new_finish: 29994780 }], finish: 30010000, new_finish: 30012000, over_before: 4, over_after: 0 } : c === "import" ? { plan: await inv("state"), notes: ["1 of 9 tasks get different dates in Datum Plan than in the file.", "  \"F Test\": file 2026-11-18 08:00 → 2026-11-18 17:00, Datum Plan 2026-11-17 08:00 → 2026-11-17 17:00"], table: null, fields: [] } : inv(c, a); });
  await p.click(".ribbon button[title='Level All'], .ribbon button:has-text('Level All')"); await p.waitForTimeout(250);
  check("Level All shows a preview first", (await p.textContent(".dlg header")).includes("preview") && (await p.$$(".dlg tbody tr")).length === 1 && !JSON.stringify(await p.evaluate(() => window.__calls.slice(-2))).includes("Level"));
  await p.click(".dlg footer .btn.pri"); await p.waitForTimeout(200);
  check("Level in the preview levels the plan", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").pop())).includes('"Level"'));
  await p.click(".tabs > button:text-is('Resource')"); await p.waitForTimeout(100);
  await p.click(".ribbon button:has-text('Resource Graph')"); await p.waitForTimeout(250);
  await p.click(".page .ptools button:text-is('Trades')"); await p.waitForTimeout(250);
  check("Resource Graph shows trades against their capacity", (await p.$$eval(".page .ptools select option", o => o.map(x => x.textContent))).includes("Engineering") && (await p.$$(".rgraph rect")).length > 3);
  await p.click(".tabs .file"); await p.waitForTimeout(150);
  await p.click(".bs aside button:has-text('Import')"); await p.waitForTimeout(300);
  check("an imported plan shows its import report", (await p.textContent(".dlg header")).includes("Import report") && (await p.$$(".dlg .notes li")).length === 2);
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(200);

  // 0.4.1: look-ahead and constraint log, manpower by trade, rules of credit, earned schedule.
  await p.click('.side button[title="Look-ahead"]'); await p.waitForTimeout(300);
  check("Look-ahead lists the coming work with a ready flag", (await p.$$(".tbl.look tbody tr")).length >= 2 && (await p.textContent(".tbl.look")).includes("Ready"));
  await p.click(".tbl.look tbody tr:nth-child(2)"); await p.waitForTimeout(100);
  await p.click(".page .panel .btn.pri:has-text('Add constraint')"); await p.waitForTimeout(200);
  const logCall = await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").pop());
  const chosen = await p.$eval(".tbl.look tbody tr:nth-child(2) td:nth-child(2)", e => e.textContent);
  check("+ Add constraint logs one for the chosen task", JSON.stringify(logCall).includes('"Log"') && (await p.textContent(".page")).includes(`to ${chosen}`));
  await p.click('.side button[title="Manpower"]'); await p.waitForTimeout(400);
  check("Manpower shows people per trade: planned, done, forecast", (await p.$$eval(".page .ptools select option", o => o.map(x => x.textContent))).includes("Engineering") && !!(await p.$(".page canvas")));
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(200);
  await p.click(".tabs > button:text-is('Progress')"); await p.waitForTimeout(100);
  await p.click(".ribbon button:has-text('Rules of Credit')"); await p.waitForTimeout(300);
  check("Rules of Credit lists the built-in rules", (await p.textContent(".dlg .credit")).includes("Cable") && (await p.textContent(".dlg .credit")).includes("built in"));
  await p.click(".dlg .credit .list button:has-text('Cable')"); await p.waitForTimeout(100);
  await p.click(".dlg footer .btn.pri"); await p.waitForTimeout(200);
  check("applying a rule of credit sends it to the engine", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").pop())).includes('"rule":"Cable"'));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await p.evaluate(() => { const inv = window.__TAURI__.core.invoke; window.__TAURI__.core.invoke = async (c, a) => c === "earned_schedule" ? { es: 8, at: 10, pd: 40, spi_t: 0.8, sv_t: -2, ieac: 50, planned_finish: 30010000, forecast_finish: 30024400 } : inv(c, a); });
  await p.click('.side button[title="S-Curves"]'); await p.waitForTimeout(400);
  check("S-Curves show earned schedule: SPI(t) and the finish at this pace", (await p.textContent(".page .kpis")).includes("SPI(t)") && (await p.textContent(".page .kpis")).includes("0.80"));
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(200);

  // 0.4.2: Schedule Risk, Look-ahead and Manpower print as themselves (not the Dashboard); the risk run is kept with the plan.
  await p.click('.side button[title="Sched. Risk"]'); await p.waitForTimeout(200);
  await p.click(".page .ptools .btn.pri"); await p.waitForTimeout(500);
  check("a risk run is kept with the plan", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").pop())).includes('"key":"risk"'));
  for (const [side, name, text] of [["Sched. Risk", "Schedule Risk", "P80"], ["Look-ahead", "Look-ahead", "Constraint log"], ["Manpower", "Manpower", "people per"]]) {
    await p.click(`.side button[title="${side}"]`); await p.waitForTimeout(300);
    await p.click(".tabs .file"); await p.waitForTimeout(150);
    await p.click(".bs aside button:has-text('Print')"); await p.waitForTimeout(900);
    const what = await p.textContent(".pwhat"), prev = await p.textContent(".pflowprev").catch(() => "");
    check(`Print shows ${name}, not the Dashboard`, what.includes(name) && prev.includes(text) && !prev.includes("Project Dashboard"), `${what} | ${prev.slice(0, 80)}`);
    await p.click(".bs aside .back"); await p.waitForTimeout(150);
  }
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(200);

  // 0.4.4: Help › Guide offers the guide in English and Chinese and asks the shell to download it.
  await p.click(".tabs > button:text-is('Help')"); await p.waitForTimeout(100);
  await p.click(".ribbon button:has-text('Guide')"); await p.waitForTimeout(250);
  const cards = await p.$$eval(".dlg .guidecard b", b => b.map(x => x.textContent));
  await p.click(".dlg .guidecard:has-text('中文')"); await p.waitForTimeout(200);
  check("Help › Guide offers English and 中文 and downloads the chosen one", cards.join() === "English,中文" && JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "guide").pop())).includes('"lang":"zh"'));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await p.click(".tabs .file"); await p.waitForTimeout(150);
  check("the File page has a Guide tile", !!(await p.$(".tile:has-text('Guide')")));
  await p.click(".bs aside .back"); await p.waitForTimeout(150);
  await p.click(".tabs > button:text-is('Task')"); await p.waitForTimeout(100);

  // 0.5.0: Sheets work like Excel: flags, suggestions, clicking cells into a formula, Plan Data, inserting rows.
  const lastApply = () => p.evaluate(() => JSON.stringify(window.__calls.filter(c => c[0] === "apply").pop()));
  await p.click('.side button[title="Sheets"]'); await p.waitForTimeout(400);
  const sc = (r, c) => p.evaluate(([r, c]) => { const el = document.querySelectorAll(".sheetgrid .gbody .row")[r].children[c + 1].getBoundingClientRect(); return { x: el.left + el.width / 2, y: el.top + el.height / 2 }; }, [r, c]);
  check("Sheets show a vendor's country with its flag", (await p.$$(".sheetgrid img.flag")).length === 2 && (await p.textContent(".sheetgrid")).includes("Germany"));
  let q = await sc(5, 3); await p.mouse.click(q.x, q.y); await p.waitForTimeout(100);
  await p.keyboard.type("=SU"); await p.waitForTimeout(250);
  const offered = await p.$$eval(".fhint button b", b => b.map(x => x.textContent));
  check("typing =SU suggests SUM and friends", offered.includes("SUM") && offered.includes("SUMIFS"), offered.join());
  await p.keyboard.press("Tab"); await p.waitForTimeout(150);
  check("Tab inserts the function with its bracket", (await p.inputValue("input.edit")) === "=SUM(");
  check("the hint shows the function's arguments", (await p.textContent(".fhint .fsig").catch(() => "")).includes("number1"));
  const a2 = await sc(1, 2), a3 = await sc(2, 2);
  await p.mouse.move(a2.x, a2.y); await p.mouse.down(); await p.mouse.move(a3.x, a3.y, { steps: 4 }); await p.mouse.up(); await p.waitForTimeout(150);
  check("clicking and dragging cells adds their range to the formula", (await p.inputValue("input.edit")) === "=SUM(C2:C3" && (await p.$$(".refbox")).length === 1, await p.inputValue("input.edit"));
  await p.keyboard.press("Enter"); await p.waitForTimeout(200);
  check("Enter closes the bracket and saves the formula", (await lastApply()).includes('["D6","=SUM(C2:C3)"]'), await lastApply());
  q = await sc(7, 0); await p.mouse.click(q.x, q.y); await p.keyboard.type("=Tasks["); await p.waitForTimeout(200);
  check("after Tasks[ the plan's columns are offered", (await p.$$eval(".fhint button b", b => b.map(x => x.textContent))).includes("Finish"));
  await p.keyboard.press("Escape"); await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await p.click(".sheettools button:has-text('Plan Data')"); await p.waitForTimeout(250);
  await p.click(".dlg .btn.pri"); await p.waitForTimeout(250);
  check("Plan Data puts a live FILTER list of tasks with headings", /FILTER\(CHOOSECOLS\(Tasks/.test(await lastApply()) && (await lastApply()).includes('"Name"'));
  q = await sc(2, 0); await p.mouse.click(q.x, q.y, { button: "right" }); await p.waitForTimeout(150);
  await p.click(".menu button:has-text('Insert Row Above')"); await p.waitForTimeout(150);
  check("Insert Row Above asks the engine to move the cells", (await lastApply()).includes('"SheetInsert"') && (await lastApply()).includes('"at":2'));
  await p.click('.side button[title="Resources"]'); await p.waitForTimeout(300);
  check("the Resource Sheet has a Country column", (await p.textContent(".tbl.sheet thead")).includes("Country"));
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(200);

  // 0.5.3: custom dashboards: from a template, then edit tiles.
  await p.click('.side button[title="Dashboard"]'); await p.waitForTimeout(300);
  await p.click(".dashtabs .add"); await p.waitForTimeout(150);
  await p.click(".menu button:has-text('From template: Client dashboard')"); await p.waitForTimeout(700);
  const tiles = async () => (await p.$$(".dboard .dtile")).length;
  check("a dashboard from a template has its tiles", (await tiles()) === 8 && (await p.textContent(".dashtabs .on")) === "Client dashboard", String(await tiles()));
  check("number tiles show what the engine works out", (await p.$$eval(".dnum .v", v => v.map(x => x.textContent))).includes("42"));
  check("dashboards ask the engine for every formula at once", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "dash_eval").pop())).includes("COUNTIF(Tasks[Late],TRUE)"));
  check("a progress ring and a chart are drawn", !!(await p.$(".dring svg .val")) && !!(await p.$(".dtile.k-chart canvas")));
  await p.click(".dashbar button:has-text('Edit layout')"); await p.waitForTimeout(150);
  await p.click(".dashbar button:has-text('+ Add tile')"); await p.waitForTimeout(200);
  await p.selectOption(".dlg select >> nth=0", "text"); await p.fill(".dlg textarea", "Steel deliveries start in week 12."); await p.click(".dlg .btn.pri"); await p.waitForTimeout(400);
  check("+ Add tile adds a text tile", (await tiles()) === 9 && (await p.textContent(".dboard")).includes("Steel deliveries"));
  await p.click(".dtile:last-child .dtools button[title='Change']"); await p.waitForTimeout(200);
  await p.selectOption(".dlg select >> nth=0", "number"); await p.waitForTimeout(100);
  const fx = p.locator(".dlg input.mono").first(); await fx.click(); await fx.press("Control+A"); await p.keyboard.type("=TotalCost", { delay: 20 });
  check("a formula can be typed into a tile without losing keys", (await fx.inputValue()) === "=TotalCost", await fx.inputValue());
  await p.click(".dlg .btn.pri"); await p.waitForTimeout(300);
  await p.click(".dtile:last-child .dtools button[title='Size']"); await p.waitForTimeout(100);
  await p.hover(".menu button:has-text('Width')"); await p.waitForTimeout(150); await p.click(".menu button:has-text('Full width')"); await p.waitForTimeout(300);
  check("a tile can be resized", await p.$eval(".dtile:last-child", t => t.classList.contains("dw12")));
  await p.click(".dtile:last-child .dtools button[title='Remove']"); await p.waitForTimeout(300);
  check("a tile can be removed", (await tiles()) === 8);
  await p.click(".dashtabs button:text-is('Project')"); await p.waitForTimeout(300);
  check("the Project tab still shows the built-in dashboard", !!(await p.$(".kpis")) && !(await p.$(".dboard")));
  // 0.5.12: the S-curve takes the whole width; the other charts fill their rows; Cards ▾ chooses the cards.
  const wide = await p.evaluate(() => { const c = document.querySelector(".charts.wide .chartbox"), pg = document.querySelector(".page.report"); return c && pg ? c.getBoundingClientRect().width / pg.clientWidth : 0; });
  check("the project S-curve spans the dashboard's width", wide > 0.9, String(wide));
  const gaps = await p.$$eval(".charts.flow > *", els => { const rows = new Map(); for (const e of els) { const r = e.getBoundingClientRect(); rows.set(Math.round(r.top), (rows.get(Math.round(r.top)) ?? 0) + r.width); } const w = document.querySelector(".charts.flow").clientWidth; return [...rows.values()].every(x => x > w * 0.95); });
  check("the other charts fill their rows without empty space", gaps);
  await p.click(".ptools button:has-text('Cards')"); await p.waitForTimeout(150);
  await p.click(".menu button:has-text('Project S-curve')"); await p.waitForTimeout(400);
  check("Cards ▾ hides a card and keeps the choice with the plan", !(await p.$(".charts.wide")) && JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").pop())).includes("dashcards"));
  await p.click(".ptools button:has-text('Cards')"); await p.waitForTimeout(150);
  await p.click(".menu button:has-text('Show the cards that have data')"); await p.waitForTimeout(400);
  check("the cards that have data come back", !!(await p.$(".charts.wide")));
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(200);

  // 0.5.5: plans in tabs, links to tasks in other plans, the Linked Plans panel.
  check("each open plan has a tab, unsaved ones marked", (await p.$$(".plantabs .plantab")).length === 2 && !!(await p.$(".plantabs .plantab:nth-child(2) .dot")));
  check("a task linked to another plan is marked, and says what it waits for", (await p.getAttribute(".row .ind.xin", "title") ?? "").includes("Engineering › E1010"));
  check("a task another plan waits for is marked too", (await p.getAttribute(".row .ind.xout", "title") ?? "").includes("Main schedule › A1040"));
  check("a link whose plan has moved is marked on the row and on that plan's tab", !!(await p.$(".row .ind.xin.moved")) && (await p.textContent(".plantabs .plantab:nth-child(2) .mv")).includes("2"));
  await p.click(".row .ind.xin"); await p.waitForTimeout(300);
  check("clicking the mark offers to go to the linked tasks, both ways", (await p.textContent(".menu")).includes("Go to (waits for): Engineering › E1010") && (await p.textContent(".menu")).includes("Go to (waits for this): Main schedule › A1040") && (await p.textContent(".menu")).includes("Update Links"));
  await p.click(".menu button:has-text('Go to (waits for): Engineering')"); await p.waitForTimeout(200);
  check("Go to shows the other plan", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "goto_link").pop())).includes("Engineering.dplan"));
  await p.click(".plantabs .plantab:nth-child(2)", { button: "right" }); await p.waitForTimeout(200);
  const tm = await p.textContent(".menu");
  check("right-clicking a tab offers Close, Close All Plans and Update Links", ["Close", "Close All Plans", "Update Links"].every(t => tm.includes(t)), tm);
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  check("a project's tabs are grouped: the master marked ◆, its subplan ›, in one colour", !!(await p.$(".plantabs .plantab.master .pm")) && !!(await p.$(".plantabs .plantab:nth-child(2) .ps")) && await p.evaluate(() => { const t = document.querySelectorAll(".plantabs .plantab"); return getComputedStyle(t[0]).borderTopColor === getComputedStyle(t[1]).borderTopColor && getComputedStyle(t[0]).borderTopWidth === "3px"; }));
  await p.click(".plantabs .plantab:nth-child(1)", { button: "right" }); await p.waitForTimeout(200);
  const pm = await p.textContent(".menu");
  check("a master's tab menu offers Open All Subplans, New Subplan and Close Project", ["Open All Subplans", "New Subplan", "Close Project"].every(t => pm.includes(t)), pm);
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await p.click(".tabs > button:text-is('Project')"); await p.waitForTimeout(150);
  await p.click(".ribbon button:has-text('Subplans')"); await p.waitForTimeout(300);
  const sp = await p.textContent(".dlg");
  check("Subplans lists the project's plans with their dates, progress and state", sp.includes("Engineering") && sp.includes("Procurement") && sp.includes("25%") && sp.includes("Not open"), sp.slice(0, 300));
  await p.click(".dlg .subs tbody tr:nth-child(2)"); await p.click(".dlg button[title='Move up']"); await p.waitForTimeout(200);
  const mv = JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").pop()));
  check("moving a subplan up saves the new order", mv.includes('"Subplans"') && mv.indexOf("Procurement") < mv.indexOf("Engineering"), mv);
  await p.click(".dlg button:has-text('New Subplan')"); await p.waitForTimeout(250);
  await p.fill(".dlg input", "Civil"); await p.selectOption(".dlg select", { index: 1 }); await p.click(".dlg .btn.pri:text-is('Create')"); await p.waitForTimeout(200);
  check("New Subplan asks for a name and its place (under Engineering)", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "new_subplan").pop())).includes('"name":"Civil","level":1,"at":2'));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await p.click(".tabs > button:text-is('Task')"); await p.waitForTimeout(100);
  // The master's combined view: its subplans under its tasks, read-only.
  await p.evaluate(() => { window.__subsView = true; });
  await p.click(".tabs .ctxgrp button:text-is('Format')"); await p.waitForTimeout(100);
  await p.click("label.chk:has-text('Subplans') input"); await p.waitForTimeout(150); await p.click("label.chk:has-text('Subplans') input"); await p.waitForTimeout(300);
  await p.evaluate(() => document.dispatchEvent(new CustomEvent("grid-scroll", { detail: 100000 }))); await p.waitForTimeout(200);
  check("the master shows its subplans under its tasks, as read-only rows", !!(await p.$(".row.subp .subico")) && (await p.textContent(".row.subp")).includes("Engineering") && (await p.textContent(".row.subc")).includes("IFC drawings issued"));
  await p.dblclick(".row.subc .c.nm"); await p.waitForTimeout(250);
  check("double-clicking a subplan's task goes to it in its plan", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "goto_link").pop())).includes("Engineering.dplan") && !(await p.$(".dlg")) && !(await p.$("input.ed")));
  check("the combined view asks the shell for the expanded subplans", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "query").pop())).includes('"subs":{"show":true'));
  await p.evaluate(() => { window.__subsView = false; });
  await p.click(".tabs > button:text-is('Task')"); await p.waitForTimeout(100);
  await p.click(".plantabs .plantab:nth-child(2)"); await p.waitForTimeout(200);
  check("clicking a tab shows that plan", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "switch_tab").pop())).includes('"k":1'));
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(200);
  q = await cellBox(2, 2); await p.mouse.click(q.x, q.y); await p.waitForTimeout(100);
  await p.click(".tabs > button:text-is('Project')"); await p.waitForTimeout(150);
  await p.click(".ribbon button:has-text('Link to Plan')"); await p.waitForTimeout(300);
  await p.click(".linkpick button:has-text('IFC drawings issued')"); await p.fill(".dlg input[type=number]", "5"); await p.click(".dlg button:has-text('Add Link')"); await p.waitForTimeout(100);
  check("the link dialog lists the new link with its type and lag", (await p.textContent(".dlg .tbl")).includes("Engineering › E1010 IFC drawings issued") && (await p.textContent(".dlg .tbl")).includes("FS+5d"));
  await p.click(".dlg .btn.pri:text-is('OK')"); await p.waitForTimeout(200);
  const ext = await lastApply();
  check("OK saves the link with the other plan's file and task", ext.includes('"ExtSet"') && ext.includes("Engineering.dplan") && ext.includes('"uid":2') && ext.includes('"lag":2400'), ext);
  await p.click(".ribbon button:has-text('Linked Plans')"); await p.waitForTimeout(300);
  check("Linked Plans shows a link whose plan has moved", (await p.textContent(".dlg .linkstates")).includes("Moved"));
  check("Linked Plans also lists what other open plans wait for", (await p.textContent(".dlg")).includes("Other open plans wait for") && (await p.textContent(".dlg")).includes("Main schedule › A1040"));
  await p.click(".dlg button:has-text('Update All Open Plans')"); await p.waitForTimeout(200);
  check("Update All Open Plans asks the engine to update every open plan", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "update_links").pop())).includes('"all":true'));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await p.click(".tabs > button:text-is('Task')"); await p.waitForTimeout(100);

  // 0.5.13: the tutorial: a note per view, Back and Next through the views, × hides the notes.
  await p.click(".tabs > button:text-is('Help')"); await p.waitForTimeout(150);
  await p.click(".ribbon button:has-text('Tutorial'):not(:has-text('Notes'))"); await p.waitForTimeout(500);
  check("Help › Tutorial opens the guided example on Setup with its note", (await p.textContent(".tour")).includes("Setup holds the facts") && (await p.textContent(".tour")).includes("1 of 3"));
  await p.click(".tour button:has-text('Next')"); await p.waitForTimeout(300);
  check("Next goes to the next view with a note, and its bullets show as a list", (await p.$eval(".side button.on", b => b.title)) === "Gantt" && (await p.$$(".tour li")).length === 2);
  await p.click(".tour .x"); await p.waitForTimeout(150);
  check("× hides the tutorial notes", !(await p.$(".tour")));
  await p.click(".ribbon button:has-text('Tutorial Notes')"); await p.waitForTimeout(150);
  check("Help › Tutorial Notes shows them again", !!(await p.$(".tour")));
  await p.click(".tour .x"); await p.waitForTimeout(100);
  await p.click(".tabs > button:text-is('Task')"); await p.waitForTimeout(100);

  // 0.5.14: the plan tabs sit between the ribbon and the timeline, and show even for one plan.
  const tabsAt = await p.evaluate(() => { const r = s => document.querySelector(s)?.getBoundingClientRect(); const t = r(".plantabs"), rb = r(".ribbon"), c = r(".content"); return t && rb && c && t.height > 20 && t.top >= rb.bottom - 1 && t.bottom <= c.top + 1; });
  check("the plan tabs sit between the ribbon and the timeline", tabsAt);
  const capsOk = await p.$$eval(".ribbon .grp .cap", cs => cs.every(c => { const r = c.getBoundingClientRect(), rb = c.closest(".ribbon").getBoundingClientRect(); return r.bottom <= rb.bottom + 0.5; }));
  check("ribbon group names are not cut off", capsOk);

  // 0.5.1: Tell Me dims the window instead of hiding it.
  await p.keyboard.press("Control+k"); await p.waitForTimeout(250);
  const ovBg = await p.$eval(".ov", o => getComputedStyle(o).backgroundColor);
  check("Tell Me shows the window behind it, dimmed", /rgba\(.*0\.\d+\)/.test(ovBg), ovBg);
  await p.keyboard.press("Escape"); await p.waitForTimeout(150);

  // 6. Every view opens without script errors.
  for (const v of await p.$$eval(".side button", bs => bs.map(x => x.title))) {
    await p.click(`.side button[title="${v}"]`);
    await p.waitForTimeout(250);
  }
  check("every view opens without errors", errors.length === 0, errors.join(" | "));

  await b.close();
  srv.close();
  console.log(failed ? `${failed} UI test(s) failed` : "All UI tests passed");
  process.exit(failed ? 1 : 0);
})();
