# Datum Plan 0.3.5: cleaner ribbon, Setup in the sidebar, column control, full printing

Status: **answered and built as 0.3.5** (tabs are Task, Schedule and Progress; no Home tab). Other views' printing, the shared grid and xlsx formatting moved to 0.3.6.

## 1. User feedback on 0.3.4 (screenshots, 24 Oct 2026)
1. The Project Setup panel takes a big area on the right. Move it to the left sidebar as the first icon ("Setup").
2. The ribbon has a horizontal scrollbar. The Task tab is crowded: group captions are cut off ("CLIPBOARD", "SCHEDULE"), items are missing at the right end, and some icons look poor.
3. The column heading menu has no Delete Column. "Hide Column" exists, but there is no way to unhide.
4. The ribbon should be split into more tabs. It does not have to keep the old layout exactly.
5. Printing shows only Task Name, Duration and a cut-off Start. Printing should include every column of the table (Start, Finish, Predecessors…). The 0.3.4 limits must be done: page layout for the other views, xlsx formatting, and so on.

## 2. Diagnosis
- Print defaulted to "first 3 columns" and skipped the Task Mode column, so only Name, Duration and Start printed. The Start column is drawn under the chart's first weeks (the shading and lines overlap its text).
- "Hide Column" removes the column from the table and keeps no record, so it is really a delete with no undo path in the menu. There is no list of columns to bring back.
- The Task tab has 9 groups (View, Clipboard, Font, Alignment, Schedule, Tasks, Insert, Properties, Editing). At 1920 px it overflows. Nothing collapses, so the ribbon scrolls sideways.
- Project Setup is a fixed 380 px panel over the right side of the work area.

## 3. Proposed changes

### A. Ribbon: more tabs, no sideways scrolling
New tab order (names open to change):

| Tab | Groups |
| --- | --- |
| **Home** (new) | Clipboard (Paste, Cut, Copy, Format Painter) · Font · Alignment · Editing (Find, Replace, Go To, Scroll to Task) · View (Gantt Chart ▾) |
| **Task** | Insert (Task, Summary, Milestone, Recurring, Delete) · Schedule (Link, Unlink, Indent, Outdent, Move Up/Down, Split, Inactivate) · Mode (Manual, Auto) · Properties (Information, Notes, Details, Add to Timeline) |
| **Progress** (new) | % complete buttons, Mark on Track, Update Tasks, Update Progress page · Status date · Baselines (Set, Clear) · Snapshots, Reporting periods · Close period |
| **Resource** | as now |
| **Report** | Dashboard, Progress report, S-curves, Report Builder, Reports gallery · Print, Page Setup, Export |
| **Project** | Project Information, Setup, Calendars, Custom fields, Codes, Schedule options · Critical/Longest path, Checks · Compare |
| **View** | as now (views, data, zoom, split view) |
| **Format** (Gantt Chart Tools) | as now |

- When the window is narrow, groups shrink in steps (as Office does): first the small buttons lose their labels, then a group folds into one button that opens it. **No horizontal scrollbar.**
- Every group caption fully visible. Review all ~80 icons and redraw the poor ones in one consistent style (two-tone, 1.6 px strokes).

### B. Project Setup in the left sidebar
- A new first sidebar icon, **Setup**, opens Project Setup as a page in the work area (same 7 steps), not a panel on top of the plan.
- New plans open on the Setup page when "Show the Project Setup panel" is on. After Done (or Skip) it goes to the Gantt chart.
- Project › Project Information and the ribbon's Setup button open the same page.

### C. Columns: delete, hide, unhide, choose
Heading right-click menu:
- **Delete Column**: takes the column out of this table, with undo. For a formula or custom column it asks: "Remove from this table only" or "Delete the field and its values".
- **Hide Column** keeps the column in the table, hidden. **Unhide Columns ▸** lists the hidden ones (also in View › Tables).
- **Columns…** (new dialog): available fields on the left, table columns on the right, with Add, Remove, Hide, Up, Down, and width. The same dialog is used by printing.

### D. Printing: complete, then better
- **Prints every column of the table by default** (hidden columns stay out), then the chart. If the table is wider than the page, the columns continue on the next page. The column-overlap bug is fixed.
- Page Setup › View: choose the columns to print (tick list, with order), row height and text size, "fit columns to page" (shrinks the text), wrap long task names, print notes on a last page, print blank pages yes or no.
- **Page layout for other views:** Network Diagram, Calendar, Resource Sheet, Resource and Task Usage, Team Planner, Sheets tabs (cell ranges with print area and gridlines), and the reports (Dashboard, Progress, S-curves, Report Builder) with the same headers, footers, logos and page numbers.
- **Print preview:** fit to width, page navigation and a page count while editing settings. The setup can be saved as named page setups per view.

### E. Also from the 0.3.4 limits
- workbook (.xlsx) export keeps cell formatting (bold, colours, fill, alignment, size) for tasks and sheets.
- One shared grid for Sheets and the task sheet. The fill handle continues series (dates +1 day/week, numbers, IDs). Paste Special (values only, formats only). Freeze panes in Sheets.
- Update `docs/MEMORY.md` and add UI tests for each item: no horizontal ribbon scroll at 1280/1600/1920 px, column delete and unhide, every column printed, each view printable.

## 4. Suggested order
1. 0.3.5: B (Setup), C (columns), D-1 (print all columns and the fix), A (ribbon and tabs). These are your blockers.
2. 0.3.6: D-2 (page layout for every other view), E (xlsx formatting, the shared grid).

## 5. Size
About 1,200 lines changed (ribbon and icons about 300, print about 400, columns dialog about 200, Setup page about 100, tests about 150).

## 6. Questions for the user
1. **Tabs:** OK with the new **Home** and **Progress** tabs as in the table above? Any names you prefer?
2. **Setup:** a page in the work area (recommended), or a narrow panel that slides out from the sidebar?
3. **Deleting a formula/custom column:** ask each time (recommended), or always remove from the table only?
4. **Printing:** besides all columns, which print features do you use most (notes, totals, blank pages, specific views)? Which other views must print first?
5. **Split:** 0.3.5 = your blockers (Setup, columns, ribbon, print columns), 0.3.6 = other views' printing + shared grid + xlsx formatting? Or everything in 0.3.5?
