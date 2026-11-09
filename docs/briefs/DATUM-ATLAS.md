# Datum Atlas: product brief

Status: **brief for a new app**. Written at the end of Datum Plan 0.3.8, from everything learned building Datum Plan and Datum Plot. Read [../FAMILY.md](../FAMILY.md) first: Atlas follows it exactly (stack, design, logo rules, releases, docs, workflow).

## 1. What it is

**Datum Atlas** is a desktop app for **materials selection and property charts**. Engineers, designers, students and researchers use it to:
- find the right material for a part;
- understand why a material is right (performance indices, trade-offs);
- compare candidates;
- document the decision.

The scope is the full Ashby-method selection workflow. Atlas aims to be **easy, fast, good-looking and open**:
- your own data, with references for every value;
- spreadsheet-style formulas;
- publication-quality charts;
- a smaller, faster app.

In user-facing text (README, app, website), Atlas stands on its own. Never name other products or use their trademarks, data or wording.

### Names considered
| Name | Why |
| --- | --- |
| **Datum Atlas** (chosen, proposed) | An "atlas of materials": maps (property charts) you navigate to find a material. Short, fits the family. |
| Datum Matter | Materials science feel; less clearly about charts. |
| Datum Select | Says what it does; generic. |
| Datum Alloy | Strong, but sounds metals-only. |
| Datum Element | Nice, but reads as chemistry/periodic table. |

## 2. Core concepts (the Ashby method)

- **Material record:** one material (e.g. "Aluminium 6061-T6") with properties. Each property is a **range** (low–high), a unit, and a source.
- **Families and classes:** a tree, e.g. Metals › Non-ferrous › Aluminium alloys › Wrought › 6061. The other top-level families are Polymers (thermoplastics, thermosets, elastomers), Ceramics and glasses, Composites, Foams, and Natural materials.
- **Property chart (Ashby chart):** two properties on log–log axes, e.g. Young's modulus vs density.
  - Each material is a **bubble**: an ellipse spanning its ranges.
  - Each family is a **hull** (envelope) in a family colour.
- **Selection stages,** combined (a material passes when it passes every active stage):
  1. **Limit stage:** min/max per property ("Max service temperature ≥ 150 °C", "Price ≤ 5 USD/kg"), Yes/No properties (recyclable, food contact), and Excellent/Good/Poor durability ratings.
  2. **Chart stage:**
     - a **box** (drag a rectangle);
     - or a **selection line** of slope n (performance index): drag the line up or across, and materials above (or below) it pass;
     - or a **polygon**.
  3. **Tree stage:** include or exclude families and classes.
- **Performance index:** from the **function** (tie, beam, panel, column, spring, …), the **objective** (minimum mass, cost, energy, CO₂…) and the **constraints**. Examples:
  - E/ρ: light, stiff tie;
  - E^½/ρ: light, stiff beam;
  - E^⅓/ρ: light, stiff panel;
  - σ_f^⅔/ρ: light, strong beam.

  On log–log axes an index is a straight line of slope 1, 2 or 3.
- **Ranking:** materials that pass, sorted by the index (or any property), with a "how much better" factor against the best.

## 3. Features

### 3.1 Browse and search
- A tree of families and classes, plus a table of records with chosen property columns. It uses the shared grid engine (`selection.ts`, `cells.ts`, `fmt.ts` from Datum Plan): sort, filter, select ranges, copy to spreadsheets.
- Search by name, synonym, standard designation (EN, UNS, ASTM, ISO, JIS) and keywords.
- A **datasheet** per material:
  - grouped properties (General, Mechanical, Thermal, Electrical, Optical, Durability, Processing, Eco);
  - ranges with units and source references;
  - notes and typical uses;
  - an image or microstructure (optional).

### 3.2 Charts (the heart of the app)
- **Bubble charts** of any two properties, or of **expressions** (e.g. `[Young's modulus]^0.5/[Density]`), on log or linear axes.
- Bar charts of one property, with material bars sorted by value.
- Ellipses drawn in log space from the property ranges.
- Family hulls computed in log space, smoothed and semi-transparent.
- Labels with collision avoidance (leader lines when moved).
- **Index lines:**
  - drawn with a slope from the index, or typed;
  - dragged to move;
  - guide lines for the classic indices (slopes 1, 2, 3);
  - the value at the line shown.
- Hover shows a tooltip with the ranges; click opens the datasheet; selections highlight across all charts, with failed materials greyed.
- Zoom, pan, fit, logarithmic gridlines, and well-formatted tick labels (10², 10³).
- Style presets: Publication (journal style), Classic (family colours), Monochrome print.
- Export SVG, PNG (300/600 dpi) and PDF; copy to clipboard.

