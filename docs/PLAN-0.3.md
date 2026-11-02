# Datum Plan 0.3: brainstorm and plan

> Status: **proposal for review**. Nothing here is built yet. Mark it up, then we build it.
> 0.3 merges the old 0.2.1 → 0.5.2 roadmap steps (network diagram included) into one release, as asked on 2026-10-24.

---

## 1. Honest audit of 0.2.0

| Area | What is wrong today |
| --- | --- |
| **Look** | It does not feel like a dense, professional scheduler. Rows are too airy, there are no column dropdown arrows, no indicator or Task Mode columns, and no Format tab. The ribbon is a flat list of buttons. |
| **Ribbon** | Some buttons do nothing useful. "Gantt Chart" should be a dropdown of views. There are no split buttons (Paste ▾, Mode ▾, Task ▾), no Font group and no contextual tabs. |
| **Getting information** | No Task Information dialog, only the bottom pane. No Resource Information or Project Information with statistics. No "Inspect" to explain why a task starts when it does. |
| **Fields** | About 30 fields. The goal is about 400 task fields plus about 200 custom fields (Text1–30, Number1–20, Flag1–20 and more). There is no Field Settings, no Custom Fields dialog, no formulas, lookups, rollups or graphical indicators. |
| **Column header menu** | Missing Cut / Copy / Paste / Paste Special / Wrap Text / Insert Column / Hide Column / Font / Text Styles / Field Settings / Custom Fields / Data Type. |
| **Organising** | No filters, groups, sorting, highlight filters, Show Outline Level, saved tables or saved views. |
| **Formatting** | No fonts, cell or row colours, bar styles, text styles, gridlines, timescale dialog or link styles. |
| **Views** | Missing Tracking/Detail Gantt, Task Usage, Resource Usage, Resource Graph, Network Diagram, Team Planner, Task/Resource forms, and split views. |
| **Reporting** | Two basic pages. No report designer, report gallery, charts, report export or proper period reporting. |
| **Updating progress** | No Update Tasks / Update Project dialogs, multiple baselines, earned value, progress lines or period snapshots. |
| **Timeline** | Basic. It needs Start/Finish labels, a draggable window, several bars, callouts and its own Format tab. |
| **Files** | No Save As type list. Missing PMXML, MPX, PDF, PNG, ICS and HTML. |
| **Navigation** | Too much jumping between places: details are in one spot, settings in another, and views and tabs are not linked. |

---

## 2. Design principles for 0.3

1. **A familiar ribbon layout first, advanced scheduling underneath.** Clear ribbon tabs, dialogs and right-click menus, so schedulers find everything where they expect it.
2. **One click to information.**
   - Double-click a task → Task Information (tabbed).
   - Double-click a resource → Resource Information.
   - Double-click a column header → Field Settings.
   - Double-click a bar → Format Bar.
   - Double-click a link → Task Dependency.
3. **Everything is a view.** View = table (columns) + filter + group + sort + screen (Gantt, Usage, Network…). All views are saved in the plan, with built-ins kept in a global template.
4. **Every ribbon button works.** Dropdowns and split buttons where useful, and a contextual Format tab for the active view.
5. **Engine in Rust.** Fields, formulas, filters, timephased data, leveling, earned value and network layout are all core code with tests. The UI only draws.
6. **Fast with 10,000+ tasks.** The core sends only the visible columns and rows.

---

## 3. Feature catalogue for 0.3

### A. Shell, ribbon and navigation (ribbon layout)
- **Tabs:** File · Task · Resource · Report · Project · View · Help, plus contextual **Format** tabs (Gantt Chart Format, Network Diagram Format, Timeline Format, Usage Format).
- **Task tab**
  - View ▾: the view list
  - Clipboard: Paste ▾, Cut, Copy, Format Painter
  - Font: font, size, bold, italic, underline, fill, colour
  - Schedule: 0–100%, Mark on Track ▾, Respect Links, Inactivate, Indent, Outdent, Link, Unlink
  - Tasks: Manually / Auto Schedule, Inspect, Move ▾, Mode ▾
  - Insert: Task ▾, Summary, Milestone, Deliverable
  - Properties: Information, Notes, Details, Add to Timeline
  - Editing: Scroll to Task, Find ▾, Clear ▾, Fill ▾
