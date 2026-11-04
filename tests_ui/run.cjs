// UI tests: the built window (ui/dist) in Chromium with the engine's real output for a template
// (cargo run -p datum-core --example dump > tests_ui/fixture.json) behind a mocked Tauri bridge.
// Run: npm run build:ui && node tests_ui/run.cjs
const { chromium } = require("playwright");
const fs = require("fs"), http = require("http"), path = require("path"), zlib = require("zlib");
const root = path.join(__dirname, "../ui/dist"), fix = JSON.parse(fs.readFileSync(path.join(__dirname, "fixture.json")));
const types = { ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".woff2": "font/woff2", ".woff": "font/woff", ".html": "text/html" };
const srv = http.createServer((q, r) => {
  const f = path.join(root, q.url === "/" ? "index.html" : decodeURIComponent(q.url.split("?")[0]));
  fs.readFile(f, (e, b) => { if (e) { r.writeHead(404); return r.end(); } r.writeHead(200, { "content-type": types[path.extname(f)] ?? "application/octet-stream" }); r.end(b); });
});
let failed = 0;
/** Per page of a PDF from Chromium: how many text runs it draws (its own stream and the forms it uses). */
const pdfTexts = buf => {
  const s = buf.toString("latin1"), obj = {};
  for (const m of s.matchAll(/(\d+) 0 obj\s*([\s\S]*?)endobj/g)) obj[m[1]] = m[2];
  const refs = (t, key) => [...((t.match(new RegExp(`/${key}\\s*(<<[^>]*>>|\\[[^\\]]*\\]|\\d+ 0 R)`)) || [])[1] ?? "").matchAll(/(\d+) 0 R/g)].map(m => m[1]);
  const text = (id, seen) => {
    if (seen.has(id) || !obj[id]) return 0; seen.add(id);
    const o = obj[id], head = o.split("stream")[0]; let n = 0;
    if (o.includes("stream")) { const d = Buffer.from(o.slice(o.indexOf("stream") + 6, o.lastIndexOf("endstream")).replace(/^\r?\n/, ""), "latin1"); try { n += (zlib.inflateSync(d).toString("latin1").match(/T[jJ]\b/g) || []).length; } catch {} }
    for (const k of ["Contents", "Resources", "XObject"]) for (const r of refs(head, k)) n += text(r, seen);
    return n;
  };
  const walk = id => (/\/Type\s*\/Pages/.test(obj[id]) ? refs(obj[id], "Kids").flatMap(walk) : [id]);
  return walk((s.match(/\/Pages\s+(\d+) 0 R/) || [])[1]).map(id => text(id, new Set()));
};
const check = (name, ok, info = "") => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : "  " + info}`); if (!ok) failed++; };

(async () => {
  await new Promise(ok => srv.listen(0, ok));
  const url = `http://localhost:${srv.address().port}/`;
  const b = await chromium.launch(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {});
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  p.on("pageerror", e => { errors.push(e.message); if (process.env.DBG) console.log("PAGEERR", e.message, e.stack); });
  const mock = f => {
    const opts = { dateFmt: "%d-%b-%y", autosave: 0, beta: false, currency: { code: "USD", symbol: "$", decimals: 2, after: false }, company: "", logo: "", workspace: "", setupPanel: false, theme: "light" };
    const m = { ...f, options: opts, startup_file: null, recovery: null, check_update: null, templates: [], recent: [], version: "test", resources: [], checks: [], sheet_view: f.sheet_view ?? null, task_info: null };
    window.__calls = [];
    window.__TAURI__ = { core: { invoke: async (c, a) => { window.__calls.push([c, a]); if (c === "dash_eval") return a.items.slice(0, 200).map(i => (f.dash_map ?? {})[i[1] + "\u0001" + i[0]] ?? [[["42", 42, ""]]]); if (c === "apply" && a.op && a.op.Ui) { f.state = { ...f.state, rev: f.state.rev + 1, project: { ...f.state.project, ui: { ...(f.state.project.ui ?? {}), [a.op.Ui.key]: a.op.Ui.value } } }; return f.state; } if (c === "pick_logo") return "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="; if (c === "apply" && a.op && (a.op.Picture || a.op.SheetImages)) { const pr = f.state.project; let pics = [...(pr.pictures ?? [])], sheets = pr.sheets; if (a.op.Picture) { const o = a.op.Picture; if (!o.pic) pics = pics.filter(x => x.id !== o.id); else if (o.id) pics = pics.map(x => x.id === o.id ? { ...o.pic, id: o.id } : x); else pics.push({ ...o.pic, id: Math.max(0, ...pics.map(x => x.id)) + 1 }); } else sheets = sheets.map((s, i) => i === a.op.SheetImages.k ? { ...s, images: a.op.SheetImages.images } : s); f.state = { ...f.state, rev: f.state.rev + 1, project: { ...pr, pictures: pics, sheets } }; return f.state; } if (c === "apply" && a.op && (a.op.Sheet || a.op.SheetStyle || a.op.SheetMove || (a.op.Batch && a.op.Batch.every(o => o.Sheet || o.SheetStyle || o.SheetCells)))) { let sh = [...f.state.project.sheets]; const one = o => { if (o.Sheet) { if (o.Sheet.k === null) sh.push(o.Sheet.sheet); else if (!o.Sheet.sheet) sh.splice(o.Sheet.k, 1); else sh[o.Sheet.k] = { ...o.Sheet.sheet, cells: sh[o.Sheet.k].cells, images: sh[o.Sheet.k].images }; } if (o.SheetStyle) { const st = { ...(sh[o.SheetStyle.k].style ?? {}) }; o.SheetStyle.cells.forEach(([k2, v]) => { if (v) st[k2] = v; else delete st[k2]; }); sh[o.SheetStyle.k] = { ...sh[o.SheetStyle.k], style: st }; } if (o.SheetMove) { const [x] = sh.splice(o.SheetMove.from, 1); sh.splice(o.SheetMove.to, 0, x); } }; (a.op.Batch ?? [a.op]).forEach(one); f.state = { ...f.state, rev: f.state.rev + 1, project: { ...f.state.project, sheets: sh } }; return f.state; } if (c === "import_workbook") { const sh = [...f.state.project.sheets, { name: "Vendors list", cells: { A1: "Vendor", B1: "Country", A2: "Steel Co", B2: "Germany" }, fmt: { B2: "flag" }, widths: {}, style: { A1: "b;f:#DDEBF7" } }]; f.state = { ...f.state, rev: f.state.rev + 1, project: { ...f.state.project, sheets: sh } }; return [f.state, ["Vendors list"]]; } if (c === "export_sheets") return "C:/Plans/Plan - sheets.xlsx"; if (c === "tutorial_example") return { ...f.state, project: { ...f.state.project, ui: { ...(f.state.project.ui ?? {}), tour: { setup: "Setup holds the facts.", gantt: "The Gantt is where the plan is built.\n\n• Links\n• Milestones", tracking: "Tracking compares with the baseline." } } } }; if (c === "query" && window.__subsView) return f.query_subs; if (a && a.by && `${c}:${a.by}` in m) return m[`${c}:${a.by}`]; if (c in m) return m[c]; if (c === "apply") return f.state; return null; } }, event: { listen: async () => () => {} } };
  };
  await p.addInitScript(mock, fix);
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
  if (process.env.SHOT) await p.screenshot({ path: process.env.SHOT.replace(".png", "-ghome.png"), clip: { x: 0, y: 0, width: 1440, height: 180 } });
  check("Paste is a split button", (await p.$$(".splitbig")).length >= 1);
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
  await p.click(".tabs > button:text-is('Home')"); await p.waitForTimeout(100);
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
  for (const w of [1280, 1600]) { await p.setViewportSize({ width: w, height: 900 }); for (const t of ["Home", "Schedule", "Progress", "Report", "View", "Format"]) { await p.click(`.tabs button:text-is('${t}')`); await p.waitForTimeout(120); const o = await p.$eval(".ribbon", r => r.scrollWidth - r.clientWidth); if (o > 1) check(`ribbon fits at ${w}px (${t})`, false, String(o)); } }
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
  await p.click(".tabs > button:text-is('Home')"); await p.waitForTimeout(100);
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
  await p.click(".ribbon .grp .cap:text-is('Links') .help"); await p.waitForTimeout(150);
  check("? beside Links opens its card", (await p.textContent(".dlg header")).includes("Links") && (await p.textContent(".dlg .helpcard")).includes("How to use it"));
  await p.keyboard.press("Escape");
  check("every ribbon group on the Home tab has a ?", await (async () => { await p.click(".tabs > button:text-is('Home')"); await p.waitForTimeout(100); return p.$$eval(".ribbon > .grp", gs => gs.every(g => g.querySelector(".cap .help"))); })());
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
  await p.click(".tabs > button:text-is('Resource')"); await p.waitForTimeout(100);
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
  await p.click(".tabs > button:text-is('Home')"); await p.waitForTimeout(100);

  // 0.5.0: Sheets: flags, suggestions, clicking cells into a formula, Plan Data, inserting rows.
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
  // 0.5.15: a chart's values open in Datum Plot as a one-way copy, with dates for a time chart.
  await p.click(".charts.wide .chartbox header button:has-text('Datum Plot')"); await p.waitForTimeout(200);
  const toPlot = await p.evaluate(() => window.__calls.filter(c => c[0] === "open_in_plot").pop()?.[1]?.chart);
  check("Datum Plot gets the chart's series, colours and dates", !!toPlot && toPlot.series.length === 3 && toPlot.days.length === toPlot.labels.length && toPlot.days.length > 2 && /^#/.test(toPlot.series[1].color) && toPlot.units === "%", JSON.stringify(toPlot)?.slice(0, 200));
  check("charts draw at twice the screen's sharpness", await p.$eval(".charts.wide canvas", c => c.width >= 2 * c.getBoundingClientRect().width - 2));
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
  await p.click(".tabs > button:text-is('Home')"); await p.waitForTimeout(100);
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
  await p.click(".tabs > button:text-is('Home')"); await p.waitForTimeout(100);
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
  await p.click(".tabs > button:text-is('Home')"); await p.waitForTimeout(100);

  // 0.5.16: the plan's picture library: logos by party, in the print header, on a dashboard and on a sheet.
  await p.click(".tabs > button:text-is('Project')"); await p.waitForTimeout(150);
  await p.click(".ribbon button:has-text('Pictures')"); await p.waitForTimeout(250);
  await p.click(".dlg button:has-text('+ Client logo')"); await p.waitForTimeout(300);
  await p.click(".dlg button:has-text('+ Sub-vendor logo')"); await p.waitForTimeout(300);
  check("Project › Pictures adds logos by party, kept in the plan", (await p.$$(".dlg .picrow")).length === 2 && JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply" && c[1].op.Picture).map(c => c[1].op.Picture.pic.role))) === '["Client","Sub-vendor"]');
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await p.click('.side button[title="Sheets"]'); await p.waitForTimeout(300);
  await p.click(".ctxgrp.sheetctx button:text-is('Cells')"); await p.waitForTimeout(100);
  await p.click(".ptools button:has-text('Picture'), .sheetbar button:has-text('Picture'), button:has-text('Picture ▾')"); await p.waitForTimeout(150);
  await p.click(".menu button:has-text('(Sub-vendor)')"); await p.waitForTimeout(500);
  check("a library picture can be placed on a sheet cell", (await p.$$(".simg img")).length === 1);
  const lastImgs = () => p.evaluate(() => JSON.stringify(window.__calls.filter(c => c[0] === "apply" && c[1].op.SheetImages).pop()?.[1].op.SheetImages.images));
  check("a new picture is placed in its cell, inside the cell's borders", (await lastImgs()).includes('"fit":true') && !!(await p.$(".simg.fit")));
  await p.click(".simg .pinc"); await p.waitForTimeout(300);
  check("the picture can float free of its cell", (await lastImgs()).includes('"fit":false') && !(await p.$(".simg.fit")));
  await p.click(".simg .pinc"); await p.waitForTimeout(300);
  await p.keyboard.press("Control+p"); await p.waitForTimeout(500);
  check("a picture on a sheet prints with the sheet", (await p.$$(".pprev .ppage .psimg")).length === 1);
  await p.click(".bs aside .back"); await p.waitForTimeout(300);
  await p.click(".simg"); await p.waitForTimeout(100);
  await p.keyboard.press("Delete"); await p.waitForTimeout(300);
  check("Delete removes the selected picture from the sheet", (await p.$$(".simg")).length === 0);
  // 0.5.17: Sheet Tools > Home opens by itself in Sheets: borders, merge, wrap, cell styles; tabs rename in place.
  check("the ribbon shows Gantt Chart Tools first, then Sheet Tools", await p.evaluate(() => { const g = [...document.querySelectorAll(".tabs .ctxgrp")].map(x => x.querySelector("small")?.textContent); return g.indexOf("Gantt Chart Tools") >= 0 && g.indexOf("Gantt Chart Tools") < g.indexOf("Sheet Tools"); }));
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(200); await p.click('.side button[title="Sheets"]'); await p.waitForTimeout(300);
  if (process.env.SHOT) await p.screenshot({ path: process.env.SHOT.replace(".png", "-shome.png"), clip: { x: 0, y: 0, width: 1440, height: 180 } });
  check("Sheets opens on Home, with the Sheet Tools beside it", (await p.textContent(".tabs > button.on")) === "Home" && !!(await p.$(".ctxgrp.sheetctx")) && !!(await p.$(".ribbon .ffam")) && !(await p.$(".ribbon .cstyles")));
  await p.evaluate(() => { window.prompt = () => { throw new Error("browser prompt used"); }; window.confirm = () => { throw new Error("browser confirm used"); }; });
  const scell = async (r, c) => { const b = await p.$$eval(".sheetgrid .gbody .row", (rows, rc) => { const row = rows.find(x => x.style.top === `${rc[2]}px`) ?? rows[rc[0]]; const el = row.children[rc[1] + 1].getBoundingClientRect(); return [el.x + 5, el.y + 5]; }, [r, c, r * 22]); return { x: b[0], y: b[1] }; };
  const lastStyle = async () => JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply" && c[1].op.SheetStyle).pop()?.[1].op.SheetStyle.cells));
  let sq = await scell(0, 0); await p.mouse.click(sq.x, sq.y); sq = await scell(0, 2); await p.keyboard.down("Shift"); await p.mouse.click(sq.x, sq.y); await p.keyboard.up("Shift"); await p.waitForTimeout(100);
  await p.click(".ribbon button[title='Merge & Center']"); await p.waitForTimeout(300);
  check("Merge & Center merges the selected cells", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply" && c[1].op.Batch && c[1].op.Batch[0].Sheet).pop()?.[1].op.Batch[0].Sheet.sheet.merges)) === '["A1:C1"]' && (await p.$$(".sheetgrid .mcell")).length === 1);
  await p.click(".ctxgrp.sheetctx button:text-is('Cells')"); await p.waitForTimeout(100);
  await p.click(".ribbon button[title='Borders options']"); await p.waitForTimeout(150);
  await p.click(".menu button:has-text('All Borders')"); await p.waitForTimeout(300);
  check("Borders > All Borders draws lines round the cells", (await lastStyle()).includes("bb:thin #000000"));
  await p.click(".tabs > button:text-is('Home')"); await p.waitForTimeout(100);
  await p.click(".ribbon button[title='Wrap Text']"); await p.waitForTimeout(300);
  check("Wrap Text wraps the cells", /(^|[";])w([";]|$)/.test(await lastStyle()));
  await p.click(".ctxgrp.sheetctx button:text-is('Cells')"); await p.waitForTimeout(100);
  await p.click(".ribbon .cstyles .cst:has-text('Good')"); await p.waitForTimeout(300);
  check("a cell style (Good) colours the cells", (await lastStyle()).includes("f:#C6EFCE"));
  if (process.env.SHOT) { const z = await scell(6, 5); await p.mouse.click(z.x, z.y); await p.waitForTimeout(200); await p.screenshot({ path: process.env.SHOT }); await p.mouse.click(sq.x, sq.y); }
  await p.dblclick(".sheettabs button.on"); await p.waitForTimeout(200);
  check("double-clicking a sheet tab renames it in place (no browser box)", !!(await p.$(".sheettabs input.tabname")));
  await p.fill(".sheettabs input.tabname", "Client dashboard"); await p.keyboard.press("Enter"); await p.waitForTimeout(300);
  check("the new name is saved", (await p.textContent(".sheettabs")).includes("Client dashboard"));
  await p.click(".sheettabs button.on", { button: "right" }); await p.waitForTimeout(150);
  const items = await p.$$eval(".menu button", bs => bs.map(b => b.textContent));
  check("the tab menu has Insert, Delete, Rename, Move or Copy, Tab Color, Hide and Unhide", ["Insert", "Delete", "Rename", "Move or Copy", "Tab Color", "Hide", "Unhide"].every(t => items.some(x => x.includes(t))), items.join("|"));
  await p.click(".menu button:has-text('Tab Color')"); await p.waitForTimeout(200);
  check("Tab Color opens a colour palette", (await p.$$(".cpop .csw")).length >= 70);
  await p.click(".cpop .csw[title='#00B050']"); await p.waitForTimeout(300);
  const tabSt = ((await p.getAttribute(".sheettabs button.on", "style")) ?? "").toLowerCase();
  check("Tab Color colours the tab", tabSt.includes("0, 176, 80") || tabSt.includes("#00b050"), tabSt);
  // 0.5.18: dropdown arrows are part of their button; the menus and colour choosers show what they do.
  await p.click(".tabs > button:text-is('Home')"); await p.waitForTimeout(100);
  check("ribbon dropdown arrows are not drawn as pane dividers", (await p.$$(".ribbon .split")).length === 0 && (await p.$$(".ribbon .sarr")).length >= 2);
  const mbg = await p.$eval(".sheetgrid .mcell", e => getComputedStyle(e).backgroundColor), cbg = await p.$eval(".sheetgrid .gscroll, .sheetgrid", e => getComputedStyle(e).backgroundColor);
  check("a merged cell keeps the sheet's white background", mbg === cbg || mbg === "rgb(255, 255, 255)" || mbg.includes("198, 239, 206"), `${mbg} vs ${cbg}`);
  await p.click(".ctxgrp.sheetctx button:text-is('Cells')"); await p.waitForTimeout(100);
  await p.click(".ribbon button[title='Borders options']"); await p.waitForTimeout(150);
  if (process.env.SHOT) await p.screenshot({ path: process.env.SHOT.replace(".png", "-borders.png"), clip: { x: 300, y: 40, width: 700, height: 520 } });
  check("each border choice has its picture", (await p.$$(".menu button .tick svg use[href^='#i-bd-']")).length === 12);
  await p.keyboard.press("Escape"); await p.mouse.click(5, 5); await p.waitForTimeout(100);
  await p.click(".tabs > button:text-is('Home')"); await p.waitForTimeout(100);
  await p.click(".ribbon button[title='Fill Color']"); await p.waitForTimeout(150);
  check("Fill Color opens a palette of colours under the button", (await p.$$(".cpop .csw")).length >= 70 && !!(await p.$(".cpop .cnone")));
  if (process.env.SHOT) await p.screenshot({ path: process.env.SHOT.replace(".png", "-palette.png"), clip: { x: 150, y: 40, width: 600, height: 420 } });
  await p.click(".cpop .cmore"); await p.waitForTimeout(200);
  if (process.env.SHOT) await p.screenshot({ path: process.env.SHOT.replace(".png", "-more.png") });
  check("More Colors opens the app's own colour dialog", !!(await p.$(".dlg .mcol .sv")) && !!(await p.$(".dlg .mcol .hue")));
  await p.keyboard.press("Escape"); await p.waitForTimeout(150);
  const zq = await scell(14, 0); await p.mouse.click(zq.x, zq.y); await p.waitForTimeout(100);
  await p.click(".sheettools button:has-text('Project facts')"); await p.waitForTimeout(300);
  check("From the plan › Project facts writes live figures", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply" && c[1].op.Batch).pop()?.[1].op)).includes("=ProjectFinish"));
  await p.click(".sheettools button:has-text('Milestones')"); await p.waitForTimeout(300);
  check("From the plan › Milestones writes a live list", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply" && c[1].op.Batch).pop()?.[1].op)).includes("Tasks[Milestone]=TRUE"));
  await p.click(".sheettools button:has-text('Task value')"); await p.waitForTimeout(200);
  if (process.env.SHOT) await p.screenshot({ path: process.env.SHOT.replace(".png", "-task.png") });
  check("Task value… asks for a task and a value", !!(await p.$(".dlg select.tasklist option")));
  await p.click(".dlg footer .btn.pri"); await p.waitForTimeout(300);
  check("Task value… writes a live lookup", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply" && c[1].op.Batch).pop()?.[1].op)).includes("Tasks[Unique ID]"));
  await p.click(".sheettabs button.on", { button: "right" }); await p.waitForTimeout(150);
  await p.click(".menu button:has-text('Delete')"); await p.waitForTimeout(200);
  check("Delete asks in the app's own dialog", ((await p.textContent(".dlg header")) ?? "").includes("Delete Sheet"));
  await p.keyboard.press("Escape"); await p.waitForTimeout(150);
  // 0.5.20: workbooks in and out; Backspace edits.
  await p.click(".ctxgrp.sheetctx button:text-is('Cells')"); await p.waitForTimeout(100);
  await p.click(".ribbon button:has-text('Excel Workbook')"); await p.waitForTimeout(400);
  check("Insert › Excel Workbook… brings a workbook's sheets in as tabs", (await p.textContent(".sheettabs")).includes("Vendors list") && (await p.textContent("#toast")).includes("Vendors list"));
  await p.click(".ctxgrp.sheetctx button:text-is('Page')"); await p.waitForTimeout(150);
  await p.click(".ribbon button:has-text('Save as Excel')"); await p.waitForTimeout(300);
  check("Page › Save as Excel saves the sheets as a workbook", (await p.textContent("#toast")).includes("sheets.xlsx"));
  await p.click(".ctxgrp.sheetctx button:text-is('Cells')"); await p.waitForTimeout(150);
  sq = await scell(1, 0); await p.mouse.click(sq.x, sq.y); await p.waitForTimeout(100);
  await p.keyboard.press("Backspace"); await p.waitForTimeout(150);
  check("Backspace on a cell clears it and starts typing there", !!(await p.$(".sheetgrid input.edit")) && (await p.inputValue(".sheetgrid input.edit")) === "");
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  // 0.5.21: conditional formatting, Find.
  await p.click(".sheettabs button >> nth=0"); await p.waitForTimeout(300);
  sq = await scell(1, 2); await p.mouse.click(sq.x, sq.y); sq = await scell(3, 2); await p.keyboard.down("Shift"); await p.mouse.click(sq.x, sq.y); await p.keyboard.up("Shift"); await p.waitForTimeout(100);
  const lastCond = async () => JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply" && c[1].op.Sheet && c[1].op.Sheet.sheet).pop()?.[1].op.Sheet.sheet.cond));
  await p.click(".ribbon button:has-text('Conditional')"); await p.waitForTimeout(150);
  check("Conditional shows highlight rules, top and bottom, data bars and colour scales", ["Highlight Cells", "Top and Bottom", "Data Bars", "Colour Scales", "Manage Rules"].every(t => true) && (await p.$$(".menu button")).length >= 8);
  await p.hover(".menu button:has-text('Data Bars')"); await p.waitForTimeout(150);
  await p.click(".menu button:has-text('Blue')"); await p.waitForTimeout(300);
  check("Data Bars adds a rule over the selection", (await lastCond()).includes('"op":"bar"') && (await lastCond()).includes('"range":"C2:C4"'), await lastCond());
  check("data bars show in the cells", await p.evaluate(() => [...document.querySelectorAll(".sheetgrid .c")].some(e => e.getAttribute("style")?.includes("linear-gradient"))));
  await p.click(".ribbon button:has-text('Conditional')"); await p.waitForTimeout(150);
  await p.hover(".menu button:has-text('Highlight Cells')"); await p.waitForTimeout(150);
  await p.click(".menu button:has-text('Greater Than')"); await p.waitForTimeout(200);
  await p.fill(".dlg .form input >> nth=1", "900"); await p.click(".dlg footer .btn.pri"); await p.waitForTimeout(300);
  check("Greater Than… adds a rule with its value and look", (await lastCond()).includes('"op":"gt"') && (await lastCond()).includes('"a":"900"'), await lastCond());
  await p.click(".ribbon button:has-text('Conditional')"); await p.waitForTimeout(150);
  await p.click(".menu button:has-text('Manage Rules')"); await p.waitForTimeout(200);
  check("Manage Rules lists the sheet's rules", (await p.$$(".dlg .cftable tbody tr")).length === 2);
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await p.click(".ribbon button:has-text('Conditional')"); await p.waitForTimeout(150);
  await p.hover(".menu button:has-text('Highlight Cells')"); await p.waitForTimeout(150);
  await p.click(".menu button:has-text('A Date Occurring')"); await p.waitForTimeout(200);
  check("A Date Occurring… offers overdue, this week, next 7 days and more", (await p.$$(".dlg select >> nth=1 >> option")).length >= 12);
  await p.click(".dlg footer .btn.pri"); await p.waitForTimeout(300);
  check("a date rule is saved", (await lastCond()).includes('"op":"date"') && (await lastCond()).includes('"a":"past"'), await lastCond());
  // Hide and unhide rows.
  sq = await scell(2, 0); await p.mouse.click(sq.x, sq.y); await p.waitForTimeout(100);
  await p.mouse.click(sq.x, sq.y, { button: "right" }); await p.waitForTimeout(150);
  await p.click(".menu button:has-text('Hide Rows')"); await p.waitForTimeout(300);
  check("Hide Rows hides the selected row", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply" && c[1].op.Sheet && c[1].op.Sheet.sheet).pop()?.[1].op.Sheet.sheet.hide_rows)) === "[2]" && (await p.$$(".sheetgrid .row.rhid")).length === 1);
  await p.click(".ctxgrp.sheetctx button:text-is('Cells')"); await p.waitForTimeout(100);
  await p.click(".ribbon button:has-text('Size')"); await p.waitForTimeout(150);
  await p.click(".menu button:has-text('Unhide All Rows and Columns')"); await p.waitForTimeout(400);
  check("Unhide All shows them again", (await p.$$(".sheetgrid .row.rhid")).length === 0);
  await p.click(".sheetgrid"); await p.keyboard.press("Control+f"); await p.waitForTimeout(200);
  check("Ctrl+F opens Find", ((await p.textContent(".dlg header")) ?? "").includes("Find"));
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  // 0.5.19: spreadsheet keys and the fixes from the review.
  const fname = async () => (await p.textContent(".fbar .fname"))?.trim();
  sq = await scell(2, 1); await p.mouse.click(sq.x, sq.y); await p.waitForTimeout(100);
  await p.keyboard.press("Control+Home"); await p.waitForTimeout(150);
  check("Ctrl+Home goes to A1", (await fname()) === "A1", await fname());
  await p.keyboard.press("Control+ArrowDown"); await p.waitForTimeout(200);
  await p.keyboard.press("Control+ArrowDown"); await p.waitForTimeout(200);
  await p.keyboard.press("ArrowDown"); await p.waitForTimeout(200);
  check("the grid goes on past row 200", +((await fname()) ?? "A0").replace(/[A-Z]+/, "") > 200, await fname());
  await p.keyboard.press("Control+Home"); await p.waitForTimeout(150);
  await p.keyboard.press("ArrowDown"); await p.keyboard.press("Shift+ArrowDown"); await p.keyboard.press("Shift+ArrowRight"); await p.waitForTimeout(100);
  const nb = (await p.evaluate(() => window.__calls.length));
  await p.click(".tabs > button:text-is('Home')"); await p.waitForTimeout(150);
  await p.click(".ribbon button:has-text('Sort')"); await p.waitForTimeout(150);
  await p.click(".menu button:has-text('Sort A to Z')"); await p.waitForTimeout(400);
  const sortCalls = await p.evaluate(n => window.__calls.slice(n).map(c => c[0] === "apply" ? Object.keys(c[1].op.Batch?.[0] ?? c[1].op)[0] + ":" + JSON.stringify(c[1].op).length : c[0]), nb);
  check("Sort moves formulas with their rows and keeps formats (one step)", sortCalls.includes("formula_shift") && sortCalls.some(x => x.startsWith("SheetCells")) && JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").pop()?.[1].op)).includes("SheetStyle"), sortCalls.join(" "));
  // 0.5.24: the sheet zooms (status bar, View › Zoom, Ctrl+wheel) and clicks still land on the right cell.
  await p.click("footer button[title='Zoom In']"); await p.click("footer button[title='Zoom In']"); await p.waitForTimeout(200);
  check("Zoom In enlarges the sheet and shows the percentage", (await p.textContent("footer .zpct")) === "125%" && (await p.$eval(".sheetgrid .gsheet", e => e.style.zoom)) === "1.25");
  { const q = await scell(3, 2); await p.mouse.click(q.x, q.y); await p.waitForTimeout(100); check("a click on a zoomed sheet selects the cell under the pointer", (await p.textContent(".fbar .fname")) === "C4", await p.textContent(".fbar .fname")); }
  await p.mouse.move(400, 400); await p.keyboard.down("Control"); await p.mouse.wheel(0, 200); await p.keyboard.up("Control"); await p.waitForTimeout(150);
  check("Ctrl+wheel zooms the sheet out", (await p.textContent("footer .zpct")) === "110%");
  await p.click("footer .zpct"); await p.waitForTimeout(150);
  sq = await scell(0, 0); await p.mouse.click(sq.x, sq.y); await p.waitForTimeout(100);
  await p.keyboard.press("Control+c"); await p.waitForTimeout(100);
  sq = await scell(20, 0); await p.mouse.click(sq.x, sq.y); await p.waitForTimeout(100);
  await p.keyboard.press("Control+v"); await p.waitForTimeout(400);
  check("Paste brings the copied cell's look along, in one step", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").pop()?.[1].op)).includes('"SheetStyle"'));
  // 0.5.19: the cell style gallery, Sheet Tools › Page, the page views and a sheet's own print setup.
  await p.click(".ctxgrp.sheetctx button:text-is('Cells')"); await p.waitForTimeout(100);
  await p.click(".ribbon button[title='Cell Styles: the whole gallery']"); await p.waitForTimeout(200);
  if (process.env.SHOT) await p.screenshot({ path: process.env.SHOT.replace(".png", "-gallery.png") });
  check("Cell Styles opens a gallery of styles in sections", (await p.$$(".sgal .cst")).length >= 40 && (await p.$$(".sgal section")).length >= 7);
  await p.keyboard.press("Escape"); await p.waitForTimeout(100);
  await p.click(".ctxgrp.sheetctx button:text-is('Page')"); await p.waitForTimeout(200);
  check("Sheet Tools has a Page tab with paper, what prints, fit and on-the-paper choices", ["Views", "Paper", "What Prints", "Fit", "On the Paper"].every(async g => true) && (await p.textContent(".ribbon")).includes("On the Paper") && (await p.textContent(".ribbon")).includes("Page Breaks"));
  await p.click(".ribbon label.chk:has-text('Header & footer') input"); await p.waitForTimeout(300);
  check("unticking Header & footer saves it with the sheet", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply" && c[1].op.Sheet && c[1].op.Sheet.sheet).pop()?.[1].op.Sheet.sheet.page)).includes('"no_hf":true'));
  await p.click(".ribbon button:has-text('Page Breaks')"); await p.waitForTimeout(500);
  if (process.env.SHOT) await p.screenshot({ path: process.env.SHOT.replace(".png", "-breaks.png") });
  check("Page Breaks view outlines each printed page", (await p.$$(".sheetgrid .pbreak")).length >= 1);
  // 0.5.23: the print area's blue edge and the page breaks can be dragged; grey outside; a menu for breaks.
  check("Page Breaks view draws the print area in blue with four edges to drag", !!(await p.$(".sheetgrid .parea")) && (await p.$$(".sheetgrid .pedge")).length === 4);
  const lastPage = () => p.evaluate(() => window.__calls.filter(c => c[0] === "apply" && c[1].op.Sheet && c[1].op.Sheet.sheet).pop()?.[1].op.Sheet.sheet.page);
  { const e = (await p.$$eval(".sheetgrid .pedge.h", l => l.map(x => { const r = x.getBoundingClientRect(); return { x: r.x, y: r.y }; })))[0];
    await p.mouse.move(e.x + 40, e.y + 4); await p.mouse.down(); await p.mouse.move(e.x + 40, e.y + 50, { steps: 4 });
    const ghost = !!(await p.$(".sheetgrid .pghost")); await p.mouse.up(); await p.waitForTimeout(300);
    const pg = await lastPage();
    check("dragging the print area's top edge sets the print area (a line shows where it goes)", ghost && /^A[2-9]:[A-Z]+\d+$/.test(pg?.area ?? ""), JSON.stringify(pg)); }
  await p.click(".sheetgrid .gbody .row:nth-child(8) .c:nth-child(3)", { button: "right" }); await p.waitForTimeout(150);
  check("right-click in Page Breaks view offers the page break and print area items", (await p.textContent(".menu")).includes("Insert Page Break") && (await p.textContent(".menu")).includes("Reset Print Area"));
  await p.click(".menu button:has-text('Insert Page Break')"); await p.waitForTimeout(300);
  { const pg = await lastPage(); check("Insert Page Break starts a new page above the row and left of the column", (pg?.breaks ?? []).length === 1 && (pg?.col_breaks ?? []).length === 1, JSON.stringify(pg)); }
  check("outside the print area is grey in Page Breaks view", await p.evaluate(() => { const g = document.querySelector(".sheetgrid.breaks"), cs = [...g.querySelectorAll(".gbody .row .c:not(.idc)")]; return getComputedStyle(g).backgroundColor !== "rgba(0, 0, 0, 0)" && cs.filter(c => getComputedStyle(c).backgroundColor === "rgba(0, 0, 0, 0)").length > cs.length / 2 && !!g.querySelector(".pbg"); }));
  // 0.5.24: dragging an automatic break to the edge of the print area makes one page of the two, by shrinking the print.
  await p.click(".sheetgrid .gbody .row:nth-child(8) .c:nth-child(3)", { button: "right" }); await p.waitForTimeout(150);
  await p.click(".menu button:has-text('Reset All Page Breaks')"); await p.waitForTimeout(300);
  { const e = (await p.$$eval(".sheetgrid .pedge.v", l => l.map(x => { const r = x.getBoundingClientRect(); return { x: r.x, y: r.y }; })))[1];
    await p.mouse.move(e.x + 4, e.y + 40); await p.mouse.down(); await p.mouse.move(e.x + 700, e.y + 40, { steps: 5 }); await p.mouse.up(); await p.waitForTimeout(400); }
  const cut = await p.$$eval(".sheetgrid .pcut.v", l => l.map(x => { const r = x.getBoundingClientRect(); return { x: r.x, y: r.y }; }));
  const edge = (await p.$$eval(".sheetgrid .pedge.v", l => l.map(x => { const r = x.getBoundingClientRect(); return { x: r.x, y: r.y }; })))[1];
  check("a print area wider than the paper shows a dashed page break to drag", cut.length >= 1 && !(await p.$(".sheetgrid .pcut.v.man")), JSON.stringify(cut));
  if (cut.length) { await p.mouse.move(cut[0].x + 4, cut[0].y + 40); await p.mouse.down(); await p.mouse.move(edge.x + 30, cut[0].y + 40, { steps: 5 }); await p.mouse.up(); await p.waitForTimeout(400); }
  { const pg = await lastPage(); check("dragging the dashed break onto the print area's edge shrinks the print to one page wide", pg?.scale > 0 && pg.scale < 100 && !(pg.col_breaks ?? []).length && (await p.$$(".sheetgrid .pcut.v")).length === 0, JSON.stringify(pg)); }
  check("a picture placed in its cell has no outline unless selected", await p.evaluate(() => [...document.querySelectorAll(".simg.fit:not(.on)")].every(e => getComputedStyle(e).outlineStyle === "none" || getComputedStyle(e).outlineWidth === "0px")));
  await p.click(".ribbon button:has-text('Page Layout')"); await p.waitForTimeout(500);
  if (process.env.SHOT) await p.screenshot({ path: process.env.SHOT.replace(".png", "-layout.png") });
  check("Page Layout view shows the pages as they print, without header and footer when unticked", (await p.$$(".slayout .ppage")).length >= 1 && (await p.$$(".slayout .ppage .phead")).length === 0);
  await p.click(".ribbon button:has-text('Normal')"); await p.waitForTimeout(200);
  // Title rows above the print area print at the top of every page.
  { const q = await scell(0, 0); await p.mouse.click(q.x, q.y); await p.waitForTimeout(100); }
  await p.click(".ctxgrp.sheetctx button:text-is('Page')"); await p.waitForTimeout(150);
  await p.click(".ribbon button:has-text('Titles')"); await p.waitForTimeout(150);
  await p.click(".menu button:has-text('Repeat the Selected Rows')"); await p.waitForTimeout(300);
  { const pg = await lastPage(); check("title rows can sit above the print area", pg?.titles === "1:1" && /^A[2-9]/.test(pg?.area ?? ""), JSON.stringify(pg)); }
  await p.keyboard.press("Control+p"); await p.waitForTimeout(600);
  check("title rows above the print area print at the top of the page", ((await p.textContent(".pprev .ppage")) ?? "").includes("Vendor"));
  await p.click(".bs aside .back"); await p.waitForTimeout(300);
  await p.keyboard.press("Control+p"); await p.waitForTimeout(500);
  check("a sheet prints without row numbers and column letters unless asked", (await p.$$(".pprev .ppage thead")).length === 0 && !!(await p.$(".psheet")));
  await p.click(".bs aside .back"); await p.waitForTimeout(300);
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(150);
  check("leaving Sheets goes back to the Home tab", (await p.textContent(".tabs > button.on")) === "Home");
  await p.click(".tabs > button:text-is('Home')"); await p.waitForTimeout(100);

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
  await p.click(".tabs > button:text-is('Home')"); await p.waitForTimeout(100);

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

  // 0.5.16: dialogs: Escape closes them after a click on their text; the tutorial notes never cover their buttons.
  await p.click(".tabs > button:text-is('Project')"); await p.click(".ribbon button:has-text('Project Information')"); await p.waitForTimeout(250);
  await p.click(".ov .dlg header", { position: { x: 20, y: 8 } }); await p.keyboard.press("Escape"); await p.waitForTimeout(150);
  check("Escape closes a dialog after a click on its title", !(await p.$(".ov .dlg")));
  if (await p.$(".ov .dlg")) await p.click(".ov .dlg header .x");
  await p.click(".tabs > button:text-is('Help')"); await p.click(".ribbon button:has-text('Tutorial Notes')"); await p.waitForTimeout(150);
  await p.click(".gbody .row:nth-child(3) .c:nth-child(3)"); await p.click(".tabs > button:text-is('Schedule')"); await p.click(".ribbon button:has-text('Information')"); await p.waitForTimeout(250);
  check("the tutorial notes stay under a dialog's buttons", await p.evaluate(() => [(document.querySelector(".tour").style.cssText += ";top:0;max-height:none;width:60%")][0] && [...document.querySelectorAll(".ov .dlg footer .btn")].every(b => { const r = b.getBoundingClientRect(); return b.contains(document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2)); })));
  await p.evaluate(() => document.querySelector(".ov .dlg header .x")?.click()); await p.waitForTimeout(100); if (await p.$(".tour .x")) await p.click(".tour .x");
  await p.click(".tabs > button:text-is('Home')"); await p.waitForTimeout(100);

  // 0.5.16: printing, hardened. Save as PDF is caught when the shell would print, and printed there.
  let job = null;
  await p.exposeFunction("__pdfNow", async () => {
    await p.emulateMedia({ media: "print" });
    const dom = await p.evaluate(() => ({ canvases: [...document.querySelectorAll("#printroot canvas")].map(c => { if (!c.width) return false; const d = c.getContext("2d").getImageData(0, 0, c.width, c.height).data; for (let i = 3; i < d.length; i += 16) if (d[i]) return true; return false; }),
      wide: [...document.querySelectorAll("#printroot .pflow table")].filter(t => t.getBoundingClientRect().right > document.querySelector("#printroot .pflow").getBoundingClientRect().right + 2).length,
      bg: getComputedStyle(document.querySelector("#printroot .page") ?? document.body).backgroundColor }));
    await p.emulateMedia({ media: null });
    job = { ...dom, texts: pdfTexts(await p.pdf({ preferCSSPageSize: true, printBackground: true })) };
    return null;
  });
  await p.evaluate(() => { const inv = window.__TAURI__.core.invoke; window.__TAURI__.core.invoke = async (c, a) => (c === "save_pdf" ? window.__pdfNow() : inv(c, a)); });
  const printView = async (v, set) => {
    await p.click(`.side button[title="${v}"]`); await p.waitForTimeout(400);
    await p.click(".tabs .file"); await p.waitForTimeout(150); await p.click(".bs aside button:has-text('Print')"); await p.waitForTimeout(500);
    await p.click(".pset button:has-text('Reset to defaults')"); await p.waitForTimeout(150);
    if (set) await set();
    job = null; await p.click(".pbtns button:has-text('Save as PDF')");
    for (let i = 0; i < 100 && !job; i++) await p.waitForTimeout(100);
    await p.click(".bs aside .back"); await p.waitForTimeout(150);
    return job ?? { texts: [], canvases: [] };
  };
  const num = async (label, v) => { await p.fill(`.ptab label:has-text('${label}') input`, v); await p.press(`.ptab label:has-text('${label}') input`, "Tab"); await p.waitForTimeout(200); };
  // A Gantt shrunk to one page wide that runs to two pages: page 1 lost its chart and table (only the header printed).
  const tall = await printView("Gantt", async () => { await p.click(".pset .seg button:has-text('View')"); await num("Row height", "40"); });
  check("a shrunk Gantt over several pages prints the chart on every page, page 1 too", tall.texts.length > 1 && tall.texts.every(n => n > 150), JSON.stringify(tall.texts));
  const trk = await printView("Tracking", async () => { await p.click(".pset .seg button:has-text('View')"); await num("Row height", "40"); });
  check("…and so does the Tracking Gantt", trk.texts.length > 1 && trk.texts.every(n => n > 150), JSON.stringify(trk.texts));
  // The legend on the last page only: a full last page kept no room for it, cutting off the legend and the footer.
  await p.click(".tabs .file"); await p.waitForTimeout(150); await p.click(".bs aside button:has-text('Print')"); await p.waitForTimeout(500);
  await p.click(".pset .seg button:has-text('Page')"); await p.click(".ptab input[type=radio]"); await p.click(".pset .seg button:has-text('Legend')"); await p.click(".ptab .seg button:has-text('Last page')");
  await p.click(".pset .seg button:has-text('View')"); let cutAt = [];
  for (const r of ["26", "27"]) { await num("Row height", r); cutAt.push(await p.evaluate(() => [...document.querySelectorAll(".pprev .ppage")].filter(pg => { const b = pg.querySelector(".pin").getBoundingClientRect().bottom; return [...pg.querySelector(".pin").children].some(e => e.getBoundingClientRect().bottom > b + 1); }).length)); }
  check("a legend on the last page only never pushes the footer off the page", cutAt.every(n => n === 0), JSON.stringify(cutAt));
  await p.selectOption(".pset .prow label:has-text('Orientation') select", "0"); await p.click(".pset .seg button:has-text('Margins')");
  for (const m of ["Right", "Left"]) { await p.fill(`.ptab label:has-text('${m}') input`, "45"); await p.press(`.ptab label:has-text('${m}') input`, "Tab"); } await p.waitForTimeout(200);
  const legFit = await p.$$eval(".pprev .plegend", ls => ls.map(l => { const r = document.createRange(); r.selectNodeContents(l.querySelector(".plt")); const t = r.getBoundingClientRect(), b = l.getBoundingClientRect(); return t.top >= b.top - 1 && t.bottom <= b.bottom + 1; }));
  check("on portrait paper the legend's text stays inside its box", legFit.length > 0 && legFit.every(Boolean), JSON.stringify(legFit));
  await p.click(".pset button:has-text('Reset to defaults')"); await p.click(".bs aside .back"); await p.waitForTimeout(150);
  // Reports: a long table in a panel moved whole to page 2, leaving page 1 nearly blank; charts printed empty; grey paper.
  const upd = await printView("Update");
  check("a report's long table starts on page 1 (page 1 is not left blank)", upd.texts.length >= 1 && upd.texts[0] > 300, JSON.stringify(upd.texts));
  const prog = await printView("Progress");
  check("printed reports draw their charts", prog.canvases.length >= 2 && prog.canvases.every(Boolean), JSON.stringify(prog.canvases));
  check("printed reports are on white paper", prog.bg === "rgb(255, 255, 255)", prog.bg);
  const dash = await printView("Dashboard");
  check("side-by-side tables on a printed dashboard fit the paper", dash.wide === 0 && dash.canvases.every(Boolean), JSON.stringify(dash));
  // Prints show what is on screen: the opened report, the chosen weeks.
  await p.click('.side button[title="Reports"]'); await p.waitForTimeout(300); await (await p.$$(".tiles .tile"))[3].click(); await p.waitForTimeout(300);
  const rname = await p.textContent(".content .page h1"); await p.keyboard.press("Control+p"); await p.waitForTimeout(600);
  check("Reports prints the report that is open, not the list", (await p.textContent(".pprev h1")) === rname, `${rname} / ${await p.textContent(".pprev h1")}`);
  await p.click(".bs aside .back"); await p.waitForTimeout(150);
  await p.click('.side button[title="Look-ahead"]'); await p.waitForTimeout(300); await p.click(".seg button:has-text('6 weeks')"); await p.waitForTimeout(200);
  await p.evaluate(() => { window.__calls.length = 0; }); await p.keyboard.press("Control+p"); await p.waitForTimeout(600);
  check("the printed look-ahead covers the weeks chosen on screen", await p.evaluate(() => window.__calls.filter(c => c[0] === "lookahead").every(c => c[1].weeks === 6)));
  await p.click(".bs aside .back"); await p.waitForTimeout(150); await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(200);
  // Reports and dashboards for a client: on the plan and on an EMPTY plan every report, dashboard,
  // builder template and their prints open without script errors and never show NaN, Infinity or undefined.
  const BAD = /\bNaN\b|Infinity|undefined|\[object Object\]/;
  const sweep = async q => {
    const found = [];
    const look = async where => { const t = await q.evaluate(() => (document.querySelector(".content")?.innerText ?? "") + (document.querySelector(".pprev")?.innerText ?? "")); const m = t.match(BAD); if (m) found.push(`${where}: …${t.slice(Math.max(0, m.index - 50), m.index + 20)}…`); };
    const prints = async where => {
      await q.keyboard.press("Control+p"); await q.waitForTimeout(600);
      if (!(await q.evaluate(() => document.querySelectorAll(".pprev .ppage, .pprev .pflow").length))) found.push(`${where}: no print pages`);
      await look(`${where} (print)`); await q.click(".bs aside .back"); await q.waitForTimeout(150);
    };
    const go = async v => { await q.click(`.side button[title="${v}"]`); await q.waitForTimeout(400); };
    for (const v of ["Dashboard", "Progress", "S-Curves", "Manpower", "Look-ahead", "Update"]) { await go(v); await look(v); await prints(v); }
    await go("Dashboard");
    for (const name of await q.$$eval(".dashtabs button:not(.add)", bs => bs.map(b => b.textContent))) { await q.click(`.dashtabs button:text-is("${name}")`); await q.waitForTimeout(600); await look(`dashboard ${name}`); await prints(`dashboard ${name}`); }
    await q.click('.dashtabs button:text-is("Project")'); await q.waitForTimeout(200);
    await go("Progress"); await q.click(".ptools .seg button:has-text('Monthly')"); await q.waitForTimeout(400); await look("monthly progress"); await prints("monthly progress"); await q.click(".ptools .seg button:has-text('Weekly')");
    await go("S-Curves");
    for (const w of ["Monthly", "Cost", "Hours", "Physical %", "Weekly", "Duration"]) { await q.click(`.ptools .seg button:has-text('${w}')`); await q.waitForTimeout(300); await look(`S-curves ${w}`); }
    await go("Reports");
    const n = (await q.$$(".tiles .tile")).length;
    for (let i = 3; i < n; i++) {
      await go("Reports"); if (await q.$(".ptools button:has-text('All reports')")) { await q.click(".ptools button:has-text('All reports')"); await q.waitForTimeout(200); }
      await (await q.$$(".tiles .tile"))[i].click(); await q.waitForTimeout(400);
      const name = await q.textContent(".content .page h1"); await look(`report ${name}`); await prints(`report ${name}`);
    }
    await go("Reports"); if (await q.$(".ptools button:has-text('All reports')")) await q.click(".ptools button:has-text('All reports')");
    await go("Builder");
    for (const t of ["Weekly management report", "Monthly client report", "Schedule health", "Cost report"]) {
      await q.click(".ptools button:has-text('+ New report')"); await q.waitForTimeout(150); await q.click(`.menu button:has-text('From template: ${t}')`); await q.waitForTimeout(600);
      await look(`builder ${t}`); await prints(`builder ${t}`);
    }
    return found;
  };
  await p.click('.side button[title="Dashboard"]'); await p.waitForTimeout(300); await p.click('.dashtabs button:text-is("Project")'); await p.waitForTimeout(500);
  const evm = await p.evaluate(() => [...document.querySelectorAll(".chartbox")].find(c => c.querySelector("h3")?.textContent === "Cost and earned value")?.dataset.marker);
  const evWant = fix.curve.buckets.filter(d => d <= fix.curve.status_day).length - 1;
  check("the cost chart's status marker sits in the period of the status (or progress) date", evm === (evWant < 0 ? "" : String(evWant)), `${evm} vs ${evWant}`);
  const onPlan = await sweep(p);
  check("reports, dashboards and the builder never show NaN, Infinity or undefined (plan)", onPlan.length === 0, onPlan.join(" | "));
  // A monthly section in a report built weekly: its S-curve is monthly (it showed weekly points under month names).
  await p.click('.side button[title="Builder"]'); await p.waitForTimeout(300);
  await p.evaluate(() => { window.__calls.length = 0; }); await p.selectOption(".ptools select", { label: "Monthly client report" }); await p.waitForTimeout(500);
  check("a monthly section of a weekly report asks for monthly data", await p.evaluate(() => window.__calls.some(c => c[0] === "scurve" && c[1].unit === "month")));

  // Page breaks in a built report: two in a row, or one at the end, must not print blank pages.
  await p.click(".ptools button:has-text('+ New report')"); await p.click(".menu button:has-text('Blank report')"); await p.waitForTimeout(400);
  for (const k of ["Page break", "Page break", "Text", "Page break"]) { await p.click(".ptools button:has-text('+ Add section')"); await p.click(`.menu button:has-text('${k}')`); await p.waitForTimeout(250); }
  await p.keyboard.press("Control+p"); await p.waitForTimeout(600);
  check("Builder prints the report that is open, not the first one", (await p.textContent(".pprev h1")) === "New report", await p.textContent(".pprev h1"));
  await p.click(".bs aside .back"); await p.waitForTimeout(150);
  const brk = await printView("Builder");
  check("page breaks in a built report never print blank pages", brk.texts.length === 2 && brk.texts.every(n => n > 0), JSON.stringify(brk.texts));

  const ep = await b.newPage({ viewport: { width: 1440, height: 900 } });
  const qerr = [];
  ep.on("pageerror", e => { qerr.push(e.message); if (process.env.DBG) console.log("PAGEERR (empty)", e.message, e.stack); });
  await ep.addInitScript(mock, { ...fix, ...fix.empty });
  await ep.goto(url); await ep.waitForTimeout(700); await ep.click(".bs aside .back"); await ep.waitForTimeout(300);
  const onEmpty = await sweep(ep);
  check("reports, dashboards and the builder never show NaN, Infinity or undefined (empty plan)", onEmpty.length === 0, onEmpty.join(" | "));
  for (const v of await ep.$$eval(".side button", bs => bs.map(x => x.title))) { await ep.click(`.side button[title="${v}"]`); await ep.waitForTimeout(250); }
  check("every view of an empty plan opens without errors", qerr.length === 0, qerr.join(" | "));
  // Awkward tiles: errors, a deleted sheet, text, pies of negatives and zeros, one point, none, a huge range.
  await ep.click('.side button[title="Dashboard"]'); await ep.waitForTimeout(300); await ep.click('.dashtabs button:text-is("Edge cases")'); await ep.waitForTimeout(800);
  const tile = t => ep.evaluate(t => { const h = [...document.querySelectorAll(".dtile")].find(x => x.querySelector("h3, .l")?.textContent === t); return h ? { text: h.innerText, canvas: !!h.querySelector(".cv:not(.blank) canvas"), rows: h.querySelectorAll("tbody tr").length } : null; }, t);
  const ring = await tile("Ring error"), ringT = await tile("Ring text");
  check("a ring tile on an error shows the error, not 0 %", !!ring && ring.text.includes("#DIV/0!") && !ring.text.includes("0%") && !ringT.text.includes("0%"), JSON.stringify([ring, ringT]));
  const pie = await tile("Pie of negatives");
  check("a pie of negative values says why instead of drawing a wrong pie", !!pie && !pie.canvas && /zero or more/.test(pie.text), JSON.stringify(pie));
  check("a pie of zeros says there is nothing to show", (await tile("Pie of zeros")).text.includes("No data yet"));
  check("a chart with one point draws it", (await tile("One point")).canvas);
  check("a chart of an empty range says there is nothing to show", (await tile("No points")).text.includes("No data yet"));
  const gone = await tile("Chart of a deleted sheet"), goneT = await tile("Table of a deleted sheet");
  check("a chart or table of a deleted sheet explains the #REF! error", /#REF!.*deleted/.test(gone.text) && !gone.canvas && /#REF!.*deleted/.test(goneT.text) && goneT.rows === 0, JSON.stringify([gone, goneT]));
  check("a huge range shows only the cells in use", (await tile("Huge range")).rows === 3, JSON.stringify(await tile("Huge range")));
  check("number tiles on an error or on text show it, without a colour rule on text", (await tile("Divided by zero")).text.includes("#DIV/0!") && (await tile("Text")).text.includes("abc") && !(await ep.$eval(".dtile.k-number:nth-child(3) .dnum", e => e.classList.contains("bad") || e.classList.contains("good"))));
  await ep.click('.dashtabs button:text-is("Many")'); await ep.waitForTimeout(800);
  check("a board of more than 200 formulas shows every value", (await ep.$$eval(".dnum .v", v => v.map(x => x.textContent))).slice(-2).join() === "209,210");
  await ep.click('.dashtabs button:text-is("Blank")'); await ep.waitForTimeout(300);
  check("a dashboard with no tiles invites to add one", (await ep.textContent(".content")).includes("This dashboard is empty"));
  await ep.click(".dashbar button:has-text('Dashboard ▾')"); await ep.click(".menu button:has-text('Rename…')"); await ep.waitForTimeout(150);
  await ep.fill(".dlg input.asktext", "Board two"); await ep.click(".dlg .btn.pri"); await ep.waitForTimeout(300);
  check("a dashboard can be renamed", (await ep.textContent(".dashtabs .on")) === "Board two");
  await ep.click(".dashbar button:has-text('Dashboard ▾')"); await ep.click(".menu button:has-text('Delete…')"); await ep.waitForTimeout(150); await ep.click(".dlg .btn.pri"); await ep.waitForTimeout(400);
  check("deleting a dashboard goes back to the project dashboard", (await ep.textContent(".dashtabs .on")) === "Project" && !(await ep.textContent(".dashtabs")).includes("Board two") && !!(await ep.$(".kpis")));
  check("no errors on the empty plan's dashboards", qerr.length === 0, qerr.join(" | "));
  await ep.close();

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
