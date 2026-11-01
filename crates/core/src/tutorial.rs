//! The tutorial example: a small office fit-out that uses every part of Datum Plan, five weeks
//! into the work. Each view carries a note (`ui.tour`, shown at the bottom of the window) saying
//! how it was filled in and what to try, so the plan teaches the app as it is opened.
use crate::{doc::{Doc, Op}, model::*, templates::{build, Template}};
use chrono::{Datelike, NaiveDate};
use serde_json::json;

const PLAN: Template = Template { id: "x-tutorial", name: "Tutorial: office fit-out", color: "#2F6FCF", desc: "",
    rows: "0|Design\n1|Kick-off meeting|1d\n1|Survey the space|2d|2\n1|Layout design|5d|3\n1|Client approves the layout|0|4\n\
0|Approvals\n1|Building permit application|3d|5\n1|Permit review by the authority|7d|7\n1|Permit granted|0|8\n\
0|Procurement\n1|Order furniture|2d|5\n1|Furniture manufacture and delivery|15d|11\n1|Order IT equipment|1d|5\n1|IT equipment delivery|10d|13\n\
0|Construction\n1|Strip out|3d|9\n1|Partition walls|5d|16\n1|Electrical first fix|4d|17SS+2d\n1|Ceilings|4d|17,18\n1|Painting|3d|19FS+1d\n1|Flooring|4d|20\n1|Feature wall (option, not ordered)|3d|20\n\
0|Fit-out and handover\n1|Install furniture|3d|21,12\n1|Install IT and network|2d|24SS+1d,14\n1|Snagging and cleaning|2d|24,25\n1|Handover to the client|0|26",
    people: "Project Manager|Management|95|1|PM\nDesigner|Design|70|1|DS\nBuilder|Construction|45|4|BLD\nElectrician|Electrical|60|1|EL\nPainter|Finishes|40|2|PNT\nIT Technician|IT|55|1|IT",
    assign: "2|Project Manager,Designer\n3|Designer,Builder\n4|Designer\n7|Project Manager[50%]\n11|Project Manager[25%]\n13|Project Manager[25%]\n16|Builder[300%]\n17|Builder[200%]\n18|Electrician\n19|Builder[200%]\n20|Painter[200%]\n21|Builder[200%]\n24|Builder[200%]\n25|IT Technician,Electrician[50%]\n26|Builder,Project Manager[50%]",
    costs: "12|18000\n14|9500" };