- **Resource tab:** Team Planner ▾, Assign Resources, Resource Pool, Add Resources ▾, Information, Notes, Details, Level Selection/Resource/All, Leveling Options, Clear Leveling, Next Overallocation.
- **Report tab:** Dashboards ▾, Resources ▾, Costs ▾, In Progress ▾, Custom ▾, Recent ▾, Compare Projects, Visual/Export.
- **Project tab:** Subproject, Project Information, Custom Fields, Links Between Projects, WBS ▾, Change Working Time, Calculate Project (F9), Set Baseline ▾, Move Project, Status Date, Update Project, Schedule Options, Check Schedule.
- **View tab:**
  - Task Views: Gantt ▾, Task Usage, Network Diagram ▾, Calendar ▾, Other Views ▾
  - Resource Views: Team Planner ▾, Resource Sheet ▾, Resource Usage ▾, Other ▾
  - Data: Sort ▾, Outline ▾, Tables ▾, Highlight ▾, Filter ▾, Group By ▾
  - Zoom: Timescale ▾, Zoom ▾, Entire Project, Selected Tasks
  - Split View: Timeline ☐, Details ☐ ▾
  - Window: New Window, Switch, Arrange
- **Quick Access Toolbar** you can customise, "Tell me what you want to do" (our Ctrl+K), and a **status bar** with view shortcuts (Gantt, Task Usage, Team Planner, Resource Sheet, Reports), the zoom slider and the new-task mode.
- **Left view bar** stays, with a vertical view name such as "GANTT CHART", collapsible.

### B. Grid, fields and custom fields
- **Field catalogue, about 450 fields** grouped by kind:
  - Task: dates, durations, work, cost, progress, baselines 0–10, earned value, constraints, IDs and codes, flags, resources, notes, links…
  - Resource fields and assignment fields.
  - Each field has a type: text, number, duration, date, cost, flag, percentage or list.
- **Custom fields:** Text1–30, Number1–20, Flag1–20, Date1–10, Cost1–10, Duration1–10, Start1–10, Finish1–10, Outline Code1–10, for tasks and resources. The **Custom Fields dialog** (as in your screenshot) has:
  - Rename
  - Lookup table with default value and "restrict to list"
  - **Formula** with a field picker and functions:
    - arithmetic and comparison operators
    - `IIf`, `Switch`, `Choose`, `ProjDateDiff`, `ProjDateAdd`, `DateDiff`, `Now`, `Abs`, `Round`, `Len`, `Left`, `Mid`, `InStr`, `IsNull`, `Format`
  - Rollup for summaries (Max, Min, Sum, Average, Count, or use the formula)
  - **Graphical indicators** (rules → icons/colours, separately for tasks, summaries and the project summary)
- **Column header menu**: Cut, Copy, Paste, Paste Special, Wrap Text, Insert Column, Hide Column, Font, Text Styles, **Field Settings** (title, alignment, width, header wrap), **Custom Fields**, Data Type.
- **Column dropdown arrows (AutoFilter):** sort A→Z / Z→A, group by this field, filter by values, date ranges and custom conditions.
- **Tables** (saved column sets): Entry, Cost, Schedule, Tracking, Variance, Work, Summary, Baseline, Earned Value, Constraint Dates, Delay, plus your own.
- **Indicator column** ("i") with notes, constraints, missed deadlines, overallocation, recurring and hyperlink icons, and tooltips. **Task Mode column** with pin/auto icons.
- **Cell and row formatting:** font, colour, fill, bold and so on. Text Styles by row type (critical, summary, milestone, late…). Format Painter.
- **Paste Special** (values only, or the whole task), Fill ▾ (down, up), Clear ▾ (formats, contents, notes, hyperlinks, all).
- Row height and wrap text, hyperlinks, and a notes editor with bold and lists.

