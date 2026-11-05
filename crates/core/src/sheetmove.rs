//! Moving or copying a sheet from one plan to another (a master's sheet into one of its subplans):
//! the sheet with the pictures it shows and its frozen panes, ready for `Op::AddSheets`.
use crate::doc::Op;
use crate::model::Project;

/// What leaves plan `p` with sheet `k`: the op that adds it to another plan, and the number of
/// its formulas that read other sheets of `p` (they will read the other plan's sheets of that
/// name, or show #REF!). Plan data (Tasks[…], ProjectFinish…) then reads the plan it is in.
pub fn package(p: &Project, k: usize) -> Option<(Op, usize)> {
    let s = p.sheets.get(k)?.clone();
    let pictures = p.pictures.iter().filter(|x| s.images.iter().any(|im| im.pic == x.id)).cloned().collect();
    let freeze = p.ui.get("freeze").and_then(|f| f.get(&s.name)).and_then(|v| Some((v.get(0)?.as_u64()? as u32, v.get(1)?.as_u64()? as u32)))
        .map(|(r, c)| vec![(s.name.clone(), r, c)]).unwrap_or_default();
    let others: Vec<String> = p.sheets.iter().enumerate().filter(|(j, _)| *j != k).map(|(_, x)| x.name.to_lowercase()).collect();
    let reads = s.cells.values().filter(|v| v.starts_with('=')).filter(|v| {
        let v = v.to_lowercase();
        others.iter().any(|n| v.contains(&format!("{n}!")) || v.contains(&format!("'{}'!", n.replace('\'', "''"))))
    }).count();
    Some((Op::AddSheets { sheets: vec![s], pictures, freeze }, reads))
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::doc::Doc;
    use crate::model::{Picture, SheetData, SheetImage};
    use chrono::NaiveDate;

    fn plan(name: &str) -> Doc { Doc::new(Project::new(name, NaiveDate::from_ymd_opt(2026, 11, 2).unwrap())) }

    #[test]
    fn a_sheet_moves_to_another_plan_with_its_pictures_and_panes() {
        let mut m = plan("Master");
        let png = "data:image/png;base64,iVBORw0KGgo=".to_string();
        m.apply(Op::Picture { id: 0, pic: Some(Picture { id: 0, name: "Vendor".into(), role: "Vendor".into(), data: png.clone() }) }, "%d/%m/%Y").unwrap();
        m.apply(Op::Picture { id: 0, pic: Some(Picture { id: 0, name: "Other".into(), role: "Client".into(), data: png.clone() }) }, "%d/%m/%Y").unwrap();
        let cells = [("A1", "Item"), ("B1", "=Summary!A1"), ("C1", "=COUNTA(Tasks[Name])")].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
        let s = SheetData { name: "1. Propane".into(), cells, images: vec![SheetImage { pic: 1, cell: "C3".into(), w: 40, h: 20, fit: true }], ..Default::default() };
        m.apply(Op::Sheet { k: None, sheet: Some(s) }, "%d/%m/%Y").unwrap();
        m.apply(Op::Sheet { k: None, sheet: Some(SheetData { name: "Summary".into(), ..Default::default() }) }, "%d/%m/%Y").unwrap();
        m.apply(Op::Ui { key: "freeze".into(), value: serde_json::json!({ "1. Propane": [1, 0] }) }, "%d/%m/%Y").unwrap();
        let (op, reads) = package(&m.p, 0).unwrap();
        assert_eq!(reads, 1, "one formula reads another sheet of the master");
        let mut sub = plan("Propane");
        sub.apply(Op::Picture { id: 0, pic: Some(Picture { id: 0, name: "Ours".into(), role: "Us".into(), data: png.clone() }) }, "%d/%m/%Y").unwrap();
        sub.apply(op, "%d/%m/%Y").unwrap();
        let got = &sub.p.sheets[0];
        assert_eq!(got.name, "1. Propane");
        assert_eq!(sub.p.pictures.len(), 2, "only the picture the sheet shows comes along");
        assert_eq!(sub.p.pictures.iter().find(|x| x.id == got.images[0].pic).map(|x| x.name.as_str()), Some("Vendor"));
        assert_eq!(sub.p.ui["freeze"]["1. Propane"], serde_json::json!([1, 0]));
        assert!(package(&m.p, 9).is_none());
    }
}