/// The tutorial plan, started on the Monday five weeks before `today` and updated to the Friday
/// of its fifth week, so it opens in the middle of the work.
pub fn tutorial(today: NaiveDate) -> Project {
    let start = today - chrono::Days::new(35 + today.weekday().num_days_from_monday() as u64);
    let day = |n: u64| start + chrono::Days::new(n);
    let iso = |d: NaiveDate| d.format("%Y-%m-%d").to_string();
    let mut doc = Doc::new(build(&PLAN, start));
    let f = "%Y-%m-%d";
    let run = |doc: &mut Doc, o: Op| doc.apply(o, f).unwrap_or_else(|e| panic!("tutorial: {e}"));
    // Project facts (Setup).
    let p = &mut doc.p;
    (p.code, p.client, p.manager, p.location) = ("FIT-01".into(), "Harbour Accounting".into(), "Project Manager".into(), "Level 3, 12 Quay Street".into());
    p.description = "Fit-out of a 400 m² office: design, permit, new partitions, services, finishes, furniture and IT.".into();
    p.id_prefix = "A".into();
    // Calendars: a public holiday, a six-day week for the painters, the electrician's leave.
    let mut six = Calendar::standard("Six-day week");
    six.week[5] = STD_HOURS.to_vec();
    p.calendars.push(six);
    for c in &mut p.calendars { c.exceptions.push(Exception { name: "Public holiday".into(), from: day(38), to: day(38), hours: vec![] }) }
    p.opts.res_cals = true;
    let countries = ["AE", "GB", "IN", "PH", "PK", "AE"];
    for (r, c) in p.resources.iter_mut().zip(countries) { r.country = c.into() }
    if let Some(r) = p.resources.iter_mut().find(|r| r.name == "Electrician") { r.exceptions.push(Exception { name: "Annual leave".into(), from: day(36), to: day(37), hours: vec![] }) }
    if let Some(r) = p.resources.iter_mut().find(|r| r.name == "Painter") { r.cal = Some("Six-day week".into()) }
    p.resources.push(Resource { uid: p.resources.iter().map(|r| r.uid).max().unwrap_or(0) + 1, name: "Paint".into(), kind: ResKind::Material, label: "litres".into(), rate: 12.0, max: 1.0, group: "Materials".into(), ..Default::default() });
    p.resources.push(Resource { uid: p.resources.iter().map(|r| r.uid).max().unwrap_or(0) + 1, name: "Permit fee".into(), kind: ResKind::Cost, group: "Fees".into(), max: 1.0, ..Default::default() });
    // Activity codes, ranges for the risk analysis.
    p.codes.push(CodeDef { name: "Trade".into(), scope: "Project".into(), values: ["Design", "Approvals", "Procurement", "Civil", "Electrical", "Finishes", "IT"].iter().map(|v| CodeValue { value: v.to_string(), desc: String::new(), color: String::new() }).collect() });
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    let mut set = |row: usize, field: &str, value: &str| run(&mut doc, Op::Set { uid: ids[row - 1], field: field.into(), value: value.into() });
    for (rows, trade) in [(&[2, 3, 4, 5][..], "Design"), (&[7, 8, 9], "Approvals"), (&[11, 12, 13], "Procurement"), (&[16, 17, 19, 21], "Civil"), (&[18], "Electrical"), (&[20, 22, 24, 26, 27], "Finishes"), (&[14, 25], "IT")] {
        for &r in rows { set(r, "code:Trade", trade) }
    }
    set(19, "cal", "Six-day week");
    set(8, "dur", "7ed");
    set(8, "ignore_rc", "Yes");
    set(8, "notes", "Elapsed days: the authority's 7 calendar days run over the weekend.");
    set(11, "ctype", "SNET");
    set(11, "cdate", &iso(day(7)));
    set(11, "notes", "Not before the layout is approved and the budget is released (Start No Earlier Than).");
    set(27, "notes", "The client moves in the week after: the deadline shows a slip as a red arrow and negative float.");
    set(22, "active", "No");
    set(17, "qty", "120");
    set(17, "qtyunit", "m²");
    for (row, lo, hi) in [(12, "12d", "22d"), (14, "8d", "15d"), (17, "4d", "8d"), (4, "4d", "7d"), (8, "8d", "14d")] { set(row, "dmin", lo); set(row, "dmax", hi) }
    for row in [5, 9, 27] { set(row, "timeline", "Yes") }
    set(20, "res", "Painter[200%],Paint[40]");
    set(8, "res", "Permit fee[1500]");
    // The weekly site meeting, a recurring task under its own heading.
    let n = doc.p.tasks.len();
    run(&mut doc, Op::Insert { at: n, name: "Project meetings".into(), ms: false });
    doc.p.tasks[n].level = 0;
    run(&mut doc, Op::Recurring { at: n + 1, name: "Weekly site meeting".into(), dur: "2h".into(), every: "week".into(), interval: 1, count: 8, start: iso(day(4)) });
    for (k, t) in doc.p.tasks[n + 1..].iter_mut().enumerate() { t.level = if k == 0 { 1 } else { 2 } }
    // Custom fields: one VBA-style and one Excel-style formula, both as columns.
    let def = |key: &str, alias: &str, formula: &str| Op::FieldDef { def: CustomDef { key: key.into(), alias: alias.into(), formula: formula.into(), ..CustomDef::default() } };
    run(&mut doc, def("number1", "Days left", "[Duration]*(100-[% Complete])/100"));
    run(&mut doc, def("text1", "Health", r#"=IF(AND([% Complete]<100,[Total Float]<1),"Watch","OK")"#));
    // A rule of credit for the partitions: four steps with weights.
    run(&mut doc, Op::Credit { old: None, rule: Some(CreditRule { name: "Partition walls".into(), steps: vec![("Set out".into(), 10.0), ("Studs and tracks".into(), 40.0), ("Boards".into(), 35.0), ("Joints and finish".into(), 15.0)] }) });
    run(&mut doc, Op::ApplyCredit { uids: vec![ids[16]], rule: "Partition walls".into() });
    // Handover has a deadline a week after its planned date; then the baseline.
    let fin = crate::sched::schedule(&doc.p).map(|s| crate::cal::date_of(s.ef[26])).unwrap_or(day(60));
    run(&mut doc, Op::Set { uid: ids[26], field: "deadline".into(), value: iso(fin + chrono::Days::new(7)) });
    // The baseline, then five weeks of progress, each week closed as a period.
    run(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 });
    for w in 0..5u64 {
        let end = iso(day(7 * w + 4));
        run(&mut doc, Op::UpdateProject { through: end.clone(), reschedule: false });
        run(&mut doc, Op::ProjectSet { field: "status".into(), value: json!(end) });
        if w == 4 {
            // Week 5: furniture and the partitions are behind.
            run(&mut doc, Op::Set { uid: ids[11], field: "pct".into(), value: "35".into() });
            run(&mut doc, Op::Set { uid: ids[16], field: "pct".into(), value: "50".into() });
        }
        if w < 4 { run(&mut doc, Op::Snapshot { label: format!("Week {}", w + 1), comment: String::new() }) }
    }
    if let Some(t) = doc.p.tasks.iter_mut().find(|t| t.uid == ids[16]) { for s in t.steps.iter_mut().take(2) { s.done = true } t.phys = 50; t.qty_done = 60.0 }
    run(&mut doc, Op::Snapshot { label: "Week 5".into(), comment: "Furniture and partitions behind.".into() });
    // Risks, the constraint log and the week's notes.
    for o in [
        Op::Risk { id: 0, risk: Some(Risk { name: "Furniture delivered late".into(), cause: "Single overseas supplier".into(), prob: 3, impact: 4, owner: "Project Manager".into(), response: "Weekly call with the supplier; keep a local back-up quote".into(), status: "Open".into(), tasks: vec![ids[11]], kind: "Threat".into(), days: 7.0, ..Default::default() }) },
        Op::Risk { id: 0, risk: Some(Risk { name: "Asbestos found during strip out".into(), cause: "Building from the 1970s".into(), prob: 2, impact: 5, owner: "Builder".into(), response: "Survey before strip out".into(), status: "Open".into(), tasks: vec![ids[15]], kind: "Threat".into(), days: 5.0, ..Default::default() }) },
        Op::Log { id: 0, item: Some(LogItem { uid: ids[17], kind: "Drawing or information".into(), text: "Socket positions from the client".into(), owner: "Client".into(), need: Some(day(30)), status: "Open".into(), ..Default::default() }) },
        Op::Log { id: 0, item: Some(LogItem { uid: ids[19], kind: "Material".into(), text: "Paint colours approved".into(), owner: "Designer".into(), need: Some(day(36)), status: "Open".into(), ..Default::default() }) },
        Op::Note { note: PeriodNote { kind: "week".into(), end: Some(day(32)), summary: "Design and approvals are done; strip out finished and the partitions have started.".into(), progress: "Permit granted. Strip out complete. Partition walls half done (set out, studs and tracks).".into(), issues: "Furniture supplier has not confirmed the delivery date.".into(), actions: "PM to call the supplier on Monday. Client to give socket positions by Wednesday.".into(), look_ahead: "Partitions, electrical first fix and the start of ceilings.".into(), photos: vec![] } },
    ] { run(&mut doc, o) }
    // A sheet that reads the plan by name, and a budget by trade.
    let cells = [("A1", "Weekly report"), ("A3", "Forecast finish"), ("B3", "=ProjectFinish"), ("A4", "Baseline finish"), ("B4", "=BaselineFinish"), ("A5", "% complete"), ("B5", "=PercentComplete"), ("A6", "Planned %"), ("B6", "=PlannedPercent"),
        ("A7", "Late tasks"), ("B7", "=COUNTIF(Tasks[Late],TRUE)"), ("A8", "SPI"), ("B8", "=SPI"), ("A10", "Milestone"), ("B10", "Finish"), ("C10", "% Complete"), ("A11", "=FILTER(CHOOSECOLS(Tasks,3,8,9),Tasks[Milestone]=TRUE,\"None\")"),
        ("E3", "Item"), ("F3", "Budget"), ("G3", "Spent"), ("E4", "Labour and other"), ("F4", "=SUM(Tasks[Baseline Cost])-F5-F6"), ("G4", "=SUM(Tasks[Actual Cost])-G5-G6"), ("E5", "Furniture"), ("F5", "18000"), ("G5", "0"), ("E6", "IT equipment"), ("F6", "9500"), ("G6", "0"),
        ("E7", "Total"), ("F7", "=SUM(F4:F6)"), ("G7", "=SUM(G4:G6)")];
    let mut fmt: Vec<(String, String)> = [("B8", "0.00"), ("B5", "0\"%\""), ("B6", "0\"%\"")].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    for r in 4..8 { fmt.push((format!("F{r}"), "#,##0".into())); fmt.push((format!("G{r}"), "#,##0".into())) }
    for r in 11..16 { fmt.push((format!("B{r}"), "date".into())); fmt.push((format!("C{r}"), "0\"%\"".into())) }
    let style = ["A1", "A10", "B10", "C10", "E3", "F3", "G3", "E7", "F7", "G7"].iter().map(|a| (a.to_string(), "b".to_string())).collect();
    doc.p.sheets.push(SheetData { name: "Weekly report".into(), cells: cells.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), fmt: fmt.into_iter().collect(), widths: [(0, 160), (4, 130)].into_iter().collect(), style });
    // Views the user can come back to: a dashboard, a saved report, a filter, a highlight.
    let tile = |kind: &str, title: &str, w: u8, h: u8, extra: serde_json::Value| { let mut t = json!({ "id": title, "kind": kind, "title": title, "w": w, "h": h }); t.as_object_mut().into_iter().for_each(|o| o.extend(extra.as_object().cloned().unwrap_or_default())); t };
    let ui = json!({
        "dashboards": [{ "name": "Client dashboard", "tiles": [
            tile("number", "Actual progress", 3, 1, json!({ "formula": "=PercentComplete", "sub": "=\"Planned \"&TEXT(PlannedPercent,\"0\")&\"%\"" })),
            tile("number", "Forecast finish", 3, 1, json!({ "formula": "=ProjectFinish", "sub": "=\"Baseline \"&TEXT(BaselineFinish,\"dd-mmm-yy\")" })),
            tile("number", "SPI", 3, 1, json!({ "formula": "=SPI", "fmt": "0.00", "good": [">=", 1], "sub": "Schedule performance index" })),
            tile("number", "Late tasks", 3, 1, json!({ "formula": "=COUNTIF(Tasks[Late],TRUE)", "good": ["<=", 0], "sub": "=\"of \"&TaskCount&\" tasks\"" })),
            tile("chart", "Progress S-curve", 8, 3, json!({ "plan": "scurve" })),
            tile("ring", "Complete", 4, 3, json!({ "formula": "=PercentComplete" })),
            tile("table", "Budget", 6, 3, json!({ "src": "='Weekly report'!E3:G7" })),
            tile("countries", "Where the team comes from", 6, 3, json!({ "formula": "=Resources[Country]" })),
        ] }],
        "reports": [{ "name": "Weekly client report", "sections": [
            { "kind": "kpis", "title": "Key figures" }, { "kind": "scurve", "title": "Progress" }, { "kind": "completed", "title": "Completed this week" },
            { "kind": "lookahead", "title": "Next two weeks" }, { "kind": "milestones", "title": "Milestones" }, { "kind": "text", "title": "Notes", "text": "Furniture delivery is the main risk; see the risk register." } ] }],
        "filters": [{ "name": "Construction trades", "rules": [{ "field": "code:Trade", "test": "equals", "value": "Civil", "value2": "", "or": false }, { "field": "code:Trade", "test": "equals", "value": "Finishes", "value2": "", "or": true }], "summaries": true, "highlight": false }],
        "cf": [{ "field": "c:text1", "test": "equals", "value": "Watch", "value2": "", "color": "#C0392B", "fill": "#FDECEA", "bold": true }],
        "tour": tour(),
    });
    doc.p.ui = ui;
    doc.p.columns = vec![("mode".into(), 34), ("code".into(), 60), ("name".into(), 250), ("dur".into(), 70), ("start".into(), 88), ("finish".into(), 88), ("pct".into(), 60), ("preds".into(), 80), ("res".into(), 160), ("c:number1".into(), 70), ("c:text1".into(), 64), ("cost".into(), 90)];
    doc.p.setup_done = true;
    doc.p
}

