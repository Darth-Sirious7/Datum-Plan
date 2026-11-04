//! Prints engine output for a template as JSON fixtures for the UI tests (tests_ui/run.cjs).
//! `dump guide` prints a plan in progress (baseline, two closed periods, risks, constraints) for
//! the guide's screenshots (tests_ui/guide_shots.cjs); `dump tutorial` the tutorial example
//! (tests_ui/tutorial_fixture.json, for the guide's picture of the tutorial notes).
use datum_core::{doc::{Doc, Op}, fields::Calc, query::Query, *};
use chrono::NaiveDate;

fn guide_plan() -> (Project, NaiveDate) {
    let d = |m, day| NaiveDate::from_ymd_opt(2026, m, day).unwrap();
    let mut doc = Doc::new(templates::build(&templates::TEMPLATES[0], d(11, 2)));
    let f = "%Y-%m-%d";
    let mut op = |o: Op| doc.apply(o, f).unwrap_or_else(|e| panic!("{e}"));
    op(Op::Baseline { uids: vec![], clear: false, n: 0 });
    op(Op::UpdateProject { through: "2026-11-20".into(), reschedule: false });
    op(Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-11-20") });
    op(Op::Snapshot { label: "Week ending 20 Nov".into(), comment: String::new() });
    op(Op::UpdateProject { through: "2026-12-04".into(), reschedule: false });
    op(Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-12-04") });
    op(Op::Snapshot { label: "Week ending 4 Dec".into(), comment: String::new() });
    let uid = |name: &str, p: &Project| p.tasks.iter().find(|t| t.name.contains(name)).map(|t| t.uid).unwrap_or(1);
    // The 3D CAD model is running late: less done than planned.
    let cad = uid("3D CAD", &doc.p);
    doc.apply(Op::Set { uid: cad, field: "pct".into(), value: "45".into() }, f).unwrap_or_else(|e| panic!("{e}"));
    let (parts, tests) = (uid("Parts lead", &doc.p), uid("Environmental", &doc.p));
    for o in [
        Op::Risk { id: 0, risk: Some(Risk { name: "Supplier delivers parts late".into(), cause: "Single supplier".into(), prob: 3, impact: 4, owner: "Buyer".into(), response: "Qualify a second supplier".into(), status: "Open".into(), tasks: vec![parts], kind: "Threat".into(), days: 10.0, ..Default::default() }) },
        Op::Risk { id: 0, risk: Some(Risk { name: "Test lab not available".into(), cause: "Shared lab".into(), prob: 2, impact: 3, owner: "Test lead".into(), response: "Book the lab now".into(), status: "Open".into(), tasks: vec![tests], kind: "Threat".into(), days: 5.0, ..Default::default() }) },
        Op::Log { id: 0, item: Some(LogItem { uid: uid("Calculations", &doc.p), kind: "Drawing or information".into(), text: "Load cases from the client".into(), owner: "Client".into(), need: Some(d(12, 3)), status: "Open".into(), ..Default::default() }) },
        Op::Log { id: 0, item: Some(LogItem { uid: uid("Drawings", &doc.p), kind: "Labour".into(), text: "Second CAD designer".into(), owner: "Engineering manager".into(), need: Some(d(12, 9)), status: "Open".into(), ..Default::default() }) },
    ] { doc.apply(o, f).unwrap_or_else(|e| panic!("{e}")) }
    // Three milestones on the timeline.
    let on: Vec<(u32, bool)> = doc.p.tasks.iter().map(|t| (t.uid, ["Requirements signed off", "Critical design review", "Released to manufacturing"].contains(&t.name.as_str()))).collect();
    for (uid, yes) in on { doc.apply(Op::Set { uid, field: "timeline".into(), value: if yes { "Yes" } else { "No" }.into() }, f).unwrap_or_else(|e| panic!("{e}")) }
    // A weekly report on a sheet: plan numbers by name, a live milestone list and the vendors with their flags.
    let cells = [
        ("A1", "Weekly report"), ("A3", "Project finish"), ("B3", "=ProjectFinish"), ("A4", "Baseline finish"), ("B4", "=BaselineFinish"), ("A5", "% complete"), ("B5", "=PercentComplete"),
        ("A6", "Late tasks"), ("B6", "=COUNTIF(Tasks[Late],TRUE)"), ("A7", "SPI"), ("B7", "=SPI"),
        ("A9", "Milestone"), ("B9", "Finish"), ("C9", "% Complete"), ("A10", "=FILTER(CHOOSECOLS(Tasks,3,8,9),Tasks[Milestone]=TRUE,\"None\")"),
        ("E3", "Vendor"), ("F3", "Country"), ("G3", "Scope"), ("E4", "Northwind Castings"), ("F4", "DE"), ("G4", "Housings"), ("E5", "Pacific Circuits"), ("F5", "TW"), ("G5", "Control boards"),
        ("E6", "Kestrel Test Labs"), ("F6", "GB"), ("G6", "Environmental tests"), ("H3", "Order ($k)"), ("H4", "120"), ("H5", "85"), ("H6", "40"), ("E7", "Total orders"), ("G7", "=COUNTA(G4:G6)"),
    ];
    let mut fmt: Vec<(String, String)> = [("B7", "0.00"), ("F4", "flag"), ("F5", "flag"), ("F6", "flag")].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    for r in 10..20 { fmt.push((format!("B{r}"), "date".into())); fmt.push((format!("C{r}"), "0\"%\"".into())); }
    let style = ["A1", "A9", "B9", "C9", "E3", "F3", "G3", "H3"].iter().map(|a| (a.to_string(), "b".to_string())).collect();
    // Where the people come from (flags on dashboards), and two dashboards like the app's templates.
    let res: Vec<u32> = doc.p.resources.iter().map(|r| r.uid).collect();
    for (uid, c) in res.iter().zip(["DE", "IN", "GB", "DE", "CN", "AE", "IN", "US"]) { doc.apply(Op::ResSet { uid: *uid, field: "country".into(), value: c.into() }, f).unwrap_or_else(|e| panic!("{e}")) }
    let tile = |kind: &str, title: &str, w: u8, h: u8, extra: serde_json::Value| { let mut t = serde_json::json!({ "id": title, "kind": kind, "title": title, "w": w, "h": h }); t.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone()); t };
    let dashes = serde_json::json!([
        { "name": "Client dashboard", "tiles": [
            tile("number", "Actual progress", 3, 1, serde_json::json!({ "formula": "=PercentComplete", "sub": "=\"Planned \"&TEXT(PlannedPercent,\"0\")&\"%\"" })),
            tile("number", "Forecast finish", 3, 1, serde_json::json!({ "formula": "=ProjectFinish", "sub": "=IF(BaselineFinish=\"\",\"No baseline yet\",\"Baseline \"&TEXT(BaselineFinish,\"dd-mmm-yy\"))" })),
            tile("number", "SPI", 3, 1, serde_json::json!({ "formula": "=SPI", "fmt": "0.00", "good": [">=", 1], "sub": "Schedule performance index" })),
            tile("number", "Late tasks", 3, 1, serde_json::json!({ "formula": "=COUNTIF(Tasks[Late],TRUE)", "good": ["<=", 0], "sub": "=\"of \"&TaskCount&\" tasks\"" })),
            tile("chart", "Progress S-curve", 8, 3, serde_json::json!({ "plan": "scurve" })),
            tile("ring", "Complete", 4, 3, serde_json::json!({ "formula": "=PercentComplete", "sub": "=\"Planned \"&TEXT(PlannedPercent,\"0\")&\"%\"" })),
            tile("table", "Milestones", 6, 3, serde_json::json!({ "plan": "milestones" })),
            tile("countries", "Where the team comes from", 6, 3, serde_json::json!({ "formula": "=Resources[Country]" })),
        ] },
        { "name": "Vendors", "tiles": [
            tile("table", "Vendors", 6, 3, serde_json::json!({ "src": "=IFNA(VSTACK({\"Name\",\"Group\",\"Country\"},FILTER(CHOOSECOLS(Resources,1,3,9),Resources[Country]<>\"\",\"None yet\")),\"\")" })),
            tile("chart", "Orders by vendor", 6, 3, serde_json::json!({ "src": "=HSTACK('Weekly report'!E4:E6,'Weekly report'!H4:H6)", "chart": "doughnut" })),
        ] },
    ]);
    doc.apply(Op::Ui { key: "dashboards".into(), value: dashes.clone() }, f).unwrap_or_else(|e| panic!("{e}"));
    doc.p.sheets.push(SheetData { name: "Weekly report".into(), cells: cells.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), fmt: fmt.into_iter().collect(), widths: [(0, 230), (4, 170), (5, 150), (6, 160)].into_iter().collect(), style, ..Default::default() });
    (doc.p, d(12, 4))
}