### 3.3 Selection projects
- A project (`.datlas`) saves:
  - stages, charts, index lines and notes;
  - the chosen database and units;
  - the result list.
- **Results:** a ranked table of materials that pass, with which stages each passes or fails and why ("Failed: max service temperature 120 °C < 150 °C"), and the index value and ranking.
- **Compare:** 2–6 materials side by side, with radar (spider) charts of normalised properties and differences highlighted.
- **Report:** one click gives a selection report covering the requirements, stages, charts, the ranked shortlist and the datasheets. Print (family print engine) or a document (.docx) or PDF via print.

### 3.4 Index builder (makes the Ashby method easy)
- A step-by-step guide:
  1. pick the **function** from pictures (tie, beam, panel, plate, column, shaft, spring, pressure vessel, heat exchanger wall, …);
  2. the **objective** (minimise mass, cost, volume, CO₂ footprint);
  3. the **constraints** (stiffness, strength, buckling…);
  4. the **free variable** (thickness, area, …).
- It derives and explains the index (with the equations), creates the chart with the right axes, and draws the index line.
- A library of about 40 standard indices with explanations.

### 3.5 Data: open, referenced, extensible
- **Built-in database (original):** ~250 generic materials to start (families and common grades), then grow toward ~1,000+. Every value is **compiled by us from public sources, with a reference**:
  - public-domain handbooks (e.g. MIL-HDBK-5J / MIL-HDBK-17, NIST data);
  - open datasets with compatible licences (check each: e.g. NIST, the Materials Project for computed properties, CC-BY);
  - manufacturer datasheets (cite them);
  - textbook-level typical ranges, rewritten as our own compilation.

  **Never copy proprietary or commercial databases.**