### C. Information dialogs (one click to everything)
- **Task Information:** General · Predecessors (editable grid) · Resources (editable grid) · Advanced (constraint, deadline, task type, effort-driven, calendar, WBS code, earned value method, mark as milestone) · Notes · Custom Fields. Selecting several tasks edits them together.
- **Resource Information:** General (type, max units, availability over time, booking type), Costs (rate tables A–E with dates), Notes, Custom Fields.
- **Project Information:** dates, status date, calendar, priority and **Statistics** (current, baseline and actual start/finish/duration/work/cost, plus variance).
- **Task Dependency:** double-click a link line to change its type or lag, or delete it.
- **Inspect pane:** why a task starts when it does (driving predecessor, constraint, calendar, leveling delay), with a **Task Path** highlight of driving and driven tasks.
- The bottom details pane stays as an option (Details ☐).

### D. Scheduling engine
- **Task types:** Fixed Units, Fixed Work and Fixed Duration, with effort-driven scheduling (duration × units = work).
- **Activity types:**
  - Activity types: task-dependent, resource-dependent, level of effort, WBS summary, start milestone, finish milestone
  - Duration types
  - % complete types: physical, duration, units
  - Steps with weights
- Elapsed durations (`3ed`), % lags (`2FS+50%`), recurring tasks, split tasks (drag to split), and schedule from the finish date.
- **Resource calendars**, availability over time, and rate tables A–E over time. Overtime work and cost.
- **Resource leveling:**
  - Leveling Options dialog: automatic/manual, look-for-overallocation basis, order (ID / Standard / Priority), level only within available slack, allow splitting, priority 0–1000
  - Level Selection / Resource / All, Clear Leveling, Next Overallocation
  - Leveling Gantt showing the delay
- **Advanced scheduling options:**
  - retained logic vs progress override
  - "critical = total float ≤ x" or the longest path
  - free/total float computed from finish or start
  - open-ended tasks warning
- **Longest path** and **multiple float paths**: number of paths, and free-float or total-float based.
- **Move Project, Calculate (F9),** and automatic or manual calculation.

### E. Views
| View | What it shows |
| --- | --- |
| Gantt Chart / Tracking Gantt / Detail Gantt (slack bars) / Leveling Gantt / Multiple Baselines Gantt | Bar styles per type, progress lines, slippage |
| **Task Usage** and **Resource Usage** | Timephased grid per day/week/month: work, actual work, cost, baseline work, overallocation highlighted. Edit hours per period. |
| **Resource Graph** | Histogram of units or work against availability per resource |
| **Network Diagram** | PERT boxes (name, ID, start, finish, duration, resources, % complete; box styles configurable), critical path in red, layout options (top-down / left-right, by WBS), zoom, collapse summaries, drag to link |
| Relationship Diagram | The selected task with its predecessors and successors |
| **Team Planner** | Resources as rows, tasks as bars; drag work between people; an unassigned-tasks lane |
| Calendar | Month, week and day, with bars spanning days |
| Timeline | Labelled, with a draggable window (see F) |
| Task Form / Resource Form / Task Details Form | The bottom split view |
| Activities layout | WBS bands, bottom detail tabs, activity codes |
| WBS Chart | Org-chart of the WBS |
| Sheets | Task Sheet, Resource Sheet |
| Split views | Any view on top of a form or usage view (Details ☐ ▾) |

### F. Timeline (as in your screenshot)
- "Add tasks with dates to the timeline" placeholder, with Start and Finish labels at the ends and Today marked.
- A **pan-and-zoom window** you drag to scroll and resize the Gantt.
- Tasks as **bars** or **callouts**, milestones as diamonds. Drag tasks from the grid onto the timeline.
- Several timeline bars, each with its own date range.
- Timeline Format tab: text styles, date format, show/hide dates, copy the timeline as an image.

### G. Organising: filters, groups, sort, highlight, outline
- **Built-in filters:**
  - All Tasks, Active, Completed, Incomplete, Critical, Late, Slipping, Milestones, Summary
  - Date Range…, Task Range…, Using Resource…, Should Start By…, Tasks With Deadlines
  - Overallocated, Unstarted, In Progress
