# Datum Plan 0.3.6: print every view, a complete Network Diagram, timeline zoom, finishing touches

Status: **answered and built as 0.3.6.** Not done, moved to 0.3.7: merging the two grids into one component, the full icon redraw, network layout by WBS.

## 1. User feedback on 0.3.5 (screenshots, 24 Oct 2026)
1. Task tab, Font group: the fill-colour icon is wrong. There is no font choice (Aptos, Times New Roman…, depending on the user).
2. The Task tab now has a lot of empty space. Put the everyday commands there to make the user's work easier.
3. File › Home, "Check again" shows nothing while it checks, so the user can't tell whether it ran.
4. Add a link to the GitHub repository (with the GitHub icon) so users can see the project and its code.
5. The timeline strip should have a zoom box: a box that can be dragged and resized. It zooms the Gantt chart to that part, and the full width shows the whole project.
6. The Network Diagram is unfinished: no legend, no clear meaning of the boxes, no print layout.
7. Do everything planned for 0.3.6: page layout for Network, Calendar, the resource views and Sheets, plus the rest of the 0.3.4 limits. Keep the memory and plan files as a log.

## 2. Planned changes

### A. Font and Task tab
- **Font family** box, listing common Windows fonts: Aptos, Calibri, Arial, Segoe UI, Tahoma, Verdana, Times New Roman, Cambria, Georgia, Garamond, Consolas, IBM Plex Sans. Each name is shown in its own font. If a font is not installed, Windows falls back to a similar one. The choice is stored in the cell style (`n:Font`), shown in the grid and Sheets, and used in printouts.
- **Fill colour:** a proper paint-bucket icon with the colour bar under it. Font colour keeps the "A" with a bar.
- **Task tab, the empty space gets the everyday commands** (they also stay on their own tabs, as Office repeats common ones):
  - **Schedule:** % complete 0–100, Mark on Track, Link and Unlink, Indent and Outdent;
  - **Editing:** Find, Scroll to Task, Fill Down.

### B. File › Home updates card
- "Check again" shows a spinner and "Checking…", then the result with the time ("Checked 18:42: you have the latest version"), or the error.
- A **GitHub** button with the GitHub mark opens the repository in the browser. It is also in Help › About and on the Updates page. A new small shell command `open_url` only allows github.com links.

### C. Timeline zoom box
- The timeline strip above the Gantt chart shows a **box for the part of the project in view**:
  - drag the box to scroll;
  - drag its left or right edge to zoom in or out;
  - double-click the strip to show the whole project;
  - scroll or zoom the Gantt chart and the box follows.
- Today and the status date are marked on the strip.

### D. Network Diagram, completed
- **Box styles by type**, each explained in a **legend** (shown on screen and printed):

  | Task | Box shape and outline |
  | --- | --- |
  | Critical | Red outline |
  | Non-critical | Blue outline |
  | Milestone | Hexagon |
  | Summary | Grey, dashed outline |
  | Completed | Crossed |
  | In progress | Half-shaded (diagonal) |
  | Late | Amber marker |

- **Box fields:** ID, name, start, finish, duration, total float, % complete, resources. Choose them in Format › Box Styles (compact, standard or detailed).
- **Links:** driving links bold, link type and lag labels (SS, FF, +2d).
- **Layout:** by WBS (summaries as bands) or by logic; spacing; show or hide summaries; collapse groups.
- **Zoom:** zoom in and out, zoom to fit, Ctrl+wheel; pan by dragging; a mini-map for large networks; click a box to select the task (the details pane follows); double-click to open Task Information.
- **Highlight the path:** select a task to highlight its predecessors and successors chain.

### E. Page layout for every view (printing)
The same Print page (headers, footers, logos, legend, margins, paper, scaling, page ranges) works for:
- **Network Diagram:** fit to N pages wide or tall, legend, page tiles with overlap marks.
- **Calendar:** month per page or weeks per page, chosen month range.
- **Resource Sheet, Task Usage, Resource Usage, Team Planner:** table + time-phased grid, split across pages like the Gantt.
- **Sheets tabs:** print area (selection or used range), gridlines, row and column headings, fit to width.
- **Reports** (Dashboard, Progress, S-curves, Report Builder, gallery): proper pages with header and footer, charts kept whole, page breaks.
- **Relationship Diagram** and **Timeline:** one page.

### F. The rest of the 0.3.4 limits
- **workbook (.xlsx) export keeps formatting:** bold, italic, underline, strikethrough, font, size, colours, fill and alignment on task rows and sheet cells. Column widths and frozen header kept.
- **One shared grid** for the task sheet and Sheets: same selection, clipboard, fill handle and formula bar.
  - The fill handle **continues series**: numbers 1, 2, 3; dates +1 day or +1 week; "Phase 1", "Phase 2"; Ctrl+drag copies.
  - **Paste Special:** values only, formats only, transpose.
  - **Freeze panes** in Sheets.
- **Icons:** one consistent style for the ribbon and sidebar; replace duplicates (Notes and Details, Columns and Custom Fields).

### G. Quality
- UI tests:
  - font family and fill;
  - the "Check again" state;
  - the timeline box drag and resize changes the zoom;
  - the network legend and box styles;
  - every view gives print pages;
  - an xlsx export with styles (checked with calamine in Rust tests);
  - series fill.
- Docs: CHANGELOG, MEMORY (decisions, files, session log), this plan marked "built".

## 3. Size and order
About 1,800 lines, the largest step since 0.3.1. Build order:
1. A, B, C (small, visible);
2. D (network);
3. E (printing every view);
4. F (xlsx formatting, shared grid, series, Paste Special, icons);
5. G.

## 4. Questions
1. **Fonts:** is the fixed list of common Windows fonts above enough, or do you want every font installed on the PC (Windows asks the user for permission the first time)?
2. **Task tab extras:** are the % complete buttons, Mark on Track, Link and Unlink, Indent and Outdent, Find, Scroll to Task and Fill Down the right ones? Any others?
3. **Network box fields:** are ID, name, start, finish, duration, total float, % complete and resources the right default?
4. **Size:** everything in 0.3.6, or 0.3.6 = A–E (visible items and printing) and 0.3.7 = F (shared grid, series fill, Paste Special, xlsx formatting, icons)?
