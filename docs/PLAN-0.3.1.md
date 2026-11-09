# Datum Plan 0.3.1+: spreadsheet core, CentriForge design, period reporting

Status: **answered and built as 0.3.1** (decisions in MEMORY.md §2). The user kept the ribbon (style only), so §2.C's command bar was dropped; reporting moved into 0.3.1.

## 1. What went wrong in 0.3.0 (honest diagnosis)

| Symptom the user saw | Root cause |
| --- | --- |
| Cells misaligned between header and rows | Header and rows are separate DOM flex rows with their own borders, padding and scrollbars. Widths drift by a pixel or two, and more when the vertical scrollbar appears. |
| Cannot select many cells, drag a range, delete or fill a block | The grid has only a row selection and one "current cell". There is no range model. |
| No obvious place for a formula (e.g. % complete − baseline %) | Formulas exist only inside Custom Fields → Formula, with classic formula syntax. There is no formula bar and no way to type `=` into a cell. |
| Reports feel thin | Reports are lists of tables. They have no period model (last week vs this week), no narrative or notes, and the charts are weak. |
| UI "not up to the mark" | It mixes a ribbon, a sidebar and a bottom pane, all with different styles. It is not the clean, consistent CentriForge look. |
| Bugs | It was tested by hand only. There are no automated UI tests in CI. |

Lesson: we built many features on a weak base. 0.3.1 rebuilds the **base**: the grid, the design and the reporting model. Then features go on top.

## 2. Direction

### A. A real spreadsheet core (using Gridcraft)
Gridcraft (storytold/gridcraft, MIT or Apache-2.0, pure Rust) is a complete spreadsheet engine split into small crates with few dependencies:

| Gridcraft crate | Size | What we use it for |
| --- | --- | --- |
| `formula` + `functions` + `calc` + `core` + `numfmt` | ~30k lines | Spreadsheet formulas (about 400 functions: IF, SUMIFS, XLOOKUP, NETWORKDAYS, WORKDAY, dates, statistics, LET/LAMBDA), dependency recalc, spreadsheet number formats |
| `model` | 3.6k | A sheet model for free spreadsheet tabs |
| `xlsx` | 25k | Real workbook (.xlsx) export and import with formulas, formats and outline (replaces our minimal xlsx writer) |
| `pdf`, `chart` | 1.3k / 4.4k | True PDF output for reports and server-side charts (later) |

We **add these as Cargo dependencies pinned to a commit** (no copy-paste fork), with attribution in NOTICE. Our scheduling engine stays our own: Gridcraft does not schedule.

How the plan uses the spreadsheet engine:
1. **Formula columns, spreadsheet style.** Type `=` in any custom column, e.g. `=[% Complete]-[Baseline % Complete]` or `=IF([Finish]>[Baseline Finish],"Late","OK")`. `[Field]` names map to task fields. All Gridcraft functions work. The classic formula syntax ([Field], IIf) still loads, translated on open.
2. **A formula bar** above the grid shows and edits the current cell's value or formula.
3. **Sheet tabs inside the plan** (workbook style): free spreadsheets for quantities, estimates, rate build-ups and progress measurement. Cells can reference plan data (`=TASK("A1040","Finish")`) and tasks can pull from sheets.
4. **workbook (.xlsx) export with live formulas** (variance columns stay formulas in the file).

### B. A new grid, built like a spreadsheet
One component used everywhere: the task sheet, resource sheet, usage, sheets and report tables.
- **Canvas-rendered body and headers from the same column metrics**, so they always line up. Frozen ID and name columns. 100k rows smooth.
- **Range selection:** click-drag, Shift+click, Shift+arrows, Ctrl+click for several ranges, row and column headers to select whole rows or columns, Ctrl+A.
- **Range operations as one undo step:** Delete clears, typing + Ctrl+Enter fills the selection, fill handle drag (series for dates and IDs), Ctrl+D and Ctrl+R, copy and paste TSV ranges to and from spreadsheets, paste special (values only).
- Spreadsheet keys: F2, Enter or Tab moves, Esc cancels, Ctrl+arrows jump, Home/End, PageUp/Down, Ctrl+Z/Y.
- Status-bar aggregates for a selection (Sum, Average, Count).
- Column header filter buttons (AutoFilter), sort, resize, drag-reorder, freeze.
- Conditional formatting: data bars and colour rules (e.g. negative float in red).
- Engine: new ops `SetRange`, `FillRange`, `ClearRange`, `PasteRange`, all applied as one batch.

### C. CentriForge design system (one family of apps)
Use the same look as Compressor Engineering Suite, so Datum Plan is clearly from the same developer:
- **Tokens:** `--bg #f2f3ef`, `--panel #f8f8f5`, `--ink #0c1a2b`, `--accent #0b5cad`, `--accent-2 #22b8e6`, pass/fail/attention colours, a dark theme (Theme · Auto/Light/Dark).
- **Type:** IBM Plex Sans for text, IBM Plex Mono for uppercase spaced labels, tabs, buttons, numbers and IDs. Square corners and 1 px lines, no shadows.
- **Shell:** top bar `DATUM PLAN | FILE ▾ | LAUNCHER | WORKSPACE | product strap | THEME`.
  - **Launcher** (home): hero with version, updates and news; the current project's KPIs; module tiles in workflow order, each with "How to use", a status pill and OPEN.
  - **Workspace:** a left nav grouped by stage, a sticky header with crumb, plan title, revision and a primary action (e.g. RECALCULATE / UPDATE PROGRESS).