- **Filter builder:** field / test (equals, contains, greater than, within, is between…) / value or a prompt "?", combined with AND/OR, optionally showing related summary rows.
- **Groups:** by any field, several levels, with intervals for dates and numbers, group header colours, and rollups.
- **Sort:** up to 3 keys, optional renumbering.
- **Highlight filters** instead of hiding rows. **Show Outline Level 1–9**, plus Show/Hide Summary Tasks and Project Summary Task (task 0).

### H. Progress, baselines and earned value
- **11 baselines** (Baseline, Baseline1–10) and **10 interim plans**. Set Baseline dialog: for entire project or selected tasks, roll up options.
- **Update Tasks** dialog (actual start/finish, actual and remaining duration, % complete) and **Update Project** (update work as complete through a date; reschedule incomplete work after a date).
- **Progress lines:** on the status date, at intervals, or at chosen dates.
- **Earned value:** BCWS/PV, BCWP/EV, ACWP/AC, SV, CV, EAC, BAC, VAC, CPI, SPI, TCPI, with physical % complete as the method. Earned-value tables and charts.
- **Period snapshots** (stored period performance): freeze the plan each week or month, with a comment and photos, for trend reports.
- Actual cost and work entry per period in the usage views.

### I. Costs
- Rate tables A–E with effective dates, overtime rates, and cost per use.
- Fixed cost with accrual (start, prorated, end). Project expenses. Cost accounts. Budget resources.
- Cash flow per week, month or quarter, cumulative cost, and budget vs actual.

### J. High-end reporting
- **Report gallery** (Report tab):
  - Dashboards: Project Overview, Burndown, Cost Overview, Work Overview, Upcoming Tasks
  - In Progress: Critical Tasks, Late Tasks, Milestones, Slipping Tasks
  - Resources: Resource Overview, Overallocated Resources
  - Costs: Cash Flow, Cost Overruns, Earned Value Report, Resource and Task Cost Overview
  - **Periodic:** Weekly and Monthly Progress Report (with narrative comments), 3-Week Look-Ahead, Executive One-Pager, Baseline Variance, Schedule Health (DCMA), Period Trend (from snapshots)
- **Report designer** (Report Tools):
  - A page canvas with tables (any fields, filter, group, sort), charts (bar, column, line, area, pie, S-curve, histogram, Gantt snapshot), text boxes with live fields (`[Project Finish]`), images, KPI tiles and page headers/footers
  - Drag to place, plus Format tabs for each element
  - Saved in the plan, and as templates you can share
- **Export** a report to PDF, PNG, Excel (.xlsx, data) and HTML. **Print** with page setup.
- **Visual reports:** pivot-style timephased exports to Excel (.xlsx) files.

### K. Analysis and quality
- **Check Schedule (DCMA 14-point):** missing logic, leads, lags, relationship types, hard constraints, high float, negative float, high duration, invalid dates, resources, missed tasks, critical path test, CPLI, BEI. Each check gives pass/fail with the list of tasks.
- **Schedule log:** open ends, out-of-sequence progress, constraints, loops, and tasks without resources.
- **Compare Projects:** two versions side by side, with changed dates, durations and links.
- **What-if scenarios:** copy the plan, change it, then compare finish and cost.
- **Risks and issues register:** probability, impact and score; owner; linked tasks; response; and a heat map.

### L. Multi-project (enterprise)
- **EPS tree** and **project portfolio** on the Home screen. Each project stays a `.dplan` file, indexed by a workspace file (`.dpws`), so files stay portable.
- **Master project with inserted subprojects**, a shared **resource pool** across projects, and cross-project links.
- **OBS** (who is responsible), **activity codes** (project and global, with values and colours), and **UDFs** (via custom fields).

