# Changelog

## 0.6.0 — 2026-11-07

Modules: registers a plan takes on only when it needs them. The first is Contract Dates & Payments.

- **Project › Modules** ticks the modules a plan uses. A plan without modules looks and saves exactly as before. Switching one off hides it and asks whether to keep its data (kept by default) or delete it.
- **Contract Dates & Payments** (sidebar › Contract, or Project › Contract):
  - **Contract dates**, each met by a task: the task's forecast (or actual) finish against the contract date, days late or early, the baseline, and a status (On track, At risk, Late, Met, Met late). "At risk" is less float than the days set at the top (14 by default).
  - **Trend:** the forecast at every period close, so a slipping date shows as a rising line.
  - **Delay damages:** a % of the value (the unit's price, or the contract value) per week or part week late, after grace days, up to a cap. The page adds up the damages at today's forecast and their cap.
  - **Payment milestones:** a share of the contract value or an amount, due some days after a task finishes, with invoiced and paid dates. A chart shows planned, forecast and received cash.
  - **On a master,** a table adds up the contract dates, damages and payments of its open subplans.
- **Apply Setup to Subplans** can give the subplans the master's modules (each keeps its own registers).

## 0.5.29 — 2026-11-05

Bigger, clearer text on printed Gantt charts.

- **Print › Page Setup › Text** (Gantt, Tracking Gantt, Timeline): the text size in **points on paper**, and the size it really prints at.
- **Fit to pages wide keeps the text size:** long task names wrap onto up to three lines and the chart can narrow to a quarter of the page before the page is shrunk. When it still has to shrink, Print says so and what to change.
- **Columns as wide as their text** (on by default), so the table takes only the room it needs and the chart gets the rest. Turn it off to print columns as wide as on screen.
- **Rows grow with their text**; the timescale, milestone dates and resource labels on the bars grow with it.
- **Sizes by row type:** top summaries, other summaries, tasks and milestones, each in points (empty = the text size).
- **Rows you choose:** select rows in the task sheet, then give them a size in Print › Text. **Clear chosen sizes** takes them back. Rows formatted with Home › Font keep their size relative to the text size.
- Text size and row height moved from Page Setup › View to Text. A master's Apply Setup to Subplans never gives its chosen rows' sizes (they name its own tasks).

## 0.5.28 — 2026-11-05

A master's setup goes to its subplans in one step.

- **Project › Apply Setup to Subplans** (also on the master's tab menu and in **Subplans**): tick what to give and the subplans to give it to, then **Apply**. The parts are project details (client, manager, company, location, logo), logos and pictures, the print header, footer and page setup, calendars and working time, currency, reporting periods, and activity codes and custom fields.
- Each subplan keeps its own **name, code and description**, its print title and subtitle, and its own logos; the master's pictures are added beside them once, and the header points at the subplan's copy.
- The subplans change in their open tabs: save them to keep it, or undo it in any one of them. A note offers **Open All** when some subplans are closed. The ticked parts are remembered.

## 0.5.27 — 2026-11-05

The last open items closed, and Ctrl+K back to Tell Me everywhere.

- **Ctrl+K** opens Tell Me in Sheets too. Add a link with Insert › Link… or right-click › Link….
- **Rules follow their cells:** validation limits, list sources and conditional formats that point at cells move with inserted, deleted and shifted cells and renamed sheets, and show #REF! when their cells go. Conditional formats can compare with a cell (`=$E$1`).
- **Validation by a custom formula** (`=AND(A2>0,A2<=B2)`), checked for each cell, with Stop or Warn and Circle Invalid Data.
- **Names for one sheet:** a defined name's scope is the plan or a single sheet. Sheet names go to and from Excel workbooks as local names, and move with their sheet to another plan.
- **Defined names in task-sheet formula columns:** `=[Cost]*(1+VAT)`, lookups into named sheet ranges and the plan's own figures, worked out per task.
- **Elapsed lags on links to other plans** (FS +7ed) in the Link to Plan dialog, scheduled in calendar time. Links to other plans now show their lags as the task sheet does (FS+5d, +4h).

## 0.5.26 — 2026-11-05

Sheets move between plans, a clearer project tab group, and the open items from 0.5.25 closed.

- **Move a sheet to another plan:** right-click a sheet tab › **Move to Plan**, or **Move or Copy…** › **To plan**. The sheet goes, with its pictures and frozen panes, to another open plan, for example from the master to one of its subplans. Its formulas on plan data then read that plan. The message counts formulas that read other sheets of the first plan.
- **Project tabs:** a project's open plans sit in one framed group with the project's name. The master comes first with a **Master** badge, and its subplans follow with **Sub** badges.
- **Sheets:**
  - **Alt+Enter** for several lines in a cell.
  - **Links:** web and e-mail addresses and `=HYPERLINK()` show as links. Ctrl+click opens them after showing the full address. **Link…** (Ctrl+K in a sheet) adds or edits one.
  - **Defined names** with a **Name Manager**: use them in any sheet formula and in dashboard tiles.
  - **Data Validation:** whole numbers, decimals, dates, times and text length within limits. Each rule has an input message and a stop or warn error, and **Circle Invalid Data** marks cells that break their rules.
  - Inserting or deleting cells, and Remove Duplicates, now also move conditional formatting, filters and validation.
  - All of these go into an exported workbook and come back from one.
- **Scheduling:**
  - **Elapsed lags** such as `4FS+7ed`: calendar time, weekends included, as for curing. They go to and from Project XML.
  - **Usage hours split at the status date:** a started task's work done sits between its actual start and the status date, and the remaining work runs from there to the finish. This applies in Task and Resource Usage, the resource graph, period hours and the cost curve.

## 0.5.25 — 2026-11-05

Sheets that work like a full spreadsheet, a simpler ribbon, and a night of bug fixing across the app (more than 70 fixes, each with a test).

- **Ribbon:**
  - **Home** also has the everyday task commands (New Task, Indent, Outdent, Information, Link, Unlink, Scroll to Task); they are on Schedule too. From Sheets, a task command opens the Gantt.
  - **Sheet Tools** is always on the ribbon, with one tab, **Format**: cells, data, views, page setup and printing. From another view it opens Sheets; Gantt Chart Tools › Format opens the Gantt.
- **Sheets:**
  - A full **right-click menu** on cells, row numbers and column letters: Cut, Copy, Paste, Paste Special, Insert… and Delete… (shift cells or whole rows and columns), Clear, Sort, Filter, Notes, Format Cells…, number format, borders, merge, wrap, rows and columns, freeze panes, AutoSum, functions, plan data and drop-down lists.
  - **Format Cells…** (Ctrl+1): number, alignment, font, border and fill.
  - **Notes** on cells (Shift+F2): a red corner; they show when you point at the cell.
  - **Filter** (Ctrl+Shift+L): values, conditions, top N, above or below average; sort from the heading.
  - **Drop-down lists** in cells, from typed values or from cells.
  - **Remove Duplicates**, **Find All**, Find and Replace on every sheet, **Go To** (F5) and a Name Box you can type in.
  - Keys: Ctrl+; today, Ctrl+Shift+; the time, Ctrl+Space and Shift+Space, Ctrl++ and Ctrl+-.
  - Notes, filters and drop-down lists go into an exported workbook and come back from one.
- **Fixed, in the sheets' calculations** (14):
  - plan dates in sheets no longer carry a hidden time of day, so COUNTIFS on Tasks[Finish], ProjectFinish lookups and date differences give the right answers;
  - dates typed as 05/01/2026 follow the plan's day order, also inside formulas; two-digit years work;
  - a date plus days shows as a date, and a date minus a date shows as days;
  - amounts typed with €, £ or the plan's currency are numbers;
  - money rounds halves up;
  - very deeply nested formulas show #VALUE! instead of closing the app;
  - Save as Excel keeps the same results and formats when the workbook is recalculated;
  - imported formulas follow renamed tabs;
  - the fill handle keeps thousands separators and continues day and month names.
- **Fixed, in dashboards and reports** (18):
  - the S-curve's actual line runs to the status date, and planned and actual are measured on the same day, so a plan on time no longer shows behind;
  - long plans' S-curves reach 100 %, and phase curves run forward;
  - earned schedule after completion, plans of milestones only, and plans with no status date give the right figures;
  - dashboard tiles: large ranges stop at the data, rings show errors and text, pies refuse negative values, tiles on a deleted sheet say so, and dashboards with more than 200 formulas fill completely;
  - the Report Builder prints the report and period on screen, without blank pages;
  - the status marker on cost and earned value charts sits in the right period.
- **Fixed, in the task sheet, Gantt and file exchange** (19):
  - Move Up/Down of a block, remaining and actual durations longer than planned, Fill Down copies instead of counting;
  - progress and scheduling-mode buttons work when a summary is selected; Insert Summary Task and Paste undo in one step;
  - percent lags in the task details; Calendar and Team Planner show collapsed tasks; renaming a calendar keeps people on it;
  - typing a start or finish on a task under way sets its actual dates; a finish typed on a milestone keeps it a milestone; daily recurring tasks skip non-working days;
  - XER: elapsed durations, row order, task calendars and % complete; Project XML: elapsed lags; CSV/Excel import: progress set after the links.

## 0.5.24 — 2026-11-04

A cleaner ribbon, zoom for sheets, and fixes to Page Breaks view.

- **Home tab:** the first tab now holds what the task sheet and the Sheets share: Clipboard, Font, Alignment and Editing (Find, Fill, Clear, Sort). Each tool acts on what is in view, so nothing is repeated.
  - Building the schedule (Insert, Links, Outline, Properties, Tasks, Paths) is on the **Schedule** tab.
  - Sheet Tools › **Cells** keeps only what a sheet has: Lines, Number, Look, Rows & Columns, Insert and Formulas.
  - Sheets opens on Home.
- **Zoom for sheets:** from 10 % to 400 %. Use the status bar slider and its − and +, View › Zoom, or Ctrl with the mouse wheel. Click the percentage to go back to 100 %. Page Layout view follows the zoom.
- **Fixed:**
  - in Page Breaks view, dragging a dashed (automatic) page break onto the print area's edge, or past it, now joins the two pages into one by lowering the sheet's scale. Before, nothing happened;
  - a picture placed in its cell no longer shows a dotted blue outline; the outline shows only while the picture is selected.

## 0.5.23 — 2026-11-04

Set the printed pages of a sheet by hand, in Page Breaks view.

- **Page Breaks view:**
  - the print area has a solid blue edge, and everything outside it is grey;
  - drag the edge to change the print area;
  - drag a page break to move it; dashed lines are automatic breaks, solid lines are breaks you set;
  - drag a break out of the print area to remove it;
  - a page made bigger than the paper lowers the sheet's scale so it fits;
  - right-click a cell for Insert Page Break, Remove Page Break, Reset All Page Breaks, Set Print Area and Reset Print Area.
- **Column page breaks:** Insert Page Break starts a new page above the cell's row and left of its column. Page › Breaks also has a new page at only the row or only the column. Column breaks go into an exported workbook and come back from one.
- **Fixed:**
  - a new picture on a sheet sits inside its cell instead of over the cell's borders;
  - title rows above the print area now print at the top of every page;
  - merges, pictures or formatting far from the cells no longer stretch the print over thousands of empty rows or columns;
  - page breaks stay in the right place when rows or columns are hidden;
  - a very large .xlsb workbook is refused with a message, as .xlsx files already were.

## 0.5.22 — 2026-11-03

The open Sheets items closed, and a hardening pass.

- **Conditional formatting by date:** Highlight Cells › **A Date Occurring…**: before today (overdue), today, yesterday, tomorrow, last or next 7 days, last, this or next week or month, after today. It goes into exported workbooks as a date rule.
- **Hide and unhide rows and columns:** right-click, or Rows & Columns › Size.
  - A double line marks where something is hidden.
  - Arrow keys step over hidden rows and columns.
  - Hidden rows and columns don't print, and stay hidden in Excel, both ways.
- **Fixed, from a hardening pass over Sheets and Excel** (28 new tests):
  - **Bringing in a workbook:**
    - a workbook with cells far apart (A1 and the last cell) could stop the app; it is now refused with a message;
    - text starting with "=" or that looks like a number, date or percentage stays text;
    - formulas from .ods files work;
    - pictures saved by spreadsheet programs come in at their real size.
  - **Sheet names and Save as Excel:**
    - Save as Excel no longer fails on sheet names that are too long, end in an apostrophe, or are called Tasks or DatumPlan;
    - names are fitted for the workbook, and formulas that point to them follow;
    - copies of long sheet names stay within 31 characters.
  - **Conditional formatting:**
    - a rule left without a value, or Top/Bottom with 0 or a huge count, no longer breaks the workbook;
    - text rules with quotes are written correctly;
    - data bars over very large ranges no longer stop the Sheets view, and now print.
  - **Inserting or deleting rows** moves the print area, title rows and pictures as well.
  - **Fill:** filling dates follows the plan's date format (day first or month first) and keeps the cells' look.
  - **Editing:**
    - Ctrl+D on one row copies the row above;
    - Cut and Paste moves merged cells;
    - Replace treats $ in the new text literally.
  - **A round trip** keeps the plan's own page setup for sheets that have none of their own.

## 0.5.21 — 2026-11-03

Conditional formatting, Find and Replace, and more Sheets fixes.

- **Conditional formatting on sheets:** Cells › Look › **Conditional**.
  - **Highlight Cells:** greater than, less than, between, equal to, text that contains, blanks, duplicates.
  - **Top and Bottom:** top 10, bottom 10, above or below average.
  - **Data Bars** and **Colour Scales.**
  - **New Rule…** and **Manage Rules…**: edit, reorder and delete rules; the first matching rule gives a cell its look.
  - Rules print, go into exported workbooks as real conditional formats, and come back when the workbook is brought in again.
- **Find and Replace:** [[Ctrl+F]] and [[Ctrl+H]], or Data › Find: match case, match the whole cell, Replace All.
- **Fill handle:** fills up and to the left as well as down and right; series run backwards.
- **Copy and paste:** a copied range takes its merged cells along.
- **Format Painter:** also brings the number format.
- **Fixed: Sort compared every number as equal;** it now sorts by the numbers themselves.
- The guide covers conditional formatting and Find, in English and Chinese.

## 0.5.20 — 2026-11-03

Sheets and Excel, both ways.

- **Bring an Excel workbook into Sheets:** Sheet Tools › Cells › **Excel Workbook…** opens .xlsx, .xlsm, .xls, .xlsb and .ods files. Every sheet becomes a tab with:
  - values, formulas and number formats;
  - fonts, fills, borders and alignment;
  - merged cells, column widths and row heights;
  - frozen panes, tab colour, page setup and pictures.

  One Undo takes the whole import back.
- **Save the sheets as an Excel workbook to share:** Sheet Tools › Page › **Save as Excel**. The workbook carries:
  - real numbers and dates, formulas with their results, and number formats;
  - styles, merged cells, sizes, pictures, frozen panes, tab colours and page setup.

  Formulas that read the plan keep working, from the plan's data on hidden sheets.
- **Excel export, improved** (also File › Export › Excel workbook):
  - number formats (currency, dates, percentages, Text) are kept;
  - typed dates are real dates;
  - formulas carry their last results, so the values show at once;
  - frozen panes, tab colours and hidden tabs are kept.
- **Country flags survive a round trip:** Excel shows the country's name, and a hidden sheet lets Datum Plan bring the flags back.
- **The ribbon shows Gantt Chart Tools first, then Sheet Tools** (Cells, Page).
- **Sheets:**
  - Backspace on a cell clears it and starts typing there;
  - a leading + or − before a cell or function starts a formula;
  - Increase and Decrease Decimal work on Currency.
- The guide describes sheets and Excel workbooks, in English and Chinese.

## 0.5.19 — 2026-11-03

Sheets that format, print and behave like a spreadsheet.

- **Sheet Tools has two tabs, Cells and Page,** laid out our own way.
  - **Cells** is grouped as Clipboard, Font, Align, Lines, Number, Look, Rows & Columns, Insert and Data.
  - **Look** has a full **cell style gallery**: good, bad and neutral, data and checks, titles and headings, accents at 20, 40, 60 % and full, and number styles.
- **Page: each sheet prints its own way.**
  - **Paper:** size, orientation and margins.
  - **What Prints:** a **print area**, **title rows** on every page, and **page breaks**.
  - **Fit:** to so many pages wide and tall, or a scale.
  - **On the Paper:** **Header & footer on or off** (the cells only, without the plan's logos and title), gridlines, row and column labels, and centre on page.
  - The same choices are at the top of File › Print. The plan's own page setup is unchanged. They also go into an exported .xlsx workbook.
- **Three ways to look at a sheet:** Normal; **Page Layout**, the pages exactly as they will print; and **Page Breaks**, each printed page outlined over the cells with its number. Switch from Sheet Tools › Page or the buttons by the sheet tabs.
- **Place in cell:** a picture on a sheet fits inside its cell, or merged cells, and follows the cell's size. A small button by the selected picture switches it on and off.
- **Fixed in Sheets, from a full review:**
  - **Paste:** it now brings formats and number formats and is one undo step. Cut clears the source only when you paste, and one cell pasted on a larger selection fills it.
  - **Fill and sort:** filling keeps styles and number formats. Sort moves formulas with their rows and keeps formats, with empty cells last.
  - **Dates:**
    - dates follow the plan's date format (05/01/2026 is 5 January when the plan writes the day first);
    - the fill handle continues dates;
    - a formula on a date shows a date;
    - the Text format keeps what was typed (001).
  - **Merged cells:** deleting the first row of a merge shrinks it instead of removing it, and Enter and Tab step past a merge.
  - **Layout changes:** a width, height, merge or page change no longer undoes an edit made a moment before.
  - **Frozen panes:** the cell editor and merged titles show in frozen rows; pictures no longer cover the column letters; the cursor stays in view under frozen panes; renaming a sheet keeps its frozen panes.
  - **Sheet size:** the sheet goes on past row 200 and column Z.
  - **Rows and text:** rows grow to fit a bigger font or wrapped text, and long text runs on over empty cells.
  - **Printing:**
    - printed sheets show country flags;
    - cells that only have borders or fills print too;
    - a sheet prints without row numbers and column letters unless you ask.
  - **Sum** in the status bar counts numbers only.
  - **Sheet names** are checked for characters a workbook cannot use.
- **New keys:**
  - [[Ctrl+Arrow]] to the edge of the data;
  - [[Ctrl+Home]], [[Home]], [[End]] and [[Ctrl+End]];
  - [[Page Up]] and [[Page Down]];
  - [[Ctrl+Enter]] puts what you typed in every selected cell;
  - [[Ctrl+D]] on one cell copies the cell above.
- The guide describes Sheet Tools › Cells and Page, in English and Chinese.

## 0.5.18 — 2026-11-03

Sheets: clearer tools, a proper colour chooser, and plan data in one click.

- **Dropdown arrows are part of their button,** as in a spreadsheet. Before, they were drawn like pane dividers: a grey bar with a resize cursor.
- **Pictures in the menus:** every border choice shows the lines it draws, line styles show their line, and the merge choices have their own icons.
- **Colour palette:** font colour, fill, border line colour and tab colour open a palette under the button, with:
  - theme colours with lighter and darker shades;
  - standard colours;
  - your recent colours;
  - **More Colors…**, a small Colors window with a colour field, a hue bar and hex and RGB boxes.

  The system colour box that opened at the top of the screen is gone.
- **Merged cells keep the sheet's white background** (they turned grey).
- **From the plan,** on the Sheets toolbar: one click puts a live block at the selected cell.
  - **Project facts:** start, finish, status date, baseline finish, % complete, planned %, SPI, CPI and the task counts.
  - **Milestones, Critical, Late, Next 4 weeks, In progress and Resources:** live lists.
  - **Task value…:** one value of one task, such as its finish or % complete.
- The guide describes them, in English and Chinese.

## 0.5.17 — 2026-11-03

Sheets with the tools of a spreadsheet.

- **Sheet Tools › Home:** a ribbon tab that opens by itself in Sheets and goes back to Task when you leave.
  - **Clipboard:** Paste (and Paste Special), Cut, Copy and Format Painter.
  - **Font:** font, size up to 48, bold, italic, underline, strikethrough, colours.
  - **Alignment:** top, middle and bottom; left, centre and right; indent; **Wrap Text**; **Merge & Center**, Merge Across, Merge Cells and Unmerge.
  - **Borders:** bottom, top, left, right, all, outside, thick outside, top and bottom, double and thick lines, no border, with a line colour and style.
  - **Number:** formats, currency, percent, comma style and decimals.
  - **Cell styles:** Good, Bad, Neutral, Calculation, Check Cell, Input, Output, Note, Title, Headings, Total and Accent.
  - **Cells:** insert and delete rows, columns and sheets; row height, column width and AutoFit.
  - **Insert:** Picture (a logo from the plan's pictures at a cell), Function, Plan Data and Freeze Panes.
  - **Editing:** AutoSum, Fill, Clear, and Sort A to Z or Z to A.
- **Row heights:** drag the line under a row number, or use Format › Row Height.
- **Sheet tabs work like a spreadsheet's:**
  - double-click to rename in place;
  - right-click for Insert, Delete, Rename, Move or Copy, Tab Color, Hide and Unhide;
  - drag a tab to move it.
- **No more browser boxes:** renaming, deleting and naming things (sheets, dashboards, reports, calendars, codes, pictures) now use Datum Plan's own dialogs.
- **Merged cells, borders, wrapped text, row heights and tab colours** are saved in the plan, print as shown, and go into exported .xlsx workbooks.
- **Selected cells keep their fill colour** under a light tint, so formatting stays visible while you work.
- **Security:** the spreadsheet reader used for imports is updated; a malformed .xls or .xlsb file could crash the app.
- The guide describes Sheet Tools, in English and Chinese.

## 0.5.16 — 2026-11-02

Logos and pictures in the plan, a fix for Save as PDF, and a full hardening pass.

- **Pictures and logos are saved in the plan.** **Project › Pictures** (or the **Logos** step of Project Setup) keeps the logos of the client, your company, the main contractor, consultants, vendors and sub-vendors, and any other picture, inside the .dplan file.
  - **Print header:** **+ Logo ▾** puts any of them on the left, in the centre or on the right.
  - **Dashboards:** a new **Picture** tile.
  - **Sheets:** **Picture ▾** places one at a cell; drag to move, drag the corner to resize, Delete to remove. It prints with the sheet and goes into an exported .xlsx workbook.
- **Fixed: Save as PDF could give a blank first page** (header and footer only) on prints of two or more pages shrunk to fit the paper. Each printed page is now kept whole, and printing waits for fonts and pictures.
- **Printing, also fixed:**
  - a long table in a report no longer leaves page 1 nearly empty;
  - charts in printed reports (Progress, Dashboard, S-Curves) are drawn;
  - printed reports are on white paper, and side-by-side tables fit the page;
  - "Legend on the last page only" never pushes the footer off the page, and on portrait paper the legend's text stays in its box;
  - the Reports view prints the open report, and the look-ahead prints the weeks you chose.
- **Fixed in the window:** Escape closes a dialog after a click on its title; the tutorial notes no longer cover a dialog's buttons.
- **Fixed in the engine** (each with a test):
  - XER export keeps finish milestones on their day and keeps deadlines;
  - XML and XER export of a master plan keep the dates held by other plans;
  - odd numbers in resource rates and limits can no longer make a file that will not open;
  - two-digit years ("5 Nov 26") read as 2026, and dates stay between 1800 and 2600;
  - recurring tasks and dates at the end of the calendar stay in order;
  - numbers are saved exactly;
  - milestones constrained to their own day no longer show −1 day float;
  - Mark on Track marks unstarted work; Actual Duration can be typed; % Complete above 100 is refused;
  - remaining duration on elapsed-day tasks gives the right %, and split pieces are drawn in order;
  - Activity ID numbering never overflows;
  - sheet formulas accept the task sheet's column names (`Tasks[Task Name]`, `Tasks[Total Slack]`).
- **Faster on big plans:** Update Project, Mark on Track, filling % Complete down, levelling and XER, XML, PMXML and MPX exchange.
- **Public text:** the changelog, docs and code comments describe Datum Plan's own design and name no other products. Product and file-format names are trademarks of their owners and are used only to describe file compatibility.
- The guide has a new section, "Pictures and logos", in English and Chinese.

## 0.5.15 — 2026-11-01

Sharper charts, and any chart in Datum Plot.

- **Charts look sharper on screen and in print.** They are drawn at twice the screen's resolution, and at three times when printing or saving a PDF, so lines and text stay crisp.
- **Easier to read:**
  - each line ends with its latest value (for example 27.6 % actual against 100 % planned);
  - the status line names its date;
  - the bottom axis has a baseline, and the gridlines are lighter.
- **Datum Plot** button on every chart: it opens a copy of the chart's values in Datum Plot, to style the graph for a report or a paper.
  - Charts over time keep their dates; other charts become a grouped table drawn as bars.
  - The copy is saved in Documents › Datum Plan › To Datum Plot.
  - It is a one-way copy: what you change in Datum Plot never changes the plan.
  - If Datum Plot is not installed, the message says where the file is.
- The guide explains the chart buttons, in English and Chinese.

## 0.5.14 — 2026-11-01

Plan tabs where you work.

- **The plan tabs moved below the ribbon,** just above the timeline and the plan, so they sit with the plans they switch between.
- **A tab even for a single plan.** **＋** beside it opens another plan in a new tab: **Blank plan** starts one at once, ready to schedule and link to the other tabs; templates, saved plans, the examples and the tutorial are there too.
- **Fixed: the ribbon's group names (View, Clipboard, Font…) were cut off** on some screens; the ribbon now grows to fit them.
- The guide describes the tabs' new place, in English and Chinese.

## 0.5.13 — 2026-11-01

A guided tutorial, and the last open items from the full check.

- **The tutorial (Help › Tutorial, or the tile on the Home page):** a small office fit-out, five weeks into the work, that uses every part of Datum Plan. A note at the bottom right of each view says how it was filled in and what to try; **Next** and **Back** go through the views in the sidebar. **×** hides the notes and **Help › Tutorial Notes** shows them again. The plan uses:
  - links with lags, a constraint, a deadline, elapsed days, an inactive task and a recurring meeting;
  - activity codes, your own columns with formulas, and conditional formatting;
  - people, a material and a cost, resource calendars with leave, and a six-day week for one task;
  - a baseline, five closed weeks of progress, and a rule of credit with quantities;
  - risks with duration ranges, a constraint log and the week's notes;
  - a sheet that reads the plan, a dashboard of your own, a saved report and a saved filter.
- **Fixed: a cost or material amount above 1,000 was cut to 1,000 when a plan was opened** (a 1,500 fee became 1,000).
- **File exchange:**
  - PMXML keeps links to and from summaries, and percent lags.
  - MPX writes link lags, and names each assignment's resource by ID and unique ID.
  - MPX writes units as 1 = 100 % and the rates in their right fields.
  - XER keeps physical % complete both ways.
  - Very large .xls and .ods sheets are refused with a message instead of exhausting memory.
- **Formulas:**
  - Resource custom fields with a classic formula (`IIf`, `[Field]`) now work out their value for each resource.
  - In a formula column, `Tasks[Cost]` and `Resources[...]` read the whole column.
  - Dates in text show in the plan's date format.
  - `Max()` and `Min()` with nothing in them show an error.
- **Sheets and dashboards:**
  - Dashboard tiles follow a sheet that is renamed, or whose rows or columns are inserted or deleted.
  - An insert that would push cells off the sheet is refused.
- **Scheduling:** a task shared by people on different calendars counts finish-to-finish links and finish constraints back on the right calendar.
- **Schedule risk:** the hit rate of a risk counts only the runs kept.
- The guide explains the tutorial, in English and Chinese.

## 0.5.12 — 2026-10-31

A dashboard that leads with the S-curve, and a full check of every engine: formulas, scheduling, links between plans, progress, file exchange and security.

- **The project dashboard:** the S-curve across the whole page, then the other charts side by side, filling each row with no empty space. A card shows only when the plan has data for it (no costs: no cost cards; no resources: no manpower). **Cards ▾** lets you choose the cards yourself; a card you turn on without data stays blank, also in print. The choice is saved with the plan.
- **Formulas:**
  - A field that reads itself, or two fields that read each other, shows an error instead of closing the app.
  - Fields that read other fields are worked out once, not again and again: a chain of 16 fields over 200 rows went from over a minute to a fraction of a second.
  - Very long or deeply nested formulas are refused instead of crashing.
  - `""` (empty text) works in formulas, as in `IIf([Text1] = "", …)`.
  - Classic formula functions give the standard results:
    - `DateDiff` in weeks and hours, and the quarter ("q") interval;
    - `Sgn(0)` is 0, and `Round` uses banker's rounding;
    - `Mod` and `\` round their operands first;
    - `InStr` counts characters, with a start position;
    - numbers in text keep their digits.
  - A finish field rolled up to a summary shows the right day.
  - A formula column that reads a classic-formula field gets the current value.
  - Sheets show numbers, not dates, for columns G to K of sheets other than Plan.
- **Scheduling:**
  - A phase with a finished task no longer shows zero float and turns critical.
  - Links to other plans use the lag calendar you chose (Schedule Options).
  - Remaining work after a split counts only the pauses still ahead.
  - A started task waits for its unfinished predecessor in another plan (retained logic), as it does for one in the same plan.
  - With several people on a task, it starts when the first of them can.
  - Links to dates before 1970 work.
- **Speed:** scheduling, the network diagram and the checks take linear time (10,000 tasks: a tenth of a second instead of one second, and 20,000 in under 0.2 s). Levelling puts 120 one-day tasks for one person on 120 days in a quarter of a second; before, it gave up after 5 s with the person still over-allocated.
- **Update Links:**
  - It names the links it cannot follow (a deleted task, a file not found) instead of saying all is up to date.
  - It tells apart plans of the same name in different folders.
- **Progress and reports:**
  - Baseline work is kept when the baseline is set, so work variance shows.
  - The planned S-curve reaches 100 % when the baseline ends after the current plan.
  - Without a status date, earned value, the cost curve and planned % are measured on the same day as the progress shown beside them.
  - A summary's % complete is rounded like the project's (67 %, not 66 %).
  - Schedule risk counts risks on milestones and on phases.
- **File exchange:**
  - XER keeps links to and from summaries, percent lags, resource rates and cost resources, and reads calendar hours in either order.
  - Project XML keeps fixed costs, elapsed durations and physical % complete.
  - A damaged XER calendar no longer stops the import.
- **Security and safety:**
  - A plan from someone else can no longer make Windows connect to a network server it names (subplans and links are looked for on your own server or next to the plan).
  - A tiny crafted .xlsx file or a deeply nested XML file can no longer crash the app on import.
  - Report names are escaped in Save as HTML, and updates are downloaded only from this app's own releases.
  - Recovery copies are written before the old ones are removed.

## 0.5.11 — 2026-10-31

Pictures and a full legend in print, subplans that print as you see them, and a checked engine.

- **Pictures in the header, as many as you like:** on the left, in the centre and on the right, side by side, on every page. Each picture has its own size; ← and → move it along the row or to the next place; × takes it out. The header grows to fit them.
- **Fixed: a picture in the centre covered the title and subtitle.** They now sit under the centre pictures. **Title size** sets how large the title prints.
- **A complete legend.** It lists only what the pages show (task, critical task, progress, milestone, critical milestone, summary, subplan, baseline, deadline, link, status date) beside the project's name, start, finish and status date. Print bars show the baseline and the deadline, as on screen.
- **Fixed: expanded subplans printed collapsed.** The **Subplans** choice now sits under What to print; "As shown on screen" prints the subplans you expanded with their tasks.
- **Engine fixes from a full check of formulas, links and progress:**
  - `DateDiff` and `DateAdd` count months and years by the calendar: 30 Nov to 1 Dec is one month, and adding a month keeps the time.
  - A finish date from a formula shows its own day, not the day before.
  - A plan of milestones only reports its % complete.
  - The cost curve's earned line adds up to the earned value, and follows physical % complete when the task uses it.
  - Earned schedule counts working time within a day.
  - A level-of-effort task ends the working day before its successor starts.
  - A start-to-start link from a summary follows the summary's start.
  - Actual dates on a day off stand as recorded.
  - A link to another plan on a summary holds back every task under it.
  - A row that mirrors a task in another plan (start-to-start and finish-to-finish to the same task) takes its length.
  - Update Links settles long hand-back chains between plans.
- The guide explains the header pictures, the legend and printing subplans, in English and Chinese.

## 0.5.10 — 2026-10-31

Printing you can rely on, with a simple header and footer.

- **Save as PDF** writes exactly the pages in the preview to a file you name, at the paper size and orientation you chose, with nothing added by Windows: no date, window title, web address or page count. **Print…** opens the print window for a printer.
- **Fixed: blank or split pages and the browser's own header and footer.** The print window read the page setup after the app had already removed it, so a landscape page came out on portrait sheets, split in two (half blank), with the browser's date, title and address. The page setup now stays in place until the print window has finished.
- **Fixed: the print settings panel was squashed into one line with a sideways scroll bar** (Header, View and every other tab). A style name of the plan tabs clashed with the panel's.
- **Header & footer, simply:** up to three pictures at the top (left, centre, right; your company logo or any picture) with one height for all, a title (the plan's name unless you type another) and a subtitle, and at the bottom only what you tick: printed date (and time), page number, status date, company name, revision and your own text. Nothing else is printed. **Advanced** keeps the code boxes for exact control; plans that used their own header or footer keep it.
- **Master plans print their subplans as on screen** (purple rows, their tasks in grey), or one row each, with all their tasks, or not at all (View › Subplans).
- Page Setup tabs fit on one line; margins read Top, Right, Bottom, Left.
- The guide explains Save as PDF and the header and footer, in English and Chinese.

## 0.5.9 — 2026-10-31

The whole project in the master's Gantt.

- **The combined view.** Under the master's own tasks, each subplan has a row (◇) with its duration, dates and the columns shown, in the order and hierarchy of Subplans, and a purple bar on the Gantt. Subplans are read from their files, open or not.
- **Expand a subplan** (▷) to see its tasks in the master, greyed and read-only.
- **Double-click a subplan's task** (or right-click › Go to this task in its plan) to open the subplan's tab with that task selected, and change it there.
- **Format › Show/Hide › Subplans** turns the combined view on and off.
- The guide explains the combined view, in English and Chinese.

## 0.5.8 — 2026-10-31

A project made of plans: a master plan and its subplans, kept together.

- **The master keeps the project's list** of subplans, in order, with a hierarchy: a subplan can sit under another (two engineering plans under Engineering). Dates still come from the links between the plans; the list arranges them.
- **Project › Subplans:** each subplan with its dates, % complete, number of tasks and whether it is open; ↑ ↓ to reorder, Indent and Outdent for the hierarchy, Show or Open, and × to take a plan out of the project (its file is kept).
- **New Subplan…** makes a plan of the project: a name and its place, saved next to the master with the same calendars and currency, and opened in its tab. **Add Existing…** takes a saved plan into the project. The master is saved straight away when it had no other changes, so the list is not lost.
- **The project's tabs sit together, in one colour:** ◆ marks the master and › its subplans, in the master's order; other plans keep plain tabs. A subplan open without its master shows **Open master** beside the tabs.
- **Opening a master offers to open its subplans.** A tab's right-click menu has Open All Subplans, New Subplan… and **Close Project** (the master and its open subplans, asking to save changes first).
- The linked-plans example is now one project: the main schedule is the master of engineering, procurement and construction.
- The browser's own right-click menu (Back, Reload…) no longer appears in the window, outside text fields.
- The guide explains projects, the master and subplans, in English and Chinese.

## 0.5.7 — 2026-10-31

Clearer links between plans, and fixes.

- **Go to the linked task.** Click the ⇠ or ⇢ mark by a task's ID (or right-click › **Linked Tasks…**): the menu lists the tasks it is linked to, and **Go to** shows the other plan's tab with that task selected, opening the plan from its file when it is not open.
- **See when a link needs updating.** When the other plan has moved, the mark turns amber (⇠!), the label on the Gantt says so, and the plan's tab shows ⟳ with how many tasks wait for new dates. **Update Links** is on the mark's menu and the tab's right-click menu.
- **File › Close and Close All**, and Close / Close All Plans on a tab's right-click menu. New and Open add a tab and leave the others open; now they are easy to close.
- **Link to a plan that is not open:** **Browse…** in Link to Plan picks a saved plan file.
- Fixed: saving one plan deleted the recovery copies of the other open plans with unsaved changes. Each unsaved plan now keeps its copy until it is saved or closed.
- Fixed: Escape did not close menus.
- The guide explains Go to, the update warnings and closing plans, in English and Chinese.

## 0.5.6 — 2026-10-31

See at a glance what is linked to what between plans.

- **Marks by the ID:** ⇠ (purple) on a task that waits for a task in another plan (click it to see or change the link), ⇢ (green) on a task that a task in another open plan waits for. Point at a mark to read the tasks.
- **On the Gantt:** what a task waits for is written before its bar in purple, with a dashed arrow into it ("Construction › A1130 Mechanical completion ⇢ ◆"); which other plan waits for it is written after the bar in green ("→ Construction › A1010 Site mobilisation").
- **Linked Plans** now shows both directions: what this plan waits for, and which tasks of this plan other open plans wait for, with **Show plan** to open that tab.
- The **External Predecessors** column reads like the Gantt: "Engineering › A1180 Final documentation issued (FS)".
- The linked-plans example shows the External Predecessors column beside Finish, and every plan's calendar closes for Christmas and New Year.
- The guide explains how to see what is linked, in English and Chinese.

## 0.5.5 — 2026-10-31

Several plans at once, in tabs, with links between them: a main schedule and its subplans, each in its own file, moving together.

- **Plan tabs.** Every plan you open or create gets a tab above the ribbon; a dot marks unsaved changes, × closes it (asking to save first), **＋** makes a new plan, opens one or opens the example. Ctrl+Tab goes to the next tab, Ctrl+W closes one. Opening a plan that is already open shows its tab. When you close the window, every plan with unsaved changes is listed and can be saved at once, and recovery covers every open plan.
- **Links between plans.** Select a task and choose **Project › Link to Plan** (or right-click › **Link to Plan…**): pick another open plan, find its task, and set the type (FS, SS, FF, SF) and lag. The task waits for it like for a normal link. The **External Predecessors** column shows the links.
- **Linked Plans** (Project tab) lists every link of the plan with the date it last knew and the date now, and marks the ones that have **Moved**. **Update All Open Plans** brings the new dates into every open plan, in the right order, until nothing moves; **Update This Plan** does one. Plans that are not open are read from their files, and links still find each other when the folder is moved. A loop between plans is refused with a message.
- **A detailed example: File › Linked plans (example).** A compressor station in four plans saved to a folder you choose and opened in tabs: a main schedule with its milestones, engineering, procurement (with long-lead equipment and costs) and construction, joined by thirteen links. Engineering and construction use resource calendars (a lead engineer's leave, a six-day site week with a holiday break). Change the manufacture time in procurement and update the links to see the main schedule move.
- The guide has a new chapter on tabs and links between plans, in English and Chinese.

## 0.5.4 — 2026-10-31

Resource calendars that drive task dates: each person's own working week and leave decide when their work gets done.

- **Working Time for each person.** **Resources › Info… › Working Time**: the person's base calendar (the project calendar, or another such as a six-day site week) and their leave or exceptions: annual leave, training, a holiday at home, or mornings or afternoons only.
- **Turn it on in Project › Schedule Options › Resource calendars.** Each assignment then runs on the person's own calendar, and a task finishes when the last person on it finishes. A task with its own calendar works only when both the task and the person work. It is off until you switch it on, so existing plans keep their dates.
- **Ignore Resource Calendar** (a task field, in the details and as a column) keeps a task on its own calendar whoever works on it: a cure time, a delivery.
- Workload, the Resource Graph and levelling count each person's own available hours, so leave shows as less capacity.
- Exchange files keep it: Project XML writes and reads a calendar per person with their leave (and Ignore Resource Calendar); XER writes resource calendars and resource-dependent activities, and reads them back.
- The guide has a new section on resource calendars, in English and Chinese.

## 0.5.3 — 2026-10-30

Your own dashboards: build the board your client or manager wants, inside the plan, always up to date.

- **Several dashboards per plan**, as tabs on the Dashboard page next to the built-in **Project** dashboard. **+ New dashboard** makes a blank one or starts from a template: **Client dashboard**, **Weekly progress** or **Vendors and countries**. Rename, duplicate or delete them from **Dashboard ▾**.
- **Tiles you lay out yourself.** **Edit layout**, then **+ Add tile**; drag tiles to reorder them, and set each one's width (a quarter to full width) and height (one to four rows).
  - **Number:** any formula, as in Sheets (`=SPI`, `=ProjectFinish`, `=COUNTIF(Tasks[Late],TRUE)`, `=Vendors!B4`), with a format, a second line and an optional rule that turns it green or red.
  - **Progress ring:** a percentage as a ring.
  - **Chart:** one of the plan's charts (S-curve, progress per period, progress by phase, cost and earned value, manpower), or a chart of a sheet range or formula: columns, stacked columns, bars, line, area, doughnut or pie.
  - **Table:** a plan list (milestones, late or critical tasks, starting next period, completed this period), or a sheet range or formula; a column of countries shows their flags.
  - **Countries:** flags with how many resources or vendors come from each country.
  - **Text:** a heading or a note.
- Dashboards are saved with the plan, follow every change and print with **Print / PDF**.
- A new name for formulas: `PlannedPercent`, where the baseline says the plan should be at the status date (0–100).
- The guide has a new section on dashboards, in English and Chinese.

## 0.5.2 — 2026-10-30

Better-looking charts that are never cut off.

- **A new look for every chart** (Dashboard, S-Curves, Manpower, Schedule Risk, reports, Report Builder): smooth lines with a soft fill under the actual progress, rounded bars, light horizontal gridlines instead of heavy black axes, round legend markers, a dark rounded tooltip and a status-date label, in a clearer set of colours.
- **No more cropped charts.** Charts follow their box when the window, the sidebar or the panes change size, and they are laid out again for printing, so a printed or PDF dashboard shows every chart whole (two per row).
- Charts in the same row are the same height, with no empty band under the shorter one.
- A chart with nothing to show says so ("No costs in this plan yet", "No hours assigned yet") instead of drawing empty axes.
- Manpower's forecast joins its weeks with straight lines (a smooth curve suggested values between the weeks).
- The guide's pictures show the new charts.

## 0.5.1 — 2026-10-30

Two fixes.

- **Scrolling the schedule no longer sticks.** The task sheet and the Gantt chart scroll together; each one used to echo the other's position back a moment late, so smooth scrolling fought itself and seemed to get stuck (worst near the end of the plan). Now a pane that was moved by the other does not echo it back.
- **Tell Me (Ctrl+K) no longer hides the window.** The search box now opens over the plan with the window dimmed behind it, instead of a blank page.

## 0.5.0 — 2026-10-30

Sheets that work like a spreadsheet and read the plan live: your own reports and vendor lists inside the plan file, with nothing copied by hand.

**Spreadsheet formulas**
- **Click cells into a formula.** Type `=`, then click a cell or drag over several to add them (A1, A1:A5), or click another sheet's tab; each reference gets a coloured box. F4 cycles the `$` anchors, and Enter closes any bracket left open.
- **Suggestions as you type:** all 490 functions, the plan's names and table columns, with Tab to insert. Inside a function, a hint shows its arguments with the current one underlined.
- **Σ AutoSum** (Alt+=; also Average, Count, Max, Min) and **fx Insert Function**, which finds a function by name or by what it does.
- **Fill and paste carry formulas:** the fill handle, Ctrl+D, Ctrl+R and copy and paste move references (`=A1*2` becomes `=A2*2`); Cut moves cells.
- **Insert and delete rows and columns** (right-click, the toolbar, Ctrl+Shift++ and Ctrl+−); formulas on every sheet follow, and references to deleted cells show `#REF!`. Renaming a sheet updates the formulas that name it.
- **Number formats:** spreadsheet format codes (`#,##0.00`, `0.0%`, `dd-mmm-yy`, custom codes), more or fewer decimals, currency and the plan's date format. Results of TODAY, DATE or a lookup of plan dates show as dates by themselves, and plan percentages get a % sign.
- Results that spill into several cells (FILTER, SORT, UNIQUE, SEQUENCE…) are shown. Column widths can be dragged or fitted with a double-click. Pointing at an error explains it.
- Fixed: typing quickly into a cell no longer loses the second key.

**The plan, live, by name**
- `Tasks[column]` has one row per task (Name, Start, Finish, % Complete, baseline dates and cost, float, Critical, Late, Milestone, Resources, earned value, custom fields…); `Resources[column]` has Name, Group, Country, Rate, Work, Cost….
- Names for the plan's numbers: `ProjectFinish`, `BaselineFinish`, `FinishVariance`, `PercentComplete`, `SPI`, `CPI`, `TotalCost`, `TasksLate`, `CriticalCount` and more.
- **Plan Data…** puts a live list at the selected cell: milestones, critical, late, unfinished or in-progress tasks, or those starting in the next four weeks, with the columns, order and number of rows you choose. One formula fills the rows and follows the plan.
- Older formulas such as `=SUM(Plan!M2:M200)` keep working. Excel export adds the Tasks and Resources tables and the names, so the workbook works in other spreadsheet programs too.

**Country flags**
- A **Country with flag** number format shows a country's flag and name: type a code (DE, cn) or a name (Germany, UK).
- Resources have a **Country** (person, crew or vendor), shown with its flag in the Resource Sheet and readable as `Resources[Country]`.

**Also**
- Help › Guide works on computers without a Downloads folder set up (it saves in the home folder's Downloads, Documents or the temporary folder instead).
- The guide's chapter on Sheets is rewritten with two new pictures, in English and Chinese.

## 0.4.4 — 2026-10-29

The Datum Plan Guide: a step-by-step guide with pictures, in English and Simplified Chinese.

**The guide**
- **Help › Guide** (and a **Guide** tile on the File page, with a PDF icon) offers the guide in **English** or **中文**. One click downloads it from GitHub, checks it against the release's checksums, saves it in your Downloads folder and opens it. The app itself stays small.
- About 30 pages with numbered screenshots: installing and updating, the window, building a plan (tasks, durations, links, constraints, calendars), people, man-hours and costs, baseline and progress, reports and printing, risk, the look-ahead and manpower, sheets and formulas, files, the command line, a glossary of planning terms and the keyboard shortcuts.
- The Chinese guide keeps the English names of buttons and menus (the app is in English) with their Chinese meaning beside them, and has an English–Chinese glossary.
- The first time Datum Plan starts, a short hint points to the guide. Tell Me (Ctrl+K) finds it too.

## 0.4.3 — 2026-10-29

- Windows now lists the publisher as Datum.
- An update keeps a custom install folder chosen at the first install (the installer used to remember it under the old publisher name).

## 0.4.2 — 2026-10-29

Stronger and safer, with a command line for scripts and AI assistants, and every report view printable.

**Robustness and security**
- **An internal error no longer closes the app.** The action that failed is stopped with a message, the plan stays as it was, and the details go to `crash.log` in the app's data folder (nothing is sent anywhere).
- **Saving can no longer corrupt a plan.** Plans, autosave copies, exports and settings are written to a temporary file first and then replace the old file in one step, so a crash or a full disk never leaves a half-written file.
- **Damaged or hand-made plan files can no longer crash or freeze the app.** Values far out of range (a duration of a million years, huge lags or delays, a zero-length working day, impossible numbers) are clamped when the plan opens, and the calendar arithmetic has a safe horizon (years 1800 to 2600). Files over 300 MB are refused.
- **Long plans stay quick.** Working time over long spans is counted a week at a time, and daily S-curves of plans longer than ten years use weeks.
- **Updates are stricter.** An update installs only if the release has a checksum file and the download matches it exactly; it downloads into its own new folder, with a size limit, and only installer files with the expected name are accepted.
- **CSV exports cannot run formulas** when opened in a spreadsheet: text starting with `=`, `+`, `-` or `@` gets a leading apostrophe. Calendar (.ics) exports escape line breaks fully.
- A tighter security policy for the window (no frames, objects, form posts or remote scripts), and links open only to this project's pages.
- New checks on every change: every engine function run on deliberately hostile plans and edits, hostile spreadsheet formulas, a security audit of the window's libraries (`npm audit`) next to the existing licence and advisory check.

**Command line and AI assistants**
- **`datum-cli`** (on the Releases page): show a plan's facts or its schedule (also as CSV), change it with named operations, export it to any format, run the schedule risk analysis or the look-ahead, and start plans from templates.
- **Named operations:** every change to a plan is one of the operations the window sends; `datum-cli ops` lists them with an example each.
- **`datum-cli mcp`**: a Model Context Protocol server, so an AI assistant can open or start a plan, read the schedule, change it with the same operations, run the risk analysis and save it. A failed change leaves the plan as it was; nothing is saved unless asked.

**Money**
- **Costs are exact to the cent:** each task's cost and actual cost are kept in whole cents, so totals are the exact sum of what you see. Typed fixed costs are kept to the cent.

**Print and polish**
- **Schedule Risk, Look-ahead and Manpower print as themselves.** Before, printing one of them printed the Dashboard. On paper, Schedule Risk's charts take the full width.
- **The last Schedule Risk run is kept with the plan**, so it is still there after reopening (with the date it was run).

**Fixes**
- Photos in progress reports could not be added: the window's security policy blocked the picture while it was being resized. They work again.
- An empty plan's work showed as "-0 hrs".

## 0.4.1 — 2026-10-28

Progress history: the actual S-curve follows what really happened at each period close, and new tools for the weeks ahead.

**Progress over time**
- **Actual S-curves follow the stored periods.** Each time you close a period (or store a snapshot), the progress of every task is kept. The actual line now runs through those points instead of a straight line from the start to today, and so do the progress report and the dashboard.
- **Earned schedule** on the S-Curves page and the Dashboard: when the plan expected the progress made so far, SPI(t) (schedule performance in time, which stays meaningful near the end of a project), and the finish date at the current pace.
- **Manpower** (Progress › Manpower, or the sidebar): people per week or month for all trades or one trade (a resource's Group), planned against the work done so far and the forecast for the rest.

**Measuring progress**
- **Rules of Credit** (Progress › Rules of Credit): split a kind of work into weighted steps (for example cable: pulled 60 %, terminated 30 %, tested 10 %). There are built-in rules for concrete, steel, piping, cable, equipment, documents and software, and you can make your own. Applying a rule gives the selected tasks those steps; ticking them off sets the task's physical % complete.
- **Installed quantities:** new task columns Quantity, Installed Quantity and Quantity Unit. Installed ÷ planned quantity becomes the task's physical % complete.
- Physical % complete now counts in the S-curves for every task that uses it, not only when progress is weighted by physical.

**Look-ahead** (Progress › Look-ahead, or the sidebar)
- The unfinished work in the next 2, 3, 4 or 6 weeks from the status date, with a bar for each task and whether it is ready to start.
- A **constraint log**: what each task needs before it can start (material, drawings, permits, access, people, equipment), who owns it, the need-by date and whether it is cleared. A task is ready when all its constraints are cleared; overdue ones are flagged.

**Fixes**
- Drop-down lists whose choices load a moment later (the Resource Graph's person or trade, the progress report's period, the constraint log's task) now show the right choice.

## 0.4.0 — 2026-10-28

Risk and resources: see how likely your finish date is, see the workload of each trade, preview levelling, and get a report on every import.

**Schedule Risk** (Report › Schedule Risk, or the sidebar)
- Runs the plan 500 to 5,000 times. In each run every unfinished task takes a duration between its optimistic and pessimistic duration (most often the planned one), and each open risk in the register happens with its probability.
- Shows the chance of finishing by the planned date, the P50, P80 and P90 finish dates, the cost at P50 and P80, a chart of finish dates with the cumulative curve, how often each task was critical, and which tasks drive the finish.
- New task columns **Optimistic Duration** and **Pessimistic Duration**. Tasks without them use a default range (−10 % to +25 %, which you can change).
- The Risk Register has a new **Delay (days)** column: working days a risk adds to its linked tasks when it happens (an opportunity takes them off). Probabilities 1–5 count as 10 %, 30 %, 50 %, 70 % and 90 %.
- The same plan and settings always give the same answer, so you can compare before and after a change.

**Resources**
- Resource Graph › **Trades** adds up everyone in the same Group (their trade or role) against the trade's capacity. **People** shows the load as people instead of hours.
- **Level All** and **Level Selection** first show a preview: which tasks move, the new finish date, and how many overallocated days remain. Nothing changes until you click Level.

**Import report**
- Opening an XML or XER file ends with a short report: anything Datum Plan changed or left out (links to tasks that are not in the file, secondary constraints, elapsed lags), and whether every task gets the same start and finish as in the file, with the first differences listed.
- XER files keep their activity types: level of effort, WBS summary, resource dependent and milestones.

**Fixes**
- Resource Graph bars and labels stay in place when you switch between people, trades and periods.
- A horizontal bar chart now names every bar.

## 0.3.12 — 2026-10-28

Trust in the dates: the scheduling engine is now checked against worked examples, plan files that carry their own dates, and thousands of random plans. The checks found two scheduling mistakes, now fixed.

**Fixes**
- **As Late As Possible** tasks with successors now move only as far as their free float allows, so they never delay the tasks that follow them. Before, they used all their total float and could push past a successor's link.
- **Float before work already under way** is right again. A task's late dates were measured against the whole duration of a successor that had already started, rather than its remaining work, so with retained logic the predecessor could show negative float it did not have.
- Finished tasks no longer limit the float of the tasks before them, and with progress override neither does work already under way.

**Under the hood**
- 47 hand-worked scheduling cases (over 90 task dates checked to the minute): every link type with leads and lags in days, hours and percent, every constraint, deadlines, holidays, part days, task calendars, elapsed durations, splits, summaries, progress with retained logic and override, scheduling from the finish, levelling, critical tasks and man-hours.
- Plan files that carry their own dates (Project XML and XER, in `crates/core/tests/parity`) are opened, rescheduled and compared task by task. Exports from other scheduling tools can be dropped into the same folder.
- Random-plan checks now include constraints, calendars, splits and elapsed tasks: every link must hold, no task may start later than its links need, and the dates must not depend on the row order. Random sequences of edits must never break a plan, and damaged CSV and Excel files must never crash the app.

## 0.3.11 — 2026-10-27

Sturdier: new automatic checks found and fixed two ways a damaged file could stop the app, and two small bugs are gone.

**Fixes**
- Dashboard: without a status date, actual progress is measured at today or at the latest progress you recorded, whichever is later, so a plan updated ahead of time no longer shows 0 %. The Dashboard says which date it used.
- Date picker: it closes as soon as you pick a date.
- Opening a damaged XER file no longer stops the app: a WBS that loops back on itself, or a cut-off calendar, is read as far as it makes sense.
- If anything inside the app ever fails, the rest of the app keeps working (before, one failure could lock every later command).

**Under the hood**
- New automatic checks on every change: a failed edit must leave the plan exactly as it was; 1,000 random plans must schedule without errors and with sensible dates; thousands of damaged XML, XER and plan files must never crash the app; 2,000 tasks must reschedule quickly (about 3 ms in the app).
- Checks on Windows and macOS as well as Linux, and a licence and security check of every library the app uses.
- SECURITY.md and CONTRIBUTING.md.

## 0.3.10 — 2026-10-25

**New**
- **Roadmap** on the File page (a Home tile and a sidebar item): everything Datum Plan will do next and when, with links to suggest a feature or report an issue.

**Fixes**
- Print: "Fit timescale to 1 page wide" no longer adds a nearly empty second page when the table is wide (e.g. with the Man-hours and Cost columns).
- Network Diagram: the cross on completed tasks is lighter, so the text stays readable.
- The Roadmap list is kept in `docs/site.json`, which the Datum family website will read too.

## 0.3.9 — 2026-10-25

A small "?" beside the tools: click it to see what the tool is for and how to use it.

**Help beside every tool**
- Every ribbon group has a small **?** next to its name. A click opens a short card: what the tool does, how to use it, and a tip where one helps.
- The views (Network Diagram, Check Schedule, Risk Register, resources, Portfolio, Update Progress, reports), the details pane, Project Setup and the main dialogs (Task Information, Change Working Time, Schedule Options, Set Baseline, Formula Column, Reporting Periods, Columns, Bar Styles, Conditional Formatting) have one too.
- Help › **What Is This Tool?** lists every card, so you can read about a tool before you look for it.

**Fixes**
- Dialogs with only a Close button (such as these help cards) now close with Escape or Enter straight away.

## 0.3.8 — 2026-10-24

**Fixes**
- Printed Gantt charts now show the links between tasks (Finish-to-Start and the other types), as on screen. Critical links are drawn in red.
- Date picker: every date can now be picked from a calendar.
  - In the task sheet, the selected date cell (Start, Finish, Deadline, Constraint Date, baseline dates…) shows a calendar button.
  - In the details pane, every date field has one.
  - Typing a date still works.

 2026-10-24

A real Tracking Gantt, templates with a team and man-hours, a whiteboard-style Network Diagram, one grid engine and a consistent icon set.

**Tracking Gantt (now different from the Gantt chart)**
- Grey bars show the baseline under each task. Coloured bars show the current dates, solid where the work is done and light where it remains. Each bar shows its % complete.
- Opens with the Tracking table: actual start, actual finish, % complete, physical %, actual and remaining duration, actual cost and work. Going back to the Gantt chart brings the Entry table back.
- A banner explains what you see. When there is no baseline yet, it says so and offers Set Baseline.

**Templates with people, man-hours and costs**
- Every template has its team, on 8-hour days on the Standard calendar, with groups, hourly rates and available units: project managers, engineers, crews, technicians…
- Tasks are assigned to people (e.g. "Design Engineer[200%]"), so man-hours, costs, resource usage, the histogram and overallocation work from the first click.
- Purchases and contracts have fixed costs.
- New Man-hours (Work) and Cost columns in the task sheet.

**Network Diagram**
- A dotted whiteboard canvas on screen; printing stays on white paper.
- Cards: white with a coloured edge (red for critical, blue otherwise) and a soft shadow. Milestones are amber hexagons; summaries are slanted boxes.
- Summary tasks: show the phases as boxes over their first task.
- By WBS: each top-level phase in its own band, with its name.

**One grid engine for the task sheet and Sheets**
- Both views now share one engine for selection, keyboard, clipboard, fill and formatting.
- Sheets gains several ranges with Ctrl+click, whole columns from the column letters, whole rows from the row numbers, Shift to extend, and Sum, Average and Count in the formula bar.
- Clear, number formats and text formatting act on every selected range.

**Icons**
- Redrawn in one line style: the progress, scroll, track, baseline, split, manual, auto, inactive and style icons, and a new Custom Fields icon.

 2026-10-24

Every view prints properly, a complete Network Diagram, a zoom box on the timeline, and the finishing touches from your 0.3.5 review.

**Task tab, fonts and formatting**
- Font list (Aptos, Calibri, Arial, Segoe UI, Tahoma, Verdana, Times New Roman, Cambria, Georgia, Garamond, Consolas, IBM Plex Sans), each shown in its own font. Used in the sheet, Sheets tabs and printouts.
- Fill colour has a proper paint-bucket icon.
- The Task tab now also holds the everyday commands: % complete, Mark on Track, Link, Unlink, Indent, Outdent, Find, Scroll to Task, Fill Down.
- Paste is a split button: Paste, Paste Special… (Ctrl+Alt+V), Paste Values, Paste Formatting, Transpose.

**Fill and paste (task sheet and Sheets)**
- The fill handle continues series:
  - numbers 1, 2, 3 (or any step);
  - "Phase 1" → "Phase 2";
  - dates continue their step.
- Hold Ctrl to copy instead. Durations, links and resources are copied.
- Paste Special: all, values only or formats only, and transpose.
- Sheets tabs: a fill handle (down or right), Ctrl+D, Ctrl+R and Freeze Panes.

**Timeline zoom box**
- The timeline shows the part of the plan in view as a box: drag it to scroll, drag its edges to zoom, double-click for the whole project. It follows the Gantt chart. The status date is marked.

**Network Diagram**
- Box styles with a legend:
  - critical (red), not critical (blue);
  - milestone (hexagon), summary (dashed);
  - completed (crossed), in progress (slashed), late (amber corner).
- Compact, Standard and Detailed boxes. Detailed adds total float, % complete and resources.
- Driving links in bold. Link labels for SS, FF, SF and lags, with a tooltip on every link.
- Zoom, Fit, 100%, Ctrl+wheel, drag to pan, and a mini-map for large networks.
- Highlight path: select a task to light up its whole chain of predecessors and successors.

**Printing for every view (File › Print)**
- Network Diagram: across pages, with its legend. It stays readable and continues on more pages.
- Calendar: a month per page with task bars.
- Resource Sheet, Task Usage, Resource Usage, Team Planner and Sheets tabs: as tables, split down and across with the first column repeated. Sheets print with their cell formatting.
- Dashboard, Progress, S-curves, Report Builder, reports and other pages: as shown, with your header and footer on every page and page numbers.
- All views use the same paper, margins, scaling, headers, footers and logos.

**Excel export keeps formatting**
- Bold, italic, underline, strikethrough, font, size, colours, fill and alignment are kept for tasks and sheet cells, along with sheet column widths.

**Help and links**
- File › Home: "Check again" shows a spinner, then the time it checked. GitHub, Releases and Report an Issue buttons open the project's pages.
- Help tab: Getting Started (for people coming from other scheduling programs or spreadsheets), a Glossary (FS, SS, FF, SF, lag, float, critical and longest path, baseline, status date, SPI/CPI…), GitHub links.
- The link type in the details pane reads "FS · Finish-to-Start", with a ? button that opens the Glossary.

 2026-10-24

A cleaner ribbon, Setup in the sidebar, full control of columns, and printing that includes every column.

**Ribbon**
- The Task tab is split into three: **Task** (clipboard, font, alignment, insert, properties), **Schedule** (links, outline, manual/auto, split, leveling, critical and longest path, find, global change), and **Progress** (% complete, Mark on Track, Update Progress, status date, reporting periods, baselines, snapshots, progress reports).
- No more sideways scrollbar. On a narrow window the ribbon tightens, then shows icons only, so every command stays in view and group names are no longer cut off.
- New icons for Setup and Notes.

**Project Setup**
- Setup is now the first icon in the left sidebar and opens as a page, not a panel over the plan. New plans open on it. Skip or Done takes you to your chosen view.

**Columns**
- Fixed: the Insert Column categories (General, Schedule, Progress…) now open their list of fields. Menus can have any number of levels. "Search all fields…" is at the top.
- Right-click a column heading:
  - Hide Column (the column stays in the table, hidden);
  - Unhide Columns ▸ (lists the hidden ones, or Unhide All);
  - Delete Column (with undo). For a custom or formula column it asks whether to remove it from the table or delete the field and all its values.
- New Columns… dialog (heading menu, View › Data, Format › Columns): all fields on the left; the table's columns on the right with Show, width, order and remove.

**Printing**
- Every column of the table prints by default (Task Name, Duration, Start, Finish, Predecessors, Resources, % Complete…), not just the first three.
- Fixed: the chart no longer overlaps the last table columns.
- A table wider than the page continues on the next page without cutting a column in half. ID and Task Name repeat on every page.
- "Fit timescale to N pages wide" now really fits: it shrinks the page when the table is wide.
- Page Setup › View:
  - choose and order the columns to print;
  - text size and row height;
  - shrink columns to fit;
  - wrap long task names;
  - print task notes on the last pages.

 2026-10-24

Printing and PDF done properly, text formatting (bold, colours, fonts), and a clearer Options page.

**Print and PDF (File › Print, Ctrl+P, or Report › Page Setup)**
- A new Print page: settings on the left, a live preview of every page on the right.
- Print the entire project or specific dates (from and to), and all pages or only some (e.g. 1-3, 5).
- Paper A4, A3, A2, A1, Letter, Legal or Tabloid, landscape or portrait.
- Scaling: adjust to a percentage, or fit the timescale to a number of pages wide.
- Margins in millimetres.
- Header and footer with left, centre and right parts. Codes for page number, total pages, date, project, company, client, manager, status date, project start and finish. The company logo and a client logo can go on either side.
- Legend on every page, the last page or none, with your own text.
- Columns: all of them or the first few, repeated on every page or only on the first. Gridlines, ID column and timescale units are adjustable.
- Pages are drawn as vector graphics, so the Gantt chart and text stay sharp in PDFs. Choose "Save as PDF" in the print window.
- The page setup is saved with the plan.

**Text formatting (Task tab › Font and Alignment)**
- Bold, italic, underline, strikethrough, font size, font colour, fill colour, and left, centre or right alignment for selected cells. Select whole rows to format the row.
- Ctrl+B, Ctrl+I and Ctrl+U, Clear Formatting, and Format Painter (Clipboard group).
- Works the same in the task sheet and in Sheets tabs. The formatting is saved with the plan and printed.

**Options**
- Fixed the Options page, where the labels and boxes had swapped places.
- Grouped into sections, each with an explanation.
- New options: reopen the last plan at start; the view plans open in; manual or auto scheduling for new tasks; the week-end and month-close day for reports; default paper and orientation; choose the workspace folder directly.

 2026-10-24

**Fix (critical):** in 0.3.2, after the first click the task sheet kept the same row selected. Clicking another cell only changed the column, so you could not select, edit or work on other tasks. Clicking now selects the cell you click. An automated test clicks cell after cell and edits one, so this can't come back unnoticed.

## 0.3.2 — 2026-10-24

Progress updating, a report builder and power tools, plus the fixes from your 0.3.1 screenshots.

**Fixes**
- The big Gantt Chart and Task buttons are now split buttons: click the top half to run, the arrow below for the menu.
- Icons on every right-click menu item (column headings and tasks).
- The Format tab sits under its own "Gantt Chart Tools" label, no longer squeezed together.
- The Gantt Chart Style gallery is back on the Format tab (10 styles), with Bar Styles and Gridlines.
- The File page sidebar is blue again (it had turned black).
- The Sheets view no longer fails on plans without sheets.

**New**
- Update Progress (Report › Update Progress or the sidebar): every task due in the period with planned vs actual; enter actual start and finish, % complete, remaining and a note; "Set all to planned %"; Close Period stores a snapshot.
- Report Builder: build your own report from sections (key figures, S-curve, period bars, tables with chosen columns and filters, notes, behind-schedule list). Templates: Weekly management, Monthly client, Schedule health, Cost report. Print to PDF or save as HTML.
- Relationship Diagram (View tab): the selected task with its predecessors and successors; driving links in bold; click a box to follow the chain.
- Recurring tasks (Task › Recurring Task): every n days, weeks or months, for a number of occurrences.
- Global Change (Task tab or a heading's menu): set one field to a value for every selected task or every task in the view, as one undo step.
- Conditional formatting per column: colour cells by rules (font colour, fill and bold), e.g. negative float in red.
- Portfolio shows plans in subfolders as an EPS tree.
- Excel export keeps formulas: formula columns stay live formulas in the workbook, dates and durations are typed, sheet tabs and a Plan sheet are included.
- Automated UI tests run on every build.

## 0.3.1 — 2026-10-24

A rebuilt foundation: a real spreadsheet grid, spreadsheet formulas, period reporting and a cleaner look.

**A task sheet that works like a spreadsheet**
- The task sheet is rebuilt. Header and rows share one scroll area, so columns always line up (fixes the misaligned headings).
- Select cells as in a spreadsheet: click and drag, Shift+click, Ctrl+click for several ranges, click a column heading for the whole column or the ID column for whole rows, Ctrl+A for everything.
- Delete clears the selected cells (Delete on whole rows, or Ctrl+−, deletes tasks). Type and press Ctrl+Enter to fill every selected cell. Ctrl+D fills down, and the fill handle at the corner of a selection drags values down.
- Copy and paste blocks of cells to and from other spreadsheets. Pasting below the last task adds new tasks.
- Formula bar above the sheet shows the active cell, and Sum, Average and Count of the selected numbers.
- Double-click any column heading to rename it. Right-click a task and choose Rename Task, or press F2.
- Filter buttons on every heading: sort, tick the values to show, custom filter.
- The task name and ID columns stay in view when you scroll sideways.

**Formula columns (spreadsheet formulas, one per column)**
- New Formula Column (right-click a heading, the fx button or Format › Formula Column): name it, choose Number, Text, Date, Duration, Cost or Yes/No, and write a formula that refers to other columns by name, e.g. `=[% Complete]-[Baseline % Complete]` or `=IF([Finish]>[Baseline Finish],"Late","On time")`. It is calculated for every task.
- Powered by the GridCraft engine: about 400 spreadsheet functions (IF, IFS, SUMIFS, XLOOKUP, WORKDAY, NETWORKDAYS, TEXT, ROUND, LET…). Formula columns can use each other.
- New fields: Baseline % Complete (where the baseline says a task should be at the status date) and % Complete Variance.

**Sheets: spreadsheets inside your plan**
- Add sheet tabs for quantities, estimates or progress measurement, with spreadsheet formulas, number formats, copy and paste.
- Sheets read your tasks through the Plan sheet, e.g. `=SUMIF(Plan!I:I,100,Plan!M:M)`.

**Progress reporting by period**
- Reporting periods are yours to set per plan: the weekday a week ends on, the day a month closes, and how progress is weighted (duration, cost, hours or physical %). Set them in Project Setup or Project › Reporting Periods.
- Weekly and monthly Progress report for any period: actual and planned progress with the change since last period, progress this period vs planned, variance, forecast finish, SPI and CPI; charts; progress by phase last period vs this period; completed, started, in progress, behind baseline, look-ahead and milestones.
- Notes for each period: executive summary, progress, issues, actions, look-ahead and photos, saved inside the plan and printed with the report.

**S-curves and charts**
- Interactive charts in a clean, scientific style: hover anywhere to read every value, a status-date line, colour-blind-safe colours, and a PNG button on every chart.
- S-curves page: project progress, progress per period, cost and hours S-curves, one S-curve per phase, and the data table; weekly or monthly, weighted as you choose.
- The Dashboard now shows KPIs with the change since last period, the project S-curve, progress per period, progress by phase, cost and earned value, and a manpower histogram.
- Reports saved as HTML keep their charts as pictures.

**Look and feel**
- A calmer design on the familiar ribbon layout: IBM Plex fonts, a calmer palette and square, clean controls.
- Light, Dark and Auto themes (Auto follows Windows). Switch from the title bar or File › Options.

**Fixes**
- Right-click menu commands did nothing in 0.3.0; they work now.
- Selecting a cell sometimes highlighted the wrong column.

## 0.3.0 — 2026-10-24

The biggest release so far: a familiar ribbon layout with advanced scheduling, rebuilt from the engine up. Everything planned for 0.2.1 to 0.5.2 is in this one release, including the network diagram.

**A new window (SolidJS)**
- The ribbon: Task, Resource, Report, Project, View and the Gantt Chart Tools Format tab, with dropdown and split buttons (Gantt Chart, Filter, Group by, Sort, Tables, Outline, Timescale, Baseline, Export).
- Tell Me (Ctrl+K or Alt+Q) searches every command. Quick access toolbar with Save, Undo, Redo and Print.
- A compact task sheet (20 px rows): drag columns to reorder, resize, double-click a heading for Field Settings, right-click a heading to insert, hide, sort, group or filter, and an "Add New Column" picker over about 450 fields.
- Inline editing with choice lists, Tab/Enter navigation, type-to-edit, a blank row for new tasks, indicators for late and manual tasks, and a task mode column.
- Status bar with filter state, task count, project figures, view shortcuts and a zoom slider.

**Fields, custom fields and formulas**
- About 450 task fields: general, schedule, progress, work and cost, 11 baselines (start, finish, duration, cost, work), variances, earned value, and custom Text1–30, Number1–20, Flag1–20, Date, Cost, Duration, Start, Finish and Outline Code fields.
- Custom Fields dialog: rename (alias), lookup tables with restricted values and defaults, classic formulas (`IIf`, `Switch`, `ProjDateDiff`, `DateAdd`, 34 functions, `[Field]` references), summary rollups and graphical indicators.
- Activity codes with values and colours: show as columns, group and filter by them.
- Inspect shows every field of a task.

**Views**
- Gantt Chart and Tracking Gantt with critical path, slack, late bars, baseline 0–10, splits, deadlines, today and status lines, bar labels, and drag to move, resize, link or set progress.
- A timeline across the top.
- Network Diagram (boxes, critical links, progress crosses, milestones).
- Calendar, Task Usage, Resource Usage (days, weeks, months, quarters; work or cost; overallocation in red), Resource Sheet, Resource Graph and Team Planner.
- Tables: Entry, Schedule, Tracking, Cost, Work, Variance, Earned Value, Summary, Baseline and Activities, plus your own.
- Filters (20 built-in plus your own, with and/or conditions, related summaries and highlight mode), groups (several levels, with rolled-up dates and costs) and sorts that keep the outline. Outline levels, expand and collapse, project summary task.

**Advanced scheduling**
- Details pane with tabs: General, Status, Resources, Predecessors, Successors (with driving links and float), Codes, Steps (weighted physical %), Notebook, Expenses, WPs & Docs, Custom Fields, Risks and Notes.
- Activity types (task dependent, resource dependent, level of effort, start/finish milestone, WBS summary) and % complete types (duration, units, physical).
- Schedule options: retained logic or progress override, critical by float or longest path, lag calendar, float paths, schedule from finish, open ends.
- Elapsed durations (`3ed`), percentage lags (`2SS+50%`), task splits, priorities and leveling delay.
- Resource leveling (standard, priority or ID order, within slack) and clearing it.
- Task types: fixed duration, fixed units and fixed work, effort driven.

**Costs, progress and earned value**
- Cost rate tables A–E with dated rate changes, overtime rates, availability periods and resource calendars.
- Fixed costs and expenses with start, prorated or end accrual.
- 11 baselines, Update Project, Store Period Performance (snapshots) and trend reports.
- Earned value per task and project: PV, EV, AC, SV, CV, SPI, CPI, BAC, EAC, VAC, TCPI.
- Currencies: any ISO 4217 code, chosen per plan, with symbol and decimals.

**Reports and dashboards**
- Dashboard: complete, finish, cost, SPI, CPI, late and critical counts, cost S-curve (planned, forecast, earned, actual), progress and cost by phase, milestones and late tasks.
- Report gallery: Project Overview, Weekly/Monthly Progress, Earned Value, Cost Overview with cash flow, Work Overview, Critical Tasks, Late & Slipping, Milestones, Upcoming Tasks and Trend.
- Your logo and company name on every report. Print or save as PDF, save as HTML, save the Gantt chart as a PNG picture.
- Check Schedule: the DCMA 14-point assessment plus further checks (long lags, SF links, soft constraints, manual tasks, summary links, out of sequence, missed deadlines), with your thresholds; click a check to filter its tasks.
- Risk register with probability × impact scoring, a heat map and links to tasks.
- Compare Projects: added, removed and changed tasks against another .dplan, XML or XER file.
- Portfolio: every plan in a folder with its dates, progress, cost, SPI, CPI and late tasks.

**New projects**
- Project Setup panel on the right (skippable): project details, Activity ID numbering (A1000, step 10), calendar, currency, WBS typed as an indented list, resources, and branding.
- Change Working Time, Project Information (with statistics), Resource Information (costs, availability, notes) and Task Information dialogs.

**Files**
- New export formats: PMXML (.xml), MPX, iCalendar (.ics), HTML and JSON. Excel and CSV export the current table's columns.
- JSON import. Plans from 0.2 open as before; a copy is kept as `name.backup.dplan` before they are upgraded.

**Updates**
- Opt in to beta versions in Options.

**Under the hood**
- The engine computes every field, filter, group and sort, so the window stays fast with large plans (20,000 tasks schedule and display in about 0.15 seconds).
- 18 engine tests, including earned value, formulas, leveling, splits, schedule from finish and the schedule checks.

**Coming in 0.3.1:** report designer, Page Setup, bar and text style editors, recurring tasks, Relationship diagram, global change, an EPS tree, and custom fields and codes in XER and project XML files.

## 0.2.0 — 2026-10-24

A big step for the scheduling engine: real calendars, constraints, resources, progress tracking, baselines, file exchange, and a lot more control in the grid and the Gantt chart.

**Scheduling engine (to the minute)**
- Durations in minutes, hours, days, weeks or months (`4h`, `2.5d`, `2w`, `1mo`). Lags take units too (`3SS+4h`).
- Calendars with working hours per day (for example 08:00–12:00 and 13:00–17:00), holidays and special working days. A plan can have several calendars, and each task can use its own.
- All constraints: As Soon/Late As Possible, Start/Finish No Earlier/Later Than, Must Start/Finish On. Deadlines, with negative float when they are missed.
- Manually scheduled tasks, inactive tasks, and links to and from summary tasks.
- A status (data) date: completed work keeps its actual dates and remaining work moves after the status date.
- Actual start and finish, remaining duration, and Mark on Track. Setting % complete fills in the actual dates.
- Milestones sit at their predecessor's finish.

**Resources and costs**
- Resource Sheet: work, material and cost resources, groups, max units, standard rate, cost per use and material labels.
- Assign resources in the Resource Names column (`Engineer[50%], Concrete[12]`) or in the details pane. New names are added to the sheet.
- Work, cost and overallocation are calculated. The Workload view shows hours per week or per day, with over-capacity in red.

**Baselines and tracking**
- Set or clear a baseline for the whole project or for selected tasks.
- Baseline bars in the Gantt chart, baseline columns, start and finish variance, and a baseline curve on the dashboard.

**Grid**
- Show any of about 30 fields: right-click a header or use "Add New Column". You can also hide, move, resize and reset columns, and the layout is saved with the plan.
- Cut, copy and paste tasks (Ctrl+X/C/V). Paste rows straight from a spreadsheet.
- Fill down (Ctrl+D), move rows up and down (Alt+Shift+↑/↓), and Find, Find Next and Replace (Ctrl+F, F3, Ctrl+H).
- Right-click menu for tasks. Indicators for manual scheduling, notes and constraints. Inactive tasks are struck through.

**Gantt chart**
- A timeline strip above the Gantt: add tasks with right-click › Add to Timeline, and click it to jump to a date.
- Drag the progress line inside a bar to set % complete. Dragging one bar onto another creates a link.
- Deadline markers, a status date line, manual task bars, and resource names next to the bars.
- A details pane below the Gantt, with tabs for General, Relationships (edit link type and lag, add or remove predecessors and successors), Resources and Notes.

**Everyday use**
- Ctrl+K searches every command, view and task.
- Ctrl+J quick-adds a task in one line: `Pour slab 5d after 4 @Crew[50%] by 20-Nov-26`.
- Options: date format, and a recovery copy of unsaved work every few minutes. If the app closes unexpectedly, it offers to recover your work.
- The status bar switches new tasks between auto and manual scheduling.
- Ribbon tabs: Task, Resource, Project, View and Help.

**Import and export**
- Import project XML and XER files, including calendars, WBS, relationships, resources, actual dates and baselines.
- Import Excel (.xlsx/.xls) or CSV task lists, with a step to match columns. Predecessors can refer to your own IDs, such as A10.
- Export to Excel (.xlsx), CSV, project XML and XER.

**Fixed**
- Project Information now shows the start date, and dialogs keep what you typed when you press OK.

**Known limits in 0.2**
- No resource leveling yet (0.4.2). Overallocation is shown so you can fix it by hand.
- XER export follows the published file format but has not yet been tested in other scheduling programs. Please try it and report any problem.
- Printing prints the current view. Page setup for the Gantt chart comes later.

## 0.1.1 — 2026-10-24

A lighter look, a view sidebar, templates, a Gantt chart you can drag, and progress reporting.

**New look**
- A light theme with white panels and blue accents in place of the dark navy chrome.
- A new, lighter logo.
- A view sidebar on the left: Gantt, Task List, Calendar, Dashboard and Reports. Network, Resources and Workload show the version they arrive in.

**Templates instead of the sample plan**
- File › Home offers Engineering design, EPC project, Software development, Building construction and Research project, each with a miniature Gantt preview. Templates start next Monday.

**Gantt you can touch**
- Drag a bar to move it. This sets a "start no earlier than" date.
- Drag the end of a bar to change its duration.
- Drag a bar down onto another task to link them (finish-to-start).
- Hover over a bar to see its dates, duration, % complete and float.
- The Finish column can now be edited. Typing a finish date sets the duration.
- The weekly timescale shows day numbers when zoomed out.

**Views and reports**
- Task List: the full-width task sheet.
- Calendar: a month view of starts, finishes and milestones. Click a task to jump to it in the Gantt.
- Dashboard: % complete against planned, finish date, task counts, late and critical tasks, a progress S-curve, phase progress and upcoming milestones.
- Reports: a weekly or monthly progress report. It lists what finished, what is due, what starts, milestones, work in progress, late tasks and a look-ahead to the next period. Print it or save it as PDF.
- Late tasks are shown in red in the task sheet.

## 0.1.0 — 2026-10-24

The first version: the application shell, the scheduling engine and automatic updates.

**Install and update**
- Windows installer for your user account, with no administrator rights needed.
- The Home screen shows your version and checks GitHub for new releases. **Update now** downloads the new version, checks its SHA-256 checksum, installs it and restarts. You never need to uninstall.
- An **Update available** badge appears in the title bar when a new version is out.

**Plan**
- A task sheet: ID, Task Name, Duration, Start, Finish, Predecessors, % Complete and Total Float.
- Type straight into the grid. Typing a name in an empty row adds a task.
- Durations: `5`, `5d`, `2w`, `1mo`. Predecessors: `3`, `3SS+2d`, `5FF-1d`.
- Summary tasks: indent and outdent (Alt+Shift+→/←), with collapse and expand.
- Link and unlink tasks (Ctrl+F2 / Ctrl+Shift+F2) with finish-to-start, start-to-start, finish-to-finish and start-to-finish links and lags.
- Typing a Start date sets a "start no earlier than" constraint.
- Task Information (Shift+F2) and Project Information (name, start date, working days).
- Undo and redo (Ctrl+Z / Ctrl+Y), up to 200 steps.

**Schedule**
- A Rust critical path engine with forward and backward passes, total and free float, a working-day calendar and summary rollups.
- Circular links are refused with a clear message.

**Gantt chart**
- Bars, summary bars, milestones, progress, link arrows, critical path in red, weekends and a today line.
- Two-tier timescale. Zoom with the status bar slider, Ctrl+wheel or View › Entire Project.

**Files**
- Save and open `.dplan` files (Ctrl+S / Ctrl+O). Double-clicking a `.dplan` file opens it.
- Recent plans on the Home screen. If you close with unsaved changes, Datum Plan asks before closing.
- A sample plan: a centrifugal compressor package from kick-off to shipment.

**Known limits in 0.1**
- Durations are whole working days on a single Monday–Friday calendar; hours and holidays arrive in 0.2.
- Summary tasks cannot be linked. Indenting a task under a linked task removes that task's links.
- Excel, CSV, Project XML and XER import and export are planned for 0.3.