- **User databases:** add your own materials and properties. Import from workbook (.xlsx) or CSV files with a mapping step (as in Datum Plan's import), and export the same.
- **Derived properties** via formulas (the GridCraft formula engine): specific stiffness, thermal diffusivity, indices, cost per volume.
- **Units:** SI by default, with a metric/imperial switch; per-property unit choice; ranges converted exactly.
- Uncertainty: low/high ranges, and an optional "typical" value. Missing values are shown as gaps, never as zero.

### 3.6 Later (beyond 1.0 or as add-ons)
- **Eco audit:** embodied energy and CO₂ through the life phases (material, manufacture, transport, use, end of life), with a bar chart per phase.
- **Process selection:** process records (casting, moulding, machining, AM…) with size, shape, tolerance, roughness and batch size; process–material compatibility; cost models.
- Substitution suggestions ("materials like this, but cheaper/greener").
- Sharing a database across a team (folder or server).

## 4. Window (family layout)

- **Ribbon tabs:**
  - **Home:** clipboard, find, new chart, new stage, units;
  - **Select:** limit, chart and tree stages, index builder, results;
  - **Chart:** axes, log/linear, expressions, index line, style, labels, export;
  - **Data:** browse, datasheet, add material, import/export, derived properties;
  - **Report:** compare, report, print;
  - **View;**
  - **Help:** Getting Started for Ashby-method newcomers, Glossary (index, hull, bubble, …), GitHub links.
- **Left sidebar:** Setup (new project: application, function, units, database) · Browse · Datasheet · Charts · Stages · Results · Compare · Index builder · Report · Data.
- **Main area:** a chart canvas (whiteboard background as in Datum Plan's network view; white when printed) with a stage panel on the right (each active stage as a card with pass counts).
- File page, Options, printing, the updater and themes are as in Datum Plan.

## 5. Technology and architecture

| Layer | Choice |
| --- | --- |
| Shell | Rust + Tauri 2 (`src-tauri`); Windows first (NSIS, WebView2) |
| Engine | `crates/core` (pure Rust, unit-tested) |
| Window | SolidJS + Vite + strict TS (`ui/src`), the family CSS tokens with the **Violet** accent |
| Charts | Our own SVG chart engine (reuse Datum Plot's plotting code if it exists; otherwise build `ui/src/chart/`); Chart.js only for simple bar/radar charts |
| Formulas | GridCraft crates (pinned git dependency, as in Datum Plan) for expressions and derived properties |
| Workbook (.xlsx) I/O | `calamine` (read) and `rust_xlsxwriter` (write), as in Datum Plan |
| Storage | The built-in database is a compressed JSON (or `bincode`) bundle embedded in the binary; user databases and projects are JSON files (`.datlas`, `.datdb`) |
| Tests | Rust unit tests (units, ranges, index maths, hulls, selection); Playwright UI tests (`tests_ui/run.cjs`) |
| Release | `release.yml` (workflow_dispatch), NSIS installer + SHA256SUMS, in-app updater (`update.rs`, `REPO = Edgar-Mendonca/Datum-Atlas`) |

**Engine modules (`crates/core/src`):**
- `model.rs`: Material, Property (low, high, typical, unit, source), Family tree, Database, Project, Stage, Chart, IndexLine.
- `units.rs`: quantities, unit conversion, SI/imperial sets, formatting.
- `db.rs`: load the built-in and user databases, merge, search (name, synonyms, designations), the tree.
- `expr.rs`: property expressions and derived properties (GridCraft, `[Property]` names as in Datum Plan's formula columns).
- `select.rs`: limit, chart (box/line/polygon in log space) and tree stages; combined pass/fail with reasons; ranking by index.
- `index.rs`: the function/objective/constraint → index table, derivation text, slope for log–log charts.
- `geometry.rs`: ellipses from ranges, convex hulls in log space with smoothing, label placement.
- `io.rs`: workbook (.xlsx) and CSV import with mapping, export, the project file.
- `report.rs`: data for the selection report.

**About Python:** the family rule is **no Python inside the apps**:
- it adds 30–60 MB to every install and update and slows the start;
- it means a second runtime to package and sign.

Everything Atlas needs (log charts, hulls, statistics, .xlsx files) is available in Rust and SVG, and is faster. If you later want custom analysis, add an **optional "Python scripts" add-on** that runs your own Python against an exported dataset, outside the app.

## 6. Data model (outline)

```text
Material { id, name, family_path: [..], synonyms, designations: {EN, UNS, ASTM, ISO, JIS},
           props: { key -> PropValue }, notes, uses, image?, source_refs }
PropValue = Range { lo, hi, typical?, unit, source }  |  Rating(Excellent..Unacceptable)  |  Flag(bool)  |  Text
Property  { key, name, group, unit_kind (stress, density, temperature, …), log_default, description }
Stage     = Limit { rules } | Chart { chart_id, shape: Box|Line{slope, intercept, side}|Polygon } | Tree { include, exclude }
Chart     { x: Expr, y: Expr, log_x, log_y, style, labels, index_lines }
Project   { database_ids, units, stages, charts, notes, results_cache }
```

Starter property set (about 40): density, price, Young's modulus, shear modulus, Poisson's ratio, yield strength, tensile strength, compressive strength, elongation, hardness (Vickers), fracture toughness, fatigue strength, loss coefficient, melting point, glass temperature, max/min service temperature, thermal conductivity, thermal expansion, specific heat, electrical resistivity, dielectric constant, transparency, durability ratings (water, acids, alkalis, UV, flammability), embodied energy, CO₂ footprint, recyclable, and processing ratings.

## 7. Release plan (small numbered releases, as in the family)

| Version | Contents |
| --- | --- |
| **0.1.0** | Shell, updater, Home, family design; ~60 generic materials (all families); Browse (tree + table) and Datasheet; one property chart (log–log bubbles + family hulls), hover and click |
| 0.1.1 | Units (SI/imperial), search, datasheet references, chart export (SVG/PNG/PDF), print |
| 0.2.0 | Selection: limit, chart (box + index line) and tree stages; Results with reasons; projects (`.datlas`) |
| 0.2.1 | Index builder with ~40 indices; expression axes; derived properties (GridCraft) |
| 0.3.0 | Compare and radar charts; the selection report; ~250 materials with references |
| 0.3.x | User databases and workbook/CSV import with mapping; label collision avoidance; style presets |
| 0.4 | Eco audit |
| 0.5 | Process selection |
| 1.0 | ~1,000 materials, user guide, signed installer |

## 8. Quality bar

- Engine tests for:
  - unit conversion round-trips;
  - range maths in log space;
  - index line pass/fail (above or below the line for slopes 1, 2, 3);
  - hulls containing all their bubbles;
  - limit stage edge cases (missing values fail, or are flagged, never pass silently).
- UI tests:
  - chart renders with bubbles and hulls;
  - dragging the index line changes the pass count;
  - the results table matches the stages;
  - every view prints.
- Data checks:
  - every value has a source;
  - low ≤ high;
  - units consistent;
  - no duplicate designations.

## 9. Questions to ask the user before building

1. Name: **Datum Atlas**, or another from the list?
2. Users first: students and teaching (simpler, more explanations), or practising engineers (more grades and designations)?
3. Database: start with generic families (~60–250), and add specific grades later?
4. Must-have properties beyond the starter set (e.g. corrosion in specific media, biocompatibility, RoHS/REACH)?
5. Eco audit and process selection: needed early, or after 1.0?
6. Are there earlier attempts at this app (repos or files) to reuse or learn from?