### M. Formatting and printing
- **Bar Styles** dialog (shape, pattern and colour for each task type and condition; text left, right, top, bottom and inside), Format Bar for one task, Text Styles, Gridlines, Layout (link style, date format, bar height), Timescale (3 tiers, units, labels, count, non-working time shading), and Format Painter.
- **Page Setup:** orientation, scaling (fit to N pages), margins, header, footer and legend with fields and logo, which columns to print, notes, and blank pages.
- **Print preview**, then print or save as **PDF**.
- **Copy Picture:** the current view to PNG or SVG.

### N. Files: open, save and export
| Format | Open | Save | Notes |
| --- | :-: | :-: | --- |
| Datum Plan `.dplan` | ✓ | ✓ | Native |
| Project XML (MSPDI) `.xml` | ✓ | ✓ | Extended with custom fields, multiple baselines, rate tables and timephased data |
| XER `.xer` | ✓ | ✓ | Extended with activity codes, UDFs, expenses and steps |
| **PMXML** `.xml` | ✓ | ✓ | New; detected automatically next to MSPDI |
| **MPX** `.mpx` | ✓ | ✓ | New; older exchange format |
| Excel `.xlsx` / CSV `.csv` | ✓ | ✓ | Choose a table or map columns |
| **PDF** | | ✓ | Print/report output |
| **PNG / SVG** | | ✓ | Copy Picture of a view |
| **iCalendar** `.ics` | | ✓ | Milestones and tasks for Outlook/Google Calendar |
| **HTML** | | ✓ | A self-contained report you can email |
| JSON | ✓ | ✓ | Open data for scripts and integrations |
| `.mpp` | ✗ | ✗ | Closed format (your decision). Save as project XML (MSPDI) instead. |
- **Save As** shows a file-type list. **Open** accepts every format in the table.

### O. Small things that make it feel professional
- Rows 20 px high and a 9 pt font, header dropdown arrows, a green selection border on the active column, and a row-number shade.
- Tooltips on every button with a description.
- Undo history list (Undo ▾), and a scheduling messages bar ("This task has a constraint conflict").
- Remember window, view, column widths and splits per plan.
- A "New Window" for the same plan, and side-by-side views.

---

### P. Datum scheduling style (our own)
Goal: ease of use with scheduling rigour, without common traps. Defaults for every new plan (each one can be changed in Schedule Options):

| Topic | Datum default | Why |
| --- | --- | --- |
| Words | Plain names with the common alternative term in brackets, e.g. "Status date (data date)", "Slack (total float)" | Easy to learn, and the vocabulary is recognisable to every scheduler |
| Task type | **Fixed duration**; adding people changes work, not dates (effort-driven off) | Avoids a common surprise, where adding a resource shortens the task |
| % complete | One **Progress** field. It defaults to duration-based, with a per-task switch to physical or units | A single number for most users, precision when needed |
| Actual dates | Filled in automatically from progress and the status date; editable | No missing-actuals errors |
| Out-of-sequence progress | **Retained logic** | The realistic forecast |
| Critical | Slack ≤ 0, with one click to switch to **Longest path** | Clear for everyone, rigorous on demand |
| Lag calendar | Successor's calendar | Predictable |
| Deadlines vs constraints | A deadline only warns (red marker, negative slack); hard constraints need a confirmation | No silent date locks |
| Calculation | Automatic, with an **Explain** panel ("why this date") and a message bar for conflicts | Never a silent change |
| Entered vs calculated | Calculated cells shown in a softer colour with a ƒ hint; baseline and actual values tagged | The source of every value stays visible |
| New tasks | Auto-scheduled, 1 day, Activity ID auto-numbered **A1000, A1010…** (step and prefix editable) | Clean activity IDs without effort |

### Q. New project workflow (step one for every plan)
- **New project** opens a **Project Setup** panel docked on the right (like the details pane, not a pop-up). You can skip it at any point and come back from Project › Project Setup.
  1. **Project:** name, code, description, client, manager, location, start date (or schedule from finish), priority.
  2. **Calendar and units:** project calendar (Standard / 6-day / 24-hour / custom), hours per day, week start, holidays.
  3. **Currency and costs:** the full ISO 4217 currency list (about 180, searchable, symbol and decimals), default rates.
  4. **Structure:** WBS levels or phases, from a template or empty.
  5. **Resources:** add people, equipment and materials now or later.
  6. **Branding:** company name and **logo upload** for reports and printouts, and header/footer text.
  7. **Finish** → opens the schedule (Gantt) with the WBS ready.