/// Every dashboard formula's result by "format\u{1}formula", for the window tests' mocked dash_eval.
fn dash_map(c: &Calc) -> serde_json::Value {
    let mut items: Vec<(String, String)> = vec![];
    for d in c.p.ui.get("dashboards").and_then(|v| v.as_array()).into_iter().flatten() {
        for t in d["tiles"].as_array().into_iter().flatten() {
            let fmt = t["fmt"].as_str().unwrap_or("").to_string();
            for k in ["formula", "sub", "src"] { if let Some(f) = t[k].as_str().filter(|f| f.starts_with('=')) { items.push((f.into(), if k == "formula" { fmt.clone() } else { String::new() })) } }
        }
    }
    let r = sheet::eval(c, &items);
    serde_json::Value::Object(items.iter().zip(r).map(|((f, fmt), v)| (format!("{fmt}\u{1}{f}"), serde_json::to_value(v).unwrap())).collect())
}

/// An empty plan (no tasks) with a sheet and a dashboard of awkward tiles: errors, a deleted
/// sheet, text where a number goes, pies of negatives and zeros, one point, none, a huge range;
/// and a dashboard with no tiles. The window tests open every report on it.
fn empty_plan() -> serde_json::Value {
    let mut p = Project::new("Empty plan", NaiveDate::from_ymd_opt(2026, 11, 2).unwrap());
    let cells = [("A1", "Item"), ("B1", "Value"), ("A2", "Up"), ("B2", "5"), ("A3", "Down"), ("B3", "-3"), ("D1", "Zero"), ("E1", "0"), ("D2", "Nil"), ("E2", "0"), ("G1", "Only"), ("H1", "7")];
    p.sheets.push(SheetData { name: "Data".into(), cells: cells.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), ..Default::default() });
    let t = |kind: &str, title: &str, extra: serde_json::Value| { let mut t = serde_json::json!({ "id": title, "kind": kind, "title": title, "w": 4, "h": 2 }); t.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone()); t };
    use serde_json::json as j;
    let mut tiles = vec![
        t("number", "Divided by zero", j!({ "formula": "=1/0" })), t("number", "Deleted sheet", j!({ "formula": "=Gone!A1" })), t("number", "Text", j!({ "formula": "=\"abc\"", "fmt": "money", "good": [">=", 1] })),
        t("ring", "Ring error", j!({ "formula": "=1/0" })), t("ring", "Ring text", j!({ "formula": "=\"abc\"" })),
        t("chart", "Pie of negatives", j!({ "src": "=Data!A2:B3", "chart": "pie" })), t("chart", "Pie of zeros", j!({ "src": "=Data!D1:E2", "chart": "doughnut" })),
        t("chart", "One point", j!({ "src": "=Data!G1:H1", "chart": "line" })), t("chart", "No points", j!({ "src": "=Data!K40:L50" })), t("chart", "Chart of a deleted sheet", j!({ "src": "=Gone!A1:B3" })),
        t("table", "Table of a deleted sheet", j!({ "src": "=Gone!A1:B3" })), t("table", "Huge range", j!({ "src": "=Data!A1:Z5000" })),
        t("countries", "Countries", j!({ "formula": "=Resources[Country]" })), t("text", "Note", j!({ "text": "" })), t("image", "No picture", j!({})),
    ];
    for k in ["scurve", "periodbars", "phases", "costev", "manpower"] { tiles.push(t("chart", &format!("Plan chart {k}"), j!({ "plan": k }))) }
    for k in ["milestones", "late", "critical", "upcoming", "completed"] { tiles.push(t("table", &format!("Plan list {k}"), j!({ "plan": k }))) }
    let many: Vec<_> = (1..=210).map(|k| t("number", &format!("N{k}"), j!({ "formula": format!("={k}") }))).collect();
    p.ui["dashboards"] = j!([{ "name": "Edge cases", "tiles": tiles }, { "name": "Blank", "tiles": [] }, { "name": "Many", "tiles": many }]);
    let c = Calc::new(&p, "%d-%b-%y", NaiveDate::from_ymd_opt(2026, 11, 6).unwrap());
    let q = Query { cols: p.columns.iter().filter(|c| c.1 > 0).map(|c| c.0.clone()).collect(), ..Query::default() };
    let mut proj = serde_json::to_value(&p).unwrap();
    proj.as_object_mut().unwrap().remove("tasks");
    j!({
        "state": { "tabs": [{ "name": p.name, "path": null, "dirty": false, "moved": 0, "role": "", "group": -1, "level": 0, "project": "" }], "tab": 0, "project": proj, "summary": c.summary(), "path": null, "dirty": false, "undo": false, "redo": false, "clip": 0, "rev": 1 },
        "query": c.grid(&q), "query:timeline": c.grid(&q), "query:tracking": c.grid(&q), "scurve": report::scurve(&c, "week", "duration", None), "period_report": report::period_report(&c, "week", None),
        "periods": report::period_ends(&c, "week"), "curve": report::curve(&c, "week"), "usage": report::usage(&c, "resource", "work", "week"), "usage:group": report::usage(&c, "group", "work", "week"),
        "manpower": progress::manpower(&c, "week"), "earned_schedule": progress::earned_schedule(&c), "lookahead": progress::lookahead(&c, 3), "risk_run": null, "level_preview": level::preview(&p, &[]).ok(),
        "sheet_view": sheet::sheet_view(&c, 0), "dash_map": dash_map(&c), "network": network::layout(&c, &[]), "link_targets": [], "link_go": [], "subplan_rows": [], "link_users": [], "link_states": [], "query_subs": c.grid(&q),
        "resources": c.res_grid(&["name", "kind", "label", "initials", "group", "max", "rate", "ovt", "per_use", "cal", "code", "country", "email", "work", "cost"].map(String::from)),
    })
}

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    let tday = NaiveDate::from_ymd_opt(2026, 11, 18).unwrap();
    let (p, today) = if arg == "guide" { guide_plan() } else if arg == "tutorial" { (tutorial::tutorial(tday), tday) } else {
        let t = &templates::TEMPLATES[2];
        let mut p = templates::build(t, NaiveDate::from_ymd_opt(2026, 10, 26).unwrap());
        // A vendor list on a sheet: flags, a total and a live count from the plan.
        let cells = [("A1", "Vendor"), ("B1", "Country"), ("C1", "Order"), ("A2", "Steel Co"), ("B2", "DE"), ("C2", "1200"), ("A3", "Pipe Works"), ("B3", "cn"), ("C3", "800"), ("C4", "=SUM(C2:C3)"), ("E1", "=ROWS(Tasks)")];
        let fmt = [("B2", "flag"), ("B3", "flag")];
        p.sheets.push(SheetData { name: "Vendors".into(), cells: cells.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), fmt: fmt.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), ..Default::default() });
        (p, NaiveDate::from_ymd_opt(2026, 10, 24).unwrap())
    };
    let guide = arg == "guide" || arg == "tutorial";
    let c = Calc::new(&p, "%d-%b-%y", today);
    let q = Query { cols: p.columns.iter().filter(|c| c.1 > 0).map(|c| c.0.clone()).collect(), ..Query::default() };
    let mut g = c.grid(&q);
    if !guide {
        // One task waits for a task in another plan, and another plan waits for one.
        let k = g.rows.iter().position(|r| r.kind == "task").unwrap_or(0);
        g.rows[k].ext = vec!["Engineering › E1010 IFC drawings issued (FS+5d)".into()];
        g.rows[k].ext_moved = true;
        g.rows[k + 1].ext_out = vec!["Main schedule › A1040 Engineering complete (FS)".into()];
    }
    let rows: Vec<usize> = g.rows.iter().filter(|r| r.kind == "task").filter_map(|r| p.idx(r.uid)).collect();
    let mut tl = serde_json::to_value(&g).unwrap();
    if let Some(r) = tl["rows"].as_array_mut() { r.retain(|x| x["uid"].as_u64().and_then(|u| p.idx(u as u32)).is_some_and(|i| p.tasks[i].timeline)) }
    let tq = Query { cols: ["name", "astart", "afinish", "pct", "phys", "adur", "rem", "acost", "awork"].map(String::from).to_vec(), ..Query::default() };
    let tracking = c.grid(&tq);
    let mut proj = serde_json::to_value(&p).unwrap();
    proj.as_object_mut().unwrap().remove("tasks");
    if !guide {
        // A project: this plan is the master of two subplans, Engineering (open) and Procurement.
        proj["subplans"] = serde_json::json!([{ "file": "C:/Plans/Engineering.dplan", "name": "Engineering" }, { "file": "C:/Plans/Procurement.dplan", "name": "Procurement", "level": 1 }]);
    }
    // A master's combined view: a subplan's row and one of its tasks after the plan's rows.
    let query_subs = {
            let mut v = serde_json::to_value(&g).unwrap();
            let t = v["rows"].as_array().unwrap().iter().find(|r| r["kind"] == "task").cloned().unwrap();
            let mut head = t.clone();
            for (k, x) in [("kind", serde_json::json!("subplan")), ("name", "Engineering".into()), ("uid", 0.into()), ("kids", true.into()), ("open", true.into()), ("sub", "C:/Plans/Engineering.dplan".into())] { head[k] = x }
            let mut child = t;
            for (k, x) in [("name", "IFC drawings issued".into()), ("uid", 0.into()), ("suid", 7.into()), ("lvl", 1.into()), ("sub", serde_json::json!("C:/Plans/Engineering.dplan"))] { child[k] = x }
            let nc = g.cols.iter().position(|c| c.key == "name").unwrap_or(0);
            for (r, n) in [(&mut head, "Engineering"), (&mut child, "IFC drawings issued")] {
                r["cells"][nc] = n.into();
                for k in ["ext", "ext_moved", "ext_out"] { r.as_object_mut().unwrap().remove(k); }
            }
            v["rows"].as_array_mut().unwrap().extend([head, child]);
            v
        };
    println!("{}", serde_json::json!({
        "state": { "tabs": if guide { serde_json::json!([{ "name": p.name, "path": null, "dirty": false, "moved": 0, "role": "", "group": -1, "level": 0, "project": "" }]) } else { serde_json::json!([{ "name": p.name, "path": "C:/Plans/Main.dplan", "dirty": false, "moved": 0, "role": "master", "group": 0, "level": 0, "project": p.name }, { "name": "Engineering", "path": "C:/Plans/Engineering.dplan", "dirty": true, "moved": 2, "role": "sub", "group": 0, "level": 0, "project": p.name }]) }, "tab": 0, "project": proj, "summary": c.summary(), "path": if guide { None } else { Some("C:/Plans/Main.dplan") }, "dirty": false, "undo": false, "redo": false, "clip": 0, "rev": 1 },
        "query": g, "query:timeline": tl, "query:tracking": tracking, "scurve": report::scurve(&c, "week", "duration", None), "period_report": report::period_report(&c, "week", None),
        "periods": report::period_ends(&c, "week"), "curve": report::curve(&c, "week"), "usage": report::usage(&c, "resource", "work", "week"), "usage:group": report::usage(&c, "group", "work", "week"),
        "risk_run": risk::simulate(&p, &risk::SimOpts { iterations: 300, ..Default::default() }).unwrap(), "level_preview": level::preview(&p, &[]).unwrap(),
        "manpower": progress::manpower(&c, "week"), "earned_schedule": progress::earned_schedule(&c), "lookahead": progress::lookahead(&c, 3), "credit_library": progress::credit_library(),
        "sheet_view": sheet::sheet_view(&c, 0), "formula_shift": ["=C3*2"], "dash_map": dash_map(&c),
        "link_targets": [{ "tab": 1, "name": "Engineering", "path": "C:/Plans/Engineering.dplan", "tasks": [[1, "E1000", "Basic engineering", 0, true, c.s.start, c.s.start + 20 * cal::DAY], [2, "E1010", "IFC drawings issued", 1, false, c.s.start + 20 * cal::DAY, c.s.start + 20 * cal::DAY]] }],
        "link_go": [{ "dir": "in", "label": "Engineering › E1010 IFC drawings issued (FS+5d)", "file": "C:/Plans/Engineering.dplan", "uid": 2, "moved": true }, { "dir": "out", "label": "Main schedule › A1040 Engineering complete (FS)", "file": "C:/Plans/Main schedule.dplan", "uid": 5, "moved": false }],
        "subplan_rows": [{ "file": "C:/Plans/Engineering.dplan", "name": "Engineering", "level": 0, "found": true, "open": true, "tab": 1, "start": c.s.start, "finish": c.s.start + 40 * cal::DAY, "pct": 25.0, "tasks": 19 }, { "file": "C:/Plans/Procurement.dplan", "name": "Procurement", "level": 1, "found": true, "open": false, "tab": -1, "start": c.s.start, "finish": c.s.start + 90 * cal::DAY, "pct": 0.0, "tasks": 15 }],
        "query_subs": query_subs,
        "link_users": [{ "uid": p.tasks[2].uid, "task": p.tasks[2].name, "tab": 1, "by": "Main schedule › A1040 Engineering complete (FS)" }],
        "link_states": [{ "uid": p.tasks[1].uid, "task": p.tasks[1].name, "k": 0, "link": { "file": "C:/Plans/Engineering.dplan", "uid": 2, "kind": "FS", "lag": 0, "plan": "Engineering", "code": "E1010", "name": "IFC drawings issued", "start": c.s.start, "finish": c.s.start + 10 * cal::DAY }, "now_start": c.s.start, "now_finish": c.s.start + 15 * cal::DAY, "state": "changed" }],
        "sheet_catalog": { "functions": sheet::functions(), "names": sheet::project_names(&c).into_iter().map(|(n, _, d)| (n, d)).collect::<Vec<_>>(), "tasks": sheet::task_columns(&c).into_iter().map(|x| x.1).collect::<Vec<_>>(), "resources": sheet::RES_COLS.iter().map(|x| x.1).collect::<Vec<_>>() },
        "resources": c.res_grid(&["name", "kind", "label", "initials", "group", "max", "rate", "ovt", "per_use", "cal", "code", "country", "email", "work", "cost"].map(String::from)), "network": network::layout(&c, &rows), "empty": if guide { serde_json::Value::Null } else { empty_plan() }, "field_list": { "task": fields::task_fields(), "resource": fields::resource_fields(), "filters": query::builtin_filters() },
    }));
}