- **Modules (left nav), in workflow order:**
  1. INPUTS: Project setup · Calendars · Codes & custom fields
  2. PLAN: WBS & tasks (sheet + Gantt) · Network · Calendar view
  3. RESOURCES: Resource sheet · Usage & histogram · Team planner · Leveling
  4. COST: Budget & rates · Cash flow · Earned value
  5. PROGRESS: Status update · Weekly · Monthly · Look-ahead
  6. CONTROL: Baselines & compare · Schedule checks · Risks
  7. REPORTS: Dashboard · Report builder · Exports
  8. PORTFOLIO: All plans
- Inside the Plan module: a **compact command bar** (CentriForge-style mono buttons) and a slim contextual toolbar. The full ribbon becomes optional (see Q1).

### D. Progress periods and reporting (what makes Datum Plan stand out)
- **Reporting periods are first-class.** A weekly or monthly calendar of cut-off dates. **Update progress** walks you through the period: tasks due, started, finished and in progress, each with actual dates, %, remaining and a comment. Closing the period stores a snapshot automatically.
- **Period comparison everywhere:** previous period, this period and variance for planned %, actual %, earned, cost, finish date, float and milestones.
- **Report structure** (the user's order): 1. **Dashboard** (KPIs with arrows vs last period) → 2. **Charts** (S-curve planned/actual/forecast cumulative %, period progress bars, manpower histogram, cost and EV curves, milestone trend chart, float erosion) → 3. **Tables** (completed this period, started, slipped, critical changes, 2–4-week look-ahead, progress by WBS this vs last period) → 4. **Notes** (editable narrative sections: summary, issues, risks, actions, photos), saved per period with the plan.
- **Report builder:** pick sections, reorder, brand them (logo, colours), save as templates (Weekly, Monthly, Client, Management).
- **Output:** real PDF (Gridcraft pdf or the print engine), workbook (.xlsx) with formulas, HTML.

### E. Python: recommendation
Python libraries (pandas, matplotlib, reportlab, numpy) are excellent, but bundling Python in the app means:
- about 30–60 MB more per install and per update, and a slower start (we are at 2.6 MB today);
- a second runtime and language to package, sign and debug on Windows.

Everything we need is already available in Rust or in the browser:
- formulas and statistics: Gridcraft;
- Monte Carlo schedule risk: a few hundred lines of Rust, faster than numpy for this;
- charts: SVG in the window;
- PDF: Gridcraft pdf, or WebView print.

**Recommendation:** no Python in 0.3.x. Keep the door open for an **optional "Python scripts" add-on** (0.5+): the user's own Python runs against an exported plan for custom analytics. That only makes sense if you want scripting.

### F. Quality bar (so the bugs stop)
- A Playwright UI test suite in CI. The harness from session 5 (real engine fixtures + mocked shell) grows into tests for each module.
- A spreadsheet checklist for the grid (selection, keys, clipboard, fill, alignment) and the acceptance scenarios as engine tests.
- Each release is checked against your bug write-up before it ships.

## 3. What stays from 0.3.0
The engine (scheduling, leveling, fields, EV, checks, network, IO), the shell commands, the updater and the file format. Most of the 0.3.0 work is engine work and stays. The **window** is what gets rebuilt.

## 4. Release steps (recommended)

| Version | Contents |
| --- | --- |
| **0.3.1 Foundation** | CentriForge design system and shell (Launcher, Workspace, theme); Gridcraft engine integrated; new spreadsheet grid (ranges, fill, clipboard, alignment, formula bar, AutoFilter, aggregates); spreadsheet formula columns; fixes from your bug list; Playwright tests in CI |
| **0.3.2 Progress & reporting** | Reporting periods, Update Progress workflow, automatic snapshots, period comparison, Dashboard → Charts → Tables → Notes reports, weekly/monthly templates, PDF and workbook (.xlsx) output |
| **0.3.3 Builder & power** | Report builder with templates, sheet tabs in plans, xlsx with formulas, recurring tasks, relationship diagram, global change, EPS tree, conditional formatting |
| 0.4 | Monte Carlo schedule risk, resource histograms by skill, multi-project resource pool |

Each step is a normal release through the updater, so you can review it before the next one.

## 5. Effort and size (estimates)
- The app grows by about 4–6 MB (Gridcraft crates and IBM Plex fonts).
- 0.3.1 is the largest step: the grid alone is about 1,500 lines.

## 6. Risks
- Gridcraft is young and moves fast (v0.11). Pinning a commit and wrapping it behind one module (`crates/core/src/sheet.rs`) keeps upgrades contained.
- A canvas grid needs care for accessibility and text input (IME). The cell editor stays a real input box laid over the canvas.

## 7. Questions for the user
1. **Ribbon:** replace the ribbon with the CentriForge command bar (cleaner, matches the suite), keep the ribbon, or offer both (ribbon off by default)?
2. **Theme:** add a dark theme with Auto/Light/Dark like CentriForge?
3. **Formula syntax:** spreadsheet functions with field names in brackets (`=[% Complete]-[Baseline % Complete]`), plus A1 references in sheet tabs. OK?
4. **Sheet tabs inside a plan** (free spreadsheets next to the schedule): wanted in 0.3.3, or not needed?
5. **Python:** agree with "no Python in 0.3.x, optional scripting add-on later"? Or do you have a specific Python library or feature in mind?
6. **Reporting periods:** does the week end on Friday or Sunday? Monthly cut-off on the last day of the month or a fixed date (e.g. the 25th)?
7. **Progress measure for S-curves:** duration-weighted, cost-weighted, hours-weighted or physical %, and selectable per plan?
8. **Photos and attachments in notes:** store them inside the .dplan file (larger files) or next to it in a folder?
9. **Steps:** release 0.3.1 → 0.3.2 → 0.3.3 as above (recommended), or one big release?
10. **Your bug and missing-feature write-up and screenshots:** please send them. They become the 0.3.1 acceptance list.