- The default currency, logo and company name are **user preferences** in Options, applied to new projects. Each project can override them.

### R. Branding, currency and safety
- **Logo upload** in Options (company default) and in Project Setup (per project). It is used in report headers, page setup and exports. The image is stored inside the `.dplan` file so it travels with it.
- **Currency** is per project: code, symbol, symbol position and decimals. It is used by every cost field, report and export (MSPDI `CurrencyCode`, XER `currtype`).
- **Beta safety:** before a beta opens a plan saved by an older version, it writes `name.backup.dplan` next to it.

### S. Capacity, look and language
- **Plan size:** smooth editing at 20,000 activities, workable at 100,000 (virtualised grid and canvases, incremental scheduling, visible columns only).
- **Look:** a dense grid (Segoe UI 9 pt, 20 px rows, header dropdown arrows) with Datum's clean light theme.
- **Language:** English only for now. Text is kept in one place so translations can be added later.

### T. Behaviour rules
Every item below is in scope for 0.3:
- Task Information tabs (A1), Field Settings vs Custom Fields (A2/A3), the Lookup, Formula and Graphical Indicators dialogs (A3–A5), Bar Styles (A6), Timescale (A7), Page Setup (A8), Reports gallery and editor (A9), Network Diagram (A10), Team Planner (A11), Resource Usage (A12), view management (A13) and Timeline (A14)
- Advanced scheduling: Activity Details tabs (B1: General, Status, Resources, Predecessors, Successors, Relationships, Codes, Steps, Notebook, Expenses, WPs & Docs, UDFs, Summary; Risks too), Layouts (B2), Activity Codes (B3), Schedule Options including retained logic, progress override, actual dates, longest path and multiple float paths (B4), and Check Schedule with all 19 checks, thresholds and drill-down (B5)

The **acceptance scenarios** become automated tests where the engine can check them, and a manual checklist for each beta.

## 4. Architecture changes

| Change | Why |
| --- | --- |
| **Field registry in Rust** (`fields.rs`): every field's ID, name, type, category, editable flag, getter and setter, for tasks, resources and assignments | One source for the grid, filters, groups, formulas, reports and export. Hundreds of fields without UI code for each. |
| **Formula engine** (`formula.rs`): an expression parser and evaluator using classic formula syntax (`[Field]` references, IIf) | Custom fields, filters and report text. |
| **Query API:** `rows(view)` returns only the visible columns for the visible rows, plus Gantt geometry | Speed with 10k tasks and 450 fields. |
| **View model in the plan:** tables, filters, groups, sorts, views, bar styles, text styles, page setup and reports, stored in `.dplan`. Built-ins live in a "global" template in app config. | One global template for built-ins. |
| **Timephased engine** (`timephase.rs`): per-period work, cost and actuals by contour; leveling and earned value use it | Usage views, graphs, cash flow, EVM. |
| `level.rs` (serial leveling), `evm.rs`, `check.rs` (DCMA/log), `paths.rs` (longest path, float paths), `network.rs` (layered layout) | The new analysis features, all with unit tests. |
| **Workspace** (`.dpws`) for EPS/portfolio, with links to `.dplan` files | Multi-project without a database server. |
| **PDF** through WebView2's native PrintToPdf (no extra library) | Small installer, accurate output. |
| **UI rebuild** (see decision 1 below): components for ribbon, dialogs, grid, canvas views and report designer | Today's string-template UI will not scale to rich dialogs. |

---

## 5. How 0.3 gets built and checked

0.3 is one release, but it is too big to check all at once at the end. So:

- **Beta channel:** an Options switch "Get beta versions". When it is on, the updater also offers pre-releases (`0.3.0-beta.1`, `-beta.2`…). You can try each milestone without waiting, and 0.3.0 final goes to everyone.
- **Milestones** (each one is a beta):

