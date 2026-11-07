// How the window copes with a very large plan: the fixture's task rows copied up to N rows
// (100,000 by default) behind the same mocked bridge as run.cjs. Prints the times; fails when
// the first paint or scrolling is far slower than the budget.
// Run: npm run build:ui && node tests_ui/perf.cjs [rows]
const { chromium } = require("playwright");
const fs = require("fs"), http = require("http"), path = require("path");
const N = +(process.argv[2] || 100000);
const root = path.join(__dirname, "../ui/dist"), fix = JSON.parse(fs.readFileSync(path.join(__dirname, "fixture.json")));
const base = fix.query.rows.filter(r => r.uid > 0), rows = [];
for (let i = 0; i < N; i++) { const r = base[i % base.length]; rows.push({ ...r, uid: i + 1, id: i + 1, name: `${r.name} ${i + 1}`, bar: { ...r.bar, links: i % 50 ? [[i, "FS"]] : [] } }); }
fix.query = { ...fix.query, rows, shown: N, total: N };
const types = { ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".woff2": "font/woff2", ".html": "text/html" };
const srv = http.createServer((q, r) => {
  const f = path.join(root, q.url === "/" ? "index.html" : decodeURIComponent(q.url.split("?")[0]));
  fs.readFile(f, (e, b) => { if (e) { r.writeHead(404); return r.end(); } r.writeHead(200, { "content-type": types[path.extname(f)] ?? "application/octet-stream" }); r.end(b); });
});
(async () => {
  await new Promise(ok => srv.listen(0, ok));
  const b = await chromium.launch(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {});
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  await p.addInitScript(f => {
    const opts = { dateFmt: "%d-%b-%y", autosave: 0, beta: false, currency: { code: "USD", symbol: "$", decimals: 2, after: false }, company: "", logo: "", workspace: "", setupPanel: false, theme: "light" };
    const m = { ...f, options: opts, startup_file: null, recovery: null, check_update: null, templates: [], recent: [], version: "test", resources: [], checks: [], task_info: null };
    window.__TAURI__ = { core: { invoke: async (c, a) => (c === "query" ? (a?.q?.filter?.rules?.some(r => r.field === "timeline") ? { ...f.query, rows: [] } : f.query) : m[c] ?? null) }, event: { listen: async () => () => {} }, window: { getCurrentWindow: () => ({ onCloseRequested: async () => () => {}, setTitle: async () => {} }) } };
  }, fix);
  await p.goto(`http://localhost:${srv.address().port}/`);
  await p.waitForTimeout(500);
  const t0 = Date.now();
  await p.click(".bs aside .back");
  await p.waitForFunction(n => document.querySelectorAll(".gbody .row").length > 10 && document.querySelector(".gbody")?.textContent?.includes(" 1"), N, { timeout: 60000 });
  const first = Date.now() - t0;
  const scroll = await p.evaluate(async () => {
    const g = document.querySelector(".gscroll"), times = [];
    for (let k = 1; k <= 30; k++) {
      const t = performance.now();
      g.scrollTop = (g.scrollHeight - g.clientHeight) * k / 30;
      g.dispatchEvent(new Event("scroll"));
      await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
      times.push(performance.now() - t);
    }
    times.sort((a, b) => a - b);
    return { median: times[15], worst: times[29], last: document.querySelector(".gbody .row:last-child")?.textContent?.slice(0, 40) };
  });
  console.log(`${N} rows: first paint ${first} ms; scroll frame median ${scroll.median.toFixed(1)} ms, worst ${scroll.worst.toFixed(1)} ms`);
  const ok = first < 15000 && scroll.median < 120;
  console.log(ok ? "Within budget" : "TOO SLOW");
  await b.close(); srv.close();
  process.exit(ok ? 0 : 1);
})();
