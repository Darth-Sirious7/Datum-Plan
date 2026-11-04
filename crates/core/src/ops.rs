//! The named operations: every change to a plan is one of these, whether it comes from the
//! window, the command line or an AI assistant. Each has a name, what it does, and an example.
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Serialize, Debug)]
pub struct OpInfo { pub name: &'static str, pub about: &'static str, pub example: Value }

/// The operations scripts and assistants use most. `example` is the JSON to send, e.g.
/// `{"Insert": {"at": 0, "name": "Survey", "ms": false}}`; task ids are unique ids (uid).
pub fn catalog() -> Vec<OpInfo> {
    let o = |name, about, example| OpInfo { name, about, example };
    vec![
        o("Insert", "Insert a task (or a milestone with ms: true) at a row position.", json!({"Insert": {"at": 0, "name": "Site survey", "ms": false}})),
        o("QuickAdd", "Add a task from a sentence: name, duration, links and people.", json!({"QuickAdd": {"at": 0, "text": "Pour slab 5d after 3 @Crew"}})),
        o("InsertRows", "Insert several tasks from (field, value) pairs. A preds value names task IDs (row numbers) as they were before the insert; to link new tasks to each other, use LinkSet afterwards.", json!({"InsertRows": {"at": 0, "rows": [[["name", "Design"], ["dur", "5d"]], [["name", "Build"], ["dur", "10d"], ["res", "Crew"]]]}})),
        o("Set", "Set one field of a task: name, dur, start, finish, preds, res, pct, astart, afinish, ctype, cdate, deadline, cal, notes, fixedcost, qty, qtydone, dmin, dmax…", json!({"Set": {"uid": 2, "field": "dur", "value": "4d"}})),
        o("Cells", "Set many task fields at once (like a paste).", json!({"Cells": {"cells": [[2, "pct", "50"], [3, "res", "Crew[200%]"]]}})),
        o("Delete", "Delete tasks (a summary with its subtasks).", json!({"Delete": {"uids": [5]}})),
        o("Indent", "Indent tasks under the task above.", json!({"Indent": {"uids": [3]}})),
        o("Outdent", "Outdent tasks.", json!({"Outdent": {"uids": [3]}})),
        o("Move", "Move tasks up or down.", json!({"Move": {"uids": [3], "up": true}})),
        o("Link", "Link the tasks in order, finish to start.", json!({"Link": {"uids": [2, 3]}})),
        o("LinkSet", "Add or change one link (kind FS, SS, FF or SF; lag like 2d, 4h or 50%), or remove it with kind null.", json!({"LinkSet": {"pred": 2, "succ": 3, "kind": "SS", "lag": "2d"}})),
        o("Unlink", "Remove all links of tasks.", json!({"Unlink": {"uids": [3]}})),
        o("Split", "Pause a task: after this many working minutes of work, a gap of this many.", json!({"Split": {"uid": 2, "after": 480, "gap": 960}})),
        o("Level", "Level resources (all tasks with uids [], or some), or clear levelling.", json!({"Level": {"uids": [], "clear": false}})),
        o("Baseline", "Save baseline n (0–10) for all tasks (uids []) or some; clear: true removes it.", json!({"Baseline": {"uids": [], "clear": false, "n": 0}})),
        o("UpdateProject", "Mark work complete through a date.", json!({"UpdateProject": {"through": "2026-11-13", "reschedule": false}})),
        o("Snapshot", "Close a period: store every task's progress for trends and S-curves.", json!({"Snapshot": {"label": "Week 46", "comment": ""}})),
        o("ProjectSet", "Set a plan setting from JSON: name, status, company, client, currency, periods, opts…", json!({"ProjectSet": {"field": "status", "value": "2026-11-13"}})),
        o("ResInsert", "Add a resource (a person, crew, material or cost).", json!({"ResInsert": {"name": "Electrician"}})),
        o("ResSet", "Set a resource field: name, kind, group (its trade), max, rate, ovt, per_use, country, cal…", json!({"ResSet": {"uid": 1, "field": "rate", "value": "65"}})),
        o("Risk", "Add or change a risk (id 0 = new); days is the delay it adds to its tasks if it happens.", json!({"Risk": {"id": 0, "risk": {"name": "Late permit", "prob": 3, "impact": 4, "status": "Open", "kind": "Threat", "tasks": [2], "days": 5}}})),
        o("Log", "Add or change a constraint-log entry (id 0 = new): what a task needs before it starts.", json!({"Log": {"id": 0, "item": {"uid": 3, "kind": "Material", "text": "Cable drums on site", "owner": "Site", "need": "2026-11-20", "status": "Open"}}})),
        o("Sheet", "Add a spreadsheet tab (k null), rename one (k and the sheet with its new name) or delete one (sheet null).", json!({"Sheet": {"k": null, "sheet": {"name": "Dashboard"}}})),
        o("SheetCells", "Type into cells of tab k: values or spreadsheet formulas. Formulas read the plan as tables and names: Tasks[Finish], Resources[Country], ProjectFinish, SPI. fmt (instead of values) sets a number format such as 0.0%, #,##0, dd-mmm-yy, money or flag.", json!({"SheetCells": {"k": 0, "cells": [["A1", "Late tasks"], ["B1", "=COUNTIF(Tasks[Late],TRUE)"]], "fmt": null}})),
        o("SheetInsert", "Insert (count > 0) or delete (count < 0) rows or columns (cols: true) of tab k at a 0-based position; formulas follow.", json!({"SheetInsert": {"k": 0, "cols": false, "at": 2, "count": 1}})),
        o("SheetShift", "Insert (or with del: delete) the cells of a range on tab k, moving the cells below (down: true) or to the right; formulas follow.", json!({"SheetShift": {"k": 0, "range": "B2:C3", "down": true, "del": false}})),
        o("SheetNotes", "Put notes on cells of tab k (empty text deletes the note).", json!({"SheetNotes": {"k": 0, "cells": [["B2", "Price confirmed by the vendor"]]}})),
        o("ApplyCredit", "Give tasks the weighted steps of a rule of credit (Concrete, Cable, Piping…).", json!({"ApplyCredit": {"uids": [3], "rule": "Cable"}})),
    ]
}

#[cfg(test)]
mod tests {
    use crate::{doc::{Doc, Op}, templates};

    #[test]
    fn every_example_is_a_working_operation() {
        let names: Vec<&str> = super::catalog().iter().map(|o| o.name).collect();
        let mut dedup = names.clone();
        dedup.sort();
        dedup.dedup();
        assert_eq!(dedup.len(), names.len(), "names are unique");
        for info in super::catalog() {
            let op: Op = serde_json::from_value(info.example.clone()).unwrap_or_else(|e| panic!("{}: {e}", info.name));
            let mut d = Doc::new(templates::build(&templates::TEMPLATES[0], chrono::NaiveDate::from_ymd_opt(2026, 11, 2).unwrap_or_default()));
            if info.name.starts_with("Sheet") && info.name != "Sheet" { let _ = d.apply(Op::Sheet { k: None, sheet: Some(crate::model::SheetData { name: "S".into(), ..Default::default() }) }, "%Y-%m-%d"); }
            if info.name == "ResSet" { let _ = d.apply(Op::ResInsert { name: "Electrician".into() }, "%Y-%m-%d"); }
            let uid1 = d.p.resources.last().map_or(1, |r| r.uid);
            let op = if let Op::ResSet { field, value, .. } = op { Op::ResSet { uid: uid1, field, value } } else { op };
            d.apply(op, "%Y-%m-%d").unwrap_or_else(|e| panic!("{}: {e}", info.name));
        }
    }
}
