//! A master's setup given to its subplans (Project › Apply Setup to Subplans): the parts ticked
//! are copied, the plan's own name, code and description never are, and its own logos are kept.
use crate::model::{Project, MAX_PICTURES, MAX_PICTURE_LEN};
use std::collections::HashMap;

/// The parts of a setup that can be shared: key, label, what it holds.
pub const PARTS: [(&str, &str, &str); 8] = [
    ("details", "Project details", "Client, manager, company, location and the company logo"),
    ("logos", "Logos and pictures", "The master's picture library; the plan's own pictures stay"),
    ("print", "Print header, footer & page setup", "Pictures, paper, margins, footer; the plan keeps its own title and subtitle"),
    ("calendars", "Calendars & working time", "The master's calendars (same names are replaced) and its project calendar"),
    ("currency", "Currency", "Symbol, code and decimals"),
    ("periods", "Reporting periods", "Week end, month end and how progress is weighted"),
    ("codes", "Activity codes & custom fields", "Their definitions and values lists (same names are replaced)"),
    ("modules", "Modules", "The modules switched on (Project › Modules) and their settings; each plan keeps its own registers"),
];

/// Copies the ticked parts of `from` into `p`.
pub fn adopt(p: &mut Project, from: &Project, parts: &[String]) -> Result<(), String> {
    if let Some(x) = parts.iter().find(|x| !PARTS.iter().any(|(k, _, _)| k == x)) { return Err(format!("Unknown setup part: {x}.")) }
    let has = |k: &str| parts.iter().any(|x| x == k);
    if has("details") {
        p.client = from.client.clone(); p.manager = from.manager.clone(); p.company = from.company.clone();
        p.location = from.location.clone(); p.logo = from.logo.clone();
    }
    // Pictures: the master's (all of them, or the ones its print header uses), matched by content.
    let page = from.ui.get("page");
    let used: Vec<u32> = page.map(|pg| pg.to_string()).map(|s| s.match_indices("@pic:").filter_map(|(i, _)| s[i + 5..].split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()).collect()).unwrap_or_default();
    let mut ids: HashMap<u32, u32> = HashMap::new();
    for x in from.pictures.iter().filter(|x| has("logos") || (has("print") && used.contains(&x.id))) {
        if let Some(y) = p.pictures.iter().find(|y| y.data == x.data) { ids.insert(x.id, y.id); continue }
        if p.pictures.len() >= MAX_PICTURES || x.data.len() > MAX_PICTURE_LEN { continue }
        let id = p.pictures.iter().map(|y| y.id).max().unwrap_or(0) + 1;
        p.pictures.push(crate::model::Picture { id, ..x.clone() });
        ids.insert(x.id, id);
    }
    if has("print") {
        if let Some(pg) = page {
            let mut pg = pg.clone();
            // The plan keeps its own title, subtitle and row sizes (they name its own tasks).
            let own = p.ui.get("page").cloned().unwrap_or_default();
            for k in ["title", "subtitle", "rowPx"] { if let Some(o) = pg.as_object_mut() { match own.get(k) { Some(v) => { o.insert(k.into(), v.clone()); } None => { o.remove(k); } } } }
            let text = ids.iter().fold(pg.to_string(), |s, (a, b)| s.replace(&format!("\"@pic:{a}\""), &format!("\"@pic:#{b}\"")));
            pg = serde_json::from_str(&text.replace("\"@pic:#", "\"@pic:")).map_err(|e| e.to_string())?;
            if !p.ui.is_object() { p.ui = serde_json::json!({}); }
            p.ui["page"] = pg;
        }
    }
    if has("calendars") {
        for c in &from.calendars {
            match p.calendars.iter_mut().find(|x| x.name == c.name) { Some(x) => *x = c.clone(), None => p.calendars.push(c.clone()) }
        }
        p.calendar = from.calendar.clone();
    }
    if has("currency") { p.currency = from.currency.clone(); }
    if has("periods") { p.periods = from.periods.clone(); }
    if has("codes") {
        for c in &from.codes { match p.codes.iter_mut().find(|x| x.name == c.name) { Some(x) => *x = c.clone(), None => p.codes.push(c.clone()) } }
        for f in &from.fields { match p.fields.iter_mut().find(|x| x.key == f.key) { Some(x) => *x = f.clone(), None => p.fields.push(f.clone()) } }
    }
    if has("modules") {
        for m in &from.modules { if !p.modules.contains(m) { p.modules.push(m.clone()) } }
        p.modules.sort_by_key(|m| crate::contract::MODULES.iter().position(|x| x.0 == m.as_str()));
        p.contract.warn_days = from.contract.warn_days;
        let (r, f) = (&mut p.documents, &from.documents);
        (r.p_date, r.p_unit, r.review_days, r.turnaround_days, r.review_cal, r.work_cal) = (f.p_date, f.p_unit.clone(), f.review_days, f.turnaround_days, f.review_cal.clone(), f.work_cal.clone());
        (r.weights, r.codes, r.purposes, r.flow) = (f.weights.clone(), f.codes.clone(), f.purposes.clone(), f.flow.clone());
        // A calendar the rules count on comes along.
        for name in [&f.review_cal, &f.work_cal] {
            if let Some(c) = from.calendars.iter().find(|c| &c.name == name) { if !p.calendars.iter().any(|x| x.name == c.name) { p.calendars.push(c.clone()) } }
        }
    }
    Ok(())
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::doc::{Doc, Op};
    use crate::model::Picture;
    use chrono::NaiveDate;

    fn plan(name: &str) -> Doc { Doc::new(Project::new(name, NaiveDate::from_ymd_opt(2026, 11, 2).unwrap())) }
    const F: &str = "%d/%m/%Y";

    #[test]
    fn a_masters_setup_goes_to_a_subplan_without_its_name_or_code() {
        let png = |n: u8| format!("data:image/png;base64,iVBORw0KGgo{n}=");
        let mut m = plan("Plant upgrade");
        m.p.code = "PU".into(); m.p.client = "Client Co".into(); m.p.location = "Site A".into(); m.p.company = "Our company".into();
        m.p.currency.symbol = "EUR".into(); m.p.periods.week_end = 3;
        m.p.pictures = vec![Picture { id: 1, name: "Client".into(), role: "Client".into(), data: png(1) }, Picture { id: 2, name: "Us".into(), role: "Us".into(), data: png(2) }];
        m.p.ui = serde_json::json!({ "page": { "paper": "A3", "title": "Master title", "subtitle": "Master sub", "pics": [[{ "v": "@pic:2", "h": 40 }], [], [{ "v": "@pic:1", "h": 30 }]] } });
        let mut s = plan("Compressor package");
        s.p.code = "PU-1".into();
        s.p.pictures = vec![Picture { id: 1, name: "Vendor".into(), role: "Vendor".into(), data: png(9) }];
        s.p.ui = serde_json::json!({ "page": { "subtitle": "Compressor package" } });
        let parts: Vec<String> = ["details", "logos", "print", "currency", "periods"].map(String::from).to_vec();
        s.apply(Op::AdoptSetup { from: Box::new(m.p.clone()), parts: parts.clone() }, F).unwrap();
        let p = &s.p;
        assert_eq!((p.name.as_str(), p.code.as_str()), ("Compressor package", "PU-1"), "name and code stay");
        assert_eq!((p.client.as_str(), p.location.as_str(), p.company.as_str(), p.currency.symbol.as_str(), p.periods.week_end), ("Client Co", "Site A", "Our company", "EUR", 3));
        assert_eq!(p.pictures.len(), 3, "the plan's own vendor logo stays and the master's two are added");
        let id = |d: &str| p.pictures.iter().find(|x| x.data == d).unwrap().id;
        assert_eq!(p.ui["page"]["pics"][0][0]["v"], format!("@pic:{}", id(&png(2))), "the header points at the plan's copy of the picture");
        assert_eq!(p.ui["page"]["pics"][2][0]["v"], format!("@pic:{}", id(&png(1))));
        assert_eq!(p.ui["page"]["paper"], "A3");
        assert_eq!(p.ui["page"]["subtitle"], "Compressor package", "the plan keeps its own subtitle");
        assert!(p.ui["page"].get("title").is_none(), "and its own (empty) title");
        // A second time adds nothing more; one Undo takes it back.
        s.apply(Op::AdoptSetup { from: Box::new(m.p.clone()), parts }, F).unwrap();
        assert_eq!(s.p.pictures.len(), 3);
        s.undo();
        assert_eq!(s.p.client, "");
        assert!(adopt(&mut s.p.clone(), &m.p, &["name".into()]).is_err());
    }
}