| Beta | Contents |
| --- | --- |
| beta.1 | New UI foundation: ribbon with dropdowns and the Format tab, status bar, dense grid with header dropdowns, indicator and Task Mode columns, field registry (about 450 fields), Field Settings, header menu, Tables, Task/Resource/Project Information dialogs, timeline with labels and a draggable window |
| beta.2 | Custom fields with formulas, lookups, rollups and graphical indicators; filters, groups, sort, highlight, outline levels; Views manager; cell/row formatting and text styles |
| beta.3 | Engine: task types, effort-driven, elapsed and % lags, recurring and split tasks, resource calendars and rate tables, leveling, activity types and advanced options, longest path and float paths, Inspect and Task Path |
| beta.4 | Task/Resource Usage, Resource Graph, Team Planner, Network Diagram, Relationship Diagram, better Calendar, split views, forms |
| beta.5 | 11 baselines and interim plans, Update Tasks and Update Project, progress lines, earned value, costs (fixed, expenses, cost accounts, cash flow), period snapshots |
| beta.6 | Reports: gallery, designer, charts, periodic reports, export to PDF/PNG/Excel (.xlsx)/HTML; Bar Styles, Timescale, Page Setup, print preview, Copy Picture |
| beta.7 | Analysis: Check Schedule (DCMA), schedule log, compare projects, what-if, risks and issues; EPS workspace, master/subprojects, resource pool, activity codes; new file formats (PMXML, MPX, ICS, HTML, JSON) |
| **0.3.0** | Fixes from your beta feedback, performance pass (10k tasks), documentation |

---

## 6. Decisions (answered 2026-10-24)

- **UI technology: SolidJS + Vite** (Astro was considered and rejected: it is a site/content framework for server-rendered pages and would still need a UI library inside it).
- **Multi-project: local files only.** Each project is a `.dplan` file, and a workspace file lists them. Nothing is hosted; everything stays on the user's computer (open source, local-first).
- **Beta channel: yes**, opt-in through the updater.
- **Priority: everything.** Reports, network diagram and earned value matter most. If something slips, it moves to 0.3.1 and is listed there.
- **Feature write-up:** received; its behaviour rules are in scope (section 3.T).
- **Scheduling style:** our own Datum style, simple and rigorous (section 3.P).
- **Currency:** all ISO 4217 currencies; a user preference with a per-project override.
- **New project workflow:** a skippable Project Setup panel docked on the right comes before the schedule (section 3.Q).
- **Activity IDs:** auto-numbered (A1000 + 10). **Network diagram:** boxes with links. **Plan size:** as large as possible (target 20k smooth, 100k workable). **Fonts:** dense, clean style. **Language:** English.
- **Branding:** logo upload and company name, for reports and printing. **Beta backups:** yes.
- **Next:** the user says "yes" to start the 0.3 build (beta.1).

### Original questions

1. **UI technology.** I recommend moving the screen layer from hand-written HTML strings to a small component library, **SolidJS** (about 7 KB), with a bundler. The engine and all logic stay in Rust. This is what lets dialogs, ribbons and the report designer reach professional quality without the code becoming fragile. The alternative is to stay as we are: possible, but slower and buggier at this size.
2. **Multi-project storage:** a workspace file plus `.dplan` files (recommended: portable, works with cloud folders and email), or a single database file.
3. **Beta channel:** OK to ship betas through the updater (opt-in) while 0.3 is being built?
4. **Priorities** if something must wait for 0.3.1: which of reports, network diagram, leveling, earned value or EPS matters most for your daily work?
5. **Screenshots that would help most:**
   - Task Information (all tabs), Field Settings, the Lookup/Formula/Graphical Indicators dialogs, Bar Styles, Timescale, Page Setup, the Report gallery and a report in the designer, the Network Diagram, the Team Planner, Resource Usage, the View ▾ dropdown, the status bar, and Timeline Format.
   - Activities with the bottom tabs, Layout options, Activity Codes, Schedule Options, and Check Schedule.