/// One note per view, keyed as the window names its views.
fn tour() -> serde_json::Value {
    let notes: [(&str, &str); 22] = [
        ("setup", "This plan is a small office fit-out, filled in from start to finish so you can see every part of Datum Plan at work. Open each view in the sidebar, top to bottom; this note changes with the view.\n\nSetup holds the project's facts: name, code FIT-01, client, manager, location, and the activity ID prefix (A). The start date is a Monday five weeks ago and the status date is last Friday, so the plan is in the middle of the work."),
        ("gantt", "The Gantt chart is where the plan is built. Each row is a task: indent a row (Alt+Shift+→) to put it under a summary such as Design or Construction.\n\n• Links: Predecessors \"17SS+2d\" starts Electrical first fix 2 days after the partition walls (row 17) start; \"19FS+1d\" leaves a day for the ceilings to set before painting.\n• Milestones (0 days) mark approvals and handover; three are on the timeline above.\n• Furniture order has a Start No Earlier Than; Handover has a deadline (green arrow).\n• Permit review is 7 elapsed days (it runs over the weekend).\n• Feature wall is inactive: kept, but not scheduled.\n• Weekly site meeting is a recurring task (Task › Recurring).\n• Days left and Health are your own columns with formulas; Health turns red with conditional formatting (Format › Conditional Formatting)."),
        ("tracking", "The Tracking Gantt compares today's plan with the baseline (grey bars), saved before any work started (Project › Set Baseline).\n\nFurniture manufacture is at 35 % when the baseline expected more, and the partitions are half done, so their bars run late against the grey ones and the finish moves. % labels show how far each task has got."),
        ("network", "The Network Diagram draws the same logic as boxes and arrows. Red boxes are critical: delay one and the handover moves. Try Format › Box style, or bands by WBS."),
        ("calendar", "The Calendar view shows the tasks on a month grid. Working time comes from Project › Change Working Time: this plan's Standard calendar has a public holiday in week 6, and a Six-day week calendar is given to Painting (Task Calendar)."),
        ("taskusage", "Task Usage spreads each task's hours over the days: who works on it and how much. The Builder works 300 % (three people) on Strip out."),
        ("ressheet", "The Resources sheet lists the team with rates, groups and countries (the flags on the dashboard). Paint is a material (12 per litre, 40 litres on Painting); Permit fee is a cost resource (1,500 on the permit review).\n\nDouble-click Electrician › Working Time: two days of annual leave. Resource calendars are on (Project › Schedule Options), so Electrical first fix works round the leave; the permit review ignores resource calendars."),
        ("resusage", "Resource Usage shows each person's hours by day. Red means more than they can give: use Resource › Level to move work, or look at Res. Usage by trade."),
        ("manpower", "Manpower counts people on site per day by trade (the resource groups): what you would show in a site meeting or a tender."),
        ("team", "The Team Planner shows each person's tasks on one line. Drag a task to another person or day to reassign it."),
        ("sheets", "Sheets work like Excel inside the plan. The Weekly report reads the plan by name (=ProjectFinish, =PercentComplete, =SPI), counts late tasks with =COUNTIF(Tasks[Late],TRUE), lists the milestones with FILTER, and keeps a small budget by item.\n\nClick a cell to see its formula; type = and click Plan Data to pick a field. Nothing is copied by hand: the numbers follow the plan."),
        ("update", "Update is where progress goes in each week. This plan was updated five times: Update Project marked the work done to each Friday, then the real differences were typed in (furniture manufacture at 35 %, the partitions behind).\n\nPartition walls use a rule of credit: four weighted steps (Set out 10 %, Studs 40 %, Boards 35 %, Finish 15 %), so ticking a step gives the physical % complete. Close period stores each week for the S-curves."),
        ("lookahead", "The Look-ahead lists the next weeks' work and the constraint log: what each task still needs before it can start (socket positions from the client, paint colours). Add items with + and close them when they arrive."),
        ("dashboard", "Project is the built-in dashboard: key figures, the S-curve across the page and the cards that have data (Cards ▾ to choose).\n\nClient dashboard is your own: tiles with formulas (SPI turns red below 1), a progress ring, the budget from the Weekly report sheet and the team's countries. Edit layout to move or resize tiles."),
        ("progress", "The weekly progress report: figures for the week against the last one, what was done and what is next, and the notes written for week 5 (summary, issues, actions). Print it or save it as HTML for the client."),
        ("scurve", "S-curves: planned (the baseline) against actual, built from the five closed weeks, and the forecast to the finish. Switch the weight between duration, cost, hours and physical %."),
        ("reports", "Ready-made reports: tasks, costs, resources, milestones, critical path and more. Each prints or saves as HTML."),
        ("builder", "The Report Builder makes your own report from sections. \"Weekly client report\" is saved with this plan: key figures, the S-curve, work completed, the next two weeks, milestones and a note."),
        ("checks", "Checks test the plan's quality (open ends, constraints, long tasks, negative float, missing baseline…), like a scheduling audit. Click a line to go to the tasks."),
        ("risks", "The risk register: furniture delivered late (7 days, likely) and asbestos found during strip out (5 days, less likely). Each is linked to its task, so the schedule risk analysis can use it."),
        ("simrisk", "Schedule Risk runs the plan a thousand times with each task's range (Optimistic and Pessimistic duration, set on five tasks here) and the risks' chances. It shows how likely each handover date is (P50, P80) and which tasks drive it. Click Run."),
        ("portfolio", "Portfolio lists every plan in a folder with its finish, progress and health, so you can see all your projects at once. Save this tutorial in a folder with other plans and choose that folder here.\n\nThat is the tour. Hide these notes with × (Help › Tutorial notes brings them back)."),
    ];
    serde_json::Value::Object(notes.iter().map(|(k, v)| (k.to_string(), json!(v))).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::Calc;
    #[test]
    fn the_tutorial_uses_every_view_and_schedules() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 18).unwrap_or_default();
        let p = tutorial(today);
        let c = Calc::new(&p, "%d-%b-%y", today);
        if std::env::var("SHOW").is_ok() {
            for (i, t) in p.tasks.iter().enumerate() { eprintln!("{:2} {:<44} {:>10} {:>10} {:>4}% tf {:>4} {}", i + 1, t.name, c.text(i, "start"), c.text(i, "finish"), c.text(i, "pct"), c.text(i, "tf"), c.text(i, "c:text1")) }
            let s = c.summary();
            eprintln!("status {} pct {} spi {:?} cpi {:?} cost {} finish {} bl {} late {}", s.status, s.pct, s.spi, s.cpi, s.cost, s.finish, s.bl_finish, s.late);
        }
        let s = c.summary();
        assert!(s.error.is_none() && s.spi.is_some_and(|x| x < 1.0) && s.cpi.is_some() && !p.snapshots.is_empty());
        let tour = p.ui["tour"].as_object().map(|o| o.len()).unwrap_or(0);
        assert_eq!(tour, 22, "a note for every view in the sidebar");
        assert!(p.resources.iter().any(|r| r.kind == ResKind::Material) && p.resources.iter().any(|r| r.kind == ResKind::Cost));
        assert!(p.tasks.iter().any(|t| t.inactive) && p.tasks.iter().any(|t| t.elapsed) && p.tasks.iter().any(|t| !t.steps.is_empty()));
        let v = crate::sheet::eval(&c, &[("=COUNTIF(Tasks[Late],TRUE)".into(), String::new()), ("='Weekly report'!F7".into(), String::new())]);
        assert!(!["#VALUE", "#NAME", "#REF", "#DIV", "#N/A"].iter().any(|e| format!("{v:?}").contains(e)), "{v:?}");
    }
}
